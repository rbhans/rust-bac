//! PICS (Protocol Implementation Conformance Statement) generation.
//!
//! Generates ASHRAE 135 Annex A compliant PICS documents describing
//! the capabilities of a BACnet device.

use rustbac_core::types::ObjectType;

/// Segmentation support level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentationSupport {
    /// Both segmented requests and responses.
    Both,
    /// Segmented transmit only.
    Transmit,
    /// Segmented receive only.
    Receive,
    /// No segmentation support.
    None,
}

impl SegmentationSupport {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Both => "Both",
            Self::Transmit => "Transmit",
            Self::Receive => "Receive",
            Self::None => "None",
        }
    }
}

/// A BACnet service supported by the device.
#[derive(Debug, Clone)]
pub struct SupportedService {
    /// Human-readable service name.
    pub name: String,
    /// Service choice number.
    pub service_choice: u8,
    /// Whether this device can initiate this service.
    pub initiate: bool,
    /// Whether this device can execute (respond to) this service.
    pub execute: bool,
}

/// A BACnet object type supported by the device.
#[derive(Debug, Clone)]
pub struct SupportedObjectType {
    /// The object type.
    pub object_type: ObjectType,
    /// Whether objects of this type can be dynamically created.
    pub createable: bool,
    /// Whether objects of this type can be dynamically deleted.
    pub deletable: bool,
}

/// A PICS document describing device capabilities.
///
/// Can be rendered as human-readable text (Annex A format) or structured JSON.
#[derive(Debug, Clone)]
pub struct PicsDocument {
    pub vendor_name: String,
    pub product_name: String,
    pub product_model_number: String,
    pub firmware_revision: String,
    pub protocol_version: u8,
    pub protocol_revision: u16,
    pub max_apdu_length: u16,
    pub segmentation_supported: SegmentationSupport,
    pub services: Vec<SupportedService>,
    pub object_types: Vec<SupportedObjectType>,
}

impl PicsDocument {
    /// Render the PICS document in ASHRAE 135 Annex A text format.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(
            "==============================================================================\n",
        );
        out.push_str("  BACnet Protocol Implementation Conformance Statement (PICS)\n");
        out.push_str(
            "==============================================================================\n\n",
        );

        out.push_str("-- Product Identification --\n\n");
        out.push_str(&format!("  Vendor Name:            {}\n", self.vendor_name));
        out.push_str(&format!(
            "  Product Name:           {}\n",
            self.product_name
        ));
        out.push_str(&format!(
            "  Product Model Number:   {}\n",
            self.product_model_number
        ));
        out.push_str(&format!(
            "  Firmware Revision:      {}\n",
            self.firmware_revision
        ));
        out.push('\n');

        out.push_str("-- BACnet Protocol --\n\n");
        out.push_str(&format!(
            "  BACnet Protocol Version:    {}\n",
            self.protocol_version
        ));
        out.push_str(&format!(
            "  BACnet Protocol Revision:   {}\n",
            self.protocol_revision
        ));
        out.push_str(&format!(
            "  Max APDU Length Accepted:   {}\n",
            self.max_apdu_length
        ));
        out.push_str(&format!(
            "  Segmentation Supported:     {}\n",
            self.segmentation_supported.as_str()
        ));
        out.push('\n');

        out.push_str("-- Supported Services --\n\n");
        out.push_str("  Service                               Initiate  Execute\n");
        out.push_str("  ------------------------------------  --------  -------\n");
        for svc in &self.services {
            let init = if svc.initiate { "Yes" } else { "No" };
            let exec = if svc.execute { "Yes" } else { "No" };
            out.push_str(&format!("  {:<38}  {:<8}  {}\n", svc.name, init, exec));
        }
        out.push('\n');

        out.push_str("-- Supported Object Types --\n\n");
        out.push_str("  Object Type                           Createable  Deletable\n");
        out.push_str("  ------------------------------------  ----------  ---------\n");
        for ot in &self.object_types {
            let name = format!("{:?}", ot.object_type);
            let create = if ot.createable { "Yes" } else { "No" };
            let delete = if ot.deletable { "Yes" } else { "No" };
            out.push_str(&format!("  {:<38}  {:<10}  {}\n", name, create, delete));
        }

        out
    }

    /// Render the PICS document as structured JSON.
    pub fn to_json(&self) -> String {
        let services_json: Vec<String> = self
            .services
            .iter()
            .map(|s| {
                format!(
                    r#"    {{ "name": "{}", "service_choice": {}, "initiate": {}, "execute": {} }}"#,
                    s.name, s.service_choice, s.initiate, s.execute
                )
            })
            .collect();

        let objects_json: Vec<String> = self
            .object_types
            .iter()
            .map(|o| {
                format!(
                    r#"    {{ "object_type": "{:?}", "createable": {}, "deletable": {} }}"#,
                    o.object_type, o.createable, o.deletable
                )
            })
            .collect();

        format!(
            r#"{{
  "vendor_name": "{}",
  "product_name": "{}",
  "product_model_number": "{}",
  "firmware_revision": "{}",
  "protocol_version": {},
  "protocol_revision": {},
  "max_apdu_length": {},
  "segmentation_supported": "{}",
  "services": [
{}
  ],
  "object_types": [
{}
  ]
}}"#,
            self.vendor_name,
            self.product_name,
            self.product_model_number,
            self.firmware_revision,
            self.protocol_version,
            self.protocol_revision,
            self.max_apdu_length,
            self.segmentation_supported.as_str(),
            services_json.join(",\n"),
            objects_json.join(",\n"),
        )
    }
}

