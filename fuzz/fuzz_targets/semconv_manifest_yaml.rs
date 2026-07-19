#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Write;
use weaver_semconv::manifest::RegistryManifest;

fuzz_target!(|data: &[u8]| {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("manifest.yaml");
    let mut f = std::fs::File::create(&path).expect("create");
    f.write_all(data).expect("write");

    let mut nfes = Vec::new();
    let _ = RegistryManifest::try_from_file(&path, &mut nfes);
});
