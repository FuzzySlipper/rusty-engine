use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::json::{
    parse_strict_json, validate_json_value, BoundedJsonWriter, JsonBudget, JSON_MAX_DEPTH,
    JSON_MAX_NODES, JSON_MAX_STRING_BYTES,
};
use crate::{
    RuleDomainId, RuleFingerprint, RulePackageDependency, RulePackageError, RulePackageId,
    RulePackageIdentity, RuleSourceId, RuleSubjectId, RuleVersion, MAX_SAFE_JSON_INTEGER,
};

pub const RULE_PACKAGE_ARTIFACT_KIND: &str = "rusty.gameplay-rules.package";
pub const RULE_PACKAGE_SCHEMA_VERSION: u64 = 1;
pub const RULE_PACKAGE_BINARY64_SCHEMA_VERSION: u64 = 2;
pub const MAX_ENCODED_RULE_PACKAGE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_DEPENDENCIES_PER_RULE_PACKAGE: usize = 32;
pub const MAX_SOURCES_PER_RULE_PACKAGE: usize = 64;
pub const MAX_PROVENANCE_PER_RULE_PACKAGE: usize = 4_096;
pub const MAX_SOURCE_PATH_BYTES: usize = 512;
pub const MAX_JSON_NESTING_DEPTH: usize = JSON_MAX_DEPTH;
pub const MAX_JSON_NODES_PER_RULE_PACKAGE: usize = JSON_MAX_NODES;
pub const MAX_JSON_STRING_BYTES: usize = JSON_MAX_STRING_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulePackageSchemaVersion {
    IntegerOnlyV1,
    Binary64V2,
}

impl RulePackageSchemaVersion {
    pub const fn get(self) -> u64 {
        match self {
            Self::IntegerOnlyV1 => RULE_PACKAGE_SCHEMA_VERSION,
            Self::Binary64V2 => RULE_PACKAGE_BINARY64_SCHEMA_VERSION,
        }
    }

