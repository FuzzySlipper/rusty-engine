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
pub enum ExactInputReference {
    Parameter { role: CapabilityRoleId, id: InputId },
    Fact { role: CapabilityRoleId, id: InputId },
    Roll { role: CapabilityRoleId, id: InputId },
    Choice { role: CapabilityRoleId, id: InputId },
    StandardFact(StandardExactFactReference),
}
impl ExactInputReference {
    pub fn kind(&self) -> InputKind {
        match self {
            Self::Parameter { .. } => InputKind::Parameter,
            Self::Fact { .. } | Self::StandardFact(_) => InputKind::Fact,
            Self::Roll { .. } => InputKind::Roll,
            Self::Choice { .. } => InputKind::Choice,
        }
    }
    pub fn role(&self) -> &CapabilityRoleId {
        match self {
            Self::Parameter { role, .. }
            | Self::Fact { role, .. }
            | Self::Roll { role, .. }
            | Self::Choice { role, .. } => role,
            Self::StandardFact(fact) => fact.role(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactInputBundle {
    values: BTreeMap<ExactInputReference, MechanicsScalar>,
}
impl ExactInputBundle {
    pub fn new(values: Vec<(ExactInputReference, MechanicsScalar)>) -> Self {
        Self {
            values: values.into_iter().collect(),
        }
    }
    pub fn get(&self, input: &ExactInputReference) -> Option<MechanicsScalar> {
        self.values.get(input).copied()
    }
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
    Min(Vec<Self>),
    Max(Vec<Self>),
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
        let a = Self::evaluate(a, inputs, limits)?;
        let b = Self::evaluate(b, inputs, limits)?;
        Ok(match k {
            0 => a == b,
            1 => a < b,
            2 => a <= b,
            3 => a > b,
            _ => a >= b,
        })
    }
}
fn validate(expr: &ExactExpr, limits: ExactExprLimits) -> Result<(), ExactEvaluationError> {
    let mut nodes = 0;
    let mut inputs = BTreeSet::new();
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
    inputs: &mut BTreeSet<ExactInputReference>,
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
        ExactExpr::Input(input) => {
            inputs.insert(input.clone());
        }
        ExactExpr::Add(a, b)
        | ExactExpr::Subtract(a, b)
        | ExactExpr::Multiply(a, b)
        | ExactExpr::FloorDivide(a, b)
        | ExactExpr::TruncatingDivide(a, b) => {
            validate_node(a, depth + 1, limits, nodes, inputs)?;
            validate_node(b, depth + 1, limits, nodes, inputs)?
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
fn eval(
    expr: &ExactExpr,
    inputs: &ExactInputBundle,
    limits: ExactExprLimits,
    work: &mut usize,
) -> Result<MechanicsScalar, ExactEvaluationError> {
    *work += 1;
    if *work > limits.maximum_work {
        return Err(ExactEvaluationError::WorkQuotaExceeded {
            actual: *work,
            maximum: limits.maximum_work,
        });
    }
    match expr {
        ExactExpr::Literal(value) => Ok(*value),
        ExactExpr::Input(input) => {
            inputs
                .get(input)
                .ok_or_else(|| ExactEvaluationError::MissingInput {
                    input: input.clone(),
                })
        }
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
        ExactExpr::Min(values) => aggregate(values, inputs, limits, work, true),
        ExactExpr::Max(values) => aggregate(values, inputs, limits, work, false),
    }
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
    MissingInput { input: ExactInputReference },
    DepthExceeded { actual: usize, maximum: usize },
    NodeQuotaExceeded { actual: usize, maximum: usize },
    InputQuotaExceeded { actual: usize, maximum: usize },
    ArityExceeded { actual: usize, maximum: usize },
    EmptyAggregate,
    WorkQuotaExceeded { actual: usize, maximum: usize },
}
impl fmt::Display for ExactEvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exact expression evaluation rejected: {self:?}")
    }
}
impl std::error::Error for ExactEvaluationError {}
