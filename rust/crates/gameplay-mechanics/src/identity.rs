use std::fmt;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

pub const MAX_MECHANICS_ID_BYTES: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MechanicsIdentityError {
    pub value: String,
    pub reason: &'static str,
}

impl fmt::Display for MechanicsIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid mechanics identity {:?}: {}",
            self.value, self.reason
        )
    }
}

impl std::error::Error for MechanicsIdentityError {}

fn validate_identity(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("identity is empty");
    }
    if value.len() > MAX_MECHANICS_ID_BYTES {
        return Err("identity exceeds the byte limit");
    }
    let mut bytes = value.bytes();
    if !bytes.next().is_some_and(|value| value.is_ascii_lowercase()) {
        return Err("identity must start with a lowercase ASCII letter");
    }
    if !bytes.all(|value| {
        value.is_ascii_lowercase() || value.is_ascii_digit() || matches!(value, b'.' | b'-' | b'_')
    }) {
        return Err("identity contains unsupported characters");
    }
    Ok(())
}

macro_rules! mechanics_id {
    ($(#[$attribute:meta])* $name:ident) => {
        $(#[$attribute])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self, MechanicsIdentityError> {
                let value = value.into();
                validate_identity(&value).map_err(|reason| MechanicsIdentityError {
                    value: value.clone(),
                    reason,
                })?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(value).map_err(de::Error::custom)
            }
        }
    };
}

mechanics_id!(/// Downstream-owned compatibility version for one admitted catalog.
    CatalogVersion);
mechanics_id!(/// One authored evaluated scalar.
    StatId);
mechanics_id!(/// One authored persistent bounded quantity.
    TrackId);
mechanics_id!(/// One authored contribution/response definition.
    SourceDefinitionId);
mechanics_id!(/// One durable intrinsic or request-local activation identity.
    SourceInstanceId);
mechanics_id!(/// One authored stacking policy group.
    StackingGroupId);
mechanics_id!(/// One authored active-effect definition.
    EffectDefinitionId);
mechanics_id!(/// One live effect activation.
    EffectInstanceId);
mechanics_id!(/// One authored item definition.
    ItemDefinitionId);
mechanics_id!(/// One authored equipment slot.
    EquipmentSlotId);
mechanics_id!(/// One caller-defined inventory capacity dimension.
    CapacityMetricId);
mechanics_id!(/// One caller-defined structural item classification.
    ItemClassificationId);
mechanics_id!(/// One caller-defined mutually exclusive equipment group.
    EquipmentExclusivityId);
mechanics_id!(/// One authored damage classification.
    DamageKindId);
mechanics_id!(/// One caller-owned operation correlation identity.
    OperationId);

#[cfg(test)]
mod tests {
    use super::{MechanicsIdentityError, StatId, MAX_MECHANICS_ID_BYTES};

    #[test]
    fn identity_is_stable_and_strictly_decoded() {
        let id = StatId::parse("max_health").unwrap();
        assert_eq!(id.as_str(), "max_health");
        let encoded = serde_json::to_string(&id).unwrap();
        assert_eq!(encoded, "\"max_health\"");
        assert_eq!(serde_json::from_str::<StatId>(&encoded).unwrap(), id);
        assert!(serde_json::from_str::<StatId>("\"MaxHealth\"").is_err());
    }

    #[test]
    fn identity_rejects_empty_invalid_and_oversized_values() {
        for value in ["", "9health", "health value", "health/maximum"] {
            assert!(matches!(
                StatId::parse(value),
                Err(MechanicsIdentityError { .. })
            ));
        }
        assert!(StatId::parse(format!("a{}", "x".repeat(MAX_MECHANICS_ID_BYTES))).is_err());
    }
}
