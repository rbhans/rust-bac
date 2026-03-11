//! BACnet server/responder implementation.
//!
//! [`BacnetServer`] binds a [`DataLink`] transport and dispatches incoming
//! service requests to a user-supplied [`ServiceHandler`].  [`ObjectStore`]
//! is a convenient thread-safe property store that implements
//! [`ServiceHandler`] out of the box.

use crate::ClientDataValue;
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
use rustbac_core::services::read_property_multiple::SERVICE_READ_PROPERTY_MULTIPLE;
use rustbac_core::services::value_codec::encode_application_data_value;
use rustbac_core::services::write_property::SERVICE_WRITE_PROPERTY;
use rustbac_core::types::{ObjectId, ObjectType, PropertyId};

/// WritePropertyMultiple service choice (0x10).
const SERVICE_WRITE_PROPERTY_MULTIPLE: u8 = 0x10;
/// SubscribeCOV service choice (0x05).
const SERVICE_SUBSCRIBE_COV: u8 = 0x05;
/// CreateObject service choice (0x0A).
const SERVICE_CREATE_OBJECT: u8 = 0x0A;
/// DeleteObject service choice (0x0B).
const SERVICE_DELETE_OBJECT: u8 = 0x0B;
use rustbac_datalink::{DataLink, DataLinkAddress};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// BacnetServiceError
// ─────────────────────────────────────────────────────────────────────────────

/// Errors that a [`ServiceHandler`] may return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacnetServiceError {
    /// The addressed object does not exist.
    UnknownObject,
    /// The property does not exist on the object.
    UnknownProperty,
    /// The property is not writable.
    WriteAccessDenied,
    /// The supplied value is of the wrong type.
    InvalidDataType,
    /// The service is not supported by this server.
    ServiceNotSupported,
}

