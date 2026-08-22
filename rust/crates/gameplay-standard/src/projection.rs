//! Immutable views over standard definitions, supplied planning evidence, and receipts.
//!
//! These are deliberately borrowing projections. Constructing one never evaluates an expression,
//! reads entity state, activates sources, or executes a service. Product-owned extension values
//! remain the generic type parameters of the resolution receipt projection.

use gameplay_resolution::ResolutionReceipt;
use gameplay_rules::AdmittedRulePackage;

use crate::{
    AdmittedContinuousDefinition, AdmittedExactDefinition, ContinuousDefinition, ExactDefinition,
    StandardExactEvaluation, StandardMechanicsReceipt, StandardOperation, StandardOperationPlan,
};

#[derive(Debug, Clone, Copy)]
pub enum StandardDefinitionProjection<'a> {
    Exact(&'a ExactDefinition),
    Continuous(&'a ContinuousDefinition),
}

impl<'a> From<&'a ExactDefinition> for StandardDefinitionProjection<'a> {
    fn from(value: &'a ExactDefinition) -> Self {
        Self::Exact(value)
    }
}
impl<'a> From<&'a ContinuousDefinition> for StandardDefinitionProjection<'a> {
    fn from(value: &'a ContinuousDefinition) -> Self {
        Self::Continuous(value)
    }
}
impl<'a> StandardDefinitionProjection<'a> {
    pub const fn family(self) -> &'static str {
        match self {
            Self::Exact(_) => crate::EXACT_FAMILY_ID,
            Self::Continuous(_) => crate::CONTINUOUS_FAMILY_ID,
        }
    }
    pub fn subject(self) -> &'a gameplay_rules::RuleSubjectId {
        match self {
            Self::Exact(value) => value.subject(),
            Self::Continuous(value) => value.subject(),
        }
    }
    pub fn source(self) -> &'a gameplay_rules::RuleSourceId {
        match self {
            Self::Exact(value) => value.source(),
            Self::Continuous(value) => value.source(),
        }
    }
    pub fn roles(self) -> &'a [crate::RoleRequirement] {
        match self {
            Self::Exact(value) => value.roles(),
            Self::Continuous(value) => value.roles(),
        }
    }
    pub fn exact_requirements(
        self,
    ) -> Option<Result<crate::ExactDefinitionRequirements, crate::StandardDefinitionError>> {
        match self {
            Self::Exact(value) => Some(value.requirements()),
            Self::Continuous(_) => None,
        }
    }
    pub fn continuous_requirements(
        self,
    ) -> Option<Result<crate::ContinuousDefinitionRequirements, crate::StandardDefinitionError>>
    {
        match self {
            Self::Exact(_) => None,
            Self::Continuous(value) => Some(value.requirements()),
        }
    }
}

/// An admitted definition and its canonical package are presented together without decoding or
/// re-admitting either value.
#[derive(Debug, Clone, Copy)]
pub enum AdmittedDefinitionProjection<'a> {
    Exact(&'a AdmittedExactDefinition),
    Continuous(&'a AdmittedContinuousDefinition),
}

impl<'a> AdmittedDefinitionProjection<'a> {
    /// The admitted standard definition, including its declared requirements.
    ///
    /// This is deliberately a borrowed view of the definition that was admitted with the
    /// package; it does not rebuild requirements from package bytes.
    pub fn definition(self) -> StandardDefinitionProjection<'a> {
        match self {
            Self::Exact(value) => StandardDefinitionProjection::Exact(value.definition()),
            Self::Continuous(value) => StandardDefinitionProjection::Continuous(value.definition()),
        }
    }
    pub fn package(self) -> PackageProvenanceProjection<'a> {
        match self {
            Self::Exact(value) => PackageProvenanceProjection(value.package()),
            Self::Continuous(value) => PackageProvenanceProjection(value.package()),
        }
    }
    pub fn identity(self) -> &'a crate::StandardDefinitionIdentity {
        match self {
            Self::Exact(value) => value.identity(),
            Self::Continuous(value) => value.identity(),
        }
    }
}

/// Canonical package provenance retained exactly as admitted by `gameplay-rules`.
#[derive(Debug, Clone, Copy)]
pub struct PackageProvenanceProjection<'a>(pub &'a AdmittedRulePackage);

impl<'a> PackageProvenanceProjection<'a> {
    pub fn package(self) -> &'a AdmittedRulePackage {
        self.0
    }
    pub const fn schema_version(self) -> gameplay_rules::RulePackageSchemaVersion {
        self.0.schema_version()
    }
    pub fn identity(self) -> &'a gameplay_rules::RulePackageIdentity {
        self.0.identity()
    }
    pub fn dependencies(self) -> &'a [gameplay_rules::RulePackageDependency] {
        self.0.dependencies()
    }
    pub fn sources(self) -> &'a [gameplay_rules::RuleSource] {
        self.0.sources()
    }
    pub fn provenance(self) -> &'a [gameplay_rules::RuleProvenance] {
        self.0.provenance()
    }
    pub fn canonical_bytes(self) -> &'a [u8] {
        self.0.canonical_bytes()
    }
    pub const fn fingerprint(self) -> &'a gameplay_rules::RuleFingerprint {
        self.0.fingerprint()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StandardOperationProjection<'a>(pub &'a StandardOperation);
