#![no_main]
use libfuzzer_sys::fuzz_target;
use std::collections::BTreeMap;
use weaver_forge::jq::execute_jq;

fuzz_target!(|data: &[u8]| {
    let Ok(filter) = std::str::from_utf8(data) else {
        return;
    };
    let input = serde_json::json!({});
    let params = BTreeMap::new();
    let _ = execute_jq(&input, filter, &params);
});
