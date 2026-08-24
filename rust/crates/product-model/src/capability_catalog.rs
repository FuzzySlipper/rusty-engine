//! Closed capability linkage for an admitted Product Composition.
//!
//! This module is deliberately a data-only pre-start linker. It resolves
//! authored targets to a fixed Engine enum or an assembly-supplied Product
//! Kernel descriptor index, but it neither stores handlers nor invokes work.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::{
    diagnostic::failure, manifest::validate_identity, AdmittedCapabilityBinding,
    AdmittedCapabilityReference, AdmittedProductComposition, ProductModelError,
    MAX_CAPABILITY_BINDINGS, MAX_COMPILED_COMPOSITION_BYTES, MAX_SCHEDULE_ACCESS_DECLARATIONS,
};

const COMPOSITION_SOURCE: &str = "compiled-composition.json";
const KERNEL_SOURCE: &str = "product-kernel-capabilities";
const ENGINE_SOURCE: &str = "engine-capability-catalog";
pub const MAX_PRODUCT_KERNEL_CAPABILITIES: usize = MAX_CAPABILITY_BINDINGS;
pub const MAX_CAPABILITY_PROVENANCE_BYTES: usize = 512;

/// The closed leaf categories recognized by Product Model linkage.
///
/// These are linkage categories, not a universal dispatch vocabulary. The
/// currently linkable Engine table is intentionally smaller than this set;
/// Product Kernel descriptors may name any category when their generated
/// assembly supplies the corresponding concrete owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityKind {
    System,
    Operation,
    Query,
    Projection,
    Migration,
}

impl CapabilityKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Operation => "operation",
            Self::Query => "query",
            Self::Projection => "projection",
            Self::Migration => "migration",
        }
    }
}

/// A Compiled Composition section that can reference one capability binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityUse {
    InputMap,
    Schedule,
    Timeline,
}

impl CapabilityUse {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InputMap => "input-map",
            Self::Schedule => "schedule",
            Self::Timeline => "timeline",
        }
    }
}

/// A closed bit set of supported Compiled Composition reference positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityUses(u8);

impl CapabilityUses {
    pub const NONE: Self = Self(0);
    pub const INPUT_MAP: Self = Self(1);
    pub const SCHEDULE: Self = Self(1 << 1);
    pub const TIMELINE: Self = Self(1 << 2);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, usage: CapabilityUse) -> bool {
        let flag = match usage {
            CapabilityUse::InputMap => Self::INPUT_MAP.0,
            CapabilityUse::Schedule => Self::SCHEDULE.0,
            CapabilityUse::Timeline => Self::TIMELINE.0,
        };
        self.0 & flag != 0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Whether a catalog entry can participate in source-linked assembly linkage.
///
/// `Linkable` proves only that a closed named owner was selected. It does not
/// claim that this crate supplies a runtime evaluator or a function table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityAvailability {
    Linkable,
    Unavailable { reason: &'static str },
}

impl CapabilityAvailability {
    pub const fn is_linkable(self) -> bool {
        matches!(self, Self::Linkable)
    }

    pub const fn reason(self) -> Option<&'static str> {
        match self {
            Self::Linkable => None,
            Self::Unavailable { reason } => Some(reason),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linkable => "linkable",
            Self::Unavailable { .. } => "unavailable",
        }
    }
}

/// Exact declared access facts required by a scheduled capability.
///
/// Access declarations remain scheduler-neutral data. Equality is checked only
/// to ensure the authored declaration matches the selected closed descriptor;
/// this module assigns neither conflict nor execution order semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityAccess {
    reads: &'static [&'static str],
    writes: &'static [&'static str],
}

impl CapabilityAccess {
    pub const fn new(reads: &'static [&'static str], writes: &'static [&'static str]) -> Self {
        Self { reads, writes }
    }

    pub const fn reads(self) -> &'static [&'static str] {
        self.reads
    }

    pub const fn writes(self) -> &'static [&'static str] {
        self.writes
    }
}

/// Per-reference compact-JSON payload budget retained by a closed descriptor.
///
/// The linker measures this with `serde_json::to_vec` over the already-admitted
/// `serde_json::Value`: compact Rust JSON, without whitespace. It is a local
/// assembly admission budget, not a promise about canonical artifact bytes or
/// a cross-language payload-size protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityBudget {
    maximum_compact_json_payload_bytes: usize,
}

