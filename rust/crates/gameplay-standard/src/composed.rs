//! Closed typed product leaves nested inside the standard exact expression grammar.
//!
//! The JSON package is only a strict transport boundary.  Admission decodes it immediately
//! through a concrete downstream codec into a typed leaf, compiles every leaf to `ExactExpr`,
//! and then delegates all arithmetic and quotas to the one `ExactEvaluator`.

use std::collections::{BTreeMap, BTreeSet};

use gameplay_mechanics::MechanicsScalar;
use gameplay_rules::{
    admit_rule_package, AdmittedRulePackage, RulePackageSchemaVersion, RuleSourceId, RuleSubjectId,
};
use serde_json::Value;

pub use crate::composed_error::{
    ComposedExactDefinitionError, ComposedExactError, ComposedExactProductContext,
};
use crate::composed_wire::{
    bounded_path, child_path, decode_expr, decode_roles, decode_schema, encode_definition, fields,
    integer, malformed, preflight_wire_tree, required, string, validate_composed_wire_structure,
    validate_correlation, WirePreflight,
};

use crate::{
    CapabilityRoleId, ExactComparison, ExactEvaluationError, ExactEvaluator, ExactExpr,
    ExactExprRequirements, ExactInputReference, RoleRequirement, StandardDefinitionIdentity,
    StandardExtensionSchema, StandardPackageContext,
};

pub const COMPOSED_EXACT_FAMILY_ID: &str = "composedExact";
pub const COMPOSED_EXACT_SEMANTICS_VERSION: u32 = 1;

/// A product-leaf identity, deliberately distinct from an exact input identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComposedExactLeafKindId(crate::InputId);
impl ComposedExactLeafKindId {
    pub fn parse(value: &str) -> Result<Self, crate::RoleRequirementError> {
        crate::InputId::parse(value).map(Self)
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// A typed downstream leaf with its package-visible provenance. `Leaf` never becomes JSON after
/// codec admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedExactProductLeaf<Leaf> {
    kind: ComposedExactLeafKindId,
    subject: RuleSubjectId,
    source: RuleSourceId,
    value: Leaf,
}
impl<Leaf> ComposedExactProductLeaf<Leaf> {
    pub fn new(
        kind: ComposedExactLeafKindId,
        subject: RuleSubjectId,
        source: RuleSourceId,
        value: Leaf,
    ) -> Self {
        Self {
            kind,
            subject,
            source,
            value,
        }
    }
    pub fn kind(&self) -> &ComposedExactLeafKindId {
        &self.kind
    }
    pub fn subject(&self) -> &RuleSubjectId {
        &self.subject
    }
    pub fn source(&self) -> &RuleSourceId {
        &self.source
    }
    pub fn value(&self) -> &Leaf {
        &self.value
    }
    pub fn into_value(self) -> Leaf {
        self.value
    }
}

/// The standard exact grammar plus one explicitly typed product-leaf arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposedExactExpr<Leaf> {
    Literal(MechanicsScalar),
    Input(ExactInputReference),
    Add(Box<Self>, Box<Self>),
    Subtract(Box<Self>, Box<Self>),
    Multiply(Box<Self>, Box<Self>),
    FloorDivide(Box<Self>, Box<Self>),
    TruncatingDivide(Box<Self>, Box<Self>),
    Min(Vec<Self>),
    Max(Vec<Self>),
    Product(ComposedExactProductLeaf<Leaf>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposedExactComparison<Leaf> {
    Equal(ComposedExactExpr<Leaf>, ComposedExactExpr<Leaf>),
    LessThan(ComposedExactExpr<Leaf>, ComposedExactExpr<Leaf>),
    LessOrEqual(ComposedExactExpr<Leaf>, ComposedExactExpr<Leaf>),
    GreaterThan(ComposedExactExpr<Leaf>, ComposedExactExpr<Leaf>),
    GreaterOrEqual(ComposedExactExpr<Leaf>, ComposedExactExpr<Leaf>),
}

/// The leaf compiler is deliberately a concrete downstream type, not a registry or callback
/// table. Its schema version is the product compiler version bound into the canonical package.
pub trait ComposedExactLeafCodec {
    type Leaf: Clone + PartialEq + Eq;
    type Error: std::error::Error + 'static;

    fn schema() -> StandardExtensionSchema;
    /// Strictly decode a product-owned JSON payload at the artifact boundary.
    fn decode_leaf(
        kind: &ComposedExactLeafKindId,
        payload: &Value,
    ) -> Result<Self::Leaf, Self::Error>;
    /// Canonically encode an already typed leaf only while authoring/admitting a package.
    fn encode_leaf(kind: &ComposedExactLeafKindId, leaf: &Self::Leaf)
        -> Result<Value, Self::Error>;
    /// Produce the product leaf's exact result and its complete declared requirement set.
    fn compile_leaf(leaf: &Self::Leaf) -> Result<CompiledComposedExactLeaf, Self::Error>;
}

/// A product compiler's typed evidence. The Engine verifies this exact requirement declaration
/// against the expression before accepting it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledComposedExactLeaf {
    expression: ExactExpr,
    requirements: ExactExprRequirements,
    capabilities: Vec<RoleRequirement>,
}

/// Typed evidence retained for each admitted product leaf; freshness and transaction guards stay
/// product-owned, while this records the exact schema/provenance that was compiled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedExactLeafEvidence {
    schema: StandardExtensionSchema,
    kind: ComposedExactLeafKindId,
    subject: RuleSubjectId,
    source: RuleSourceId,
}
impl ComposedExactLeafEvidence {
    pub fn schema(&self) -> &StandardExtensionSchema {
        &self.schema
    }
    pub fn kind(&self) -> &ComposedExactLeafKindId {
        &self.kind
    }
    pub fn subject(&self) -> &RuleSubjectId {
        &self.subject
    }
    pub fn source(&self) -> &RuleSourceId {
        &self.source
    }
}

