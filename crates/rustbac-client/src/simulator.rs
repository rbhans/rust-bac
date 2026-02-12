//! Lightweight simulated BACnet device.
//!
//! [`SimulatedDevice`] responds to Who-Is, ReadProperty, and WriteProperty
//! requests. Useful for testing and development without physical hardware.

use crate::{ClientDataValue, ClientError};
use rustbac_core::apdu::{
    ApduType, ComplexAckHeader, ConfirmedRequestHeader, SimpleAck, UnconfirmedRequestHeader,
};
use rustbac_core::encoding::{
    primitives::{decode_unsigned, encode_ctx_unsigned},
    reader::Reader,
    tag::Tag,
    writer::Writer,
};
use rustbac_core::npdu::Npdu;
use rustbac_core::services::i_am::IAmRequest;
use rustbac_core::services::read_property::SERVICE_READ_PROPERTY;
use rustbac_core::services::value_codec::encode_application_data_value;
use rustbac_core::services::write_property::SERVICE_WRITE_PROPERTY;
use rustbac_core::types::{DataValue, ObjectId, ObjectType, PropertyId};
use rustbac_datalink::{DataLink, DataLinkAddress};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A simulated BACnet device.
pub struct SimulatedDevice<D: DataLink> {
    pub device_id: ObjectId,
    objects: Arc<RwLock<HashMap<ObjectId, HashMap<PropertyId, ClientDataValue>>>>,
    datalink: D,
}

impl<D: DataLink> SimulatedDevice<D> {
    /// Create a new simulated device with the given instance number.
    pub fn new(instance: u32, datalink: D) -> Self {
        let device_id = ObjectId::new(ObjectType::Device, instance);
        let mut device_props = HashMap::new();
        device_props.insert(
            PropertyId::ObjectIdentifier,
            ClientDataValue::ObjectId(device_id),
        );
        device_props.insert(
            PropertyId::ObjectName,
            ClientDataValue::CharacterString(format!("SimDevice-{instance}")),
        );
        device_props.insert(
            PropertyId::ObjectType,
            ClientDataValue::Enumerated(ObjectType::Device.to_u16() as u32),
        );

        let mut objects = HashMap::new();
        objects.insert(device_id, device_props);

        Self {
            device_id,
            objects: Arc::new(RwLock::new(objects)),
            datalink,
        }
    }

    /// Add an object with its properties to the simulated device.
    pub async fn add_object(&self, id: ObjectId, properties: HashMap<PropertyId, ClientDataValue>) {
        self.objects.write().await.insert(id, properties);
    }

    /// Run the device loop, responding to incoming requests until stopped.
    pub async fn run(&self) -> Result<(), ClientError> {
        let mut buf = [0u8; 1500];
        loop {
            let (n, source) = self.datalink.recv(&mut buf).await?;
            if let Err(e) = self.handle_frame(&buf[..n], source).await {
                log::debug!("simulator: error handling frame: {e}");
            }
        }
    }

