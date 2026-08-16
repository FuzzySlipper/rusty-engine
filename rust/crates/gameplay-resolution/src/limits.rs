use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolutionLimits {
    pub max_evidence: usize,
    pub max_program_nodes: usize,
    pub max_program_depth: u16,
    pub max_selected_subjects: usize,
    pub max_interceptors: usize,
    pub max_effects: usize,
    pub max_events: usize,
    pub max_trace_records: usize,
    pub max_child_resolutions: usize,
    pub max_child_depth: u16,
}

impl Default for ResolutionLimits {
    fn default() -> Self {
        Self {
            max_evidence: 256,
            max_program_nodes: 4_096,
            max_program_depth: 64,
            max_selected_subjects: 1_024,
            max_interceptors: 256,
            max_effects: 4_096,
            max_events: 4_096,
            max_trace_records: 16_384,
            max_child_resolutions: 1_024,
            max_child_depth: 32,
        }
    }
}

impl ResolutionLimits {
    pub fn validate(self) -> Result<Self, ResolutionLimitError> {
        let fields = [
            ("evidence", self.max_evidence),
            ("program nodes", self.max_program_nodes),
            ("program depth", usize::from(self.max_program_depth)),
            ("selected subjects", self.max_selected_subjects),
            ("interceptors", self.max_interceptors),
            ("effects", self.max_effects),
            ("events", self.max_events),
            ("trace records", self.max_trace_records),
            ("child resolutions", self.max_child_resolutions),
            ("child depth", usize::from(self.max_child_depth)),
        ];
        for (resource, maximum) in fields {
            if maximum == 0 {
                return Err(ResolutionLimitError::InvalidMaximum { resource });
            }
        }
        Ok(self)
    }

    pub(crate) fn enforce(
        resource: &'static str,
        actual: usize,
        maximum: usize,
    ) -> Result<(), ResolutionLimitError> {
        if actual > maximum {
            return Err(ResolutionLimitError::Exceeded {
                resource,
                actual,
                maximum,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionLimitError {
    InvalidMaximum {
        resource: &'static str,
    },
    Exceeded {
        resource: &'static str,
        actual: usize,
        maximum: usize,
    },
    ArithmeticOverflow {
        resource: &'static str,
    },
}

impl fmt::Display for ResolutionLimitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "gameplay resolution limit rejected: {self:?}")
    }
}

impl std::error::Error for ResolutionLimitError {}
