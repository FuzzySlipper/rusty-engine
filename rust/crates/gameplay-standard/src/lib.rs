//! Incubating metadata readouts for independently selectable gameplay capabilities.
//!
//! This crate has no gameplay runtime, registry, scheduler, persistence, or aggregate world
//! owner. Each module keeps the exact API of its focused owner adjacent to a stable capability
//! readout; downstream code chooses the modules it uses and retains all gameplay meaning.

#![forbid(unsafe_code)]

use std::fmt;

/// Maximum number of ASCII bytes in a capability identity.
pub const MAX_CAPABILITY_IDENTITY_BYTES: usize = 64;

/// A bounded stable identity for one selectable capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityIdentity(&'static str);

impl CapabilityIdentity {
    /// Validates a lowercase hyphenated capability identity for a static readout.
    pub const fn new(value: &'static str) -> Result<Self, CapabilityIdentityError> {
        let bytes = value.as_bytes();
        if bytes.is_empty() {
            return Err(CapabilityIdentityError::Empty);
        }
        if bytes.len() > MAX_CAPABILITY_IDENTITY_BYTES {
            return Err(CapabilityIdentityError::TooLong);
        }
        if !is_ascii_lowercase(bytes[0]) {
            return Err(CapabilityIdentityError::InvalidStart);
        }
        if !is_ascii_lowercase_or_digit(bytes[bytes.len() - 1]) {
            return Err(CapabilityIdentityError::InvalidEnd);
        }

        let mut index = 1;
        while index < bytes.len() {
            let byte = bytes[index];
            if !is_ascii_lowercase_or_digit(byte) && byte != b'-' {
                return Err(CapabilityIdentityError::InvalidCharacter);
            }
            index += 1;
        }

        Ok(Self(value))
    }

    /// Returns the stable authored identity.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Why a capability identity could not be admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityIdentityError {
    Empty,
    TooLong,
    InvalidStart,
    InvalidEnd,
    InvalidCharacter,
}

impl fmt::Display for CapabilityIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self {
            Self::Empty => "identity is empty",
            Self::TooLong => "identity exceeds the byte limit",
            Self::InvalidStart => "identity must start with a lowercase ASCII letter",
            Self::InvalidEnd => "identity must end with a lowercase ASCII letter or digit",
            Self::InvalidCharacter => "identity contains an unsupported character",
        };
        formatter.write_str(reason)
    }
}

impl std::error::Error for CapabilityIdentityError {}

/// A positive capability schema/API version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityVersion(u32);

impl CapabilityVersion {
    /// Admits a positive version.
    pub const fn new(value: u32) -> Result<Self, CapabilityVersionError> {
        if value == 0 {
            return Err(CapabilityVersionError::Zero);
        }
        Ok(Self(value))
    }

    /// Returns the admitted positive version.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Why a capability version could not be admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityVersionError {
    Zero,
}

impl fmt::Display for CapabilityVersionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("capability version must be positive")
    }
}

impl std::error::Error for CapabilityVersionError {}

/// The current contract maturity of a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CapabilityMaturity {
    /// The metadata and adoption route are available, but compatible additive growth is expected.
    Incubating,
}

/// Static metadata for one independently selectable capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityReadout {
    identity: CapabilityIdentity,
    version: CapabilityVersion,
    maturity: CapabilityMaturity,
}

impl CapabilityReadout {
    const fn new(
        identity: CapabilityIdentity,
        version: CapabilityVersion,
        maturity: CapabilityMaturity,
    ) -> Self {
        Self {
            identity,
            version,
            maturity,
        }
    }

    /// Returns the stable capability identity.
    pub const fn identity(self) -> CapabilityIdentity {
        self.identity
    }

    /// Returns the positive metadata version.
    pub const fn version(self) -> CapabilityVersion {
        self.version
    }

    /// Returns the current contract maturity.
    pub const fn maturity(self) -> CapabilityMaturity {
        self.maturity
    }
}

const fn identity(value: &'static str) -> CapabilityIdentity {
    match CapabilityIdentity::new(value) {
        Ok(identity) => identity,
        Err(_) => panic!("capability identities must be valid"),
    }
}

const fn version(value: u32) -> CapabilityVersion {
    match CapabilityVersion::new(value) {
        Ok(version) => version,
        Err(_) => panic!("capability versions must be positive"),
    }
}

const fn is_ascii_lowercase(byte: u8) -> bool {
    byte >= b'a' && byte <= b'z'
}

const fn is_ascii_lowercase_or_digit(byte: u8) -> bool {
    is_ascii_lowercase(byte) || (byte >= b'0' && byte <= b'9')
}

/// Independently selectable capability modules and their exact focused-owner APIs.
pub mod modules {
    use super::{identity, version, CapabilityMaturity, CapabilityReadout};

    /// Entity facts and typed component mutation remain owned by `entity-state`.
    pub mod entity_state {
        use super::{identity, version, CapabilityMaturity, CapabilityReadout};

        pub use entity_state::*;

        /// Incubating metadata for the entity-state capability.
        pub static READOUT: CapabilityReadout = CapabilityReadout::new(
            identity("entity-state"),
            version(1),
            CapabilityMaturity::Incubating,
        );
    }

    /// Reusable mechanics remain owned by `gameplay-mechanics`.
    pub mod mechanics {
        use super::{identity, version, CapabilityMaturity, CapabilityReadout};

        pub use gameplay_mechanics::*;

        /// Incubating metadata for the mechanics capability.
        pub static READOUT: CapabilityReadout = CapabilityReadout::new(
            identity("mechanics"),
            version(1),
            CapabilityMaturity::Incubating,
        );
    }

    /// Gameplay-attempt lifecycle structure remains owned by `gameplay-resolution`.
    pub mod resolution {
        use super::{identity, version, CapabilityMaturity, CapabilityReadout};

        pub use gameplay_resolution::*;

        /// Incubating metadata for the resolution capability.
        pub static READOUT: CapabilityReadout = CapabilityReadout::new(
            identity("resolution"),
            version(1),
            CapabilityMaturity::Incubating,
        );
    }

    /// Opaque downstream rules-package support remains owned by `gameplay-rules`.
    pub mod rules {
        use super::{identity, version, CapabilityMaturity, CapabilityReadout};

        pub use gameplay_rules::*;

        /// Incubating metadata for the rules capability.
        pub static READOUT: CapabilityReadout = CapabilityReadout::new(
            identity("rules"),
            version(1),
            CapabilityMaturity::Incubating,
        );
    }
}
