use rustbac_core::types::ObjectId;
use rustbac_datalink::DataLinkAddress;

/// A BACnet device discovered via a Who-Is / I-Am exchange.
///
/// `device_id` is `None` if the I-Am reply could not be decoded to extract the object
/// identifier (this should not happen with a standards-compliant device).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiscoveredDevice {
    /// Transport address from which the I-Am reply was received.
    pub address: DataLinkAddress,
    /// The device's object identifier (type = Device, instance = device instance number).
    pub device_id: Option<ObjectId>,
}

/// A BACnet object discovered via a Who-Has / I-Have exchange.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiscoveredObject {
    /// Transport address from which the I-Have reply was received.
    pub address: DataLinkAddress,
    /// Object identifier of the device that owns this object.
    pub device_id: ObjectId,
    /// Object identifier of the discovered object.
    pub object_id: ObjectId,
    /// Human-readable name of the discovered object as reported by the device.
    pub object_name: String,
}
