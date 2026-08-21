//! Inspector composition for caller-supplied standard planning evidence.
//!
//! This leaf deliberately accepts a completed plan and optional owner-supplied evaluation
//! readouts. It never accepts `EntityState`, a catalog, or an evaluator, so inspection cannot
//! trigger a second evaluation or mutation while formatting a report.

use gameplay_standard::{
    AdmittedDefinitionProjection, ResolutionReceiptProjection, StandardMechanicsReceiptProjection,
    StandardOperationPlan, StandardOperationPlanProjection, StandardOperationProjection,
};

use crate::{MechanicsEvaluationReadoutInspection, MechanicsStructuralEntityInspection};

/// The actual generic resolution receipt projection accepted by standard inspection.
///
/// This alias keeps the product's type parameters visible without permitting an unrelated
/// carrier to stand in for a resolution receipt.
pub type StandardResolutionProjection<
    'a,
    RawIntent,
    Intent,
    Facts,
    Evidence,
    Effect,
    Event,
    Rejection,
    Fault,
    Suspension,
    TraceDetail,
    TransactionError,
> = ResolutionReceiptProjection<
    'a,
    RawIntent,
    Intent,
    Facts,
    Evidence,
    Effect,
    Event,
    Rejection,
    Fault,
    Suspension,
    TraceDetail,
    TransactionError,
>;

/// An optional, still fully typed standard resolution receipt.
pub type OptionalStandardResolutionProjection<
    'a,
    RawIntent,
    Intent,
    Facts,
    Evidence,
    Effect,
    Event,
    Rejection,
    Fault,
    Suspension,
    TraceDetail,
    TransactionError,
> = Option<
    StandardResolutionProjection<
        'a,
        RawIntent,
        Intent,
        Facts,
        Evidence,
        Effect,
        Event,
        Rejection,
        Fault,
        Suspension,
        TraceDetail,
        TransactionError,
    >,
>;

#[derive(Debug, Clone, Copy)]
pub struct StandardInspection<'a> {
    mechanics: &'a MechanicsStructuralEntityInspection,
    plan: StandardOperationPlanProjection<'a>,
    evaluations: Option<&'a MechanicsEvaluationReadoutInspection>,
}

/// A caller-owned product explanation paired with the unaltered standard inspection inputs.
/// The explanation remains typed; inspector composition never serializes it into a generic
/// standard vocabulary or evaluates it.
#[derive(Debug, Clone, Copy)]
pub struct StandardInspectionWithExplanation<'a, Explanation> {
    inspection: StandardInspection<'a>,
    explanation: &'a Explanation,
}

/// Complete borrowed standard evidence composed by the inspection leaf.
///
/// All derived facts are supplied by their owners. In particular, `resolution` retains the
/// product's raw intent, facts, effects, trace detail, and transaction result types unchanged.
#[derive(Debug, Clone, Copy)]
pub struct StandardBorrowedEvidence<
    'a,
    RawIntent,
    Intent,
    Facts,
    Evidence,
    Effect,
    Event,
    Rejection,
    Fault,
    Suspension,
    TraceDetail,
    TransactionError,
    Explanation,
> {
    mechanics: &'a MechanicsStructuralEntityInspection,
    evaluations: Option<&'a MechanicsEvaluationReadoutInspection>,
    parts: StandardBorrowedEvidenceParts<
        'a,
        RawIntent,
        Intent,
        Facts,
        Evidence,
        Effect,
        Event,
        Rejection,
        Fault,
        Suspension,
        TraceDetail,
        TransactionError,
        Explanation,
    >,
}

/// The non-structural inputs for one complete borrowed standard report.
#[derive(Debug, Clone, Copy)]
pub struct StandardBorrowedEvidenceParts<
    'a,
    RawIntent,
    Intent,
    Facts,
    Evidence,
    Effect,
    Event,
    Rejection,
    Fault,
    Suspension,
    TraceDetail,
    TransactionError,
    Explanation,
