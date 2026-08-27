use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};

use gameplay_mechanics::MechanicsScalar;
use gameplay_rules::{
    admit_rule_package, canonical_rule_json_value_len, decode_canonical_rule_package,
    decode_rule_package, select_rule_payload_subtree, AdmittedRulePackage, RuleDomainId,
    RuleFingerprint, RulePackageCandidate, RulePackageId, RulePackageSchemaVersion,
    RulePayloadPath, RulePayloadPathSegment, RuleProvenance, RuleSource, RuleSourceId,
    RuleSubjectId, RuleVersion,
};
use gameplay_standard::{
    attempt_quantize_continuous_to_mechanics, quantize_continuous_to_mechanics,
    CapabilityRequirementId, CapabilityRoleId, ComposedExactLeafCodec, ContinuousEvaluationError,
    ContinuousEvaluator, ContinuousExpr, ContinuousExprLimits, ContinuousQuantizationMode,
    ContinuousQuantizationSource, ContinuousValue, ContinuousValueError, ExactEvaluator, ExactExpr,
    ExactExprLimits, ExactInputBundle, ExactInputReference, InputId, RoleRequirement,
};

fn scalar(value: i64) -> MechanicsScalar {
    MechanicsScalar::new(value).unwrap()
}

fn continuous(value: f64) -> ContinuousValue {
    ContinuousValue::new(value).unwrap()
}
fn direct_input() -> ContinuousQuantizationSource {
    ContinuousQuantizationSource::DirectInput {
        input: gameplay_standard::ContinuousInputReference::Parameter {
            role: role("quantizer"),
            id: InputId::parse("quantized-value").unwrap(),
        },
    }
}
fn role(value: &str) -> CapabilityRoleId {
    CapabilityRoleId::parse(value).unwrap()
}
fn parameter(role_name: &str, id: &str) -> gameplay_standard::ExactInputReference {
    gameplay_standard::ExactInputReference::Parameter {
        role: role(role_name),
        id: InputId::parse(id).unwrap(),
    }
}

fn bounded_roll(
    role_name: &str,
    id: &str,
    minimum: i64,
    maximum: i64,
) -> gameplay_standard::ExactInputReference {
    gameplay_standard::ExactInputReference::bounded_roll(
        role(role_name),
        InputId::parse(id).unwrap(),
        scalar(minimum),
        scalar(maximum),
    )
}

#[test]
fn definition_requirements_are_canonical_and_reject_undeclared_input_roles() {
    let subject = RuleSubjectId::parse("requirement_formula").unwrap();
    let source = RuleSourceId::parse("rules").unwrap();
    let expression = ExactExpr::Add(
        Box::new(ExactExpr::Input(parameter("caster", "bonus"))),
        Box::new(ExactExpr::Input(parameter("caster", "bonus"))),
    );
    assert!(matches!(
        gameplay_standard::ExactDefinition::new(
            subject.clone(),
            source.clone(),
            expression.clone(),
            vec![]
        ),
        Err(gameplay_standard::StandardDefinitionError::UndeclaredInputRole { .. })
    ));
    let requirements = gameplay_standard::ExactDefinition::new(
        subject,
        source,
        expression,
        vec![RoleRequirement::new(role("caster"), vec![]).unwrap()],
    )
    .unwrap()
    .requirements()
    .unwrap();
    assert_eq!(requirements.inputs().len(), 1);
    assert_eq!(
        requirements.inputs()[0].kind(),
        gameplay_standard::InputKind::Parameter
    );
    assert_eq!(requirements.roles().len(), 1);
}

#[test]
fn fixed_power_and_bounded_roll_have_exact_identity_and_runtime_semantics() {
    let fixed = |exponent| {
        ExactExpr::fixed_power(
            ExactExpr::Literal(scalar(1_040)),
            ExactExpr::Literal(scalar(exponent)),
            scalar(1_000),
        )
    };
    for (exponent, expected) in [
        (0, 1_000),
        (1, 1_040),
        (2, 1_081),
        (3, 1_124),
        (4, 1_168),
        (8, 1_364),
        (16, 1_861),
        (32, 3_474),
        (64, 12_153),
    ] {
        assert_eq!(
            ExactEvaluator::evaluate(
                &fixed(exponent),
                &ExactInputBundle::empty(),
                ExactExprLimits::default(),
            )
            .unwrap()
            .get(),
            expected,
        );
    }
    assert_eq!(
        ExactEvaluator::evaluate(
            &ExactExpr::fixed_power(
                ExactExpr::Literal(scalar(0)),
                ExactExpr::Literal(scalar(0)),
                scalar(7),
            ),
            &ExactInputBundle::empty(),
            ExactExprLimits::default(),
        )
        .unwrap()
        .get(),
        7,
    );
    let zero_exponent_inputs = ExactExpr::fixed_power(
        ExactExpr::Input(parameter("self", "base")),
        ExactExpr::Input(parameter("self", "exponent")),
        scalar(1),
    );
    assert!(matches!(
        ExactEvaluator::evaluate(&zero_exponent_inputs, &ExactInputBundle::new(vec![(parameter("self", "base"), scalar(0))]).unwrap(), ExactExprLimits::default()),
        Err(gameplay_standard::ExactEvaluationError::MissingInput { input }) if input == parameter("self", "exponent")
    ));
    for expression in [
        ExactExpr::fixed_power(
            ExactExpr::Literal(scalar(1)),
            ExactExpr::Literal(scalar(1)),
            scalar(0),
        ),
        ExactExpr::fixed_power(
            ExactExpr::Literal(scalar(1)),
            ExactExpr::Literal(scalar(1)),
            scalar(-1),
        ),
        ExactExpr::fixed_power(
            ExactExpr::Literal(scalar(1)),
            ExactExpr::Literal(scalar(1)),
            scalar(1_000_001),
        ),
    ] {
        assert!(matches!(
            ExactEvaluator::evaluate(
                &expression,
                &ExactInputBundle::empty(),
                ExactExprLimits::default()
            ),
            Err(gameplay_standard::ExactEvaluationError::FixedPowerScaleOutOfRange { .. })
        ));
    }
    assert!(matches!(
        ExactEvaluator::evaluate(
            &ExactExpr::fixed_power(
                ExactExpr::Literal(scalar(-1)),
                ExactExpr::Literal(scalar(1)),
                scalar(1),
            ),
            &ExactInputBundle::empty(),
            ExactExprLimits::default()
        ),
        Err(gameplay_standard::ExactEvaluationError::FixedPowerNegativeBase { .. })
    ));
    assert!(matches!(
        ExactEvaluator::evaluate(
            &fixed(65),
            &ExactInputBundle::empty(),
            ExactExprLimits::default()
        ),
        Err(gameplay_standard::ExactEvaluationError::FixedPowerExponentOutOfRange { .. })
    ));
    assert!(matches!(
        ExactEvaluator::evaluate(
            &fixed(1),
            &ExactInputBundle::empty(),
            ExactExprLimits {
                maximum_depth: 32,
                maximum_nodes: 256,
                maximum_inputs: 64,
                maximum_arity: 16,
                maximum_work: 3
            }
        ),
        Err(gameplay_standard::ExactEvaluationError::WorkQuotaExceeded { .. })
    ));
    assert!(matches!(
        ExactEvaluator::evaluate(
            &ExactExpr::fixed_power(
                ExactExpr::Literal(scalar(1_000_000_000_000)),
                ExactExpr::Literal(scalar(2)),
                scalar(1),
            ),
            &ExactInputBundle::empty(),
            ExactExprLimits::default(),
        ),
        Err(gameplay_standard::ExactEvaluationError::FixedPowerScalarRange { .. })
    ));

    let roll = bounded_roll("self", "attack", 1, 20);
    let expression = ExactExpr::Input(roll.clone());
    assert!(matches!(
        ExactEvaluator::evaluate(
            &expression,
            &ExactInputBundle::empty(),
            ExactExprLimits::default()
        ),
        Err(gameplay_standard::ExactEvaluationError::MissingBoundedRoll { .. })
    ));
    assert_eq!(
        ExactEvaluator::evaluate(
            &expression,
            &ExactInputBundle::new(vec![(roll.clone(), scalar(20))]).unwrap(),
            ExactExprLimits::default()
        )
        .unwrap()
        .get(),
        20,
    );
    assert!(matches!(
        ExactEvaluator::evaluate(
            &expression,
            &ExactInputBundle::new(vec![(roll.clone(), scalar(21))]).unwrap(),
            ExactExprLimits::default()
        ),
        Err(gameplay_standard::ExactEvaluationError::BoundedRollOutOfRange { .. })
    ));
    assert!(matches!(
        ExactInputBundle::new(vec![(roll.clone(), scalar(1)), (roll.clone(), scalar(2))]),
        Err(gameplay_standard::ExactInputBundleError::ConflictingValue { .. })
    ));
    assert!(matches!(
        ExactInputBundle::new(vec![
            (roll.clone(), scalar(1)),
            (bounded_roll("self", "attack", 0, 20), scalar(1))
        ]),
        Err(gameplay_standard::ExactInputBundleError::ConflictingDescriptor { .. })
    ));
    assert!(ExactInputBundle::new(vec![(roll.clone(), scalar(1)), (roll, scalar(1))]).is_ok());
    assert!(ExactInputBundle::new(vec![
        (bounded_roll("self", "attack", 1, 20), scalar(1)),
        (bounded_roll("other", "attack", 1, 20), scalar(1)),
        (
            gameplay_standard::ExactInputReference::Roll {
                role: role("self"),
                id: InputId::parse("attack").unwrap()
            },
            scalar(1)
        ),
    ])
    .is_ok());
    assert!(matches!(
        ExactEvaluator::validate_structure(
            &ExactExpr::Input(bounded_roll("self", "bad", 2, 1)),
            ExactExprLimits::default(),
        ),
        Err(gameplay_standard::ExactEvaluationError::BoundedRollInvalidBounds { .. })
    ));
}

struct ExactLeaf;
impl gameplay_standard::CompileExactExpr for ExactLeaf {
    type Error = std::convert::Infallible;

    fn compile_exact_expr(&self) -> Result<ExactExpr, Self::Error> {
        Ok(ExactExpr::Literal(scalar(7)))
    }
}
struct ContinuousLeaf;
impl gameplay_standard::CompileContinuousExpr for ContinuousLeaf {
    type Error = std::convert::Infallible;

    fn compile_continuous_expr(&self) -> Result<ContinuousExpr, Self::Error> {
        Ok(ContinuousExpr::Literal(continuous(7.0)))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeafError {
    MissingCoefficient,
}
impl std::fmt::Display for LeafError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("missing coefficient")
    }
}
impl std::error::Error for LeafError {}
struct FailingExactLeaf;
impl gameplay_standard::CompileExactExpr for FailingExactLeaf {
    type Error = LeafError;

