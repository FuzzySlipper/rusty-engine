use std::fmt;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

pub const MAX_CONTINUOUS_MECHANICS_ID_BYTES: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousMechanicsIdentityError {
    pub value: String,
    pub reason: &'static str,
}

impl fmt::Display for ContinuousMechanicsIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid continuous mechanics identity {:?}: {}",
            self.value, self.reason
        )
    }
}
impl std::error::Error for ContinuousMechanicsIdentityError {}

fn validate(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("identity is empty");
    }
    if value.len() > MAX_CONTINUOUS_MECHANICS_ID_BYTES {
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

macro_rules! continuous_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);
        impl $name {
            pub fn parse(
                value: impl Into<String>,
            ) -> Result<Self, ContinuousMechanicsIdentityError> {
                let value = value.into();
                validate(&value).map_err(|reason| ContinuousMechanicsIdentityError {
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
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
        impl Serialize for $name {
            fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                s.serialize_str(self.as_str())
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(d: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Self::parse(String::deserialize(d)?).map_err(de::Error::custom)
            }
        }
    };
}

continuous_id!(ContinuousCatalogVersion);
continuous_id!(ContinuousStatId);
continuous_id!(ContinuousTrackId);
continuous_id!(ContinuousSourceDefinitionId);
continuous_id!(ContinuousSourceInstanceId);
continuous_id!(ContinuousEffectDefinitionId);
continuous_id!(ContinuousEffectInstanceId);
continuous_id!(ContinuousStackingGroupId);
continuous_id!(ContinuousOperationId);
