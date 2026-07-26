#![no_main]

use libfuzzer_sys::fuzz_target;
use raidhos_core::__fuzz_api::parse_lsblk_disks;

fuzz_target!(|data: &[u8]| {
    let _ = parse_lsblk_disks(data);
});
