#![no_main]
use libfuzzer_sys::fuzz_target;
use rustbac_core::encoding::reader::Reader;
use rustbac_core::npdu::Npdu;

fuzz_target!(|data: &[u8]| {
    let mut r = Reader::new(data);
    let _ = Npdu::decode(&mut r);
});
