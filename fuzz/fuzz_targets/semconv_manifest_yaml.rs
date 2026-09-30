#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Write;
use weaver_semconv::registry_repo::{ManifestPath, VersionedManifest};

fuzz_target!(|data: &[u8]| {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest_path = dir.path().join("manifest.yaml");
    let legacy_path = dir.path().join("registry_manifest.yaml");
    let _ = std::fs::write(&manifest_path, data);
    let _ = std::fs::write(&legacy_path, data);

    let mut nfes = Vec::new();
    let _ = VersionedManifest::try_from_file(&ManifestPath::RegistryPath(manifest_path), &mut nfes);
    let mut nfes = Vec::new();
    let _ = VersionedManifest::try_from_file(&ManifestPath::LegacyPath(legacy_path), &mut nfes);
});
