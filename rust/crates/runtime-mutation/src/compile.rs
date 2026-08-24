use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

use product_model::{CapabilityKind, LinkedCapabilityTarget, LinkedProductComposition};

use crate::{
    error::RuntimeMutationError,
    inspection::RuntimeMutationInspection,
    model::{validate_identity, MutationCatalogIdentity},
};

/// Maximum number of immutable Product Assembly selections in one catalog.
pub const MAX_COMPILED_MUTATION_CAPABILITIES: usize = product_model::MAX_CAPABILITY_BINDINGS;
/// Maximum compact inspection bytes for one compiled catalog.
pub const MAX_RUNTIME_MUTATION_INSPECTION_BYTES: usize = 1_048_576;

/// Immutable Product Assembly selection for one already-linked operation
/// binding. Mutation is a runtime phase, not an authored CapabilityUse, so
/// this descriptor intentionally lives in this runtime crate rather than in
/// the Compiled Composition schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MutationCapabilityDescriptor {
    binding_id: &'static str,
    target: &'static str,
    publication_domain: &'static str,
    owner: &'static str,
    operation_type: &'static str,
}

impl MutationCapabilityDescriptor {
    pub const fn new(
        binding_id: &'static str,
        target: &'static str,
        publication_domain: &'static str,
        owner: &'static str,
        operation_type: &'static str,
    ) -> Self {
        Self {
            binding_id,
            target,
            publication_domain,
            owner,
            operation_type,
        }
    }

    pub const fn binding_id(self) -> &'static str {
        self.binding_id
    }

    pub const fn target(self) -> &'static str {
        self.target
    }

    pub const fn publication_domain(self) -> &'static str {
        self.publication_domain
    }

    pub const fn owner(self) -> &'static str {
        self.owner
    }

    /// Static Product Kernel operation/result wire identity expected by this
    /// binding. The mutation lane does not interpret payload meaning, but it
    /// retains this identity so standard Engine producers cannot accidentally
    /// feed a different operation contract into the selected binding.
    pub const fn operation_type(self) -> &'static str {
        self.operation_type
    }
}

/// One closed operation binding selected by the Product Assembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledMutationCapability {
    binding_index: usize,
    binding_id: String,
    target: String,
    resolved_target: String,
    publication_domain: String,
    owner: String,
    provenance_source: String,
    provenance_path: String,
    kind: String,
    operation_type: String,
    maximum_payload_bytes: usize,
}

impl CompiledMutationCapability {
    fn from_descriptor(
        binding: &product_model::LinkedCapabilityBinding,
        descriptor: MutationCapabilityDescriptor,
    ) -> Self {
        let metadata = binding.metadata();
        let provenance = metadata.provenance();
        let resolved_target = match binding.resolved_target() {
            LinkedCapabilityTarget::Engine(capability) => capability.target().to_owned(),
            LinkedCapabilityTarget::ProductKernel(index) => {
                format!("product-kernel[{}]", index.index())
            }
        };
        Self {
            binding_index: binding.binding_index(),
            binding_id: binding.id().to_owned(),
            target: binding.target().to_owned(),
            resolved_target,
            publication_domain: descriptor.publication_domain().to_owned(),
            owner: descriptor.owner().to_owned(),
            provenance_source: provenance.source().to_owned(),
            provenance_path: provenance.logical_path().to_owned(),
            kind: metadata.kind().as_str().to_owned(),
            operation_type: descriptor.operation_type().to_owned(),
            maximum_payload_bytes: metadata.budget().maximum_compact_json_payload_bytes(),
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

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn operation_type(&self) -> &str {
        &self.operation_type
    }

    pub const fn maximum_payload_bytes(&self) -> usize {
        self.maximum_payload_bytes
    }
}

/// Immutable static mutation catalog compiled from complete Product Model
/// linkage and an explicit Product Assembly descriptor slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledMutationCatalog {
    capabilities: Vec<CompiledMutationCapability>,
    publication_domain: Option<String>,
    catalog_identity: MutationCatalogIdentity,
    inspection: RuntimeMutationInspection,
}

impl CompiledMutationCatalog {
    pub fn compile(
        linked: &LinkedProductComposition,
        descriptors: &[MutationCapabilityDescriptor],
    ) -> Result<Self, RuntimeMutationError<()>> {
        if descriptors.is_empty() {
            return Err(RuntimeMutationError::EmptyCatalog);
        }
        if descriptors.len() > MAX_COMPILED_MUTATION_CAPABILITIES {
            return Err(RuntimeMutationError::BoundsExceeded(
                "mutation capabilities",
            ));
        }
        let mut binding_ids = BTreeSet::new();
        let mut targets = BTreeSet::new();
        let mut capabilities = Vec::with_capacity(descriptors.len());
        let mut publication_domain: Option<&'static str> = None;
        for (index, descriptor) in descriptors.iter().copied().enumerate() {
            validate_descriptor(index, descriptor)?;
            if let Some(expected) = publication_domain {
                if expected != descriptor.publication_domain() {
                    return Err(RuntimeMutationError::MultiplePublicationDomains {
                        expected: expected.to_owned(),
                        received: descriptor.publication_domain().to_owned(),
                    });
                }
            } else {
                publication_domain = Some(descriptor.publication_domain());
            }
            if !binding_ids.insert(descriptor.binding_id()) {
                return Err(RuntimeMutationError::DuplicateDescriptorBinding(
                    descriptor.binding_id().to_owned(),
                ));
            }
            if !targets.insert(descriptor.target()) {
                return Err(RuntimeMutationError::DuplicateDescriptorTarget(
                    descriptor.target().to_owned(),
                ));
            }
            let binding = linked
                .capability_bindings()
                .iter()
                .find(|binding| binding.id() == descriptor.binding_id())
                .ok_or_else(|| {
                    RuntimeMutationError::UnknownBinding(descriptor.binding_id().to_owned())
                })?;
            if binding.target() != descriptor.target() {
                return Err(RuntimeMutationError::BindingTargetMismatch {
                    binding: descriptor.binding_id().to_owned(),
                    expected: descriptor.target().to_owned(),
                    received: binding.target().to_owned(),
                });
            }
            let metadata = binding.metadata();
            if !metadata.availability().is_linkable() {
                return Err(RuntimeMutationError::CapabilityUnavailable {
                    target: binding.target().to_owned(),
                });
            }
            if metadata.kind() != CapabilityKind::Operation {
                return Err(RuntimeMutationError::CapabilityKindMismatch {
                    target: binding.target().to_owned(),
                    expected: CapabilityKind::Operation.as_str(),
                    received: metadata.kind().as_str().to_owned(),
                });
            }
            capabilities.push(CompiledMutationCapability::from_descriptor(
                binding, descriptor,
            ));
        }
        capabilities.sort_by(|left, right| left.binding_id.cmp(&right.binding_id));
        let catalog_identity = catalog_identity(&capabilities)?;
        let publication_domain = publication_domain.map(str::to_owned);
        let inspection = RuntimeMutationInspection::from_capabilities(
            &capabilities,
            publication_domain.as_deref(),
        );
        let bytes = inspection
            .to_json_newline()
            .map_err(|_| RuntimeMutationError::BoundsExceeded("mutation inspection"))?;
        if bytes.len() > MAX_RUNTIME_MUTATION_INSPECTION_BYTES {
            return Err(RuntimeMutationError::BoundsExceeded("mutation inspection"));
        }
        Ok(Self {
            capabilities,
            publication_domain,
            catalog_identity,
            inspection,
        })
    }

