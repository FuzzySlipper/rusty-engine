use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use gameplay_mechanics::MechanicsScalar;
use gameplay_rules::{
    admit_rule_package, decode_canonical_rule_package, decode_rule_package, RuleDomainId,
    RulePackageCandidate, RulePackageId, RulePackageSchemaVersion, RuleProvenance, RuleSource,
    RuleSourceId, RuleSubjectId, RuleVersion,
};
use gameplay_standard::{
    attempt_quantize_continuous_to_mechanics, quantize_continuous_to_mechanics,
    CapabilityRequirementId, CapabilityRoleId, ContinuousEvaluationError, ContinuousEvaluator,
    ContinuousExpr, ContinuousExprLimits, ContinuousQuantizationMode, ContinuousQuantizationSource,
    ContinuousValue, ContinuousValueError, ExactEvaluator, ExactExpr, ExactExprLimits,
    ExactInputBundle, ExactInputReference, InputId, RoleRequirement,
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
    ]);
    let aggregate = ExactExpr::Min(vec![literal.clone(), literal.clone()]);
    let mut limits = ExactExprLimits {
        maximum_depth: 2,
        maximum_nodes: 3,
        maximum_inputs: 2,
        maximum_arity: 2,
        maximum_work: 3,
    };
    assert!(ExactEvaluator::evaluate(&nested, &ExactInputBundle::new(vec![]), limits).is_ok());
    limits.maximum_depth = 1;
    assert!(matches!(
        ExactEvaluator::evaluate(&nested, &ExactInputBundle::new(vec![]), limits),
        Err(gameplay_standard::ExactEvaluationError::DepthExceeded { .. })
    ));
    limits.maximum_depth = 2;
    limits.maximum_nodes = 2;
    assert!(matches!(
        ExactEvaluator::evaluate(&nested, &ExactInputBundle::new(vec![]), limits),
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
        ExactEvaluator::evaluate(&aggregate, &ExactInputBundle::new(vec![]), limits),
        Err(gameplay_standard::ExactEvaluationError::ArityExceeded { .. })
    ));
    limits.maximum_arity = 2;
    limits.maximum_work = 2;
    assert!(matches!(
        ExactEvaluator::evaluate(&nested, &ExactInputBundle::new(vec![]), limits),
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
    ]);
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
        &gameplay_standard::ContinuousInputBundle::new(vec![]),
        limits
    )
    .is_ok());
    limits.maximum_depth = 1;
    assert!(matches!(
        ContinuousEvaluator::evaluate(
            &nested,
            &gameplay_standard::ContinuousInputBundle::new(vec![]),
            limits
        ),
        Err(gameplay_standard::ContinuousEvaluationError::DepthExceeded { .. })
    ));
    limits.maximum_depth = 2;
    limits.maximum_nodes = 2;
    assert!(matches!(
        ContinuousEvaluator::evaluate(
            &nested,
            &gameplay_standard::ContinuousInputBundle::new(vec![]),
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
            &gameplay_standard::ContinuousInputBundle::new(vec![]),
            limits
        ),
        Err(gameplay_standard::ContinuousEvaluationError::ArityExceeded { .. })
    ));
    limits.maximum_arity = 2;
    limits.maximum_work = 2;
    assert!(matches!(
        ContinuousEvaluator::evaluate(
            &nested,
            &gameplay_standard::ContinuousInputBundle::new(vec![]),
            limits
        ),
        Err(gameplay_standard::ContinuousEvaluationError::WorkQuotaExceeded { .. })
    ));
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
    assert!(ExactEvaluator::evaluate(&operand, &ExactInputBundle::new(vec![]), each).is_ok());
    let node_limited = ExactExprLimits {
        maximum_depth: 2,
        maximum_nodes: 5,
        maximum_inputs: 0,
        maximum_arity: 0,
        maximum_work: 6,
    };
    assert!(matches!(
        ExactEvaluator::evaluate_predicate(
            &predicate,
            &ExactInputBundle::new(vec![]),
            node_limited
        ),
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
        ExactEvaluator::evaluate_predicate(
            &predicate,
            &ExactInputBundle::new(vec![]),
            work_limited
        ),
        Err(gameplay_standard::ExactEvaluationError::WorkQuotaExceeded { .. })
    ));
    let accepted = ExactExprLimits {
        maximum_depth: 2,
        maximum_nodes: 6,
        maximum_inputs: 0,
        maximum_arity: 0,
        maximum_work: 6,
    };
    assert!(ExactEvaluator::evaluate_predicate(
        &predicate,
        &ExactInputBundle::new(vec![]),
        accepted
    )
    .unwrap());
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
        &gameplay_standard::ContinuousInputBundle::new(vec![]),
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
            &gameplay_standard::ContinuousInputBundle::new(vec![]),
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
            &gameplay_standard::ContinuousInputBundle::new(vec![]),
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
        &gameplay_standard::ContinuousInputBundle::new(vec![]),
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
            &gameplay_standard::ContinuousInputBundle::new(vec![]),
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
            &gameplay_standard::ContinuousInputBundle::new(vec![]),
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
            &gameplay_standard::ContinuousInputBundle::new(vec![]),
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
            &gameplay_standard::ContinuousInputBundle::new(vec![]),
            ContinuousExprLimits::default()
        )
        .unwrap(),
        continuous(1.0)
    );
    assert_eq!(
        ContinuousEvaluator::evaluate(
            &right_grouped,
            &gameplay_standard::ContinuousInputBundle::new(vec![]),
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
            &gameplay_standard::ContinuousInputBundle::new(vec![]),
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
            &ExactInputBundle::new(vec![]),
            ExactExprLimits::default()
        )
        .unwrap()
        .get(),
        -3
    );
    assert_eq!(
        ExactEvaluator::evaluate(
            &truncating,
            &ExactInputBundle::new(vec![]),
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