impl CapabilityBudget {
    pub const fn new(maximum_compact_json_payload_bytes: usize) -> Self {
        Self {
            maximum_compact_json_payload_bytes,
        }
    }

    pub const fn maximum_compact_json_payload_bytes(self) -> usize {
        self.maximum_compact_json_payload_bytes
    }
}

/// Source ownership and a stable logical owner path for a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityProvenance {
    owner: &'static str,
    source: &'static str,
    logical_path: &'static str,
}

impl CapabilityProvenance {
    pub const fn new(
        owner: &'static str,
        source: &'static str,
        logical_path: &'static str,
    ) -> Self {
        Self {
            owner,
            source,
            logical_path,
        }
    }

    pub const fn owner(self) -> &'static str {
        self.owner
    }

    pub const fn source(self) -> &'static str {
        self.source
    }

    pub const fn logical_path(self) -> &'static str {
        self.logical_path
    }
}

/// Metadata shared by Engine and Product Kernel closed descriptors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityMetadata {
    kind: CapabilityKind,
    uses: CapabilityUses,
    availability: CapabilityAvailability,
    access: CapabilityAccess,
    budget: CapabilityBudget,
    provenance: CapabilityProvenance,
}

impl CapabilityMetadata {
    pub const fn new(
        kind: CapabilityKind,
        uses: CapabilityUses,
        availability: CapabilityAvailability,
        access: CapabilityAccess,
        budget: CapabilityBudget,
        provenance: CapabilityProvenance,
    ) -> Self {
        Self {
            kind,
            uses,
            availability,
            access,
            budget,
            provenance,
        }
    }

    pub const fn kind(self) -> CapabilityKind {
        self.kind
    }

    pub const fn uses(self) -> CapabilityUses {
        self.uses
    }

    pub const fn availability(self) -> CapabilityAvailability {
        self.availability
    }

    pub const fn access(self) -> CapabilityAccess {
        self.access
    }

    pub const fn budget(self) -> CapabilityBudget {
        self.budget
    }

    pub const fn provenance(self) -> CapabilityProvenance {
        self.provenance
    }
}

/// One Engine-owned static catalog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineCapabilityDescriptor {
    capability: EngineCapability,
    metadata: CapabilityMetadata,
}

impl EngineCapabilityDescriptor {
    pub const fn capability(self) -> EngineCapability {
        self.capability
    }

    pub const fn target(self) -> &'static str {
        self.capability.target()
    }

    pub const fn metadata(self) -> CapabilityMetadata {
        self.metadata
    }
}

/// Defines the complete Engine catalog once, then expands it into both the
/// closed Rust binding enum/match and the descriptor table exported to
/// TypeScript. Adding an Engine capability cannot update one surface without
/// the other.
macro_rules! define_engine_capability_catalog {
    ($(
        $variant:ident {
            target: $target:literal,
            kind: $kind:expr,
            uses: $uses:expr,
            availability: $availability:expr,
            reads: $reads:expr,
            writes: $writes:expr,
            maximum_compact_json_payload_bytes: $budget:expr,
            owner: $owner:literal,
            source: $source:literal,
            logical_path: $logical_path:literal,
        }
    )+) => {
        /// The complete fixed set of current Engine capability bindings.
        ///
        /// This enum has no catch-all variant: a new Engine target requires an
        /// intentional catalog declaration and TypeScript catalog regeneration.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum EngineCapability {
            $($variant,)+
        }

        impl EngineCapability {
            pub const fn target(self) -> &'static str {
                match self {
                    $(Self::$variant => $target,)+
                }
            }

            pub const fn all() -> &'static [Self] {
                &[$(Self::$variant,)+]
            }
        }

        const ENGINE_CAPABILITIES: &[EngineCapabilityDescriptor] = &[
            $(EngineCapabilityDescriptor {
                capability: EngineCapability::$variant,
                metadata: CapabilityMetadata::new(
                    $kind,
                    $uses,
                    $availability,
                    CapabilityAccess::new($reads, $writes),
                    CapabilityBudget::new($budget),
                    CapabilityProvenance::new($owner, $source, $logical_path),
                ),
            },)+
        ];
    };
}

