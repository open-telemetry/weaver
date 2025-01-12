// SPDX-License-Identifier: Apache-2.0

//! Emit a semantic convention registry to an OTLP receiver.

use clap::Args;

use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_resolved_schema::attribute::Attribute;
use weaver_semconv::attribute::{AttributeType, PrimitiveOrArrayTypeSpec};
use weaver_semconv::group::{GroupType, SpanKindSpec};

use crate::registry::{PolicyArgs, RegistryArgs};
use crate::util::prepare_main_registry;
use crate::{DiagnosticArgs, ExitDirectives};

use opentelemetry::global;
use opentelemetry::trace::{SpanKind, TraceError, Tracer};
use opentelemetry::{Array, KeyValue, Value};
use opentelemetry_sdk::trace as sdktrace;
use opentelemetry_sdk::trace::{Span, TracerProvider};

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
}

// For the given attribute, return a name/value pair.
// The name is the name of the attribute.
// The value is data compatible with the type of the attribute
// TODO Get the data from the examples
fn get_attribute_name_value(attribute: &Attribute) -> KeyValue {
    let name = attribute.name.clone();
    let value = match &attribute.r#type {
        AttributeType::PrimitiveOrArray(primitive_or_array) => match primitive_or_array {
            PrimitiveOrArrayTypeSpec::Boolean => Value::Bool(true),
            PrimitiveOrArrayTypeSpec::Int => Value::I64(42),
            PrimitiveOrArrayTypeSpec::Double => Value::F64(3.13),
            PrimitiveOrArrayTypeSpec::String => Value::String("value".into()),
            PrimitiveOrArrayTypeSpec::Booleans => Value::Array(Array::Bool(vec![true, false])),
            PrimitiveOrArrayTypeSpec::Ints => Value::Array(Array::I64(vec![42, 43])),
            PrimitiveOrArrayTypeSpec::Doubles => Value::Array(Array::F64(vec![3.13, 3.15])),
            PrimitiveOrArrayTypeSpec::Strings => {
                Value::Array(Array::String(vec!["value1".into(), "value2".into()]))
            }
        },
        AttributeType::Enum { members, .. } => Value::String(members[0].value.to_string().into()),
        AttributeType::Template(_) => "value".into(),
    };
    KeyValue::new(name, value)
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

fn init_tracer_provider() -> Result<sdktrace::TracerProvider, TraceError> {
    // First, create a OTLP exporter builder. Configure it as you need.
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint("http://localhost:4317")
        .with_timeout(std::time::Duration::from_secs(3))
        .build()?;
    Ok(sdktrace::TracerProvider::builder()
        .with_resource(Resource::new(vec![KeyValue::new("service.name", "weaver")]))
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .build())
}

/// Resolve a semantic convention registry and write the resolved schema to a
/// file or print it to stdout.
pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    args: &RegistryEmitArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    logger.log("Weaver Registry Emit");
    logger.loading(&format!("Emitting registry `{}`", args.registry.registry));

    let mut diag_msgs = DiagnosticMessages::empty();

    let (registry, _) =
        prepare_main_registry(&args.registry, &args.policy, logger.clone(), &mut diag_msgs)?;

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async {
        let tracer_provider = init_tracer_provider().expect("OTLP Tracer Provider must be created");
        let _ = global::set_tracer_provider(tracer_provider.clone());
        let tracer = global::tracer("weaver");
        // Go through the resolved registry and emit each group of type Span to the
        // OTLP receiver.
        for group in registry.groups.iter() {
            if group.r#type == GroupType::Span {
                logger.success(&format!("Emitting {}", group.id));

                let span = tracer
                    .span_builder(group.id.clone())
                    .with_kind(otel_span_kind(group.span_kind.as_ref()))
                    .with_attributes(group.attributes.iter().map(get_attribute_name_value))
                    .start(&tracer);
                drop(span);
            }
        }

        // Shutdown trace pipeline
        global::shutdown_tracer_provider();
    });

    if !diag_msgs.is_empty() {
        return Err(diag_msgs);
    }

    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}

#[cfg(test)]
mod tests {
    use weaver_common::TestLogger;

    use crate::cli::{Cli, Commands};
    use crate::registry::emit::RegistryEmitArgs;
    use crate::registry::{
        PolicyArgs, RegistryArgs, RegistryCommand, RegistryPath, RegistrySubCommand,
    };
    use crate::run_command;

    #[test]
    fn test_registry_resolve() {
        let logger = TestLogger::new();
        let cli = Cli {
            debug: 1,
            quiet: false,
            future: false,
            command: Some(Commands::Registry(RegistryCommand {
                command: RegistrySubCommand::Emit(RegistryEmitArgs {
                    registry: RegistryArgs {
                        registry: RegistryPath::LocalFolder {
                            path: "crates/weaver_codegen_test/semconv_registry/".to_owned(),
                        },
                        follow_symlinks: false,
                    },
                    policy: PolicyArgs {
                        policies: vec![],
                        skip_policies: true,
                        display_policy_coverage: false,
                    },
                    diagnostic: Default::default(),
                }),
            })),
        };

        let exit_directive = run_command(&cli, logger.clone());
        // The command should succeed.
        assert_eq!(exit_directive.exit_code, 0);
    }
}