    fn compile_exact_expr(&self) -> Result<ExactExpr, Self::Error> {
        Err(LeafError::MissingCoefficient)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NeutralProductLeaf {
    EquippedTool,
    Protection,
}
struct NeutralProductCodec;
impl gameplay_standard::ComposedExactLeafCodec for NeutralProductCodec {
    type Leaf = NeutralProductLeaf;
    type Error = LeafError;

    fn schema() -> gameplay_standard::StandardExtensionSchema {
        gameplay_standard::StandardExtensionSchema::new(
            CapabilityRequirementId::parse("example.combat").unwrap(),
            1,
        )
        .unwrap()
    }
    fn decode_leaf(
        kind: &gameplay_standard::ComposedExactLeafKindId,
        payload: &serde_json::Value,
    ) -> Result<Self::Leaf, Self::Error> {
        let object = payload.as_object().ok_or(LeafError::MissingCoefficient)?;
        if object.len() != 1
            || object.get("slot") != Some(&serde_json::Value::String(kind.as_str().to_owned()))
        {
            return Err(LeafError::MissingCoefficient);
        }
        match kind.as_str() {
            "combat.equipped-tool" => Ok(NeutralProductLeaf::EquippedTool),
            "combat.protection" => Ok(NeutralProductLeaf::Protection),
            _ => Err(LeafError::MissingCoefficient),
        }
    }
    fn encode_leaf(
        kind: &gameplay_standard::ComposedExactLeafKindId,
        leaf: &Self::Leaf,
    ) -> Result<serde_json::Value, Self::Error> {
        let expected = match leaf {
            NeutralProductLeaf::EquippedTool => "combat.equipped-tool",
            NeutralProductLeaf::Protection => "combat.protection",
        };
        if kind.as_str() != expected {
            return Err(LeafError::MissingCoefficient);
        }
        Ok(serde_json::json!({"slot":expected}))
    }
    fn compile_leaf(
        leaf: &Self::Leaf,
    ) -> Result<gameplay_standard::CompiledComposedExactLeaf, Self::Error> {
        let (role_name, input, capability) = match leaf {
            NeutralProductLeaf::EquippedTool => ("attacker", "equipped-tool", "read.equipped-tool"),
            NeutralProductLeaf::Protection => ("defender", "protection", "read.protection"),
        };
        let expression = ExactExpr::Input(parameter(role_name, input));
        let requirements = gameplay_standard::ExactExprRequirements::inspect(&expression).unwrap();
        Ok(gameplay_standard::CompiledComposedExactLeaf::new(
            expression,
            requirements,
            vec![RoleRequirement::new(
                role(role_name),
                vec![CapabilityRequirementId::parse(capability).unwrap()],
            )
            .unwrap()],
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SharedCapabilityLeaf {
    Z,
    A,
}

struct SharedCapabilityCodec;
impl gameplay_standard::ComposedExactLeafCodec for SharedCapabilityCodec {
    type Leaf = SharedCapabilityLeaf;
    type Error = LeafError;

    fn schema() -> gameplay_standard::StandardExtensionSchema {
        gameplay_standard::StandardExtensionSchema::new(
            CapabilityRequirementId::parse("example.shared").unwrap(),
            1,
        )
        .unwrap()
    }
    fn decode_leaf(
        _kind: &gameplay_standard::ComposedExactLeafKindId,
        payload: &serde_json::Value,
    ) -> Result<Self::Leaf, Self::Error> {
        match payload.get("mode").and_then(serde_json::Value::as_str) {
            Some("z") => Ok(SharedCapabilityLeaf::Z),
            Some("a") => Ok(SharedCapabilityLeaf::A),
            _ => Err(LeafError::MissingCoefficient),
        }
    }
    fn encode_leaf(
        _kind: &gameplay_standard::ComposedExactLeafKindId,
        leaf: &Self::Leaf,
    ) -> Result<serde_json::Value, Self::Error> {
        Ok(serde_json::json!({
            "mode": match leaf {
                SharedCapabilityLeaf::Z => "z",
                SharedCapabilityLeaf::A => "a",
            }
        }))
    }
    fn compile_leaf(
        leaf: &Self::Leaf,
    ) -> Result<gameplay_standard::CompiledComposedExactLeaf, Self::Error> {
        let capability = match leaf {
            SharedCapabilityLeaf::Z => "read.z",
            SharedCapabilityLeaf::A => "read.a",
        };
        let expression = ExactExpr::Literal(scalar(1));
        let requirements = gameplay_standard::ExactExprRequirements::inspect(&expression).unwrap();
        Ok(gameplay_standard::CompiledComposedExactLeaf::new(
            expression,
            requirements,
            vec![RoleRequirement::new(
                role("shared"),
                vec![CapabilityRequirementId::parse(capability).unwrap()],
            )
            .unwrap()],
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CountingProductLeaf(usize);

static COUNTING_DECODE_CALLS: AtomicUsize = AtomicUsize::new(0);
static COUNTING_COMPILE_CALLS: AtomicUsize = AtomicUsize::new(0);

struct CountingProductCodec;
impl gameplay_standard::ComposedExactLeafCodec for CountingProductCodec {
    type Leaf = CountingProductLeaf;
    type Error = LeafError;

    fn schema() -> gameplay_standard::StandardExtensionSchema {
        gameplay_standard::StandardExtensionSchema::new(
            CapabilityRequirementId::parse("example.counting").unwrap(),
            1,
        )
        .unwrap()
    }
    fn decode_leaf(
        _kind: &gameplay_standard::ComposedExactLeafKindId,
        payload: &serde_json::Value,
    ) -> Result<Self::Leaf, Self::Error> {
        COUNTING_DECODE_CALLS.fetch_add(1, Ordering::SeqCst);
        let length = payload
            .get("blob")
            .and_then(serde_json::Value::as_str)
            .ok_or(LeafError::MissingCoefficient)?
            .len();
        Ok(CountingProductLeaf(length))
    }
    fn encode_leaf(
        _kind: &gameplay_standard::ComposedExactLeafKindId,
        leaf: &Self::Leaf,
    ) -> Result<serde_json::Value, Self::Error> {
        Ok(serde_json::json!({"blob":"x".repeat(leaf.0)}))
    }
    fn compile_leaf(
        _leaf: &Self::Leaf,
    ) -> Result<gameplay_standard::CompiledComposedExactLeaf, Self::Error> {
        COUNTING_COMPILE_CALLS.fetch_add(1, Ordering::SeqCst);
        let expression = ExactExpr::Literal(scalar(1));
        let requirements = gameplay_standard::ExactExprRequirements::inspect(&expression).unwrap();
        Ok(gameplay_standard::CompiledComposedExactLeaf::new(
            expression,
            requirements,
            vec![],
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatrixProductLeaf {
    Valid,
    CompileFailure,
    RequirementMismatch,
    NonConvergent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatrixLeafError {
    Decode,
    Compile,
}
impl std::fmt::Display for MatrixLeafError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Decode => "matrix decode failed",
            Self::Compile => "matrix compile failed",
        })
    }
}
impl std::error::Error for MatrixLeafError {}

struct MatrixProductCodec;
impl gameplay_standard::ComposedExactLeafCodec for MatrixProductCodec {
    type Leaf = MatrixProductLeaf;
    type Error = MatrixLeafError;

    fn schema() -> gameplay_standard::StandardExtensionSchema {
        gameplay_standard::StandardExtensionSchema::new(
            CapabilityRequirementId::parse("example.matrix").unwrap(),
            1,
        )
        .unwrap()
    }
    fn decode_leaf(
        _kind: &gameplay_standard::ComposedExactLeafKindId,
        payload: &serde_json::Value,
    ) -> Result<Self::Leaf, Self::Error> {
        match payload.get("mode").and_then(serde_json::Value::as_str) {
            Some("decode") => Err(MatrixLeafError::Decode),
            Some("compile") => Ok(MatrixProductLeaf::CompileFailure),
            Some("requirements") => Ok(MatrixProductLeaf::RequirementMismatch),
            Some("nonconvergent") => Ok(MatrixProductLeaf::NonConvergent),
            _ => Ok(MatrixProductLeaf::Valid),
        }
    }
    fn encode_leaf(
        _kind: &gameplay_standard::ComposedExactLeafKindId,
        leaf: &Self::Leaf,
    ) -> Result<serde_json::Value, Self::Error> {
        let mode = match leaf {
            MatrixProductLeaf::Valid => "valid",
            MatrixProductLeaf::CompileFailure => "compile",
            MatrixProductLeaf::RequirementMismatch => "requirements",
            MatrixProductLeaf::NonConvergent => "valid",
        };
        Ok(serde_json::json!({"mode":mode}))
    }
    fn compile_leaf(
        leaf: &Self::Leaf,
    ) -> Result<gameplay_standard::CompiledComposedExactLeaf, Self::Error> {
        match leaf {
            MatrixProductLeaf::Valid | MatrixProductLeaf::NonConvergent => {
                let expression = ExactExpr::Literal(scalar(7));
                let requirements =
                    gameplay_standard::ExactExprRequirements::inspect(&expression).unwrap();
                Ok(gameplay_standard::CompiledComposedExactLeaf::new(
                    expression,
                    requirements,
                    vec![],
                ))
            }
            MatrixProductLeaf::CompileFailure => Err(MatrixLeafError::Compile),
            MatrixProductLeaf::RequirementMismatch => {
                let expression = ExactExpr::Input(parameter("matrix", "value"));
                let declared = ExactExpr::Literal(scalar(0));
                let requirements =
                    gameplay_standard::ExactExprRequirements::inspect(&declared).unwrap();
                Ok(gameplay_standard::CompiledComposedExactLeaf::new(
                    expression,
                    requirements,
                    vec![],
                ))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuotaProductLeaf {
    LargeNodeTree,
    ManyInputs(u8),
    WorkTree,
}

struct QuotaProductCodec;
impl gameplay_standard::ComposedExactLeafCodec for QuotaProductCodec {
    type Leaf = QuotaProductLeaf;
    type Error = LeafError;

    fn schema() -> gameplay_standard::StandardExtensionSchema {
        gameplay_standard::StandardExtensionSchema::new(
            CapabilityRequirementId::parse("example.quotas").unwrap(),
            1,
        )
        .unwrap()
    }
    fn decode_leaf(
        _kind: &gameplay_standard::ComposedExactLeafKindId,
        payload: &serde_json::Value,
    ) -> Result<Self::Leaf, Self::Error> {
        match payload.get("mode").and_then(serde_json::Value::as_str) {
            Some("nodes") => Ok(QuotaProductLeaf::LargeNodeTree),
            Some("inputs-a") => Ok(QuotaProductLeaf::ManyInputs(0)),
            Some("inputs-b") => Ok(QuotaProductLeaf::ManyInputs(1)),
            Some("work") => Ok(QuotaProductLeaf::WorkTree),
            _ => Err(LeafError::MissingCoefficient),
        }
    }
    fn encode_leaf(
        _kind: &gameplay_standard::ComposedExactLeafKindId,
        leaf: &Self::Leaf,
    ) -> Result<serde_json::Value, Self::Error> {
        let mode = match leaf {
            QuotaProductLeaf::LargeNodeTree => "nodes",
            QuotaProductLeaf::ManyInputs(0) => "inputs-a",
            QuotaProductLeaf::ManyInputs(1) => "inputs-b",
            QuotaProductLeaf::ManyInputs(_) => return Err(LeafError::MissingCoefficient),
            QuotaProductLeaf::WorkTree => "work",
        };
        Ok(serde_json::json!({"mode":mode}))
    }
    fn compile_leaf(
        leaf: &Self::Leaf,
    ) -> Result<gameplay_standard::CompiledComposedExactLeaf, Self::Error> {
        let expression = match leaf {
            QuotaProductLeaf::LargeNodeTree => exact_full_binary_tree(7),
            QuotaProductLeaf::ManyInputs(group) => {
                let inputs = (0..64)
                    .map(|index| parameter("quota", &format!("input-{group}-{index}")))
                    .collect::<Vec<_>>();
                exact_sum_tree(&inputs)
            }
            QuotaProductLeaf::WorkTree => ExactExpr::Add(
                Box::new(ExactExpr::Literal(scalar(1))),
                Box::new(ExactExpr::Literal(scalar(2))),
            ),
        };
        let requirements = gameplay_standard::ExactExprRequirements::inspect(&expression)
            .map_err(|_| LeafError::MissingCoefficient)?;
        Ok(gameplay_standard::CompiledComposedExactLeaf::new(
            expression,
            requirements,
            vec![],
        ))
    }
}

fn raw_composed_payload(
    family: &str,
    extension: serde_json::Value,
    tree: serde_json::Value,
    roles: serde_json::Value,
    subject: &str,
    source: &str,
) -> serde_json::Value {
    serde_json::json!({
        "family": family,
        "semanticsVersion": 1,
        "subject": subject,
        "source": source,
        "roles": roles,
        "extension": extension,
        "tree": tree,
    })
}

fn raw_product_tree(
    kind: &str,
    subject: &str,
    source: &str,
    payload: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "op":"product",
        "kind":kind,
        "subject":subject,
        "source":source,
        "payload":payload,
    })
}

fn raw_package(
    schema_version: RulePackageSchemaVersion,
    payload: serde_json::Value,
    sources: Vec<RuleSource>,
    provenance: Vec<RuleProvenance>,
) -> AdmittedRulePackage {
    admit_rule_package(RulePackageCandidate::new_with_schema(
        schema_version,
        RuleDomainId::parse("game").unwrap(),
        RulePackageId::parse("composed-matrix").unwrap(),
        RuleVersion::new(1).unwrap(),
        vec![],
        sources,
        provenance,
        payload,
    ))
    .unwrap()
}

fn matrix_extension_payload() -> serde_json::Value {
    serde_json::json!({"namespace":"example.matrix","schemaVersion":1})
}

fn counting_extension_payload() -> serde_json::Value {
    serde_json::json!({"namespace":"example.counting","schemaVersion":1})
}

fn source(id: &str) -> RuleSource {
    RuleSource::new(RuleSourceId::parse(id).unwrap(), format!("{id}.json")).unwrap()
}

fn provenance(subject: &str, source: &str) -> RuleProvenance {
    RuleProvenance::new(
        RuleSubjectId::parse(subject).unwrap(),
        RuleSourceId::parse(source).unwrap(),
        None,
        None,
    )
    .unwrap()
}

fn literal_wire() -> serde_json::Value {
    serde_json::json!({"op":"literal","value":1})
}

fn left_deep_wire_tree(depth: usize, leaf: serde_json::Value) -> serde_json::Value {
    let mut tree = leaf;
    for _ in 1..depth {
        tree = serde_json::json!({"op":"add","left":tree,"right":{"op":"literal","value":1}});
    }
    tree
}

fn full_binary_wire_tree(levels: usize, product: serde_json::Value) -> serde_json::Value {
    if levels == 0 {
        return product;
    }
    let left = if levels == 1 {
        product
    } else {
        full_binary_wire_tree(levels - 1, product)
    };
    let right = full_binary_wire_tree(levels - 1, literal_wire());
    serde_json::json!({"op":"add","left":left,"right":right})
}

fn exact_full_binary_tree(levels: usize) -> ExactExpr {
    if levels == 0 {
        return ExactExpr::Literal(scalar(1));
    }
    ExactExpr::Add(
        Box::new(exact_full_binary_tree(levels - 1)),
        Box::new(exact_full_binary_tree(levels - 1)),
    )
}

fn composed_full_binary_tree(
    levels: usize,
) -> gameplay_standard::ComposedExactExpr<CountingProductLeaf> {
    if levels == 0 {
        return gameplay_standard::ComposedExactExpr::Literal(scalar(1));
    }
    gameplay_standard::ComposedExactExpr::Add(
        Box::new(composed_full_binary_tree(levels - 1)),
        Box::new(composed_full_binary_tree(levels - 1)),
    )
}

fn exact_sum_tree(inputs: &[ExactInputReference]) -> ExactExpr {
    match inputs {
        [input] => ExactExpr::Input(input.clone()),
        _ => {
            let midpoint = inputs.len() / 2;
            ExactExpr::Add(
                Box::new(exact_sum_tree(&inputs[..midpoint])),
                Box::new(exact_sum_tree(&inputs[midpoint..])),
            )
        }
    }
}

fn payload_with_canonical_len(target: usize) -> serde_json::Value {
    let empty = serde_json::json!({"blob":""});
    let base =
        canonical_rule_json_value_len(&empty, RulePackageSchemaVersion::IntegerOnlyV1, target + 1)
            .unwrap();
    let payload = serde_json::json!({"blob":"x".repeat(target - base)});
    assert_eq!(
        canonical_rule_json_value_len(
            &payload,
            RulePackageSchemaVersion::IntegerOnlyV1,
            target + 1,
        )
        .unwrap(),
        target
    );
    payload
}

fn counting_product(subject: &str, payload: serde_json::Value) -> serde_json::Value {
    raw_product_tree("counting.leaf", subject, "rules", payload)
}

fn matrix_product(kind: &str, subject: &str, payload: serde_json::Value) -> serde_json::Value {
    raw_product_tree(kind, subject, "rules", payload)
}

fn standard_context(
    subjects: &[&str],
    source_ids: &[&str],
) -> gameplay_standard::StandardPackageContext {
    let sources = source_ids.iter().map(|id| source(id)).collect::<Vec<_>>();
    let provenance = subjects
        .iter()
        .map(|subject| provenance(subject, source_ids[0]))
        .collect::<Vec<_>>();
    gameplay_standard::StandardPackageContext::new(
        RulePackageSchemaVersion::IntegerOnlyV1,
        RuleDomainId::parse("game").unwrap(),
        RulePackageId::parse("composed-matrix").unwrap(),
        RuleVersion::new(1).unwrap(),
        vec![],
        sources,
        provenance,
    )
}

#[test]
fn composed_wire_quotas_and_payload_bytes_reject_before_product_decode() {
    let limits = ExactExprLimits::default();
    let direct_definition = |expression| {
        gameplay_standard::ComposedExactDefinition::new(
            CountingProductCodec::schema(),
            RuleSubjectId::parse("direct").unwrap(),
            RuleSourceId::parse("rules").unwrap(),
            expression,
            vec![],
        )
    };

    let mut too_deep = gameplay_standard::ComposedExactExpr::Literal(scalar(1));
    for _ in 1..=limits.maximum_depth {
        too_deep = gameplay_standard::ComposedExactExpr::Add(
            Box::new(too_deep),
            Box::new(gameplay_standard::ComposedExactExpr::Literal(scalar(1))),
        );
    }
    assert!(matches!(
        direct_definition(too_deep),
        Err(gameplay_standard::ComposedExactDefinitionError::DepthQuotaExceeded {
            actual,
            maximum
        }) if actual == limits.maximum_depth + 1 && maximum == limits.maximum_depth
    ));
    let too_many_nodes = gameplay_standard::ComposedExactExpr::Add(
        Box::new(composed_full_binary_tree(7)),
        Box::new(gameplay_standard::ComposedExactExpr::Literal(scalar(1))),
    );
    assert!(matches!(
        direct_definition(too_many_nodes),
        Err(gameplay_standard::ComposedExactDefinitionError::NodeQuotaExceeded {
            actual,
            maximum
        }) if actual == limits.maximum_nodes + 1 && maximum == limits.maximum_nodes
    ));
    let too_many_aggregate_children = gameplay_standard::ComposedExactExpr::Min(
        (0..=limits.maximum_arity)
            .map(|_| gameplay_standard::ComposedExactExpr::Literal(scalar(1)))
            .collect(),
    );
    assert!(matches!(
        direct_definition(too_many_aggregate_children),
        Err(gameplay_standard::ComposedExactDefinitionError::ArityQuotaExceeded {
            actual,
            maximum
        }) if actual == limits.maximum_arity + 1 && maximum == limits.maximum_arity
    ));

    COUNTING_DECODE_CALLS.store(0, Ordering::SeqCst);
    COUNTING_COMPILE_CALLS.store(0, Ordering::SeqCst);
    let sources = vec![source("rules")];
    let schema = counting_extension_payload();
    let roleless = serde_json::json!([]);
    let deep_tree = left_deep_wire_tree(
        limits.maximum_depth + 1,
        counting_product("deep-leaf", serde_json::json!({"blob":"x"})),
    );
    let deep_package = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            schema.clone(),
            deep_tree,
            roleless.clone(),
            "formula",
            "rules",
        ),
        sources.clone(),
        vec![
            provenance("formula", "rules"),
            provenance("deep-leaf", "rules"),
        ],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<CountingProductCodec>(&deep_package),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::DepthQuotaExceeded {
                actual,
                maximum
            }
        )) if actual == limits.maximum_depth + 1 && maximum == limits.maximum_depth
    ));
    assert_eq!(COUNTING_DECODE_CALLS.load(Ordering::SeqCst), 0);

    let node_tree = serde_json::json!({
        "op":"add",
        "left":full_binary_wire_tree(7, counting_product("node-leaf", serde_json::json!({"blob":"x"}))),
        "right":{"op":"literal","value":1}
    });
    let node_package = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            schema.clone(),
            node_tree,
            roleless.clone(),
            "formula",
            "rules",
        ),
        sources.clone(),
        vec![
            provenance("formula", "rules"),
            provenance("node-leaf", "rules"),
        ],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<CountingProductCodec>(&node_package),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::NodeQuotaExceeded {
                actual,
                maximum
            }
        )) if actual == limits.maximum_nodes + 1 && maximum == limits.maximum_nodes
    ));
    assert_eq!(COUNTING_DECODE_CALLS.load(Ordering::SeqCst), 0);

    let arity_tree = serde_json::json!({
        "op":"min",
        "values":std::iter::once(counting_product("arity-leaf", serde_json::json!({"blob":"x"})))
            .chain((0..limits.maximum_arity).map(|_| literal_wire()))
            .collect::<Vec<_>>()
    });
    let arity_package = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            schema.clone(),
            arity_tree,
            roleless.clone(),
            "formula",
            "rules",
        ),
        sources.clone(),
        vec![
            provenance("formula", "rules"),
            provenance("arity-leaf", "rules"),
        ],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<CountingProductCodec>(&arity_package),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::ArityQuotaExceeded {
                actual,
                maximum
            }
        )) if actual == limits.maximum_arity + 1 && maximum == limits.maximum_arity
    ));
    assert_eq!(COUNTING_DECODE_CALLS.load(Ordering::SeqCst), 0);

    let maximum = gameplay_standard::MAX_STANDARD_EXTENSION_PAYLOAD_BYTES;
    let exact_package = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            schema.clone(),
            counting_product("exact-leaf", payload_with_canonical_len(maximum)),
            roleless.clone(),
            "formula",
            "rules",
        ),
        sources.clone(),
        vec![
            provenance("formula", "rules"),
            provenance("exact-leaf", "rules"),
        ],
    );
    assert!(
        gameplay_standard::compile_composed_exact_package::<CountingProductCodec>(&exact_package)
            .is_ok()
    );
    assert_eq!(COUNTING_DECODE_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(COUNTING_COMPILE_CALLS.load(Ordering::SeqCst), 1);

    let over_maximum_package = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            schema.clone(),
            counting_product("over-leaf", payload_with_canonical_len(maximum + 1)),
            roleless.clone(),
            "formula",
            "rules",
        ),
        sources.clone(),
        vec![
            provenance("formula", "rules"),
            provenance("over-leaf", "rules"),
        ],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<CountingProductCodec>(
            &over_maximum_package
        ),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::Package(_)
        ))
    ));
    assert_eq!(COUNTING_DECODE_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(COUNTING_COMPILE_CALLS.load(Ordering::SeqCst), 1);

    let half = maximum / 2;
    let aggregate_exact_package = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            schema.clone(),
            serde_json::json!({
                "op":"add",
                "left":counting_product("aggregate-a", payload_with_canonical_len(half)),
                "right":counting_product("aggregate-b", payload_with_canonical_len(maximum - half))
            }),
            roleless.clone(),
            "formula",
            "rules",
        ),
        sources.clone(),
        vec![
            provenance("formula", "rules"),
            provenance("aggregate-a", "rules"),
            provenance("aggregate-b", "rules"),
        ],
    );
    assert!(
        gameplay_standard::compile_composed_exact_package::<CountingProductCodec>(
            &aggregate_exact_package
        )
        .is_ok()
    );
    assert_eq!(COUNTING_DECODE_CALLS.load(Ordering::SeqCst), 3);
    assert_eq!(COUNTING_COMPILE_CALLS.load(Ordering::SeqCst), 3);

    let aggregate_over_package = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            schema,
            serde_json::json!({
                "op":"add",
                "left":counting_product("aggregate-c", payload_with_canonical_len(half)),
                "right":counting_product("aggregate-d", payload_with_canonical_len(maximum - half + 1))
            }),
            roleless,
            "formula",
            "rules",
        ),
        sources,
        vec![
            provenance("formula", "rules"),
            provenance("aggregate-c", "rules"),
            provenance("aggregate-d", "rules"),
        ],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<CountingProductCodec>(
            &aggregate_over_package
        ),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::PayloadQuotaExceeded {
                actual,
                maximum: max,
            }
        )) if actual == maximum + 1 && max == maximum
    ));
    assert_eq!(COUNTING_DECODE_CALLS.load(Ordering::SeqCst), 3);
    assert_eq!(COUNTING_COMPILE_CALLS.load(Ordering::SeqCst), 3);
}

