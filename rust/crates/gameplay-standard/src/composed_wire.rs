//! Strict JSON transport for composed exact definitions.

use gameplay_mechanics::MechanicsScalar;
use gameplay_rules::{AdmittedRulePackage, RulePackageSchemaVersion, RuleSourceId, RuleSubjectId};
use serde_json::{json, Value};

use crate::composed::{
    canonicalize_roles, ComposedExactDefinition, ComposedExactExpr, ComposedExactLeafCodec,
    ComposedExactLeafKindId, ComposedExactProductLeaf, COMPOSED_EXACT_FAMILY_ID,
    COMPOSED_EXACT_SEMANTICS_VERSION,
};
use crate::composed_error::{
    ComposedExactDefinitionError, ComposedExactError, ComposedExactProductContext,
};
use crate::{CapabilityRoleId, ExactInputReference, RoleRequirement, StandardExtensionSchema};

pub(crate) fn encode_definition<C: ComposedExactLeafCodec>(
    definition: &ComposedExactDefinition<C::Leaf>,
) -> Result<Value, ComposedExactError<C::Error>> {
    Ok(
        json!({"family":COMPOSED_EXACT_FAMILY_ID,"semanticsVersion":COMPOSED_EXACT_SEMANTICS_VERSION,"subject":definition.subject().as_str(),"source":definition.source().as_str(),"roles":roles_payload(definition.roles()),"extension":schema_payload(definition.schema()),"tree":encode_expr::<C>(definition.expression(), "payload.tree")?}),
    )
}
pub(crate) fn encode_expr<C: ComposedExactLeafCodec>(
    expr: &ComposedExactExpr<C::Leaf>,
    path: &str,
) -> Result<Value, ComposedExactError<C::Error>> {
    Ok(match expr {
        ComposedExactExpr::Literal(value) => json!({"op":"literal","value":value.get()}),
        ComposedExactExpr::Input(input) => json!({"op":"input","input":input_payload(input)}),
        ComposedExactExpr::Add(a, b) => wire_binary::<C>("add", a, b, path)?,
        ComposedExactExpr::Subtract(a, b) => wire_binary::<C>("subtract", a, b, path)?,
        ComposedExactExpr::Multiply(a, b) => wire_binary::<C>("multiply", a, b, path)?,
        ComposedExactExpr::FloorDivide(a, b) => wire_binary::<C>("floorDivide", a, b, path)?,
        ComposedExactExpr::TruncatingDivide(a, b) => {
            wire_binary::<C>("truncatingDivide", a, b, path)?
        }
        ComposedExactExpr::Min(values) => {
            json!({"op":"min","values":values.iter().enumerate().map(|(index, value)| encode_expr::<C>(value, &child_path(path, &format!(".values[{index}]")))).collect::<Result<Vec<_>,_>>()?})
        }
        ComposedExactExpr::Max(values) => {
            json!({"op":"max","values":values.iter().enumerate().map(|(index, value)| encode_expr::<C>(value, &child_path(path, &format!(".values[{index}]")))).collect::<Result<Vec<_>,_>>()?})
        }
        ComposedExactExpr::Product(leaf) => {
            let payload = C::encode_leaf(leaf.kind(), leaf.value()).map_err(|error| {
                ComposedExactError::ProductEncode {
                    context: Box::new(ComposedExactProductContext::new(
                        bounded_path(path),
                        C::schema(),
                        leaf.kind().clone(),
                        leaf.subject().clone(),
                        leaf.source().clone(),
                    )),
                    error: Box::new(error),
                }
            })?;
            canonical_payload_len(&payload).map_err(ComposedExactError::Wire)?;
            json!({"op":"product","kind":leaf.kind().as_str(),"subject":leaf.subject().as_str(),"source":leaf.source().as_str(),"payload":payload})
        }
    })
}
pub(crate) fn wire_binary<C: ComposedExactLeafCodec>(
    op: &'static str,
    a: &ComposedExactExpr<C::Leaf>,
    b: &ComposedExactExpr<C::Leaf>,
    path: &str,
) -> Result<Value, ComposedExactError<C::Error>> {
    Ok(
        json!({"op":op,"left":encode_expr::<C>(a, &child_path(path, ".left"))?,"right":encode_expr::<C>(b, &child_path(path, ".right"))?}),
    )
}

