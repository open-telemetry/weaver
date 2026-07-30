#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Write;
use weaver_live_check::text_file_ingester::TextFileIngester;
use weaver_live_check::Ingester;

fuzz_target!(|data: &[u8]| {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("input.txt");
    let mut f = std::fs::File::create(&path).expect("create");
    f.write_all(data).expect("write");

    let ingester = TextFileIngester::new(&path);
    let _ = ingester.ingest();
});
