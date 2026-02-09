use crate::types::ObjectType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectId(u32);

impl ObjectId {
    pub const fn new(object_type: ObjectType, instance: u32) -> Self {
        Self((((object_type.to_u16() as u32) & 0x03FF) << 22) | (instance & 0x3F_FFFF))
    }

    pub const fn raw(self) -> u32 {
        self.0
    }

    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn object_type(self) -> ObjectType {
        let t = ((self.0 >> 22) & 0x03FF) as u16;
        ObjectType::from_u16(t)
    }

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
