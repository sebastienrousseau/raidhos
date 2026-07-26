#![no_main]

use libfuzzer_sys::fuzz_target;
use raidhos_core::__fuzz_api::parse_disks_plist;

fuzz_target!(|data: &[u8]| {
    let _ = parse_disks_plist(data);
});
