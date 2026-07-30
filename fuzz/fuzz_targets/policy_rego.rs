#![no_main]
use libfuzzer_sys::fuzz_target;
use weaver_checker::Engine;

fuzz_target!(|data: &[u8]| {
    if let Ok(rego) = std::str::from_utf8(data) {
        let mut engine = Engine::new();
        let _ = engine.add_policy("fuzz_target", rego);
    }
});