    async fn handle_frame(&self, frame: &[u8], source: DataLinkAddress) -> Result<(), ClientError> {
        let mut r = Reader::new(frame);
        let _npdu = Npdu::decode(&mut r)?;

        if r.is_empty() {
            return Ok(());
        }

        let first = r.peek_u8()?;
        let apdu_type = ApduType::from_u8(first >> 4);

        match apdu_type {
            Some(ApduType::UnconfirmedRequest) => {
                let header = UnconfirmedRequestHeader::decode(&mut r)?;
                if header.service_choice == 0x08 {
                    // Who-Is — decode optional limits from remaining payload.
                    let who_is_limits = self.decode_who_is_limits(&mut r);
                    if self.matches_who_is(who_is_limits) {
                        self.send_i_am(source).await?;
                    }
                }
            }
            Some(ApduType::ConfirmedRequest) => {
                let header = ConfirmedRequestHeader::decode(&mut r)?;
                match header.service_choice {
                    SERVICE_READ_PROPERTY => {
                        self.handle_read_property(&mut r, header.invoke_id, source)
                            .await?;
                    }
                    SERVICE_WRITE_PROPERTY => {
                        self.handle_write_property(&mut r, header.invoke_id, source)
                            .await?;
                    }
                    _ => {
                        // Unknown service — ignore.
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn decode_who_is_limits(&self, r: &mut Reader<'_>) -> Option<(u32, u32)> {
        // Who-Is has optional [0] low-limit, [1] high-limit.
        if r.is_empty() {
            return None; // Global Who-Is — no limits.
        }
        let tag0 = Tag::decode(r).ok()?;
        let low = match tag0 {
            Tag::Context { tag_num: 0, len } => decode_unsigned(r, len as usize).ok()?,
            _ => return None,
        };
        let tag1 = Tag::decode(r).ok()?;
        let high = match tag1 {
            Tag::Context { tag_num: 1, len } => decode_unsigned(r, len as usize).ok()?,
            _ => return None,
        };
        Some((low, high))
    }

    fn matches_who_is(&self, limits: Option<(u32, u32)>) -> bool {
        let instance = self.device_id.instance();
        match limits {
            None => true, // Global Who-Is.
            Some((low, high)) => instance >= low && instance <= high,
        }
    }

    async fn send_i_am(&self, target: DataLinkAddress) -> Result<(), ClientError> {
        let req = IAmRequest {
            device_id: self.device_id,
            max_apdu: 1476,
            segmentation: 3, // no-segmentation
            vendor_id: 0,
        };

        let mut buf = [0u8; 128];
        let mut w = Writer::new(&mut buf);
        Npdu::new(0).encode(&mut w)?;
        req.encode(&mut w)?;
        let data = w.as_written();
        self.datalink.send(target, data).await?;
        Ok(())
    }

    async fn handle_read_property(
        &self,
        r: &mut Reader<'_>,
        invoke_id: u8,
        source: DataLinkAddress,
    ) -> Result<(), ClientError> {
        // ReadPropertyRequest has no decode method — decode manually.
        let object_id = crate::decode_ctx_object_id(r)?;
        let property_id = PropertyId::from_u32(crate::decode_ctx_unsigned(r)?);
        let objects = self.objects.read().await;

        let value = objects
            .get(&object_id)
            .and_then(|props| props.get(&property_id));

        match value {
            Some(val) => {
                let borrowed = client_value_to_borrowed(val);
                let mut buf = [0u8; 1400];
                let mut w = Writer::new(&mut buf);
                Npdu::new(0).encode(&mut w)?;
                ComplexAckHeader {
                    segmented: false,
                    more_follows: false,
                    invoke_id,
                    sequence_number: None,
                    proposed_window_size: None,
                    service_choice: SERVICE_READ_PROPERTY,
                }
                .encode(&mut w)?;
                // Encode the ReadPropertyAck payload manually.
                encode_ctx_unsigned(&mut w, 0, object_id.raw())?;
                encode_ctx_unsigned(&mut w, 1, property_id.to_u32())?;
                Tag::Opening { tag_num: 3 }.encode(&mut w)?;
                encode_application_data_value(&mut w, &borrowed)?;
                Tag::Closing { tag_num: 3 }.encode(&mut w)?;
                let data = w.as_written();
                self.datalink.send(source, data).await?;
            }
            None => {
                // Send error: unknown-property.
                let mut buf = [0u8; 64];
                let mut w = Writer::new(&mut buf);
                Npdu::new(0).encode(&mut w)?;
                // BACnet Error PDU: type=5, invoke_id, service_choice, error_class, error_code
                w.write_u8(0x50)?; // Error PDU type (5 << 4)
                w.write_u8(invoke_id)?;
                w.write_u8(SERVICE_READ_PROPERTY)?;
                // error-class: property (2), error-code: unknown-property (32)
                Tag::Application {
                    tag: rustbac_core::encoding::tag::AppTag::Enumerated,
                    len: 1,
                }
                .encode(&mut w)?;
                w.write_u8(2)?; // property
                Tag::Application {
                    tag: rustbac_core::encoding::tag::AppTag::Enumerated,
                    len: 1,
                }
                .encode(&mut w)?;
                w.write_u8(32)?; // unknown-property
                let data = w.as_written();
                self.datalink.send(source, data).await?;
            }
        }

        Ok(())
    }

    async fn handle_write_property(
        &self,
        r: &mut Reader<'_>,
        invoke_id: u8,
        source: DataLinkAddress,
    ) -> Result<(), ClientError> {
        // Decode object_id [0], property_id [1], optional array_index [2], value [3]
        let object_id = crate::decode_ctx_object_id(r)?;
        let property_id_raw = crate::decode_ctx_unsigned(r)?;
        let property_id = PropertyId::from_u32(property_id_raw);

        let next_tag = Tag::decode(r)?;
        let value_start_tag = match next_tag {
            Tag::Context { tag_num: 2, len } => {
                let _array_index = decode_unsigned(r, len as usize)?;
                Tag::decode(r)?
            }
            other => other,
        };
        if value_start_tag != (Tag::Opening { tag_num: 3 }) {
            return Err(rustbac_core::DecodeError::InvalidTag.into());
        }
        let val = rustbac_core::services::value_codec::decode_application_data_value(r)?;
        match Tag::decode(r)? {
            Tag::Closing { tag_num: 3 } => {}
            _ => return Err(rustbac_core::DecodeError::InvalidTag.into()),
        }

        let client_val = crate::data_value_to_client(val);
        let mut objects = self.objects.write().await;
        if let Some(props) = objects.get_mut(&object_id) {
            props.insert(property_id, client_val);
        }

        // Send SimpleAck
        let mut buf = [0u8; 32];
        let mut w = Writer::new(&mut buf);
        Npdu::new(0).encode(&mut w)?;
        SimpleAck {
            invoke_id,
            service_choice: SERVICE_WRITE_PROPERTY,
        }
        .encode(&mut w)?;
        let data = w.as_written();
        self.datalink.send(source, data).await?;

        Ok(())
    }
}

/// Convert an owned ClientDataValue to a borrowed DataValue.
///
/// This is a shallow conversion — strings and byte arrays reference the owned data.
fn client_value_to_borrowed(val: &ClientDataValue) -> DataValue<'_> {
    match val {
        ClientDataValue::Null => DataValue::Null,
        ClientDataValue::Boolean(v) => DataValue::Boolean(*v),
        ClientDataValue::Unsigned(v) => DataValue::Unsigned(*v),
        ClientDataValue::Signed(v) => DataValue::Signed(*v),
        ClientDataValue::Real(v) => DataValue::Real(*v),
        ClientDataValue::Double(v) => DataValue::Double(*v),
        ClientDataValue::OctetString(v) => DataValue::OctetString(v),
        ClientDataValue::CharacterString(v) => DataValue::CharacterString(v),
        ClientDataValue::BitString { unused_bits, data } => {
            DataValue::BitString(rustbac_core::types::BitString {
                unused_bits: *unused_bits,
                data,
            })
        }
        ClientDataValue::Enumerated(v) => DataValue::Enumerated(*v),
        ClientDataValue::Date(v) => DataValue::Date(*v),
        ClientDataValue::Time(v) => DataValue::Time(*v),
        ClientDataValue::ObjectId(v) => DataValue::ObjectId(*v),
        ClientDataValue::Constructed { tag_num, values } => DataValue::Constructed {
            tag_num: *tag_num,
            values: values.iter().map(client_value_to_borrowed).collect(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustbac_core::encoding::{primitives::encode_ctx_unsigned, reader::Reader, writer::Writer};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MockDataLink {
        sent: Arc<Mutex<Vec<(DataLinkAddress, Vec<u8>)>>>,
    }

    impl DataLink for MockDataLink {
        async fn send(
            &self,
            address: DataLinkAddress,
            payload: &[u8],
        ) -> Result<(), rustbac_datalink::DataLinkError> {
            self.sent
                .lock()
                .expect("poisoned lock")
                .push((address, payload.to_vec()));
            Ok(())
        }

        async fn recv(
            &self,
            _buf: &mut [u8],
        ) -> Result<(usize, DataLinkAddress), rustbac_datalink::DataLinkError> {
            Err(rustbac_datalink::DataLinkError::InvalidFrame)
        }
    }

    #[tokio::test]
    async fn handle_write_property_accepts_optional_array_index() {
        let dl = MockDataLink::default();
        let sent = dl.sent.clone();
        let sim = SimulatedDevice::new(1, dl);

        let mut payload = [0u8; 256];
        let mut w = Writer::new(&mut payload);
        encode_ctx_unsigned(&mut w, 0, sim.device_id.raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, PropertyId::ObjectName.to_u32()).unwrap();
        encode_ctx_unsigned(&mut w, 2, 0).unwrap();
        Tag::Opening { tag_num: 3 }.encode(&mut w).unwrap();
        rustbac_core::services::value_codec::encode_application_data_value(
            &mut w,
            &DataValue::CharacterString("updated-name"),
        )
        .unwrap();
        Tag::Closing { tag_num: 3 }.encode(&mut w).unwrap();

        let source = DataLinkAddress::Ip("127.0.0.1:47808".parse().unwrap());
        let mut r = Reader::new(w.as_written());
        sim.handle_write_property(&mut r, 9, source).await.unwrap();

        let objects = sim.objects.read().await;
        let props = objects.get(&sim.device_id).unwrap();
        assert_eq!(
            props.get(&PropertyId::ObjectName),
            Some(&ClientDataValue::CharacterString(
                "updated-name".to_string()
            ))
        );

        let sent = sent.lock().expect("poisoned lock");
        assert_eq!(sent.len(), 1);
        let mut ack_reader = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut ack_reader).unwrap();
        let ack = SimpleAck::decode(&mut ack_reader).unwrap();
        assert_eq!(ack.invoke_id, 9);
        assert_eq!(ack.service_choice, SERVICE_WRITE_PROPERTY);
    }
}