define_engine_capability_catalog! {
    EntityRenderProject {
        target: "engine.render.entity-project",
        kind: CapabilityKind::Projection,
        uses: CapabilityUses::SCHEDULE,
        availability: CapabilityAvailability::Linkable,
        reads: &["entity-state.projection"],
        writes: &["render-frame.diff"],
        maximum_compact_json_payload_bytes: 1_024,
        owner: "rusty-engine.render-projection",
        source: "rust/crates/render-projection/src/entity.rs",
        logical_path: "EntityRenderProjector::project",
    }
}

/// Returns the deterministic Rust-owned Engine descriptor table.
pub fn engine_capability_descriptors() -> &'static [EngineCapabilityDescriptor] {
    ENGINE_CAPABILITIES
}

/// Verifies that the Rust-owned static Engine table remains a closed, complete
/// source for generated names and pre-start linkage. The check is intentionally
/// explicit so export and linkage fail loudly if a future table edit drifts.
pub fn validate_engine_capability_descriptors() -> Result<(), ProductModelError> {
    let mut targets = BTreeSet::new();
    for (index, descriptor) in engine_capability_descriptors().iter().copied().enumerate() {
        let target = descriptor.target();
        let local = target.strip_prefix("engine.").ok_or_else(|| {
            failure(
                "RUNTIME_CAPABILITY_ENGINE_TARGET_NAMESPACE",
                ENGINE_SOURCE,
                format!("engineCapabilities[{index}].target"),
                format!("Engine descriptor target `{target}` must start with engine."),
            )
        })?;
        validate_identity(
            local,
            ENGINE_SOURCE,
            &format!("engineCapabilities[{index}].target"),
        )?;
        if !targets.insert(target) {
            return Err(failure(
                "RUNTIME_CAPABILITY_DUPLICATE_ENGINE_TARGET",
                ENGINE_SOURCE,
                format!("engineCapabilities[{index}].target"),
                format!("Engine capability target `{target}` is declared more than once"),
            ));
        }
        validate_metadata(
            descriptor.metadata(),
            ENGINE_SOURCE,
            &format!("engineCapabilities[{index}]"),
        )?;
    }
    Ok(())
}

/// One immutable Product Kernel descriptor supplied by the source-linked
/// Product Assembly. It contains no handler, callback, trait object, or
/// registration operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductKernelCapabilityDescriptor {
    identity: &'static str,
    metadata: CapabilityMetadata,
}

impl ProductKernelCapabilityDescriptor {
    pub const fn new(identity: &'static str, metadata: CapabilityMetadata) -> Self {
        Self { identity, metadata }
    }

    pub const fn identity(self) -> &'static str {
        self.identity
    }

    pub const fn metadata(self) -> CapabilityMetadata {
        self.metadata
    }
}

/// A deterministic bytewise-identity ordinal for the closed Product Kernel
/// descriptor aggregate. It is independent of declaration order and is a
/// resolved declaration identity, not a dynamic dispatch key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductKernelCapabilityIndex(usize);

impl ProductKernelCapabilityIndex {
    pub const fn index(self) -> usize {
        self.0
    }
}

/// The resolved owner selected for one authored binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkedCapabilityTarget {
    Engine(EngineCapability),
    ProductKernel(ProductKernelCapabilityIndex),
}

/// One immutable binding after complete catalog linkage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedCapabilityBinding {
    binding_index: usize,
    id: String,
    target: String,
    resolved_target: LinkedCapabilityTarget,
    metadata: CapabilityMetadata,
}

impl LinkedCapabilityBinding {
    pub const fn binding_index(&self) -> usize {
        self.binding_index
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub const fn resolved_target(&self) -> LinkedCapabilityTarget {
        self.resolved_target
    }

    pub const fn metadata(&self) -> CapabilityMetadata {
        self.metadata
    }
}

/// An admitted Product Composition whose every declared target and every use
/// has been resolved against the closed Engine/Kernel capability set.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkedProductComposition {
    admitted: AdmittedProductComposition,
    capability_bindings: Vec<LinkedCapabilityBinding>,
}