#[test]
fn composed_wire_preflights_literals_and_inputs_before_product_decode() {
    let sources = vec![source("rules")];
    let roles = serde_json::json!([]);
    let schema = counting_extension_payload();
    let product = counting_product("preflight-leaf", serde_json::json!({"blob":"x"}));

    COUNTING_DECODE_CALLS.store(0, Ordering::SeqCst);
    COUNTING_COMPILE_CALLS.store(0, Ordering::SeqCst);
    let invalid_literal = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            schema.clone(),
            serde_json::json!({
                "op":"add",
                "left":product.clone(),
                "right":{"op":"literal","value":1_000_000_000_001_i64}
            }),
            roles.clone(),
            "formula",
            "rules",
        ),
        sources.clone(),
        vec![
            provenance("formula", "rules"),
            provenance("preflight-leaf", "rules"),
        ],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<CountingProductCodec>(
            &invalid_literal
        ),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::ExactLiteral { path, .. }
        )) if path == "payload.tree.right"
    ));
    assert_eq!(COUNTING_DECODE_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(COUNTING_COMPILE_CALLS.load(Ordering::SeqCst), 0);

    COUNTING_DECODE_CALLS.store(0, Ordering::SeqCst);
    COUNTING_COMPILE_CALLS.store(0, Ordering::SeqCst);
    let undeclared_input = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            schema.clone(),
            serde_json::json!({
                "op":"add",
                "left":product.clone(),
                "right":{"op":"input","input":{"kind":"parameter","role":"rogue","id":"value"}}
            }),
            roles,
            "formula",
            "rules",
        ),
        sources.clone(),
        vec![
            provenance("formula", "rules"),
            provenance("preflight-leaf", "rules"),
        ],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<CountingProductCodec>(
            &undeclared_input
        ),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::UndeclaredInputRole { role }
        )) if role.as_str() == "rogue"
    ));
    assert_eq!(COUNTING_DECODE_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(COUNTING_COMPILE_CALLS.load(Ordering::SeqCst), 0);

    COUNTING_DECODE_CALLS.store(0, Ordering::SeqCst);
    COUNTING_COMPILE_CALLS.store(0, Ordering::SeqCst);
    let invalid_reference = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            schema,
            serde_json::json!({
                "op":"add",
                "left":product,
                "right":{"op":"input","input":{"kind":"parameter","role":"self","id":"not valid"}}
            }),
            serde_json::json!([{"role":"self","capabilities":[]}]),
            "formula",
            "rules",
        ),
        sources,
        vec![
            provenance("formula", "rules"),
            provenance("preflight-leaf", "rules"),
        ],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<CountingProductCodec>(
            &invalid_reference
        ),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::Role(_)
        ))
    ));
    assert_eq!(COUNTING_DECODE_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(COUNTING_COMPILE_CALLS.load(Ordering::SeqCst), 0);
}

#[test]
fn composed_wire_preflights_bounded_descriptors_before_product_decode() {
    let sources = vec![source("rules")];
    let roles = serde_json::json!([{"role":"self","capabilities":[]}]);
    let schema = counting_extension_payload();
    let product = counting_product("preflight-leaf", serde_json::json!({"blob":"x"}));
    let provenance = vec![
        provenance("formula", "rules"),
        provenance("preflight-leaf", "rules"),
    ];

    COUNTING_DECODE_CALLS.store(0, Ordering::SeqCst);
    COUNTING_COMPILE_CALLS.store(0, Ordering::SeqCst);
    let invalid_range = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            schema.clone(),
            serde_json::json!({
                "op":"add",
                "left":product.clone(),
                "right":{"op":"input","input":{
                    "kind":"boundedRoll","role":"self","id":"attack","minimum":20,"maximum":1
                }}
            }),
            roles.clone(),
            "formula",
            "rules",
        ),
        sources.clone(),
        provenance.clone(),
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<CountingProductCodec>(&invalid_range),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::InvalidBoundedRollDescriptor { path, .. }
        )) if path == "payload.tree.right.input"
    ));
    assert_eq!(COUNTING_DECODE_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(COUNTING_COMPILE_CALLS.load(Ordering::SeqCst), 0);

    COUNTING_DECODE_CALLS.store(0, Ordering::SeqCst);
    COUNTING_COMPILE_CALLS.store(0, Ordering::SeqCst);
    let conflicting_range = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            schema,
            serde_json::json!({
                "op":"min",
                "values":[
                    product,
                    {"op":"input","input":{
                        "kind":"boundedRoll","role":"self","id":"attack","minimum":1,"maximum":20
                    }},
                    {"op":"input","input":{
                        "kind":"boundedRoll","role":"self","id":"attack","minimum":2,"maximum":20
                    }}
                ]
            }),
            roles,
            "formula",
            "rules",
        ),
        sources,
        provenance,
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<CountingProductCodec>(
            &conflicting_range
        ),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::ConflictingInputDescriptor {
                path,
                identity: gameplay_standard::ExactInputIdentity::Ordinary {
                    kind: gameplay_standard::InputKind::BoundedRoll,
                    ..
                },
                ..
            }
        )) if path == "payload.tree.values[2].input"
    ));
    assert_eq!(COUNTING_DECODE_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(COUNTING_COMPILE_CALLS.load(Ordering::SeqCst), 0);
}

