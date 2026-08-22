use std::{collections::BTreeSet, fmt};

pub const MAX_ROLE_ID_BYTES: usize = 96;
pub const MAX_CAPABILITY_REQUIREMENTS_PER_ROLE: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityRoleId(String);

impl CapabilityRoleId {
    pub fn parse(value: impl Into<String>) -> Result<Self, RoleRequirementError> {
        let value = value.into();
        validate_id(&value).map_err(|reason| RoleRequirementError::InvalidRoleId {
            value: value.clone(),
            reason,
        })?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityRequirementId(String);

impl CapabilityRequirementId {
    pub fn parse(value: impl Into<String>) -> Result<Self, RoleRequirementError> {
        let value = value.into();
        validate_id(&value).map_err(|reason| RoleRequirementError::InvalidCapabilityId {
            value: value.clone(),
            reason,
        })?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InputId(String);

impl InputId {
    pub fn parse(value: impl Into<String>) -> Result<Self, RoleRequirementError> {
        let value = value.into();
        validate_id(&value).map_err(|reason| RoleRequirementError::InvalidInputId {
            value: value.clone(),
            reason,
        })?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InputKind {
    Parameter,
    Fact,
    Roll,
    BoundedRoll,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleRequirement {
    role: CapabilityRoleId,
    capabilities: Vec<CapabilityRequirementId>,
}

impl RoleRequirement {
    pub fn new(
        role: CapabilityRoleId,
        capabilities: Vec<CapabilityRequirementId>,
    ) -> Result<Self, RoleRequirementError> {
        if capabilities.len() > MAX_CAPABILITY_REQUIREMENTS_PER_ROLE {
            return Err(RoleRequirementError::CapabilityQuotaExceeded {
                actual: capabilities.len(),
                maximum: MAX_CAPABILITY_REQUIREMENTS_PER_ROLE,
            });
        }
        if capabilities.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(RoleRequirementError::NonCanonicalCapabilities);
        }
        Ok(Self { role, capabilities })
    }

    pub fn role(&self) -> &CapabilityRoleId {
        &self.role
    }
    pub fn capabilities(&self) -> &[CapabilityRequirementId] {
        &self.capabilities
    }
}

pub(crate) fn canonicalize_roles(
    roles: Vec<RoleRequirement>,
) -> Result<Vec<RoleRequirement>, RoleRequirementError> {
    let mut merged =
        std::collections::BTreeMap::<CapabilityRoleId, BTreeSet<CapabilityRequirementId>>::new();
    for requirement in roles {
        let capabilities = merged.entry(requirement.role).or_default();
        capabilities.extend(requirement.capabilities);
        if capabilities.len() > MAX_CAPABILITY_REQUIREMENTS_PER_ROLE {
            return Err(RoleRequirementError::CapabilityQuotaExceeded {
                actual: capabilities.len(),
                maximum: MAX_CAPABILITY_REQUIREMENTS_PER_ROLE,
            });
        }
    }
    Ok(merged
        .into_iter()
        .map(|(role, capabilities)| RoleRequirement {
            role,
            capabilities: capabilities.into_iter().collect(),
        })
        .collect())
}

fn validate_id(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("identity is empty");
    }
    if value.len() > MAX_ROLE_ID_BYTES {
        return Err("identity exceeds the byte limit");
    }
    let mut bytes = value.bytes();
    if !bytes.next().is_some_and(|byte| byte.is_ascii_lowercase()) {
        return Err("identity must start with a lowercase ASCII letter");
    }
    if !bytes.all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
    }) {
        return Err("identity contains unsupported characters");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleRequirementError {
    InvalidRoleId { value: String, reason: &'static str },
    InvalidCapabilityId { value: String, reason: &'static str },
    InvalidInputId { value: String, reason: &'static str },
    CapabilityQuotaExceeded { actual: usize, maximum: usize },
    NonCanonicalCapabilities,
}

impl fmt::Display for RoleRequirementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid gameplay-standard role requirement: {self:?}"
        )
    }
}
impl std::error::Error for RoleRequirementError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn capability(value: &str) -> CapabilityRequirementId {
        CapabilityRequirementId::parse(value).unwrap()
    }

    #[test]
    fn role_requirements_reject_unsorted_and_duplicate_capabilities() {
        let role = CapabilityRoleId::parse("self").unwrap();
        for capabilities in [
            vec![capability("read.z"), capability("read.a")],
            vec![capability("read.a"), capability("read.a")],
        ] {
            assert!(matches!(
                RoleRequirement::new(role.clone(), capabilities),
                Err(RoleRequirementError::NonCanonicalCapabilities)
            ));
        }
        assert!(
            RoleRequirement::new(role, vec![capability("read.a"), capability("read.z")]).is_ok()
        );
    }
}
