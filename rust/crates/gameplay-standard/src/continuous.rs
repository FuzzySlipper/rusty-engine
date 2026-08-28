use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fmt,
    hash::{Hash, Hasher},
};

use crate::{CapabilityRoleId, InputId, InputKind, RoleRequirementError};

pub const CONTINUOUS_EVALUATOR_SEMANTICS_VERSION: u32 = 1;
pub const MAX_CONTINUOUS_EXPRESSION_DEPTH: usize = 32;
pub const MAX_CONTINUOUS_EXPRESSION_NODES: usize = 256;
pub const MAX_CONTINUOUS_EXPRESSION_INPUTS: usize = 64;
pub const MAX_CONTINUOUS_MIN_MAX_ARITY: usize = 16;
pub const MAX_CONTINUOUS_EVALUATION_WORK: usize = 512;

#[derive(Debug, Clone, Copy)]
pub struct ContinuousValue(f64);

impl ContinuousValue {
    pub fn new(value: f64) -> Result<Self, ContinuousValueError> {
        if !value.is_finite() {
            return Err(ContinuousValueError::NonFinite {
                bits: value.to_bits(),
            });
        }
        Ok(Self(if value == 0.0 { 0.0 } else { value }))
    }
    pub fn from_bits(bits: u64) -> Result<Self, ContinuousValueError> {
        Self::new(f64::from_bits(bits))
    }
    pub const fn bits(self) -> u64 {
        self.0.to_bits()
    }
    /// Returns the admitted finite binary64 value.
    pub const fn get(self) -> f64 {
        self.0
    }
    pub(crate) const fn raw(self) -> f64 {
        self.0
    }
    pub fn checked_add(self, other: Self) -> Result<Self, ContinuousValueError> {
        Self::new(self.0 + other.0)
    }
    pub fn checked_sub(self, other: Self) -> Result<Self, ContinuousValueError> {
        Self::new(self.0 - other.0)
    }
    pub fn checked_mul(self, other: Self) -> Result<Self, ContinuousValueError> {
        Self::new(self.0 * other.0)
    }
    pub fn checked_div(self, other: Self) -> Result<Self, ContinuousValueError> {
        if other.0 == 0.0 {
            return Err(ContinuousValueError::DivisionByZero);
        }
        Self::new(self.0 / other.0)
    }
}
impl PartialEq for ContinuousValue {
    fn eq(&self, other: &Self) -> bool {
        self.bits() == other.bits()
    }
}
impl Eq for ContinuousValue {}
impl Hash for ContinuousValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bits().hash(state);
    }
}
impl PartialOrd for ContinuousValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ContinuousValue {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuousValueError {
    NonFinite { bits: u64 },
    DivisionByZero,
}
impl fmt::Display for ContinuousValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid continuous value: {self:?}")
    }
}
impl std::error::Error for ContinuousValueError {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContinuousInputReference {
    Parameter { role: CapabilityRoleId, id: InputId },
    Fact { role: CapabilityRoleId, id: InputId },
    Roll { role: CapabilityRoleId, id: InputId },
    Choice { role: CapabilityRoleId, id: InputId },
}
impl ContinuousInputReference {
    pub fn kind(&self) -> InputKind {
        match self {
            Self::Parameter { .. } => InputKind::Parameter,
            Self::Fact { .. } => InputKind::Fact,
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousInputBundle {
    values: BTreeMap<ContinuousInputReference, ContinuousValue>,
}
impl ContinuousInputBundle {
    /// Builds a continuous input bundle without silently overwriting duplicate
    /// observations. Each input reference has one unambiguous value, so both
    /// identical and conflicting duplicate observations are rejected.
    pub fn new(
        values: Vec<(ContinuousInputReference, ContinuousValue)>,
    ) -> Result<Self, ContinuousInputBundleError> {
        let mut accepted = BTreeMap::new();
        for (input, value) in values {
            if accepted.contains_key(&input) {
                return Err(ContinuousInputBundleError::DuplicateInput { input });
            }
            accepted.insert(input, value);
        }
        Ok(Self { values: accepted })
    }
    pub fn get(&self, input: &ContinuousInputReference) -> Option<ContinuousValue> {
        self.values.get(input).copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuousInputBundleError {
    DuplicateInput { input: ContinuousInputReference },
}
impl fmt::Display for ContinuousInputBundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "continuous input bundle rejected: {self:?}")
    }
}
impl std::error::Error for ContinuousInputBundleError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuousExpr {
    Literal(ContinuousValue),
    Input(ContinuousInputReference),
    Add(Box<Self>, Box<Self>),
    Subtract(Box<Self>, Box<Self>),
    Multiply(Box<Self>, Box<Self>),
    Divide(Box<Self>, Box<Self>),
    Min(Vec<Self>),
    Max(Vec<Self>),
}
/// Stable, family-specific input requirements discovered from a continuous tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousExprRequirements {
    inputs: Vec<ContinuousInputReference>,
}
impl ContinuousExprRequirements {
    pub fn inspect(expr: &ContinuousExpr) -> Result<Self, ContinuousEvaluationError> {
        ContinuousEvaluator::validate_structure(expr, ContinuousExprLimits::default())?;
        let mut inputs = BTreeSet::new();
        collect_inputs(expr, &mut inputs);
        Ok(Self {
            inputs: inputs.into_iter().collect(),
        })
    }
    pub fn inputs(&self) -> &[ContinuousInputReference] {
        &self.inputs
    }
}
/// The closed, static compilation seam for a downstream continuous product leaf.
pub trait CompileContinuousExpr {
    type Error: std::error::Error + 'static;