impl LinkedProductComposition {
    pub fn admitted(&self) -> &AdmittedProductComposition {
        &self.admitted
    }

    pub fn capability_bindings(&self) -> &[LinkedCapabilityBinding] {
        &self.capability_bindings
    }

    pub fn capability_binding(&self, index: usize) -> Option<&LinkedCapabilityBinding> {
        self.capability_bindings.get(index)
    }
}

/// Links an admitted composition to all current Engine descriptors and one
/// complete, caller-supplied Product Kernel descriptor slice.
///
/// The function validates every binding, including declared-but-unreferenced
/// bindings, before returning any linked value. It performs no scheduling,
/// evaluation, mutation, service lookup, registration, or callback invocation.
pub fn link_admitted_product_composition(
    admitted: AdmittedProductComposition,
    kernel_capabilities: &[ProductKernelCapabilityDescriptor],
) -> Result<LinkedProductComposition, ProductModelError> {
    validate_engine_capability_descriptors()?;
    let kernel_indices = validate_kernel_capabilities(kernel_capabilities)?;
    let mut targets = BTreeMap::new();
    let mut bindings = Vec::with_capacity(admitted.capability_bindings().len());

    for binding in admitted.capability_bindings() {
        if let Some(first_id) = targets.insert(binding.target().to_owned(), binding.id().to_owned())
        {
            return Err(composition_failure(
                "RUNTIME_CAPABILITY_DUPLICATE_TARGET",
                format!("capabilityBindings[{}].target", binding.index()),
                format!(
                    "capability target `{}` is bound by both `{first_id}` and `{}`; bind it once and reuse that authored capability id",
                    binding.target(),
                    binding.id(),
                ),
            ));
        }

        let (resolved_target, metadata) =
            resolve_target(binding, kernel_capabilities, &kernel_indices)?;
        bindings.push(LinkedCapabilityBinding {
            binding_index: binding.index(),
            id: binding.id().to_owned(),
            target: binding.target().to_owned(),
            resolved_target,
            metadata,
        });
    }

    validate_input_map_uses(&admitted, &bindings)?;
    validate_schedule_uses(&admitted, &bindings)?;
    validate_timeline_uses(&admitted, &bindings)?;

    Ok(LinkedProductComposition {
        admitted,
        capability_bindings: bindings,
    })
}

fn validate_kernel_capabilities(
    descriptors: &[ProductKernelCapabilityDescriptor],
) -> Result<BTreeMap<&'static str, KernelDescriptorLocation>, ProductModelError> {
    if descriptors.len() > MAX_PRODUCT_KERNEL_CAPABILITIES {
        return Err(failure(
            "RUNTIME_CAPABILITY_KERNEL_DESCRIPTOR_COUNT",
            KERNEL_SOURCE,
            "kernelCapabilities",
            format!(
                "Product Kernel capability descriptors are limited to {MAX_PRODUCT_KERNEL_CAPABILITIES}"
            ),
        ));
    }
    let mut descriptor_positions = BTreeMap::new();
    for (index, descriptor) in descriptors.iter().copied().enumerate() {
        validate_identity(
            descriptor.identity(),
            KERNEL_SOURCE,
            &format!("kernelCapabilities[{index}].identity"),
        )?;
        validate_metadata(
            descriptor.metadata(),
            KERNEL_SOURCE,
            &format!("kernelCapabilities[{index}]"),
        )?;
        if let Some(first_index) = descriptor_positions.insert(descriptor.identity(), index) {
            return Err(failure(
                "RUNTIME_CAPABILITY_DUPLICATE_KERNEL_DESCRIPTOR",
                KERNEL_SOURCE,
                format!("kernelCapabilities[{index}].identity"),
                format!(
                    "Product Kernel capability `{}` duplicates descriptor at kernelCapabilities[{first_index}]",
                    descriptor.identity(),
                ),
            ));
        }
    }
    Ok(descriptor_positions
        .into_iter()
        .enumerate()
        .map(|(stable_index, (identity, descriptor_index))| {
            (
                identity,
                KernelDescriptorLocation {
                    descriptor_index,
                    stable_index,
                },
            )
        })
        .collect())
}