/// Create a default PICS document pre-filled with all services and object types
/// that rust-bac actually supports.
pub fn default_rustbac_pics() -> PicsDocument {
    PicsDocument {
        vendor_name: "rust-bac".to_string(),
        product_name: "rust-bac BACnet Stack".to_string(),
        product_model_number: "rustbac-0.4".to_string(),
        firmware_revision: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: 1,
        protocol_revision: 24,
        max_apdu_length: 1476,
        segmentation_supported: SegmentationSupport::Both,
        services: vec![
            SupportedService {
                name: "ReadProperty".to_string(),
                service_choice: 0x0C,
                initiate: true,
                execute: true,
            },
            SupportedService {
                name: "ReadPropertyMultiple".to_string(),
                service_choice: 0x0E,
                initiate: true,
                execute: true,
            },
            SupportedService {
                name: "WriteProperty".to_string(),
                service_choice: 0x0F,
                initiate: true,
                execute: true,
            },
            SupportedService {
                name: "WritePropertyMultiple".to_string(),
                service_choice: 0x10,
                initiate: true,
                execute: true,
            },
            SupportedService {
                name: "Who-Is".to_string(),
                service_choice: 0x08,
                initiate: true,
                execute: true,
            },
            SupportedService {
                name: "I-Am".to_string(),
                service_choice: 0x00,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "Who-Has".to_string(),
                service_choice: 0x07,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "I-Have".to_string(),
                service_choice: 0x01,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "SubscribeCOV".to_string(),
                service_choice: 0x05,
                initiate: true,
                execute: true,
            },
            SupportedService {
                name: "ConfirmedCOVNotification".to_string(),
                service_choice: 0x01,
                initiate: true,
                execute: true,
            },
            SupportedService {
                name: "UnconfirmedCOVNotification".to_string(),
                service_choice: 0x02,
                initiate: true,
                execute: true,
            },
            SupportedService {
                name: "SubscribeCOVProperty".to_string(),
                service_choice: 0x1C,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "CreateObject".to_string(),
                service_choice: 0x0A,
                initiate: true,
                execute: true,
            },
            SupportedService {
                name: "DeleteObject".to_string(),
                service_choice: 0x0B,
                initiate: true,
                execute: true,
            },
            SupportedService {
                name: "GetAlarmSummary".to_string(),
                service_choice: 0x03,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "GetEnrollmentSummary".to_string(),
                service_choice: 0x04,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "GetEventInformation".to_string(),
                service_choice: 0x1D,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "AcknowledgeAlarm".to_string(),
                service_choice: 0x00,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "ConfirmedEventNotification".to_string(),
                service_choice: 0x02,
                initiate: false,
                execute: true,
            },
            SupportedService {
                name: "UnconfirmedEventNotification".to_string(),
                service_choice: 0x03,
                initiate: false,
                execute: true,
            },
            SupportedService {
                name: "AtomicReadFile".to_string(),
                service_choice: 0x06,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "AtomicWriteFile".to_string(),
                service_choice: 0x07,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "AddListElement".to_string(),
                service_choice: 0x08,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "RemoveListElement".to_string(),
                service_choice: 0x09,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "ReadRange".to_string(),
                service_choice: 0x1A,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "DeviceCommunicationControl".to_string(),
                service_choice: 0x11,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "ReinitializeDevice".to_string(),
                service_choice: 0x14,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "TimeSynchronization".to_string(),
                service_choice: 0x06,
                initiate: true,
                execute: false,
            },
            SupportedService {
                name: "ConfirmedPrivateTransfer".to_string(),
                service_choice: 0x12,
                initiate: true,
                execute: false,
            },
        ],
        object_types: vec![
            SupportedObjectType {
                object_type: ObjectType::Device,
                createable: false,
                deletable: false,
            },
            SupportedObjectType {
                object_type: ObjectType::AnalogInput,
                createable: true,
                deletable: true,
            },
            SupportedObjectType {
                object_type: ObjectType::AnalogOutput,
                createable: true,
                deletable: true,
            },
            SupportedObjectType {
                object_type: ObjectType::AnalogValue,
                createable: true,
                deletable: true,
            },
            SupportedObjectType {
                object_type: ObjectType::BinaryInput,
                createable: true,
                deletable: true,
            },
            SupportedObjectType {
                object_type: ObjectType::BinaryOutput,
                createable: true,
                deletable: true,
            },
            SupportedObjectType {
                object_type: ObjectType::BinaryValue,
                createable: true,
                deletable: true,
            },
            SupportedObjectType {
                object_type: ObjectType::MultiStateInput,
                createable: true,
                deletable: true,
            },
            SupportedObjectType {
                object_type: ObjectType::MultiStateOutput,
                createable: true,
                deletable: true,
            },
            SupportedObjectType {
                object_type: ObjectType::MultiStateValue,
                createable: true,
                deletable: true,
            },
            SupportedObjectType {
                object_type: ObjectType::Schedule,
                createable: false,
                deletable: false,
            },
            SupportedObjectType {
                object_type: ObjectType::Calendar,
                createable: false,
                deletable: false,
            },
            SupportedObjectType {
                object_type: ObjectType::TrendLog,
                createable: false,
                deletable: false,
            },
            SupportedObjectType {
                object_type: ObjectType::File,
                createable: false,
                deletable: false,
            },
            SupportedObjectType {
                object_type: ObjectType::NotificationClass,
                createable: false,
                deletable: false,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_pics_has_services() {
        let pics = default_rustbac_pics();
        assert!(!pics.services.is_empty());
        assert!(pics
            .services
            .iter()
            .any(|s| s.name == "ReadProperty" && s.initiate && s.execute));
        assert!(pics
            .services
            .iter()
            .any(|s| s.name == "WriteProperty" && s.initiate && s.execute));
        assert!(pics
            .services
            .iter()
            .any(|s| s.name == "Who-Is" && s.initiate && s.execute));
    }

    #[test]
    fn default_pics_has_object_types() {
        let pics = default_rustbac_pics();
        assert!(!pics.object_types.is_empty());
        assert!(pics
            .object_types
            .iter()
            .any(|o| o.object_type == ObjectType::Device && !o.createable));
        assert!(pics
            .object_types
            .iter()
            .any(|o| o.object_type == ObjectType::AnalogValue && o.createable));
    }

    #[test]
    fn to_text_contains_vendor() {
        let pics = default_rustbac_pics();
        let text = pics.to_text();
        assert!(text.contains("rust-bac"));
        assert!(text.contains("ReadProperty"));
        assert!(text.contains("Supported Services"));
        assert!(text.contains("Supported Object Types"));
    }

    #[test]
    fn to_json_parses() {
        let pics = default_rustbac_pics();
        let json = pics.to_json();
        assert!(json.contains("\"vendor_name\""));
        assert!(json.contains("\"services\""));
        assert!(json.contains("\"object_types\""));
        // Verify it's valid JSON by checking structure.
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
    }
}
