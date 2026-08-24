use std::{fmt, marker::PhantomData};

use product_model::{
    CapabilityKind, LinkedCapabilityTarget, LinkedProductComposition, ProductModelError,
};

use crate::declaration::{
    ProductKernelDeclaration, ProductKernelOwner, ProductKernelSelection,
    MAX_PRODUCT_KERNEL_CONTRACT_TEXT_BYTES,
};

/// A selected binding after source-linked Product Assembly validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedProductKernelSelection<O> {
    binding_index: usize,
    binding_id: String,
    owner: O,
}

impl<O: Copy> LinkedProductKernelSelection<O> {
    pub(crate) fn new(binding_index: usize, binding_id: String, owner: O) -> Self {
        Self {
            binding_index,
            binding_id,
            owner,
        }
    }

    pub const fn binding_index(&self) -> usize {
        self.binding_index
    }

    pub fn binding_id(&self) -> &str {
        &self.binding_id
    }

    pub const fn owner(&self) -> O {
        self.owner
    }
}

/// One complete pre-start source-linked Product Assembly.
///
/// Linkage resolves the existing semantic-neutral Product Model catalog first,
/// then checks the caller's concrete owner selections against that immutable
/// result. No lifecycle, schedule, timeline, or mutation instance is created
/// by this type.
#[derive(Debug, Clone, PartialEq)]
pub struct ProductAssembly<D: ProductKernelDeclaration> {
    linked: LinkedProductComposition,
    selections: Vec<LinkedProductKernelSelection<D::Owner>>,
    _declaration: PhantomData<D>,
}

impl<D: ProductKernelDeclaration> ProductAssembly<D> {
    /// Links one admitted composition and all selected Product Kernel owners.
    ///
    /// Every Product Kernel binding must have exactly one selection. Engine
    /// bindings are not selected here. The declaration's own metadata/type
    /// checks and Product Model linkage all happen before this value is
    /// constructed, so a failure cannot publish a partial assembly.
    pub fn link(
        admitted: product_model::AdmittedProductComposition,
        selections: &[ProductKernelSelection<D::Owner>],
    ) -> Result<Self, ProductAssemblyError> {
        validate_declaration::<D>()?;
        let linked = product_model::link_admitted_product_composition(admitted, D::descriptors())
            .map_err(ProductAssemblyError::ProductModel)?;
        let linked_selections = validate_selections::<D>(&linked, selections)?;
        Ok(Self {
            linked,
            selections: linked_selections,
            _declaration: PhantomData,
        })
    }

    pub fn linked(&self) -> &LinkedProductComposition {
        &self.linked
    }

    pub fn selections(&self) -> &[LinkedProductKernelSelection<D::Owner>] {
        &self.selections
    }

    pub fn contract_json() -> Result<String, ProductAssemblyError> {
        D::contract_json()
    }

    pub fn contract_typescript() -> Result<String, ProductAssemblyError> {
        D::contract_typescript()
    }
}

/// Structured failures from source-linked declaration and selection checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductAssemblyError {
    Declaration(DeclarationError),
    ProductModel(ProductModelError),
    DuplicateSelection {
        binding_id: String,
    },
    MissingSelection {
        binding_id: String,
        target: String,
    },
    UnknownSelectionBinding {
        binding_id: String,
    },
    SelectionTargetsEngine {
        binding_id: String,
        target: String,
    },
    SelectionTargetMismatch {
        binding_id: String,
        expected: String,
        received: String,
    },
    SelectionKindMismatch {
        binding_id: String,
        expected: CapabilityKind,
        received: CapabilityKind,
    },
    SelectionTypeMismatch {
        binding_id: String,
        expected: String,
        received: String,
    },
}

impl fmt::Display for ProductAssemblyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "product assembly rejected: {self:?}")
    }
}

impl std::error::Error for ProductAssemblyError {}

