use crate::types::ObjectType;
#[cfg(feature = "serde")]
use core::fmt;

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

#[cfg(feature = "serde")]
impl serde::Serialize for ObjectId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            use serde::ser::SerializeStruct;

            let mut state = serializer.serialize_struct("ObjectId", 2)?;
            state.serialize_field("type", &self.object_type())?;
            state.serialize_field("instance", &self.instance())?;
            state.end()
        } else {
            serializer.serialize_u32(self.raw())
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ObjectId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_any(ObjectIdHumanReadableVisitor)
        } else {
            u32::deserialize(deserializer).map(Self::from_raw)
        }
    }
}

#[cfg(feature = "serde")]
struct ObjectIdHumanReadableVisitor;

#[cfg(feature = "serde")]
impl<'de> serde::de::Visitor<'de> for ObjectIdHumanReadableVisitor {
    type Value = ObjectId;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an object identifier map ({type, instance}) or packed u32")
    }

    fn visit_u32<E>(self, value: u32) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(ObjectId::from_raw(value))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if value > u32::MAX as u64 {
            return Err(E::custom("object id raw value exceeds u32"));
        }
        Ok(ObjectId::from_raw(value as u32))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        #[derive(Clone, Copy)]
        enum Field {
            Type,
            Instance,
        }

        impl<'de> serde::Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> serde::de::Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("`type` or `instance`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "type" => Ok(Field::Type),
                            "instance" => Ok(Field::Instance),
                            _ => Err(E::unknown_field(value, &["type", "instance"])),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        let mut object_type: Option<ObjectType> = None;
        let mut instance: Option<u32> = None;

        while let Some(field) = map.next_key::<Field>()? {
            match field {
                Field::Type => {
                    if object_type.is_some() {
                        return Err(serde::de::Error::duplicate_field("type"));
                    }
                    object_type = Some(map.next_value()?);
                }
                Field::Instance => {
                    if instance.is_some() {
                        return Err(serde::de::Error::duplicate_field("instance"));
                    }
                    instance = Some(map.next_value()?);
                }
            }
        }

        let object_type = object_type.ok_or_else(|| serde::de::Error::missing_field("type"))?;
        let instance = instance.ok_or_else(|| serde::de::Error::missing_field("instance"))?;
        validate_object_id_components::<A::Error>(object_type, instance)?;
        Ok(ObjectId::new(object_type, instance))
    }
}

#[cfg(feature = "serde")]
fn validate_object_id_components<E>(object_type: ObjectType, instance: u32) -> Result<(), E>
where
    E: serde::de::Error,
{
    const MAX_OBJECT_TYPE: u16 = 0x03FF;
    const MAX_INSTANCE: u32 = 0x3F_FFFF;

    if object_type.to_u16() > MAX_OBJECT_TYPE {
        return Err(E::custom(format!(
            "object type {} exceeds 10-bit BACnet range",
            object_type.to_u16()
        )));
    }
    if instance > MAX_INSTANCE {
        return Err(E::custom(format!(
            "instance {} exceeds 22-bit BACnet range",
            instance
        )));
    }
    Ok(())
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

    #[cfg(feature = "serde")]
    #[test]
    fn serde_rejects_out_of_range_instance() {
        let err = serde_json::from_str::<ObjectId>(
            r#"{"type":"AnalogInput","instance":4194304}"#,
        )
        .expect_err("instance above 22-bit range should fail");
        assert!(
            err.to_string().contains("instance"),
            "unexpected error: {err}"
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_rejects_out_of_range_object_type() {
        let err = serde_json::from_str::<ObjectId>(
            r#"{"type":{"Proprietary":1024},"instance":1}"#,
        )
        .expect_err("object type above 10-bit range should fail");
        assert!(
            err.to_string().contains("object type"),
            "unexpected error: {err}"
        );
    }
}
