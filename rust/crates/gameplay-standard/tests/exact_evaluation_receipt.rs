use gameplay_mechanics::MechanicsScalar;
use gameplay_standard::{
    ExactComparison, ExactEvaluationError, ExactEvaluator, ExactExpr, ExactExprLimits,
    ExactInputBundle,
};

fn scalar(value: i64) -> MechanicsScalar {
    MechanicsScalar::new(value).expect("test scalar is in range")
}

#[test]
fn expression_receipt_reports_the_existing_evaluation_work() {
    let expression = ExactExpr::Add(
        Box::new(ExactExpr::Literal(scalar(2))),
        Box::new(ExactExpr::Multiply(
            Box::new(ExactExpr::Literal(scalar(3))),
            Box::new(ExactExpr::Literal(scalar(4))),
        )),
    );

    let receipt = ExactEvaluator::evaluate_with_receipt(
        &expression,
        &ExactInputBundle::empty(),
        ExactExprLimits::default(),
    )
    .expect("expression evaluates");

    assert_eq!(receipt.value(), scalar(14));
    assert_eq!(receipt.work_used(), 5);
    assert_eq!(
        ExactEvaluator::evaluate(
            &expression,
            &ExactInputBundle::empty(),
            ExactExprLimits::default(),
        )
        .expect("compatibility evaluation succeeds"),
        receipt.value()
    );
}

#[test]
fn predicate_receipt_reports_combined_operand_work() {
    let predicate = ExactComparison::GreaterThan(
        ExactExpr::Add(
            Box::new(ExactExpr::Literal(scalar(4))),
            Box::new(ExactExpr::Literal(scalar(3))),
        ),
        ExactExpr::Multiply(
            Box::new(ExactExpr::Literal(scalar(2))),
            Box::new(ExactExpr::Literal(scalar(3))),
        ),
    );

    let receipt = ExactEvaluator::evaluate_predicate_with_receipt(
        &predicate,
        &ExactInputBundle::empty(),
        ExactExprLimits::default(),
    )
    .expect("predicate evaluates");

    assert!(receipt.value());
    assert_eq!(receipt.work_used(), 6);
    assert_eq!(
        ExactEvaluator::evaluate_predicate(
            &predicate,
            &ExactInputBundle::empty(),
            ExactExprLimits::default(),
        )
        .expect("compatibility predicate evaluation succeeds"),
        receipt.value()
    );
}

#[test]
fn receipt_evaluation_preserves_the_existing_work_quota_error() {
    let expression = ExactExpr::Add(
        Box::new(ExactExpr::Literal(scalar(2))),
        Box::new(ExactExpr::Multiply(
            Box::new(ExactExpr::Literal(scalar(3))),
            Box::new(ExactExpr::Literal(scalar(4))),
        )),
    );
    let limits = ExactExprLimits {
        maximum_work: 4,
        ..ExactExprLimits::default()
    };

    assert_eq!(
        ExactEvaluator::evaluate_with_receipt(&expression, &ExactInputBundle::empty(), limits),
        Err(ExactEvaluationError::WorkQuotaExceeded {
            actual: 5,
            maximum: 4,
        })
    );
}