#[test]
fn composed_product_expansion_uses_global_exact_node_input_and_work_quotas() {
    let source_id = RuleSourceId::parse("rules").unwrap();
    let formula = RuleSubjectId::parse("formula").unwrap();
    let node_leaf = RuleSubjectId::parse("node-leaf").unwrap();
    let node_expression = gameplay_standard::ComposedExactExpr::Add(
        Box::new(gameplay_standard::ComposedExactExpr::Product(
            gameplay_standard::ComposedExactProductLeaf::new(
                gameplay_standard::ComposedExactLeafKindId::parse("quota.nodes").unwrap(),
                node_leaf.clone(),
                source_id.clone(),
                QuotaProductLeaf::LargeNodeTree,
            ),
        )),
        Box::new(gameplay_standard::ComposedExactExpr::Literal(scalar(1))),
    );
    let node_definition = gameplay_standard::ComposedExactDefinition::new(
        QuotaProductCodec::schema(),
        formula.clone(),
        source_id.clone(),
        node_expression,
        vec![],
    )
    .unwrap();
    assert!(matches!(
        gameplay_standard::admit_composed_exact_definition::<QuotaProductCodec>(
            &standard_context(&["formula", "node-leaf"], &["rules"]),
            node_definition,
        ),
        Err(gameplay_standard::ComposedExactError::Standard(
            gameplay_standard::ExactEvaluationError::NodeQuotaExceeded {
                actual,
                maximum,
            }
        )) if actual == ExactExprLimits::default().maximum_nodes + 1
            && maximum == ExactExprLimits::default().maximum_nodes
    ));

    let input_a = RuleSubjectId::parse("input-a").unwrap();
    let input_b = RuleSubjectId::parse("input-b").unwrap();
    let input_leaf = |subject: RuleSubjectId, group| {
        gameplay_standard::ComposedExactExpr::Product(
            gameplay_standard::ComposedExactProductLeaf::new(
                gameplay_standard::ComposedExactLeafKindId::parse("quota.inputs").unwrap(),
                subject,
                source_id.clone(),
                QuotaProductLeaf::ManyInputs(group),
            ),
        )
    };
    let input_expression = gameplay_standard::ComposedExactExpr::Add(
        Box::new(input_leaf(input_a.clone(), 0)),
        Box::new(input_leaf(input_b.clone(), 1)),
    );
    let input_definition = gameplay_standard::ComposedExactDefinition::new(
        QuotaProductCodec::schema(),
        formula.clone(),
        source_id.clone(),
        input_expression,
        vec![RoleRequirement::new(role("quota"), vec![]).unwrap()],
    )
    .unwrap();
    assert!(matches!(
        gameplay_standard::admit_composed_exact_definition::<QuotaProductCodec>(
            &standard_context(&["formula", "input-a", "input-b"], &["rules"]),
            input_definition,
        ),
        Err(gameplay_standard::ComposedExactError::Standard(
            gameplay_standard::ExactEvaluationError::InputQuotaExceeded {
                actual,
                maximum,
            }
        )) if actual == 128 && maximum == ExactExprLimits::default().maximum_inputs
    ));

    let work_leaf = RuleSubjectId::parse("work-leaf").unwrap();
    let work_definition = gameplay_standard::ComposedExactDefinition::new(
        QuotaProductCodec::schema(),
        formula,
        source_id,
        gameplay_standard::ComposedExactExpr::Product(
            gameplay_standard::ComposedExactProductLeaf::new(
                gameplay_standard::ComposedExactLeafKindId::parse("quota.work").unwrap(),
                work_leaf,
                RuleSourceId::parse("rules").unwrap(),
                QuotaProductLeaf::WorkTree,
            ),
        ),
        vec![],
    )
    .unwrap();
    let admitted = gameplay_standard::admit_composed_exact_definition::<QuotaProductCodec>(
        &standard_context(&["formula", "work-leaf"], &["rules"]),
        work_definition,
    )
    .unwrap();
    let low_work = ExactExprLimits {
        maximum_work: 2,
        ..ExactExprLimits::default()
    };
    assert!(matches!(
        ExactEvaluator::evaluate(admitted.compiled(), &ExactInputBundle::empty(), low_work,),
        Err(gameplay_standard::ExactEvaluationError::WorkQuotaExceeded {
            actual: 3,
            maximum: 2,
        })
    ));
}

#[test]
fn composed_product_errors_keep_distinct_identity_and_bounded_wire_paths() {
    let malformed_package = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            matrix_extension_payload(),
            serde_json::json!({
                "op":"add",
                "left":{"op":"not-an-operation"},
                "right":{"op":"literal","value":1}
            }),
            serde_json::json!([]),
            "formula",
            "rules",
        ),
        vec![source("rules")],
        vec![provenance("formula", "rules")],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<MatrixProductCodec>(
            &malformed_package
        ),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::MalformedPayload { path, .. }
        )) if path == "payload.tree.left.op"
    ));

    let decode_subject = RuleSubjectId::parse("decode-leaf").unwrap();
    let decode_package = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            matrix_extension_payload(),
            matrix_product(
                "matrix.decode",
                "decode-leaf",
                serde_json::json!({"mode":"decode"}),
            ),
            serde_json::json!([]),
            "formula",
            "rules",
        ),
        vec![source("rules")],
        vec![
            provenance("formula", "rules"),
            provenance("decode-leaf", "rules"),
        ],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<MatrixProductCodec>(&decode_package),
        Err(gameplay_standard::ComposedExactError::ProductDecode {
            context,
            error,
        }) if context.path() == "payload.tree"
            && context.schema() == &MatrixProductCodec::schema()
            && context.kind().as_str() == "matrix.decode"
            && context.subject() == &decode_subject
            && context.source().as_str() == "rules"
            && *error == MatrixLeafError::Decode
    ));

    let source_id = RuleSourceId::parse("rules").unwrap();
    let compile_subject = RuleSubjectId::parse("compile-leaf").unwrap();
    let compile_definition = gameplay_standard::ComposedExactDefinition::new(
        MatrixProductCodec::schema(),
        RuleSubjectId::parse("formula").unwrap(),
        source_id.clone(),
        gameplay_standard::ComposedExactExpr::Product(
            gameplay_standard::ComposedExactProductLeaf::new(
                gameplay_standard::ComposedExactLeafKindId::parse("matrix.compile").unwrap(),
                compile_subject.clone(),
                source_id.clone(),
                MatrixProductLeaf::CompileFailure,
            ),
        ),
        vec![],
    )
    .unwrap();
    assert!(matches!(
        gameplay_standard::admit_composed_exact_definition::<MatrixProductCodec>(
            &standard_context(&["formula", "compile-leaf"], &["rules"]),
            compile_definition,
        ),
        Err(gameplay_standard::ComposedExactError::ProductCompile {
            context,
            error,
        }) if context.path() == "payload.tree"
            && context.schema() == &MatrixProductCodec::schema()
            && context.kind().as_str() == "matrix.compile"
            && context.subject() == &compile_subject
            && context.source().as_str() == "rules"
            && *error == MatrixLeafError::Compile
    ));

    let requirements_subject = RuleSubjectId::parse("requirements-leaf").unwrap();
    let requirements_definition = gameplay_standard::ComposedExactDefinition::new(
        MatrixProductCodec::schema(),
        RuleSubjectId::parse("formula").unwrap(),
        source_id.clone(),
        gameplay_standard::ComposedExactExpr::Product(
            gameplay_standard::ComposedExactProductLeaf::new(
                gameplay_standard::ComposedExactLeafKindId::parse("matrix.requirements").unwrap(),
                requirements_subject.clone(),
                source_id,
                MatrixProductLeaf::RequirementMismatch,
            ),
        ),
        vec![],
    )
    .unwrap();
    assert!(matches!(
        gameplay_standard::admit_composed_exact_definition::<MatrixProductCodec>(
            &standard_context(&["formula", "requirements-leaf"], &["rules"]),
            requirements_definition,
        ),
        Err(gameplay_standard::ComposedExactError::ProductRequirementMismatch {
            context,
        }) if context.path() == "payload.tree"
            && context.schema() == &MatrixProductCodec::schema()
            && context.kind().as_str() == "matrix.requirements"
            && context.subject() == &requirements_subject
            && context.source().as_str() == "rules"
    ));
}

#[test]
fn composed_wire_requires_product_payloads_to_converge_through_the_codec() {
    let package = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            matrix_extension_payload(),
            matrix_product(
                "matrix.nonconvergent",
                "nonconvergent-leaf",
                serde_json::json!({"mode":"nonconvergent"}),
            ),
            serde_json::json!([]),
            "formula",
            "rules",
        ),
        vec![source("rules")],
        vec![
            provenance("formula", "rules"),
            provenance("nonconvergent-leaf", "rules"),
        ],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<MatrixProductCodec>(&package),
        Err(gameplay_standard::ComposedExactError::ProductNonConvergentPayload {
            context,
        }) if context.path() == "payload.tree"
            && context.schema() == &MatrixProductCodec::schema()
            && context.kind().as_str() == "matrix.nonconvergent"
            && context.subject().as_str() == "nonconvergent-leaf"
            && context.source().as_str() == "rules"
    ));
}

#[test]
fn embedded_composed_exact_selects_one_binary64_aggregate_subtree_with_parent_evidence() {
    let formula = raw_composed_payload(
        gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
        matrix_extension_payload(),
        serde_json::json!({
            "op":"add",
            "left": literal_wire(),
            "right": matrix_product("matrix.valid", "formula-leaf", serde_json::json!({"mode":"valid"})),
        }),
        serde_json::json!([]),
        "formula",
        "rules",
    );
    let package = raw_package(
        RulePackageSchemaVersion::Binary64V2,
        serde_json::json!({"actions":[{"formula":formula,"weight":0.5}]}),
        vec![source("rules")],
        vec![
            provenance("formula", "rules"),
            provenance("formula-leaf", "rules"),
        ],
    );
    let path = RulePayloadPath::new(vec![
        RulePayloadPathSegment::field("actions").unwrap(),
        RulePayloadPathSegment::index(0).unwrap(),
        RulePayloadPathSegment::field("formula").unwrap(),
    ])
    .unwrap();
    let selected = select_rule_payload_subtree(&package, package.fingerprint(), path).unwrap();
    assert_eq!(selected.path().display(), "payload.actions[0].formula");
    assert_eq!(selected.parent_fingerprint(), package.fingerprint());
    assert_eq!(selected.parent_identity(), package.identity());
    assert_eq!(
        selected.canonical_bytes(),
        br#"{"extension":{"namespace":"example.matrix","schemaVersion":1},"family":"composedExact","roles":[],"semanticsVersion":1,"source":"rules","subject":"formula","tree":{"left":{"op":"literal","value":1},"op":"add","right":{"kind":"matrix.valid","op":"product","payload":{"mode":"valid"},"source":"rules","subject":"formula-leaf"}}}"#,
    );
    let compiled =
        gameplay_standard::compile_composed_exact_embedded::<MatrixProductCodec>(&selected)
            .unwrap();
    assert_eq!(
        compiled.evaluate(&ExactInputBundle::empty()).unwrap(),
        scalar(8)
    );
    assert_eq!(compiled.package(), &package);
    assert_eq!(
        compiled.embedded_selection().unwrap().canonical_bytes(),
        selected.canonical_bytes()
    );

    let wrong_path =
        RulePayloadPath::new(vec![RulePayloadPathSegment::field("missing").unwrap()]).unwrap();
    assert!(matches!(
        select_rule_payload_subtree(&package, package.fingerprint(), wrong_path),
        Err(gameplay_rules::RuleSubtreeSelectionError::MissingField { path }) if path == "payload.missing"
    ));
    let wrong_fingerprint =
        RuleFingerprint::parse("0000000000000000000000000000000000000000000000000000000000000000")
            .unwrap();
    assert!(matches!(
        select_rule_payload_subtree(
            &package,
            &wrong_fingerprint,
            RulePayloadPath::new(vec![RulePayloadPathSegment::field("actions").unwrap()]).unwrap(),
        ),
        Err(gameplay_rules::RuleSubtreeSelectionError::ParentFingerprintMismatch { .. })
    ));
}

#[test]
fn fixed_power_converges_through_standalone_and_embedded_composed_routes() {
    let tree = serde_json::json!({
        "op":"fixedPower",
        "base":{"op":"literal","value":1040},
        "exponent":{"op":"literal","value":2},
        "scale":1000,
    });
    let standalone = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            matrix_extension_payload(),
            tree.clone(),
            serde_json::json!([]),
            "formula",
            "rules",
        ),
        vec![source("rules")],
        vec![provenance("formula", "rules")],
    );
    let compiled =
        gameplay_standard::compile_composed_exact_package::<MatrixProductCodec>(&standalone)
            .unwrap();
    assert_eq!(
        compiled.evaluate(&ExactInputBundle::empty()).unwrap().get(),
        1081
    );

    let aggregate = raw_package(
        RulePackageSchemaVersion::Binary64V2,
        serde_json::json!({"formula":raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            matrix_extension_payload(),
            tree,
            serde_json::json!([]),
            "formula",
            "rules",
        )}),
        vec![source("rules")],
        vec![provenance("formula", "rules")],
    );
    let selected = select_rule_payload_subtree(
        &aggregate,
        aggregate.fingerprint(),
        RulePayloadPath::new(vec![RulePayloadPathSegment::field("formula").unwrap()]).unwrap(),
    )
    .unwrap();
    let embedded =
        gameplay_standard::compile_composed_exact_embedded::<MatrixProductCodec>(&selected)
            .unwrap();
    assert_eq!(
        embedded.evaluate(&ExactInputBundle::empty()).unwrap().get(),
        1081
    );

    let invalid_scale = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            matrix_extension_payload(),
            serde_json::json!({
                "op":"fixedPower",
                "base":{"op":"literal","value":1040},
                "exponent":{"op":"literal","value":2},
                "scale":0,
            }),
            serde_json::json!([]),
            "formula",
            "rules",
        ),
        vec![source("rules")],
        vec![provenance("formula", "rules")],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<MatrixProductCodec>(&invalid_scale),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::FixedPowerScaleOutOfRange { .. }
        ))
    ));
}