impl<'a> StandardOperationProjection<'a> {
    pub fn operation(self) -> &'a StandardOperation {
        self.0
    }
    pub fn requirements(self) -> Vec<crate::RoleRequirement> {
        self.0.requirements()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StandardExactEvaluationProjection<'a>(pub &'a StandardExactEvaluation);
impl<'a> StandardExactEvaluationProjection<'a> {
    pub fn evaluation(self) -> &'a StandardExactEvaluation {
        self.0
    }
    pub const fn semantics_version(self) -> u32 {
        self.0.semantics_version()
    }
    pub fn definition_identity(self) -> Option<&'a crate::StandardDefinitionIdentity> {
        self.0.definition_identity()
    }
    pub fn expression(self) -> &'a crate::ExactExpr {
        self.0.expression()
    }
    pub fn requirements(self) -> &'a crate::ExactExprRequirements {
        self.0.requirements()
    }
    pub fn inputs(
        self,
    ) -> &'a [(
        crate::ExactInputReference,
        gameplay_mechanics::MechanicsScalar,
    )] {
        self.0.inputs()
    }
    pub const fn result(self) -> gameplay_mechanics::MechanicsScalar {
        self.0.result()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StandardOperationPlanProjection<'a>(pub &'a StandardOperationPlan);
impl<'a> StandardOperationPlanProjection<'a> {
    pub fn plan(self) -> &'a StandardOperationPlan {
        self.0
    }
    pub fn effect(self) -> &'a crate::StandardMechanicsEffect {
        self.0.effect()
    }
    pub fn observed_revisions(self) -> &'a [crate::StandardObservedComponentRevision] {
        self.0.observed_revisions()
    }
    pub fn catalog(self) -> &'a crate::StandardCatalogProvenance {
        self.0.catalog()
    }
    pub fn exact_evaluations(self) -> impl Iterator<Item = StandardExactEvaluationProjection<'a>> {
        self.0
            .exact_evaluations()
            .iter()
            .map(StandardExactEvaluationProjection)
    }
    pub const fn observed_state_revision(self) -> Option<u64> {
        self.0.observed_state_revision()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StandardMechanicsReceiptProjection<'a>(pub &'a StandardMechanicsReceipt);
impl<'a> StandardMechanicsReceiptProjection<'a> {
    pub fn receipt(self) -> &'a StandardMechanicsReceipt {
        self.0
    }
    pub fn track(self) -> Option<&'a gameplay_mechanics::TrackMutationReceipt> {
        match self.0 {
            StandardMechanicsReceipt::Track(value) => Some(value),
            _ => None,
        }
    }
    pub fn damage(self) -> Option<&'a gameplay_mechanics::DamageReceipt> {
        match self.0 {
            StandardMechanicsReceipt::Damage(value) => Some(value),
            _ => None,
        }
    }
    pub fn effect(self) -> Option<&'a gameplay_mechanics::EffectMutationReceipt> {
        match self.0 {
            StandardMechanicsReceipt::Effect(value) => Some(value),
            _ => None,
        }
    }
    pub fn inventory(self) -> Option<&'a gameplay_mechanics::InventoryMutationReceipt> {
        match self.0 {
            StandardMechanicsReceipt::Inventory(value) => Some(value),
            _ => None,
        }
    }
    pub fn inventory_transfer(self) -> Option<&'a gameplay_mechanics::InventoryTransferReceipt> {
        match self.0 {
            StandardMechanicsReceipt::InventoryTransfer(value) => Some(value),
            _ => None,
        }
    }
}

/// A generic borrowed resolution receipt. It preserves typed product intent, facts, effects,
/// events, transaction errors, and trace detail rather than converting them to a standard enum.
#[derive(Debug, Clone, Copy)]
pub struct ResolutionReceiptProjection<
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
>(
    pub  &'a ResolutionReceipt<
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
);

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
    >
    ResolutionReceiptProjection<
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
    >
{
    pub fn receipt(
        self,
    ) -> &'a ResolutionReceipt<
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
        self.0
    }
    pub const fn mode(self) -> gameplay_resolution::ResolutionMode {
        self.0.mode()
    }
    pub fn attempt(
        self,
    ) -> &'a gameplay_resolution::AttemptReceipt<
        RawIntent,
        Intent,
        Facts,
        Evidence,
        Rejection,
        Fault,
        Suspension,
        TraceDetail,
    > {
        self.0.attempt()
    }
    pub fn effects(self) -> &'a [Effect] {
        self.0.effects()
    }
    pub fn events(self) -> &'a [Event] {
        self.0.events()
    }
    pub const fn commit(self) -> &'a gameplay_resolution::CommitStatus<TransactionError> {
        self.0.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gameplay_mechanics::MechanicsScalar;
    use gameplay_rules::{RuleSourceId, RuleSubjectId};

    fn scalar(value: i64) -> MechanicsScalar {
        MechanicsScalar::new(value).unwrap()
    }

    #[test]
    fn definition_projection_only_borrows_the_existing_expression() {
        let definition = ExactDefinition::new(
            RuleSubjectId::parse("projection").unwrap(),
            RuleSourceId::parse("author").unwrap(),
            crate::ExactExpr::Literal(scalar(4)),
            vec![],
        )
        .unwrap();
        let projection = StandardDefinitionProjection::from(&definition);
        assert!(
            matches!(projection, StandardDefinitionProjection::Exact(value) if value.expression() == definition.expression())
        );
    }
}