#[derive(Debug, Clone, Copy)]
struct KernelDescriptorLocation {
    descriptor_index: usize,
    stable_index: usize,
}

fn validate_metadata(
    metadata: CapabilityMetadata,
    source: &str,
    path: &str,
) -> Result<(), ProductModelError> {
    if metadata.uses().is_empty() {
        return Err(failure(
            "RUNTIME_CAPABILITY_EMPTY_USES",
            source,
            format!("{path}.uses"),
            "a capability descriptor must declare at least one supported Compiled Composition use",
        ));
    }
    if metadata.budget().maximum_compact_json_payload_bytes() == 0 {
        return Err(failure(
            "RUNTIME_CAPABILITY_ZERO_PAYLOAD_BUDGET",
            source,
            format!("{path}.budget.maximumCompactJsonPayloadBytes"),
            "a capability descriptor compact JSON payload budget must be positive",
        ));
    }
    for (kind, values) in [
        ("reads", metadata.access().reads()),
        ("writes", metadata.access().writes()),
    ] {
        if values.len() > MAX_SCHEDULE_ACCESS_DECLARATIONS {
            return Err(failure(
                "RUNTIME_CAPABILITY_ACCESS_DECLARATION_COUNT",
                source,
                format!("{path}.access.{kind}"),
                format!(
                    "capability access declarations are limited to {MAX_SCHEDULE_ACCESS_DECLARATIONS} {kind}"
                ),
            ));
        }
        let mut declared = BTreeSet::new();
        for (index, value) in values.iter().enumerate() {
            validate_identity(value, source, &format!("{path}.access.{kind}[{index}]"))?;
            if !declared.insert(*value) {
                return Err(failure(
                    "RUNTIME_CAPABILITY_DUPLICATE_ACCESS_DECLARATION",
                    source,
                    format!("{path}.access.{kind}[{index}]"),
                    format!("capability descriptor declares `{value}` more than once in {kind}"),
                ));
            }
        }
    }
    let provenance = metadata.provenance();
    for (field, value) in [
        ("owner", provenance.owner()),
        ("source", provenance.source()),
        ("logicalPath", provenance.logical_path()),
    ] {
        if value.is_empty() || value.len() > MAX_CAPABILITY_PROVENANCE_BYTES {
            return Err(failure(
                "RUNTIME_CAPABILITY_PROVENANCE_BOUNDS",
                source,
                format!("{path}.provenance.{field}"),
                format!(
                    "capability provenance {field} must contain 1..={MAX_CAPABILITY_PROVENANCE_BYTES} UTF-8 bytes"
                ),
            ));
        }
    }
    if metadata.budget().maximum_compact_json_payload_bytes() > MAX_COMPILED_COMPOSITION_BYTES {
        return Err(failure(
            "RUNTIME_CAPABILITY_PAYLOAD_BUDGET_BOUNDS",
            source,
            format!("{path}.budget.maximumCompactJsonPayloadBytes"),
            format!(
                "capability compact JSON payload budget cannot exceed the {MAX_COMPILED_COMPOSITION_BYTES}-byte Compiled Composition limit"
            ),
        ));
    }
    if let CapabilityAvailability::Unavailable { reason } = metadata.availability() {
        if reason.is_empty() || reason.len() > MAX_CAPABILITY_PROVENANCE_BYTES {
            return Err(failure(
                "RUNTIME_CAPABILITY_UNAVAILABLE_REASON_BOUNDS",
                source,
                format!("{path}.availability.reason"),
                format!(
                    "an unavailable capability reason must contain 1..={MAX_CAPABILITY_PROVENANCE_BYTES} UTF-8 bytes"
                ),
            ));
        }
    }
    Ok(())
}