#[test]
fn embedded_composed_exact_retains_parent_and_path_on_unsafe_binary64_exact_node_failure() {
    let package = raw_package(
        RulePackageSchemaVersion::Binary64V2,
        serde_json::json!({"items":[{"formula":raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            matrix_extension_payload(),
            serde_json::json!({"op":"literal","value":9007199254740992.0}),
            serde_json::json!([]), "formula", "rules") }]}),
        vec![source("rules")],
        vec![provenance("formula", "rules")],
    );
    let selected = select_rule_payload_subtree(
        &package,
        package.fingerprint(),
        RulePayloadPath::new(vec![
            RulePayloadPathSegment::field("items").unwrap(),
            RulePayloadPathSegment::index(0).unwrap(),
            RulePayloadPathSegment::field("formula").unwrap(),
        ])
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        gameplay_standard::compile_composed_exact_embedded::<MatrixProductCodec>(&selected),
        Err(error) if error.context().parent_identity() == package.identity()
            && error.context().parent_fingerprint() == package.fingerprint()
            && error.context().path() == "payload.items[0].formula"
            && matches!(error.error(), gameplay_standard::ComposedExactError::Wire(
                gameplay_standard::ComposedExactDefinitionError::MalformedPayload { path, .. }
            ) if path == "payload.items[0].formula.tree.value")
    ));
}

#[test]
fn embedded_composed_exact_product_compile_failure_uses_the_selected_root_path() {
    let package = raw_package(
        RulePackageSchemaVersion::Binary64V2,
        serde_json::json!({"actions":[{"formula":raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            matrix_extension_payload(),
            matrix_product("matrix.compile", "formula-leaf", serde_json::json!({"mode":"compile"})),
            serde_json::json!([]), "formula", "rules") }]}),
        vec![source("rules")],
        vec![
            provenance("formula", "rules"),
            provenance("formula-leaf", "rules"),
        ],
    );
    let selected = select_rule_payload_subtree(
        &package,
        package.fingerprint(),
        RulePayloadPath::new(vec![
            RulePayloadPathSegment::field("actions").unwrap(),
            RulePayloadPathSegment::index(0).unwrap(),
            RulePayloadPathSegment::field("formula").unwrap(),
        ])
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        gameplay_standard::compile_composed_exact_embedded::<MatrixProductCodec>(&selected),
        Err(error) if matches!(error.error(), gameplay_standard::ComposedExactError::ProductCompile { context, .. }
            if context.path() == "payload.actions[0].formula.tree")
    ));
}

#[test]
fn composed_wire_rejects_wrong_family_schema_and_leaf_provenance() {
    let valid_tree = matrix_product(
        "matrix.valid",
        "valid-leaf",
        serde_json::json!({"mode":"valid"}),
    );
    let valid_roles = serde_json::json!([]);
    let valid_sources = vec![source("rules")];
    let valid_provenance = vec![
        provenance("formula", "rules"),
        provenance("valid-leaf", "rules"),
    ];

    let wrong_family = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            "exact",
            matrix_extension_payload(),
            valid_tree.clone(),
            valid_roles.clone(),
            "formula",
            "rules",
        ),
        valid_sources.clone(),
        valid_provenance.clone(),
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<MatrixProductCodec>(&wrong_family),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::WrongFamily
        ))
    ));

    let wrong_package_schema = raw_package(
        RulePackageSchemaVersion::Binary64V2,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            matrix_extension_payload(),
            valid_tree.clone(),
            valid_roles.clone(),
            "formula",
            "rules",
        ),
        valid_sources.clone(),
        valid_provenance.clone(),
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<MatrixProductCodec>(
            &wrong_package_schema
        ),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::WrongSchema {
                expected: 1,
                actual: 2,
            }
        ))
    ));

    let wrong_extension_schema = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            serde_json::json!({"namespace":"example.other","schemaVersion":2}),
            valid_tree.clone(),
            valid_roles.clone(),
            "formula",
            "rules",
        ),
        valid_sources.clone(),
        valid_provenance.clone(),
    );
    let actual_schema = gameplay_standard::StandardExtensionSchema::new(
        CapabilityRequirementId::parse("example.other").unwrap(),
        2,
    )
    .unwrap();
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<MatrixProductCodec>(
            &wrong_extension_schema
        ),
        Err(gameplay_standard::ComposedExactError::SchemaMismatch { expected, actual })
            if expected == MatrixProductCodec::schema() && actual == actual_schema
    ));

    let missing_provenance = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            matrix_extension_payload(),
            valid_tree.clone(),
            valid_roles.clone(),
            "formula",
            "rules",
        ),
        valid_sources.clone(),
        vec![provenance("formula", "rules")],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<MatrixProductCodec>(
            &missing_provenance
        ),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::MissingCorrelation {
                subject,
                source,
            }
        )) if subject.as_str() == "valid-leaf" && source.as_str() == "rules"
    ));

    let mismatched_provenance = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            matrix_extension_payload(),
            raw_product_tree(
                "matrix.valid",
                "valid-leaf",
                "other",
                serde_json::json!({"mode":"valid"}),
            ),
            valid_roles,
            "formula",
            "rules",
        ),
        vec![source("rules"), source("other")],
        vec![
            provenance("formula", "rules"),
            provenance("valid-leaf", "rules"),
        ],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<MatrixProductCodec>(
            &mismatched_provenance
        ),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::SourceMismatch {
                subject,
                expected,
                actual,
            }
        )) if subject.as_str() == "valid-leaf"
            && expected.as_str() == "other"
            && actual.as_str() == "rules"
    ));
}

#[test]
fn composed_wire_distinguishes_undeclared_roles_from_missing_product_capabilities() {
    let undeclared_role = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            matrix_extension_payload(),
            serde_json::json!({
                "op":"input",
                "input":{"kind":"parameter","role":"rogue","id":"value"}
            }),
            serde_json::json!([]),
            "formula",
            "rules",
        ),
        vec![source("rules")],
        vec![provenance("formula", "rules")],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<MatrixProductCodec>(
            &undeclared_role
        ),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::UndeclaredInputRole { role }
        )) if role.as_str() == "rogue"
    ));

    let missing_capability = raw_package(
        RulePackageSchemaVersion::IntegerOnlyV1,
        raw_composed_payload(
            gameplay_standard::COMPOSED_EXACT_FAMILY_ID,
            serde_json::json!({"namespace":"example.combat","schemaVersion":1}),
            raw_product_tree(
                "combat.equipped-tool",
                "capability-leaf",
                "rules",
                serde_json::json!({"slot":"combat.equipped-tool"}),
            ),
            serde_json::json!([{"role":"attacker","capabilities":[]}]),
            "formula",
            "rules",
        ),
        vec![source("rules")],
        vec![
            provenance("formula", "rules"),
            provenance("capability-leaf", "rules"),
        ],
    );
    assert!(matches!(
        gameplay_standard::compile_composed_exact_package::<NeutralProductCodec>(
            &missing_capability
        ),
        Err(gameplay_standard::ComposedExactError::Wire(
            gameplay_standard::ComposedExactDefinitionError::MissingProductCapability {
                role,
                capability,
            }
        )) if role.as_str() == "attacker" && capability.as_str() == "read.equipped-tool"
    ));
}

#[test]
fn composed_comparisons_merge_both_product_sides_into_canonical_inputs_capabilities_and_evidence() {
    let source_id = RuleSourceId::parse("rules").unwrap();
    let tool_subject = RuleSubjectId::parse("tool-leaf").unwrap();
    let protection_subject = RuleSubjectId::parse("protection-leaf").unwrap();
    let product = |kind: &str,
                   subject: RuleSubjectId,
                   leaf: NeutralProductLeaf|
     -> gameplay_standard::ComposedExactExpr<NeutralProductLeaf> {
        gameplay_standard::ComposedExactExpr::Product(
            gameplay_standard::ComposedExactProductLeaf::new(
                gameplay_standard::ComposedExactLeafKindId::parse(kind).unwrap(),
                subject,
                source_id.clone(),
                leaf,
            ),
        )
    };
    let comparison = gameplay_standard::ComposedExactComparison::GreaterThan(
        gameplay_standard::ComposedExactExpr::Multiply(
            Box::new(product(
                "combat.equipped-tool",
                tool_subject.clone(),
                NeutralProductLeaf::EquippedTool,
            )),
            Box::new(gameplay_standard::ComposedExactExpr::Literal(scalar(2))),
        ),
        gameplay_standard::ComposedExactExpr::Add(
            Box::new(product(
                "combat.protection",
                protection_subject.clone(),
                NeutralProductLeaf::Protection,
            )),
            Box::new(gameplay_standard::ComposedExactExpr::Literal(scalar(1))),
        ),
    );
    let compiled =
        gameplay_standard::compile_composed_exact_comparison::<NeutralProductCodec>(&comparison)
            .unwrap();
    assert_eq!(
        compiled.requirements().inputs(),
        &vec![
            parameter("attacker", "equipped-tool"),
            parameter("defender", "protection"),
        ]
    );
    assert_eq!(
        compiled.product_capabilities(),
        &[
            RoleRequirement::new(
                role("attacker"),
                vec![CapabilityRequirementId::parse("read.equipped-tool").unwrap()],
            )
            .unwrap(),
            RoleRequirement::new(
                role("defender"),
                vec![CapabilityRequirementId::parse("read.protection").unwrap()],
            )
            .unwrap(),
        ]
    );
    assert_eq!(compiled.leaves().len(), 2);
    assert_eq!(
        compiled.leaves()[0].schema(),
        &NeutralProductCodec::schema()
    );
    assert_eq!(compiled.leaves()[0].kind().as_str(), "combat.equipped-tool");
    assert_eq!(compiled.leaves()[0].subject(), &tool_subject);
    assert_eq!(compiled.leaves()[0].source(), &source_id);
    assert_eq!(
        compiled.leaves()[1].schema(),
        &NeutralProductCodec::schema()
    );
    assert_eq!(compiled.leaves()[1].kind().as_str(), "combat.protection");
    assert_eq!(compiled.leaves()[1].subject(), &protection_subject);
    assert_eq!(compiled.leaves()[1].source(), &source_id);
    assert!(ExactEvaluator::evaluate_predicate(
        compiled.comparison(),
        &ExactInputBundle::new(vec![
            (parameter("attacker", "equipped-tool"), scalar(9)),
            (parameter("defender", "protection"), scalar(2)),
        ])
        .expect("distinct input evidence is valid"),
        ExactExprLimits::default(),
    )
    .unwrap());
}

#[test]
fn composed_comparison_merges_same_role_capabilities_independent_of_operand_order() {
    let product = |kind: &str,
                   subject: &str,
                   leaf: SharedCapabilityLeaf|
     -> gameplay_standard::ComposedExactExpr<SharedCapabilityLeaf> {
        gameplay_standard::ComposedExactExpr::Product(
            gameplay_standard::ComposedExactProductLeaf::new(
                gameplay_standard::ComposedExactLeafKindId::parse(kind).unwrap(),
                RuleSubjectId::parse(subject).unwrap(),
                RuleSourceId::parse("rules").unwrap(),
                leaf,
            ),
        )
    };
    let comparison = gameplay_standard::ComposedExactComparison::Equal(
        product("shared.z", "z-leaf", SharedCapabilityLeaf::Z),
        product("shared.a", "a-leaf", SharedCapabilityLeaf::A),
    );
    let compiled =
        gameplay_standard::compile_composed_exact_comparison::<SharedCapabilityCodec>(&comparison)
            .unwrap();
    assert_eq!(compiled.product_capabilities().len(), 1);
    assert_eq!(
        compiled.product_capabilities()[0],
        RoleRequirement::new(
            role("shared"),
            vec![
                CapabilityRequirementId::parse("read.a").unwrap(),
                CapabilityRequirementId::parse("read.z").unwrap(),
            ],
        )
        .unwrap()
    );
}

#[test]
fn composed_comparison_accepts_max_depth_on_each_operand() {
    let limits = ExactExprLimits::default();
    let deep_literal = || {
        let mut expression = gameplay_standard::ComposedExactExpr::Literal(scalar(1));
        for _ in 1..limits.maximum_depth {
            expression = gameplay_standard::ComposedExactExpr::Add(
                Box::new(expression),
                Box::new(gameplay_standard::ComposedExactExpr::Literal(scalar(0))),
            );
        }
        expression
    };
    let comparison =
        gameplay_standard::ComposedExactComparison::Equal(deep_literal(), deep_literal());
    let compiled =
        gameplay_standard::compile_composed_exact_comparison::<NeutralProductCodec>(&comparison)
            .unwrap();
    assert!(compiled.leaves().is_empty());
    assert!(ExactEvaluator::evaluate_predicate(
        compiled.comparison(),
        &ExactInputBundle::empty(),
        limits,
    )
    .unwrap());
}

