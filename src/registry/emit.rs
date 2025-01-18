// SPDX-License-Identifier: Apache-2.0

//! Emit a semantic convention registry to an OTLP receiver.

use clap::Args;

use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_forge::registry::ResolvedRegistry;
use weaver_resolved_schema::attribute::Attribute;
use weaver_semconv::attribute::{
    AttributeType, Examples, PrimitiveOrArrayTypeSpec, TemplateTypeSpec,
};
use weaver_semconv::group::{GroupType, SpanKindSpec};

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::util::prepare_main_registry;
use crate::{DiagnosticArgs, ExitDirectives};

use opentelemetry::global;
use opentelemetry::trace::{SpanKind, TraceContextExt, TraceError, Tracer};
use opentelemetry::{Array, KeyValue, Value};
use opentelemetry_sdk::trace as sdktrace;

/// Parameters for the `registry emit` sub-command
#[derive(Debug, Args)]
pub struct RegistryEmitArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Policy parameters
    #[command(flatten)]
    policy: PolicyArgs,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,

    /// Write the telemetry to standard output
    #[arg(long)]
    stdout: bool,
}

// For the given attribute, return a name/value pair.
// Values are generated based on the attribute type and examples where possible.
fn get_attribute_name_value(attribute: &Attribute) -> KeyValue {
    let name = attribute.name.clone();
    match &attribute.r#type {
        AttributeType::PrimitiveOrArray(primitive_or_array) => {
            let value = match primitive_or_array {
                PrimitiveOrArrayTypeSpec::Boolean => Value::Bool(true),
                PrimitiveOrArrayTypeSpec::Int => match &attribute.examples {
                    Some(Examples::Int(i)) => Value::I64(*i),
                    Some(Examples::Ints(ints)) => Value::I64(*ints.first().unwrap_or(&42)),
                    _ => Value::I64(42),
                },
                PrimitiveOrArrayTypeSpec::Double => match &attribute.examples {
                    Some(Examples::Double(d)) => Value::F64(f64::from(*d)),
                    Some(Examples::Doubles(doubles)) => {
                        Value::F64(f64::from(*doubles.first().unwrap_or((&3.13).into())))
                    }
                    _ => Value::F64(3.15),
                },
                PrimitiveOrArrayTypeSpec::String => match &attribute.examples {
                    Some(Examples::String(s)) => Value::String(s.clone().into()),
                    Some(Examples::Strings(strings)) => Value::String(
                        strings
                            .first()
                            .unwrap_or(&"value".to_owned())
                            .clone()
                            .into(),
                    ),
                    _ => Value::String("value".into()),
                },
                PrimitiveOrArrayTypeSpec::Booleans => Value::Array(Array::Bool(vec![true, false])),
                PrimitiveOrArrayTypeSpec::Ints => match &attribute.examples {
                    Some(Examples::Ints(ints)) => Value::Array(Array::I64(ints.to_vec())),
                    Some(Examples::ListOfInts(list_of_ints)) => Value::Array(Array::I64(
                        list_of_ints.first().unwrap_or(&vec![42, 43]).to_vec(),
                    )),
                    _ => Value::Array(Array::I64(vec![42, 43])),
                },
                PrimitiveOrArrayTypeSpec::Doubles => match &attribute.examples {
                    Some(Examples::Doubles(doubles)) => {
                        Value::Array(Array::F64(doubles.iter().map(|d| f64::from(*d)).collect()))
                    }
                    Some(Examples::ListOfDoubles(list_of_doubles)) => Value::Array(Array::F64(
                        list_of_doubles
                            .first()
                            .unwrap_or(&vec![(3.13).into(), (3.15).into()])
                            .iter()
                            .map(|d| f64::from(*d))
                            .collect(),
                    )),
                    _ => Value::Array(Array::F64(vec![3.13, 3.15])),
                },
                PrimitiveOrArrayTypeSpec::Strings => match &attribute.examples {
                    Some(Examples::Strings(strings)) => Value::Array(Array::String(
                        strings.iter().map(|s| s.clone().into()).collect(),
                    )),
                    Some(Examples::ListOfStrings(list_of_strings)) => Value::Array(Array::String(
                        list_of_strings
                            .first()
                            .unwrap_or(&vec!["value1".to_owned(), "value2".to_owned()])
                            .iter()
                            .map(|s| s.clone().into())
                            .collect(),
                    )),
                    _ => Value::Array(Array::String(vec!["value1".into(), "value2".into()])),
                },
            };
            KeyValue::new(name, value)
        }
        AttributeType::Enum { members, .. } => {
            KeyValue::new(name, Value::String(members[0].value.to_string().into()))
        }
        AttributeType::Template(template_type_spec) => {
            // TODO Support examples when https://github.com/open-telemetry/semantic-conventions/issues/1740 is complete
            let value = match template_type_spec {
                TemplateTypeSpec::String => Value::String("template_value".into()),
                TemplateTypeSpec::Int => Value::I64(42),
                TemplateTypeSpec::Double => Value::F64(3.13),
                TemplateTypeSpec::Boolean => Value::Bool(true),
                TemplateTypeSpec::Strings => Value::Array(Array::String(vec![
                    "template_value1".into(),
                    "template_value2".into(),
                ])),
                TemplateTypeSpec::Ints => Value::Array(Array::I64(vec![42, 43])),
                TemplateTypeSpec::Doubles => Value::Array(Array::F64(vec![3.13, 3.15])),
                TemplateTypeSpec::Booleans => Value::Array(Array::Bool(vec![true, false])),
            };
            KeyValue::new(format!("{name}.key"), value)
        }
    }
}