fn resolve_target(
    binding: &AdmittedCapabilityBinding,
    kernel_capabilities: &[ProductKernelCapabilityDescriptor],
    kernel_indices: &BTreeMap<&'static str, KernelDescriptorLocation>,
) -> Result<(LinkedCapabilityTarget, CapabilityMetadata), ProductModelError> {
    let target_path = format!("capabilityBindings[{}].target", binding.index());
    if let Some(local) = binding.target().strip_prefix("engine.") {
        let descriptor = engine_capability_descriptors()
            .iter()
            .copied()
            .find(|descriptor| descriptor.target() == binding.target())
            .ok_or_else(|| {
                composition_failure(
                    "RUNTIME_CAPABILITY_UNKNOWN_ENGINE_TARGET",
                    target_path.clone(),
                    format!(
                        "Engine capability `{}` is not in the closed Engine catalog; regenerate authoring names or select a current target",
                        binding.target()
                    ),
                )
            })?;
        debug_assert_eq!(
            local,
            descriptor
                .target()
                .strip_prefix("engine.")
                .unwrap_or_default()
        );
        require_linkable(binding.target(), target_path, descriptor.metadata())?;
        return Ok((
            LinkedCapabilityTarget::Engine(descriptor.capability()),
            descriptor.metadata(),
        ));
    }
    if let Some(local) = binding.target().strip_prefix("kernel.") {
        let location = kernel_indices.get(local).copied().ok_or_else(|| {
            composition_failure(
                "RUNTIME_CAPABILITY_UNKNOWN_KERNEL_TARGET",
                target_path.clone(),
                format!(
                    "Product Kernel capability `{local}` is not present in the complete assembly descriptor slice",
                ),
            )
        })?;
        let descriptor = kernel_capabilities
            .get(location.descriptor_index)
            .copied()
            .ok_or_else(|| {
                composition_failure(
                    "RUNTIME_CAPABILITY_KERNEL_DESCRIPTOR_INDEX",
                    target_path.clone(),
                    format!(
                        "Product Kernel capability `{local}` had an invalid descriptor index {}",
                        location.descriptor_index,
                    ),
                )
            })?;
        require_linkable(binding.target(), target_path, descriptor.metadata())?;
        return Ok((
            LinkedCapabilityTarget::ProductKernel(ProductKernelCapabilityIndex(
                location.stable_index,
            )),
            descriptor.metadata(),
        ));
    }
    Err(composition_failure(
        "RUNTIME_CAPABILITY_TARGET_NAMESPACE",
        target_path,
        format!(
            "capability target `{}` must use the admitted engine. or kernel. namespace",
            binding.target()
        ),
    ))
}

fn require_linkable(
    target: &str,
    path: String,
    metadata: CapabilityMetadata,
) -> Result<(), ProductModelError> {
    if metadata.availability().is_linkable() {
        return Ok(());
    }
    let reason = metadata
        .availability()
        .reason()
        .unwrap_or("no reason was supplied");
    Err(composition_failure(
        "RUNTIME_CAPABILITY_UNAVAILABLE",
        path,
        format!(
            "capability `{target}` is unavailable: {reason} (owner `{}`, {} at `{}`)",
            metadata.provenance().owner(),
            metadata.provenance().source(),
            metadata.provenance().logical_path(),
        ),
    ))
}

fn validate_input_map_uses(
    admitted: &AdmittedProductComposition,
    bindings: &[LinkedCapabilityBinding],
) -> Result<(), ProductModelError> {
    for (index, descriptor) in admitted.intent_descriptors().iter().enumerate() {
        let binding = linked_reference(
            bindings,
            descriptor.capability(),
            &format!("intentDescriptors[{index}].capability"),
        )?;
        require_use(
            binding,
            CapabilityUse::InputMap,
            &format!("intentDescriptors[{index}].capability"),
        )?;
        require_payload_budget(
            binding,
            descriptor.payload(),
            &format!("intentDescriptors[{index}].payload"),
        )?;
    }
    Ok(())
}

fn validate_schedule_uses(
    admitted: &AdmittedProductComposition,
    bindings: &[LinkedCapabilityBinding],
) -> Result<(), ProductModelError> {
    for (index, entry) in admitted.schedule().iter().enumerate() {
        let binding = linked_reference(
            bindings,
            entry.capability(),
            &format!("schedule[{index}].capability"),
        )?;
        require_use(
            binding,
            CapabilityUse::Schedule,
            &format!("schedule[{index}].capability"),
        )?;
        require_access(binding, entry.reads(), entry.writes(), index)?;
        require_payload_budget(
            binding,
            entry.payload(),
            &format!("schedule[{index}].payload"),
        )?;
    }
    Ok(())
}