#[test]
fn composed_typed_leaves_expand_before_the_one_standard_evaluator() {
    let schema = NeutralProductCodec::schema();
    let source = RuleSourceId::parse("rules").unwrap();
    let subject = RuleSubjectId::parse("damage_check").unwrap();
    let tool_subject = RuleSubjectId::parse("tool_leaf").unwrap();
    let protection_subject = RuleSubjectId::parse("protection_leaf").unwrap();
    let leaf = |kind: &str, subject: RuleSubjectId, value| {
        gameplay_standard::ComposedExactExpr::Product(
            gameplay_standard::ComposedExactProductLeaf::new(
                gameplay_standard::ComposedExactLeafKindId::parse(kind).unwrap(),
                subject,
                source.clone(),
                value,
            ),
        )
    };
    let expression = gameplay_standard::ComposedExactExpr::Max(vec![
        gameplay_standard::ComposedExactExpr::Literal(scalar(1)),
        gameplay_standard::ComposedExactExpr::Min(vec![
            gameplay_standard::ComposedExactExpr::FloorDivide(
                Box::new(gameplay_standard::ComposedExactExpr::Multiply(
                    Box::new(leaf(
                        "combat.equipped-tool",
                        tool_subject.clone(),
                        NeutralProductLeaf::EquippedTool,
                    )),
                    Box::new(gameplay_standard::ComposedExactExpr::Literal(scalar(2))),
                )),
                Box::new(gameplay_standard::ComposedExactExpr::Add(
                    Box::new(leaf(
                        "combat.protection",
                        protection_subject.clone(),
                        NeutralProductLeaf::Protection,
                    )),
                    Box::new(gameplay_standard::ComposedExactExpr::Literal(scalar(1))),
                )),
            ),
            gameplay_standard::ComposedExactExpr::Literal(scalar(7)),
        ]),
    ]);
    let roles = vec![
        RoleRequirement::new(
            role("attacker"),
            vec![CapabilityRequirementId::parse("read.equipped-tool").unwrap()],
        )
        .unwrap(),
        RoleRequirement::new(
            role("defender"),
            vec![CapabilityRequirementId::parse("read.protection").unwrap()],
        )
        .unwrap(),
    ];
    let definition = gameplay_standard::ComposedExactDefinition::new(
        schema,
        subject.clone(),
        source.clone(),
        expression,
        roles,
    )
    .unwrap();
    let context = gameplay_standard::StandardPackageContext::new(
        RulePackageSchemaVersion::IntegerOnlyV1,
        RuleDomainId::parse("game").unwrap(),
        RulePackageId::parse("composed").unwrap(),
        RuleVersion::new(1).unwrap(),
        vec![],
        vec![RuleSource::new(source.clone(), "rules.json").unwrap()],
        vec![
            RuleProvenance::new(subject, source.clone(), None, None).unwrap(),
            RuleProvenance::new(tool_subject, source.clone(), None, None).unwrap(),
            RuleProvenance::new(protection_subject, source.clone(), None, None).unwrap(),
        ],
    );
    let admitted = gameplay_standard::admit_composed_exact_definition::<NeutralProductCodec>(
        &context,
        definition.clone(),
    )
    .unwrap();
    let reopened = gameplay_standard::compile_composed_exact_package::<NeutralProductCodec>(
        admitted.package(),
    )
    .unwrap();
    assert_eq!(reopened.definition(), &definition);
    assert_eq!(
        reopened.package().canonical_bytes(),
        admitted.package().canonical_bytes()
    );
    assert_eq!(
        admitted.package().canonical_bytes(),
        include_bytes!(
            "../../../../fixtures/gameplay-standard/composed-exact-schema-1.canonical.json"
        ),
    );
    assert_eq!(
        admitted
            .evaluate(
                &ExactInputBundle::new(vec![
                    (parameter("attacker", "equipped-tool"), scalar(9)),
                    (parameter("defender", "protection"), scalar(2)),
                ])
                .expect("distinct input evidence is valid")
            )
            .unwrap()
            .get(),
        6,
    );
    let comparison = gameplay_standard::ComposedExactComparison::GreaterThan(
        definition.expression().clone(),
        gameplay_standard::ComposedExactExpr::Literal(scalar(5)),
    );
    let comparison =
        gameplay_standard::compile_composed_exact_comparison::<NeutralProductCodec>(&comparison)
            .unwrap();
    assert!(matches!(
        comparison.comparison(),
        gameplay_standard::ExactComparison::GreaterThan(_, _)
    ));
    assert_eq!(comparison.leaves().len(), 2);
}
#[test]
fn typed_product_leaves_compile_to_closed_family_trees() {
    assert_eq!(
        gameplay_standard::compile_exact_expr(&ExactLeaf).unwrap(),
        ExactExpr::Literal(scalar(7))
    );
    assert_eq!(
        gameplay_standard::compile_continuous_expr(&ContinuousLeaf).unwrap(),
        ContinuousExpr::Literal(continuous(7.0))
    );
    assert!(matches!(
        gameplay_standard::compile_exact_expr(&FailingExactLeaf),
        Err(gameplay_standard::ExactCompileError::Product(
            LeafError::MissingCoefficient
        ))
    ));
}

#[test]
fn exact_quota_matrix_accepts_each_limit_and_rejects_one_over() {
    let literal = ExactExpr::Literal(scalar(1));
    let nested = ExactExpr::Add(Box::new(literal.clone()), Box::new(literal.clone()));
    let inputs = ExactExpr::Add(
        Box::new(ExactExpr::Input(ExactInputReference::Parameter {
            role: role("r"),
            id: InputId::parse("a").unwrap(),
        })),
        Box::new(ExactExpr::Input(ExactInputReference::Parameter {
            role: role("r"),
            id: InputId::parse("b").unwrap(),
        })),
    );
    let bundle = ExactInputBundle::new(vec![
        (
            ExactInputReference::Parameter {
                role: role("r"),
                id: InputId::parse("a").unwrap(),
            },
            scalar(1),
        ),
        (
            ExactInputReference::Parameter {
                role: role("r"),
                id: InputId::parse("b").unwrap(),
            },
            scalar(2),
        ),
    ])
    .expect("distinct input evidence is valid");
    let aggregate = ExactExpr::Min(vec![literal.clone(), literal.clone()]);
    let mut limits = ExactExprLimits {
        maximum_depth: 2,
        maximum_nodes: 3,
        maximum_inputs: 2,
        maximum_arity: 2,
        maximum_work: 3,
    };
    assert!(ExactEvaluator::evaluate(&nested, &ExactInputBundle::empty(), limits).is_ok());
    limits.maximum_depth = 1;
    assert!(matches!(
        ExactEvaluator::evaluate(&nested, &ExactInputBundle::empty(), limits),
        Err(gameplay_standard::ExactEvaluationError::DepthExceeded { .. })
    ));
    limits.maximum_depth = 2;
    limits.maximum_nodes = 2;
    assert!(matches!(
        ExactEvaluator::evaluate(&nested, &ExactInputBundle::empty(), limits),
        Err(gameplay_standard::ExactEvaluationError::NodeQuotaExceeded { .. })
    ));
    limits.maximum_nodes = 3;
    limits.maximum_inputs = 1;
    assert!(matches!(
        ExactEvaluator::evaluate(&inputs, &bundle, limits),
        Err(gameplay_standard::ExactEvaluationError::InputQuotaExceeded { .. })
    ));
    limits.maximum_inputs = 2;
    limits.maximum_arity = 1;
    assert!(matches!(
        ExactEvaluator::evaluate(&aggregate, &ExactInputBundle::empty(), limits),
        Err(gameplay_standard::ExactEvaluationError::ArityExceeded { .. })
    ));
    limits.maximum_arity = 2;
    limits.maximum_work = 2;
    assert!(matches!(
        ExactEvaluator::evaluate(&nested, &ExactInputBundle::empty(), limits),
        Err(gameplay_standard::ExactEvaluationError::WorkQuotaExceeded { .. })
    ));
}

#[test]
fn continuous_quota_matrix_accepts_each_limit_and_rejects_one_over() {
    let literal = ContinuousExpr::Literal(continuous(1.0));
    let nested = ContinuousExpr::Add(Box::new(literal.clone()), Box::new(literal.clone()));
    let a = gameplay_standard::ContinuousInputReference::Parameter {
        role: role("r"),
        id: InputId::parse("a").unwrap(),
    };
    let b = gameplay_standard::ContinuousInputReference::Parameter {
        role: role("r"),
        id: InputId::parse("b").unwrap(),
    };
    let inputs = ContinuousExpr::Add(
        Box::new(ContinuousExpr::Input(a.clone())),
        Box::new(ContinuousExpr::Input(b.clone())),
    );
    let bundle = gameplay_standard::ContinuousInputBundle::new(vec![
        (a, continuous(1.0)),
        (b, continuous(2.0)),
    ])
    .unwrap();
    let aggregate = ContinuousExpr::Max(vec![literal.clone(), literal.clone()]);
    let mut limits = ContinuousExprLimits {
        maximum_depth: 2,
        maximum_nodes: 3,
        maximum_inputs: 2,
        maximum_arity: 2,
        maximum_work: 3,
    };
    assert!(ContinuousEvaluator::evaluate(
        &nested,
        &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
        limits
    )
    .is_ok());
    limits.maximum_depth = 1;
    assert!(matches!(
        ContinuousEvaluator::evaluate(
            &nested,
            &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
            limits
        ),
        Err(gameplay_standard::ContinuousEvaluationError::DepthExceeded { .. })
    ));
    limits.maximum_depth = 2;
    limits.maximum_nodes = 2;
    assert!(matches!(
        ContinuousEvaluator::evaluate(
            &nested,
            &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
            limits
        ),
        Err(gameplay_standard::ContinuousEvaluationError::NodeQuotaExceeded { .. })
    ));
    limits.maximum_nodes = 3;
    limits.maximum_inputs = 1;
    assert!(matches!(
        ContinuousEvaluator::evaluate(&inputs, &bundle, limits),
        Err(gameplay_standard::ContinuousEvaluationError::InputQuotaExceeded { .. })
    ));
    limits.maximum_inputs = 2;
    limits.maximum_arity = 1;
    assert!(matches!(
        ContinuousEvaluator::evaluate(
            &aggregate,
            &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
            limits
        ),
        Err(gameplay_standard::ContinuousEvaluationError::ArityExceeded { .. })
    ));
    limits.maximum_arity = 2;
    limits.maximum_work = 2;
    assert!(matches!(
        ContinuousEvaluator::evaluate(
            &nested,
            &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
            limits
        ),
        Err(gameplay_standard::ContinuousEvaluationError::WorkQuotaExceeded { .. })
    ));
}

#[test]
fn continuous_input_bundles_reject_identical_and_conflicting_duplicates() {
    let input = gameplay_standard::ContinuousInputReference::Parameter {
        role: role("continuous"),
        id: InputId::parse("rate").unwrap(),
    };
    let expected = gameplay_standard::ContinuousInputBundleError::DuplicateInput {
        input: input.clone(),
    };

    for observations in [
        vec![
            (input.clone(), continuous(1.0)),
            (input.clone(), continuous(2.0)),
        ],
        vec![
            (input.clone(), continuous(1.0)),
            (input.clone(), continuous(1.0)),
        ],
    ] {
        assert_eq!(
            gameplay_standard::ContinuousInputBundle::new(observations).unwrap_err(),
            expected
        );
    }

    let bundle =
        gameplay_standard::ContinuousInputBundle::new(vec![(input.clone(), continuous(3.0))])
            .expect("one continuous observation is valid");
    assert_eq!(bundle.get(&input), Some(continuous(3.0)));
}

#[test]
fn exact_predicates_use_one_aggregate_budget_for_both_operands() {
    let operand = ExactExpr::Add(
        Box::new(ExactExpr::Literal(scalar(1))),
        Box::new(ExactExpr::Literal(scalar(1))),
    );
    let predicate = gameplay_standard::ExactComparison::Equal(operand.clone(), operand.clone());
    let each = ExactExprLimits {
        maximum_depth: 2,
        maximum_nodes: 3,
        maximum_inputs: 0,
        maximum_arity: 0,
        maximum_work: 3,
    };
    assert!(ExactEvaluator::evaluate(&operand, &ExactInputBundle::empty(), each).is_ok());
    let node_limited = ExactExprLimits {
        maximum_depth: 2,
        maximum_nodes: 5,
        maximum_inputs: 0,
        maximum_arity: 0,
        maximum_work: 6,
    };
    assert!(matches!(
        ExactEvaluator::evaluate_predicate(&predicate, &ExactInputBundle::empty(), node_limited),
        Err(gameplay_standard::ExactEvaluationError::NodeQuotaExceeded { .. })
    ));
    let work_limited = ExactExprLimits {
        maximum_depth: 2,
        maximum_nodes: 6,
        maximum_inputs: 0,
        maximum_arity: 0,
        maximum_work: 5,
    };
    assert!(matches!(
        ExactEvaluator::evaluate_predicate(&predicate, &ExactInputBundle::empty(), work_limited),
        Err(gameplay_standard::ExactEvaluationError::WorkQuotaExceeded { .. })
    ));
    let accepted = ExactExprLimits {
        maximum_depth: 2,
        maximum_nodes: 6,
        maximum_inputs: 0,
        maximum_arity: 0,
        maximum_work: 6,
    };
    assert!(
        ExactEvaluator::evaluate_predicate(&predicate, &ExactInputBundle::empty(), accepted)
            .unwrap()
    );
}

