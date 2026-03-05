#![no_main]
use libfuzzer_sys::fuzz_target;
use rustbac_core::apdu::confirmed::{AbortPdu, ComplexAckHeader, ConfirmedRequestHeader};
use rustbac_core::encoding::reader::Reader;

fuzz_target!(|data: &[u8]| {
    // Fuzz ConfirmedRequestHeader::decode
    {
        let mut r = Reader::new(data);
        let _ = ConfirmedRequestHeader::decode(&mut r);
    }
    // Fuzz ComplexAckHeader::decode
    {
        let mut r = Reader::new(data);
        let _ = ComplexAckHeader::decode(&mut r);
    }
    // Fuzz AbortPdu::decode
    {
        let mut r = Reader::new(data);
        let _ = AbortPdu::decode(&mut r);
    }
});
