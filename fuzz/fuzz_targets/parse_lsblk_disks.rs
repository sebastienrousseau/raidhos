#![no_main]

// Linux-only parser; same caveats as the plist target.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|_data: &[u8]| {});
