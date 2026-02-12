use crate::types::ObjectType;

/// A packed BACnet object identifier combining an [`ObjectType`] and a 22-bit
/// instance number into a single `u32`.
///
/// The upper 10 bits encode the object type and the lower 22 bits encode the
/// instance number, matching the BACnet wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(u32);

impl ObjectId {
    /// Creates an `ObjectId` from a type and instance number.
    pub const fn new(object_type: ObjectType, instance: u32) -> Self {
        Self((((object_type.to_u16() as u32) & 0x03FF) << 22) | (instance & 0x3F_FFFF))
    }

    /// Returns the raw packed `u32` representation.
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Constructs an `ObjectId` from a pre-packed `u32`.
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Extracts the [`ObjectType`] from the upper 10 bits.
    pub const fn object_type(self) -> ObjectType {
        let t = ((self.0 >> 22) & 0x03FF) as u16;
        ObjectType::from_u16(t)
    }

    /// Extracts the 22-bit instance number.
    pub const fn instance(self) -> u32 {
        self.0 & 0x3F_FFFF
    }
}

#[cfg(test)]
mod tests {
    use super::ObjectId;
    use crate::types::ObjectType;

    #[test]
    fn encodes_object_id() {
        let id = ObjectId::new(ObjectType::AnalogInput, 1);
        assert_eq!(id.object_type(), ObjectType::AnalogInput);
        assert_eq!(id.instance(), 1);
    }
}
