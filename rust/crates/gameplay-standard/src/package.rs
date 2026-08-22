use std::fmt;

use gameplay_rules::{
    admit_rule_package, AdmittedRulePackage, RuleDomainId, RuleFingerprint, RulePackageCandidate,
    RulePackageDependency, RulePackageError, RulePackageId, RulePackageSchemaVersion,
    RuleProvenance, RuleSource, RuleSourceId, RuleSubjectId, RuleVersion,
};
use serde_json::{json, Value};

use crate::input::canonicalize_roles;
use crate::{
    ContinuousExpr, ExactExpr, RoleRequirement, RoleRequirementError,
    CONTINUOUS_EVALUATOR_SEMANTICS_VERSION, EXACT_EVALUATOR_SEMANTICS_VERSION,
};

pub const EXACT_FAMILY_ID: &str = "exact";
pub const CONTINUOUS_FAMILY_ID: &str = "continuous";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardDefinitionIdentity {
    package_fingerprint: RuleFingerprint,
    subject: RuleSubjectId,
    family: &'static str,
    semantics_version: u32,
}
impl StandardDefinitionIdentity {
    pub(crate) fn new(
        package_fingerprint: RuleFingerprint,
        subject: RuleSubjectId,
        family: &'static str,
        semantics_version: u32,
    ) -> Self {
        Self {
            package_fingerprint,
            subject,
            family,
            semantics_version,
        }
    }
    pub fn package_fingerprint(&self) -> &RuleFingerprint {
        &self.package_fingerprint
    }
    pub fn subject(&self) -> &RuleSubjectId {
        &self.subject
    }
    pub const fn family(&self) -> &'static str {
        self.family
    }
    pub const fn semantics_version(&self) -> u32 {
        self.semantics_version
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactDefinition {
    subject: RuleSubjectId,
    source: RuleSourceId,
    expression: ExactExpr,
    roles: Vec<RoleRequirement>,
}
/// Inspectable, canonical requirements for one exact definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactDefinitionRequirements {
    roles: Vec<RoleRequirement>,
    inputs: Vec<crate::ExactInputReference>,
}
impl ExactDefinitionRequirements {
    pub fn roles(&self) -> &[RoleRequirement] {
        &self.roles
    }
    pub fn inputs(&self) -> &[crate::ExactInputReference] {
        &self.inputs
    }
}
impl ExactDefinition {
    pub fn new(
        subject: RuleSubjectId,
        source: RuleSourceId,
        expression: ExactExpr,
        roles: Vec<RoleRequirement>,
    ) -> Result<Self, StandardDefinitionError> {
        crate::ExactEvaluator::validate_structure(&expression, crate::ExactExprLimits::default())
            .map_err(StandardDefinitionError::ExactStructure)?;
        let roles = canonicalize_roles(roles).map_err(StandardDefinitionError::Role)?;
        validate_exact_roles(&expression, &roles)?;
        Ok(Self {
            subject,
            source,
            expression,
            roles,
        })
    }
    pub fn subject(&self) -> &RuleSubjectId {
        &self.subject
    }
    pub fn expression(&self) -> &ExactExpr {
        &self.expression
    }
    pub fn source(&self) -> &RuleSourceId {
        &self.source
    }
    pub fn roles(&self) -> &[RoleRequirement] {
        &self.roles
    }
    pub fn requirements(&self) -> Result<ExactDefinitionRequirements, StandardDefinitionError> {
        Ok(ExactDefinitionRequirements {
            roles: self.roles.clone(),
            inputs: crate::ExactExprRequirements::inspect(&self.expression)
                .map_err(StandardDefinitionError::ExactStructure)?
                .inputs()
                .to_vec(),
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousDefinition {
    subject: RuleSubjectId,
    source: RuleSourceId,
    expression: ContinuousExpr,
    roles: Vec<RoleRequirement>,
}
/// Inspectable, canonical requirements for one continuous definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuousDefinitionRequirements {
    roles: Vec<RoleRequirement>,
    inputs: Vec<crate::ContinuousInputReference>,
}
impl ContinuousDefinitionRequirements {
    pub fn roles(&self) -> &[RoleRequirement] {
        &self.roles
    }
    pub fn inputs(&self) -> &[crate::ContinuousInputReference] {
        &self.inputs
    }
}
impl ContinuousDefinition {
    pub fn new(
        subject: RuleSubjectId,
        source: RuleSourceId,
        expression: ContinuousExpr,
        roles: Vec<RoleRequirement>,
    ) -> Result<Self, StandardDefinitionError> {
        crate::ContinuousEvaluator::validate_structure(
            &expression,
            crate::ContinuousExprLimits::default(),
        )
        .map_err(StandardDefinitionError::ContinuousStructure)?;
        let roles = canonicalize_roles(roles).map_err(StandardDefinitionError::Role)?;
        validate_continuous_roles(&expression, &roles)?;
        Ok(Self {
            subject,
            source,
            expression,
            roles,
        })
    }
    pub fn subject(&self) -> &RuleSubjectId {
        &self.subject
    }
    pub fn expression(&self) -> &ContinuousExpr {
        &self.expression
    }
    pub fn source(&self) -> &RuleSourceId {
        &self.source
    }
    pub fn roles(&self) -> &[RoleRequirement] {
        &self.roles
    }
    pub fn requirements(
        &self,
    ) -> Result<ContinuousDefinitionRequirements, StandardDefinitionError> {
        Ok(ContinuousDefinitionRequirements {
            roles: self.roles.clone(),
            inputs: crate::ContinuousExprRequirements::inspect(&self.expression)
                .map_err(StandardDefinitionError::ContinuousStructure)?
                .inputs()
                .to_vec(),
        })
    }
}

fn role_is_declared(role: &crate::CapabilityRoleId, roles: &[RoleRequirement]) -> bool {
    roles
        .binary_search_by(|requirement| requirement.role().cmp(role))
        .is_ok()
}
fn validate_exact_roles(
    expression: &ExactExpr,
    roles: &[RoleRequirement],
) -> Result<(), StandardDefinitionError> {
    for input in crate::ExactExprRequirements::inspect(expression)
        .map_err(StandardDefinitionError::ExactStructure)?
        .inputs()
    {
        if !role_is_declared(input.role(), roles) {
            return Err(StandardDefinitionError::UndeclaredInputRole {
                role: input.role().clone(),
            });
        }
    }
    Ok(())
}
fn validate_continuous_roles(
    expression: &ContinuousExpr,
    roles: &[RoleRequirement],
) -> Result<(), StandardDefinitionError> {
    for input in crate::ContinuousExprRequirements::inspect(expression)
        .map_err(StandardDefinitionError::ContinuousStructure)?
        .inputs()
    {
        if !role_is_declared(input.role(), roles) {
            return Err(StandardDefinitionError::UndeclaredInputRole {
                role: input.role().clone(),
            });
        }
    }
    Ok(())
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardPackageContext {
    schema_version: RulePackageSchemaVersion,
    domain: RuleDomainId,
    package: RulePackageId,
    version: RuleVersion,
    dependencies: Vec<RulePackageDependency>,
    sources: Vec<RuleSource>,
    provenance: Vec<RuleProvenance>,
}

impl StandardPackageContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: RulePackageSchemaVersion,
        domain: RuleDomainId,
        package: RulePackageId,
        version: RuleVersion,
        dependencies: Vec<RulePackageDependency>,
        sources: Vec<RuleSource>,
        provenance: Vec<RuleProvenance>,
    ) -> Self {
        Self {
            schema_version,
            domain,
            package,
            version,
            dependencies,
            sources,
            provenance,
        }
    }

    pub(crate) fn candidate(&self, payload: Value) -> RulePackageCandidate {
        RulePackageCandidate::new_with_schema(
            self.schema_version,
            self.domain.clone(),
            self.package.clone(),
            self.version,
            self.dependencies.clone(),
            self.sources.clone(),
            self.provenance.clone(),
            payload,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedExactDefinition {
    package: AdmittedRulePackage,
    definition: ExactDefinition,
    identity: StandardDefinitionIdentity,
}
impl AdmittedExactDefinition {
    pub fn package(&self) -> &AdmittedRulePackage {
        &self.package
    }
    pub fn definition(&self) -> &ExactDefinition {
        &self.definition
    }
    pub fn identity(&self) -> &StandardDefinitionIdentity {
        &self.identity
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedContinuousDefinition {
    package: AdmittedRulePackage,
    definition: ContinuousDefinition,
    identity: StandardDefinitionIdentity,
}
impl AdmittedContinuousDefinition {
    pub fn package(&self) -> &AdmittedRulePackage {
        &self.package
    }
    pub fn definition(&self) -> &ContinuousDefinition {
        &self.definition
    }
    pub fn identity(&self) -> &StandardDefinitionIdentity {
        &self.identity
    }
}

pub fn admit_exact_definition(
    context: &StandardPackageContext,
    definition: ExactDefinition,
) -> Result<AdmittedExactDefinition, StandardDefinitionError> {
    let admitted = admit_rule_package(context.candidate(exact_payload(&definition)))
        .map_err(StandardDefinitionError::Package)?;
    let decoded = decode_exact_definition(&admitted)?;
    validate_correlation(&admitted, definition.subject(), definition.source())?;
    if decoded.definition != definition {
        return Err(StandardDefinitionError::NonConvergentPayload);
    }
    Ok(AdmittedExactDefinition {
        package: admitted,
        definition,
        identity: decoded.identity,
    })
}
pub fn admit_continuous_definition(
    context: &StandardPackageContext,
    definition: ContinuousDefinition,
) -> Result<AdmittedContinuousDefinition, StandardDefinitionError> {
    let admitted = admit_rule_package(context.candidate(continuous_payload(&definition)))
        .map_err(StandardDefinitionError::Package)?;
    let decoded = decode_continuous_definition(&admitted)?;
    validate_correlation(&admitted, definition.subject(), definition.source())?;
    if decoded.definition != definition {
        return Err(StandardDefinitionError::NonConvergentPayload);
    }
    Ok(AdmittedContinuousDefinition {
        package: admitted,
        definition,
        identity: decoded.identity,
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedExactDefinition {
    pub identity: StandardDefinitionIdentity,
    pub definition: ExactDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedContinuousDefinition {
    pub identity: StandardDefinitionIdentity,
    pub definition: ContinuousDefinition,
}

/// Rehydrates the schema-1 exact tree from an admitted canonical rule package.
pub fn decode_exact_definition(
    package: &AdmittedRulePackage,
) -> Result<DecodedExactDefinition, StandardDefinitionError> {
    let payload = payload_object(
        package,
        EXACT_FAMILY_ID,
        EXACT_EVALUATOR_SEMANTICS_VERSION,
        RulePackageSchemaVersion::IntegerOnlyV1,
    )?;
    let (subject, source, roles) = definition_header(package, payload)?;
    let expression = decode_exact_expr(required(payload, "tree")?, "payload.tree")?;
    crate::ExactEvaluator::validate_structure(&expression, crate::ExactExprLimits::default())
        .map_err(StandardDefinitionError::ExactStructure)?;
    Ok(DecodedExactDefinition {
        identity: identity(
            package,
            subject.clone(),
            EXACT_FAMILY_ID,
            EXACT_EVALUATOR_SEMANTICS_VERSION,
        ),
        definition: ExactDefinition::new(subject, source, expression, roles)?,
    })
}

/// Rehydrates the schema-2 continuous tree from an admitted canonical rule package.
pub fn decode_continuous_definition(
    package: &AdmittedRulePackage,
) -> Result<DecodedContinuousDefinition, StandardDefinitionError> {
    let payload = payload_object(
        package,
        CONTINUOUS_FAMILY_ID,
        CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,
        RulePackageSchemaVersion::Binary64V2,
    )?;
    let (subject, source, roles) = definition_header(package, payload)?;
    let expression = decode_continuous_expr(required(payload, "tree")?, "payload.tree")?;
    crate::ContinuousEvaluator::validate_structure(
        &expression,
        crate::ContinuousExprLimits::default(),
    )
    .map_err(StandardDefinitionError::ContinuousStructure)?;
    Ok(DecodedContinuousDefinition {
        identity: identity(
            package,
            subject.clone(),
            CONTINUOUS_FAMILY_ID,
            CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,
        ),
        definition: ContinuousDefinition::new(subject, source, expression, roles)?,
    })
}

fn identity(
    package: &AdmittedRulePackage,
    subject: RuleSubjectId,
    family: &'static str,
    semantics_version: u32,
) -> StandardDefinitionIdentity {
    StandardDefinitionIdentity::new(
        package.fingerprint().clone(),
        subject,
        family,
        semantics_version,
    )
}
fn payload_object<'a>(
    package: &'a AdmittedRulePackage,
    family: &'static str,
    semantics_version: u32,
    expected_schema: RulePackageSchemaVersion,
) -> Result<&'a serde_json::Map<String, Value>, StandardDefinitionError> {
    if package.schema_version() != expected_schema {
        return Err(StandardDefinitionError::WrongSchema {
            expected: expected_schema.get(),
            actual: package.schema_version().get(),
        });
    }
    let payload = package
        .payload()
        .as_object()
        .ok_or_else(|| malformed("payload", "must be an object"))?;
    let actual = payload
        .get("family")
        .and_then(|value| value.as_str())
        .ok_or_else(|| malformed("payload.family", "is required and must be a string"))?;
    if actual != family {
        return Err(StandardDefinitionError::WrongFamily {
            expected: family,
            actual: actual.to_owned(),
        });
    }
    let actual_version = payload
        .get("semanticsVersion")
        .and_then(|value| {
            value.as_u64().or_else(|| {
                value
                    .as_f64()
                    .filter(|number| {
                        number.is_finite() && number.fract() == 0.0 && *number <= u64::MAX as f64
                    })
                    .map(|number| number as u64)
            })
        })
        .ok_or_else(|| {
            malformed(
                "payload.semanticsVersion",
                "is required and must be an integer",
            )
        })?;
    if actual_version != u64::from(semantics_version) {
        return Err(StandardDefinitionError::UnsupportedSemanticsVersion {
            family,
            actual: actual_version,
        });
    }
    Ok(payload)
}

fn definition_header(
    package: &AdmittedRulePackage,
    payload: &serde_json::Map<String, Value>,
) -> Result<(RuleSubjectId, RuleSourceId, Vec<RoleRequirement>), StandardDefinitionError> {
    ensure_fields(
        payload,
        &[
            "family",
            "semanticsVersion",
            "subject",
            "source",
            "roles",
            "tree",
        ],
        "payload",
    )?;
    let subject = RuleSubjectId::parse(string(required(payload, "subject")?, "payload.subject")?)
        .map_err(StandardDefinitionError::Package)?;
    let source = RuleSourceId::parse(string(required(payload, "source")?, "payload.source")?)
        .map_err(StandardDefinitionError::Package)?;
    validate_correlation(package, &subject, &source)?;
    Ok((subject, source, decode_roles(required(payload, "roles")?)?))
}

fn required<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
) -> Result<&'a Value, StandardDefinitionError> {
    object
        .get(name)
        .ok_or_else(|| malformed(&format!("payload.{name}"), "is required"))
}
fn string<'a>(value: &'a Value, path: &str) -> Result<&'a str, StandardDefinitionError> {
    value
        .as_str()
        .ok_or_else(|| malformed(path, "must be a string"))
}
fn malformed(path: &str, reason: &str) -> StandardDefinitionError {
    StandardDefinitionError::MalformedPayload {
        path: path.to_owned(),
        reason: reason.to_owned(),
    }
}
fn ensure_fields(
    object: &serde_json::Map<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<(), StandardDefinitionError> {
    if let Some(field) = object
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        return Err(malformed(&format!("{path}.{field}"), "is not recognized"));
    }
    Ok(())
}

fn decode_roles(value: &Value) -> Result<Vec<RoleRequirement>, StandardDefinitionError> {
    let array = value
        .as_array()
        .ok_or_else(|| malformed("payload.roles", "must be an array"))?;
    let mut roles = Vec::with_capacity(array.len());
    for (index, role) in array.iter().enumerate() {
        let path = format!("payload.roles[{index}]");
        let role = role
            .as_object()
            .ok_or_else(|| malformed(&path, "must be an object"))?;
        ensure_fields(role, &["role", "capabilities"], &path)?;
        let id = crate::CapabilityRoleId::parse(string(
            required(role, "role")?,
            &format!("{path}.role"),
        )?)
        .map_err(StandardDefinitionError::Role)?;
        let caps = required(role, "capabilities")?
            .as_array()
            .ok_or_else(|| malformed(&format!("{path}.capabilities"), "must be an array"))?
            .iter()
            .enumerate()
            .map(|(cap_index, cap)| {
                crate::CapabilityRequirementId::parse(string(
                    cap,
                    &format!("{path}.capabilities[{cap_index}]"),
                )?)
                .map_err(StandardDefinitionError::Role)
            })
            .collect::<Result<Vec<_>, _>>()?;
        roles.push(RoleRequirement::new(id, caps).map_err(StandardDefinitionError::Role)?);
    }
    let canonical = canonicalize_roles(roles.clone()).map_err(StandardDefinitionError::Role)?;
    if canonical != roles {
        return Err(malformed(
            "payload.roles",
            "must be sorted, deduplicated, and merged by role",
        ));
    }
    Ok(roles)
}

fn decode_exact_expr(value: &Value, path: &str) -> Result<ExactExpr, StandardDefinitionError> {
    let object = value
        .as_object()
        .ok_or_else(|| malformed(path, "must be an object"))?;
    let op = string(required(object, "op")?, &format!("{path}.op"))?;
    match op {
        "literal" => {
            ensure_fields(object, &["op", "value"], path)?;
            let value = required(object, "value")?.as_i64().ok_or_else(|| {
                malformed(&format!("{path}.value"), "must be an exact signed integer")
            })?;
            Ok(ExactExpr::Literal(
                gameplay_mechanics::MechanicsScalar::new(value).map_err(|error| {
                    StandardDefinitionError::ExactLiteral {
                        path: path.to_owned(),
                        error,
                    }
                })?,
            ))
        }
        "input" => {
            ensure_fields(object, &["op", "input"], path)?;
            Ok(ExactExpr::Input(decode_exact_input(
                required(object, "input")?,
                &format!("{path}.input"),
            )?))
        }
        "add" => binary_exact(object, path, ExactExpr::Add),
        "subtract" => binary_exact(object, path, ExactExpr::Subtract),
        "multiply" => binary_exact(object, path, ExactExpr::Multiply),
        "floorDivide" => binary_exact(object, path, ExactExpr::FloorDivide),
        "truncatingDivide" => binary_exact(object, path, ExactExpr::TruncatingDivide),
        "min" => aggregate_exact(object, path, ExactExpr::Min),
        "max" => aggregate_exact(object, path, ExactExpr::Max),
        _ => Err(malformed(
            &format!("{path}.op"),
            "is not a supported exact operation",
        )),
    }
}
fn binary_exact(
    object: &serde_json::Map<String, Value>,
    path: &str,
    build: impl Fn(Box<ExactExpr>, Box<ExactExpr>) -> ExactExpr,
) -> Result<ExactExpr, StandardDefinitionError> {
    ensure_fields(object, &["op", "left", "right"], path)?;
    Ok(build(
        Box::new(decode_exact_expr(
            required(object, "left")?,
            &format!("{path}.left"),
        )?),
        Box::new(decode_exact_expr(
            required(object, "right")?,
            &format!("{path}.right"),
        )?),
    ))
}
fn aggregate_exact(
    object: &serde_json::Map<String, Value>,
    path: &str,
    build: impl Fn(Vec<ExactExpr>) -> ExactExpr,
) -> Result<ExactExpr, StandardDefinitionError> {
    ensure_fields(object, &["op", "values"], path)?;
    let values = required(object, "values")?
        .as_array()
        .ok_or_else(|| malformed(&format!("{path}.values"), "must be an array"))?;
    Ok(build(
        values
            .iter()
            .enumerate()
            .map(|(i, value)| decode_exact_expr(value, &format!("{path}.values[{i}]")))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}
fn decode_exact_input(
    value: &Value,
    path: &str,
) -> Result<crate::ExactInputReference, StandardDefinitionError> {
    use crate::{ExactInputReference as Input, StandardExactFactReference as Fact};
    let object = value
        .as_object()
        .ok_or_else(|| malformed(path, "must be an object"))?;
    let kind = string(required(object, "kind")?, &format!("{path}.kind"))?;
    let role = || {
        crate::CapabilityRoleId::parse(string(required(object, "role")?, &format!("{path}.role"))?)
            .map_err(StandardDefinitionError::Role)
    };
    let ordinary = |build: fn(crate::CapabilityRoleId, crate::InputId) -> Input| -> Result<Input, StandardDefinitionError> {
        ensure_fields(object, &["kind", "role", "id"], path)?;
        Ok(build(role()?, crate::InputId::parse(string(required(object, "id")?, &format!("{path}.id"))?).map_err(StandardDefinitionError::Role)?))
    };
    match kind {
        "parameter" => ordinary(|role, id| Input::Parameter { role, id }),
        "fact" => ordinary(|role, id| Input::Fact { role, id }),
        "roll" => ordinary(|role, id| Input::Roll { role, id }),
        "choice" => ordinary(|role, id| Input::Choice { role, id }),
        "standardStat" => {
            ensure_fields(object, &["kind", "role", "stat"], path)?;
            let stat_path = format!("{path}.stat");
            Ok(Input::StandardFact(Fact::Stat {
                role: role()?,
                stat: gameplay_mechanics::StatId::parse(string(
                    required(object, "stat")?,
                    &stat_path,
                )?)
                .map_err(|error| malformed(&stat_path, &error.to_string()))?,
            }))
        }
        "standardTrackCurrent" => {
            ensure_fields(object, &["kind", "role", "track"], path)?;
            let track_path = format!("{path}.track");
            Ok(Input::StandardFact(Fact::TrackCurrent {
                role: role()?,
                track: gameplay_mechanics::TrackId::parse(string(
                    required(object, "track")?,
                    &track_path,
                )?)
                .map_err(|error| malformed(&track_path, &error.to_string()))?,
            }))
        }
        "standardTrackMaximum" => {
            ensure_fields(object, &["kind", "role", "track"], path)?;
            let track_path = format!("{path}.track");
            Ok(Input::StandardFact(Fact::TrackMaximum {
                role: role()?,
                track: gameplay_mechanics::TrackId::parse(string(
                    required(object, "track")?,
                    &track_path,
                )?)
                .map_err(|error| malformed(&track_path, &error.to_string()))?,
            }))
        }
        _ => Err(malformed(
            &format!("{path}.kind"),
            "is not a supported exact input",
        )),
    }
}

fn decode_continuous_expr(
    value: &Value,
    path: &str,
) -> Result<ContinuousExpr, StandardDefinitionError> {
    let object = value
        .as_object()
        .ok_or_else(|| malformed(path, "must be an object"))?;
    let op = string(required(object, "op")?, &format!("{path}.op"))?;
    match op {
        "literal" => {
            ensure_fields(object, &["op", "bits"], path)?;
            let bits_text = string(required(object, "bits")?, &format!("{path}.bits"))?;
            if bits_text.len() != 16
                || !bits_text
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(malformed(
                    &format!("{path}.bits"),
                    "must be sixteen lowercase hexadecimal binary64 bits",
                ));
            }
            let bits = u64::from_str_radix(bits_text, 16).map_err(|_| {
                malformed(
                    &format!("{path}.bits"),
                    "must be sixteen lowercase hexadecimal binary64 bits",
                )
            })?;
            let value = crate::ContinuousValue::from_bits(bits)
                .map_err(StandardDefinitionError::ContinuousLiteral)?;
            if value.bits() != bits {
                return Err(malformed(
                    &format!("{path}.bits"),
                    "must use the canonical finite binary64 encoding",
                ));
            }
            Ok(ContinuousExpr::Literal(value))
        }
        "input" => {
            ensure_fields(object, &["op", "input"], path)?;
            Ok(ContinuousExpr::Input(decode_continuous_input(
                required(object, "input")?,
                &format!("{path}.input"),
            )?))
        }
        "add" => binary_continuous(object, path, ContinuousExpr::Add),
        "subtract" => binary_continuous(object, path, ContinuousExpr::Subtract),
        "multiply" => binary_continuous(object, path, ContinuousExpr::Multiply),
        "divide" => binary_continuous(object, path, ContinuousExpr::Divide),
        "min" => aggregate_continuous(object, path, ContinuousExpr::Min),
        "max" => aggregate_continuous(object, path, ContinuousExpr::Max),
        _ => Err(malformed(
            &format!("{path}.op"),
            "is not a supported continuous operation",
        )),
    }
}
fn binary_continuous(
    object: &serde_json::Map<String, Value>,
    path: &str,
    build: impl Fn(Box<ContinuousExpr>, Box<ContinuousExpr>) -> ContinuousExpr,
) -> Result<ContinuousExpr, StandardDefinitionError> {
    ensure_fields(object, &["op", "left", "right"], path)?;
    Ok(build(
        Box::new(decode_continuous_expr(
            required(object, "left")?,
            &format!("{path}.left"),
        )?),
        Box::new(decode_continuous_expr(
            required(object, "right")?,
            &format!("{path}.right"),
        )?),
    ))
}
fn aggregate_continuous(
    object: &serde_json::Map<String, Value>,
    path: &str,
    build: impl Fn(Vec<ContinuousExpr>) -> ContinuousExpr,
) -> Result<ContinuousExpr, StandardDefinitionError> {
    ensure_fields(object, &["op", "values"], path)?;
    let values = required(object, "values")?
        .as_array()
        .ok_or_else(|| malformed(&format!("{path}.values"), "must be an array"))?;
    Ok(build(
        values
            .iter()
            .enumerate()
            .map(|(i, value)| decode_continuous_expr(value, &format!("{path}.values[{i}]")))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}
fn decode_continuous_input(
    value: &Value,
    path: &str,
) -> Result<crate::ContinuousInputReference, StandardDefinitionError> {
    use crate::ContinuousInputReference as Input;
    let object = value
        .as_object()
        .ok_or_else(|| malformed(path, "must be an object"))?;
    ensure_fields(object, &["kind", "role", "id"], path)?;
    let role =
        crate::CapabilityRoleId::parse(string(required(object, "role")?, &format!("{path}.role"))?)
            .map_err(StandardDefinitionError::Role)?;
    let id = crate::InputId::parse(string(required(object, "id")?, &format!("{path}.id"))?)
        .map_err(StandardDefinitionError::Role)?;
    match string(required(object, "kind")?, &format!("{path}.kind"))? {
        "parameter" => Ok(Input::Parameter { role, id }),
        "fact" => Ok(Input::Fact { role, id }),
        "roll" => Ok(Input::Roll { role, id }),
        "choice" => Ok(Input::Choice { role, id }),
        _ => Err(malformed(
            &format!("{path}.kind"),
            "is not a supported continuous input",
        )),
    }
}

fn validate_correlation(
    package: &AdmittedRulePackage,
    subject: &RuleSubjectId,
    source: &RuleSourceId,
) -> Result<(), StandardDefinitionError> {
    match package.correlated_source(subject) {
        Some((provenance, _)) if provenance.source() == source => Ok(()),
        Some((provenance, _)) => Err(StandardDefinitionError::SourceMismatch {
            subject: subject.clone(),
            expected: source.clone(),
            actual: provenance.source().clone(),
        }),
        None => Err(StandardDefinitionError::MissingCorrelation {
            subject: subject.clone(),
            source: source.clone(),
        }),
    }
}

fn roles_payload(roles: &[RoleRequirement]) -> Value {
    Value::Array(roles.iter().map(|role| json!({"role":role.role().as_str(),"capabilities":role.capabilities().iter().map(|capability| Value::String(capability.as_str().to_owned())).collect::<Vec<_>>() })).collect())
}

fn exact_payload(definition: &ExactDefinition) -> Value {
    json!({"family":EXACT_FAMILY_ID,"semanticsVersion":EXACT_EVALUATOR_SEMANTICS_VERSION,"subject":definition.subject().as_str(),"source":definition.source().as_str(),"roles":roles_payload(definition.roles()),"tree":exact_expr_payload(definition.expression())})
}

fn continuous_payload(definition: &ContinuousDefinition) -> Value {
    json!({"family":CONTINUOUS_FAMILY_ID,"semanticsVersion":CONTINUOUS_EVALUATOR_SEMANTICS_VERSION,"subject":definition.subject().as_str(),"source":definition.source().as_str(),"roles":roles_payload(definition.roles()),"tree":continuous_expr_payload(definition.expression())})
}

fn exact_expr_payload(expression: &ExactExpr) -> Value {
    use ExactExpr::*;
    match expression {
        Literal(value) => json!({"op":"literal","value":value.get()}),
        Input(input) => json!({"op":"input","input":exact_input_payload(input)}),
        Add(a, b) => binary("add", exact_expr_payload(a), exact_expr_payload(b)),
        Subtract(a, b) => binary("subtract", exact_expr_payload(a), exact_expr_payload(b)),
        Multiply(a, b) => binary("multiply", exact_expr_payload(a), exact_expr_payload(b)),
        FloorDivide(a, b) => binary("floorDivide", exact_expr_payload(a), exact_expr_payload(b)),
        TruncatingDivide(a, b) => binary(
            "truncatingDivide",
            exact_expr_payload(a),
            exact_expr_payload(b),
        ),
        Min(values) => aggregate("min", values, exact_expr_payload),
        Max(values) => aggregate("max", values, exact_expr_payload),
    }
}

fn continuous_expr_payload(expression: &ContinuousExpr) -> Value {
    use ContinuousExpr::*;
    match expression {
        Literal(value) => json!({"op":"literal","bits":format!("{:016x}", value.bits())}),
        Input(input) => json!({"op":"input","input":continuous_input_payload(input)}),
        Add(a, b) => binary(
            "add",
            continuous_expr_payload(a),
            continuous_expr_payload(b),
        ),
        Subtract(a, b) => binary(
            "subtract",
            continuous_expr_payload(a),
            continuous_expr_payload(b),
        ),
        Multiply(a, b) => binary(
            "multiply",
            continuous_expr_payload(a),
            continuous_expr_payload(b),
        ),
        Divide(a, b) => binary(
            "divide",
            continuous_expr_payload(a),
            continuous_expr_payload(b),
        ),
        Min(values) => aggregate("min", values, continuous_expr_payload),
        Max(values) => aggregate("max", values, continuous_expr_payload),
    }
}
fn exact_input_payload(input: &crate::ExactInputReference) -> Value {
    use crate::{ExactInputReference as Input, StandardExactFactReference as Fact};
    match input {
        Input::Parameter { role, id } => {
            json!({"kind":"parameter","role":role.as_str(),"id":id.as_str()})
        }
        Input::Fact { role, id } => json!({"kind":"fact","role":role.as_str(),"id":id.as_str()}),
        Input::Roll { role, id } => json!({"kind":"roll","role":role.as_str(),"id":id.as_str()}),
        Input::Choice { role, id } => {
            json!({"kind":"choice","role":role.as_str(),"id":id.as_str()})
        }
        Input::StandardFact(Fact::Stat { role, stat }) => {
            json!({"kind":"standardStat","role":role.as_str(),"stat":stat.as_str()})
        }
        Input::StandardFact(Fact::TrackCurrent { role, track }) => {
            json!({"kind":"standardTrackCurrent","role":role.as_str(),"track":track.as_str()})
        }
        Input::StandardFact(Fact::TrackMaximum { role, track }) => {
            json!({"kind":"standardTrackMaximum","role":role.as_str(),"track":track.as_str()})
        }
    }
}
fn continuous_input_payload(input: &crate::ContinuousInputReference) -> Value {
    use crate::ContinuousInputReference as Input;
    match input {
        Input::Parameter { role, id } => {
            json!({"kind":"parameter","role":role.as_str(),"id":id.as_str()})
        }
        Input::Fact { role, id } => json!({"kind":"fact","role":role.as_str(),"id":id.as_str()}),
        Input::Roll { role, id } => json!({"kind":"roll","role":role.as_str(),"id":id.as_str()}),
        Input::Choice { role, id } => {
            json!({"kind":"choice","role":role.as_str(),"id":id.as_str()})
        }
    }
}
fn binary(op: &'static str, left: Value, right: Value) -> Value {
    json!({"op":op,"left":left,"right":right})
}
fn aggregate<T>(op: &'static str, values: &[T], encode: impl Fn(&T) -> Value) -> Value {
    json!({"op":op,"values":values.iter().map(encode).collect::<Vec<_>>()})
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandardDefinitionError {
    Package(RulePackageError),
    Role(RoleRequirementError),
    ExactLiteral {
        path: String,
        error: gameplay_mechanics::MechanicsArithmeticError,
    },
    ContinuousLiteral(crate::ContinuousValueError),
    ExactStructure(crate::ExactEvaluationError),
    ContinuousStructure(crate::ContinuousEvaluationError),
    MalformedPayload {
        path: String,
        reason: String,
    },
    WrongSchema {
        expected: u64,
        actual: u64,
    },
    WrongFamily {
        expected: &'static str,
        actual: String,
    },
    UnsupportedSemanticsVersion {
        family: &'static str,
        actual: u64,
    },
    MissingCorrelation {
        subject: RuleSubjectId,
        source: RuleSourceId,
    },
    SourceMismatch {
        subject: RuleSubjectId,
        expected: RuleSourceId,
        actual: RuleSourceId,
    },
    NonConvergentPayload,
    UndeclaredInputRole {
        role: crate::CapabilityRoleId,
    },
}
impl fmt::Display for StandardDefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedPayload { path, reason } => write!(
                f,
                "gameplay-standard definition has invalid {path}: {reason}"
            ),
            Self::WrongFamily { expected, actual } => write!(
                f,
                "gameplay-standard definition family must be {expected}, got {actual}"
            ),
            Self::WrongSchema { expected, actual } => write!(
                f,
                "gameplay-standard definition requires package schema {expected}, got {actual}"
            ),
            Self::UnsupportedSemanticsVersion { family, actual } => write!(
                f,
                "gameplay-standard {family} evaluator semantics version {actual} is unsupported"
            ),
            _ => write!(f, "gameplay-standard definition rejected: {self:?}"),
        }
    }
}
impl std::error::Error for StandardDefinitionError {}
