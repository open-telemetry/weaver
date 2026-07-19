#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Write;
use weaver_forge::config::WeaverConfig;

fuzz_target!(|data: &[u8]| {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("weaver.yaml");
    let mut f = std::fs::File::create(&path).expect("create");
    f.write_all(data).expect("write");

    let _ = WeaverConfig::try_from_config_files(&[&path]);
});