> {
    pub plan: Option<StandardOperationPlanProjection<'a>>,
    pub operation: Option<StandardOperationProjection<'a>>,
    pub definition: Option<AdmittedDefinitionProjection<'a>>,
    pub mechanics_receipt: Option<StandardMechanicsReceiptProjection<'a>>,
    pub resolution: OptionalStandardResolutionProjection<
        'a,
        RawIntent,
        Intent,
        Facts,
        Evidence,
        Effect,
        Event,
        Rejection,
        Fault,
        Suspension,
        TraceDetail,
        TransactionError,
    >,
    pub explanation: &'a Explanation,
}

impl<
        'a,
        RawIntent,
        Intent,
        Facts,
        Evidence,
        Effect,
        Event,
        Rejection,
        Fault,
        Suspension,
        TraceDetail,
        TransactionError,
        Explanation,
    >
    StandardBorrowedEvidence<
        'a,
        RawIntent,
        Intent,
        Facts,
        Evidence,
        Effect,
        Event,
        Rejection,
        Fault,
        Suspension,
        TraceDetail,
        TransactionError,
        Explanation,
    >
{
    pub const fn mechanics(self) -> &'a MechanicsStructuralEntityInspection {
        self.mechanics
    }
    pub const fn evaluations(self) -> Option<&'a MechanicsEvaluationReadoutInspection> {
        self.evaluations
    }
    pub const fn plan(self) -> Option<StandardOperationPlanProjection<'a>> {
        self.parts.plan
    }
    pub const fn operation(self) -> Option<StandardOperationProjection<'a>> {
        self.parts.operation
    }
    pub const fn definition(self) -> Option<AdmittedDefinitionProjection<'a>> {
        self.parts.definition
    }
    pub const fn mechanics_receipt(self) -> Option<StandardMechanicsReceiptProjection<'a>> {
        self.parts.mechanics_receipt
    }
    pub const fn resolution(
        self,
    ) -> OptionalStandardResolutionProjection<
        'a,
        RawIntent,
        Intent,
        Facts,
        Evidence,
        Effect,
        Event,
        Rejection,
        Fault,
        Suspension,
        TraceDetail,
        TransactionError,
    > {
        self.parts.resolution
    }
    pub const fn explanation(self) -> &'a Explanation {
        self.parts.explanation
    }
}

pub fn inspect_standard_borrowed_evidence<
    'a,
    RawIntent,
    Intent,
    Facts,
    Evidence,
    Effect,
    Event,
    Rejection,
    Fault,
    Suspension,
    TraceDetail,
    TransactionError,
    Explanation,
>(
    mechanics: &'a MechanicsStructuralEntityInspection,
    evaluations: Option<&'a MechanicsEvaluationReadoutInspection>,
    parts: StandardBorrowedEvidenceParts<
        'a,
        RawIntent,
        Intent,
        Facts,
        Evidence,
        Effect,
        Event,
        Rejection,
        Fault,
        Suspension,
        TraceDetail,
        TransactionError,
        Explanation,
    >,
) -> StandardBorrowedEvidence<
    'a,
    RawIntent,
    Intent,
    Facts,
    Evidence,
    Effect,
    Event,
    Rejection,
    Fault,
    Suspension,
    TraceDetail,
    TransactionError,
    Explanation,
> {
    StandardBorrowedEvidence {
        mechanics,
        evaluations,
        parts,
    }
}

impl<'a, Explanation> StandardInspectionWithExplanation<'a, Explanation> {
    pub const fn inspection(self) -> StandardInspection<'a> {
        self.inspection
    }
    pub const fn explanation(self) -> &'a Explanation {
        self.explanation
    }
}

impl<'a> StandardInspection<'a> {
    pub fn mechanics(self) -> &'a MechanicsStructuralEntityInspection {
        self.mechanics
    }
    pub fn plan(self) -> StandardOperationPlanProjection<'a> {
        self.plan
    }
    pub fn evaluations(self) -> Option<&'a MechanicsEvaluationReadoutInspection> {
        self.evaluations
    }

    pub fn to_text(self) -> String {
        let evaluated = self
            .evaluations
            .map_or_else(String::new, MechanicsEvaluationReadoutInspection::to_text);
        format!(
            "gameplay-standard plan observedRevisions={} exactEvaluations={} suppliedEvaluationReadouts={}\n{}{}",
            self.plan.plan().observed_revisions().len(),
            self.plan.plan().exact_evaluations().len(),
            self.evaluations.map_or(0, |value| value.stats.len()),
            evaluated,
            self.mechanics.to_text(),
        )
    }
}