/// Convert the span kind to an OTLP span kind.
/// If the span kind is not specified, return `SpanKind::Internal`.
fn otel_span_kind(span_kind: Option<&SpanKindSpec>) -> SpanKind {
    match span_kind {
        Some(SpanKindSpec::Client) => SpanKind::Client,
        Some(SpanKindSpec::Server) => SpanKind::Server,
        Some(SpanKindSpec::Producer) => SpanKind::Producer,
        Some(SpanKindSpec::Consumer) => SpanKind::Consumer,
        Some(SpanKindSpec::Internal) | None => SpanKind::Internal,
    }
}

/// Initialise a grpc OTLP exporter, sends to by default http://localhost:4317
/// but can be overridden with the standard OTEL_EXPORTER_OTLP_ENDPOINT env var.
fn init_tracer_provider() -> Result<sdktrace::TracerProvider, TraceError> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint("http://localhost:4317")
        .build()?;
    Ok(sdktrace::TracerProvider::builder()
        .with_resource(Resource::new(vec![KeyValue::new("service.name", "weaver")])) // TODO meta semconv!
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .build())
}

/// Initialise a stdout exporter for debug
fn init_stdout_tracer_provider() -> sdktrace::TracerProvider {
    sdktrace::TracerProvider::builder()
        .with_resource(Resource::new(vec![KeyValue::new("service.name", "weaver")])) // TODO meta semconv!
        .with_simple_exporter(opentelemetry_stdout::SpanExporter::default())
        .build()
}

/// Uses the global tracer_provider to emit a single trace for all the defined
/// spans in the registry
fn emit_trace_for_registry(registry: &ResolvedRegistry, registry_path: &str) {
    let tracer = global::tracer("weaver");
    // Start a parent span here and use this context to create child spans
    tracer.in_span("weaver.emit", |cx| {
        let span = cx.span();
        span.set_attribute(KeyValue::new(
            "weaver.registry_path", // TODO meta semconv!
            registry_path.to_owned(),
        ));

        // Emit each span to the OTLP receiver.
        for group in registry.groups.iter() {
            if group.r#type == GroupType::Span {
                let _span = tracer
                    .span_builder(group.id.clone())
                    .with_kind(otel_span_kind(group.span_kind.as_ref()))
                    .with_attributes(group.attributes.iter().map(get_attribute_name_value))
                    .start_with_context(&tracer, &cx);
            }
        }
    });
}

/// Emit all spans in the resolved registry.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &RegistryEmitArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    logger.log("Weaver Registry Emit");
    logger.loading(&format!("Resolving registry `{}`", args.registry.registry));

    let mut diag_msgs = DiagnosticMessages::empty();

    let (registry, _) =
        prepare_main_registry(&args.registry, &args.policy, logger.clone(), &mut diag_msgs)?;

    logger.loading(&format!("Emitting registry `{}`", args.registry.registry));
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async {
        let tracer_provider = if args.stdout {
            logger.mute();
            init_stdout_tracer_provider()
        } else {
            init_tracer_provider().expect("OTLP Tracer Provider must be created")
        };
        let _ = global::set_tracer_provider(tracer_provider.clone());

        emit_trace_for_registry(&registry, &args.registry.registry.to_string());

        global::shutdown_tracer_provider();
    });
    logger.success(&format!("Emitted registry `{}`", args.registry.registry));

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: args.stdout,
    })
}

#[cfg(test)]
mod tests {
    use opentelemetry::{global, Array, Value};
    use opentelemetry_sdk::{trace as sdktrace, Resource};
    use weaver_common::diagnostic::DiagnosticMessages;
    use weaver_common::TestLogger;

    use crate::cli::{Cli, Commands};
    use crate::registry::emit::RegistryEmitArgs;
    use crate::registry::{
        PolicyArgs, RegistryArgs, RegistryCommand, RegistryPath, RegistrySubCommand,
    };
    use crate::run_command;
    use opentelemetry::KeyValue;

    use futures_util::future::BoxFuture;
    use opentelemetry::trace::{SpanKind, TraceError};
    use opentelemetry_sdk::export::{self, trace::ExportResult};
    use std::sync::{atomic, Arc, Mutex};

