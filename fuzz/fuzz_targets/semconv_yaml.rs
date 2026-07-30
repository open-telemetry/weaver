#![no_main]
use libfuzzer_sys::fuzz_target;
use std::io::Write;
use weaver_semconv::semconv::SemConvSpecWithProvenance;
use weaver_semconv::schema_url::SchemaUrl;

fuzz_target!(|data: &[u8]| {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("input.yaml");
    let mut f = std::fs::File::create(&path).expect("create");
    f.write_all(data).expect("write");

    let _ = SemConvSpecWithProvenance::from_file(SchemaUrl::new_unknown(), &path);
});