    fn parse(value: &str) -> Result<Self, RulePackageError> {
        match value {
            "1" => Ok(Self::IntegerOnlyV1),
            "2" => Ok(Self::Binary64V2),
            _ => Err(RulePackageError::UnsupportedSchemaVersion {
                actual: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSource {
    id: RuleSourceId,
    path: String,
}

impl RuleSource {
    pub fn new(id: RuleSourceId, path: impl Into<String>) -> Result<Self, RulePackageError> {
        Self::new_at(id, path.into(), "source.path")
    }

    fn new_at(
        id: RuleSourceId,
        path: String,
        logical_path: &str,
    ) -> Result<Self, RulePackageError> {
        validate_source_path(&path, logical_path)?;
        Ok(Self { id, path })
    }

    pub const fn id(&self) -> &RuleSourceId {
        &self.id
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleProvenance {
    subject: RuleSubjectId,
    source: RuleSourceId,
    line: Option<u64>,
    column: Option<u64>,
}

impl RuleProvenance {
    pub fn new(
        subject: RuleSubjectId,
        source: RuleSourceId,
        line: Option<u64>,
        column: Option<u64>,
    ) -> Result<Self, RulePackageError> {
        Self::new_at(subject, source, line, column, "provenance")
    }

    fn new_at(
        subject: RuleSubjectId,
        source: RuleSourceId,
        line: Option<u64>,
        column: Option<u64>,
        path: &str,
    ) -> Result<Self, RulePackageError> {
        validate_source_location(line, &format!("{path}/line"))?;
        validate_source_location(column, &format!("{path}/column"))?;
        Ok(Self {
            subject,
            source,
            line,
            column,
        })
    }

    pub const fn subject(&self) -> &RuleSubjectId {
        &self.subject
    }

    pub const fn source(&self) -> &RuleSourceId {
        &self.source
    }

    pub const fn line(&self) -> Option<u64> {
        self.line
    }

    pub const fn column(&self) -> Option<u64> {
        self.column
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulePackageCandidate {
    schema_version: RulePackageSchemaVersion,
    identity: RulePackageIdentity,
    dependencies: Vec<RulePackageDependency>,
    sources: Vec<RuleSource>,
    provenance: Vec<RuleProvenance>,
    payload: Value,
}

impl RulePackageCandidate {
    pub fn new(
        domain: RuleDomainId,
        package: RulePackageId,
        version: RuleVersion,
        dependencies: Vec<RulePackageDependency>,
        sources: Vec<RuleSource>,
        provenance: Vec<RuleProvenance>,
        payload: Value,
    ) -> Self {
        Self::new_with_schema(
            RulePackageSchemaVersion::IntegerOnlyV1,
            domain,
            package,
            version,
            dependencies,
            sources,
            provenance,
            payload,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_schema(
        schema_version: RulePackageSchemaVersion,
        domain: RuleDomainId,
        package: RulePackageId,
        version: RuleVersion,
        dependencies: Vec<RulePackageDependency>,
        sources: Vec<RuleSource>,
        provenance: Vec<RuleProvenance>,
        payload: Value,
    ) -> Self {
        Self {
            schema_version,
            identity: RulePackageIdentity::new(domain, package, version),
            dependencies,
            sources,
            provenance,
            payload,
        }
    }

    pub const fn schema_version(&self) -> RulePackageSchemaVersion {
        self.schema_version
    }

    pub const fn identity(&self) -> &RulePackageIdentity {
        &self.identity
    }

    pub fn dependencies(&self) -> &[RulePackageDependency] {
        &self.dependencies
    }

    pub fn sources(&self) -> &[RuleSource] {
        &self.sources
    }

    pub fn provenance(&self) -> &[RuleProvenance] {
        &self.provenance
    }

    pub const fn payload(&self) -> &Value {
        &self.payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedRulePackage {
    schema_version: RulePackageSchemaVersion,
    identity: RulePackageIdentity,
    dependencies: Vec<RulePackageDependency>,
    sources: Vec<RuleSource>,
    provenance: Vec<RuleProvenance>,
    payload: Value,
    canonical_bytes: Vec<u8>,
    fingerprint: RuleFingerprint,
    json_nodes: usize,
}

impl AdmittedRulePackage {
    pub const fn schema_version(&self) -> RulePackageSchemaVersion {
        self.schema_version
    }

    pub const fn identity(&self) -> &RulePackageIdentity {
        &self.identity
    }

    pub fn dependencies(&self) -> &[RulePackageDependency] {
        &self.dependencies
    }

    pub fn sources(&self) -> &[RuleSource] {
        &self.sources
    }

    pub fn provenance(&self) -> &[RuleProvenance] {
        &self.provenance
    }

    pub const fn payload(&self) -> &Value {
        &self.payload
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn fingerprint(&self) -> &RuleFingerprint {
        &self.fingerprint
    }

    pub const fn json_nodes(&self) -> usize {
        self.json_nodes
    }

    pub fn source(&self, id: &RuleSourceId) -> Option<&RuleSource> {
        self.sources
            .binary_search_by(|source| source.id.cmp(id))
            .ok()
            .map(|index| &self.sources[index])
    }

    pub fn provenance_for(&self, subject: &RuleSubjectId) -> Option<&RuleProvenance> {
        self.provenance
            .binary_search_by(|provenance| provenance.subject.cmp(subject))
            .ok()
            .map(|index| &self.provenance[index])
    }

    pub fn correlated_source(
        &self,
        subject: &RuleSubjectId,
    ) -> Option<(&RuleProvenance, &RuleSource)> {
        let provenance = self.provenance_for(subject)?;
        let source = self.source(provenance.source())?;
        Some((provenance, source))
    }
}

pub fn admit_rule_package(
    mut candidate: RulePackageCandidate,
) -> Result<AdmittedRulePackage, RulePackageError> {
    enforce_quota(
        "dependencies",
        candidate.dependencies.len(),
        MAX_DEPENDENCIES_PER_RULE_PACKAGE,
    )?;
    enforce_quota(
        "sources",
        candidate.sources.len(),
        MAX_SOURCES_PER_RULE_PACKAGE,
    )?;
    enforce_quota(
        "provenance",
        candidate.provenance.len(),
        MAX_PROVENANCE_PER_RULE_PACKAGE,
    )?;

    candidate.dependencies.sort();
    if let Some(pair) = candidate
        .dependencies
        .windows(2)
        .find(|pair| pair[0].identity() == pair[1].identity())
    {
        return Err(RulePackageError::DuplicateDependency {
            dependency: pair[1].clone(),
        });
    }
    if let Some(dependency) = candidate.dependencies.iter().find(|dependency| {
        dependency.domain() == candidate.identity.domain()
            && dependency.package() == candidate.identity.package()
    }) {
        return Err(RulePackageError::SelfDependency {
            dependency: dependency.clone(),
        });
    }

    candidate
        .sources
        .sort_by(|left, right| left.id.cmp(&right.id));
    if let Some(pair) = candidate
        .sources
        .windows(2)
        .find(|pair| pair[0].id == pair[1].id)
    {
        return Err(RulePackageError::DuplicateSource {
            source: pair[1].id.clone(),
        });
    }

    candidate
        .provenance
        .sort_by(|left, right| left.subject.cmp(&right.subject));
    if let Some(pair) = candidate
        .provenance
        .windows(2)
        .find(|pair| pair[0].subject == pair[1].subject)
    {
        return Err(RulePackageError::DuplicateProvenance {
            subject: pair[1].subject.clone(),
        });
    }
    let source_ids = candidate
        .sources
        .iter()
        .map(|source| &source.id)
        .collect::<BTreeSet<_>>();
    if let Some(provenance) = candidate
        .provenance
        .iter()
        .find(|provenance| !source_ids.contains(&provenance.source))
    {
        return Err(RulePackageError::UnknownProvenanceSource {
            subject: provenance.subject.clone(),
            source: provenance.source.clone(),
        });
    }

    let json_nodes = validate_candidate_json(&mut candidate)?;
    let canonical_bytes = canonical_bytes(&candidate)?;
    let fingerprint = RuleFingerprint::for_bytes(&canonical_bytes);
    Ok(AdmittedRulePackage {
        schema_version: candidate.schema_version,
        identity: candidate.identity,
        dependencies: candidate.dependencies,
        sources: candidate.sources,
        provenance: candidate.provenance,
        payload: candidate.payload,
        canonical_bytes,
        fingerprint,
        json_nodes,
    })
}

pub fn decode_rule_package(input: &[u8]) -> Result<AdmittedRulePackage, RulePackageError> {
    if input.len() > MAX_ENCODED_RULE_PACKAGE_BYTES {
        return Err(RulePackageError::ArtifactQuotaExceeded {
            actual: input.len(),
            maximum: MAX_ENCODED_RULE_PACKAGE_BYTES,
        });
    }
    let input = std::str::from_utf8(input).map_err(|error| RulePackageError::MalformedUtf8 {
        valid_up_to: error.valid_up_to(),
    })?;
    let (value, parsed_nodes) = parse_strict_json(
        input,
        &[
            ("$/dependencies", MAX_DEPENDENCIES_PER_RULE_PACKAGE),
            ("$/sources", MAX_SOURCES_PER_RULE_PACKAGE),
            ("$/provenance", MAX_PROVENANCE_PER_RULE_PACKAGE),
        ],
    )?;
    let candidate = candidate_from_value(value)?;
    let package = admit_rule_package(candidate)?;
    debug_assert_eq!(package.json_nodes, parsed_nodes);
    Ok(package)
}

pub fn decode_canonical_rule_package(
    input: &[u8],
) -> Result<AdmittedRulePackage, RulePackageError> {
    let package = decode_rule_package(input)?;
    if package.canonical_bytes != input {
        return Err(RulePackageError::NonCanonicalArtifact {
            canonical_fingerprint: package.fingerprint.clone(),
        });
    }
    Ok(package)
}

pub fn encode_rule_package(package: &AdmittedRulePackage) -> Vec<u8> {
    package.canonical_bytes.clone()
}

fn validate_candidate_json(
    candidate: &mut RulePackageCandidate,
) -> Result<usize, RulePackageError> {
    let mut budget = JsonBudget::new();
    budget.add_node("$")?;
    for path in [
        "$/kind",
        "$/schemaVersion",
        "$/domain",
        "$/package",
        "$/version",
    ] {
        budget.add_node(path)?;
    }
    budget.add_node("$/dependencies")?;
    for (index, dependency) in candidate.dependencies.iter().enumerate() {
        let path = format!("$/dependencies/{index}");
        budget.add_node(&path)?;
        for field in ["domain", "package", "version"] {
            budget.add_node(&format!("{path}/{field}"))?;
        }
        if dependency.fingerprint().is_some() {
            budget.add_node(&format!("{path}/fingerprint"))?;
        }
    }
    budget.add_node("$/sources")?;
    for index in 0..candidate.sources.len() {
        let path = format!("$/sources/{index}");
        budget.add_node(&path)?;
        budget.add_node(&format!("{path}/id"))?;
        budget.add_node(&format!("{path}/path"))?;
    }
    budget.add_node("$/provenance")?;
    for (index, provenance) in candidate.provenance.iter().enumerate() {
        let path = format!("$/provenance/{index}");
        budget.add_node(&path)?;
        budget.add_node(&format!("{path}/subject"))?;
        budget.add_node(&format!("{path}/source"))?;
        if provenance.line.is_some() {
            budget.add_node(&format!("{path}/line"))?;
        }
        if provenance.column.is_some() {
            budget.add_node(&format!("{path}/column"))?;
        }
    }
    validate_json_value(
        &mut candidate.payload,
        candidate.schema_version,
        2,
        "$/payload",
        &mut budget,
    )?;
    Ok(budget.nodes())
}

/// Encodes one JSON value with the exact canonical writer used by rule packages.
///
/// This is intentionally a value-level helper for bounded sub-artifacts such as a
/// typed extension leaf. It validates a cloned value under the selected package
/// schema before writing, so callers cannot accidentally measure a representation
/// that a full package would reject or normalize differently.
pub fn canonical_rule_json_value_bytes(
    value: &Value,
    schema_version: RulePackageSchemaVersion,
    maximum_bytes: usize,
) -> Result<Vec<u8>, RulePackageError> {
    let mut value = value.clone();
    let mut budget = JsonBudget::new();
    validate_json_value(&mut value, schema_version, 1, "$", &mut budget)?;
    let mut output = BoundedJsonWriter::new(maximum_bytes);
    output.write_value(&value, "$")?;
    Ok(output.into_bytes())
}

/// Returns the byte length of one canonical JSON value using package semantics.
pub fn canonical_rule_json_value_len(
    value: &Value,
    schema_version: RulePackageSchemaVersion,
    maximum_bytes: usize,
) -> Result<usize, RulePackageError> {
    canonical_rule_json_value_bytes(value, schema_version, maximum_bytes).map(|bytes| bytes.len())
}

fn canonical_bytes(candidate: &RulePackageCandidate) -> Result<Vec<u8>, RulePackageError> {
    let mut output = BoundedJsonWriter::new(MAX_ENCODED_RULE_PACKAGE_BYTES);
    output.extend(br#"{"kind":"#, "$/kind")?;
    output.write_string(RULE_PACKAGE_ARTIFACT_KIND, "$/kind")?;
    output.extend(br#","schemaVersion":"#, "$/schemaVersion")?;
    output.extend(
        candidate.schema_version.get().to_string().as_bytes(),
        "$/schemaVersion",
    )?;
    output.extend(br#","domain":"#, "$/domain")?;
    output.write_string(candidate.identity.domain().as_str(), "$/domain")?;
    output.extend(br#","package":"#, "$/package")?;
    output.write_string(candidate.identity.package().as_str(), "$/package")?;
    output.extend(br#","version":"#, "$/version")?;
    output.extend(
        candidate.identity.version().get().to_string().as_bytes(),
        "$/version",
    )?;
    output.extend(br#","dependencies":["#, "$/dependencies")?;
    for (index, dependency) in candidate.dependencies.iter().enumerate() {
        if index != 0 {
            output.push(b',', "$/dependencies")?;
        }
        let path = format!("$/dependencies/{index}");
        output.extend(br#"{"domain":"#, &format!("{path}/domain"))?;
        output.write_string(dependency.domain().as_str(), &format!("{path}/domain"))?;
        output.extend(br#","package":"#, &format!("{path}/package"))?;
        output.write_string(dependency.package().as_str(), &format!("{path}/package"))?;
        output.extend(br#","version":"#, &format!("{path}/version"))?;
        output.extend(
            dependency.version().get().to_string().as_bytes(),
            &format!("{path}/version"),
        )?;
        if let Some(fingerprint) = dependency.fingerprint() {
            output.extend(br#","fingerprint":"#, &format!("{path}/fingerprint"))?;
            output.write_string(fingerprint.as_str(), &format!("{path}/fingerprint"))?;
        }
        output.push(b'}', &path)?;
    }
    output.extend(br#"],"sources":["#, "$/sources")?;
    for (index, source) in candidate.sources.iter().enumerate() {
        if index != 0 {
            output.push(b',', "$/sources")?;
        }
        let path = format!("$/sources/{index}");
        output.extend(br#"{"id":"#, &format!("{path}/id"))?;
        output.write_string(source.id.as_str(), &format!("{path}/id"))?;
        output.extend(br#","path":"#, &format!("{path}/path"))?;
        output.write_string(&source.path, &format!("{path}/path"))?;
        output.push(b'}', &path)?;
    }
    output.extend(br#"],"provenance":["#, "$/provenance")?;
    for (index, provenance) in candidate.provenance.iter().enumerate() {
        if index != 0 {
            output.push(b',', "$/provenance")?;
        }
        let path = format!("$/provenance/{index}");
        output.extend(br#"{"subject":"#, &format!("{path}/subject"))?;
        output.write_string(provenance.subject.as_str(), &format!("{path}/subject"))?;
        output.extend(br#","source":"#, &format!("{path}/source"))?;
        output.write_string(provenance.source.as_str(), &format!("{path}/source"))?;
        if let Some(line) = provenance.line {
            output.extend(br#","line":"#, &format!("{path}/line"))?;
            output.extend(line.to_string().as_bytes(), &format!("{path}/line"))?;
        }
        if let Some(column) = provenance.column {
            output.extend(br#","column":"#, &format!("{path}/column"))?;
            output.extend(column.to_string().as_bytes(), &format!("{path}/column"))?;
        }
        output.push(b'}', &path)?;
    }
    output.extend(br#"],"payload":"#, "$/payload")?;
    output.write_value(&candidate.payload, "$/payload")?;
    output.extend(b"}\n", "$")?;
    Ok(output.into_bytes())
}

fn candidate_from_value(value: Value) -> Result<RulePackageCandidate, RulePackageError> {
    let mut root = into_object(value, "$")?;
    ensure_known_fields(
        &root,
        &[
            "kind",
            "schemaVersion",
            "domain",
            "package",
            "version",
            "dependencies",
            "sources",
            "provenance",
            "payload",
        ],
        "$",
    )?;
    let kind = into_string(take_required(&mut root, "kind", "$")?, "$/kind")?;
    if kind != RULE_PACKAGE_ARTIFACT_KIND {
        return Err(RulePackageError::WrongArtifactKind { actual: kind });
    }
    let schema = into_integer_string(
        take_required(&mut root, "schemaVersion", "$")?,
        "$/schemaVersion",
    )?;
    let schema_version = RulePackageSchemaVersion::parse(&schema)?;
    let domain = RuleDomainId::parse_at(
        into_string(take_required(&mut root, "domain", "$")?, "$/domain")?,
        "$/domain",
    )?;
    let package = RulePackageId::parse_at(
        into_string(take_required(&mut root, "package", "$")?, "$/package")?,
        "$/package",
    )?;
    let version = parse_version(take_required(&mut root, "version", "$")?, "$/version")?;
    let dependencies = parse_dependencies(take_required(&mut root, "dependencies", "$")?)?;
    let sources = parse_sources(take_required(&mut root, "sources", "$")?)?;
    let provenance = parse_provenance(take_required(&mut root, "provenance", "$")?)?;
    let payload = take_required(&mut root, "payload", "$")?;
    Ok(RulePackageCandidate::new_with_schema(
        schema_version,
        domain,
        package,
        version,
        dependencies,
        sources,
        provenance,
        payload,
    ))
}

fn parse_dependencies(value: Value) -> Result<Vec<RulePackageDependency>, RulePackageError> {
    let values = into_array(value, "$/dependencies")?;
    enforce_quota(
        "$/dependencies",
        values.len(),
        MAX_DEPENDENCIES_PER_RULE_PACKAGE,
    )?;
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let path = format!("$/dependencies/{index}");
            let mut value = into_object(value, &path)?;
            ensure_known_fields(
                &value,
                &["domain", "package", "version", "fingerprint"],
                &path,
            )?;
            let domain_path = format!("{path}/domain");
            let domain = RuleDomainId::parse_at(
                into_string(take_required(&mut value, "domain", &path)?, &domain_path)?,
                &domain_path,
            )?;
            let package_path = format!("{path}/package");
            let package = RulePackageId::parse_at(
                into_string(take_required(&mut value, "package", &path)?, &package_path)?,
                &package_path,
            )?;
            let version = parse_version(
                take_required(&mut value, "version", &path)?,
                &format!("{path}/version"),
            )?;
            let fingerprint = value
                .remove("fingerprint")
                .map(|value| {
                    let fingerprint_path = format!("{path}/fingerprint");
                    let value = into_string(value, &fingerprint_path)?;
                    RuleFingerprint::parse_at(value, &fingerprint_path)
                })
                .transpose()?;
            Ok(RulePackageDependency::new(
                domain,
                package,
                version,
                fingerprint,
            ))
        })
        .collect()
}

fn parse_sources(value: Value) -> Result<Vec<RuleSource>, RulePackageError> {
    let values = into_array(value, "$/sources")?;
    enforce_quota("$/sources", values.len(), MAX_SOURCES_PER_RULE_PACKAGE)?;
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let path = format!("$/sources/{index}");
            let mut value = into_object(value, &path)?;
            ensure_known_fields(&value, &["id", "path"], &path)?;
            let id_path = format!("{path}/id");
            let id = RuleSourceId::parse_at(
                into_string(take_required(&mut value, "id", &path)?, &id_path)?,
                &id_path,
            )?;
            let source_path_path = format!("{path}/path");
            let source_path =
                into_string(take_required(&mut value, "path", &path)?, &source_path_path)?;
            RuleSource::new_at(id, source_path, &source_path_path)
        })
        .collect()
}

fn parse_provenance(value: Value) -> Result<Vec<RuleProvenance>, RulePackageError> {
    let values = into_array(value, "$/provenance")?;
    enforce_quota(
        "$/provenance",
        values.len(),
        MAX_PROVENANCE_PER_RULE_PACKAGE,
    )?;
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let path = format!("$/provenance/{index}");
            let mut value = into_object(value, &path)?;
            ensure_known_fields(&value, &["subject", "source", "line", "column"], &path)?;
            let subject_path = format!("{path}/subject");
            let subject = RuleSubjectId::parse_at(
                into_string(take_required(&mut value, "subject", &path)?, &subject_path)?,
                &subject_path,
            )?;
            let source_path = format!("{path}/source");
            let source = RuleSourceId::parse_at(
                into_string(take_required(&mut value, "source", &path)?, &source_path)?,
                &source_path,
            )?;
            let line = value
                .remove("line")
                .map(|value| parse_location(value, &format!("{path}/line")))
                .transpose()?;
            let column = value
                .remove("column")
                .map(|value| parse_location(value, &format!("{path}/column")))
                .transpose()?;
            RuleProvenance::new_at(subject, source, line, column, &path)
        })
        .collect()
}

fn validate_source_path(value: &str, path: &str) -> Result<(), RulePackageError> {
    if value.is_empty() {
        return Err(RulePackageError::InvalidSourcePath {
            path: path.to_string(),
            reason: "source path is empty",
        });
    }
    if value.len() > MAX_SOURCE_PATH_BYTES {
        return Err(RulePackageError::QuotaExceeded {
            path: path.to_string(),
            actual: value.len(),
            maximum: MAX_SOURCE_PATH_BYTES,
        });
    }
    if value.trim() != value {
        return Err(RulePackageError::InvalidSourcePath {
            path: path.to_string(),
            reason: "source path has leading or trailing whitespace",
        });
    }
    if value.chars().any(char::is_control) {
        return Err(RulePackageError::InvalidSourcePath {
            path: path.to_string(),
            reason: "source path contains a control character",
        });
    }
    Ok(())
}

fn validate_source_location(value: Option<u64>, path: &str) -> Result<(), RulePackageError> {
    if let Some(value) = value {
        if value == 0 || value > MAX_SAFE_JSON_INTEGER {
            return Err(RulePackageError::InvalidSourceLocation {
                path: path.to_string(),
                value: value.to_string(),
            });
        }
    }
    Ok(())
}

fn parse_version(value: Value, path: &str) -> Result<RuleVersion, RulePackageError> {
    let value = into_u64(value, path).map_err(|_| RulePackageError::InvalidVersion {
        path: path.to_string(),
        value: "non-positive or non-integer".to_string(),
    })?;
    RuleVersion::new_at(value, path)
}

fn parse_location(value: Value, path: &str) -> Result<u64, RulePackageError> {
    let value = into_u64(value, path).map_err(|_| RulePackageError::InvalidSourceLocation {
        path: path.to_string(),
        value: "non-positive or non-integer".to_string(),
    })?;
    validate_source_location(Some(value), path)?;
    Ok(value)
}

fn ensure_known_fields(
    value: &Map<String, Value>,
    fields: &[&str],
    path: &str,
) -> Result<(), RulePackageError> {
    if let Some(field) = value.keys().find(|field| !fields.contains(&field.as_str())) {
        return Err(RulePackageError::UnknownField {
            path: format!("{path}/{field}"),
        });
    }
    Ok(())
}

fn take_required(
    value: &mut Map<String, Value>,
    field: &str,
    path: &str,
) -> Result<Value, RulePackageError> {
    value
        .remove(field)
        .ok_or_else(|| RulePackageError::MissingField {
            path: format!("{path}/{field}"),
        })
}

fn into_object(value: Value, path: &str) -> Result<Map<String, Value>, RulePackageError> {
    match value {
        Value::Object(value) => Ok(value),
        _ => Err(RulePackageError::InvalidFieldType {
            path: path.to_string(),
            expected: "object",
        }),
    }
}

fn into_array(value: Value, path: &str) -> Result<Vec<Value>, RulePackageError> {
    match value {
        Value::Array(value) => Ok(value),
        _ => Err(RulePackageError::InvalidFieldType {
            path: path.to_string(),
            expected: "array",
        }),
    }
}

fn into_string(value: Value, path: &str) -> Result<String, RulePackageError> {
    match value {
        Value::String(value) => Ok(value),
        _ => Err(RulePackageError::InvalidFieldType {
            path: path.to_string(),
            expected: "string",
        }),
    }
}

fn into_integer_string(value: Value, path: &str) -> Result<String, RulePackageError> {
    match value {
        Value::Number(value) if value.as_i64().is_some() || value.as_u64().is_some() => {
            Ok(value.to_string())
        }
        _ => Err(RulePackageError::InvalidFieldType {
            path: path.to_string(),
            expected: "integer",
        }),
    }
}

fn into_u64(value: Value, path: &str) -> Result<u64, RulePackageError> {
    match value {
        Value::Number(value) => value
            .as_u64()
            .ok_or_else(|| RulePackageError::InvalidFieldType {
                path: path.to_string(),
                expected: "positive integer",
            }),
        _ => Err(RulePackageError::InvalidFieldType {
            path: path.to_string(),
            expected: "positive integer",
        }),
    }
}

fn enforce_quota(path: &str, actual: usize, maximum: usize) -> Result<(), RulePackageError> {
    if actual > maximum {
        return Err(RulePackageError::QuotaExceeded {
            path: path.to_string(),
            actual,
            maximum,
        });
    }
    Ok(())
}
