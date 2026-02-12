//! Point type inference for BACnet objects.
//!
//! Maps BACnet [`ObjectType`](rustbac_core::types::ObjectType) to a simplified
//! classification useful for building automation integrations.

use rustbac_core::types::ObjectType;

/// The data kind of a BACnet point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointKind {
    Analog,
    Binary,
    MultiState,
    Accumulator,
    Unknown,
}

/// Whether a BACnet point is an input, output, or value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointDirection {
    Input,
    Output,
    Value,
    Unknown,
}

/// A simplified classification of a BACnet object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointClassification {
    pub kind: PointKind,
    pub direction: PointDirection,
    /// `true` when the object type supports writing to its present-value.
    pub writable: bool,
}

/// Classify a BACnet object type into a simplified point description.
pub fn classify_point(object_type: ObjectType) -> PointClassification {
    match object_type {
        ObjectType::AnalogInput => PointClassification {
            kind: PointKind::Analog,
            direction: PointDirection::Input,
            writable: false,
        },
        ObjectType::AnalogOutput => PointClassification {
            kind: PointKind::Analog,
            direction: PointDirection::Output,
            writable: true,
        },
        ObjectType::AnalogValue => PointClassification {
            kind: PointKind::Analog,
            direction: PointDirection::Value,
            writable: true,
        },
        ObjectType::BinaryInput => PointClassification {
            kind: PointKind::Binary,
            direction: PointDirection::Input,
            writable: false,
        },
        ObjectType::BinaryOutput => PointClassification {
            kind: PointKind::Binary,
            direction: PointDirection::Output,
            writable: true,
        },
        ObjectType::BinaryValue => PointClassification {
            kind: PointKind::Binary,
            direction: PointDirection::Value,
            writable: true,
        },
        ObjectType::MultiStateInput => PointClassification {
            kind: PointKind::MultiState,
            direction: PointDirection::Input,
            writable: false,
        },
        ObjectType::MultiStateOutput => PointClassification {
            kind: PointKind::MultiState,
            direction: PointDirection::Output,
            writable: true,
        },
        ObjectType::MultiStateValue => PointClassification {
            kind: PointKind::MultiState,
            direction: PointDirection::Value,
            writable: true,
        },
        ObjectType::Accumulator => PointClassification {
            kind: PointKind::Accumulator,
            direction: PointDirection::Input,
            writable: false,
        },
        ObjectType::PulseConverter => PointClassification {
            kind: PointKind::Accumulator,
            direction: PointDirection::Value,
            writable: true,
        },
        _ => PointClassification {
            kind: PointKind::Unknown,
            direction: PointDirection::Unknown,
            writable: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_analog_io() {
        let c = classify_point(ObjectType::AnalogInput);
        assert_eq!(c.kind, PointKind::Analog);
        assert_eq!(c.direction, PointDirection::Input);
        assert!(!c.writable);

        let c = classify_point(ObjectType::AnalogOutput);
        assert_eq!(c.kind, PointKind::Analog);
        assert_eq!(c.direction, PointDirection::Output);
        assert!(c.writable);

        let c = classify_point(ObjectType::AnalogValue);
        assert_eq!(c.kind, PointKind::Analog);
        assert_eq!(c.direction, PointDirection::Value);
        assert!(c.writable);
    }

    #[test]
    fn classify_binary_io() {
        let c = classify_point(ObjectType::BinaryInput);
        assert_eq!(c.kind, PointKind::Binary);
        assert!(!c.writable);

        let c = classify_point(ObjectType::BinaryOutput);
        assert_eq!(c.kind, PointKind::Binary);
        assert!(c.writable);
    }

    #[test]
    fn classify_multistate() {
        let c = classify_point(ObjectType::MultiStateInput);
        assert_eq!(c.kind, PointKind::MultiState);
        assert_eq!(c.direction, PointDirection::Input);

        let c = classify_point(ObjectType::MultiStateOutput);
        assert_eq!(c.kind, PointKind::MultiState);
        assert!(c.writable);
    }

    #[test]
    fn classify_accumulator() {
        let c = classify_point(ObjectType::Accumulator);
        assert_eq!(c.kind, PointKind::Accumulator);
        assert!(!c.writable);

        let c = classify_point(ObjectType::PulseConverter);
        assert_eq!(c.kind, PointKind::Accumulator);
        assert!(c.writable);
    }

    #[test]
    fn classify_unknown() {
        let c = classify_point(ObjectType::Device);
        assert_eq!(c.kind, PointKind::Unknown);
        assert_eq!(c.direction, PointDirection::Unknown);
        assert!(!c.writable);
    }
}