/// Failures in the generated source-linked catalog itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclarationError {
    Empty,
    CountMismatch,
    EmptyIdentity {
        index: usize,
    },
    InvalidTarget {
        index: usize,
        target: String,
    },
    DuplicateIdentity {
        identity: String,
    },
    DuplicateTarget {
        target: String,
    },
    ContractTargetMismatch {
        target: String,
        expected: String,
        received: String,
    },
    ContractKindMismatch {
        target: String,
        expected: CapabilityKind,
        received: CapabilityKind,
    },
    EmptyContractType {
        target: String,
    },
    DescriptorMismatch {
        index: usize,
    },
    InvalidIdentity {
        field: String,
        received: String,
    },
    InvalidContractText {
        field: String,
        bytes: usize,
    },
    InvalidMetadata {
        target: String,
        field: String,
    },
    MigrationCapability {
        target: String,
    },
    EmptySchemaIdentity {
        index: usize,
    },
    EmptySchemaContractType {
        index: usize,
    },
    SchemaContractIdentityMismatch {
        identity: String,
        received: String,
    },
    DuplicateSchema {
        identity: String,
    },
    EmptyMigrationIdentity {
        index: usize,
    },
    EmptyMigrationContractType {
        index: usize,
    },
    DuplicateMigration {
        identity: String,
    },
    MissingMigrationSchema {
        migration: String,
        schema: String,
    },
    MigrationContractIdentityMismatch {
        identity: String,
        received: String,
    },
    MigrationFromMismatch {
        identity: String,
        expected: String,
        received: String,
    },
    MigrationToMismatch {
        identity: String,
        expected: String,
        received: String,
    },
}

impl fmt::Display for DeclarationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid Product Kernel declaration: {self:?}")
    }
}

impl std::error::Error for DeclarationError {}

/// Checks the static catalog and its concrete contract metadata before Product
/// Model linkage. This is public for generated build/admission checks.
pub fn validate_declaration<D: ProductKernelDeclaration>() -> Result<(), ProductAssemblyError> {
    let entries = D::entries();
    let descriptors = D::descriptors();
    if entries.is_empty() {
        return Err(ProductAssemblyError::Declaration(DeclarationError::Empty));
    }
    if entries.len() != descriptors.len() {
        return Err(ProductAssemblyError::Declaration(
            DeclarationError::CountMismatch,
        ));
    }
    let mut identities = std::collections::BTreeSet::new();
    let mut targets = std::collections::BTreeSet::new();
    for (index, (entry, descriptor)) in entries.iter().zip(descriptors).enumerate() {
        validate_identity_field(&format!("capabilities[{index}].identity"), entry.identity())?;
        validate_contract_text(
            &format!("capabilities[{index}].contractType"),
            entry.contract_type(),
        )?;
        if entry.metadata().kind() == CapabilityKind::Migration {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::MigrationCapability {
                    target: entry.target().to_owned(),
                },
            ));
        }
        if entry.identity().is_empty() {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::EmptyIdentity { index },
            ));
        }
        let expected_target = format!("kernel.{}", entry.identity());
        if entry.target() != expected_target {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::InvalidTarget {
                    index,
                    target: entry.target().to_owned(),
                },
            ));
        }
        if !identities.insert(entry.identity()) {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::DuplicateIdentity {
                    identity: entry.identity().to_owned(),
                },
            ));
        }
        if !targets.insert(entry.target()) {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::DuplicateTarget {
                    target: entry.target().to_owned(),
                },
            ));
        }
        if entry.contract_type().is_empty() {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::EmptyContractType {
                    target: entry.target().to_owned(),
                },
            ));
        }
        validate_metadata(entry.target(), entry.metadata())?;
        if entry.target() != entry.owner().target() {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::ContractTargetMismatch {
                    target: entry.target().to_owned(),
                    expected: entry.target().to_owned(),
                    received: entry.owner().target().to_owned(),
                },
            ));
        }
        if entry.target() != entry.contract_target() {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::ContractTargetMismatch {
                    target: entry.target().to_owned(),
                    expected: entry.target().to_owned(),
                    received: entry.contract_target().to_owned(),
                },
            ));
        }
        if entry.metadata().kind() != entry.contract_kind() {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::ContractKindMismatch {
                    target: entry.target().to_owned(),
                    expected: entry.metadata().kind(),
                    received: entry.contract_kind(),
                },
            ));
        }
        if descriptor.identity() != entry.identity() || descriptor.metadata() != entry.metadata() {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::DescriptorMismatch { index },
            ));
        }
    }
    validate_schema_declarations::<D>()?;
    Ok(())
}

