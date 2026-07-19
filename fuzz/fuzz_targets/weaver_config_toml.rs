#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Write;

fuzz_target!(|data: &[u8]| {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(".weaver.toml");
    let mut f = std::fs::File::create(&path).expect("create");
    f.write_all(data).expect("write");

    let _ = weaver_config::load(&path);
});