pub(crate) fn decode_expr<C: ComposedExactLeafCodec>(
    value: &Value,
    path: &str,
    package: &AdmittedRulePackage,
) -> Result<ComposedExactExpr<C::Leaf>, ComposedExactError<C::Error>> {
    let object = value
        .as_object()
        .ok_or_else(|| malformed(path, "must be an object"))?;
    let op = string(object, "op", path)?;
    match op {
        "literal" => {
            fields(object, &["op", "value"], path)?;
            let value = required(object, "value", path)?.as_i64().ok_or_else(|| {
                malformed(&format!("{path}.value"), "must be an exact signed integer")
            })?;
            Ok(ComposedExactExpr::Literal(
                MechanicsScalar::new(value).map_err(|error| {
                    ComposedExactError::Wire(ComposedExactDefinitionError::ExactLiteral {
                        path: path.to_owned(),
                        error,
                    })
                })?,
            ))
        }
        "input" => {
            fields(object, &["op", "input"], path)?;
            Ok(ComposedExactExpr::Input(decode_input(
                required(object, "input", path)?,
                &format!("{path}.input"),
            )?))
        }
        "add" => decode_binary::<C>(object, path, package, ComposedExactExpr::Add),
        "subtract" => decode_binary::<C>(object, path, package, ComposedExactExpr::Subtract),
        "multiply" => decode_binary::<C>(object, path, package, ComposedExactExpr::Multiply),
        "floorDivide" => decode_binary::<C>(object, path, package, ComposedExactExpr::FloorDivide),
        "truncatingDivide" => {
            decode_binary::<C>(object, path, package, ComposedExactExpr::TruncatingDivide)
        }
        "min" => decode_aggregate::<C>(object, path, package, ComposedExactExpr::Min),
        "max" => decode_aggregate::<C>(object, path, package, ComposedExactExpr::Max),
        "product" => {
            fields(
                object,
                &["op", "kind", "subject", "source", "payload"],
                path,
            )?;
            let kind =
                ComposedExactLeafKindId::parse(string(object, "kind", path)?).map_err(|error| {
                    ComposedExactError::Wire(ComposedExactDefinitionError::Role(error))
                })?;
            let subject = RuleSubjectId::parse(string(object, "subject", path)?)
                .map_err(ComposedExactError::Package)?;
            let source = RuleSourceId::parse(string(object, "source", path)?)
                .map_err(ComposedExactError::Package)?;
            validate_correlation(package, &subject, &source)?;
            let payload = required(object, "payload", path)?;
            let wire_bytes = canonical_payload_bytes(payload)?;
            let leaf = C::decode_leaf(&kind, payload).map_err(|error| {
                ComposedExactError::ProductDecode {
                    context: Box::new(ComposedExactProductContext::new(
                        bounded_path(path),
                        C::schema(),
                        kind.clone(),
                        subject.clone(),
                        source.clone(),
                    )),
                    error: Box::new(error),
                }
            })?;
            let encoded = C::encode_leaf(&kind, &leaf).map_err(|error| {
                ComposedExactError::ProductEncode {
                    context: Box::new(ComposedExactProductContext::new(
                        bounded_path(path),
                        C::schema(),
                        kind.clone(),
                        subject.clone(),
                        source.clone(),
                    )),
                    error: Box::new(error),
                }
            })?;
            let encoded_bytes = canonical_payload_bytes(&encoded)?;
            if encoded_bytes != wire_bytes {
                return Err(ComposedExactError::ProductNonConvergentPayload {
                    context: Box::new(ComposedExactProductContext::new(
                        bounded_path(path),
                        C::schema(),
                        kind,
                        subject,
                        source,
                    )),
                });
            }
            Ok(ComposedExactExpr::Product(ComposedExactProductLeaf::new(
                kind, subject, source, leaf,
            )))
        }
        _ => Err(ComposedExactError::Wire(malformed(
            &format!("{path}.op"),
            "is not a supported composed exact operation",
        ))),
    }
}
#[derive(Default)]
pub(crate) struct WirePreflight {
    nodes: usize,
    product_bytes: usize,
}
pub(crate) fn preflight_wire_tree(
    value: &Value,
    path: &str,
    package: &AdmittedRulePackage,
    depth: usize,
    preflight: &mut WirePreflight,
    roles: &[RoleRequirement],
) -> Result<(), ComposedExactDefinitionError> {
    if depth > crate::exact::MAX_EXACT_EXPRESSION_DEPTH {
        return Err(ComposedExactDefinitionError::DepthQuotaExceeded {
            actual: depth,
            maximum: crate::exact::MAX_EXACT_EXPRESSION_DEPTH,
        });
    }
    preflight.nodes += 1;
    if preflight.nodes > crate::exact::MAX_EXACT_EXPRESSION_NODES {
        return Err(ComposedExactDefinitionError::NodeQuotaExceeded {
            actual: preflight.nodes,
            maximum: crate::exact::MAX_EXACT_EXPRESSION_NODES,
        });
    }
    let object = value
        .as_object()
        .ok_or_else(|| malformed(path, "must be an object"))?;
    let op = string(object, "op", path)?;
    match op {
        "literal" => {
            fields(object, &["op", "value"], path)?;
            let value = required(object, "value", path)?.as_i64().ok_or_else(|| {
                malformed(&format!("{path}.value"), "must be an exact signed integer")
            })?;
            MechanicsScalar::new(value).map_err(|error| {
                ComposedExactDefinitionError::ExactLiteral {
                    path: path.to_owned(),
                    error,
                }
            })?;
        }
        "input" => {
            fields(object, &["op", "input"], path)?;
            let input = decode_input(required(object, "input", path)?, &format!("{path}.input"))?;
            if roles
                .binary_search_by(|role| role.role().cmp(input.role()))
                .is_err()
            {
                return Err(ComposedExactDefinitionError::UndeclaredInputRole {
                    role: input.role().clone(),
                });
            }
        }
        "add" | "subtract" | "multiply" | "floorDivide" | "truncatingDivide" => {
            fields(object, &["op", "left", "right"], path)?;
            preflight_wire_tree(
                required(object, "left", path)?,
                &format!("{path}.left"),
                package,
                depth + 1,
                preflight,
                roles,
            )?;
            preflight_wire_tree(
                required(object, "right", path)?,
                &format!("{path}.right"),
                package,
                depth + 1,
                preflight,
                roles,
            )?;
        }
        "min" | "max" => {
            fields(object, &["op", "values"], path)?;
            let values = required(object, "values", path)?
                .as_array()
                .ok_or_else(|| malformed(&format!("{path}.values"), "must be an array"))?;
            if values.is_empty() {
                return Err(ComposedExactDefinitionError::EmptyAggregate);
            }
            if values.len() > crate::exact::MAX_EXACT_MIN_MAX_ARITY {
                return Err(ComposedExactDefinitionError::ArityQuotaExceeded {
                    actual: values.len(),
                    maximum: crate::exact::MAX_EXACT_MIN_MAX_ARITY,
                });
            }
            for (index, child) in values.iter().enumerate() {
                preflight_wire_tree(
                    child,
                    &format!("{path}.values[{index}]"),
                    package,
                    depth + 1,
                    preflight,
                    roles,
                )?
            }
        }
        "product" => {
            fields(
                object,
                &["op", "kind", "subject", "source", "payload"],
                path,
            )?;
            ComposedExactLeafKindId::parse(string(object, "kind", path)?)
                .map_err(ComposedExactDefinitionError::Role)?;
            let subject = RuleSubjectId::parse(string(object, "subject", path)?)
                .map_err(ComposedExactDefinitionError::Package)?;
            let source = RuleSourceId::parse(string(object, "source", path)?)
                .map_err(ComposedExactDefinitionError::Package)?;
            validate_correlation(package, &subject, &source)?;
            let bytes = canonical_payload_len(required(object, "payload", path)?)?;
            preflight.product_bytes = preflight.product_bytes.checked_add(bytes).ok_or(
                ComposedExactDefinitionError::PayloadQuotaExceeded {
                    actual: usize::MAX,
                    maximum: crate::MAX_STANDARD_EXTENSION_PAYLOAD_BYTES,
                },
            )?;
            if preflight.product_bytes > crate::MAX_STANDARD_EXTENSION_PAYLOAD_BYTES {
                return Err(ComposedExactDefinitionError::PayloadQuotaExceeded {
                    actual: preflight.product_bytes,
                    maximum: crate::MAX_STANDARD_EXTENSION_PAYLOAD_BYTES,
                });
            }
        }
        _ => {
            return Err(malformed(
                &format!("{path}.op"),
                "is not a supported composed exact operation",
            ))
        }
    };
    Ok(())
}
pub(crate) fn decode_binary<C: ComposedExactLeafCodec>(
    object: &serde_json::Map<String, Value>,
    path: &str,
    package: &AdmittedRulePackage,
    make: impl Fn(
        Box<ComposedExactExpr<C::Leaf>>,
        Box<ComposedExactExpr<C::Leaf>>,
    ) -> ComposedExactExpr<C::Leaf>,
) -> Result<ComposedExactExpr<C::Leaf>, ComposedExactError<C::Error>> {
    fields(object, &["op", "left", "right"], path)?;
    Ok(make(
        Box::new(decode_expr::<C>(
            required(object, "left", path)?,
            &format!("{path}.left"),
            package,
        )?),
        Box::new(decode_expr::<C>(
            required(object, "right", path)?,
            &format!("{path}.right"),
            package,
        )?),
    ))
}
pub(crate) fn decode_aggregate<C: ComposedExactLeafCodec>(
    object: &serde_json::Map<String, Value>,
    path: &str,
    package: &AdmittedRulePackage,
    make: impl Fn(Vec<ComposedExactExpr<C::Leaf>>) -> ComposedExactExpr<C::Leaf>,
) -> Result<ComposedExactExpr<C::Leaf>, ComposedExactError<C::Error>> {
    fields(object, &["op", "values"], path)?;
    let values = required(object, "values", path)?
        .as_array()
        .ok_or_else(|| malformed(&format!("{path}.values"), "must be an array"))?;
    Ok(make(
        values
            .iter()
            .enumerate()
            .map(|(i, value)| decode_expr::<C>(value, &format!("{path}.values[{i}]"), package))
            .collect::<Result<_, _>>()?,
    ))
}

