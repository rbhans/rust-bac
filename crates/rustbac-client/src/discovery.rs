use rustbac_core::types::ObjectId;
use rustbac_datalink::DataLinkAddress;

#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub address: DataLinkAddress,
    pub device_id: Option<ObjectId>,
}

#[derive(Debug, Clone)]
pub struct DiscoveredObject {
    pub address: DataLinkAddress,
    pub device_id: ObjectId,
    pub object_id: ObjectId,
    pub object_name: String,
}
