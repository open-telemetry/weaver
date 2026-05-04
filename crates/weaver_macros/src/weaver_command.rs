// SPDX-License-Identifier: Apache-2.0

use proc_macro::TokenStream;
use proc_macro2::{Literal, Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, Data, DeriveInput, Fields, GenericArgument, Ident, PathArguments, Type,
};

// ── Top-level parse result ────────────────────────────────────────────────────

struct WeaverCommandAttr {
    section: String,
    no_policy: bool,
    /// Use an existing config type instead of generating one.
    config_type: Option<String>,
    /// Additional config-only field names (from the external config struct, no CLI counterpart).
    extra_config_only: Vec<String>,
}

enum SharedKind {
    Registry,
    Policy,
    Diagnostic,
}

struct ConfigAnnotation {
    default: Option<String>,
    config_only: bool,
    /// Nested config path e.g. `"otlp.grpc_address"` — maps this CLI arg to a nested field.
    path: Option<String>,
    /// When true, the config destination is `Option<T>` (use optional form of override_if_set!).
    is_optional: bool,
}

enum FieldKind {
    Shared(SharedKind, Type),
    Config(ConfigAnnotation),
    CliOnly,
}

struct ClassifiedField<'a> {
    ident: &'a Ident,
    ty: &'a Type,
    kind: FieldKind,
    doc_attrs: Vec<&'a syn::Attribute>,
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let struct_attr = match parse_struct_attr(&input) {
        Ok(a) => a,
        Err(e) => return e.into_compile_error().into(),
    };

    let fields = match parse_fields(&input) {
        Ok(f) => f,
        Err(e) => return e.into_compile_error().into(),
    };

    let config_name = section_to_config_ident(&struct_attr.section);
    let args_ident = &input.ident;
    let struct_doc_attrs: Vec<&syn::Attribute> = input
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .collect();

    // When config_type is set, the Config struct already exists — don't generate it.
    let config_struct = if struct_attr.config_type.is_none() {
        gen_config_struct(&config_name, &struct_doc_attrs, &fields)
    } else {
        quote! {}
    };
    let default_impl = if struct_attr.config_type.is_none() {
        gen_default_impl(&config_name, &fields)
    } else {
        quote! {}
    };
    let cli_overrides_impl =
        gen_cli_overrides_impl(args_ident, &config_name, &struct_attr, &fields);

    let expanded = quote! {
        #config_struct
        #default_impl
        #cli_overrides_impl
    };

    expanded.into()
}

// ── Parse #[weaver_command(...)] ──────────────────────────────────────────────

fn parse_struct_attr(input: &DeriveInput) -> syn::Result<WeaverCommandAttr> {
    let mut section = None;
    let mut no_policy = false;
    let mut config_type = None;
    let mut extra_config_only = Vec::new();

    for attr in &input.attrs {
        if !attr.path().is_ident("weaver_command") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("section") {
                let value: syn::LitStr = meta.value()?.parse()?;
                section = Some(value.value());
            } else if meta.path.is_ident("no_policy") {
                no_policy = true;
            } else if meta.path.is_ident("config_type") {
                let value: syn::LitStr = meta.value()?.parse()?;
                config_type = Some(value.value());
            } else if meta.path.is_ident("extra_config_only") {
                let value: syn::LitStr = meta.value()?.parse()?;
                extra_config_only = value
                    .value()
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            Ok(())
        })?;
    }

    let section = section.ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            "#[derive(WeaverCommand)] requires #[weaver_command(section = \"name\")]",
        )
    })?;

    Ok(WeaverCommandAttr {
        section,
        no_policy,
        config_type,
        extra_config_only,
    })
}

// ── Parse and classify fields ─────────────────────────────────────────────────

fn parse_fields<'a>(input: &'a DeriveInput) -> syn::Result<Vec<ClassifiedField<'a>>> {
    let Data::Struct(data_struct) = &input.data else {
        return Err(syn::Error::new(
            Span::call_site(),
            "#[derive(WeaverCommand)] only works on structs",
        ));
    };
    let Fields::Named(named) = &data_struct.fields else {
        return Err(syn::Error::new(
            Span::call_site(),
            "#[derive(WeaverCommand)] requires named fields",
        ));
    };

    let mut result = Vec::new();
    for field in &named.named {
        let ident = field.ident.as_ref().expect("named field");
        let ty = &field.ty;
        let kind = classify_field(field)?;
        let doc_attrs = field
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("doc"))
            .collect();
        result.push(ClassifiedField {
            ident,
            ty,
            kind,
            doc_attrs,
        });
    }
    Ok(result)
}