    pub fn capabilities(&self) -> &[CompiledMutationCapability] {
        &self.capabilities
    }

    pub fn capability(&self, binding_id: &str) -> Option<&CompiledMutationCapability> {
        self.capabilities
            .binary_search_by(|capability| capability.binding_id.as_str().cmp(binding_id))
            .ok()
            .and_then(|index| self.capabilities.get(index))
    }

    pub fn publication_domain(&self) -> Option<&str> {
        self.publication_domain.as_deref()
    }

    pub const fn catalog_identity(&self) -> MutationCatalogIdentity {
        self.catalog_identity
    }

    pub fn inspection(&self) -> &RuntimeMutationInspection {
        &self.inspection
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogIdentityEnvelope<'a> {
    capabilities: Vec<CatalogIdentityCapability<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogIdentityCapability<'a> {
    binding_index: usize,
    binding_id: &'a str,
    target: &'a str,
    resolved_target: &'a str,
    publication_domain: &'a str,
    owner: &'a str,
    provenance_source: &'a str,
    provenance_path: &'a str,
    kind: &'a str,
    operation_type: &'a str,
    maximum_payload_bytes: usize,
}

fn catalog_identity(
    capabilities: &[CompiledMutationCapability],
) -> Result<MutationCatalogIdentity, RuntimeMutationError<()>> {
    let envelope = CatalogIdentityEnvelope {
        capabilities: capabilities
            .iter()
            .map(|capability| CatalogIdentityCapability {
                binding_index: capability.binding_index(),
                binding_id: capability.binding_id(),
                target: capability.target(),
                resolved_target: capability.resolved_target(),
                publication_domain: capability.publication_domain(),
                owner: capability.owner(),
                provenance_source: capability.provenance_source(),
                provenance_path: capability.provenance_path(),
                kind: capability.kind(),
                operation_type: capability.operation_type(),
                maximum_payload_bytes: capability.maximum_payload_bytes(),
            })
            .collect(),
    };
    let bytes = serde_json::to_vec(&envelope)
        .map_err(|_| RuntimeMutationError::BoundsExceeded("mutation catalog identity"))?;
    Ok(MutationCatalogIdentity::from_bytes(
        Sha256::digest(bytes).into(),
    ))
}

fn validate_descriptor(
    index: usize,
    descriptor: MutationCapabilityDescriptor,
) -> Result<(), RuntimeMutationError<()>> {
    for (field, value) in [
        ("binding id", descriptor.binding_id()),
        ("target", descriptor.target()),
        ("publication domain", descriptor.publication_domain()),
        ("owner", descriptor.owner()),
        ("operation type", descriptor.operation_type()),
    ] {
        validate_identity(value, product_model::MAX_IDENTITY_BYTES, field)
            .map_err(|_| RuntimeMutationError::InvalidDescriptor { index, field })?;
    }
    Ok(())
}
