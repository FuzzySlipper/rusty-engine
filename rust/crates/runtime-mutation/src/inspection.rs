use serde::Serialize;

use crate::compile::CompiledMutationCapability;

/// Deterministic provenance and Product Assembly ownership for one selected
/// mutation operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationCapabilityInspection {
    binding_index: usize,
    binding_id: String,
    target: String,
    resolved_target: String,
    kind: String,
    publication_domain: String,
    owner: String,
    provenance_source: String,
    provenance_path: String,
    maximum_payload_bytes: usize,
}

impl MutationCapabilityInspection {
    fn from_capability(capability: &CompiledMutationCapability) -> Self {
        Self {
            binding_index: capability.binding_index(),
            binding_id: capability.binding_id().to_owned(),
            target: capability.target().to_owned(),
            resolved_target: capability.resolved_target().to_owned(),
            kind: capability.kind().to_owned(),
            publication_domain: capability.publication_domain().to_owned(),
            owner: capability.owner().to_owned(),
            provenance_source: capability.provenance_source().to_owned(),
            provenance_path: capability.provenance_path().to_owned(),
            maximum_payload_bytes: capability.maximum_payload_bytes(),
        }
    }

    pub const fn binding_index(&self) -> usize {
        self.binding_index
    }

    pub fn binding_id(&self) -> &str {
        &self.binding_id
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn resolved_target(&self) -> &str {
        &self.resolved_target
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn publication_domain(&self) -> &str {
        &self.publication_domain
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn provenance_source(&self) -> &str {
        &self.provenance_source
    }

    pub fn provenance_path(&self) -> &str {
        &self.provenance_path
    }

    pub const fn maximum_payload_bytes(&self) -> usize {
        self.maximum_payload_bytes
    }
}

/// Complete deterministic static mutation inspection. It has no independent
/// version: the Product Model and Product Assembly contracts are the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMutationInspection {
    publication_domain: Option<String>,
    capabilities: Vec<MutationCapabilityInspection>,
}

impl RuntimeMutationInspection {
    pub(crate) fn from_capabilities(
        capabilities: &[CompiledMutationCapability],
        publication_domain: Option<&str>,
    ) -> Self {
        Self {
            publication_domain: publication_domain.map(str::to_owned),
            capabilities: capabilities
                .iter()
                .map(MutationCapabilityInspection::from_capability)
                .collect(),
        }
    }

    pub fn publication_domain(&self) -> Option<&str> {
        self.publication_domain.as_deref()
    }

    pub fn capabilities(&self) -> &[MutationCapabilityInspection] {
        &self.capabilities
    }

    pub fn to_json_newline(&self) -> Result<Vec<u8>, String> {
        let mut bytes = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}
