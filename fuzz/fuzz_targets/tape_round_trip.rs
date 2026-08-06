//! The tape must reconstruct any input exactly.
//!
//! Every syntax-aware edit is computed from byte ranges the tape reports. If
//! the tape can lose, duplicate, or reorder a byte for some input, an edit
//! built on it corrupts the file. The property tests cover generated ASCII;
//! this covers whatever a fuzzer invents.

#![no_main]

use libfuzzer_sys::fuzz_target;
use normfix_c_syntax::CParser;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(mut parser) = CParser::new() else {
        return;
    };
    let Ok(parsed) = parser.parse(source) else {
        return;
    };
    assert_eq!(
        parsed.tape().reconstruct(),
        source,
        "the tape did not reconstruct its input"
    );
});