fn validate_timeline_uses(
    admitted: &AdmittedProductComposition,
    bindings: &[LinkedCapabilityBinding],
) -> Result<(), ProductModelError> {
    for (timeline_index, timeline) in admitted.timelines().iter().enumerate() {
        for (step_index, step) in timeline.steps().iter().enumerate() {
            let prefix = format!("timelines[{timeline_index}].steps[{step_index}]");
            let binding =
                linked_reference(bindings, step.capability(), &format!("{prefix}.capability"))?;
            require_use(
                binding,
                CapabilityUse::Timeline,
                &format!("{prefix}.capability"),
            )?;
            require_payload_budget(binding, step.payload(), &format!("{prefix}.payload"))?;
        }
    }
    Ok(())
}

fn linked_reference<'a>(
    bindings: &'a [LinkedCapabilityBinding],
    reference: &AdmittedCapabilityReference,
    path: &str,
) -> Result<&'a LinkedCapabilityBinding, ProductModelError> {
    bindings.get(reference.binding_index()).ok_or_else(|| {
        composition_failure(
            "RUNTIME_CAPABILITY_LINKED_INDEX",
            path,
            format!(
                "admitted capability `{}` refers to missing linked binding index {}",
                reference.id(),
                reference.binding_index(),
            ),
        )
    })
}

fn require_use(
    binding: &LinkedCapabilityBinding,
    usage: CapabilityUse,
    path: &str,
) -> Result<(), ProductModelError> {
    if binding.metadata().uses().contains(usage) {
        return Ok(());
    }
    let provenance = binding.metadata().provenance();
    Err(composition_failure(
        "RUNTIME_CAPABILITY_INCOMPATIBLE_USE",
        path,
        format!(
            "capability `{}` is a {} owned by `{}` and cannot be used in {}; its descriptor is {} at `{}`",
            binding.target(),
            binding.metadata().kind().as_str(),
            provenance.owner(),
            usage.as_str(),
            provenance.source(),
            provenance.logical_path(),
        ),
    ))
}

fn require_access(
    binding: &LinkedCapabilityBinding,
    reads: &[String],
    writes: &[String],
    schedule_index: usize,
) -> Result<(), ProductModelError> {
    let expected = binding.metadata().access();
    if reads
        .iter()
        .map(String::as_str)
        .eq(expected.reads().iter().copied())
    {
        if writes
            .iter()
            .map(String::as_str)
            .eq(expected.writes().iter().copied())
        {
            return Ok(());
        }
        return Err(access_failure(
            binding,
            schedule_index,
            "writes",
            expected.writes(),
        ));
    }
    Err(access_failure(
        binding,
        schedule_index,
        "reads",
        expected.reads(),
    ))
}

fn access_failure(
    binding: &LinkedCapabilityBinding,
    schedule_index: usize,
    field: &str,
    expected: &[&str],
) -> ProductModelError {
    composition_failure(
        "RUNTIME_CAPABILITY_ACCESS_MISMATCH",
        format!("schedule[{schedule_index}].{field}"),
        format!(
            "capability `{}` requires declared {field} {:?}; its closed descriptor is {} at `{}`",
            binding.target(),
            expected,
            binding.metadata().provenance().source(),
            binding.metadata().provenance().logical_path(),
        ),
    )
}

fn require_payload_budget(
    binding: &LinkedCapabilityBinding,
    payload: &Value,
    path: &str,
) -> Result<(), ProductModelError> {
    let payload_bytes = serde_json::to_vec(payload)
        .map_err(|error| {
            composition_failure("RUNTIME_CAPABILITY_PAYLOAD_ENCODE", path, error.to_string())
        })?
        .len();
    let maximum = binding
        .metadata()
        .budget()
        .maximum_compact_json_payload_bytes();
    if payload_bytes <= maximum {
        return Ok(());
    }
    Err(composition_failure(
        "RUNTIME_CAPABILITY_PAYLOAD_BUDGET",
        path,
        format!(
            "capability `{}` compact Rust JSON payload is {payload_bytes} bytes, exceeding its closed descriptor budget of {maximum} bytes",
            binding.target(),
        ),
    ))
}

fn composition_failure(
    code: &str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> ProductModelError {
    failure(code, COMPOSITION_SOURCE, path, message)
}
