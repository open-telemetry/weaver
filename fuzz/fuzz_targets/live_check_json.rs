#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Write;
use weaver_live_check::json_file_ingester::JsonFileIngester;
use weaver_live_check::Ingester;

fuzz_target!(|data: &[u8]| {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("input.json");
    let mut f = std::fs::File::create(&path).expect("create");
    f.write_all(data).expect("write");

    // ingest() returns Err on malformed JSON — that's fine.
    // A panic or OOM here is what the fuzzer is looking for.
    let ingester = JsonFileIngester::new(&path);
    let _ = ingester.ingest();
});