    fn compile_continuous_expr(&self) -> Result<ContinuousExpr, Self::Error>;
}
#[derive(Debug)]
pub enum ContinuousCompileError<E> {
    Structure(ContinuousEvaluationError),
    Product(E),
}
impl<E: fmt::Display> fmt::Display for ContinuousCompileError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Structure(error) => write!(
                f,
                "continuous product expression has invalid structure: {error}"
            ),
            Self::Product(error) => write!(
                f,
                "continuous product expression compilation failed: {error}"
            ),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ContinuousCompileError<E> {}
pub fn compile_continuous_expr<T: CompileContinuousExpr>(
    leaf: &T,
) -> Result<ContinuousExpr, ContinuousCompileError<T::Error>> {
    let expression = leaf
        .compile_continuous_expr()
        .map_err(ContinuousCompileError::Product)?;
    ContinuousEvaluator::validate_structure(&expression, ContinuousExprLimits::default())
        .map_err(ContinuousCompileError::Structure)?;
    Ok(expression)
}
pub(crate) fn collect_inputs(
    expr: &ContinuousExpr,
    inputs: &mut BTreeSet<ContinuousInputReference>,
) {
    match expr {
        ContinuousExpr::Literal(_) => {}
        ContinuousExpr::Input(input) => {
            inputs.insert(input.clone());
        }
        ContinuousExpr::Add(a, b)
        | ContinuousExpr::Subtract(a, b)
        | ContinuousExpr::Multiply(a, b)
        | ContinuousExpr::Divide(a, b) => {
            collect_inputs(a, inputs);
            collect_inputs(b, inputs);
        }
        ContinuousExpr::Min(values) | ContinuousExpr::Max(values) => {
            for value in values {
                collect_inputs(value, inputs);
            }
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuousComparison {
    Equal(ContinuousExpr, ContinuousExpr),
    LessThan(ContinuousExpr, ContinuousExpr),
    LessOrEqual(ContinuousExpr, ContinuousExpr),
    GreaterThan(ContinuousExpr, ContinuousExpr),
    GreaterOrEqual(ContinuousExpr, ContinuousExpr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContinuousExprLimits {
    pub maximum_depth: usize,
    pub maximum_nodes: usize,
    pub maximum_inputs: usize,
    pub maximum_arity: usize,
    pub maximum_work: usize,
}
impl Default for ContinuousExprLimits {
    fn default() -> Self {
        Self {
            maximum_depth: MAX_CONTINUOUS_EXPRESSION_DEPTH,
            maximum_nodes: MAX_CONTINUOUS_EXPRESSION_NODES,
            maximum_inputs: MAX_CONTINUOUS_EXPRESSION_INPUTS,
            maximum_arity: MAX_CONTINUOUS_MIN_MAX_ARITY,
            maximum_work: MAX_CONTINUOUS_EVALUATION_WORK,
        }
    }
}

pub struct ContinuousEvaluator;

/// The deterministic result and work consumed by one continuous expression evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContinuousEvaluationReceipt {
    value: ContinuousValue,
    work_used: usize,
}
impl ContinuousEvaluationReceipt {
    /// Returns the evaluated finite binary64 value.
    pub fn value(self) -> ContinuousValue {
        self.value
    }

    /// Returns the evaluator work consumed while producing this result.
    pub fn work_used(self) -> usize {
        self.work_used
    }
}

/// The deterministic result and work consumed by one continuous predicate evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContinuousPredicateEvaluationReceipt {
    value: bool,
    work_used: usize,
}
impl ContinuousPredicateEvaluationReceipt {
    /// Returns the evaluated predicate value.
    pub fn value(self) -> bool {
        self.value
    }

    /// Returns the evaluator work consumed while producing this result.
    pub fn work_used(self) -> usize {
        self.work_used
    }
}

impl ContinuousEvaluator {
    /// Checks only deterministic tree quotas; it never gathers or evaluates inputs.
    pub fn validate_structure(
        expr: &ContinuousExpr,
        limits: ContinuousExprLimits,
    ) -> Result<(), ContinuousEvaluationError> {
        validate(expr, limits)
    }
    pub fn evaluate(
        expr: &ContinuousExpr,
        inputs: &ContinuousInputBundle,
        limits: ContinuousExprLimits,
    ) -> Result<ContinuousValue, ContinuousEvaluationError> {
        Ok(Self::evaluate_with_receipt(expr, inputs, limits)?.value())
    }

    /// Evaluates an expression and reports the same work counter used to enforce its quota.
    pub fn evaluate_with_receipt(
        expr: &ContinuousExpr,
        inputs: &ContinuousInputBundle,
        limits: ContinuousExprLimits,
    ) -> Result<ContinuousEvaluationReceipt, ContinuousEvaluationError> {
        Self::validate_structure(expr, limits)?;
        let mut work = 0;
        let value = eval(expr, inputs, limits, &mut work)?;
        Ok(ContinuousEvaluationReceipt {
            value,
            work_used: work,
        })
    }

    pub fn evaluate_predicate(
        predicate: &ContinuousComparison,
        inputs: &ContinuousInputBundle,
        limits: ContinuousExprLimits,
    ) -> Result<bool, ContinuousEvaluationError> {
        Ok(Self::evaluate_predicate_with_receipt(predicate, inputs, limits)?.value())
    }

    /// Evaluates a predicate and reports the same work counter used to enforce its quota.
    pub fn evaluate_predicate_with_receipt(
        predicate: &ContinuousComparison,
        inputs: &ContinuousInputBundle,
        limits: ContinuousExprLimits,
    ) -> Result<ContinuousPredicateEvaluationReceipt, ContinuousEvaluationError> {
        let (left, right, comparison) = match predicate {
            ContinuousComparison::Equal(a, b) => (a, b, 0),
            ContinuousComparison::LessThan(a, b) => (a, b, 1),
            ContinuousComparison::LessOrEqual(a, b) => (a, b, 2),
            ContinuousComparison::GreaterThan(a, b) => (a, b, 3),
            ContinuousComparison::GreaterOrEqual(a, b) => (a, b, 4),
        };
        validate_predicate(left, right, limits)?;
        let mut work = 0;
        let left = eval(left, inputs, limits, &mut work)?;
        let right = eval(right, inputs, limits, &mut work)?;
        let value = match comparison {
            0 => left == right,
            1 => left < right,
            2 => left <= right,
            3 => left > right,
            _ => left >= right,
        };
        Ok(ContinuousPredicateEvaluationReceipt {
            value,
            work_used: work,
        })
    }
}
fn validate_predicate(
    left: &ContinuousExpr,
    right: &ContinuousExpr,
    limits: ContinuousExprLimits,
) -> Result<(), ContinuousEvaluationError> {
    validate(left, limits)?;
    validate(right, limits)?;
    let nodes = node_count(left) + node_count(right);
    if nodes > limits.maximum_nodes {
        return Err(ContinuousEvaluationError::NodeQuotaExceeded {
            actual: nodes,
            maximum: limits.maximum_nodes,
        });
    }
    let depth = expression_depth(left).max(expression_depth(right));
    if depth > limits.maximum_depth {
        return Err(ContinuousEvaluationError::DepthExceeded {
            actual: depth,
            maximum: limits.maximum_depth,
        });
    }
    let mut inputs = BTreeSet::new();
    collect_inputs(left, &mut inputs);
    collect_inputs(right, &mut inputs);
    if inputs.len() > limits.maximum_inputs {
        return Err(ContinuousEvaluationError::InputQuotaExceeded {
            actual: inputs.len(),
            maximum: limits.maximum_inputs,
        });
    }
    Ok(())
}
fn node_count(expr: &ContinuousExpr) -> usize {
    match expr {
        ContinuousExpr::Literal(_) | ContinuousExpr::Input(_) => 1,
        ContinuousExpr::Add(a, b)
        | ContinuousExpr::Subtract(a, b)
        | ContinuousExpr::Multiply(a, b)
        | ContinuousExpr::Divide(a, b) => 1 + node_count(a) + node_count(b),
        ContinuousExpr::Min(values) | ContinuousExpr::Max(values) => {
            1 + values.iter().map(node_count).sum::<usize>()
        }
    }
}
fn expression_depth(expr: &ContinuousExpr) -> usize {
    match expr {
        ContinuousExpr::Literal(_) | ContinuousExpr::Input(_) => 1,
        ContinuousExpr::Add(a, b)
        | ContinuousExpr::Subtract(a, b)
        | ContinuousExpr::Multiply(a, b)
        | ContinuousExpr::Divide(a, b) => 1 + expression_depth(a).max(expression_depth(b)),
        ContinuousExpr::Min(values) | ContinuousExpr::Max(values) => {
            1 + values.iter().map(expression_depth).max().unwrap_or(0)
        }
    }
}

fn validate(
    expr: &ContinuousExpr,
    limits: ContinuousExprLimits,
) -> Result<(), ContinuousEvaluationError> {
    let mut nodes = 0;
    let mut inputs = BTreeSet::new();
    validate_node(expr, 1, limits, &mut nodes, &mut inputs)?;
    if inputs.len() > limits.maximum_inputs {
        return Err(ContinuousEvaluationError::InputQuotaExceeded {
            actual: inputs.len(),
            maximum: limits.maximum_inputs,
        });
    }
    Ok(())
}
fn validate_node(
    expr: &ContinuousExpr,
    depth: usize,
    limits: ContinuousExprLimits,
    nodes: &mut usize,
    inputs: &mut BTreeSet<ContinuousInputReference>,
) -> Result<(), ContinuousEvaluationError> {
    if depth > limits.maximum_depth {
        return Err(ContinuousEvaluationError::DepthExceeded {
            actual: depth,
            maximum: limits.maximum_depth,
        });
    }
    *nodes += 1;
    if *nodes > limits.maximum_nodes {
        return Err(ContinuousEvaluationError::NodeQuotaExceeded {
            actual: *nodes,
            maximum: limits.maximum_nodes,
        });
    }
    match expr {
        ContinuousExpr::Literal(_) => {}
        ContinuousExpr::Input(input) => {
            inputs.insert(input.clone());
        }
        ContinuousExpr::Add(a, b)
        | ContinuousExpr::Subtract(a, b)
        | ContinuousExpr::Multiply(a, b)
        | ContinuousExpr::Divide(a, b) => {
            validate_node(a, depth + 1, limits, nodes, inputs)?;
            validate_node(b, depth + 1, limits, nodes, inputs)?
        }
        ContinuousExpr::Min(values) | ContinuousExpr::Max(values) => {
            if values.is_empty() {
                return Err(ContinuousEvaluationError::EmptyAggregate);
            }
            if values.len() > limits.maximum_arity {
                return Err(ContinuousEvaluationError::ArityExceeded {
                    actual: values.len(),
                    maximum: limits.maximum_arity,
                });
            }
            for value in values {
                validate_node(value, depth + 1, limits, nodes, inputs)?;
            }
        }
    };
    Ok(())
}
fn eval(
    expr: &ContinuousExpr,
    inputs: &ContinuousInputBundle,
    limits: ContinuousExprLimits,
    work: &mut usize,
) -> Result<ContinuousValue, ContinuousEvaluationError> {
    *work += 1;
    if *work > limits.maximum_work {
        return Err(ContinuousEvaluationError::WorkQuotaExceeded {
            actual: *work,
            maximum: limits.maximum_work,
        });
    }
    let result: Result<ContinuousValue, ContinuousValueError> = match expr {
        ContinuousExpr::Literal(value) => Ok(*value),
        ContinuousExpr::Input(input) => {
            Ok(inputs
                .get(input)
                .ok_or_else(|| ContinuousEvaluationError::MissingInput {
                    input: input.clone(),
                })?)
        }
        ContinuousExpr::Add(a, b) => {
            eval(a, inputs, limits, work)?.checked_add(eval(b, inputs, limits, work)?)
        }
        ContinuousExpr::Subtract(a, b) => {
            eval(a, inputs, limits, work)?.checked_sub(eval(b, inputs, limits, work)?)
        }
        ContinuousExpr::Multiply(a, b) => {
            eval(a, inputs, limits, work)?.checked_mul(eval(b, inputs, limits, work)?)
        }
        ContinuousExpr::Divide(a, b) => {
            eval(a, inputs, limits, work)?.checked_div(eval(b, inputs, limits, work)?)
        }
        ContinuousExpr::Min(values) => {
            let mut values = values.iter();
            let mut best = eval(values.next().expect("validated"), inputs, limits, work)?;
            for value in values {
                best = best.min(eval(value, inputs, limits, work)?);
            }
            Ok(best)
        }
        ContinuousExpr::Max(values) => {
            let mut values = values.iter();
            let mut best = eval(values.next().expect("validated"), inputs, limits, work)?;
            for value in values {
                best = best.max(eval(value, inputs, limits, work)?);
            }
            Ok(best)
        }
    };
    result.map_err(ContinuousEvaluationError::Value)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuousEvaluationError {
    Value(ContinuousValueError),
    MissingInput { input: ContinuousInputReference },
    DepthExceeded { actual: usize, maximum: usize },
    NodeQuotaExceeded { actual: usize, maximum: usize },
    InputQuotaExceeded { actual: usize, maximum: usize },
    ArityExceeded { actual: usize, maximum: usize },
    EmptyAggregate,
    WorkQuotaExceeded { actual: usize, maximum: usize },
    Role(RoleRequirementError),
}
impl fmt::Display for ContinuousEvaluationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "continuous expression evaluation rejected: {self:?}")
    }
}
impl std::error::Error for ContinuousEvaluationError {}