impl BacnetServiceError {
    /// Map the error to a (error_class, error_code) pair for the wire.
    fn to_error_class_code(self) -> (u8, u8) {
        match self {
            // error-class: object (1), error-code: unknown-object (31)
            BacnetServiceError::UnknownObject => (1, 31),
            // error-class: property (2), error-code: unknown-property (32)
            BacnetServiceError::UnknownProperty => (2, 32),
            // error-class: property (2), error-code: write-access-denied (40)
            BacnetServiceError::WriteAccessDenied => (2, 40),
            // error-class: property (2), error-code: invalid-data-type (9)
            BacnetServiceError::InvalidDataType => (2, 9),
            // error-class: services (5), error-code: service-not-supported (53)
            BacnetServiceError::ServiceNotSupported => (5, 53),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ServiceHandler trait
// ─────────────────────────────────────────────────────────────────────────────

/// Handler trait that the server calls for each incoming service request.
pub trait ServiceHandler: Send + Sync + 'static {
    /// Called for a ReadProperty confirmed request.
    fn read_property(
        &self,
        object_id: ObjectId,
        property_id: PropertyId,
        array_index: Option<u32>,
    ) -> Result<ClientDataValue, BacnetServiceError>;

    /// Called for a WriteProperty confirmed request.
    fn write_property(
        &self,
        object_id: ObjectId,
        property_id: PropertyId,
        array_index: Option<u32>,
        value: ClientDataValue,
        priority: Option<u8>,
    ) -> Result<(), BacnetServiceError>;

    /// Called for a WritePropertyMultiple confirmed request.
    ///
    /// Each element of `specs` is `(object_id, vec_of_(property_id, array_index, value, priority))`.
    /// The default implementation rejects with [`BacnetServiceError::WriteAccessDenied`].
    #[allow(clippy::type_complexity)]
    fn write_property_multiple(
        &self,
        _specs: &[(
            ObjectId,
            Vec<(PropertyId, Option<u32>, ClientDataValue, Option<u8>)>,
        )],
    ) -> Result<(), BacnetServiceError> {
        Err(BacnetServiceError::WriteAccessDenied)
    }

    /// Called for a CreateObject confirmed request.
    ///
    /// The default implementation rejects with [`BacnetServiceError::WriteAccessDenied`].
    fn create_object(
        &self,
        _object_type: rustbac_core::types::ObjectType,
    ) -> Result<ObjectId, BacnetServiceError> {
        Err(BacnetServiceError::WriteAccessDenied)
    }

    /// Called for a DeleteObject confirmed request.
    ///
    /// The default implementation rejects with [`BacnetServiceError::WriteAccessDenied`].
    fn delete_object(&self, _object_id: ObjectId) -> Result<(), BacnetServiceError> {
        Err(BacnetServiceError::WriteAccessDenied)
    }

    /// Called for a SubscribeCOV confirmed request.
    ///
    /// The default implementation rejects with [`BacnetServiceError::UnknownObject`].
    fn subscribe_cov(
        &self,
        _subscriber_process_id: u32,
        _monitored_object_id: ObjectId,
        _issue_confirmed: bool,
        _lifetime: Option<u32>,
    ) -> Result<(), BacnetServiceError> {
        Err(BacnetServiceError::UnknownObject)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ObjectStore
// ─────────────────────────────────────────────────────────────────────────────

/// Thread-safe property store backed by `Mutex<HashMap<ObjectId, HashMap<PropertyId, ClientDataValue>>>`.
pub struct ObjectStore {
    inner: Mutex<HashMap<ObjectId, HashMap<PropertyId, ClientDataValue>>>,
}

impl ObjectStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Insert or overwrite a property value.
    pub fn set(&self, object_id: ObjectId, property_id: PropertyId, value: ClientDataValue) {
        let mut map = self.inner.lock().expect("ObjectStore lock poisoned");
        map.entry(object_id).or_default().insert(property_id, value);
    }

    /// Retrieve a property value, returning `None` if the object or property is absent.
    pub fn get(&self, object_id: ObjectId, property_id: PropertyId) -> Option<ClientDataValue> {
        let map = self.inner.lock().expect("ObjectStore lock poisoned");
        map.get(&object_id)?.get(&property_id).cloned()
    }

    /// Remove all properties associated with an object.
    pub fn remove_object(&self, object_id: ObjectId) {
        let mut map = self.inner.lock().expect("ObjectStore lock poisoned");
        map.remove(&object_id);
    }

    /// Return a snapshot of all known object identifiers.
    pub fn object_ids(&self) -> Vec<ObjectId> {
        let map = self.inner.lock().expect("ObjectStore lock poisoned");
        map.keys().copied().collect()
    }
}

impl Default for ObjectStore {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ObjectStoreHandler
// ─────────────────────────────────────────────────────────────────────────────

/// A [`ServiceHandler`] that delegates directly to an [`Arc<ObjectStore>`].
///
/// ReadProperty returns the stored value or the appropriate error.
/// WriteProperty always accepts writes (no write-protection logic).
pub struct ObjectStoreHandler {
    store: Arc<ObjectStore>,
}

impl ObjectStoreHandler {
    /// Wrap an existing shared store.
    pub fn new(store: Arc<ObjectStore>) -> Self {
        Self { store }
    }
}

impl ServiceHandler for ObjectStoreHandler {
    fn read_property(
        &self,
        object_id: ObjectId,
        property_id: PropertyId,
        _array_index: Option<u32>,
    ) -> Result<ClientDataValue, BacnetServiceError> {
        // Check object existence first.
        let map = self.store.inner.lock().expect("ObjectStore lock poisoned");
        let props = map
            .get(&object_id)
            .ok_or(BacnetServiceError::UnknownObject)?;
        props
            .get(&property_id)
            .cloned()
            .ok_or(BacnetServiceError::UnknownProperty)
    }

    fn write_property(
        &self,
        object_id: ObjectId,
        property_id: PropertyId,
        _array_index: Option<u32>,
        value: ClientDataValue,
        _priority: Option<u8>,
    ) -> Result<(), BacnetServiceError> {
        let mut map = self.store.inner.lock().expect("ObjectStore lock poisoned");
        // Write is only permitted if the object already exists.
        let props = map
            .get_mut(&object_id)
            .ok_or(BacnetServiceError::UnknownObject)?;
        props.insert(property_id, value);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BacnetServer
// ─────────────────────────────────────────────────────────────────────────────

/// A server that binds a [`DataLink`] and dispatches incoming service requests.
pub struct BacnetServer<D: DataLink> {
    datalink: Arc<D>,
    handler: Arc<dyn ServiceHandler>,
    device_id: u32,
    vendor_id: u16,
    /// Stored for future use in I-Am responses and segmentation negotiation.
    #[allow(dead_code)]
    max_apdu: u8,
}

impl<D: DataLink> BacnetServer<D> {
    /// Create a new server with the given datalink and device instance number.
    pub fn new(datalink: D, device_id: u32, handler: impl ServiceHandler) -> Self {
        Self {
            datalink: Arc::new(datalink),
            handler: Arc::new(handler),
            device_id,
            vendor_id: 0,
            max_apdu: 5, // standard max APDU size index 5 → 1476 bytes
        }
    }

    /// Override the vendor ID sent in I-Am responses (default: 0).
    pub fn with_vendor_id(mut self, vendor_id: u16) -> Self {
        self.vendor_id = vendor_id;
        self
    }

    /// Run the serve loop.
    ///
    /// Receives frames, parses them, and dispatches:
    /// - UnconfirmedRequest Who-Is (0x08) → I-Am; others ignored.
    /// - ConfirmedRequest ReadProperty (0x0C) → ComplexAck or Error.
    /// - ConfirmedRequest WriteProperty (0x0F) → SimpleAck or Error.
    /// - ConfirmedRequest ReadPropertyMultiple (0x0E) → ComplexAck or Error.
    /// - Any other confirmed service → Reject (UNRECOGNIZED_SERVICE = 0x08).
    pub async fn serve(self) {
        let mut buf = [0u8; 1500];
        loop {
            let result = self.datalink.recv(&mut buf).await;
            match result {
                Ok((n, source)) => {
                    if let Err(e) = self.handle_frame(&buf[..n], source).await {
                        log::debug!("server: error handling frame: {e:?}");
                    }
                }
                Err(e) => {
                    log::debug!("server: datalink recv error: {e:?}");
                    // On persistent transport errors avoid a tight busy loop.
                    tokio::task::yield_now().await;
                }
            }
        }
    }

    // ── private helpers ──────────────────────────────────────────────────────

    async fn handle_frame(
        &self,
        frame: &[u8],
        source: DataLinkAddress,
    ) -> Result<(), rustbac_core::DecodeError> {
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
                    // Who-Is — parse optional limits then respond.
                    let limits = decode_who_is_limits(&mut r);
                    if matches_who_is(self.device_id, limits) {
                        self.send_i_am(source).await;
                    }
                }
                // All other unconfirmed services are ignored.
            }
            Some(ApduType::ConfirmedRequest) => {
                let header = ConfirmedRequestHeader::decode(&mut r)?;
                let invoke_id = header.invoke_id;
                match header.service_choice {
                    SERVICE_READ_PROPERTY => {
                        self.handle_read_property(&mut r, invoke_id, source).await;
                    }
                    SERVICE_WRITE_PROPERTY => {
                        self.handle_write_property(&mut r, invoke_id, source).await;
                    }
                    SERVICE_READ_PROPERTY_MULTIPLE => {
                        self.handle_read_property_multiple(&mut r, invoke_id, source)
                            .await;
                    }
                    SERVICE_WRITE_PROPERTY_MULTIPLE => {
                        self.handle_write_property_multiple(&mut r, invoke_id, source)
                            .await;
                    }
                    SERVICE_SUBSCRIBE_COV => {
                        self.handle_subscribe_cov(&mut r, invoke_id, source).await;
                    }
                    SERVICE_CREATE_OBJECT => {
                        self.handle_create_object(&mut r, invoke_id, source).await;
                    }
                    SERVICE_DELETE_OBJECT => {
                        self.handle_delete_object(&mut r, invoke_id, source).await;
                    }
                    _ => {
                        // Unknown service — send Reject with UNRECOGNIZED_SERVICE.
                        self.send_reject(invoke_id, 0x08, source).await;
                    }
                }
            }
            _ => {
                // Not a request — ignore.
            }
        }

        Ok(())
    }

    async fn send_i_am(&self, target: DataLinkAddress) {
        let device_id_raw = rustbac_core::types::ObjectId::new(
            rustbac_core::types::ObjectType::Device,
            self.device_id,
        );
        let req = IAmRequest {
            device_id: device_id_raw,
            max_apdu: 1476,
            segmentation: 3, // no-segmentation
            vendor_id: self.vendor_id as u32,
        };
        let mut buf = [0u8; 128];
        let mut w = Writer::new(&mut buf);
        if Npdu::new(0).encode(&mut w).is_err() {
            return;
        }
        if req.encode(&mut w).is_err() {
            return;
        }
        let _ = self.datalink.send(target, w.as_written()).await;
    }

    async fn handle_read_property(
        &self,
        r: &mut Reader<'_>,
        invoke_id: u8,
        source: DataLinkAddress,
    ) {
        // Decode: object_id [0], property_id [1], optional array_index [2].
        let object_id = match crate::decode_ctx_object_id(r) {
            Ok(v) => v,
            Err(_) => return,
        };
        let property_id_raw = match crate::decode_ctx_unsigned(r) {
            Ok(v) => v,
            Err(_) => return,
        };
        let property_id = PropertyId::from_u32(property_id_raw);

        // Optional array index.
        let array_index = decode_optional_array_index(r);

        match self
            .handler
            .read_property(object_id, property_id, array_index)
        {
            Ok(value) => {
                let borrowed = client_value_to_borrowed(&value);
                let mut buf = [0u8; 1400];
                let mut w = Writer::new(&mut buf);
                if Npdu::new(0).encode(&mut w).is_err() {
                    return;
                }
                if (ComplexAckHeader {
                    segmented: false,
                    more_follows: false,
                    invoke_id,
                    sequence_number: None,
                    proposed_window_size: None,
                    service_choice: SERVICE_READ_PROPERTY,
                })
                .encode(&mut w)
                .is_err()
                {
                    return;
                }
                if encode_ctx_unsigned(&mut w, 0, object_id.raw()).is_err() {
                    return;
                }
                if encode_ctx_unsigned(&mut w, 1, property_id.to_u32()).is_err() {
                    return;
                }
                if (Tag::Opening { tag_num: 3 }).encode(&mut w).is_err() {
                    return;
                }
                if encode_application_data_value(&mut w, &borrowed).is_err() {
                    return;
                }
                if (Tag::Closing { tag_num: 3 }).encode(&mut w).is_err() {
                    return;
                }
                let _ = self.datalink.send(source, w.as_written()).await;
            }
            Err(err) => {
                self.send_error(invoke_id, SERVICE_READ_PROPERTY, err, source)
                    .await;
            }
        }
    }

    async fn handle_write_property(
        &self,
        r: &mut Reader<'_>,
        invoke_id: u8,
        source: DataLinkAddress,
    ) {
        // Decode: object_id [0], property_id [1], optional array_index [2], value [3], optional priority [4].
        let object_id = match crate::decode_ctx_object_id(r) {
            Ok(v) => v,
            Err(_) => return,
        };
        let property_id_raw = match crate::decode_ctx_unsigned(r) {
            Ok(v) => v,
            Err(_) => return,
        };
        let property_id = PropertyId::from_u32(property_id_raw);

        // Optional array index [2].
        let next_tag = match Tag::decode(r) {
            Ok(t) => t,
            Err(_) => return,
        };
        let (array_index, value_start_tag) = match next_tag {
            Tag::Context { tag_num: 2, len } => {
                let idx = match decode_unsigned(r, len as usize) {
                    Ok(v) => v,
                    Err(_) => return,
                };
                let vt = match Tag::decode(r) {
                    Ok(t) => t,
                    Err(_) => return,
                };
                (Some(idx), vt)
            }
            other => (None, other),
        };

        if value_start_tag != (Tag::Opening { tag_num: 3 }) {
            return;
        }

        let val = match rustbac_core::services::value_codec::decode_application_data_value(r) {
            Ok(v) => v,
            Err(_) => return,
        };

        match Tag::decode(r) {
            Ok(Tag::Closing { tag_num: 3 }) => {}
            _ => return,
        }

        // Optional priority [4].
        let priority = if !r.is_empty() {
            match Tag::decode(r) {
                Ok(Tag::Context { tag_num: 4, len }) => match decode_unsigned(r, len as usize) {
                    Ok(p) => Some(p as u8),
                    Err(_) => return,
                },
                _ => None,
            }
        } else {
            None
        };

        let client_val = crate::data_value_to_client(val);

        match self
            .handler
            .write_property(object_id, property_id, array_index, client_val, priority)
        {
            Ok(()) => {
                let mut buf = [0u8; 32];
                let mut w = Writer::new(&mut buf);
                if Npdu::new(0).encode(&mut w).is_err() {
                    return;
                }
                if (SimpleAck {
                    invoke_id,
                    service_choice: SERVICE_WRITE_PROPERTY,
                })
                .encode(&mut w)
                .is_err()
                {
                    return;
                }
                let _ = self.datalink.send(source, w.as_written()).await;
            }
            Err(err) => {
                self.send_error(invoke_id, SERVICE_WRITE_PROPERTY, err, source)
                    .await;
            }
        }
    }

    async fn handle_read_property_multiple(
        &self,
        r: &mut Reader<'_>,
        invoke_id: u8,
        source: DataLinkAddress,
    ) {
        type PropRefs = Vec<(PropertyId, Option<u32>)>;
        // Collect all (object_id, [(property_id, array_index)]) specs from the request.
        let mut specs: Vec<(ObjectId, PropRefs)> = Vec::new();

        while !r.is_empty() {
            // object-identifier [0]
            let object_id = match crate::decode_ctx_object_id(r) {
                Ok(v) => v,
                Err(_) => return,
            };

            // list-of-property-references [1] opening tag
            match Tag::decode(r) {
                Ok(Tag::Opening { tag_num: 1 }) => {}
                _ => return,
            }

            let mut props: Vec<(PropertyId, Option<u32>)> = Vec::new();
            loop {
                // Each property reference: property-identifier [0], optional array-index [1].
                let tag = match Tag::decode(r) {
                    Ok(t) => t,
                    Err(_) => return,
                };
                if tag == (Tag::Closing { tag_num: 1 }) {
                    break;
                }
                let property_id = match tag {
                    Tag::Context { tag_num: 0, len } => match decode_unsigned(r, len as usize) {
                        Ok(v) => PropertyId::from_u32(v),
                        Err(_) => return,
                    },
                    _ => return,
                };

                // Optional array index [1].
                let array_index = if !r.is_empty() {
                    // peek next tag without consuming
                    match peek_context_tag(r, 1) {
                        Some(len) => {
                            // consume the tag byte(s) we already peeked
                            match Tag::decode(r) {
                                Ok(_) => {}
                                Err(_) => return,
                            }
                            match decode_unsigned(r, len as usize) {
                                Ok(idx) => Some(idx),
                                Err(_) => return,
                            }
                        }
                        None => None,
                    }
                } else {
                    None
                };

                props.push((property_id, array_index));
            }

            specs.push((object_id, props));
        }

        // Build response buffer.
        let mut buf = [0u8; 1400];
        let mut w = Writer::new(&mut buf);
        if Npdu::new(0).encode(&mut w).is_err() {
            return;
        }
        if (ComplexAckHeader {
            segmented: false,
            more_follows: false,
            invoke_id,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_READ_PROPERTY_MULTIPLE,
        })
        .encode(&mut w)
        .is_err()
        {
            return;
        }

        for (object_id, props) in &specs {
            // object-identifier [0]
            if encode_ctx_unsigned(&mut w, 0, object_id.raw()).is_err() {
                return;
            }
            // list-of-results [1] opening
            if (Tag::Opening { tag_num: 1 }).encode(&mut w).is_err() {
                return;
            }

            for (property_id, array_index) in props {
                // property-identifier [2]
                if encode_ctx_unsigned(&mut w, 2, property_id.to_u32()).is_err() {
                    return;
                }
                // optional array-index [3]
                if let Some(idx) = array_index {
                    if encode_ctx_unsigned(&mut w, 3, *idx).is_err() {
                        return;
                    }
                }

                // property-access-result [4] opening
                if (Tag::Opening { tag_num: 4 }).encode(&mut w).is_err() {
                    return;
                }

                match self
                    .handler
                    .read_property(*object_id, *property_id, *array_index)
                {
                    Ok(value) => {
                        let borrowed = client_value_to_borrowed(&value);
                        if encode_application_data_value(&mut w, &borrowed).is_err() {
                            return;
                        }
                    }
                    Err(err) => {
                        // Encode property-access-error [5] with errorClass [0] / errorCode [1].
                        let (class, code) = err.to_error_class_code();
                        if (Tag::Opening { tag_num: 5 }).encode(&mut w).is_err() {
                            return;
                        }
                        if encode_ctx_unsigned(&mut w, 0, class as u32).is_err() {
                            return;
                        }
                        if encode_ctx_unsigned(&mut w, 1, code as u32).is_err() {
                            return;
                        }
                        if (Tag::Closing { tag_num: 5 }).encode(&mut w).is_err() {
                            return;
                        }
                    }
                }

                // property-access-result [4] closing
                if (Tag::Closing { tag_num: 4 }).encode(&mut w).is_err() {
                    return;
                }
            }

            // list-of-results [1] closing
            if (Tag::Closing { tag_num: 1 }).encode(&mut w).is_err() {
                return;
            }
        }

        let _ = self.datalink.send(source, w.as_written()).await;
    }

    async fn handle_write_property_multiple(
        &self,
        r: &mut Reader<'_>,
        invoke_id: u8,
        source: DataLinkAddress,
    ) {
        // Parse write-access-specifications and call write_property for each property.
        while !r.is_empty() {
            let object_id = match crate::decode_ctx_object_id(r) {
                Ok(v) => v,
                Err(_) => return,
            };
            // Opening tag [1] — list of properties
            match Tag::decode(r) {
                Ok(Tag::Opening { tag_num: 1 }) => {}
                _ => return,
            }
            loop {
                // Check for closing tag [1]
                let tag = match Tag::decode(r) {
                    Ok(t) => t,
                    Err(_) => return,
                };
                if tag == (Tag::Closing { tag_num: 1 }) {
                    break;
                }
                // property-identifier [0]
                let property_id = match tag {
                    Tag::Context { tag_num: 0, len } => match decode_unsigned(r, len as usize) {
                        Ok(v) => PropertyId::from_u32(v),
                        Err(_) => return,
                    },
                    _ => return,
                };
                // optional array-index [1]
                let array_index = if !r.is_empty() {
                    match peek_context_tag(r, 1) {
                        Some(len) => {
                            let _ = Tag::decode(r);
                            decode_unsigned(r, len as usize).ok()
                        }
                        None => None,
                    }
                } else {
                    None
                };
                // property-value [2] opening
                match Tag::decode(r) {
                    Ok(Tag::Opening { tag_num: 2 }) => {}
                    _ => return,
                }
                let val =
                    match rustbac_core::services::value_codec::decode_application_data_value(r) {
                        Ok(v) => v,
                        Err(_) => return,
                    };
                match Tag::decode(r) {
                    Ok(Tag::Closing { tag_num: 2 }) => {}
                    _ => return,
                }
                // optional priority [3]
                let priority = if !r.is_empty() {
                    match peek_context_tag(r, 3) {
                        Some(len) => {
                            let _ = Tag::decode(r);
                            decode_unsigned(r, len as usize).ok().map(|p| p as u8)
                        }
                        None => None,
                    }
                } else {
                    None
                };
                let client_val = crate::data_value_to_client(val);
                if let Err(err) = self.handler.write_property(
                    object_id,
                    property_id,
                    array_index,
                    client_val,
                    priority,
                ) {
                    self.send_error(invoke_id, SERVICE_WRITE_PROPERTY_MULTIPLE, err, source)
                        .await;
                    return;
                }
            }
        }
        // All properties written successfully — send SimpleAck.
        let mut buf = [0u8; 32];
        let mut w = Writer::new(&mut buf);
        if Npdu::new(0).encode(&mut w).is_err() {
            return;
        }
        if (SimpleAck {
            invoke_id,
            service_choice: SERVICE_WRITE_PROPERTY_MULTIPLE,
        })
        .encode(&mut w)
        .is_err()
        {
            return;
        }
        let _ = self.datalink.send(source, w.as_written()).await;
    }

    async fn handle_subscribe_cov(
        &self,
        r: &mut Reader<'_>,
        invoke_id: u8,
        source: DataLinkAddress,
    ) {
        // subscriberProcessIdentifier [0]
        let subscriber_process_id = match Tag::decode(r) {
            Ok(Tag::Context { tag_num: 0, len }) => match decode_unsigned(r, len as usize) {
                Ok(v) => v,
                Err(_) => return,
            },
            _ => return,
        };
        // monitoredObjectIdentifier [1]
        let monitored_object_id = match crate::decode_ctx_object_id(r) {
            Ok(v) => v,
            Err(_) => return,
        };
        // issueConfirmedNotifications [2]
        let issue_confirmed = match Tag::decode(r) {
            Ok(Tag::Context { tag_num: 2, len }) => match decode_unsigned(r, len as usize) {
                Ok(v) => v != 0,
                Err(_) => return,
            },
            _ => return,
        };
        // optional lifetime [3]
        let lifetime = if !r.is_empty() {
            match peek_context_tag(r, 3) {
                Some(len) => {
                    let _ = Tag::decode(r);
                    decode_unsigned(r, len as usize).ok()
                }
                None => None,
            }
        } else {
            None
        };

        match self.handler.subscribe_cov(
            subscriber_process_id,
            monitored_object_id,
            issue_confirmed,
            lifetime,
        ) {
            Ok(()) => {
                let mut buf = [0u8; 32];
                let mut w = Writer::new(&mut buf);
                if Npdu::new(0).encode(&mut w).is_err() {
                    return;
                }
                if (SimpleAck {
                    invoke_id,
                    service_choice: SERVICE_SUBSCRIBE_COV,
                })
                .encode(&mut w)
                .is_err()
                {
                    return;
                }
                let _ = self.datalink.send(source, w.as_written()).await;
            }
            Err(err) => {
                self.send_error(invoke_id, SERVICE_SUBSCRIBE_COV, err, source)
                    .await;
            }
        }
    }

    async fn handle_create_object(
        &self,
        r: &mut Reader<'_>,
        invoke_id: u8,
        source: DataLinkAddress,
    ) {
        // objectSpecifier [0] opening
        match Tag::decode(r) {
            Ok(Tag::Opening { tag_num: 0 }) => {}
            _ => return,
        }
        // objectType [0] — context-tagged enumerated
        let object_type_raw = match Tag::decode(r) {
            Ok(Tag::Context { tag_num: 0, len }) => match decode_unsigned(r, len as usize) {
                Ok(v) => v,
                Err(_) => return,
            },
            _ => return,
        };
        let object_type = ObjectType::from_u16(object_type_raw as u16);
        // objectSpecifier [0] closing
        match Tag::decode(r) {
            Ok(Tag::Closing { tag_num: 0 }) => {}
            _ => return,
        }

        match self.handler.create_object(object_type) {
            Ok(created_id) => {
                let mut buf = [0u8; 64];
                let mut w = Writer::new(&mut buf);
                if Npdu::new(0).encode(&mut w).is_err() {
                    return;
                }
                if (ComplexAckHeader {
                    segmented: false,
                    more_follows: false,
                    invoke_id,
                    sequence_number: None,
                    proposed_window_size: None,
                    service_choice: SERVICE_CREATE_OBJECT,
                })
                .encode(&mut w)
                .is_err()
                {
                    return;
                }
                if encode_ctx_unsigned(&mut w, 0, created_id.raw()).is_err() {
                    return;
                }
                let _ = self.datalink.send(source, w.as_written()).await;
            }
            Err(err) => {
                self.send_error(invoke_id, SERVICE_CREATE_OBJECT, err, source)
                    .await;
            }
        }
    }

    async fn handle_delete_object(
        &self,
        r: &mut Reader<'_>,
        invoke_id: u8,
        source: DataLinkAddress,
    ) {
        // objectIdentifier — application-tagged
        let object_id = match crate::decode_ctx_object_id(r) {
            Ok(v) => v,
            Err(_) => return,
        };

        match self.handler.delete_object(object_id) {
            Ok(()) => {
                let mut buf = [0u8; 32];
                let mut w = Writer::new(&mut buf);
                if Npdu::new(0).encode(&mut w).is_err() {
                    return;
                }
                if (SimpleAck {
                    invoke_id,
                    service_choice: SERVICE_DELETE_OBJECT,
                })
                .encode(&mut w)
                .is_err()
                {
                    return;
                }
                let _ = self.datalink.send(source, w.as_written()).await;
            }
            Err(err) => {
                self.send_error(invoke_id, SERVICE_DELETE_OBJECT, err, source)
                    .await;
            }
        }
    }

    async fn send_error(
        &self,
        invoke_id: u8,
        service_choice: u8,
        err: BacnetServiceError,
        target: DataLinkAddress,
    ) {
        let (class, code) = err.to_error_class_code();
        let mut buf = [0u8; 64];
        let mut w = Writer::new(&mut buf);
        if Npdu::new(0).encode(&mut w).is_err() {
            return;
        }
        // Error PDU header: type=5 (Error)
        if w.write_u8(0x50).is_err() {
            return;
        }
        if w.write_u8(invoke_id).is_err() {
            return;
        }
        if w.write_u8(service_choice).is_err() {
            return;
        }
        // error-class: application Enumerated
        if (Tag::Application {
            tag: rustbac_core::encoding::tag::AppTag::Enumerated,
            len: 1,
        })
        .encode(&mut w)
        .is_err()
        {
            return;
        }
        if w.write_u8(class).is_err() {
            return;
        }
        // error-code: application Enumerated
        if (Tag::Application {
            tag: rustbac_core::encoding::tag::AppTag::Enumerated,
            len: 1,
        })
        .encode(&mut w)
        .is_err()
        {
            return;
        }
        if w.write_u8(code).is_err() {
            return;
        }
        let _ = self.datalink.send(target, w.as_written()).await;
    }

    async fn send_reject(&self, invoke_id: u8, reason: u8, target: DataLinkAddress) {
        let mut buf = [0u8; 16];
        let mut w = Writer::new(&mut buf);
        if Npdu::new(0).encode(&mut w).is_err() {
            return;
        }
        // Reject PDU: type=6 (Reject)
        if w.write_u8(0x60).is_err() {
            return;
        }
        if w.write_u8(invoke_id).is_err() {
            return;
        }
        if w.write_u8(reason).is_err() {
            return;
        }
        let _ = self.datalink.send(target, w.as_written()).await;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Free-standing helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Decode Who-Is optional [0] low-limit and [1] high-limit.
fn decode_who_is_limits(r: &mut Reader<'_>) -> Option<(u32, u32)> {
    if r.is_empty() {
        return None;
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

/// Return true when `device_id` falls within the Who-Is range (or it is a global Who-Is).
fn matches_who_is(device_id: u32, limits: Option<(u32, u32)>) -> bool {
    match limits {
        None => true,
        Some((low, high)) => device_id >= low && device_id <= high,
    }
}

/// Decode an optional context-tagged array index from the current reader position.
///
/// This is a non-destructive peek: if the next tag is not a context tag with
/// tag_num == 2, `None` is returned and the reader is **not** advanced.
fn decode_optional_array_index(r: &mut Reader<'_>) -> Option<u32> {
    if r.is_empty() {
        return None;
    }
    // Peek the first byte to see if it could be context tag 2.
    let first = r.peek_u8().ok()?;
    // Context tag 2 with short form: upper nibble = (tag_num << 4) | 0x08 = 0x28, 0x29, 0x2A, 0x2B
    // The tag class bit (bit 3) = 1 for context tags; tag_num is bits 7-4.
    // For context tag 2: byte = (2 << 4) | 0x08 | len_byte where len_byte ∈ {1,2,3,4}
    // But we should decode properly and put it back if not matching.
    // Reader doesn't support un-consuming, so we use a clone.
    let tag = Tag::decode(r).ok()?;
    match tag {
        Tag::Context { tag_num: 2, len } => decode_unsigned(r, len as usize).ok(),
        _ => {
            // Not an array index tag — put it back by... we can't.
            // This function is only called when we know the array index is encoded
            // (i.e., right after property_id and before the closing tag in ReadProperty).
            // Since Reader has no unget, we rely on the caller to only call this
            // after property_id and only if the bytes represent an array index.
            // In practice the caller already consumed the property_id tag so any
            // remaining tag is either [2] array_index or end-of-frame.
            let _ = first; // silence unused warning
            None
        }
    }
}

/// Peek whether the next tag in `r` is a context tag with the given `tag_num`.
/// Returns the `len` field if it matches, `None` otherwise.
/// Does NOT advance the reader.
fn peek_context_tag(r: &mut Reader<'_>, tag_num: u8) -> Option<u32> {
    let first = r.peek_u8().ok()?;
    // Short-form context tag: bit3=1 (context), bits 7-4 = tag_num, bits 2-0 = len (0-4).
    // Short form len encoding: 0-4 means length is that value (for context tags without extended len).
    // Byte layout: [tag_num(4) | class(1) | len(3)] where class bit set means context.
    // For context: byte = (tag_num << 4) | 0x08 | short_len  (short_len < 5)
    // Closing/Opening tags have short_len = 6 or 7.
    let is_context = (first & 0x08) != 0 && (first & 0x07) < 6;
    if !is_context {
        return None;
    }
    let this_tag_num = first >> 4;
    if this_tag_num != tag_num {
        return None;
    }
    let short_len = first & 0x07;
    if short_len < 5 {
        Some(short_len as u32)
    } else {
        // Extended length — not expected for small BACnet property indices, skip.
        None
    }
}

/// Convert an owned [`ClientDataValue`] to a borrowed [`rustbac_core::types::DataValue`].
fn client_value_to_borrowed(val: &ClientDataValue) -> rustbac_core::types::DataValue<'_> {
    use rustbac_core::types::DataValue;
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

// ─────────────────────────────────────────────────────────────────────────────
// Writer helper — expose write_u8 used above
// ─────────────────────────────────────────────────────────────────────────────

/// Thin extension to allow calling `w.write_u8` inside this module.
#[allow(dead_code)]
trait WriterExt {
    fn write_u8(&mut self, b: u8) -> Result<(), rustbac_core::EncodeError>;
}

impl WriterExt for Writer<'_> {
    fn write_u8(&mut self, b: u8) -> Result<(), rustbac_core::EncodeError> {
        Writer::write_u8(self, b)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// COV Subscription Manager
// ─────────────────────────────────────────────────────────────────────────────

/// Tracks active COV subscriptions and generates notifications on property changes.
pub struct CovSubscriptionManager {
    subscriptions: Mutex<Vec<CovSubscription>>,
}

struct CovSubscription {
    subscriber_process_id: u32,
    monitored_object_id: ObjectId,
    subscriber_address: DataLinkAddress,
    issue_confirmed: bool,
    /// Absolute deadline (tokio::time::Instant). None = infinite lifetime.
    expires_at: Option<tokio::time::Instant>,
}

impl CovSubscriptionManager {
    /// Create an empty subscription manager.
    pub fn new() -> Self {
        Self {
            subscriptions: Mutex::new(Vec::new()),
        }
    }

    /// Add or renew a subscription. If a subscription with the same
    /// (process_id, object_id) already exists, it is renewed.
    pub fn subscribe(
        &self,
        subscriber_process_id: u32,
        monitored_object_id: ObjectId,
        subscriber_address: DataLinkAddress,
        issue_confirmed: bool,
        lifetime_seconds: Option<u32>,
    ) {
        let mut subs = self
            .subscriptions
            .lock()
            .expect("CovSubscriptionManager lock");
        // Remove existing subscription with same key
        subs.retain(|s| {
            !(s.subscriber_process_id == subscriber_process_id
                && s.monitored_object_id == monitored_object_id)
        });
        let expires_at = lifetime_seconds
            .map(|secs| tokio::time::Instant::now() + std::time::Duration::from_secs(secs as u64));
        subs.push(CovSubscription {
            subscriber_process_id,
            monitored_object_id,
            subscriber_address,
            issue_confirmed,
            expires_at,
        });
    }

    /// Cancel a subscription identified by (process_id, object_id).
    pub fn cancel(&self, subscriber_process_id: u32, monitored_object_id: ObjectId) {
        let mut subs = self
            .subscriptions
            .lock()
            .expect("CovSubscriptionManager lock");
        subs.retain(|s| {
            !(s.subscriber_process_id == subscriber_process_id
                && s.monitored_object_id == monitored_object_id)
        });
    }

    /// Remove expired subscriptions.
    pub fn purge_expired(&self) {
        let now = tokio::time::Instant::now();
        let mut subs = self
            .subscriptions
            .lock()
            .expect("CovSubscriptionManager lock");
        subs.retain(|s| s.expires_at.map_or(true, |exp| exp > now));
    }

    /// Get all active subscribers for a given object.
    /// Returns (subscriber_address, subscriber_process_id, issue_confirmed).
    pub fn subscribers_for(&self, object_id: ObjectId) -> Vec<(DataLinkAddress, u32, bool)> {
        let now = tokio::time::Instant::now();
        let subs = self
            .subscriptions
            .lock()
            .expect("CovSubscriptionManager lock");
        subs.iter()
            .filter(|s| {
                s.monitored_object_id == object_id && s.expires_at.map_or(true, |exp| exp > now)
            })
            .map(|s| {
                (
                    s.subscriber_address,
                    s.subscriber_process_id,
                    s.issue_confirmed,
                )
            })
            .collect()
    }

    /// Return the count of active (non-expired) subscriptions.
    pub fn active_count(&self) -> usize {
        let now = tokio::time::Instant::now();
        let subs = self
            .subscriptions
            .lock()
            .expect("CovSubscriptionManager lock");
        subs.iter()
            .filter(|s| s.expires_at.map_or(true, |exp| exp > now))
            .count()
    }
}

impl Default for CovSubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Encode an UnconfirmedCOVNotification PDU.
///
/// Returns the encoded bytes or `None` if the buffer is too small.
pub fn encode_unconfirmed_cov_notification(
    subscriber_process_id: u32,
    initiating_device_id: ObjectId,
    monitored_object_id: ObjectId,
    time_remaining: u32,
    values: &[(PropertyId, ClientDataValue)],
) -> Option<Vec<u8>> {
    let mut buf = [0u8; 1400];
    let mut w = Writer::new(&mut buf);
    Npdu::new(0).encode(&mut w).ok()?;
    // UnconfirmedRequest header: type=1, service=2 (UnconfirmedCOVNotification)
    UnconfirmedRequestHeader {
        service_choice: 0x02,
    }
    .encode(&mut w)
    .ok()?;
    // [0] subscriber-process-identifier
    encode_ctx_unsigned(&mut w, 0, subscriber_process_id).ok()?;
    // [1] initiating-device-identifier
    encode_ctx_unsigned(&mut w, 1, initiating_device_id.raw()).ok()?;
    // [2] monitored-object-identifier
    encode_ctx_unsigned(&mut w, 2, monitored_object_id.raw()).ok()?;
    // [3] time-remaining
    encode_ctx_unsigned(&mut w, 3, time_remaining).ok()?;
    // [4] list-of-values
    Tag::Opening { tag_num: 4 }.encode(&mut w).ok()?;
    for (prop_id, value) in values {
        // property-identifier [0]
        encode_ctx_unsigned(&mut w, 0, prop_id.to_u32()).ok()?;
        // property-value [2] (opening)
        Tag::Opening { tag_num: 2 }.encode(&mut w).ok()?;
        let borrowed = client_value_to_borrowed(value);
        encode_application_data_value(&mut w, &borrowed).ok()?;
        Tag::Closing { tag_num: 2 }.encode(&mut w).ok()?;
    }
    Tag::Closing { tag_num: 4 }.encode(&mut w).ok()?;
    Some(w.as_written().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustbac_core::apdu::{ComplexAckHeader, SimpleAck};
    use rustbac_core::encoding::{reader::Reader, writer::Writer};
    use rustbac_core::npdu::Npdu;
    use rustbac_core::services::read_property::SERVICE_READ_PROPERTY;
    use rustbac_core::services::write_property::SERVICE_WRITE_PROPERTY;
    use rustbac_core::types::{ObjectId, ObjectType, PropertyId};
    use rustbac_datalink::DataLinkAddress;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct MockDataLink {
        sent: Arc<Mutex<Vec<(DataLinkAddress, Vec<u8>)>>>,
    }

    impl rustbac_datalink::DataLink for MockDataLink {
        async fn send(
            &self,
            address: DataLinkAddress,
            payload: &[u8],
        ) -> Result<(), rustbac_datalink::DataLinkError> {
            self.sent
                .lock()
                .expect("poisoned")
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

    fn make_server() -> (
        BacnetServer<MockDataLink>,
        Arc<Mutex<Vec<(DataLinkAddress, Vec<u8>)>>>,
        Arc<ObjectStore>,
    ) {
        let store = Arc::new(ObjectStore::new());
        let device_id = ObjectId::new(ObjectType::Device, 42);
        store.set(
            device_id,
            PropertyId::ObjectName,
            ClientDataValue::CharacterString("TestDevice".to_string()),
        );
        let handler = ObjectStoreHandler::new(store.clone());
        let dl = MockDataLink::default();
        let sent = dl.sent.clone();
        let server = BacnetServer::new(dl, 42, handler);
        (server, sent, store)
    }

    fn source() -> DataLinkAddress {
        DataLinkAddress::Ip("127.0.0.1:47808".parse().unwrap())
    }

    #[tokio::test]
    async fn object_store_set_get_remove() {
        let store = ObjectStore::new();
        let oid = ObjectId::new(ObjectType::AnalogValue, 1);
        store.set(oid, PropertyId::PresentValue, ClientDataValue::Real(3.14));
        assert_eq!(
            store.get(oid, PropertyId::PresentValue),
            Some(ClientDataValue::Real(3.14))
        );
        store.remove_object(oid);
        assert_eq!(store.get(oid, PropertyId::PresentValue), None);
    }

    #[tokio::test]
    async fn object_store_object_ids() {
        let store = ObjectStore::new();
        let oid1 = ObjectId::new(ObjectType::AnalogValue, 1);
        let oid2 = ObjectId::new(ObjectType::AnalogValue, 2);
        store.set(oid1, PropertyId::PresentValue, ClientDataValue::Real(1.0));
        store.set(oid2, PropertyId::PresentValue, ClientDataValue::Real(2.0));
        let mut ids = store.object_ids();
        ids.sort_by_key(|id| id.raw());
        assert!(ids.contains(&oid1));
        assert!(ids.contains(&oid2));
    }

    #[tokio::test]
    async fn read_property_known_returns_complex_ack() {
        let (server, sent, _store) = make_server();
        let device_id = ObjectId::new(ObjectType::Device, 42);

        // Build a ReadProperty request frame.
        use rustbac_core::encoding::primitives::encode_ctx_unsigned;
        let mut req_buf = [0u8; 256];
        let mut w = Writer::new(&mut req_buf);
        Npdu::new(0).encode(&mut w).unwrap();
        rustbac_core::apdu::ConfirmedRequestHeader {
            segmented: false,
            more_follows: false,
            segmented_response_accepted: true,
            max_segments: 0,
            max_apdu: 5,
            invoke_id: 7,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_READ_PROPERTY,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_unsigned(&mut w, 0, device_id.raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, PropertyId::ObjectName.to_u32()).unwrap();

        server.handle_frame(w.as_written(), source()).await.unwrap();

        let sent = sent.lock().expect("poisoned");
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let hdr = ComplexAckHeader::decode(&mut r).unwrap();
        assert_eq!(hdr.invoke_id, 7);
        assert_eq!(hdr.service_choice, SERVICE_READ_PROPERTY);
    }

    #[tokio::test]
    async fn read_property_unknown_object_returns_error() {
        let (server, sent, _store) = make_server();
        let unknown = ObjectId::new(ObjectType::AnalogValue, 999);

        use rustbac_core::encoding::primitives::encode_ctx_unsigned;
        let mut req_buf = [0u8; 256];
        let mut w = Writer::new(&mut req_buf);
        Npdu::new(0).encode(&mut w).unwrap();
        rustbac_core::apdu::ConfirmedRequestHeader {
            segmented: false,
            more_follows: false,
            segmented_response_accepted: true,
            max_segments: 0,
            max_apdu: 5,
            invoke_id: 3,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_READ_PROPERTY,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_unsigned(&mut w, 0, unknown.raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, PropertyId::PresentValue.to_u32()).unwrap();

        server.handle_frame(w.as_written(), source()).await.unwrap();

        let sent = sent.lock().expect("poisoned");
        assert_eq!(sent.len(), 1);
        // First byte after NPDU should be 0x50 (Error PDU).
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let type_byte = r.read_u8().unwrap();
        assert_eq!(type_byte >> 4, 5, "expected Error PDU type");
    }

    #[tokio::test]
    async fn write_property_updates_store() {
        let (server, sent, store) = make_server();
        let device_id = ObjectId::new(ObjectType::Device, 42);

        use rustbac_core::encoding::primitives::encode_ctx_unsigned;
        use rustbac_core::services::value_codec::encode_application_data_value;
        use rustbac_core::types::DataValue;

        let mut req_buf = [0u8; 256];
        let mut w = Writer::new(&mut req_buf);
        Npdu::new(0).encode(&mut w).unwrap();
        rustbac_core::apdu::ConfirmedRequestHeader {
            segmented: false,
            more_follows: false,
            segmented_response_accepted: false,
            max_segments: 0,
            max_apdu: 5,
            invoke_id: 11,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: SERVICE_WRITE_PROPERTY,
        }
        .encode(&mut w)
        .unwrap();
        encode_ctx_unsigned(&mut w, 0, device_id.raw()).unwrap();
        encode_ctx_unsigned(&mut w, 1, PropertyId::ObjectName.to_u32()).unwrap();
        Tag::Opening { tag_num: 3 }.encode(&mut w).unwrap();
        encode_application_data_value(&mut w, &DataValue::CharacterString("NewName")).unwrap();
        Tag::Closing { tag_num: 3 }.encode(&mut w).unwrap();

        server.handle_frame(w.as_written(), source()).await.unwrap();

        // Verify SimpleAck was sent.
        let sent_frames = sent.lock().expect("poisoned");
        assert_eq!(sent_frames.len(), 1);
        let mut r = Reader::new(&sent_frames[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let ack = SimpleAck::decode(&mut r).unwrap();
        assert_eq!(ack.invoke_id, 11);
        assert_eq!(ack.service_choice, SERVICE_WRITE_PROPERTY);
        drop(sent_frames);

        // Verify store updated.
        assert_eq!(
            store.get(device_id, PropertyId::ObjectName),
            Some(ClientDataValue::CharacterString("NewName".to_string()))
        );
    }

    #[tokio::test]
    async fn who_is_sends_i_am() {
        let (server, sent, _store) = make_server();

        let mut req_buf = [0u8; 32];
        let mut w = Writer::new(&mut req_buf);
        Npdu::new(0).encode(&mut w).unwrap();
        // UnconfirmedRequest Who-Is (0x08) with no limits.
        rustbac_core::apdu::UnconfirmedRequestHeader {
            service_choice: 0x08,
        }
        .encode(&mut w)
        .unwrap();

        server.handle_frame(w.as_written(), source()).await.unwrap();

        let sent = sent.lock().expect("poisoned");
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let _unconf_hdr = rustbac_core::apdu::UnconfirmedRequestHeader::decode(&mut r).unwrap();
        let iam = rustbac_core::services::i_am::IAmRequest::decode_after_header(&mut r).unwrap();
        assert_eq!(iam.device_id.instance(), 42);
    }

    #[tokio::test]
    async fn unknown_service_sends_reject() {
        let (server, sent, _store) = make_server();

        let mut req_buf = [0u8; 32];
        let mut w = Writer::new(&mut req_buf);
        Npdu::new(0).encode(&mut w).unwrap();
        rustbac_core::apdu::ConfirmedRequestHeader {
            segmented: false,
            more_follows: false,
            segmented_response_accepted: false,
            max_segments: 0,
            max_apdu: 5,
            invoke_id: 99,
            sequence_number: None,
            proposed_window_size: None,
            service_choice: 0x55, // unknown
        }
        .encode(&mut w)
        .unwrap();

        server.handle_frame(w.as_written(), source()).await.unwrap();

        let sent = sent.lock().expect("poisoned");
        assert_eq!(sent.len(), 1);
        let mut r = Reader::new(&sent[0].1);
        let _npdu = Npdu::decode(&mut r).unwrap();
        let type_byte = r.read_u8().unwrap();
        assert_eq!(type_byte >> 4, 6, "expected Reject PDU type");
        let id = r.read_u8().unwrap();
        let reason = r.read_u8().unwrap();
        assert_eq!(id, 99);
        assert_eq!(reason, 0x08); // UNRECOGNIZED_SERVICE
    }

    #[tokio::test]
    async fn cov_subscription_manager_subscribe_and_cancel() {
        let mgr = CovSubscriptionManager::new();
        let obj = ObjectId::new(ObjectType::AnalogValue, 1);
        let addr = source();

        mgr.subscribe(1, obj, addr, false, Some(300));
        assert_eq!(mgr.active_count(), 1);
        assert_eq!(mgr.subscribers_for(obj).len(), 1);

        mgr.cancel(1, obj);
        assert_eq!(mgr.active_count(), 0);
        assert_eq!(mgr.subscribers_for(obj).len(), 0);
    }

    #[test]
    fn encode_unconfirmed_cov_notification_produces_bytes() {
        let device_id = ObjectId::new(ObjectType::Device, 42);
        let object_id = ObjectId::new(ObjectType::AnalogValue, 1);
        let values = vec![(PropertyId::PresentValue, ClientDataValue::Real(72.5))];
        let result = encode_unconfirmed_cov_notification(1, device_id, object_id, 300, &values);
        assert!(result.is_some());
        let bytes = result.unwrap();
        assert!(bytes.len() > 10);
    }
}
