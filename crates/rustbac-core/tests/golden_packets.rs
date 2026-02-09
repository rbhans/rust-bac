#[cfg(feature = "alloc")]
use rustbac_core::encoding::reader::Reader;
use rustbac_core::encoding::writer::Writer;
use rustbac_core::npdu::Npdu;
use rustbac_core::services::acknowledge_alarm::{AcknowledgeAlarmRequest, EventState, TimeStamp};
use rustbac_core::services::alarm_summary::GetAlarmSummaryRequest;
use rustbac_core::services::atomic_read_file::AtomicReadFileRequest;
use rustbac_core::services::atomic_write_file::AtomicWriteFileRequest;
use rustbac_core::services::device_management::{
    DeviceCommunicationControlRequest, DeviceCommunicationState, ReinitializeDeviceRequest,
    ReinitializeState,
};
use rustbac_core::services::enrollment_summary::GetEnrollmentSummaryRequest;
use rustbac_core::services::event_information::GetEventInformationRequest;
use rustbac_core::services::list_element::{AddListElementRequest, RemoveListElementRequest};
use rustbac_core::services::object_management::{CreateObjectRequest, DeleteObjectRequest};
use rustbac_core::services::read_property::ReadPropertyRequest;
use rustbac_core::services::read_range::ReadRangeRequest;
use rustbac_core::services::subscribe_cov::SubscribeCovRequest;
use rustbac_core::services::time_synchronization::TimeSynchronizationRequest;
use rustbac_core::services::who_has::WhoHasRequest;
use rustbac_core::services::who_is::WhoIsRequest;
use rustbac_core::types::{Date, ObjectId, ObjectType, PropertyId, Time};

#[test]
fn who_is_global_frame_matches_fixture() {
    let mut buf = [0u8; 32];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    WhoIsRequest::global().encode(&mut w).unwrap();

    assert_eq!(w.as_written(), &[0x01, 0x00, 0x10, 0x08]);
}

#[test]
fn read_property_frame_matches_fixture() {
    let mut buf = [0u8; 64];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    ReadPropertyRequest {
        object_id: ObjectId::new(ObjectType::Device, 123),
        property_id: PropertyId::ObjectName,
        array_index: None,
        invoke_id: 1,
    }
    .encode(&mut w)
    .unwrap();

    assert_eq!(
        w.as_written(),
        &[0x01, 0x00, 0x02, 0x05, 0x01, 0x0C, 0x0C, 0x02, 0x00, 0x00, 0x7B, 0x19, 0x4D,]
    );
}

#[test]
fn subscribe_cov_frame_matches_fixture() {
    let mut buf = [0u8; 64];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    SubscribeCovRequest {
        subscriber_process_id: 7,
        monitored_object_id: ObjectId::new(ObjectType::AnalogInput, 2),
        issue_confirmed_notifications: Some(false),
        lifetime_seconds: Some(600),
        invoke_id: 17,
    }
    .encode(&mut w)
    .unwrap();

    assert_eq!(
        w.as_written(),
        &[
            0x01, 0x00, 0x00, 0x05, 0x11, 0x05, 0x09, 0x07, 0x1C, 0x00, 0x00, 0x00, 0x02, 0x29,
            0x00, 0x3A, 0x02, 0x58,
        ]
    );
}

#[test]
fn read_range_frame_matches_fixture() {
    let mut buf = [0u8; 96];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    ReadRangeRequest::by_position(
        ObjectId::new(ObjectType::TrendLog, 1),
        PropertyId::PresentValue,
        None,
        1,
        2,
        4,
    )
    .encode(&mut w)
    .unwrap();

    assert_eq!(
        w.as_written(),
        &[
            0x01, 0x00, 0x02, 0x05, 0x04, 0x1A, 0x0C, 0x05, 0x00, 0x00, 0x01, 0x19, 0x55, 0x3E,
            0x21, 0x01, 0x31, 0x02, 0x3F,
        ]
    );
}

#[test]
fn who_has_by_object_id_frame_matches_fixture() {
    let mut buf = [0u8; 32];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    WhoHasRequest::for_object_id(ObjectId::new(ObjectType::AnalogInput, 2))
        .encode(&mut w)
        .unwrap();

    assert_eq!(
        w.as_written(),
        &[0x01, 0x00, 0x10, 0x07, 0x2C, 0x00, 0x00, 0x00, 0x02]
    );
}

