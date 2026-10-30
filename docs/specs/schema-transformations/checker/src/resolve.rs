//! Resolves the registries and prints group / attribute counts.

use std::path::Path;

use weaver_common::result::WResult;
use weaver_common::vdir::VirtualDirectoryPath;
use weaver_resolved_schema::v1::ResolvedTelemetrySchema;
use weaver_resolver::{DefaultSchemaVisitor, WeaverResolver, WeaverResolverConfig};
use weaver_semconv::registry_repo::RegistryRepo;
use weaver_semconv::schema_url::SchemaUrl;

/// Resolves the registry at `path` into a V1 schema.
pub fn resolve(
    path: &Path,
    overrides: &[(&str, &Path)],
) -> anyhow::Result<ResolvedTelemetrySchema> {
    let vpath = VirtualDirectoryPath::LocalFolder {
        path: path.to_string_lossy().into_owned(),
    };
    let mut nfes = vec![];
    let repo = RegistryRepo::try_new(None, &vpath, &mut nfes)
        .map_err(|e| anyhow::anyhow!("registry repo: {e}"))?;
    let config = WeaverResolverConfig {
        include_unreferenced: true,
        ..Default::default()
    };
    let mut resolver = WeaverResolver::new(config);
    for (url, p) in overrides {
        let url = SchemaUrl::try_from(*url).map_err(|e| anyhow::anyhow!("{e}"))?;
        resolver.add_schema_url_override(
            url,
            VirtualDirectoryPath::LocalFolder {
                path: p.to_string_lossy().into_owned(),
            },
        );
    }
    let resolved = match resolver.load_and_resolve_schema(repo, DefaultSchemaVisitor) {
        WResult::Ok(r) | WResult::OkWithNFEs(r, _) => r,
        WResult::FatalErr(e) => anyhow::bail!("{e}"),
    };
    resolved.into_v1().map_err(|e| anyhow::anyhow!("{e}"))
}
