use gameplay_standard::{
    ContinuousComparison, ContinuousEvaluationError, ContinuousEvaluator, ContinuousExpr,
    ContinuousExprLimits, ContinuousInputBundle, ContinuousValue,
};

fn value(value: f64) -> ContinuousValue {
    ContinuousValue::new(value).expect("test value is finite")
}

#[test]
fn expression_receipt_reports_the_existing_evaluation_work() {
    let expression = ContinuousExpr::Add(
        Box::new(ContinuousExpr::Literal(value(2.0))),
        Box::new(ContinuousExpr::Multiply(
            Box::new(ContinuousExpr::Literal(value(3.0))),
            Box::new(ContinuousExpr::Literal(value(4.0))),
        )),
    );

    let receipt = ContinuousEvaluator::evaluate_with_receipt(
        &expression,
        &ContinuousInputBundle::new(vec![]).expect("empty input bundle"),
        ContinuousExprLimits::default(),
    )
    .expect("expression evaluates");

    assert_eq!(receipt.value(), value(14.0));
    assert_eq!(receipt.work_used(), 5);
    assert_eq!(
        ContinuousEvaluator::evaluate(
            &expression,
            &ContinuousInputBundle::new(vec![]).expect("empty input bundle"),
            ContinuousExprLimits::default(),
        )
        .expect("compatibility evaluation succeeds"),
        receipt.value()
    );
}

#[test]
fn predicate_receipt_reports_combined_operand_work() {
    let predicate = ContinuousComparison::GreaterThan(
        ContinuousExpr::Add(
            Box::new(ContinuousExpr::Literal(value(4.0))),
            Box::new(ContinuousExpr::Literal(value(3.0))),
        ),
        ContinuousExpr::Multiply(
            Box::new(ContinuousExpr::Literal(value(2.0))),
            Box::new(ContinuousExpr::Literal(value(3.0))),
        ),
    );

    let receipt = ContinuousEvaluator::evaluate_predicate_with_receipt(
        &predicate,
        &ContinuousInputBundle::new(vec![]).expect("empty input bundle"),
        ContinuousExprLimits::default(),
    )
    .expect("predicate evaluates");

    assert!(receipt.value());
    assert_eq!(receipt.work_used(), 6);
    assert_eq!(
        ContinuousEvaluator::evaluate_predicate(
            &predicate,
            &ContinuousInputBundle::new(vec![]).expect("empty input bundle"),
            ContinuousExprLimits::default(),
        )
        .expect("compatibility predicate evaluation succeeds"),
        receipt.value()
    );
}

#[test]
fn receipt_evaluation_preserves_the_existing_work_quota_error() {
    let expression = ContinuousExpr::Add(
        Box::new(ContinuousExpr::Literal(value(2.0))),
        Box::new(ContinuousExpr::Multiply(
            Box::new(ContinuousExpr::Literal(value(3.0))),
            Box::new(ContinuousExpr::Literal(value(4.0))),
        )),
    );
    let limits = ContinuousExprLimits {
        maximum_work: 4,
        ..ContinuousExprLimits::default()
    };

    assert_eq!(
        ContinuousEvaluator::evaluate_with_receipt(
            &expression,
            &ContinuousInputBundle::new(vec![]).expect("empty input bundle"),
            limits,
        ),
        Err(ContinuousEvaluationError::WorkQuotaExceeded {
            actual: 5,
            maximum: 4,
        })
    );
}
