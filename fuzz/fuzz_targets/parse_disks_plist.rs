#![no_main]

// This target only runs meaningfully when raidhos-core is compiled for
// macOS — the plist parser is gated on `target_os = "macos"`. The
// target compiles on every host so CI doesn't break; on non-macOS the
// parser is unreachable and the body short-circuits.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|_data: &[u8]| {
    // No public surface to the parser today; treated as a placeholder
    // until we expose it via a doc(hidden) helper.
});
