//! Device discovery walk — reads the object list and common properties for
//! every object on a BACnet device.

use crate::{BacnetClient, ClientDataValue, ClientError};
use rustbac_core::types::{ObjectId, ObjectType, PropertyId};
use rustbac_datalink::{DataLink, DataLinkAddress};

/// Summary of a single object on a device.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ObjectSummary {
    pub object_id: ObjectId,
    pub object_name: Option<String>,
    pub object_type: ObjectType,
    pub present_value: Option<ClientDataValue>,
    pub description: Option<String>,
    pub units: Option<u32>,
    pub status_flags: Option<ClientDataValue>,
}

/// Metadata read from the Device object during a walk.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeviceInfo {
    pub vendor_name: Option<String>,
    pub model_name: Option<String>,
    pub firmware_revision: Option<String>,
}

/// Result of a full device walk.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeviceWalkResult {
    pub device_id: ObjectId,
    pub device_info: DeviceInfo,
    pub objects: Vec<ObjectSummary>,
}

/// Walk a BACnet device: read its object list, then batch-read common
/// properties for each object.
pub async fn walk_device<D: DataLink>(
    client: &BacnetClient<D>,
    addr: DataLinkAddress,
    device_id: ObjectId,
) -> Result<DeviceWalkResult, ClientError> {
    // 1. Read the object list.
    let object_list_value = client
        .read_property(addr, device_id, PropertyId::ObjectList)
        .await?;

    let object_ids = extract_object_ids(&object_list_value);

    // 2. For each object, read common properties via ReadPropertyMultiple.
    let properties = &[
        PropertyId::ObjectName,
        PropertyId::ObjectType,
        PropertyId::PresentValue,
        PropertyId::Description,
        PropertyId::Units,
        PropertyId::StatusFlags,
    ];

    let mut objects = Vec::with_capacity(object_ids.len());
    for &oid in &object_ids {
        let props = client.read_property_multiple(addr, oid, properties).await;

        let summary = match props {
            Ok(prop_values) => build_summary(oid, &prop_values),
            Err(_) => ObjectSummary {
                object_id: oid,
                object_name: None,
                object_type: oid.object_type(),
                present_value: None,
                description: None,
                units: None,
                status_flags: None,
            },
        };
        objects.push(summary);
    }

    // 3. Read device metadata (vendor, model, firmware) from the Device object.
    let device_info = read_device_info(client, addr, device_id).await;

    Ok(DeviceWalkResult {
        device_id,
        device_info,
        objects,
    })
}

async fn read_device_info<D: DataLink>(
    client: &BacnetClient<D>,
    addr: DataLinkAddress,
    device_id: ObjectId,
) -> DeviceInfo {
    let info_props = &[
        PropertyId::VendorName,
        PropertyId::ModelName,
        PropertyId::FirmwareRevision,
    ];

    let prop_values = match client.read_property_multiple(addr, device_id, info_props).await {
        Ok(v) => v,
        Err(_) => return DeviceInfo::default(),
    };

    let mut info = DeviceInfo::default();
    for (pid, val) in &prop_values {
        if let ClientDataValue::CharacterString(s) = val {
            match pid {
                PropertyId::VendorName => info.vendor_name = Some(s.clone()),
                PropertyId::ModelName => info.model_name = Some(s.clone()),
                PropertyId::FirmwareRevision => info.firmware_revision = Some(s.clone()),
                _ => {}
            }
        }
    }
    info
}

fn extract_object_ids(value: &ClientDataValue) -> Vec<ObjectId> {
    match value {
        ClientDataValue::ObjectId(oid) => vec![*oid],
        ClientDataValue::Constructed { values, .. } => values
            .iter()
            .filter_map(|v| {
                if let ClientDataValue::ObjectId(oid) = v {
                    Some(*oid)
                } else {
                    None
                }
            })
            .collect(),
        _ => vec![],
    }
}

fn build_summary(oid: ObjectId, props: &[(PropertyId, ClientDataValue)]) -> ObjectSummary {
    let mut summary = ObjectSummary {
        object_id: oid,
        object_name: None,
        object_type: oid.object_type(),
        present_value: None,
        description: None,
        units: None,
        status_flags: None,
    };

    for (pid, val) in props {
        match pid {
            PropertyId::ObjectName => {
                if let ClientDataValue::CharacterString(s) = val {
                    summary.object_name = Some(s.clone());
                }
            }
            PropertyId::ObjectType => {
                if let ClientDataValue::Enumerated(v) = val {
                    summary.object_type = ObjectType::from_u16(*v as u16);
                }
            }
            PropertyId::PresentValue => {
                summary.present_value = Some(val.clone());
            }
            PropertyId::Description => {
                if let ClientDataValue::CharacterString(s) = val {
                    summary.description = Some(s.clone());
                }
            }
            PropertyId::Units => {
                if let ClientDataValue::Enumerated(v) = val {
                    summary.units = Some(*v);
                }
            }
            PropertyId::StatusFlags => {
                summary.status_flags = Some(val.clone());
            }
            _ => {}
        }
    }

    summary
}
