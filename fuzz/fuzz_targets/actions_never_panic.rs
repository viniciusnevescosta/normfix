//! The action pipeline must fail closed, never panic.
//!
//! `normfix` runs over source it did not write, including files that are being
//! edited while it runs. Any input must produce either a formatted buffer or a
//! typed error; a panic aborts a run that may already hold a transaction.

#![no_main]

use libfuzzer_sys::fuzz_target;
use normfix_c_actions::{CActionOptions, apply_c_actions};

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    // A fuzzer finds pathological inputs faster than it finds slow ones, so keep
    // the pass bound low enough that a case is a crash rather than a timeout.
    let options = CActionOptions {
        max_passes: 8,
        ..CActionOptions::default()
    };
    let _ = apply_c_actions(camino::Utf8Path::new("fuzz.c"), source, &[], &options);
});
