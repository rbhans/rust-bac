pub mod acknowledge_alarm;
pub mod alarm_summary;
pub mod atomic_read_file;
pub mod atomic_write_file;
pub mod cov_notification;
pub mod device_management;
pub mod enrollment_summary;
pub mod event_information;
pub mod event_notification;
pub mod i_am;
pub mod list_element;
pub mod object_management;
pub mod private_transfer;
pub mod read_property;
pub mod read_property_multiple;
pub mod read_range;
pub mod subscribe_cov;
pub mod subscribe_cov_property;
pub mod time_synchronization;
pub mod value_codec;
pub mod who_has;
pub mod who_is;
pub mod write_property;
pub mod write_property_multiple;

#[cfg(feature = "alloc")]
use crate::encoding::{primitives::decode_unsigned, reader::Reader, tag::Tag};
#[cfg(feature = "alloc")]
use crate::types::ObjectId;
#[cfg(feature = "alloc")]
use crate::DecodeError;

/// Decode a required context-tagged unsigned integer at the expected tag number.
#[cfg(feature = "alloc")]
pub(crate) fn decode_required_ctx_unsigned(
    r: &mut Reader<'_>,
    expected_tag_num: u8,
) -> Result<u32, DecodeError> {
    match Tag::decode(r)? {
        Tag::Context { tag_num, len } if tag_num == expected_tag_num => {
            decode_unsigned(r, len as usize)
        }
        _ => Err(DecodeError::InvalidTag),
    }
}

/// Decode a required context-tagged BACnet object identifier at the expected tag number.
#[cfg(feature = "alloc")]
pub(crate) fn decode_required_ctx_object_id(
    r: &mut Reader<'_>,
    expected_tag_num: u8,
) -> Result<ObjectId, DecodeError> {
    Ok(ObjectId::from_raw(decode_required_ctx_unsigned(
        r,
        expected_tag_num,
    )?))
}