#[test]
fn dcc_frame_matches_fixture() {
    let mut buf = [0u8; 64];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    DeviceCommunicationControlRequest {
        time_duration_seconds: Some(120),
        enable_disable: DeviceCommunicationState::Disable,
        password: None,
        invoke_id: 7,
    }
    .encode(&mut w)
    .unwrap();

    assert_eq!(
        w.as_written(),
        &[0x01, 0x00, 0x00, 0x05, 0x07, 0x11, 0x09, 0x78, 0x19, 0x01]
    );
}

#[test]
fn reinitialize_frame_matches_fixture() {
    let mut buf = [0u8; 64];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    ReinitializeDeviceRequest {
        state: ReinitializeState::Warmstart,
        password: None,
        invoke_id: 9,
    }
    .encode(&mut w)
    .unwrap();

    assert_eq!(
        w.as_written(),
        &[0x01, 0x00, 0x00, 0x05, 0x09, 0x14, 0x09, 0x01]
    );
}

#[test]
fn time_sync_frame_matches_fixture() {
    let mut buf = [0u8; 64];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    TimeSynchronizationRequest::local(
        Date {
            year_since_1900: 126,
            month: 2,
            day: 7,
            weekday: 6,
        },
        Time {
            hour: 10,
            minute: 11,
            second: 12,
            hundredths: 13,
        },
    )
    .encode(&mut w)
    .unwrap();

    assert_eq!(
        w.as_written(),
        &[0x01, 0x00, 0x10, 0x06, 0xA4, 0x7E, 0x02, 0x07, 0x06, 0xB4, 0x0A, 0x0B, 0x0C, 0x0D,]
    );
}

#[test]
fn atomic_read_file_stream_frame_matches_fixture() {
    let mut buf = [0u8; 96];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    AtomicReadFileRequest::stream(ObjectId::new(ObjectType::File, 7), 0, 512, 4)
        .encode(&mut w)
        .unwrap();

    assert_eq!(
        w.as_written(),
        &[
            0x01, 0x00, 0x02, 0x05, 0x04, 0x06, 0xC4, 0x02, 0x80, 0x00, 0x07, 0x0E, 0x31, 0x00,
            0x22, 0x02, 0x00, 0x0F,
        ]
    );
}

#[test]
fn atomic_write_file_stream_frame_matches_fixture() {
    let mut buf = [0u8; 96];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    AtomicWriteFileRequest::stream(
        ObjectId::new(ObjectType::File, 3),
        128,
        &[0xAA, 0xBB, 0xCC],
        5,
    )
    .encode(&mut w)
    .unwrap();

    assert_eq!(
        w.as_written(),
        &[
            0x01, 0x00, 0x00, 0x05, 0x05, 0x07, 0xC4, 0x02, 0x80, 0x00, 0x03, 0x0E, 0x32, 0x00,
            0x80, 0x63, 0xAA, 0xBB, 0xCC, 0x0F,
        ]
    );
}

#[test]
fn acknowledge_alarm_frame_matches_fixture() {
    let mut buf = [0u8; 160];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    AcknowledgeAlarmRequest {
        acknowledging_process_id: 10,
        event_object_id: ObjectId::new(ObjectType::AnalogInput, 1),
        event_state_acknowledged: EventState::Offnormal,
        event_time_stamp: TimeStamp::SequenceNumber(42),
        acknowledgment_source: "operator",
        time_of_acknowledgment: TimeStamp::SequenceNumber(43),
        invoke_id: 11,
    }
    .encode(&mut w)
    .unwrap();

    assert_eq!(
        w.as_written(),
        &[
            0x01, 0x00, 0x00, 0x05, 0x0B, 0x00, 0x09, 0x0A, 0x1C, 0x00, 0x00, 0x00, 0x01, 0x29,
            0x02, 0x3E, 0x19, 0x2A, 0x3F, 0x4D, 0x09, 0x00, 0x6F, 0x70, 0x65, 0x72, 0x61, 0x74,
            0x6F, 0x72, 0x5E, 0x19, 0x2B, 0x5F,
        ]
    );
}

#[test]
fn get_alarm_summary_frame_matches_fixture() {
    let mut buf = [0u8; 32];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    GetAlarmSummaryRequest { invoke_id: 12 }
        .encode(&mut w)
        .unwrap();

    assert_eq!(w.as_written(), &[0x01, 0x00, 0x02, 0x05, 0x0C, 0x03]);
}

#[test]
fn get_enrollment_summary_frame_matches_fixture() {
    let mut buf = [0u8; 32];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    GetEnrollmentSummaryRequest { invoke_id: 13 }
        .encode(&mut w)
        .unwrap();

    assert_eq!(w.as_written(), &[0x01, 0x00, 0x02, 0x05, 0x0D, 0x04]);
}

