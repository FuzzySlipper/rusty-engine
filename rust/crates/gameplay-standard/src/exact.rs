use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::{CapabilityRoleId, InputId, InputKind};
use gameplay_mechanics::{MechanicsArithmeticError, MechanicsScalar, StatId, TrackId};

pub const EXACT_EVALUATOR_SEMANTICS_VERSION: u32 = 1;
pub const MAX_EXACT_EXPRESSION_DEPTH: usize = 32;
pub const MAX_EXACT_EXPRESSION_NODES: usize = 256;
pub const MAX_EXACT_EXPRESSION_INPUTS: usize = 64;
pub const MAX_EXACT_MIN_MAX_ARITY: usize = 16;
pub const MAX_EXACT_EVALUATION_WORK: usize = 512;
pub const MAX_FIXED_POWER_SCALE: i64 = 1_000_000;
pub const MAX_FIXED_POWER_EXPONENT: i64 = 64;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StandardExactFactReference {
    Stat {
        role: CapabilityRoleId,
        stat: StatId,
    },
    TrackCurrent {
        role: CapabilityRoleId,
        track: TrackId,
    },
    TrackMaximum {
        role: CapabilityRoleId,
        track: TrackId,
    },
}
impl StandardExactFactReference {
    pub fn role(&self) -> &CapabilityRoleId {
        match self {
            Self::Stat { role, .. }
            | Self::TrackCurrent { role, .. }
            | Self::TrackMaximum { role, .. } => role,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BoundedRollDescriptor {
    role: CapabilityRoleId,
    id: InputId,
    minimum: MechanicsScalar,
    maximum: MechanicsScalar,
}
impl BoundedRollDescriptor {
    pub fn new(
        role: CapabilityRoleId,
        id: InputId,
        minimum: MechanicsScalar,
        maximum: MechanicsScalar,
    ) -> Self {
        Self {
            role,
            id,
            minimum,
            maximum,
        }
    }
    pub fn role(&self) -> &CapabilityRoleId {
        &self.role
    }
    pub fn id(&self) -> &InputId {
        &self.id
    }
    pub fn minimum(&self) -> MechanicsScalar {
        self.minimum
    }
    pub fn maximum(&self) -> MechanicsScalar {
        self.maximum
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExactInputReference {
    Parameter {
        role: CapabilityRoleId,
        id: InputId,
    },
    Fact {
        role: CapabilityRoleId,
        id: InputId,
    },
    Roll {
        role: CapabilityRoleId,
        id: InputId,
    },
    /// A caller-supplied inclusive integer observation. This is deliberately
    /// not an RNG operation: the engine only validates and consumes its value.
    BoundedRoll {
        descriptor: Box<BoundedRollDescriptor>,
    },
    Choice {
        role: CapabilityRoleId,
        id: InputId,
    },
    StandardFact(StandardExactFactReference),
}
impl ExactInputReference {
    /// Builds a caller-supplied bounded integer observation.
    pub fn bounded_roll(
        role: CapabilityRoleId,
        id: InputId,
        minimum: MechanicsScalar,
        maximum: MechanicsScalar,
    ) -> Self {
        Self::BoundedRoll {
            descriptor: Box::new(BoundedRollDescriptor::new(role, id, minimum, maximum)),
        }
    }

    pub fn kind(&self) -> InputKind {
        match self {
            Self::Parameter { .. } => InputKind::Parameter,
            Self::Fact { .. } | Self::StandardFact(_) => InputKind::Fact,
            Self::Roll { .. } => InputKind::Roll,
            Self::BoundedRoll { .. } => InputKind::BoundedRoll,
            Self::Choice { .. } => InputKind::Choice,
        }
    }
    pub fn role(&self) -> &CapabilityRoleId {
        match self {
            Self::Parameter { role, .. }
            | Self::Fact { role, .. }
            | Self::Roll { role, .. }
            | Self::Choice { role, .. } => role,
            Self::BoundedRoll { descriptor } => descriptor.role(),
            Self::StandardFact(fact) => fact.role(),
        }
    }
}

/// The authored identity of an exact input. A bounded-roll descriptor's
/// range is intentionally *not* part of identity: two different ranges for
/// the same kind, role, and id are contradictory declarations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExactInputIdentity {
    Ordinary {
        kind: InputKind,
        role: CapabilityRoleId,
        id: InputId,
    },
    StandardStat {
        role: CapabilityRoleId,
        stat: StatId,
    },
    StandardTrackCurrent {
        role: CapabilityRoleId,
        track: TrackId,
    },
    StandardTrackMaximum {
        role: CapabilityRoleId,
        track: TrackId,
    },
}
impl ExactInputReference {
    pub fn identity(&self) -> ExactInputIdentity {
        match self {
            Self::Parameter { role, id } => ExactInputIdentity::Ordinary {
                kind: InputKind::Parameter,
                role: role.clone(),
                id: id.clone(),
            },
            Self::Fact { role, id } => ExactInputIdentity::Ordinary {
                kind: InputKind::Fact,
                role: role.clone(),
                id: id.clone(),
            },
            Self::Roll { role, id } => ExactInputIdentity::Ordinary {
                kind: InputKind::Roll,
                role: role.clone(),
                id: id.clone(),
            },
            Self::BoundedRoll { descriptor } => ExactInputIdentity::Ordinary {
                kind: InputKind::BoundedRoll,
                role: descriptor.role().clone(),
                id: descriptor.id().clone(),
            },
            Self::Choice { role, id } => ExactInputIdentity::Ordinary {
                kind: InputKind::Choice,
                role: role.clone(),
                id: id.clone(),
            },
            Self::StandardFact(StandardExactFactReference::Stat { role, stat }) => {
                ExactInputIdentity::StandardStat {
                    role: role.clone(),
                    stat: stat.clone(),
                }
            }
            Self::StandardFact(StandardExactFactReference::TrackCurrent { role, track }) => {
                ExactInputIdentity::StandardTrackCurrent {
                    role: role.clone(),
                    track: track.clone(),
                }
            }
            Self::StandardFact(StandardExactFactReference::TrackMaximum { role, track }) => {
                ExactInputIdentity::StandardTrackMaximum {
                    role: role.clone(),
                    track: track.clone(),
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactInputBundle {
    values: BTreeMap<ExactInputReference, MechanicsScalar>,
}
impl ExactInputBundle {
    /// An explicit empty evidence bundle has no duplicate-or-conflict surface.
    pub fn empty() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    /// Builds an evidence bundle without silently overwriting duplicate input
    /// observations. Identical duplicate pairs are harmless; conflicting
    /// values and contradictory bounded-roll descriptors are rejected.
    pub fn new(
        values: Vec<(ExactInputReference, MechanicsScalar)>,
    ) -> Result<Self, ExactInputBundleError> {
        let mut accepted = BTreeMap::new();
        let mut descriptors = BTreeMap::<ExactInputIdentity, ExactInputReference>::new();
        for (input, value) in values {
            validate_bounded_roll_descriptor(&input)?;
            let identity = input.identity();
            if let Some(existing) = descriptors.get(&identity) {
                if existing != &input {
                    return Err(ExactInputBundleError::ConflictingDescriptor {
                        identity,
                        first: Box::new(existing.clone()),
                        second: Box::new(input),
                    });
                }
            } else {
                descriptors.insert(identity, input.clone());
            }
            if let Some(existing) = accepted.get(&input) {
                if existing != &value {
                    return Err(ExactInputBundleError::ConflictingValue {
                        input,
                        first: *existing,
                        second: value,
                    });
                }
            } else {
                accepted.insert(input, value);
            }
        }
        Ok(Self { values: accepted })
    }
    pub fn get(&self, input: &ExactInputReference) -> Option<MechanicsScalar> {
        self.values.get(input).copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactInputBundleError {
    ConflictingDescriptor {
        identity: ExactInputIdentity,
        first: Box<ExactInputReference>,
        second: Box<ExactInputReference>,
    },
    ConflictingValue {
        input: ExactInputReference,
        first: MechanicsScalar,
        second: MechanicsScalar,
    },
    InvalidBoundedRollDescriptor {
        input: ExactInputReference,
    },
}
impl fmt::Display for ExactInputBundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exact input bundle rejected: {self:?}")
    }
}
impl std::error::Error for ExactInputBundleError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedPowerExpr {
    pub(crate) base: Box<ExactExpr>,
    pub(crate) exponent: Box<ExactExpr>,
    pub(crate) scale: MechanicsScalar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactExpr {
    Literal(MechanicsScalar),
    Input(ExactInputReference),
    Add(Box<Self>, Box<Self>),
    Subtract(Box<Self>, Box<Self>),
    Multiply(Box<Self>, Box<Self>),
    FloorDivide(Box<Self>, Box<Self>),
    TruncatingDivide(Box<Self>, Box<Self>),
    /// Fixed-point exponentiation using the authored positive scale.
    FixedPower(Box<FixedPowerExpr>),
    Min(Vec<Self>),
    Max(Vec<Self>),
}
impl ExactExpr {
    /// Builds a fixed-point exponentiation node without exposing its compact
    /// storage representation to authors.
    pub fn fixed_power(base: Self, exponent: Self, scale: MechanicsScalar) -> Self {
        Self::FixedPower(Box::new(FixedPowerExpr {
            base: Box::new(base),
            exponent: Box::new(exponent),
            scale,
        }))
    }
}
/// Stable, family-specific input requirements discovered from an exact tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactExprRequirements {
    inputs: Vec<ExactInputReference>,
}
impl ExactExprRequirements {
    pub fn inspect(expr: &ExactExpr) -> Result<Self, ExactEvaluationError> {
        ExactEvaluator::validate_structure(expr, ExactExprLimits::default())?;
        let mut inputs = BTreeSet::new();
        collect_inputs(expr, &mut inputs);
        Ok(Self {
            inputs: inputs.into_iter().collect(),
        })
    }
    pub fn inputs(&self) -> &[ExactInputReference] {
        &self.inputs
    }

    /// Inspects both operands with comparison semantics. Unlike wrapping the
    /// operands in an artificial arithmetic node, this preserves the exact
    /// comparison depth rule while still enforcing combined node and input
    /// quotas.
    pub fn inspect_comparison(comparison: &ExactComparison) -> Result<Self, ExactEvaluationError> {
        ExactEvaluator::validate_comparison_structure(comparison, ExactExprLimits::default())?;
        let (left, right) = comparison_operands(comparison);
        let mut inputs = BTreeSet::new();
        collect_inputs(left, &mut inputs);
        collect_inputs(right, &mut inputs);
        Ok(Self {
            inputs: inputs.into_iter().collect(),
        })
    }
}
/// The closed, static compilation seam for a downstream exact product leaf.
pub trait CompileExactExpr {
    type Error: std::error::Error + 'static;

    fn compile_exact_expr(&self) -> Result<ExactExpr, Self::Error>;
}
#[derive(Debug)]
pub enum ExactCompileError<E> {
    Structure(ExactEvaluationError),
    Product(E),
}
impl<E: fmt::Display> fmt::Display for ExactCompileError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Structure(error) => {
                write!(f, "exact product expression has invalid structure: {error}")
            }
            Self::Product(error) => {
                write!(f, "exact product expression compilation failed: {error}")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ExactCompileError<E> {}
pub fn compile_exact_expr<T: CompileExactExpr>(
    leaf: &T,
) -> Result<ExactExpr, ExactCompileError<T::Error>> {
    let expression = leaf
        .compile_exact_expr()
        .map_err(ExactCompileError::Product)?;
    ExactEvaluator::validate_structure(&expression, ExactExprLimits::default())
        .map_err(ExactCompileError::Structure)?;
    Ok(expression)
}
pub(crate) fn collect_inputs(expr: &ExactExpr, inputs: &mut BTreeSet<ExactInputReference>) {
    match expr {
        ExactExpr::Literal(_) => {}
        ExactExpr::Input(input) => {
            inputs.insert(input.clone());
        }
        ExactExpr::Add(a, b)
        | ExactExpr::Subtract(a, b)
        | ExactExpr::Multiply(a, b)
        | ExactExpr::FloorDivide(a, b)
        | ExactExpr::TruncatingDivide(a, b) => {
            collect_inputs(a, inputs);
            collect_inputs(b, inputs);
        }
        ExactExpr::FixedPower(power) => {
            collect_inputs(&power.base, inputs);
            collect_inputs(&power.exponent, inputs);
        }
        ExactExpr::Min(values) | ExactExpr::Max(values) => {
            for value in values {
                collect_inputs(value, inputs);
            }
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactComparison {
    Equal(ExactExpr, ExactExpr),
    LessThan(ExactExpr, ExactExpr),
    LessOrEqual(ExactExpr, ExactExpr),
    GreaterThan(ExactExpr, ExactExpr),
    GreaterOrEqual(ExactExpr, ExactExpr),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactExprLimits {
    pub maximum_depth: usize,
    pub maximum_nodes: usize,
    pub maximum_inputs: usize,
    pub maximum_arity: usize,
    pub maximum_work: usize,
}
impl Default for ExactExprLimits {
    fn default() -> Self {
        Self {
            maximum_depth: MAX_EXACT_EXPRESSION_DEPTH,
            maximum_nodes: MAX_EXACT_EXPRESSION_NODES,
            maximum_inputs: MAX_EXACT_EXPRESSION_INPUTS,
            maximum_arity: MAX_EXACT_MIN_MAX_ARITY,
            maximum_work: MAX_EXACT_EVALUATION_WORK,
        }
    }
}
pub struct ExactEvaluator;
impl ExactEvaluator {
    /// Checks only deterministic tree quotas; it never gathers or evaluates inputs.
    pub fn validate_structure(
        expr: &ExactExpr,
        limits: ExactExprLimits,
    ) -> Result<(), ExactEvaluationError> {
        validate(expr, limits)
    }
    pub fn evaluate(
        expr: &ExactExpr,
        inputs: &ExactInputBundle,
        limits: ExactExprLimits,
    ) -> Result<MechanicsScalar, ExactEvaluationError> {
        Self::validate_structure(expr, limits)?;
        let mut work = 0;
        eval(expr, inputs, limits, &mut work)
    }
    pub fn evaluate_predicate(
        predicate: &ExactComparison,
        inputs: &ExactInputBundle,
        limits: ExactExprLimits,
    ) -> Result<bool, ExactEvaluationError> {
        let (a, b, k) = match predicate {
            ExactComparison::Equal(a, b) => (a, b, 0),
            ExactComparison::LessThan(a, b) => (a, b, 1),
            ExactComparison::LessOrEqual(a, b) => (a, b, 2),
            ExactComparison::GreaterThan(a, b) => (a, b, 3),
            ExactComparison::GreaterOrEqual(a, b) => (a, b, 4),
        };
        validate_predicate(a, b, limits)?;
        let mut work = 0;
        let a = eval(a, inputs, limits, &mut work)?;
        let b = eval(b, inputs, limits, &mut work)?;
        Ok(match k {
            0 => a == b,
            1 => a < b,
            2 => a <= b,
            3 => a > b,
            _ => a >= b,
        })
    }
    /// Checks the combined structural quota of both predicate operands without evaluating.
    pub fn validate_comparison_structure(
        predicate: &ExactComparison,
        limits: ExactExprLimits,
    ) -> Result<(), ExactEvaluationError> {
        let (left, right) = comparison_operands(predicate);
        validate_predicate(left, right, limits)
    }
}
fn comparison_operands(comparison: &ExactComparison) -> (&ExactExpr, &ExactExpr) {
    match comparison {
        ExactComparison::Equal(left, right)
        | ExactComparison::LessThan(left, right)
        | ExactComparison::LessOrEqual(left, right)
        | ExactComparison::GreaterThan(left, right)
        | ExactComparison::GreaterOrEqual(left, right) => (left, right),
    }
}
fn validate_predicate(
    left: &ExactExpr,
    right: &ExactExpr,
    limits: ExactExprLimits,
) -> Result<(), ExactEvaluationError> {
    validate(left, limits)?;
    validate(right, limits)?;
    let nodes = node_count(left) + node_count(right);
    if nodes > limits.maximum_nodes {
        return Err(ExactEvaluationError::NodeQuotaExceeded {
            actual: nodes,
            maximum: limits.maximum_nodes,
        });
    }
    let depth = expression_depth(left).max(expression_depth(right));
    if depth > limits.maximum_depth {
        return Err(ExactEvaluationError::DepthExceeded {
            actual: depth,
            maximum: limits.maximum_depth,
        });
    }
    let mut inputs = BTreeMap::new();
    collect_input_descriptors(left, &mut inputs)?;
    collect_input_descriptors(right, &mut inputs)?;
    if inputs.len() > limits.maximum_inputs {
        return Err(ExactEvaluationError::InputQuotaExceeded {
            actual: inputs.len(),
            maximum: limits.maximum_inputs,
        });
    }
    Ok(())
}
fn collect_input_descriptors(
    expr: &ExactExpr,
    inputs: &mut BTreeMap<ExactInputIdentity, ExactInputReference>,
) -> Result<(), ExactEvaluationError> {
    match expr {
        ExactExpr::Literal(_) => Ok(()),
        ExactExpr::Input(input) => insert_input_descriptor(inputs, input),
        ExactExpr::Add(left, right)
        | ExactExpr::Subtract(left, right)
        | ExactExpr::Multiply(left, right)
        | ExactExpr::FloorDivide(left, right)
        | ExactExpr::TruncatingDivide(left, right) => {
            collect_input_descriptors(left, inputs)?;
            collect_input_descriptors(right, inputs)
        }
        ExactExpr::FixedPower(power) => {
            collect_input_descriptors(&power.base, inputs)?;
            collect_input_descriptors(&power.exponent, inputs)
        }
        ExactExpr::Min(values) | ExactExpr::Max(values) => {
            for value in values {
                collect_input_descriptors(value, inputs)?;
            }
            Ok(())
        }
    }
}
fn validate_bounded_roll_descriptor(
    input: &ExactInputReference,
) -> Result<(), ExactInputBundleError> {
    if let ExactInputReference::BoundedRoll { descriptor } = input {
        if descriptor.minimum() > descriptor.maximum() {
            return Err(ExactInputBundleError::InvalidBoundedRollDescriptor {
                input: input.clone(),
            });
        }
    }
    Ok(())
}
fn node_count(expr: &ExactExpr) -> usize {
    match expr {
        ExactExpr::Literal(_) | ExactExpr::Input(_) => 1,
        ExactExpr::Add(a, b)
        | ExactExpr::Subtract(a, b)
        | ExactExpr::Multiply(a, b)
        | ExactExpr::FloorDivide(a, b)
        | ExactExpr::TruncatingDivide(a, b) => 1 + node_count(a) + node_count(b),
        ExactExpr::FixedPower(power) => 1 + node_count(&power.base) + node_count(&power.exponent),
        ExactExpr::Min(values) | ExactExpr::Max(values) => {
            1 + values.iter().map(node_count).sum::<usize>()
        }
    }
}
fn expression_depth(expr: &ExactExpr) -> usize {
    match expr {
        ExactExpr::Literal(_) | ExactExpr::Input(_) => 1,
        ExactExpr::Add(a, b)
        | ExactExpr::Subtract(a, b)
        | ExactExpr::Multiply(a, b)
        | ExactExpr::FloorDivide(a, b)
        | ExactExpr::TruncatingDivide(a, b) => 1 + expression_depth(a).max(expression_depth(b)),
        ExactExpr::FixedPower(power) => {
            1 + expression_depth(&power.base).max(expression_depth(&power.exponent))
        }
        ExactExpr::Min(values) | ExactExpr::Max(values) => {
            1 + values.iter().map(expression_depth).max().unwrap_or(0)
        }
    }
}
fn validate(expr: &ExactExpr, limits: ExactExprLimits) -> Result<(), ExactEvaluationError> {
    let mut nodes = 0;
    let mut inputs = BTreeMap::new();
    validate_node(expr, 1, limits, &mut nodes, &mut inputs)?;
    if inputs.len() > limits.maximum_inputs {
        return Err(ExactEvaluationError::InputQuotaExceeded {
            actual: inputs.len(),
            maximum: limits.maximum_inputs,
        });
    }
    Ok(())
}
fn validate_node(
    expr: &ExactExpr,
    depth: usize,
    limits: ExactExprLimits,
    nodes: &mut usize,
    inputs: &mut BTreeMap<ExactInputIdentity, ExactInputReference>,
) -> Result<(), ExactEvaluationError> {
    if depth > limits.maximum_depth {
        return Err(ExactEvaluationError::DepthExceeded {
            actual: depth,
            maximum: limits.maximum_depth,
        });
    }
    *nodes += 1;
    if *nodes > limits.maximum_nodes {
        return Err(ExactEvaluationError::NodeQuotaExceeded {
            actual: *nodes,
            maximum: limits.maximum_nodes,
        });
    }
    match expr {
        ExactExpr::Literal(_) => {}
        ExactExpr::Input(input) => insert_input_descriptor(inputs, input)?,
        ExactExpr::Add(a, b)
        | ExactExpr::Subtract(a, b)
        | ExactExpr::Multiply(a, b)
        | ExactExpr::FloorDivide(a, b)
        | ExactExpr::TruncatingDivide(a, b) => {
            validate_node(a, depth + 1, limits, nodes, inputs)?;
            validate_node(b, depth + 1, limits, nodes, inputs)?
        }
        ExactExpr::FixedPower(power) => {
            validate_fixed_power_scale(power.scale)?;
            validate_node(&power.base, depth + 1, limits, nodes, inputs)?;
            validate_node(&power.exponent, depth + 1, limits, nodes, inputs)?;
        }
        ExactExpr::Min(values) | ExactExpr::Max(values) => {
            if values.is_empty() {
                return Err(ExactEvaluationError::EmptyAggregate);
            }
            if values.len() > limits.maximum_arity {
                return Err(ExactEvaluationError::ArityExceeded {
                    actual: values.len(),
                    maximum: limits.maximum_arity,
                });
            }
            for value in values {
                validate_node(value, depth + 1, limits, nodes, inputs)?
            }
        }
    };
    Ok(())
}
fn insert_input_descriptor(
    inputs: &mut BTreeMap<ExactInputIdentity, ExactInputReference>,
    input: &ExactInputReference,
) -> Result<(), ExactEvaluationError> {
    validate_bounded_roll_descriptor(input).map_err(|error| match error {
        ExactInputBundleError::InvalidBoundedRollDescriptor { input } => {
            ExactEvaluationError::BoundedRollInvalidBounds { input }
        }
        _ => unreachable!("bounded-roll descriptor validation returns one error"),
    })?;
    let identity = input.identity();
    if let Some(existing) = inputs.get(&identity) {
        if existing != input {
            return Err(ExactEvaluationError::ConflictingInputDescriptor {
                identity,
                first: Box::new(existing.clone()),
                second: Box::new(input.clone()),
            });
        }
    } else {
        inputs.insert(identity, input.clone());
    }
    Ok(())
}
fn validate_fixed_power_scale(scale: MechanicsScalar) -> Result<(), ExactEvaluationError> {
    if !(1..=MAX_FIXED_POWER_SCALE).contains(&scale.get()) {
        return Err(ExactEvaluationError::FixedPowerScaleOutOfRange { actual: scale });
    }
    Ok(())
}
fn eval(
    expr: &ExactExpr,
    inputs: &ExactInputBundle,
    limits: ExactExprLimits,
    work: &mut usize,
) -> Result<MechanicsScalar, ExactEvaluationError> {
    charge_work(limits, work)?;
    match expr {
        ExactExpr::Literal(value) => Ok(*value),
        ExactExpr::Input(input) => input_value(input, inputs),
        ExactExpr::Add(a, b) => eval(a, inputs, limits, work)?
            .checked_add(eval(b, inputs, limits, work)?)
            .map_err(ExactEvaluationError::Arithmetic),
        ExactExpr::Subtract(a, b) => eval(a, inputs, limits, work)?
            .checked_sub(eval(b, inputs, limits, work)?)
            .map_err(ExactEvaluationError::Arithmetic),
        ExactExpr::Multiply(a, b) => checked_i128(
            i128::from(eval(a, inputs, limits, work)?.get())
                * i128::from(eval(b, inputs, limits, work)?.get()),
        )
        .map_err(ExactEvaluationError::Arithmetic),
        ExactExpr::FloorDivide(a, b) => divide(
            eval(a, inputs, limits, work)?,
            eval(b, inputs, limits, work)?,
            true,
        )
        .map_err(ExactEvaluationError::Arithmetic),
        ExactExpr::TruncatingDivide(a, b) => divide(
            eval(a, inputs, limits, work)?,
            eval(b, inputs, limits, work)?,
            false,
        )
        .map_err(ExactEvaluationError::Arithmetic),
        ExactExpr::FixedPower(power) => {
            let base = eval(&power.base, inputs, limits, work)?;
            let exponent = eval(&power.exponent, inputs, limits, work)?;
            fixed_power(base, exponent, power.scale, limits, work)
        }
        ExactExpr::Min(values) => aggregate(values, inputs, limits, work, true),
        ExactExpr::Max(values) => aggregate(values, inputs, limits, work, false),
    }
}
fn charge_work(limits: ExactExprLimits, work: &mut usize) -> Result<(), ExactEvaluationError> {
    *work += 1;
    if *work > limits.maximum_work {
        return Err(ExactEvaluationError::WorkQuotaExceeded {
            actual: *work,
            maximum: limits.maximum_work,
        });
    }
    Ok(())
}
fn input_value(
    input: &ExactInputReference,
    inputs: &ExactInputBundle,
) -> Result<MechanicsScalar, ExactEvaluationError> {
    let value = match input {
        ExactInputReference::BoundedRoll { .. } => {
            inputs
                .get(input)
                .ok_or_else(|| ExactEvaluationError::MissingBoundedRoll {
                    input: input.clone(),
                })?
        }
        _ => inputs
            .get(input)
            .ok_or_else(|| ExactEvaluationError::MissingInput {
                input: input.clone(),
            })?,
    };
    if let ExactInputReference::BoundedRoll { descriptor } = input {
        if value < descriptor.minimum() || value > descriptor.maximum() {
            return Err(ExactEvaluationError::BoundedRollOutOfRange {
                input: input.clone(),
                value,
            });
        }
    }
    Ok(value)
}
fn fixed_power(
    base: MechanicsScalar,
    exponent: MechanicsScalar,
    scale: MechanicsScalar,
    limits: ExactExprLimits,
    work: &mut usize,
) -> Result<MechanicsScalar, ExactEvaluationError> {
    validate_fixed_power_scale(scale)?;
    if base.get() < 0 {
        return Err(ExactEvaluationError::FixedPowerNegativeBase { actual: base });
    }
    if !(0..=MAX_FIXED_POWER_EXPONENT).contains(&exponent.get()) {
        return Err(ExactEvaluationError::FixedPowerExponentOutOfRange { actual: exponent });
    }
    let mut accumulator = i128::from(scale.get());
    for _ in 0..exponent.get() {
        charge_work(limits, work)?;
        let product = accumulator
            .checked_mul(i128::from(base.get()))
            .ok_or(ExactEvaluationError::FixedPowerMultiplicationOverflow)?;
        let quotient = product / i128::from(scale.get());
        accumulator = i128::from(
            checked_i128(quotient)
                .map_err(|_| ExactEvaluationError::FixedPowerScalarRange { actual: quotient })?
                .get(),
        );
    }
    checked_i128(accumulator).map_err(|_| ExactEvaluationError::FixedPowerScalarRange {
        actual: accumulator,
    })
}
fn checked_i128(value: i128) -> Result<MechanicsScalar, MechanicsArithmeticError> {
    let value = i64::try_from(value).map_err(|_| MechanicsArithmeticError::Overflow)?;
    MechanicsScalar::new(value)
}
fn divide(
    left: MechanicsScalar,
    right: MechanicsScalar,
    floor: bool,
) -> Result<MechanicsScalar, MechanicsArithmeticError> {
    if right.get() == 0 {
        return Err(MechanicsArithmeticError::ZeroDenominator);
    }
    let q = if floor {
        let quotient = left.get() / right.get();
        let remainder = left.get() % right.get();
        if remainder != 0 && (left.get() < 0) != (right.get() < 0) {
            quotient
                .checked_sub(1)
                .ok_or(MechanicsArithmeticError::Overflow)?
        } else {
            quotient
        }
    } else {
        left.get() / right.get()
    };
    MechanicsScalar::new(q)
}
fn aggregate(
    values: &[ExactExpr],
    inputs: &ExactInputBundle,
    limits: ExactExprLimits,
    work: &mut usize,
    min: bool,
) -> Result<MechanicsScalar, ExactEvaluationError> {
    let mut iter = values.iter();
    let mut best = eval(iter.next().expect("validated"), inputs, limits, work)?;
    for value in iter {
        let next = eval(value, inputs, limits, work)?;
        if (min && next < best) || (!min && next > best) {
            best = next;
        }
    }
    Ok(best)
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactEvaluationError {
    Arithmetic(MechanicsArithmeticError),
    MissingInput {
        input: ExactInputReference,
    },
    MissingBoundedRoll {
        input: ExactInputReference,
    },
    BoundedRollOutOfRange {
        input: ExactInputReference,
        value: MechanicsScalar,
    },
    BoundedRollInvalidBounds {
        input: ExactInputReference,
    },
    ConflictingInputDescriptor {
        identity: ExactInputIdentity,
        first: Box<ExactInputReference>,
        second: Box<ExactInputReference>,
    },
    FixedPowerScaleOutOfRange {
        actual: MechanicsScalar,
    },
    FixedPowerNegativeBase {
        actual: MechanicsScalar,
    },
    FixedPowerExponentOutOfRange {
        actual: MechanicsScalar,
    },
    FixedPowerMultiplicationOverflow,
    FixedPowerScalarRange {
        actual: i128,
    },
    DepthExceeded {
        actual: usize,
        maximum: usize,
    },
    NodeQuotaExceeded {
        actual: usize,
        maximum: usize,
    },
    InputQuotaExceeded {
        actual: usize,
        maximum: usize,
    },
    ArityExceeded {
        actual: usize,
        maximum: usize,
    },
    EmptyAggregate,
    WorkQuotaExceeded {
        actual: usize,
        maximum: usize,
    },
}
impl fmt::Display for ExactEvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exact expression evaluation rejected: {self:?}")
    }
}
impl std::error::Error for ExactEvaluationError {}