fn validate_schema_declarations<D: ProductKernelDeclaration>() -> Result<(), ProductAssemblyError> {
    let mut schemas = std::collections::BTreeSet::new();
    for (index, schema) in D::schemas().iter().copied().enumerate() {
        validate_identity_field(&format!("schemas[{index}].identity"), schema.identity())?;
        validate_contract_text(
            &format!("schemas[{index}].contractType"),
            schema.contract_type(),
        )?;
        if schema.identity().is_empty() {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::EmptySchemaIdentity { index },
            ));
        }
        if schema.contract_type().is_empty() {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::EmptySchemaContractType { index },
            ));
        }
        if schema.identity() != schema.contract_identity() {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::SchemaContractIdentityMismatch {
                    identity: schema.identity().to_owned(),
                    received: schema.contract_identity().to_owned(),
                },
            ));
        }
        if !schemas.insert(schema.identity()) {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::DuplicateSchema {
                    identity: schema.identity().to_owned(),
                },
            ));
        }
    }
    let mut migrations = std::collections::BTreeSet::new();
    for (index, migration) in D::migrations().iter().copied().enumerate() {
        validate_identity_field(
            &format!("migrations[{index}].identity"),
            migration.identity(),
        )?;
        validate_identity_field(
            &format!("migrations[{index}].from"),
            migration.from_schema(),
        )?;
        validate_identity_field(&format!("migrations[{index}].to"), migration.to_schema())?;
        validate_contract_text(
            &format!("migrations[{index}].contractType"),
            migration.contract_type(),
        )?;
        if migration.identity().is_empty() {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::EmptyMigrationIdentity { index },
            ));
        }
        if migration.contract_type().is_empty() {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::EmptyMigrationContractType { index },
            ));
        }
        if !migrations.insert(migration.identity()) {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::DuplicateMigration {
                    identity: migration.identity().to_owned(),
                },
            ));
        }
        for schema in [migration.from_schema(), migration.to_schema()] {
            if !schemas.contains(schema) {
                return Err(ProductAssemblyError::Declaration(
                    DeclarationError::MissingMigrationSchema {
                        migration: migration.identity().to_owned(),
                        schema: schema.to_owned(),
                    },
                ));
            }
        }
        if migration.identity() != migration.contract_identity() {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::MigrationContractIdentityMismatch {
                    identity: migration.identity().to_owned(),
                    received: migration.contract_identity().to_owned(),
                },
            ));
        }
        if migration.from_schema() != migration.contract_from_schema() {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::MigrationFromMismatch {
                    identity: migration.identity().to_owned(),
                    expected: migration.from_schema().to_owned(),
                    received: migration.contract_from_schema().to_owned(),
                },
            ));
        }
        if migration.to_schema() != migration.contract_to_schema() {
            return Err(ProductAssemblyError::Declaration(
                DeclarationError::MigrationToMismatch {
                    identity: migration.identity().to_owned(),
                    expected: migration.to_schema().to_owned(),
                    received: migration.contract_to_schema().to_owned(),
                },
            ));
        }
    }
    Ok(())
}

fn validate_identity_field(field: &str, value: &str) -> Result<(), ProductAssemblyError> {
    product_model::validate_product_identity(value).map_err(|_| {
        ProductAssemblyError::Declaration(DeclarationError::InvalidIdentity {
            field: field.to_owned(),
            received: value.to_owned(),
        })
    })
}

fn validate_contract_text(field: &str, value: &str) -> Result<(), ProductAssemblyError> {
    if value.is_empty() || value.len() > MAX_PRODUCT_KERNEL_CONTRACT_TEXT_BYTES {
        return Err(ProductAssemblyError::Declaration(
            DeclarationError::InvalidContractText {
                field: field.to_owned(),
                bytes: value.len(),
            },
        ));
    }
    Ok(())
}

