use crate::apdu::ConfirmedRequestHeader;
use crate::encoding::{
    primitives::{encode_ctx_character_string, encode_ctx_unsigned},
    writer::Writer,
};
use crate::EncodeError;

pub const SERVICE_DEVICE_COMMUNICATION_CONTROL: u8 = 0x11;
pub const SERVICE_REINITIALIZE_DEVICE: u8 = 0x14;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DeviceCommunicationState {
    Enable = 0,
    Disable = 1,
    DisableInitiation = 2,
}

impl DeviceCommunicationState {
    pub const fn to_u32(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ReinitializeState {
    Coldstart = 0,
    Warmstart = 1,
    StartBackup = 2,
    EndBackup = 3,
    StartRestore = 4,
    EndRestore = 5,
    AbortRestore = 6,
    ActivateChanges = 7,
}

impl ReinitializeState {
    pub const fn to_u32(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceCommunicationControlRequest<'a> {
    pub time_duration_seconds: Option<u16>,
    pub enable_disable: DeviceCommunicationState,
    pub password: Option<&'a str>,
    pub invoke_id: u8,
}

impl<'a> DeviceCommunicationControlRequest<'a> {
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        ConfirmedRequestHeader {
            segmented: false,
            more_follows: false,
            segmented_response_accepted: false,
            max_segments: 0,
            max_apdu: 5,
            invoke_id: self.invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_DEVICE_COMMUNICATION_CONTROL,
        }
        .encode(w)?;
        if let Some(duration) = self.time_duration_seconds {
            encode_ctx_unsigned(w, 0, duration as u32)?;
        }
        encode_ctx_unsigned(w, 1, self.enable_disable.to_u32())?;
        if let Some(password) = self.password {
            encode_ctx_character_string(w, 2, password)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReinitializeDeviceRequest<'a> {
    pub state: ReinitializeState,
    pub password: Option<&'a str>,
    pub invoke_id: u8,
}

impl<'a> ReinitializeDeviceRequest<'a> {
    pub fn encode(&self, w: &mut Writer<'_>) -> Result<(), EncodeError> {
        ConfirmedRequestHeader {
            segmented: false,
            more_follows: false,
            segmented_response_accepted: false,
            max_segments: 0,
            max_apdu: 5,
            invoke_id: self.invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_REINITIALIZE_DEVICE,
        }
        .encode(w)?;
        encode_ctx_unsigned(w, 0, self.state.to_u32())?;
        if let Some(password) = self.password {
            encode_ctx_character_string(w, 1, password)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DeviceCommunicationControlRequest, DeviceCommunicationState, ReinitializeDeviceRequest,
        ReinitializeState, SERVICE_DEVICE_COMMUNICATION_CONTROL, SERVICE_REINITIALIZE_DEVICE,
    };
    use crate::apdu::ConfirmedRequestHeader;
    use crate::encoding::{reader::Reader, writer::Writer};

    #[test]
    fn encode_device_communication_control_request() {
        let req = DeviceCommunicationControlRequest {
            time_duration_seconds: Some(120),
            enable_disable: DeviceCommunicationState::Disable,
            password: Some("secret"),
            invoke_id: 7,
        };
        let mut buf = [0u8; 96];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_DEVICE_COMMUNICATION_CONTROL);
        assert_eq!(hdr.invoke_id, 7);
    }

    #[test]
    fn encode_reinitialize_device_request() {
        let req = ReinitializeDeviceRequest {
            state: ReinitializeState::ActivateChanges,
            password: None,
            invoke_id: 11,
        };
        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        req.encode(&mut w).unwrap();

        let mut r = Reader::new(w.as_written());
        let hdr = ConfirmedRequestHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.service_choice, SERVICE_REINITIALIZE_DEVICE);
        assert_eq!(hdr.invoke_id, 11);
    }
}
