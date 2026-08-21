use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

pub const CURRENT_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion(1);
pub const MAX_COMMAND_ID_BYTES: usize = 128;
pub const MAX_CORRELATION_ID_BYTES: usize = 128;
pub const MAX_RUNTIME_INSTANCE_ID_BYTES: usize = 128;
pub const MAX_PROFILE_ID_BYTES: usize = 128;

/// A nonzero version for a public command envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct ProtocolVersion(u16);

impl ProtocolVersion {
    pub const fn new(value: u16) -> Option<Self> {
        if value == 0 {
            None
        } else {
            Some(Self(value))
        }
    }

    pub const fn current() -> Self {
        CURRENT_PROTOCOL_VERSION
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}

impl<'de> Deserialize<'de> for ProtocolVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = u16::deserialize(deserializer)?;
        Self::new(value).ok_or_else(|| {
            <D::Error as serde::de::Error>::custom("protocol version must be nonzero")
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandIdentityError {
    pub value: String,
    pub reason: &'static str,
}

impl fmt::Display for CommandIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid command identity {:?}: {}",
            self.value, self.reason
        )
    }
}

impl std::error::Error for CommandIdentityError {}

macro_rules! stable_identity {
    ($name:ident, $limit:ident, $description:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self, CommandIdentityError> {
                let value = value.into();
                validate_identity(&value, $limit).map_err(|reason| CommandIdentityError {
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

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                Self::parse(value)
                    .map_err(|error| <D::Error as serde::de::Error>::custom(error.to_string()))
            }
        }
    };
}

stable_identity!(CommandId, MAX_COMMAND_ID_BYTES, "command identity");
stable_identity!(CommandAlias, MAX_COMMAND_ID_BYTES, "command alias");
stable_identity!(
    CorrelationId,
    MAX_CORRELATION_ID_BYTES,
    "correlation identity"
);
stable_identity!(
    RuntimeInstanceId,
    MAX_RUNTIME_INSTANCE_ID_BYTES,
    "runtime identity"
);
stable_identity!(ProfileId, MAX_PROFILE_ID_BYTES, "profile identity");

fn validate_identity(value: &str, max_bytes: usize) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("must not be empty");
    }
    if value.len() > max_bytes {
        return Err("exceeds the byte limit");
    }
    if !value.bytes().all(|byte| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || matches!(byte, b'.' | b'-' | b'_' | b':')
    }) {
        return Err("must use lowercase ASCII letters, digits, '.', '-', '_', or ':'");
    }
    Ok(())
}