fn validate_metadata(
    target: &str,
    metadata: product_model::CapabilityMetadata,
) -> Result<(), ProductAssemblyError> {
    if metadata.uses().is_empty()
        || metadata.budget().maximum_compact_json_payload_bytes() == 0
        || metadata.budget().maximum_compact_json_payload_bytes()
            > product_model::MAX_COMPILED_COMPOSITION_BYTES
    {
        return Err(invalid_metadata(target, "uses or budget"));
    }
    for (field, values) in [
        ("access.reads", metadata.access().reads()),
        ("access.writes", metadata.access().writes()),
    ] {
        if values.len() > product_model::MAX_SCHEDULE_ACCESS_DECLARATIONS {
            return Err(invalid_metadata(target, field));
        }
        let mut seen = std::collections::BTreeSet::new();
        for (index, value) in values.iter().enumerate() {
            validate_identity_field(&format!("{target}.{field}[{index}]"), value)?;
            if !seen.insert(*value) {
                return Err(invalid_metadata(target, field));
            }
        }
    }
    let provenance = metadata.provenance();
    for (field, value) in [
        ("provenance.owner", provenance.owner()),
        ("provenance.source", provenance.source()),
        ("provenance.logicalPath", provenance.logical_path()),
    ] {
        validate_contract_text(&format!("{target}.{field}"), value)?;
    }
    if let product_model::CapabilityAvailability::Unavailable { reason } = metadata.availability() {
        validate_contract_text(&format!("{target}.availability.reason"), reason)?;
    }
    Ok(())
}

fn invalid_metadata(target: &str, field: &str) -> ProductAssemblyError {
    ProductAssemblyError::Declaration(DeclarationError::InvalidMetadata {
        target: target.to_owned(),
        field: field.to_owned(),
    })
}

fn validate_selections<D: ProductKernelDeclaration>(
    linked: &LinkedProductComposition,
    selections: &[ProductKernelSelection<D::Owner>],
) -> Result<Vec<LinkedProductKernelSelection<D::Owner>>, ProductAssemblyError> {
    let mut selected = std::collections::BTreeSet::new();
    let mut linked_selections = Vec::with_capacity(selections.len());
    for selection in selections {
        if !selected.insert(selection.binding_id()) {
            return Err(ProductAssemblyError::DuplicateSelection {
                binding_id: selection.binding_id().to_owned(),
            });
        }
        let binding = linked
            .capability_bindings()
            .iter()
            .find(|binding| binding.id() == selection.binding_id())
            .ok_or_else(|| ProductAssemblyError::UnknownSelectionBinding {
                binding_id: selection.binding_id().to_owned(),
            })?;
        let target = binding.target().to_owned();
        let owner = selection.owner();
        if !matches!(
            binding.resolved_target(),
            LinkedCapabilityTarget::ProductKernel(_)
        ) {
            return Err(ProductAssemblyError::SelectionTargetsEngine {
                binding_id: selection.binding_id().to_owned(),
                target,
            });
        }
        if target != owner.target() {
            return Err(ProductAssemblyError::SelectionTargetMismatch {
                binding_id: selection.binding_id().to_owned(),
                expected: owner.target().to_owned(),
                received: target,
            });
        }
        let received_kind = binding.metadata().kind();
        if received_kind != owner.kind() {
            return Err(ProductAssemblyError::SelectionKindMismatch {
                binding_id: selection.binding_id().to_owned(),
                expected: owner.kind(),
                received: received_kind,
            });
        }
        if selection.contract_type() != owner.contract_type() {
            return Err(ProductAssemblyError::SelectionTypeMismatch {
                binding_id: selection.binding_id().to_owned(),
                expected: owner.contract_type().to_owned(),
                received: selection.contract_type().to_owned(),
            });
        }
        linked_selections.push(LinkedProductKernelSelection::new(
            binding.binding_index(),
            binding.id().to_owned(),
            owner,
        ));
    }
    for binding in linked.capability_bindings() {
        if matches!(
            binding.resolved_target(),
            LinkedCapabilityTarget::ProductKernel(_)
        ) && !selected.contains(binding.id())
        {
            return Err(ProductAssemblyError::MissingSelection {
                binding_id: binding.id().to_owned(),
                target: binding.target().to_owned(),
            });
        }
    }
    linked_selections.sort_by(|left, right| left.binding_id.cmp(&right.binding_id));
    Ok(linked_selections)
}
