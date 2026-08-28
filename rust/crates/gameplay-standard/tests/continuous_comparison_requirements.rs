use gameplay_standard::{
    CapabilityRoleId, ContinuousComparison, ContinuousExpr, ContinuousExprRequirements,
    ContinuousInputReference, InputId,
};

fn parameter(role: &str, id: &str) -> ContinuousInputReference {
    ContinuousInputReference::Parameter {
        role: CapabilityRoleId::parse(role).expect("valid role"),
        id: InputId::parse(id).expect("valid input"),
    }
}

#[test]
fn comparison_requirements_include_both_operands_in_stable_order() {
    let shared = parameter("beta", "shared");
    let first = parameter("alpha", "first");
    let last = parameter("alpha", "last");
    let comparison = ContinuousComparison::GreaterThan(
        ContinuousExpr::Add(
            Box::new(ContinuousExpr::Input(shared.clone())),
            Box::new(ContinuousExpr::Input(first.clone())),
        ),
        ContinuousExpr::Multiply(
            Box::new(ContinuousExpr::Input(shared.clone())),
            Box::new(ContinuousExpr::Input(last.clone())),
        ),
    );

    let requirements = ContinuousExprRequirements::inspect_comparison(&comparison)
        .expect("comparison structure is valid");

    assert_eq!(requirements.inputs(), &[first, last, shared]);
}