    #[test]
    fn test_registry_emit() {
        let logger = TestLogger::new();

        let cli = Cli {
            debug: 1,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Emit(RegistryEmitArgs {
                    registry: RegistryArgs {
                        registry: RegistryPath::LocalFolder {
                            path: "data/emit/".to_owned(),
                        },
                        follow_symlinks: false,
                    },
                    policy: PolicyArgs {
                        policies: vec![],
                        skip_policies: true,
                        display_policy_coverage: false,
                    },
                    diagnostic: Default::default(),
                    stdout: true,
                }),
            })),
        };

        let exit_directive = run_command(&cli, logger.clone());
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);
    }

    #[derive(Debug)]
    pub struct SpanExporter {
        resource: Resource,
        is_shutdown: atomic::AtomicBool,
        spans: Arc<Mutex<Vec<export::trace::SpanData>>>,
    }

    impl export::trace::SpanExporter for SpanExporter {
        fn export(
            &mut self,
            batch: Vec<export::trace::SpanData>,
        ) -> BoxFuture<'static, ExportResult> {
            if self.is_shutdown.load(atomic::Ordering::SeqCst) {
                Box::pin(std::future::ready(Err(TraceError::from(
                    "exporter is shut down",
                ))))
            } else {
                self.spans.lock().unwrap().extend(batch);
                Box::pin(std::future::ready(Ok(())))
            }
        }

        fn shutdown(&mut self) {
            self.is_shutdown.store(true, atomic::Ordering::SeqCst);
        }

        fn set_resource(&mut self, res: &Resource) {
            self.resource = res.clone();
        }
    }

    #[test]
    fn test_emit_trace_for_registry() {
        let arg_registry = RegistryArgs {
            registry: RegistryPath::LocalFolder {
                path: "data/emit/".to_owned(),
            },
            follow_symlinks: false,
        };
        let arg_policy = PolicyArgs {
            policies: vec![],
            skip_policies: true,
            display_policy_coverage: false,
        };

        let logger = TestLogger::new();
        let mut diag_msgs = DiagnosticMessages::empty();

        let spans = Arc::new(Mutex::new(Vec::new()));
        let span_exporter = SpanExporter {
            resource: Resource::empty(),
            is_shutdown: atomic::AtomicBool::new(false),
            spans: spans.clone(),
        };
        let tracer_provider = sdktrace::TracerProvider::builder()
            .with_resource(Resource::new(vec![KeyValue::new("service.name", "weaver")]))
            .with_simple_exporter(span_exporter)
            .build();

        let _ = global::set_tracer_provider(tracer_provider.clone());

        let (registry, _) = super::prepare_main_registry(
            &arg_registry,
            &arg_policy,
            logger.clone(),
            &mut diag_msgs,
        )
        .expect("Test registry must be prepared");

        super::emit_trace_for_registry(&registry, &arg_registry.registry.to_string());

        global::shutdown_tracer_provider();

        // Now check the spans stored in the span exporter
        assert_eq!(spans.lock().unwrap().len(), 6);

        let expected = vec![
            (
                "test.comprehensive.client",
                SpanKind::Client,
                vec![
                    KeyValue::new("test.string", "value1".to_owned()),
                    KeyValue::new("test.integer", Value::I64(42)),
                    KeyValue::new("test.double", Value::F64(3.13)),
                    KeyValue::new("test.boolean", Value::Bool(true)),
                    KeyValue::new(
                        "test.string_array",
                        Value::Array(Array::String(vec!["val1".into(), "val2".into()])),
                    ),
                    KeyValue::new("test.int_array", Value::Array(Array::I64(vec![1, 2]))),
                    KeyValue::new(
                        "test.double_array",
                        Value::Array(Array::F64(vec![1.1, 2.2])),
                    ),
                    KeyValue::new(
                        "test.boolean_array",
                        Value::Array(Array::Bool(vec![true, false])),
                    ),
                    KeyValue::new(
                        "test.template_string.key",
                        Value::String("template_value".into()),
                    ),
                    KeyValue::new(
                        "test.template_string_array.key",
                        Value::Array(Array::String(vec![
                            "template_value1".into(),
                            "template_value2".into(),
                        ])),
                    ),
                    KeyValue::new("test.enum", Value::String("VALUE_1".into())),
                ],
            ),
            (
                "test.comprehensive.server",
                SpanKind::Server,
                vec![KeyValue::new("test.string", Value::String("value1".into()))],
            ),
            (
                "test.comprehensive.producer",
                SpanKind::Producer,
                vec![KeyValue::new("test.string", Value::String("value1".into()))],
            ),
            (
                "test.comprehensive.consumer",
                SpanKind::Consumer,
                vec![KeyValue::new("test.string", Value::String("value1".into()))],
            ),
            (
                "test.comprehensive.internal",
                SpanKind::Internal,
                vec![KeyValue::new("test.string", Value::String("value1".into()))],
            ),
            (
                "weaver.emit",
                SpanKind::Internal,
                vec![KeyValue::new(
                    "weaver.registry_path",
                    Value::String("data/emit/".into()),
                )],
            ),
        ];
        for (i, span_data) in spans.lock().unwrap().iter().enumerate() {
            assert_eq!(span_data.name, expected[i].0);
            assert_eq!(span_data.span_kind, expected[i].1);
            for (j, attr) in span_data.attributes.iter().enumerate() {
                assert_eq!(attr.key, expected[i].2[j].key);
                assert_eq!(attr.value, expected[i].2[j].value);
            }
        }
    }
}