fn classify_field(field: &syn::Field) -> syn::Result<FieldKind> {
    for attr in &field.attrs {
        let path = attr.path();

        if path.is_ident("shared") {
            let mut shared_kind = None;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("registry") {
                    shared_kind = Some(SharedKind::Registry);
                } else if meta.path.is_ident("policy") {
                    shared_kind = Some(SharedKind::Policy);
                } else if meta.path.is_ident("diagnostic") {
                    shared_kind = Some(SharedKind::Diagnostic);
                }
                Ok(())
            })?;
            if let Some(k) = shared_kind {
                return Ok(FieldKind::Shared(k, field.ty.clone()));
            }
        }

        if path.is_ident("config") {
            let annotation = parse_config_annotation(attr, false)?;
            return Ok(FieldKind::Config(annotation));
        }

        if path.is_ident("config_only") {
            let annotation = parse_config_annotation(attr, true)?;
            return Ok(FieldKind::Config(annotation));
        }
    }
    Ok(FieldKind::CliOnly)
}

fn parse_config_annotation(
    attr: &syn::Attribute,
    config_only: bool,
) -> syn::Result<ConfigAnnotation> {
    // #[config] or #[config_only] with no arguments
    if matches!(attr.meta, syn::Meta::Path(_)) {
        return Ok(ConfigAnnotation {
            default: None,
            config_only,
            path: None,
            is_optional: false,
        });
    }
    // #[config(...)] or #[config_only(...)]
    let mut default = None;
    let mut path = None;
    let mut is_optional = false;
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("default") {
            let value: syn::LitStr = meta.value()?.parse()?;
            default = Some(value.value());
        } else if meta.path.is_ident("path") {
            let value: syn::LitStr = meta.value()?.parse()?;
            path = Some(value.value());
        } else if meta.path.is_ident("optional") {
            is_optional = true;
        }
        Ok(())
    })?;
    Ok(ConfigAnnotation {
        default,
        config_only,
        path,
        is_optional,
    })
}

// ── Config struct generation ──────────────────────────────────────────────────