/// A compiled comparison retains the standard predicate plus the complete typed leaf evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledComposedExactComparison {
    comparison: ExactComparison,
    requirements: ExactExprRequirements,
    product_capabilities: Vec<RoleRequirement>,
    leaves: Vec<ComposedExactLeafEvidence>,
}
impl CompiledComposedExactComparison {
    pub fn comparison(&self) -> &ExactComparison {
        &self.comparison
    }
    pub fn requirements(&self) -> &ExactExprRequirements {
        &self.requirements
    }
    pub fn product_capabilities(&self) -> &[RoleRequirement] {
        &self.product_capabilities
    }
    pub fn leaves(&self) -> &[ComposedExactLeafEvidence] {
        &self.leaves
    }
}
impl CompiledComposedExactLeaf {
    pub fn new(
        expression: ExactExpr,
        requirements: ExactExprRequirements,
        capabilities: Vec<RoleRequirement>,
    ) -> Self {
        Self {
            expression,
            requirements,
            capabilities,
        }
    }
    pub fn expression(&self) -> &ExactExpr {
        &self.expression
    }
    pub fn requirements(&self) -> &ExactExprRequirements {
        &self.requirements
    }
    pub fn capabilities(&self) -> &[RoleRequirement] {
        &self.capabilities
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedExactDefinition<Leaf> {
    schema: StandardExtensionSchema,
    subject: RuleSubjectId,
    source: RuleSourceId,
    expression: ComposedExactExpr<Leaf>,
    roles: Vec<RoleRequirement>,
}
impl<Leaf> ComposedExactDefinition<Leaf> {
    pub fn new(
        schema: StandardExtensionSchema,
        subject: RuleSubjectId,
        source: RuleSourceId,
        expression: ComposedExactExpr<Leaf>,
        roles: Vec<RoleRequirement>,
    ) -> Result<Self, ComposedExactDefinitionError> {
        validate_composed_wire_structure(&expression, crate::ExactExprLimits::default())?;
        let roles = canonicalize_roles(roles)?;
        Ok(Self {
            schema,
            subject,
            source,
            expression,
            roles,
        })
    }
    pub fn schema(&self) -> &StandardExtensionSchema {
        &self.schema
    }
    pub fn subject(&self) -> &RuleSubjectId {
        &self.subject
    }
    pub fn source(&self) -> &RuleSourceId {
        &self.source
    }
    pub fn expression(&self) -> &ComposedExactExpr<Leaf> {
        &self.expression
    }
    pub fn roles(&self) -> &[RoleRequirement] {
        &self.roles
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedComposedExactDefinition<Leaf> {
    package: AdmittedRulePackage,
    definition: ComposedExactDefinition<Leaf>,
    compiled: ExactExpr,
    requirements: ExactExprRequirements,
    product_capabilities: Vec<RoleRequirement>,
    identity: StandardDefinitionIdentity,
}
impl<Leaf> AdmittedComposedExactDefinition<Leaf> {
    pub fn package(&self) -> &AdmittedRulePackage {
        &self.package
    }
    pub fn definition(&self) -> &ComposedExactDefinition<Leaf> {
        &self.definition
    }
    pub fn compiled(&self) -> &ExactExpr {
        &self.compiled
    }
    pub fn requirements(&self) -> &ExactExprRequirements {
        &self.requirements
    }
    pub fn product_capabilities(&self) -> &[RoleRequirement] {
        &self.product_capabilities
    }
    pub fn identity(&self) -> &StandardDefinitionIdentity {
        &self.identity
    }
    pub fn evaluate(
        &self,
        inputs: &crate::ExactInputBundle,
    ) -> Result<MechanicsScalar, ExactEvaluationError> {
        ExactEvaluator::evaluate(&self.compiled, inputs, crate::ExactExprLimits::default())
    }
}

pub fn admit_composed_exact_definition<C: ComposedExactLeafCodec>(
    context: &StandardPackageContext,
    definition: ComposedExactDefinition<C::Leaf>,
) -> Result<AdmittedComposedExactDefinition<C::Leaf>, ComposedExactError<C::Error>> {
    let codec_schema = C::schema();
    if definition.schema() != &codec_schema {
        return Err(ComposedExactError::SchemaMismatch {
            expected: codec_schema,
            actual: definition.schema().clone(),
        });
    }
    let payload = encode_definition::<C>(&definition)?;
    let package =
        admit_rule_package(context.candidate(payload)).map_err(ComposedExactError::Package)?;
    let admitted = compile_composed_exact_package::<C>(&package)?;
    if admitted.definition != definition {
        return Err(ComposedExactError::NonConvergentPayload);
    }
    Ok(admitted)
}

/// Rehydrates a canonical TypeScript-authored package directly into typed leaves and expands it
/// before any exact evaluation. No public decoded object retains the transport JSON.
pub fn compile_composed_exact_package<C: ComposedExactLeafCodec>(
    package: &AdmittedRulePackage,
) -> Result<AdmittedComposedExactDefinition<C::Leaf>, ComposedExactError<C::Error>> {
    if package.schema_version() != RulePackageSchemaVersion::IntegerOnlyV1 {
        return Err(ComposedExactError::Wire(
            ComposedExactDefinitionError::WrongSchema {
                expected: 1,
                actual: package.schema_version().get(),
            },
        ));
    }
    let root = package
        .payload()
        .as_object()
        .ok_or_else(|| malformed("payload", "must be an object"))?;
    fields(
        root,
        &[
            "family",
            "semanticsVersion",
            "subject",
            "source",
            "roles",
            "extension",
            "tree",
        ],
        "payload",
    )?;
    if string(root, "family", "payload")? != COMPOSED_EXACT_FAMILY_ID {
        return Err(ComposedExactError::Wire(
            ComposedExactDefinitionError::WrongFamily,
        ));
    }
    if integer(root, "semanticsVersion", "payload")? != u64::from(COMPOSED_EXACT_SEMANTICS_VERSION)
    {
        return Err(ComposedExactError::Wire(
            ComposedExactDefinitionError::UnsupportedSemanticsVersion,
        ));
    }
    let subject = RuleSubjectId::parse(string(root, "subject", "payload")?)
        .map_err(ComposedExactError::Package)?;
    let source = RuleSourceId::parse(string(root, "source", "payload")?)
        .map_err(ComposedExactError::Package)?;
    validate_correlation(package, &subject, &source)?;
    let schema = decode_schema(required(root, "extension", "payload")?)?;
    let codec_schema = C::schema();
    if schema != codec_schema {
        return Err(ComposedExactError::SchemaMismatch {
            expected: codec_schema,
            actual: schema,
        });
    }
    let roles = decode_roles(required(root, "roles", "payload")?)?;
    // Reject malformed, over-budget raw transport before a product decoder can observe it.
    let mut preflight = WirePreflight::default();
    preflight_wire_tree(
        required(root, "tree", "payload")?,
        "payload.tree",
        package,
        1,
        &mut preflight,
        &roles,
    )?;
    let expression = decode_expr::<C>(required(root, "tree", "payload")?, "payload.tree", package)?;
    let definition =
        ComposedExactDefinition::new(C::schema(), subject.clone(), source, expression, roles)?;
    let (compiled, requirements, product_capabilities) =
        compile_expression::<C>(definition.expression(), "payload.tree")?;
    validate_roles(&requirements, definition.roles())?;
    validate_product_capabilities(&product_capabilities, definition.roles())?;
    Ok(AdmittedComposedExactDefinition {
        identity: StandardDefinitionIdentity::new(
            package.fingerprint().clone(),
            subject,
            COMPOSED_EXACT_FAMILY_ID,
            COMPOSED_EXACT_SEMANTICS_VERSION,
        ),
        package: package.clone(),
        definition,
        compiled,
        requirements,
        product_capabilities,
    })
}

pub fn compile_composed_exact_comparison<C: ComposedExactLeafCodec>(
    comparison: &ComposedExactComparison<C::Leaf>,
) -> Result<CompiledComposedExactComparison, ComposedExactError<C::Error>> {
    let map = |left: &ComposedExactExpr<C::Leaf>,
               right: &ComposedExactExpr<C::Leaf>|
     -> Result<ComparisonCompileParts, ComposedExactError<C::Error>> {
        let (compiled_left, _, mut capabilities) =
            compile_expression::<C>(left, "comparison.left")?;
        let (compiled_right, _, more) = compile_expression::<C>(right, "comparison.right")?;
        capabilities.extend(more);
        let candidate = ExactComparison::Equal(compiled_left.clone(), compiled_right.clone());
        ExactEvaluator::validate_comparison_structure(
            &candidate,
            crate::ExactExprLimits::default(),
        )
        .map_err(ComposedExactError::Standard)?;
        let requirements = ExactExprRequirements::inspect_comparison(&candidate)
            .map_err(ComposedExactError::Standard)?;
        let capabilities = canonicalize_roles(capabilities).map_err(ComposedExactError::Wire)?;
        let mut leaves = Vec::new();
        collect_leaf_evidence(left, &C::schema(), &mut leaves);
        collect_leaf_evidence(right, &C::schema(), &mut leaves);
        Ok(ComparisonCompileParts {
            left: compiled_left,
            right: compiled_right,
            requirements,
            capabilities,
            leaves,
        })
    };
    let (requirements, product_capabilities, leaves, comparison) = match comparison {
        ComposedExactComparison::Equal(a, b) => {
            let parts = map(a, b)?;
            (
                parts.requirements,
                parts.capabilities,
                parts.leaves,
                ExactComparison::Equal(parts.left, parts.right),
            )
        }
        ComposedExactComparison::LessThan(a, b) => {
            let parts = map(a, b)?;
            (
                parts.requirements,
                parts.capabilities,
                parts.leaves,
                ExactComparison::LessThan(parts.left, parts.right),
            )
        }
        ComposedExactComparison::LessOrEqual(a, b) => {
            let parts = map(a, b)?;
            (
                parts.requirements,
                parts.capabilities,
                parts.leaves,
                ExactComparison::LessOrEqual(parts.left, parts.right),
            )
        }
        ComposedExactComparison::GreaterThan(a, b) => {
            let parts = map(a, b)?;
            (
                parts.requirements,
                parts.capabilities,
                parts.leaves,
                ExactComparison::GreaterThan(parts.left, parts.right),
            )
        }
        ComposedExactComparison::GreaterOrEqual(a, b) => {
            let parts = map(a, b)?;
            (
                parts.requirements,
                parts.capabilities,
                parts.leaves,
                ExactComparison::GreaterOrEqual(parts.left, parts.right),
            )
        }
    };
    Ok(CompiledComposedExactComparison {
        comparison,
        requirements,
        product_capabilities,
        leaves,
    })
}
struct ComparisonCompileParts {
    left: ExactExpr,
    right: ExactExpr,
    requirements: ExactExprRequirements,
    capabilities: Vec<RoleRequirement>,
    leaves: Vec<ComposedExactLeafEvidence>,
}
fn collect_leaf_evidence<Leaf>(
    expr: &ComposedExactExpr<Leaf>,
    schema: &StandardExtensionSchema,
    into: &mut Vec<ComposedExactLeafEvidence>,
) {
    match expr {
        ComposedExactExpr::Product(leaf) => into.push(ComposedExactLeafEvidence {
            schema: schema.clone(),
            kind: leaf.kind().clone(),
            subject: leaf.subject().clone(),
            source: leaf.source().clone(),
        }),
        ComposedExactExpr::Add(left, right)
        | ComposedExactExpr::Subtract(left, right)
        | ComposedExactExpr::Multiply(left, right)
        | ComposedExactExpr::FloorDivide(left, right)
        | ComposedExactExpr::TruncatingDivide(left, right) => {
            collect_leaf_evidence(left, schema, into);
            collect_leaf_evidence(right, schema, into);
        }
        ComposedExactExpr::Min(values) | ComposedExactExpr::Max(values) => {
            for value in values {
                collect_leaf_evidence(value, schema, into);
            }
        }
        ComposedExactExpr::Literal(_) | ComposedExactExpr::Input(_) => {}
    }
}

fn compile_expression<C: ComposedExactLeafCodec>(
    expr: &ComposedExactExpr<C::Leaf>,
    path: &str,
) -> Result<(ExactExpr, ExactExprRequirements, Vec<RoleRequirement>), ComposedExactError<C::Error>>
{
    let (compiled, capabilities) = match expr {
        ComposedExactExpr::Literal(value) => (ExactExpr::Literal(*value), Vec::new()),
        ComposedExactExpr::Input(input) => (ExactExpr::Input(input.clone()), Vec::new()),
        ComposedExactExpr::Add(a, b) => binary::<C>(a, b, ExactExpr::Add, path)?,
        ComposedExactExpr::Subtract(a, b) => binary::<C>(a, b, ExactExpr::Subtract, path)?,
        ComposedExactExpr::Multiply(a, b) => binary::<C>(a, b, ExactExpr::Multiply, path)?,
        ComposedExactExpr::FloorDivide(a, b) => binary::<C>(a, b, ExactExpr::FloorDivide, path)?,
        ComposedExactExpr::TruncatingDivide(a, b) => {
            binary::<C>(a, b, ExactExpr::TruncatingDivide, path)?
        }
        ComposedExactExpr::Min(values) => aggregate_compile::<C>(values, ExactExpr::Min, path)?,
        ComposedExactExpr::Max(values) => aggregate_compile::<C>(values, ExactExpr::Max, path)?,
        ComposedExactExpr::Product(leaf) => {
            let compiled = C::compile_leaf(leaf.value()).map_err(|error| {
                ComposedExactError::ProductCompile {
                    context: Box::new(ComposedExactProductContext::new(
                        bounded_path(path),
                        C::schema(),
                        leaf.kind().clone(),
                        leaf.subject().clone(),
                        leaf.source().clone(),
                    )),
                    error: Box::new(error),
                }
            })?;
            let inspected = ExactExprRequirements::inspect(compiled.expression())
                .map_err(ComposedExactError::Standard)?;
            if inspected != *compiled.requirements() {
                return Err(ComposedExactError::ProductRequirementMismatch {
                    context: Box::new(ComposedExactProductContext::new(
                        bounded_path(path),
                        C::schema(),
                        leaf.kind().clone(),
                        leaf.subject().clone(),
                        leaf.source().clone(),
                    )),
                });
            }
            (
                compiled.expression().clone(),
                compiled.capabilities().to_vec(),
            )
        }
    };
    ExactEvaluator::validate_structure(&compiled, crate::ExactExprLimits::default())
        .map_err(ComposedExactError::Standard)?;
    let requirements =
        ExactExprRequirements::inspect(&compiled).map_err(ComposedExactError::Standard)?;
    Ok((compiled, requirements, capabilities))
}
fn binary<C: ComposedExactLeafCodec>(
    a: &ComposedExactExpr<C::Leaf>,
    b: &ComposedExactExpr<C::Leaf>,
    make: impl Fn(Box<ExactExpr>, Box<ExactExpr>) -> ExactExpr,
    path: &str,
) -> Result<(ExactExpr, Vec<RoleRequirement>), ComposedExactError<C::Error>> {
    let (a, _, mut capabilities) = compile_expression::<C>(a, &child_path(path, ".left"))?;
    let (b, _, more) = compile_expression::<C>(b, &child_path(path, ".right"))?;
    capabilities.extend(more);
    Ok((make(Box::new(a), Box::new(b)), capabilities))
}
fn aggregate_compile<C: ComposedExactLeafCodec>(
    values: &[ComposedExactExpr<C::Leaf>],
    make: impl Fn(Vec<ExactExpr>) -> ExactExpr,
    path: &str,
) -> Result<(ExactExpr, Vec<RoleRequirement>), ComposedExactError<C::Error>> {
    let mut capabilities = Vec::new();
    let values = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let (value, _, more) =
                compile_expression::<C>(value, &child_path(path, &format!(".values[{index}]")))?;
            capabilities.extend(more);
            Ok(value)
        })
        .collect::<Result<Vec<_>, ComposedExactError<C::Error>>>()?;
    Ok((make(values), capabilities))
}

fn validate_roles(
    requirements: &ExactExprRequirements,
    roles: &[RoleRequirement],
) -> Result<(), ComposedExactDefinitionError> {
    for input in requirements.inputs() {
        if roles
            .binary_search_by(|r| r.role().cmp(input.role()))
            .is_err()
        {
            return Err(ComposedExactDefinitionError::UndeclaredInputRole {
                role: input.role().clone(),
            });
        }
    }
    Ok(())
}
fn validate_product_capabilities(
    required: &[RoleRequirement],
    roles: &[RoleRequirement],
) -> Result<(), ComposedExactDefinitionError> {
    for requirement in required {
        let declared = roles
            .binary_search_by(|role| role.role().cmp(requirement.role()))
            .map(|index| &roles[index])
            .map_err(|_| ComposedExactDefinitionError::UndeclaredInputRole {
                role: requirement.role().clone(),
            })?;
        for capability in requirement.capabilities() {
            if declared.capabilities().binary_search(capability).is_err() {
                return Err(ComposedExactDefinitionError::MissingProductCapability {
                    role: requirement.role().clone(),
                    capability: capability.clone(),
                });
            }
        }
    }
    Ok(())
}
pub(crate) fn canonicalize_roles(
    roles: Vec<RoleRequirement>,
) -> Result<Vec<RoleRequirement>, ComposedExactDefinitionError> {
    let mut merged = BTreeMap::<CapabilityRoleId, BTreeSet<crate::CapabilityRequirementId>>::new();
    for role in roles {
        merged
            .entry(role.role().clone())
            .or_default()
            .extend(role.capabilities().iter().cloned());
    }
    merged
        .into_iter()
        .map(|(role, capabilities)| {
            RoleRequirement::new(role, capabilities.into_iter().collect())
                .map_err(ComposedExactDefinitionError::Role)
        })
        .collect()
}