pub fn inspect_standard_plan<'a>(
    mechanics: &'a MechanicsStructuralEntityInspection,
    plan: &'a StandardOperationPlan,
) -> StandardInspection<'a> {
    StandardInspection {
        mechanics,
        plan: StandardOperationPlanProjection(plan),
        evaluations: None,
    }
}

pub fn inspect_standard_plan_with_readouts<'a>(
    mechanics: &'a MechanicsStructuralEntityInspection,
    plan: &'a StandardOperationPlan,
    evaluations: &'a MechanicsEvaluationReadoutInspection,
) -> StandardInspection<'a> {
    StandardInspection {
        mechanics,
        plan: StandardOperationPlanProjection(plan),
        evaluations: Some(evaluations),
    }
}

pub fn inspect_standard_plan_with_explanation<'a, Explanation>(
    mechanics: &'a MechanicsStructuralEntityInspection,
    plan: &'a StandardOperationPlan,
    evaluations: Option<&'a MechanicsEvaluationReadoutInspection>,
    explanation: &'a Explanation,
) -> StandardInspectionWithExplanation<'a, Explanation> {
    StandardInspectionWithExplanation {
        inspection: StandardInspection {
            mechanics,
            plan: StandardOperationPlanProjection(plan),
            evaluations,
        },
        explanation,
    }
}

#[cfg(test)]
mod tests {
    use gameplay_mechanics::{
        CatalogVersion, MechanicsScalar, OperationId, SourceInstanceId, SourceInstanceIdentity,
        TrackAdjustmentKind, TrackMutationReceipt,
    };
    use gameplay_rules::{
        RuleDomainId, RulePackageId, RulePackageSchemaVersion, RuleProvenance, RuleSource,
        RuleSourceId, RuleSubjectId, RuleVersion,
    };
    use gameplay_standard::{
        admit_exact_definition, CapabilityRequirementId, CapabilityRoleId, ExactExpr,
        ExactInputReference, InputId, RoleRequirement, StandardPackageContext,
    };

    use super::*;

    fn scalar(value: i64) -> MechanicsScalar {
        MechanicsScalar::new(value).unwrap()
    }