fn gen_config_struct(
    config_name: &Ident,
    struct_docs: &[&syn::Attribute],
    fields: &[ClassifiedField<'_>],
) -> TokenStream2 {
    let config_fields: Vec<TokenStream2> = fields
        .iter()
        .filter_map(|f| {
            let FieldKind::Config(ann) = &f.kind else {
                return None;
            };
            // Skip path fields — they belong to an external config struct
            if ann.path.is_some() {
                return None;
            }
            let ident = f.ident;
            let config_ty = config_field_type(f.ty, ann);
            let docs = &f.doc_attrs;
            Some(quote! { #(#docs)* pub #ident: #config_ty, })
        })
        .collect();

    // If no config fields exist the struct is empty (like CheckConfig).
    quote! {
        #(#struct_docs)*
        #[derive(Debug, Clone, ::serde::Deserialize, ::schemars::JsonSchema, PartialEq)]
        #[serde(default)]
        #[schemars(inline)]
        pub struct #config_name {
            #(#config_fields)*
        }
    }
}

/// Returns the type that goes in the Config struct for a given Args field type
/// and annotation.
fn config_field_type(args_ty: &Type, ann: &ConfigAnnotation) -> TokenStream2 {
    if ann.default.is_some() {
        // #[config(default = ...)] or #[config_only(default = ...)] → unwrap Option<T> → T
        let inner = extract_option_inner(args_ty).unwrap_or(args_ty);
        quote! { #inner }
    } else {
        // #[config] or #[config_only] (no default) → keep as Option<T>
        quote! { #args_ty }
    }
}

// ── Default impl generation ───────────────────────────────────────────────────

fn gen_default_impl(config_name: &Ident, fields: &[ClassifiedField<'_>]) -> TokenStream2 {
    let default_fields: Vec<TokenStream2> = fields
        .iter()
        .filter_map(|f| {
            let FieldKind::Config(ann) = &f.kind else {
                return None;
            };
            // Skip path fields — defaults come from the external config struct
            if ann.path.is_some() {
                return None;
            }
            let ident = f.ident;
            let inner = extract_option_inner(f.ty).unwrap_or(f.ty);
            let default_expr = gen_default_expr(inner, ann.default.as_deref());
            Some(quote! { #ident: #default_expr, })
        })
        .collect();

    quote! {
        impl Default for #config_name {
            fn default() -> Self {
                Self {
                    #(#default_fields)*
                }
            }
        }
    }
}

/// Generate a Rust expression for a default value given the inner type and an
/// optional string from the annotation.
fn gen_default_expr(inner_ty: &Type, default_str: Option<&str>) -> TokenStream2 {
    let Some(s) = default_str else {
        return quote! { None };
    };
    let type_name = last_type_segment(inner_ty).unwrap_or_default();
    match type_name.as_str() {
        "String" => quote! { #s.to_owned() },
        "PathBuf" => quote! { ::std::path::PathBuf::from(#s) },
        "SocketAddr" => {
            quote! { #s.parse::<::std::net::SocketAddr>().expect("valid default bind address") }
        }
        "bool" => {
            let b: bool = s.parse().unwrap_or(false);
            quote! { #b }
        }
        "u8" => {
            let n: u8 = s.parse().unwrap_or(0);
            let lit = Literal::u8_suffixed(n);
            quote! { #lit }
        }
        "u16" => {
            let n: u16 = s.parse().unwrap_or(0);
            let lit = Literal::u16_suffixed(n);
            quote! { #lit }
        }
        "u32" => {
            let n: u32 = s.parse().unwrap_or(0);
            let lit = Literal::u32_suffixed(n);
            quote! { #lit }
        }
        "u64" => {
            let n: u64 = s.parse().unwrap_or(0);
            let lit = Literal::u64_suffixed(n);
            quote! { #lit }
        }
        "usize" => {
            let n: usize = s.parse().unwrap_or(0);
            let lit = Literal::usize_suffixed(n);
            quote! { #lit }
        }
        "i8" => {
            let n: i8 = s.parse().unwrap_or(0);
            let lit = Literal::i8_suffixed(n);
            quote! { #lit }
        }
        "i16" => {
            let n: i16 = s.parse().unwrap_or(0);
            let lit = Literal::i16_suffixed(n);
            quote! { #lit }
        }
        "i32" => {
            let n: i32 = s.parse().unwrap_or(0);
            let lit = Literal::i32_suffixed(n);
            quote! { #lit }
        }
        "i64" => {
            let n: i64 = s.parse().unwrap_or(0);
            let lit = Literal::i64_suffixed(n);
            quote! { #lit }
        }
        "f32" => {
            let n: f32 = s.parse().unwrap_or(0.0);
            let lit = Literal::f32_suffixed(n);
            quote! { #lit }
        }
        "f64" => {
            let n: f64 = s.parse().unwrap_or(0.0);
            let lit = Literal::f64_suffixed(n);
            quote! { #lit }
        }
        _ => {
            // Unknown type: emit `.to_owned()` and let the compiler validate.
            quote! { #s.to_owned() }
        }
    }
}

// ── CliOverrides impl generation ──────────────────────────────────────────────

fn gen_cli_overrides_impl(
    args_ident: &Ident,
    config_name: &Ident,
    struct_attr: &WeaverCommandAttr,
    fields: &[ClassifiedField<'_>],
) -> TokenStream2 {
    let section = &struct_attr.section;
    let section_lit = syn::LitStr::new(section, Span::call_site());

    // Resolve the config type: either the external type or the generated name.
    let config_ty: TokenStream2 = if let Some(ct) = &struct_attr.config_type {
        let parsed: Type = match syn::parse_str(ct) {
            Ok(t) => t,
            Err(e) => return e.into_compile_error(),
        };
        quote! { #parsed }
    } else {
        quote! { #config_name }
    };

    // excluded_args slices
    let excluded = gen_excluded_args(fields);

    // config_only_fields: from #[config_only] fields + extra_config_only
    let config_only_names: Vec<TokenStream2> = fields
        .iter()
        .filter_map(|f| {
            if let FieldKind::Config(ann) = &f.kind {
                if ann.config_only {
                    let name = f.ident.to_string();
                    return Some(quote! { #name });
                }
            }
            None
        })
        .chain(struct_attr.extra_config_only.iter().map(|name| {
            quote! { #name }
        }))
        .collect();

    let config_only_impl = if config_only_names.is_empty() {
        quote! {}
    } else {
        quote! {
            fn config_only_fields() -> &'static [&'static str] {
                &[#(#config_only_names),*]
            }
        }
    };

    // field_mappings: generated from #[config(path = "...")] where flattened ≠ CLI name
    let mapping_entries: Vec<TokenStream2> = fields
        .iter()
        .filter_map(|f| {
            let FieldKind::Config(ann) = &f.kind else {
                return None;
            };
            let path = ann.path.as_deref()?;
            let flattened: String = path.split('.').collect::<Vec<_>>().join("_");
            let cli_name = f.ident.to_string();
            if flattened == cli_name {
                return None;
            }
            Some(quote! {
                ::weaver_config::FieldMapping {
                    config_name: #flattened,
                    cli_name: #cli_name,
                }
            })
        })
        .collect();

    let field_mappings_method = if mapping_entries.is_empty() {
        quote! {}
    } else {
        quote! {
            fn field_mappings() -> &'static [::weaver_config::FieldMapping] {
                &[#(#mapping_entries),*]
            }
        }
    };

    // apply_overrides: handle both regular and path-based config fields
    let override_stmts: Vec<TokenStream2> = fields
        .iter()
        .filter_map(|f| {
            let FieldKind::Config(ann) = &f.kind else {
                return None;
            };
            let ident = f.ident;

            if let Some(path) = &ann.path {
                // Nested path: build `config.a.b.c` token stream
                let path_expr = {
                    let mut tokens = quote! { config };
                    for segment in path.split('.') {
                        let seg_ident = format_ident!("{}", segment);
                        tokens = quote! { #tokens.#seg_ident };
                    }
                    tokens
                };
                if ann.is_optional {
                    Some(quote! {
                        ::weaver_config::override_if_set!(#path_expr, self.#ident, optional);
                    })
                } else {
                    Some(quote! {
                        ::weaver_config::override_if_set!(#path_expr, self.#ident);
                    })
                }
            } else if ann.default.is_some() {
                Some(quote! {
                    ::weaver_config::override_if_set!(config.#ident, self.#ident);
                })
            } else {
                Some(quote! {
                    ::weaver_config::override_if_set!(config.#ident, self.#ident, optional);
                })
            }
        })
        .collect();

    // shared override methods
    let mut registry_method = quote! {};
    let mut policy_method = quote! {};
    let mut diagnostic_method = quote! {};
    for f in fields {
        let FieldKind::Shared(kind, _) = &f.kind else {
            continue;
        };
        let field_ident = f.ident;
        match kind {
            SharedKind::Registry => {
                registry_method = quote! {
                    fn apply_registry_overrides(
                        &self,
                        config: &mut ::weaver_config::EffectiveRegistryConfig,
                    ) {
                        self.#field_ident.apply_to(config);
                    }
                };
            }
            SharedKind::Policy => {
                policy_method = quote! {
                    fn apply_policy_overrides(
                        &self,
                        config: &mut ::weaver_config::EffectivePolicyConfig,
                    ) {
                        self.#field_ident.apply_to(config);
                    }
                };
            }
            SharedKind::Diagnostic => {
                diagnostic_method = quote! {
                    fn apply_diagnostic_overrides(
                        &self,
                        config: &mut ::weaver_config::EffectiveDiagnosticConfig,
                    ) {
                        self.#field_ident.apply_to(config);
                    }
                };
            }
        }
    }

    // uses_policy override
    let uses_policy_method = if struct_attr.no_policy {
        quote! {
            fn uses_policy() -> bool { false }
        }
    } else {
        quote! {}
    };

    quote! {
        impl ::weaver_config::CliOverrides for #args_ident {
            type Config = #config_ty;
            const SUBCOMMAND: &'static str = #section_lit;

            fn extract_config(wc: &::weaver_config::WeaverConfig) -> #config_ty {
                wc.command_config(#section_lit)
            }

            #excluded

            #config_only_impl

            #field_mappings_method

            fn apply_overrides(&self, config: &mut #config_ty) {
                #(#override_stmts)*
            }

            #registry_method
            #policy_method
            #diagnostic_method
            #uses_policy_method
        }
    }
}

/// Generate the `excluded_args()` method from `#[shared(...)]` fields and
/// CLI-only fields.
fn gen_excluded_args(fields: &[ClassifiedField<'_>]) -> TokenStream2 {
    let mut shared_slices: Vec<TokenStream2> = Vec::new();
    let mut cli_only_names: Vec<String> = Vec::new();

    for f in fields {
        match &f.kind {
            FieldKind::Shared(_, ty) => {
                shared_slices.push(quote! { <#ty>::EXCLUDED_ARGS });
            }
            FieldKind::CliOnly => {
                cli_only_names.push(f.ident.to_string());
            }
            FieldKind::Config(_) => {}
        }
    }

    if shared_slices.is_empty() && cli_only_names.is_empty() {
        return quote! {};
    }

    let all_slices: Vec<TokenStream2> = shared_slices
        .into_iter()
        .chain(if cli_only_names.is_empty() {
            vec![]
        } else {
            vec![quote! { &[#(#cli_only_names),*] }]
        })
        .collect();

    if all_slices.len() == 1 {
        let single = &all_slices[0];
        quote! {
            fn excluded_args() -> &'static [&'static str] {
                ::weaver_config::excluded_args!(#single,)
            }
        }
    } else {
        quote! {
            fn excluded_args() -> &'static [&'static str] {
                ::weaver_config::excluded_args!(#(#all_slices),*)
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract `T` from `Option<T>`, or return the type unchanged.
fn extract_option_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        let last = type_path.path.segments.last()?;
        if last.ident == "Option" {
            if let PathArguments::AngleBracketed(args) = &last.arguments {
                if let Some(GenericArgument::Type(inner)) = args.args.first() {
                    return Some(inner);
                }
            }
        }
    }
    None
}

/// Return the last path segment name of a type (e.g. `String`, `PathBuf`).
fn last_type_segment(ty: &Type) -> Option<String> {
    if let Type::Path(type_path) = ty {
        Some(type_path.path.segments.last()?.ident.to_string())
    } else {
        None
    }
}

/// Convert a kebab-case or snake_case section name to a PascalCase Config ident.
/// `"emit"` → `EmitConfig`, `"update-markdown"` → `UpdateMarkdownConfig`.
fn section_to_config_ident(section: &str) -> Ident {
    let pascal: String = section
        .split(['-', '_'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let mut s: String = c.to_uppercase().collect();
                    s.push_str(chars.as_str());
                    s
                }
            }
        })
        .collect();
    format_ident!("{}Config", pascal)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── section_to_config_ident ───────────────────────────────────────────────

    #[test]
    fn test_section_to_config_ident_single_word() {
        assert_eq!(section_to_config_ident("emit").to_string(), "EmitConfig");
        assert_eq!(section_to_config_ident("check").to_string(), "CheckConfig");
    }

    #[test]
    fn test_section_to_config_ident_kebab_case() {
        assert_eq!(
            section_to_config_ident("update-markdown").to_string(),
            "UpdateMarkdownConfig"
        );
        assert_eq!(
            section_to_config_ident("live-check").to_string(),
            "LiveCheckConfig"
        );
    }

    #[test]
    fn test_section_to_config_ident_snake_case() {
        assert_eq!(
            section_to_config_ident("update_markdown").to_string(),
            "UpdateMarkdownConfig"
        );
    }

    #[test]
    fn test_section_to_config_ident_single_letter() {
        assert_eq!(section_to_config_ident("a").to_string(), "AConfig");
    }

    // ── extract_option_inner ──────────────────────────────────────────────────

    fn parse_type(s: &str) -> Type {
        syn::parse_str(s).unwrap()
    }

    #[test]
    fn test_extract_option_inner_some() {
        let ty = parse_type("Option<String>");
        let inner = extract_option_inner(&ty);
        assert!(inner.is_some());
        assert_eq!(quote::quote!(#inner).to_string(), "String");
    }

    #[test]
    fn test_extract_option_inner_nested() {
        let ty = parse_type("Option<Vec<PathBuf>>");
        let inner = extract_option_inner(&ty);
        assert!(inner.is_some());
        assert_eq!(quote::quote!(#inner).to_string(), "Vec < PathBuf >");
    }

    #[test]
    fn test_extract_option_inner_non_option() {
        let ty = parse_type("String");
        assert!(extract_option_inner(&ty).is_none());
    }

    #[test]
    fn test_extract_option_inner_bool() {
        let ty = parse_type("Option<bool>");
        let inner = extract_option_inner(&ty);
        assert!(inner.is_some());
        assert_eq!(quote::quote!(#inner).to_string(), "bool");
    }

    // ── last_type_segment ─────────────────────────────────────────────────────

    #[test]
    fn test_last_type_segment_simple() {
        assert_eq!(
            last_type_segment(&parse_type("String")),
            Some("String".to_owned())
        );
        assert_eq!(
            last_type_segment(&parse_type("PathBuf")),
            Some("PathBuf".to_owned())
        );
        assert_eq!(
            last_type_segment(&parse_type("bool")),
            Some("bool".to_owned())
        );
    }

    #[test]
    fn test_last_type_segment_qualified() {
        assert_eq!(
            last_type_segment(&parse_type("std::path::PathBuf")),
            Some("PathBuf".to_owned())
        );
    }

    #[test]
    fn test_last_type_segment_generic() {
        assert_eq!(
            last_type_segment(&parse_type("Option<String>")),
            Some("Option".to_owned())
        );
    }

    // ── gen_default_expr ──────────────────────────────────────────────────────

    fn expr_str(ty_str: &str, default: Option<&str>) -> String {
        let ty = parse_type(ty_str);
        gen_default_expr(&ty, default).to_string()
    }

    #[test]
    fn test_gen_default_expr_none() {
        assert_eq!(expr_str("String", None), "None");
        assert_eq!(expr_str("bool", None), "None");
    }

    #[test]
    fn test_gen_default_expr_string() {
        let out = expr_str("String", Some("hello"));
        assert!(out.contains("to_owned"), "expected .to_owned() in: {out}");
    }

    #[test]
    fn test_gen_default_expr_bool_true() {
        assert_eq!(expr_str("bool", Some("true")), "true");
    }

    #[test]
    fn test_gen_default_expr_bool_false() {
        assert_eq!(expr_str("bool", Some("false")), "false");
    }

    #[test]
    fn test_gen_default_expr_u32() {
        assert_eq!(expr_str("u32", Some("42")), "42u32");
    }

    #[test]
    fn test_gen_default_expr_u64() {
        assert_eq!(expr_str("u64", Some("100")), "100u64");
    }

    #[test]
    fn test_gen_default_expr_pathbuf() {
        let out = expr_str("PathBuf", Some("./templates"));
        assert!(
            out.contains("PathBuf") && out.contains("from"),
            "expected PathBuf::from in: {out}"
        );
    }

    #[test]
    fn test_gen_default_expr_socket_addr() {
        let out = expr_str("SocketAddr", Some("127.0.0.1:8080"));
        assert!(
            out.contains("parse") && out.contains("SocketAddr"),
            "expected parse::<SocketAddr>() in: {out}"
        );
    }

    #[test]
    fn test_gen_default_expr_unknown_type() {
        // Unknown types fall through to `.to_owned()`.
        let out = expr_str("MyCustomType", Some("value"));
        assert!(out.contains("to_owned"), "expected .to_owned() in: {out}");
    }

    #[test]
    fn test_gen_default_expr_integer_types() {
        assert_eq!(expr_str("u8", Some("1")), "1u8");
        assert_eq!(expr_str("u16", Some("2")), "2u16");
        assert_eq!(expr_str("usize", Some("3")), "3usize");
        assert_eq!(expr_str("i8", Some("4")), "4i8");
        assert_eq!(expr_str("i16", Some("5")), "5i16");
        assert_eq!(expr_str("i32", Some("6")), "6i32");
        assert_eq!(expr_str("i64", Some("7")), "7i64");
    }

    #[test]
    fn test_gen_default_expr_float_types() {
        let f32_out = expr_str("f32", Some("1.5"));
        assert!(f32_out.contains("f32"), "expected f32 suffix in: {f32_out}");
        let f64_out = expr_str("f64", Some("2.5"));
        assert!(f64_out.contains("f64"), "expected f64 suffix in: {f64_out}");
    }
}
