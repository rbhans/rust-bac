#![no_main]
use libfuzzer_sys::fuzz_target;
use rustbac_core::encoding::reader::Reader;
use rustbac_datalink::bip::bvlc::BvlcHeader;

fuzz_target!(|data: &[u8]| {
    let mut r = Reader::new(data);
    let _ = BvlcHeader::decode(&mut r);
});