#[test]
fn get_event_information_frame_matches_fixture() {
    let mut buf = [0u8; 32];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    GetEventInformationRequest {
        last_received_object_id: None,
        invoke_id: 14,
    }
    .encode(&mut w)
    .unwrap();

    assert_eq!(w.as_written(), &[0x01, 0x00, 0x02, 0x05, 0x0E, 0x1D]);
}

#[test]
fn create_object_frame_matches_fixture() {
    let mut buf = [0u8; 64];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    CreateObjectRequest::by_type(ObjectType::AnalogValue, 6)
        .encode(&mut w)
        .unwrap();

    assert_eq!(
        w.as_written(),
        &[0x01, 0x00, 0x02, 0x05, 0x06, 0x0A, 0x09, 0x02]
    );
}

#[test]
fn delete_object_frame_matches_fixture() {
    let mut buf = [0u8; 64];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).unwrap();
    DeleteObjectRequest {
        object_id: ObjectId::new(ObjectType::AnalogValue, 42),
        invoke_id: 7,
    }
    .encode(&mut w)
    .unwrap();

    assert_eq!(
        w.as_written(),
        &[0x01, 0x00, 0x00, 0x05, 0x07, 0x0B, 0x0C, 0x00, 0x80, 0x00, 0x2A]
    );
}

#[test]
fn add_list_element_frame_matches_fixture() {
    let mut buf = [0u8; 96];
    let mut w = Writer::new(&mut buf);
    let values = [
        rustbac_core::types::DataValue::Unsigned(1),
        rustbac_core::types::DataValue::Unsigned(2),
    ];
    Npdu::new(0).encode(&mut w).unwrap();
    AddListElementRequest {
        object_id: ObjectId::new(ObjectType::AnalogValue, 1),
        property_id: PropertyId::Proprietary(512),
        array_index: None,
        elements: &values,
        invoke_id: 8,
    }
    .encode(&mut w)
    .unwrap();

    assert_eq!(
        w.as_written(),
        &[
            0x01, 0x00, 0x00, 0x05, 0x08, 0x08, 0x0C, 0x00, 0x80, 0x00, 0x01, 0x1A, 0x02, 0x00,
            0x3E, 0x21, 0x01, 0x21, 0x02, 0x3F,
        ]
    );
}

#[test]
fn remove_list_element_frame_matches_fixture() {
    let mut buf = [0u8; 96];
    let mut w = Writer::new(&mut buf);
    let values = [rustbac_core::types::DataValue::Unsigned(1)];
    Npdu::new(0).encode(&mut w).unwrap();
    RemoveListElementRequest {
        object_id: ObjectId::new(ObjectType::AnalogValue, 1),
        property_id: PropertyId::Proprietary(513),
        array_index: None,
        elements: &values,
        invoke_id: 9,
    }
    .encode(&mut w)
    .unwrap();

    assert_eq!(
        w.as_written(),
        &[
            0x01, 0x00, 0x00, 0x05, 0x09, 0x09, 0x0C, 0x00, 0x80, 0x00, 0x01, 0x1A, 0x02, 0x01,
            0x3E, 0x21, 0x01, 0x3F,
        ]
    );
}

#[cfg(feature = "alloc")]
#[test]
fn cov_notification_fixture_decodes_expected() {
    use rustbac_core::apdu::UnconfirmedRequestHeader;
    use rustbac_core::services::cov_notification::{
        CovNotificationRequest, SERVICE_UNCONFIRMED_COV_NOTIFICATION,
    };

    let fixture = [
        0x10, 0x02, // unconfirmed COV
        0x09, 0x11, // [0] process id 17
        0x1C, 0x02, 0x00, 0x00, 0x01, // [1] initiating device: device,1
        0x2C, 0x00, 0x00, 0x00, 0x01, // [2] monitored object: analog-input,1
        0x39, 0x3C, // [3] time remaining 60
        0x4E, // [4] opening listOfValues
        0x09, 0x55, // [0] present-value
        0x2E, // [2] opening value
        0x44, 0x42, 0x20, 0x00, 0x00, // real 40.0
        0x2F, // [2] closing value
        0x4F, // [4] closing listOfValues
    ];

    let mut r = Reader::new(&fixture);
    let header = UnconfirmedRequestHeader::decode(&mut r).unwrap();
    assert_eq!(header.service_choice, SERVICE_UNCONFIRMED_COV_NOTIFICATION);

    let cov = CovNotificationRequest::decode_after_header(&mut r).unwrap();
    assert_eq!(cov.subscriber_process_id, 17);
    assert_eq!(cov.values.len(), 1);
    assert_eq!(cov.values[0].property_id, PropertyId::PresentValue);
}