pub(crate) fn validate_composed_wire_structure<Leaf>(
    expr: &ComposedExactExpr<Leaf>,
    limits: crate::ExactExprLimits,
) -> Result<(), ComposedExactDefinitionError> {
    let mut nodes = 0;
    validate_wire_node(expr, 1, limits, &mut nodes)
}
pub(crate) fn validate_wire_node<Leaf>(
    expr: &ComposedExactExpr<Leaf>,
    depth: usize,
    limits: crate::ExactExprLimits,
    nodes: &mut usize,
) -> Result<(), ComposedExactDefinitionError> {
    if depth > limits.maximum_depth {
        return Err(ComposedExactDefinitionError::DepthQuotaExceeded {
            actual: depth,
            maximum: limits.maximum_depth,
        });
    };
    *nodes += 1;
    if *nodes > limits.maximum_nodes {
        return Err(ComposedExactDefinitionError::NodeQuotaExceeded {
            actual: *nodes,
            maximum: limits.maximum_nodes,
        });
    };
    match expr {
        ComposedExactExpr::Literal(_)
        | ComposedExactExpr::Input(_)
        | ComposedExactExpr::Product(_) => Ok(()),
        ComposedExactExpr::Add(a, b)
        | ComposedExactExpr::Subtract(a, b)
        | ComposedExactExpr::Multiply(a, b)
        | ComposedExactExpr::FloorDivide(a, b)
        | ComposedExactExpr::TruncatingDivide(a, b) => {
            validate_wire_node(a, depth + 1, limits, nodes)?;
            validate_wire_node(b, depth + 1, limits, nodes)
        }
        ComposedExactExpr::Min(values) | ComposedExactExpr::Max(values) => {
            if values.is_empty() {
                return Err(ComposedExactDefinitionError::EmptyAggregate);
            }
            if values.len() > limits.maximum_arity {
                return Err(ComposedExactDefinitionError::ArityQuotaExceeded {
                    actual: values.len(),
                    maximum: limits.maximum_arity,
                });
            }
            for value in values {
                validate_wire_node(value, depth + 1, limits, nodes)?
            }
            Ok(())
        }
    }
}
pub(crate) fn decode_schema(
    value: &Value,
) -> Result<StandardExtensionSchema, ComposedExactDefinitionError> {
    let object = value
        .as_object()
        .ok_or_else(|| malformed("payload.extension", "must be an object"))?;
    fields(object, &["namespace", "schemaVersion"], "payload.extension")?;
    let namespace =
        crate::CapabilityRequirementId::parse(string(object, "namespace", "payload.extension")?)
            .map_err(ComposedExactDefinitionError::Role)?;
    let version = integer(object, "schemaVersion", "payload.extension")?;
    let version = u32::try_from(version)
        .map_err(|_| malformed("payload.extension.schemaVersion", "exceeds u32"))?;
    StandardExtensionSchema::new(namespace, version)
        .map_err(ComposedExactDefinitionError::Extension)
}
pub(crate) fn schema_payload(schema: &StandardExtensionSchema) -> Value {
    json!({"namespace":schema.namespace().as_str(),"schemaVersion":schema.version()})
}
pub(crate) fn roles_payload(roles: &[RoleRequirement]) -> Value {
    Value::Array(roles.iter().map(|role|json!({"role":role.role().as_str(),"capabilities":role.capabilities().iter().map(|cap|Value::String(cap.as_str().to_owned())).collect::<Vec<_>>() })).collect())
}
pub(crate) fn decode_roles(
    value: &Value,
) -> Result<Vec<RoleRequirement>, ComposedExactDefinitionError> {
    let values = value
        .as_array()
        .ok_or_else(|| malformed("payload.roles", "must be an array"))?;
    let mut roles = Vec::new();
    for (index, value) in values.iter().enumerate() {
        let path = format!("payload.roles[{index}]");
        let object = value
            .as_object()
            .ok_or_else(|| malformed(&path, "must be an object"))?;
        fields(object, &["role", "capabilities"], &path)?;
        let role = CapabilityRoleId::parse(string(object, "role", &path)?)
            .map_err(ComposedExactDefinitionError::Role)?;
        let caps = required(object, "capabilities", &path)?
            .as_array()
            .ok_or_else(|| malformed(&format!("{path}.capabilities"), "must be an array"))?
            .iter()
            .enumerate()
            .map(|(i, v)| {
                crate::CapabilityRequirementId::parse(v.as_str().ok_or_else(|| {
                    malformed(&format!("{path}.capabilities[{i}]"), "must be a string")
                })?)
                .map_err(ComposedExactDefinitionError::Role)
            })
            .collect::<Result<Vec<_>, _>>()?;
        roles.push(RoleRequirement::new(role, caps).map_err(ComposedExactDefinitionError::Role)?)
    }
    let canonical = canonicalize_roles(roles.clone())?;
    if canonical != roles {
        return Err(malformed(
            "payload.roles",
            "must be sorted, deduplicated, and merged by role",
        ));
    }
    Ok(roles)
}
pub(crate) fn decode_input(
    value: &Value,
    path: &str,
) -> Result<ExactInputReference, ComposedExactDefinitionError> {
    use crate::{ExactInputReference as Input, StandardExactFactReference as Fact};
    let object = value
        .as_object()
        .ok_or_else(|| malformed(path, "must be an object"))?;
    let kind = string(object, "kind", path)?;
    let role = || {
        CapabilityRoleId::parse(string(object, "role", path)?)
            .map_err(ComposedExactDefinitionError::Role)
    };
    let ordinary=|make:fn(CapabilityRoleId,crate::InputId)->Input|->Result<Input,ComposedExactDefinitionError>{fields(object,&["kind","role","id"],path)?;Ok(make(role()?,crate::InputId::parse(string(object,"id",path)?).map_err(ComposedExactDefinitionError::Role)?))};
    match kind {
        "parameter" => ordinary(|r, i| Input::Parameter { role: r, id: i }),
        "fact" => ordinary(|r, i| Input::Fact { role: r, id: i }),
        "roll" => ordinary(|r, i| Input::Roll { role: r, id: i }),
        "choice" => ordinary(|r, i| Input::Choice { role: r, id: i }),
        "standardStat" => {
            fields(object, &["kind", "role", "stat"], path)?;
            Ok(Input::StandardFact(Fact::Stat {
                role: role()?,
                stat: gameplay_mechanics::StatId::parse(string(object, "stat", path)?)
                    .map_err(|e| malformed(path, &e.to_string()))?,
            }))
        }
        "standardTrackCurrent" => {
            fields(object, &["kind", "role", "track"], path)?;
            Ok(Input::StandardFact(Fact::TrackCurrent {
                role: role()?,
                track: gameplay_mechanics::TrackId::parse(string(object, "track", path)?)
                    .map_err(|e| malformed(path, &e.to_string()))?,
            }))
        }
        "standardTrackMaximum" => {
            fields(object, &["kind", "role", "track"], path)?;
            Ok(Input::StandardFact(Fact::TrackMaximum {
                role: role()?,
                track: gameplay_mechanics::TrackId::parse(string(object, "track", path)?)
                    .map_err(|e| malformed(path, &e.to_string()))?,
            }))
        }
        _ => Err(malformed(
            &format!("{path}.kind"),
            "is not a supported exact input",
        )),
    }
}
pub(crate) fn input_payload(input: &ExactInputReference) -> Value {
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
pub(crate) fn validate_correlation(
    package: &AdmittedRulePackage,
    subject: &RuleSubjectId,
    source: &RuleSourceId,
) -> Result<(), ComposedExactDefinitionError> {
    match package.correlated_source(subject) {
        Some((p, _)) if p.source() == source => Ok(()),
        Some((p, _)) => Err(ComposedExactDefinitionError::SourceMismatch {
            subject: subject.clone(),
            expected: source.clone(),
            actual: p.source().clone(),
        }),
        None => Err(ComposedExactDefinitionError::MissingCorrelation {
            subject: subject.clone(),
            source: source.clone(),
        }),
    }
}
pub(crate) fn canonical_payload_bytes(
    value: &Value,
) -> Result<Vec<u8>, ComposedExactDefinitionError> {
    gameplay_rules::canonical_rule_json_value_bytes(
        value,
        RulePackageSchemaVersion::IntegerOnlyV1,
        crate::MAX_STANDARD_EXTENSION_PAYLOAD_BYTES,
    )
    .map_err(ComposedExactDefinitionError::Package)
}
pub(crate) fn canonical_payload_len(value: &Value) -> Result<usize, ComposedExactDefinitionError> {
    canonical_payload_bytes(value).map(|bytes| bytes.len())
}
const MAX_COMPOSED_ERROR_PATH_BYTES: usize = 512;

pub(crate) fn bounded_path(path: &str) -> String {
    if path.len() <= MAX_COMPOSED_ERROR_PATH_BYTES {
        return path.to_owned();
    }
    let mut end = MAX_COMPOSED_ERROR_PATH_BYTES - 3;
    while !path.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &path[..end])
}

