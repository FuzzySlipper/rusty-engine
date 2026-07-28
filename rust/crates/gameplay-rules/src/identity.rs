use std::fmt;

use sha2::{Digest, Sha256};

use crate::RulePackageError;

pub const MAX_RULE_ID_BYTES: usize = 128;
pub const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

fn validate_identity(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("identity is empty");
    }
    if value.len() > MAX_RULE_ID_BYTES {
        return Err("identity exceeds the byte limit");
    }
    if value.trim() != value {
        return Err("identity has leading or trailing whitespace");
    }
    if !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        return Err("identity must contain printable ASCII only");
    }
    Ok(())
}

macro_rules! rule_id {
    ($name:ident, $path:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self, RulePackageError> {
                Self::parse_at(value.into(), $path)
            }

            pub(crate) fn parse_at(value: String, path: &str) -> Result<Self, RulePackageError> {
                validate_identity(&value).map_err(|reason| RulePackageError::InvalidIdentity {
                    path: path.to_string(),
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
    };
}

rule_id!(RuleDomainId, "domain");
rule_id!(RulePackageId, "package");
rule_id!(RuleSourceId, "source");
rule_id!(RuleSubjectId, "subject");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleVersion(u64);

impl RuleVersion {
    pub fn new(value: u64) -> Result<Self, RulePackageError> {
        Self::new_at(value, "version")
    }

    pub(crate) fn new_at(value: u64, path: &str) -> Result<Self, RulePackageError> {
        if value == 0 || value > MAX_SAFE_JSON_INTEGER {
            return Err(RulePackageError::InvalidVersion {
                path: path.to_string(),
                value: value.to_string(),
            });
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for RuleVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleFingerprint(String);

impl RuleFingerprint {
    pub fn parse(value: impl Into<String>) -> Result<Self, RulePackageError> {
        Self::parse_at(value.into(), "fingerprint")
    }

    pub(crate) fn parse_at(value: String, path: &str) -> Result<Self, RulePackageError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(RulePackageError::InvalidFingerprint {
                path: path.to_string(),
                value,
            });
        }
        Ok(Self(value))
    }

    pub(crate) fn for_bytes(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut value = String::with_capacity(64);
        for byte in digest {
            use std::fmt::Write;
            write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
        }
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RuleFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RulePackageIdentity {
    domain: RuleDomainId,
    package: RulePackageId,
    version: RuleVersion,
}

impl RulePackageIdentity {
    pub fn new(domain: RuleDomainId, package: RulePackageId, version: RuleVersion) -> Self {
        Self {
            domain,
            package,
            version,
        }
    }

    pub const fn domain(&self) -> &RuleDomainId {
        &self.domain
    }

    pub const fn package(&self) -> &RulePackageId {
        &self.package
    }

    pub const fn version(&self) -> RuleVersion {
        self.version
    }
}

impl fmt::Display for RulePackageIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}/{}@{}",
            self.domain, self.package, self.version
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RulePackageDependency {
    identity: RulePackageIdentity,
    fingerprint: Option<RuleFingerprint>,
}

impl RulePackageDependency {
    pub fn new(
        domain: RuleDomainId,
        package: RulePackageId,
        version: RuleVersion,
        fingerprint: Option<RuleFingerprint>,
    ) -> Self {
        Self {
            identity: RulePackageIdentity::new(domain, package, version),
            fingerprint,
        }
    }

    pub const fn identity(&self) -> &RulePackageIdentity {
        &self.identity
    }

    pub const fn domain(&self) -> &RuleDomainId {
        self.identity.domain()
    }

    pub const fn package(&self) -> &RulePackageId {
        self.identity.package()
    }

    pub const fn version(&self) -> RuleVersion {
        self.identity.version()
    }

    pub const fn fingerprint(&self) -> Option<&RuleFingerprint> {
        self.fingerprint.as_ref()
    }
}
