use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResolutionId(u64);

impl ResolutionId {
    pub fn new(value: u64) -> Result<Self, ResolutionIdentityError> {
        if value == 0 {
            return Err(ResolutionIdentityError::ZeroResolutionId);
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CorrelationId(u64);

impl CorrelationId {
    pub fn new(value: u64) -> Result<Self, ResolutionIdentityError> {
        if value == 0 {
            return Err(ResolutionIdentityError::ZeroCorrelationId);
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResolutionIdentity {
    resolution: ResolutionId,
    correlation: CorrelationId,
    parent: Option<ResolutionId>,
    depth: u16,
}

impl ResolutionIdentity {
    pub const fn root(resolution: ResolutionId, correlation: CorrelationId) -> Self {
        Self {
            resolution,
            correlation,
            parent: None,
            depth: 0,
        }
    }

    pub const fn resolution(self) -> ResolutionId {
        self.resolution
    }

    pub const fn correlation(self) -> CorrelationId {
        self.correlation
    }

    pub const fn parent(self) -> Option<ResolutionId> {
        self.parent
    }

    pub const fn depth(self) -> u16 {
        self.depth
    }

    pub(crate) fn child(self, resolution: ResolutionId) -> Result<Self, ResolutionIdentityError> {
        let depth = self
            .depth
            .checked_add(1)
            .ok_or(ResolutionIdentityError::DepthOverflow)?;
        Ok(Self {
            resolution,
            correlation: self.correlation,
            parent: Some(self.resolution),
            depth,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionIdentityError {
    ZeroResolutionId,
    ZeroCorrelationId,
    DepthOverflow,
}

impl fmt::Display for ResolutionIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid gameplay resolution identity: {self:?}")
    }
}

impl std::error::Error for ResolutionIdentityError {}