pub(crate) fn child_path(path: &str, suffix: &str) -> String {
    bounded_path(&format!("{path}{suffix}"))
}
pub(crate) fn required<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    path: &str,
) -> Result<&'a Value, ComposedExactDefinitionError> {
    object
        .get(name)
        .ok_or_else(|| malformed(&format!("{path}.{name}"), "is required"))
}
pub(crate) fn string<'a>(
    object: &'a serde_json::Map<String, Value>,
    name: &str,
    path: &str,
) -> Result<&'a str, ComposedExactDefinitionError> {
    required(object, name, path)?
        .as_str()
        .ok_or_else(|| malformed(&format!("{path}.{name}"), "must be a string"))
}
pub(crate) fn integer(
    object: &serde_json::Map<String, Value>,
    name: &str,
    path: &str,
) -> Result<u64, ComposedExactDefinitionError> {
    required(object, name, path)?
        .as_u64()
        .ok_or_else(|| malformed(&format!("{path}.{name}"), "must be an integer"))
}
pub(crate) fn fields(
    object: &serde_json::Map<String, Value>,
    expected: &[&str],
    path: &str,
) -> Result<(), ComposedExactDefinitionError> {
    if let Some(field) = object
        .keys()
        .find(|field| !expected.contains(&field.as_str()))
    {
        return Err(malformed(&format!("{path}.{field}"), "is not recognized"));
    }
    if let Some(field) = expected.iter().find(|field| !object.contains_key(**field)) {
        return Err(malformed(&format!("{path}.{field}"), "is required"));
    }
    Ok(())
}
pub(crate) fn malformed(path: &str, reason: &str) -> ComposedExactDefinitionError {
    ComposedExactDefinitionError::MalformedPayload {
        path: path.to_owned(),
        reason: reason.to_owned(),
    }
}
