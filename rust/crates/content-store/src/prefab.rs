use std::collections::{BTreeMap, BTreeSet};

use core_assets::{AssetId, AssetKind};
use core_ids::{PrefabId, PrefabInstanceId, PrefabPartId};

pub const PREFAB_REGISTRY_SCHEMA_VERSION: u32 = 1;
pub const PREFAB_DEFINITION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrefabTransform {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl PrefabTransform {
    pub const IDENTITY: Self = Self {
        translation: [0.0, 0.0, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0, 1.0, 1.0],
    };

    pub(crate) fn is_valid(self) -> bool {
        self.translation
            .into_iter()
            .chain(self.rotation)
            .chain(self.scale)
            .all(f32::is_finite)
            && self.scale.into_iter().all(|axis| axis != 0.0)
            && self
                .rotation
                .into_iter()
                .map(|axis| axis * axis)
                .sum::<f32>()
                > f32::EPSILON
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefabPartSource {
    Scene { asset: String },
    EntityDefinition { stable_id: String },
    VoxelObject { asset: String },
}

impl PrefabPartSource {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Scene { .. } => "scene",
            Self::EntityDefinition { .. } => "entityDefinition",
            Self::VoxelObject { .. } => "voxelObject",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrefabPart {
    pub id: PrefabPartId,
    pub namespace: String,
    pub display_name: String,
    pub parent: Option<PrefabPartId>,
    pub transform: PrefabTransform,
    pub source: PrefabPartSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefabPartRoleBinding {
    pub role: String,
    pub part: PrefabPartId,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrefabOverrideValue {
    Transform { transform: PrefabTransform },
    EntityDefinition { stable_id: String },
    Asset { asset: String },
    Material { asset: String },
    Activation { active: bool },
}

impl PrefabOverrideValue {
    pub const fn field(&self) -> &'static str {
        match self {
            Self::Transform { .. } => "transform",
            Self::EntityDefinition { .. } => "entityDefinition",
            Self::Asset { .. } => "asset",
            Self::Material { .. } => "material",
            Self::Activation { .. } => "activation",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrefabOverride {
    pub target_role: String,
    pub value: PrefabOverrideValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrefabVariantDelta {
    pub variant_id: String,
    pub base: PrefabId,
    pub removed_roles: Vec<String>,
    pub overrides: Vec<PrefabOverride>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrefabDefinition {
    pub id: PrefabId,
    pub schema_version: u32,
    pub display_name: String,
    pub parts: Vec<PrefabPart>,
    pub part_roles: Vec<PrefabPartRoleBinding>,
    pub variant: Option<PrefabVariantDelta>,
}

impl PrefabDefinition {
    fn canonicalize(&mut self) {
        self.parts.sort_by_key(|part| part.id.raw());
        self.part_roles
            .sort_by(|left, right| left.role.cmp(&right.role));
        if let Some(variant) = &mut self.variant {
            variant.removed_roles.sort();
            variant.overrides.sort_by(|left, right| {
                (left.target_role.as_str(), left.value.field())
                    .cmp(&(right.target_role.as_str(), right.value.field()))
            });
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrefabRegistry {
    pub schema_version: u32,
    pub definitions: Vec<PrefabDefinition>,
}

impl PrefabRegistry {
    pub fn canonical(&self) -> Self {
        let mut registry = self.clone();
        for definition in &mut registry.definitions {
            definition.canonicalize();
        }
        registry
            .definitions
            .sort_by_key(|definition| definition.id.raw());
        registry
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrefabInstanceRecord {
    pub instance: PrefabInstanceId,
    pub prefab: PrefabId,
    pub seed: u64,
    pub transform: PrefabTransform,
    pub overrides: Vec<PrefabOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefabPartReference {
    pub prefab: PrefabId,
    pub role: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrefabRegistryValidationContext {
    pub asset_ids: BTreeSet<String>,
    pub entity_definition_ids: BTreeSet<String>,
}

impl PrefabRegistryValidationContext {
    pub fn from_asset_ids(
        assets: impl IntoIterator<Item = AssetId>,
        entity_definition_ids: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            asset_ids: assets
                .into_iter()
                .map(|asset| asset.as_str().to_owned())
                .collect(),
            entity_definition_ids: entity_definition_ids.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrefabDiagnosticCode {
    UnsupportedRegistrySchema,
    UnsupportedDefinitionSchema,
    DuplicatePrefabId,
    MissingDisplayName,
    DuplicatePartId,
    InvalidPartNamespace,
    DuplicatePartNamespace,
    MissingParentPart,
    PartHierarchyCycle,
    InvalidPartTransform,
    UnknownAsset,
    AssetKindMismatch,
    UnknownEntityDefinition,
    InvalidPartRole,
    DuplicatePartRole,
    DanglingPartRole,
    MissingBasePrefab,
    InvalidVariantId,
    DuplicateVariantId,
    VariantCycle,
    VariantDepthExceeded,
    VariantDefinesParts,
    UnknownRemovedRole,
    DuplicateRemovedRole,
    UnsafePartRemoval,
    InvalidOverrideTarget,
    DuplicateOverride,
    InvalidOverrideValue,
    DeletedRoleReferenced,
}

impl PrefabDiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedRegistrySchema => "unsupportedRegistrySchema",
            Self::UnsupportedDefinitionSchema => "unsupportedDefinitionSchema",
            Self::DuplicatePrefabId => "duplicatePrefabId",
            Self::MissingDisplayName => "missingDisplayName",
            Self::DuplicatePartId => "duplicatePartId",
            Self::InvalidPartNamespace => "invalidPartNamespace",
            Self::DuplicatePartNamespace => "duplicatePartNamespace",
            Self::MissingParentPart => "missingParentPart",
            Self::PartHierarchyCycle => "partHierarchyCycle",
            Self::InvalidPartTransform => "invalidPartTransform",
            Self::UnknownAsset => "unknownAsset",
            Self::AssetKindMismatch => "assetKindMismatch",
            Self::UnknownEntityDefinition => "unknownEntityDefinition",
            Self::InvalidPartRole => "invalidPartRole",
            Self::DuplicatePartRole => "duplicatePartRole",
            Self::DanglingPartRole => "danglingPartRole",
            Self::MissingBasePrefab => "missingBasePrefab",
            Self::InvalidVariantId => "invalidVariantId",
            Self::DuplicateVariantId => "duplicateVariantId",
            Self::VariantCycle => "variantCycle",
            Self::VariantDepthExceeded => "variantDepthExceeded",
            Self::VariantDefinesParts => "variantDefinesParts",
            Self::UnknownRemovedRole => "unknownRemovedRole",
            Self::DuplicateRemovedRole => "duplicateRemovedRole",
            Self::UnsafePartRemoval => "unsafePartRemoval",
            Self::InvalidOverrideTarget => "invalidOverrideTarget",
            Self::DuplicateOverride => "duplicateOverride",
            Self::InvalidOverrideValue => "invalidOverrideValue",
            Self::DeletedRoleReferenced => "deletedRoleReferenced",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefabDiagnostic {
    pub code: PrefabDiagnosticCode,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrefabValidationReport {
    pub diagnostics: Vec<PrefabDiagnostic>,
}

impl PrefabValidationReport {
    pub fn is_valid(&self) -> bool {
        self.diagnostics.is_empty()
    }

    fn push(
        &mut self,
        code: PrefabDiagnosticCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(PrefabDiagnostic {
            code,
            path: path.into(),
            message: message.into(),
        });
    }

    fn canonicalize(&mut self) {
        self.diagnostics.sort_by(|left, right| {
            (
                left.path.as_str(),
                left.code.as_str(),
                left.message.as_str(),
            )
                .cmp(&(
                    right.path.as_str(),
                    right.code.as_str(),
                    right.message.as_str(),
                ))
        });
        self.diagnostics.dedup();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedPrefabRegistry {
    registry: PrefabRegistry,
    validation_context: PrefabRegistryValidationContext,
}

impl ValidatedPrefabRegistry {
    pub fn new(
        registry: PrefabRegistry,
        context: &PrefabRegistryValidationContext,
    ) -> Result<Self, PrefabValidationReport> {
        let report = validate_prefab_registry(&registry, context);
        if report.is_valid() {
            Ok(Self {
                registry: registry.canonical(),
                validation_context: context.clone(),
            })
        } else {
            Err(report)
        }
    }

    pub fn as_registry(&self) -> &PrefabRegistry {
        &self.registry
    }

    pub fn validation_context(&self) -> &PrefabRegistryValidationContext {
        &self.validation_context
    }

    pub fn into_registry(self) -> PrefabRegistry {
        self.registry
    }
}

pub fn validate_prefab_registry(
    registry: &PrefabRegistry,
    context: &PrefabRegistryValidationContext,
) -> PrefabValidationReport {
    let mut report = PrefabValidationReport::default();
    if registry.schema_version != PREFAB_REGISTRY_SCHEMA_VERSION {
        report.push(
            PrefabDiagnosticCode::UnsupportedRegistrySchema,
            "schemaVersion",
            format!("expected schema {PREFAB_REGISTRY_SCHEMA_VERSION}"),
        );
    }
    let mut definitions = BTreeMap::new();
    for (index, definition) in registry.definitions.iter().enumerate() {
        let path = format!("definitions[{index}]");
        if definitions.insert(definition.id, definition).is_some() {
            report.push(
                PrefabDiagnosticCode::DuplicatePrefabId,
                format!("{path}.id"),
                format!("duplicate prefab id {}", definition.id.raw()),
            );
        }
        validate_definition(definition, context, &path, &mut report);
    }
    validate_variants(&definitions, context, &mut report);
    report.canonicalize();
    report
}

fn validate_definition(
    definition: &PrefabDefinition,
    context: &PrefabRegistryValidationContext,
    path: &str,
    report: &mut PrefabValidationReport,
) {
    if definition.schema_version != PREFAB_DEFINITION_SCHEMA_VERSION {
        report.push(
            PrefabDiagnosticCode::UnsupportedDefinitionSchema,
            format!("{path}.schemaVersion"),
            format!("expected schema {PREFAB_DEFINITION_SCHEMA_VERSION}"),
        );
    }
    if definition.display_name.trim().is_empty() {
        report.push(
            PrefabDiagnosticCode::MissingDisplayName,
            format!("{path}.displayName"),
            "display name must not be blank",
        );
    }
    if definition.variant.is_some()
        && (!definition.parts.is_empty() || !definition.part_roles.is_empty())
    {
        report.push(
            PrefabDiagnosticCode::VariantDefinesParts,
            path,
            "variants are deltas and may not define parts or roles",
        );
    }
    let mut parts = BTreeMap::new();
    let mut namespaces = BTreeSet::new();
    for (index, part) in definition.parts.iter().enumerate() {
        let part_path = format!("{path}.parts[{index}]");
        if parts.insert(part.id, part).is_some() {
            report.push(
                PrefabDiagnosticCode::DuplicatePartId,
                format!("{part_path}.id"),
                format!("duplicate part id {}", part.id.raw()),
            );
        }
        if !is_scoped_key(&part.namespace) {
            report.push(
                PrefabDiagnosticCode::InvalidPartNamespace,
                format!("{part_path}.namespace"),
                "namespace must be slash-scoped lowercase kebab-case",
            );
        } else if !namespaces.insert(part.namespace.as_str()) {
            report.push(
                PrefabDiagnosticCode::DuplicatePartNamespace,
                format!("{part_path}.namespace"),
                format!("duplicate namespace {}", part.namespace),
            );
        }
        if !part.transform.is_valid() {
            report.push(
                PrefabDiagnosticCode::InvalidPartTransform,
                format!("{part_path}.transform"),
                "transform must be finite, non-degenerate, and have non-zero scale",
            );
        }
        validate_source(
            &part.source,
            context,
            &format!("{part_path}.source"),
            report,
        );
    }
    for (index, part) in definition.parts.iter().enumerate() {
        if part
            .parent
            .is_some_and(|parent| !parts.contains_key(&parent))
        {
            report.push(
                PrefabDiagnosticCode::MissingParentPart,
                format!("{path}.parts[{index}].parent"),
                "parent is not a part of this prefab",
            );
        }
    }
    validate_part_cycles(&parts, path, report);
    let mut roles = BTreeSet::new();
    for (index, binding) in definition.part_roles.iter().enumerate() {
        let role_path = format!("{path}.partRoles[{index}]");
        if !is_scoped_key(&binding.role) {
            report.push(
                PrefabDiagnosticCode::InvalidPartRole,
                format!("{role_path}.role"),
                "role must be slash-scoped lowercase kebab-case",
            );
        }
        if !roles.insert(binding.role.as_str()) {
            report.push(
                PrefabDiagnosticCode::DuplicatePartRole,
                format!("{role_path}.role"),
                format!("duplicate role {}", binding.role),
            );
        }
        if !parts.contains_key(&binding.part) {
            report.push(
                PrefabDiagnosticCode::DanglingPartRole,
                format!("{role_path}.part"),
                format!("unknown part {}", binding.part.raw()),
            );
        }
    }
}

fn validate_source(
    source: &PrefabPartSource,
    context: &PrefabRegistryValidationContext,
    path: &str,
    report: &mut PrefabValidationReport,
) {
    match source {
        PrefabPartSource::Scene { asset } => {
            validate_asset(asset, AssetKind::Scene, context, path, report)
        }
        PrefabPartSource::VoxelObject { asset } => {
            validate_asset(asset, AssetKind::VoxelObject, context, path, report)
        }
        PrefabPartSource::EntityDefinition { stable_id } => {
            if !context.entity_definition_ids.contains(stable_id) {
                report.push(
                    PrefabDiagnosticCode::UnknownEntityDefinition,
                    path,
                    format!("unknown entity definition {stable_id}"),
                );
            }
        }
    }
}

fn validate_asset(
    asset: &str,
    expected: AssetKind,
    context: &PrefabRegistryValidationContext,
    path: &str,
    report: &mut PrefabValidationReport,
) {
    let Ok(id) = AssetId::parse(asset) else {
        report.push(
            PrefabDiagnosticCode::AssetKindMismatch,
            path,
            format!("malformed {expected} asset id {asset}"),
        );
        return;
    };
    if id.kind() != expected {
        report.push(
            PrefabDiagnosticCode::AssetKindMismatch,
            path,
            format!("expected {expected}, found {}", id.kind()),
        );
    } else if !context.asset_ids.contains(asset) {
        report.push(
            PrefabDiagnosticCode::UnknownAsset,
            path,
            format!("unknown asset {asset}"),
        );
    }
}

fn validate_part_cycles(
    parts: &BTreeMap<PrefabPartId, &PrefabPart>,
    path: &str,
    report: &mut PrefabValidationReport,
) {
    for start in parts.keys() {
        let mut seen = BTreeSet::new();
        let mut cursor = Some(*start);
        while let Some(id) = cursor {
            if !seen.insert(id) {
                report.push(
                    PrefabDiagnosticCode::PartHierarchyCycle,
                    format!("{path}.parts"),
                    format!("cycle includes part {}", id.raw()),
                );
                break;
            }
            cursor = parts.get(&id).and_then(|part| part.parent);
        }
    }
}

fn validate_variants(
    definitions: &BTreeMap<PrefabId, &PrefabDefinition>,
    context: &PrefabRegistryValidationContext,
    report: &mut PrefabValidationReport,
) {
    let mut variant_ids_by_base = BTreeMap::<PrefabId, BTreeSet<&str>>::new();
    for definition in definitions.values() {
        let Some(variant) = &definition.variant else {
            continue;
        };
        let path = format!("prefab[{}].variant", definition.id.raw());
        if !is_scoped_key(&variant.variant_id) {
            report.push(
                PrefabDiagnosticCode::InvalidVariantId,
                format!("{path}.variantId"),
                "variant id must be slash-scoped lowercase kebab-case",
            );
        } else if !variant_ids_by_base
            .entry(variant.base)
            .or_default()
            .insert(variant.variant_id.as_str())
        {
            report.push(
                PrefabDiagnosticCode::DuplicateVariantId,
                format!("{path}.variantId"),
                format!("duplicate variant id {} for this base", variant.variant_id),
            );
        }
        let Some(base) = definitions.get(&variant.base).copied() else {
            report.push(
                PrefabDiagnosticCode::MissingBasePrefab,
                format!("{path}.base"),
                format!("unknown base prefab {}", variant.base.raw()),
            );
            continue;
        };
        if base.id == definition.id {
            report.push(
                PrefabDiagnosticCode::VariantCycle,
                &path,
                "variant may not base itself",
            );
            continue;
        }
        if base.variant.is_some() {
            let code = if variant_chain_reaches(base, definition.id, definitions) {
                PrefabDiagnosticCode::VariantCycle
            } else {
                PrefabDiagnosticCode::VariantDepthExceeded
            };
            report.push(code, &path, "only one variant level is supported");
            continue;
        }
        validate_variant_delta(variant, base, context, &path, report);
    }
}

fn variant_chain_reaches(
    start: &PrefabDefinition,
    target: PrefabId,
    definitions: &BTreeMap<PrefabId, &PrefabDefinition>,
) -> bool {
    let mut cursor = Some(start);
    let mut seen = BTreeSet::new();
    while let Some(definition) = cursor {
        if definition.id == target || !seen.insert(definition.id) {
            return true;
        }
        cursor = definition
            .variant
            .as_ref()
            .and_then(|variant| definitions.get(&variant.base).copied());
    }
    false
}

fn validate_variant_delta(
    variant: &PrefabVariantDelta,
    base: &PrefabDefinition,
    context: &PrefabRegistryValidationContext,
    path: &str,
    report: &mut PrefabValidationReport,
) {
    let roles: BTreeMap<&str, PrefabPartId> = base
        .part_roles
        .iter()
        .map(|binding| (binding.role.as_str(), binding.part))
        .collect();
    let parts: BTreeMap<PrefabPartId, &PrefabPart> =
        base.parts.iter().map(|part| (part.id, part)).collect();
    let mut removed = BTreeSet::new();
    for (index, role) in variant.removed_roles.iter().enumerate() {
        if !removed.insert(role.as_str()) {
            report.push(
                PrefabDiagnosticCode::DuplicateRemovedRole,
                format!("{path}.removedRoles[{index}]"),
                format!("role {role} is removed more than once"),
            );
        }
        if !roles.contains_key(role.as_str()) {
            report.push(
                PrefabDiagnosticCode::UnknownRemovedRole,
                format!("{path}.removedRoles[{index}]"),
                format!("unknown base role {role}"),
            );
        }
    }
    let removed_parts: BTreeSet<_> = removed
        .iter()
        .filter_map(|role| roles.get(role).copied())
        .collect();
    for removed_part in &removed_parts {
        for binding in &base.part_roles {
            if binding.part == *removed_part && !removed.contains(binding.role.as_str()) {
                report.push(
                    PrefabDiagnosticCode::UnsafePartRemoval,
                    format!("{path}.removedRoles"),
                    format!("retained role {} aliases a removed part", binding.role),
                );
            }
        }
        if parts
            .values()
            .any(|part| part.parent == Some(*removed_part) && !removed_parts.contains(&part.id))
        {
            report.push(
                PrefabDiagnosticCode::UnsafePartRemoval,
                format!("{path}.removedRoles"),
                format!(
                    "removing part {} leaves a retained child",
                    removed_part.raw()
                ),
            );
        }
    }

    let mut targets = BTreeSet::new();
    for (index, item) in variant.overrides.iter().enumerate() {
        let item_path = format!("{path}.overrides[{index}]");
        let Some(part_id) = roles.get(item.target_role.as_str()).copied() else {
            report.push(
                PrefabDiagnosticCode::InvalidOverrideTarget,
                format!("{item_path}.targetRole"),
                format!("unknown base role {}", item.target_role),
            );
            continue;
        };
        if removed_parts.contains(&part_id) {
            report.push(
                PrefabDiagnosticCode::DeletedRoleReferenced,
                &item_path,
                format!("override resolves to removed part {}", part_id.raw()),
            );
        }
        if !targets.insert((item.target_role.as_str(), item.value.field())) {
            report.push(
                PrefabDiagnosticCode::DuplicateOverride,
                &item_path,
                format!("duplicate {} override", item.value.field()),
            );
        }
        let Some(part) = parts.get(&part_id) else {
            continue;
        };
        validate_override_value(item, part, context, &item_path, report);
    }
}

fn validate_override_value(
    item: &PrefabOverride,
    part: &PrefabPart,
    context: &PrefabRegistryValidationContext,
    path: &str,
    report: &mut PrefabValidationReport,
) {
    match &item.value {
        PrefabOverrideValue::Transform { transform } if !transform.is_valid() => report.push(
            PrefabDiagnosticCode::InvalidOverrideValue,
            path,
            "override transform is invalid",
        ),
        PrefabOverrideValue::EntityDefinition { stable_id } => {
            if !matches!(part.source, PrefabPartSource::EntityDefinition { .. })
                || !context.entity_definition_ids.contains(stable_id)
            {
                report.push(
                    PrefabDiagnosticCode::InvalidOverrideValue,
                    path,
                    "entity-definition override requires a matching part and known id",
                );
            }
        }
        PrefabOverrideValue::Asset { asset } => match part.source {
            PrefabPartSource::Scene { .. } => {
                validate_asset(asset, AssetKind::Scene, context, path, report)
            }
            PrefabPartSource::VoxelObject { .. } => {
                validate_asset(asset, AssetKind::VoxelObject, context, path, report)
            }
            PrefabPartSource::EntityDefinition { .. } => report.push(
                PrefabDiagnosticCode::InvalidOverrideValue,
                path,
                "asset override cannot target an entity-definition part",
            ),
        },
        PrefabOverrideValue::Material { asset } => {
            if matches!(part.source, PrefabPartSource::EntityDefinition { .. }) {
                report.push(
                    PrefabDiagnosticCode::InvalidOverrideValue,
                    path,
                    "material override requires a scene or voxel part",
                );
            } else {
                validate_asset(asset, AssetKind::Material, context, path, report);
            }
        }
        PrefabOverrideValue::Activation { .. } | PrefabOverrideValue::Transform { .. } => {}
    }
}

fn is_scoped_key(value: &str) -> bool {
    !value.is_empty() && value.split('/').all(is_kebab_segment)
}

fn is_kebab_segment(segment: &str) -> bool {
    !segment.is_empty()
        && !segment.starts_with('-')
        && !segment.ends_with('-')
        && !segment.contains("--")
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