    #[test]
    fn borrowed_evidence_exposes_operation_requirements_and_admitted_provenance() {
        let role = CapabilityRoleId::parse("actor").unwrap();
        let source = RuleSourceId::parse("source").unwrap();
        let subject = RuleSubjectId::parse("formula").unwrap();
        let context = StandardPackageContext::new(
            RulePackageSchemaVersion::IntegerOnlyV1,
            RuleDomainId::parse("standard.test").unwrap(),
            RulePackageId::parse("projection").unwrap(),
            RuleVersion::new(1).unwrap(),
            vec![],
            vec![RuleSource::new(source.clone(), "rules/projection.json").unwrap()],
            vec![RuleProvenance::new(subject.clone(), source, Some(1), Some(1)).unwrap()],
        );
        let input = ExactInputReference::Parameter {
            role: role.clone(),
            id: InputId::parse("bonus").unwrap(),
        };
        let definition = ExactExpr::Input(input.clone());
        let admitted = admit_exact_definition(
            &context,
            gameplay_standard::ExactDefinition::new(
                subject,
                RuleSourceId::parse("source").unwrap(),
                definition,
                vec![RoleRequirement::new(
                    role.clone(),
                    vec![CapabilityRequirementId::parse("read.bonus").unwrap()],
                )
                .unwrap()],
            )
            .unwrap(),
        )
        .unwrap();
        let operation = gameplay_standard::StandardOperation::SpendTrack {
            role,
            track: gameplay_mechanics::TrackId::parse("actor_resource").unwrap(),
            amount: ExactExpr::Literal(scalar(1)).into(),
        };
        let receipt = gameplay_standard::StandardMechanicsReceipt::Track(TrackMutationReceipt {
            catalog_version: CatalogVersion::parse("projection-test").unwrap(),
            catalog_fingerprint: "fingerprint".to_string(),
            operation: OperationId::parse("projection").unwrap(),
            source: SourceInstanceIdentity::Request {
                operation: OperationId::parse("projection").unwrap(),
                instance: SourceInstanceId::parse("receipt").unwrap(),
            },
            entity: core_ids::EntityId::new(7),
            track: gameplay_mechanics::TrackId::parse("actor_resource").unwrap(),
            kind: TrackAdjustmentKind::Spend,
            requested_amount: scalar(2),
            applied_amount: scalar(2),
            before: scalar(5),
            after: scalar(3),
            minimum: scalar(0),
            maximum: scalar(10),
            observed_tracks_revision: 4,
            committed_tracks_revision: 5,
            observed_revisions: vec![],
            source_cost: Default::default(),
        });
        let mechanics = MechanicsStructuralEntityInspection {
            entity: 7,
            catalog_version: "projection-test".to_string(),
            catalog_fingerprint: "fingerprint".to_string(),
            components: vec![],
            stats: vec![],
            tracks: vec![],
            intrinsic_sources: vec![],
            effects: vec![],
            inventory: None,
            item: None,
            equipment: vec![],
        };
        let evaluations = MechanicsEvaluationReadoutInspection { stats: vec![] };
        let explanation = "product explanation";
        let evidence =
            inspect_standard_borrowed_evidence::<(), (), (), (), (), (), (), (), (), (), (), _>(
                &mechanics,
                Some(&evaluations),
                StandardBorrowedEvidenceParts {
                    plan: None,
                    operation: Some(gameplay_standard::StandardOperationProjection(&operation)),
                    definition: Some(gameplay_standard::AdmittedDefinitionProjection::Exact(
                        &admitted,
                    )),
                    mechanics_receipt: Some(gameplay_standard::StandardMechanicsReceiptProjection(
                        &receipt,
                    )),
                    resolution: None,
                    explanation: &explanation,
                },
            );
        assert_eq!(evidence.mechanics().entity, 7);
        assert!(evidence.evaluations().is_some());
        assert!(trace_access_is_typed(evidence).is_none());
        let operation = evidence.operation().unwrap();
        assert_eq!(operation.requirements()[0].role().as_str(), "actor");
        let definition = evidence.definition().unwrap();
        assert_eq!(definition.identity().subject().as_str(), "formula");
        let package = definition.package();
        assert_eq!(package.provenance()[0].subject().as_str(), "formula");
        assert!(package.canonical_bytes().ends_with(b"\n"));
        assert_eq!(evidence.explanation(), &"product explanation");
        assert_eq!(
            evidence
                .mechanics_receipt()
                .unwrap()
                .track()
                .unwrap()
                .after
                .get(),
            3
        );
        let exact = evidence.definition().unwrap().definition();
        assert_eq!(
            exact.exact_requirements().unwrap().unwrap().inputs(),
            &[input]
        );
    }

    fn trace_access_is_typed<
        'a,
        RawIntent,
        Intent,
        Facts,
        Evidence,
        Effect,
        Event,
        Rejection,
        Fault,
        Suspension,
        TraceDetail,
        TransactionError,
        Explanation,
    >(
        report: StandardBorrowedEvidence<
            'a,
            RawIntent,
            Intent,
            Facts,
            Evidence,
            Effect,
            Event,
            Rejection,
            Fault,
            Suspension,
            TraceDetail,
            TransactionError,
            Explanation,
        >,
    ) -> Option<&'a [gameplay_resolution::ResolutionTraceRecord<TraceDetail>]> {
        report.resolution().map(|receipt| receipt.attempt().trace())
    }

    #[test]
    fn generic_resolution_trace_access_remains_part_of_the_borrowed_contract() {
        let _ = trace_access_is_typed::<(), (), (), (), (), (), (), (), (), (), (), ()>;
    }
}