#[test]
fn continuous_predicates_use_one_aggregate_budget_for_both_operands() {
    let operand = ContinuousExpr::Add(
        Box::new(ContinuousExpr::Literal(continuous(1.0))),
        Box::new(ContinuousExpr::Literal(continuous(1.0))),
    );
    let predicate =
        gameplay_standard::ContinuousComparison::Equal(operand.clone(), operand.clone());
    let each = ContinuousExprLimits {
        maximum_depth: 2,
        maximum_nodes: 3,
        maximum_inputs: 0,
        maximum_arity: 0,
        maximum_work: 3,
    };
    assert!(ContinuousEvaluator::evaluate(
        &operand,
        &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
        each
    )
    .is_ok());
    let node_limited = ContinuousExprLimits {
        maximum_depth: 2,
        maximum_nodes: 5,
        maximum_inputs: 0,
        maximum_arity: 0,
        maximum_work: 6,
    };
    assert!(matches!(
        ContinuousEvaluator::evaluate_predicate(
            &predicate,
            &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
            node_limited
        ),
        Err(gameplay_standard::ContinuousEvaluationError::NodeQuotaExceeded { .. })
    ));
    let work_limited = ContinuousExprLimits {
        maximum_depth: 2,
        maximum_nodes: 6,
        maximum_inputs: 0,
        maximum_arity: 0,
        maximum_work: 5,
    };
    assert!(matches!(
        ContinuousEvaluator::evaluate_predicate(
            &predicate,
            &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
            work_limited
        ),
        Err(gameplay_standard::ContinuousEvaluationError::WorkQuotaExceeded { .. })
    ));
    let accepted = ContinuousExprLimits {
        maximum_depth: 2,
        maximum_nodes: 6,
        maximum_inputs: 0,
        maximum_arity: 0,
        maximum_work: 6,
    };
    assert!(ContinuousEvaluator::evaluate_predicate(
        &predicate,
        &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
        accepted
    )
    .unwrap());
}

#[test]
fn continuous_values_normalize_zero_accept_finite_edges_and_order_by_bits() {
    let negative_zero = continuous(-0.0);
    assert_eq!(negative_zero.bits(), 0.0_f64.to_bits());
    assert_eq!(negative_zero, continuous(0.0));
    assert_eq!(continuous(f64::from_bits(1)).bits(), 1);
    assert_eq!(continuous(f64::MAX).bits(), f64::MAX.to_bits());
    assert!(continuous(-1.0) < continuous(0.0));
    assert!(matches!(
        ContinuousValue::new(f64::NAN),
        Err(ContinuousValueError::NonFinite { .. })
    ));
    assert!(matches!(
        ContinuousValue::new(f64::INFINITY),
        Err(ContinuousValueError::NonFinite { .. })
    ));

    let mut left = DefaultHasher::new();
    negative_zero.hash(&mut left);
    let mut right = DefaultHasher::new();
    continuous(0.0).hash(&mut right);
    assert_eq!(left.finish(), right.finish());
}

#[test]
fn continuous_evaluation_preserves_order_and_typed_failures() {
    let underflow = ContinuousExpr::Multiply(
        Box::new(ContinuousExpr::Literal(continuous(f64::from_bits(1)))),
        Box::new(ContinuousExpr::Literal(continuous(0.5))),
    );
    assert_eq!(
        ContinuousEvaluator::evaluate(
            &underflow,
            &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
            ContinuousExprLimits::default()
        )
        .unwrap()
        .bits(),
        0,
    );
    let divide_by_zero = ContinuousExpr::Divide(
        Box::new(ContinuousExpr::Literal(continuous(1.0))),
        Box::new(ContinuousExpr::Literal(continuous(0.0))),
    );
    assert!(matches!(
        ContinuousEvaluator::evaluate(
            &divide_by_zero,
            &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
            ContinuousExprLimits::default()
        ),
        Err(ContinuousEvaluationError::Value(
            ContinuousValueError::DivisionByZero
        ))
    ));
    let min = ContinuousExpr::Min(vec![
        ContinuousExpr::Literal(continuous(3.0)),
        ContinuousExpr::Literal(continuous(1.0)),
        ContinuousExpr::Literal(continuous(2.0)),
    ]);
    assert_eq!(
        ContinuousEvaluator::evaluate(
            &min,
            &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
            ContinuousExprLimits::default()
        )
        .unwrap(),
        continuous(1.0)
    );
    let left_grouped = ContinuousExpr::Add(
        Box::new(ContinuousExpr::Add(
            Box::new(ContinuousExpr::Literal(continuous(1.0e16))),
            Box::new(ContinuousExpr::Literal(continuous(-1.0e16))),
        )),
        Box::new(ContinuousExpr::Literal(continuous(1.0))),
    );
    let right_grouped = ContinuousExpr::Add(
        Box::new(ContinuousExpr::Literal(continuous(1.0e16))),
        Box::new(ContinuousExpr::Add(
            Box::new(ContinuousExpr::Literal(continuous(-1.0e16))),
            Box::new(ContinuousExpr::Literal(continuous(1.0))),
        )),
    );
    assert_eq!(
        ContinuousEvaluator::evaluate(
            &left_grouped,
            &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
            ContinuousExprLimits::default()
        )
        .unwrap(),
        continuous(1.0)
    );
    assert_eq!(
        ContinuousEvaluator::evaluate(
            &right_grouped,
            &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
            ContinuousExprLimits::default()
        )
        .unwrap(),
        continuous(0.0)
    );
    let overflow_before_later_subtraction = ContinuousExpr::Subtract(
        Box::new(ContinuousExpr::Multiply(
            Box::new(ContinuousExpr::Literal(continuous(f64::MAX))),
            Box::new(ContinuousExpr::Literal(continuous(2.0))),
        )),
        Box::new(ContinuousExpr::Literal(continuous(f64::MAX))),
    );
    assert!(matches!(
        ContinuousEvaluator::evaluate(
            &overflow_before_later_subtraction,
            &gameplay_standard::ContinuousInputBundle::new(vec![]).unwrap(),
            ContinuousExprLimits::default()
        ),
        Err(ContinuousEvaluationError::Value(
            ContinuousValueError::NonFinite { .. }
        ))
    ));
}

#[test]
fn exact_floor_and_truncating_division_remain_distinct() {
    let floor = ExactExpr::FloorDivide(
        Box::new(ExactExpr::Literal(scalar(-5))),
        Box::new(ExactExpr::Literal(scalar(2))),
    );
    let truncating = ExactExpr::TruncatingDivide(
        Box::new(ExactExpr::Literal(scalar(-5))),
        Box::new(ExactExpr::Literal(scalar(2))),
    );
    assert_eq!(
        ExactEvaluator::evaluate(
            &floor,
            &ExactInputBundle::empty(),
            ExactExprLimits::default()
        )
        .unwrap()
        .get(),
        -3
    );
    assert_eq!(
        ExactEvaluator::evaluate(
            &truncating,
            &ExactInputBundle::empty(),
            ExactExprLimits::default()
        )
        .unwrap()
        .get(),
        -2
    );
}

#[test]
fn quantization_uses_named_modes_and_preserves_remainder() {
    let source = continuous(-2.5);
    let modes = [
        (ContinuousQuantizationMode::TowardZero, -2),
        (ContinuousQuantizationMode::Floor, -3),
        (ContinuousQuantizationMode::Ceil, -2),
        (ContinuousQuantizationMode::NearestTiesToEven, -2),
    ];
    for (mode, expected) in modes {
        let receipt = quantize_continuous_to_mechanics(source, mode, direct_input()).unwrap();
        assert_eq!(receipt.result().unwrap().get(), expected);
        assert!(receipt.remainder().unwrap().bits() != f64::NAN.to_bits());
        assert!(receipt.remainder().unwrap().get().is_finite());
    }
    for (mode, expected) in [
        (ContinuousQuantizationMode::TowardZero, 2),
        (ContinuousQuantizationMode::Floor, 2),
        (ContinuousQuantizationMode::Ceil, 3),
        (ContinuousQuantizationMode::NearestTiesToEven, 2),
    ] {
        assert_eq!(
            quantize_continuous_to_mechanics(continuous(2.5), mode, direct_input())
                .unwrap()
                .result()
                .unwrap()
                .get(),
            expected
        );
    }
    assert!(quantize_continuous_to_mechanics(
        continuous(1_000_000_000_001.0),
        ContinuousQuantizationMode::TowardZero,
        direct_input()
    )
    .is_err());
    assert_eq!(
        CapabilityRoleId::parse("caster").unwrap().as_str(),
        "caster"
    );
}

#[test]
fn quantization_remainders_stay_in_the_named_mode_intervals() {
    for source in [2.25, -2.25] {
        let toward_zero = quantize_continuous_to_mechanics(
            continuous(source),
            ContinuousQuantizationMode::TowardZero,
            direct_input(),
        )
        .unwrap()
        .remainder()
        .unwrap()
        .get();
        assert!((-1.0..=1.0).contains(&toward_zero));
        let floor = quantize_continuous_to_mechanics(
            continuous(source),
            ContinuousQuantizationMode::Floor,
            direct_input(),
        )
        .unwrap()
        .remainder()
        .unwrap()
        .get();
        assert!((0.0..1.0).contains(&floor));
        let ceil = quantize_continuous_to_mechanics(
            continuous(source),
            ContinuousQuantizationMode::Ceil,
            direct_input(),
        )
        .unwrap()
        .remainder()
        .unwrap()
        .get();
        assert!((-1.0..=0.0).contains(&ceil));
        let nearest = quantize_continuous_to_mechanics(
            continuous(source),
            ContinuousQuantizationMode::NearestTiesToEven,
            direct_input(),
        )
        .unwrap()
        .remainder()
        .unwrap()
        .get();
        assert!((-0.5..=0.5).contains(&nearest));
    }
}

#[test]
fn quantization_direct_input_evidence_keeps_role_and_kind() {
    let id = InputId::parse("same-local-id").unwrap();
    let parameter = ContinuousQuantizationSource::DirectInput {
        input: gameplay_standard::ContinuousInputReference::Parameter {
            role: role("caster"),
            id: id.clone(),
        },
    };
    let fact = ContinuousQuantizationSource::DirectInput {
        input: gameplay_standard::ContinuousInputReference::Fact {
            role: role("target"),
            id,
        },
    };
    assert_ne!(parameter, fact);
    let receipt = quantize_continuous_to_mechanics(
        continuous(4.25),
        ContinuousQuantizationMode::TowardZero,
        parameter.clone(),
    )
    .unwrap();
    assert_eq!(receipt.source(), &parameter);
    assert!(matches!(
        attempt_quantize_continuous_to_mechanics(
            continuous(1_000_000_000_001.0),
            ContinuousQuantizationMode::TowardZero,
            fact.clone()
        ),
        gameplay_standard::ContinuousQuantizationAttempt::Rejected { source, .. } if source == fact
    ));
}

#[test]
fn quantization_records_boundaries_ties_and_failure_without_a_result() {
    for value in [-1_000_000_000_000.0, 1_000_000_000_000.0] {
        let receipt = quantize_continuous_to_mechanics(
            continuous(value),
            ContinuousQuantizationMode::NearestTiesToEven,
            direct_input(),
        )
        .unwrap();
        assert_eq!(receipt.result().unwrap().get(), value as i64);
        assert_eq!(receipt.remainder().unwrap().bits(), 0);
        assert_eq!(receipt.minimum(), -1_000_000_000_000);
        assert_eq!(receipt.maximum(), 1_000_000_000_000);
    }
    for (value, expected) in [(2.5, 2), (3.5, 4), (-3.5, -4)] {
        assert_eq!(
            quantize_continuous_to_mechanics(
                continuous(value),
                ContinuousQuantizationMode::NearestTiesToEven,
                direct_input()
            )
            .unwrap()
            .result()
            .unwrap()
            .get(),
            expected
        );
    }
    for mode in [
        ContinuousQuantizationMode::TowardZero,
        ContinuousQuantizationMode::Floor,
        ContinuousQuantizationMode::Ceil,
        ContinuousQuantizationMode::NearestTiesToEven,
    ] {
        for value in [-1_000_000_000_001.0, 1_000_000_000_001.0] {
            assert!(matches!(
                attempt_quantize_continuous_to_mechanics(continuous(value), mode, direct_input()),
                gameplay_standard::ContinuousQuantizationAttempt::Rejected { .. }
            ));
        }
    }
}

#[test]
fn direct_exact_definition_uses_the_existing_canonical_package_path() {
    let subject = RuleSubjectId::parse("health_formula").unwrap();
    let source = RuleSourceId::parse("rules").unwrap();
    let context = gameplay_standard::StandardPackageContext::new(
        RulePackageSchemaVersion::IntegerOnlyV1,
        RuleDomainId::parse("game").unwrap(),
        RulePackageId::parse("standard").unwrap(),
        RuleVersion::new(1).unwrap(),
        vec![],
        vec![RuleSource::new(source.clone(), "rules.json").unwrap()],
        vec![RuleProvenance::new(subject.clone(), source.clone(), None, None).unwrap()],
    );
    let definition = gameplay_standard::ExactDefinition::new(
        subject,
        source,
        ExactExpr::Add(
            Box::new(ExactExpr::Input(
                gameplay_standard::ExactInputReference::StandardFact(
                    gameplay_standard::StandardExactFactReference::Stat {
                        role: role("self"),
                        stat: gameplay_mechanics::StatId::parse("health").unwrap(),
                    },
                ),
            )),
            Box::new(ExactExpr::Max(vec![
                ExactExpr::Literal(scalar(3)),
                ExactExpr::Multiply(
                    Box::new(ExactExpr::Literal(scalar(2))),
                    Box::new(ExactExpr::Input(parameter("self", "bonus"))),
                ),
            ])),
        ),
        vec![RoleRequirement::new(
            role("self"),
            vec![CapabilityRequirementId::parse("read.stat").unwrap()],
        )
        .unwrap()],
    )
    .unwrap();
    let admitted = gameplay_standard::admit_exact_definition(&context, definition.clone()).unwrap();
    assert_eq!(
        admitted.package().canonical_bytes(),
        include_bytes!("../../../../fixtures/gameplay-standard/exact-schema-1.canonical.json")
    );
    assert_eq!(
        admitted.package().fingerprint().as_str(),
        "ceef579d35d5eef87d68f2f47c44b068a13433b20155d7344ec222b31d10a9c6"
    );
    assert!(admitted.package().canonical_bytes().ends_with(b"\n"));
    let decoded = gameplay_standard::decode_exact_definition(admitted.package()).unwrap();
    assert_eq!(decoded.identity.family(), "exact");
    assert_eq!(decoded.identity.subject().as_str(), "health_formula");
    assert_eq!(decoded.definition, definition);
}

