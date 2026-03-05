#![no_main]
use libfuzzer_sys::fuzz_target;
use rustbac_core::encoding::reader::Reader;
use rustbac_core::services::{
    alarm_summary::GetAlarmSummaryAck,
    cov_notification::CovNotificationRequest,
    event_notification::EventNotificationRequest,
    i_am::IAmRequest,
    read_property::ReadPropertyAck,
    read_property_multiple::ReadPropertyMultipleAck,
};

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // Use the first byte to pick which service decoder to exercise.
    let selector = data[0] % 6;
    let payload = &data[1..];

    match selector {
        0 => {
            // ReadPropertyAck::decode_after_header
            let mut r = Reader::new(payload);
            let _ = ReadPropertyAck::decode_after_header(&mut r);
        }
        1 => {
            // ReadPropertyMultipleAck::decode_after_header
            let mut r = Reader::new(payload);
            let _ = ReadPropertyMultipleAck::decode_after_header(&mut r);
        }
        2 => {
            // CovNotificationRequest::decode_after_header
            let mut r = Reader::new(payload);
            let _ = CovNotificationRequest::decode_after_header(&mut r);
        }
        3 => {
            // EventNotificationRequest::decode_after_header
            let mut r = Reader::new(payload);
            let _ = EventNotificationRequest::decode_after_header(&mut r);
        }
        4 => {
            // IAmRequest::decode_after_header
            let mut r = Reader::new(payload);
            let _ = IAmRequest::decode_after_header(&mut r);
        }
        _ => {
            // GetAlarmSummaryAck::decode_after_header
            let mut r = Reader::new(payload);
            let _ = GetAlarmSummaryAck::decode_after_header(&mut r);
        }
    }
});