#[test]
fn continuous_definition_uses_schema_two_and_rehydrates_ordered_binary64_tree() {
    let subject = RuleSubjectId::parse("wind_formula").unwrap();
    let source = RuleSourceId::parse("rules").unwrap();
    let context = gameplay_standard::StandardPackageContext::new(
        RulePackageSchemaVersion::Binary64V2,
        RuleDomainId::parse("game").unwrap(),
        RulePackageId::parse("standard").unwrap(),
        RuleVersion::new(2).unwrap(),
        vec![],
        vec![RuleSource::new(source.clone(), "rules.json").unwrap()],
        vec![RuleProvenance::new(subject.clone(), source.clone(), None, None).unwrap()],
    );
    let role = CapabilityRoleId::parse("caster").unwrap();
    let definition = gameplay_standard::ContinuousDefinition::new(
        subject,
        source,
        ContinuousExpr::Subtract(
            Box::new(ContinuousExpr::Add(
                Box::new(ContinuousExpr::Literal(continuous(-0.0))),
                Box::new(ContinuousExpr::Input(
                    gameplay_standard::ContinuousInputReference::Parameter {
                        role: role.clone(),
                        id: InputId::parse("wind").unwrap(),
                    },
                )),
            )),
            Box::new(ContinuousExpr::Literal(continuous(f64::from_bits(1)))),
        ),
        vec![RoleRequirement::new(
            role.clone(),
            vec![CapabilityRequirementId::parse("read.wind").unwrap()],
        )
        .unwrap()],
    )
    .unwrap();
    let admitted =
        gameplay_standard::admit_continuous_definition(&context, definition.clone()).unwrap();
    assert_eq!(
        admitted.package().canonical_bytes(),
        include_bytes!("../../../../fixtures/gameplay-standard/continuous-schema-2.canonical.json")
    );
    assert_eq!(
        admitted.package().fingerprint().as_str(),
        "11ccb782820a3409e78d644855be751dfdccdf354f33dc2b71c40769024b5034"
    );
    let decoded = gameplay_standard::decode_continuous_definition(admitted.package()).unwrap();
    assert_eq!(decoded.identity.family(), "continuous");
    assert_eq!(decoded.identity.subject().as_str(), "wind_formula");
    assert_eq!(decoded.definition, definition);
    assert_eq!(role.as_str(), "caster");
}

#[test]
fn committed_goldens_are_the_cross_language_definition_surface() {
    let exact = decode_canonical_rule_package(include_bytes!(
        "../../../../fixtures/gameplay-standard/exact-schema-1.canonical.json"
    ))
    .unwrap();
    let continuous = decode_canonical_rule_package(include_bytes!(
        "../../../../fixtures/gameplay-standard/continuous-schema-2.canonical.json"
    ))
    .unwrap();
    let numeric = decode_canonical_rule_package(include_bytes!(
        "../../../../fixtures/gameplay-standard/fixed-power-bounded-roll-schema-1.canonical.json"
    ))
    .unwrap();
    assert_eq!(
        gameplay_standard::decode_exact_definition(&exact)
            .unwrap()
            .identity
            .family(),
        "exact"
    );
    assert_eq!(
        gameplay_standard::decode_continuous_definition(&continuous)
            .unwrap()
            .identity
            .family(),
        "continuous"
    );
    let numeric = gameplay_standard::decode_exact_definition(&numeric).unwrap();
    assert!(matches!(
        numeric.definition.expression(),
        ExactExpr::FixedPower(_)
    ));
}

#[derive(Debug, PartialEq, Eq)]
enum ExtensionOutput {
    Guard,
}
#[derive(Debug, PartialEq, Eq)]
enum ExtensionCompileFailure {
    WeightIsProductDefined,
}
impl std::fmt::Display for ExtensionCompileFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("weight is product-defined")
    }
}
impl std::error::Error for ExtensionCompileFailure {}
struct GuardExtensionCompiler {
    schema: gameplay_standard::StandardExtensionSchema,
    reject: bool,
}
impl gameplay_standard::CompileStandardExtension for GuardExtensionCompiler {
    type Output = ExtensionOutput;
    type Error = ExtensionCompileFailure;

    fn schema(&self) -> &gameplay_standard::StandardExtensionSchema {
        &self.schema
    }
    fn compile(
        &self,
        artifact: &gameplay_standard::StandardExtensionArtifact,
    ) -> Result<Self::Output, Self::Error> {
        if self.reject || artifact.kind().as_str() != "combat.option" {
            return Err(ExtensionCompileFailure::WeightIsProductDefined);
        }
        Ok(ExtensionOutput::Guard)
    }
}

#[test]
fn typescript_extension_goldens_rehydrate_and_compile_without_a_runtime_registry() {
    assert!(matches!(
        gameplay_standard::StandardExtensionSchema::new(
            CapabilityRequirementId::parse("example_combat").unwrap(),
            1,
        ),
        Err(gameplay_standard::StandardExtensionError::InvalidNamespace { .. })
    ));
    let too_large_version = admit_rule_package(RulePackageCandidate::new_with_schema(
        RulePackageSchemaVersion::Binary64V2,
        RuleDomainId::parse("game").unwrap(),
        RulePackageId::parse("combat-extension").unwrap(),
        RuleVersion::new(9).unwrap(),
        vec![],
        vec![RuleSource::new(RuleSourceId::parse("rules").unwrap(), "rules.json").unwrap()],
        vec![RuleProvenance::new(
            RuleSubjectId::parse("guard").unwrap(),
            RuleSourceId::parse("rules").unwrap(),
            None,
            None,
        )
        .unwrap()],
        serde_json::json!({"family":"standardExtension","namespace":"example.combat","schemaVersion":4_294_967_296.0,"kind":"combat.option","subject":"guard","source":"rules","payload":null}),
    ))
    .unwrap();
    assert!(matches!(
        gameplay_standard::decode_standard_extension(&too_large_version),
        Err(gameplay_standard::StandardExtensionError::Malformed(
            "schemaVersion exceeds u32"
        ))
    ));
    let schema = gameplay_standard::StandardExtensionSchema::new(
        CapabilityRequirementId::parse("example.combat").unwrap(),
        1,
    )
    .unwrap();
    let schema_one = decode_canonical_rule_package(include_bytes!(
        "../../../../fixtures/gameplay-standard/extension-schema-1.canonical.json"
    ))
    .unwrap();
    let schema_two = decode_canonical_rule_package(include_bytes!(
        "../../../../fixtures/gameplay-standard/extension-schema-2.canonical.json"
    ))
    .unwrap();
    assert_eq!(
        schema_one.fingerprint().as_str(),
        "c7a52b632668beec257686fb913b396f1890ebdba0170c2dd1603c3fb50947df"
    );
    assert_eq!(
        schema_two.fingerprint().as_str(),
        "084f6b21150093408bf9b8d2690a6a68bd084b87eccfb8098c97988d7cdae7a8"
    );
    let extension_one = gameplay_standard::decode_standard_extension(&schema_one).unwrap();
    let extension_two = gameplay_standard::decode_standard_extension(&schema_two).unwrap();
    assert_eq!(extension_one.schema(), &schema);
    assert_eq!(extension_two.schema(), &schema);
    assert_eq!(extension_one.kind().as_str(), "combat.option");
    assert_eq!(extension_two.kind().as_str(), "combat.weight");
    assert_eq!(extension_two.payload(), &serde_json::json!({"weight":1.5}));

    let context = gameplay_standard::StandardPackageContext::new(
        RulePackageSchemaVersion::IntegerOnlyV1,
        RuleDomainId::parse("game").unwrap(),
        RulePackageId::parse("combat-extension").unwrap(),
        RuleVersion::new(1).unwrap(),
        vec![],
        vec![RuleSource::new(RuleSourceId::parse("rules").unwrap(), "rules.json").unwrap()],
        vec![RuleProvenance::new(
            RuleSubjectId::parse("guard").unwrap(),
            RuleSourceId::parse("rules").unwrap(),
            None,
            None,
        )
        .unwrap()],
    );
    let admitted = gameplay_standard::admit_standard_extension(&context, extension_one).unwrap();
    assert_eq!(
        admitted.package().canonical_bytes(),
        include_bytes!("../../../../fixtures/gameplay-standard/extension-schema-1.canonical.json")
    );
    let binary_context = gameplay_standard::StandardPackageContext::new(
        RulePackageSchemaVersion::Binary64V2,
        RuleDomainId::parse("game").unwrap(),
        RulePackageId::parse("combat-extension").unwrap(),
        RuleVersion::new(2).unwrap(),
        vec![],
        vec![RuleSource::new(RuleSourceId::parse("rules").unwrap(), "rules.json").unwrap()],
        vec![RuleProvenance::new(
            RuleSubjectId::parse("guard-weight").unwrap(),
            RuleSourceId::parse("rules").unwrap(),
            None,
            None,
        )
        .unwrap()],
    );
    let admitted_binary =
        gameplay_standard::admit_standard_extension(&binary_context, extension_two).unwrap();
    assert_eq!(
        admitted_binary.package().canonical_bytes(),
        include_bytes!("../../../../fixtures/gameplay-standard/extension-schema-2.canonical.json")
    );
    let compiler = GuardExtensionCompiler {
        schema: schema.clone(),
        reject: false,
    };
    let compiled = gameplay_standard::compile_standard_extension(&admitted, &compiler).unwrap();
    assert_eq!(compiled.output(), &ExtensionOutput::Guard);
    assert_eq!(
        compiled.admitted().package().fingerprint(),
        admitted.package().fingerprint()
    );
    let rejecting = GuardExtensionCompiler {
        schema,
        reject: true,
    };
    assert!(matches!(
        gameplay_standard::compile_standard_extension(&admitted, &rejecting),
        Err(gameplay_standard::StandardExtensionCompileError::Product(
            ExtensionCompileFailure::WeightIsProductDefined
        ))
    ));
}

#[test]
fn package_decoder_rejects_wrong_schema_family_version_and_lexical_underflow() {
    let exact = decode_canonical_rule_package(include_bytes!(
        "../../../../fixtures/gameplay-standard/exact-schema-1.canonical.json"
    ))
    .unwrap();
    assert!(matches!(
        gameplay_standard::decode_continuous_definition(&exact),
        Err(gameplay_standard::StandardDefinitionError::WrongSchema { .. })
    ));
    let subject = RuleSubjectId::parse("wind_formula").unwrap();
    let source = RuleSourceId::parse("rules").unwrap();
    let package = admit_rule_package(RulePackageCandidate::new_with_schema(
        RulePackageSchemaVersion::Binary64V2,
        RuleDomainId::parse("game").unwrap(),
        RulePackageId::parse("standard").unwrap(),
        RuleVersion::new(3).unwrap(),
        vec![],
        vec![RuleSource::new(source.clone(), "rules.json").unwrap()],
        vec![RuleProvenance::new(subject.clone(), source.clone(), None, None).unwrap()],
        serde_json::json!({"family":"exact","semanticsVersion":1,"subject":subject.as_str(),"source":source.as_str(),"roles":[],"tree":{"op":"literal","bits":"0000000000000000"}}),
    )).unwrap();
    assert!(matches!(
        gameplay_standard::decode_continuous_definition(&package),
        Err(gameplay_standard::StandardDefinitionError::WrongFamily { .. })
    ));
    let versioned = admit_rule_package(RulePackageCandidate::new_with_schema(
        RulePackageSchemaVersion::Binary64V2,
        RuleDomainId::parse("game").unwrap(),
        RulePackageId::parse("standard").unwrap(),
        RuleVersion::new(4).unwrap(),
        vec![],
        vec![RuleSource::new(source.clone(), "rules.json").unwrap()],
        vec![RuleProvenance::new(subject.clone(), source, None, None).unwrap()],
        serde_json::json!({"family":"continuous","semanticsVersion":2,"subject":subject.as_str(),"source":"rules","roles":[],"tree":{"op":"literal","bits":"0000000000000000"}}),
    )).unwrap();
    assert!(matches!(
        gameplay_standard::decode_continuous_definition(&versioned),
        Err(gameplay_standard::StandardDefinitionError::UnsupportedSemanticsVersion { .. })
    ));
    let underflow = std::str::from_utf8(include_bytes!(
        "../../../../fixtures/gameplay-standard/continuous-schema-2.canonical.json"
    ))
    .unwrap()
    .replace(
        "\"family\":\"continuous\"",
        "\"family\":\"continuous\",\"probe\":1e-5000",
    );
    assert!(decode_rule_package(underflow.as_bytes()).is_err());

    for (version, bits) in [
        (5, "8000000000000000"),
        (6, "fff0000000000000"),
        (7, "fff8000000000000"),
    ] {
        let source = RuleSourceId::parse("rules").unwrap();
        let rejected = admit_rule_package(RulePackageCandidate::new_with_schema(
            RulePackageSchemaVersion::Binary64V2,
            RuleDomainId::parse("game").unwrap(),
            RulePackageId::parse("standard").unwrap(),
            RuleVersion::new(version).unwrap(),
            vec![],
            vec![RuleSource::new(source.clone(), "rules.json").unwrap()],
            vec![RuleProvenance::new(
                RuleSubjectId::parse("wind_formula").unwrap(),
                source.clone(),
                None,
                None,
            )
            .unwrap()],
            serde_json::json!({"family":"continuous","semanticsVersion":1,"subject":"wind_formula","source":source.as_str(),"roles":[],"tree":{"op":"literal","bits":bits}}),
        ))
        .unwrap();
        assert!(gameplay_standard::decode_continuous_definition(&rejected).is_err());
    }
}
