use gameplay_rules::*;
use serde_json::{json, Value};

fn domain(value: &str) -> RuleDomainId {
    RuleDomainId::parse(value).unwrap()
}

fn package_id(value: &str) -> RulePackageId {
    RulePackageId::parse(value).unwrap()
}

fn source_id(value: &str) -> RuleSourceId {
    RuleSourceId::parse(value).unwrap()
}

fn subject_id(value: &str) -> RuleSubjectId {
    RuleSubjectId::parse(value).unwrap()
}

fn version(value: u64) -> RuleVersion {
    RuleVersion::new(value).unwrap()
}

fn dependency(package: &str, version_value: u64) -> RulePackageDependency {
    RulePackageDependency::new(
        domain("test"),
        package_id(package),
        version(version_value),
        None,
    )
}

fn candidate(
    package: &str,
    version_value: u64,
    dependencies: Vec<RulePackageDependency>,
    sources: Vec<RuleSource>,
    provenance: Vec<RuleProvenance>,
    payload: Value,
) -> RulePackageCandidate {
    RulePackageCandidate::new(
        domain("test"),
        package_id(package),
        version(version_value),
        dependencies,
        sources,
        provenance,
        payload,
    )
}

fn admit(package: &str, dependencies: Vec<RulePackageDependency>) -> AdmittedRulePackage {
    admit_rule_package(candidate(
        package,
        1,
        dependencies,
        vec![],
        vec![],
        Value::Null,
    ))
    .unwrap()
}

#[test]
fn direct_and_decoded_paths_converge_on_one_immutable_canonical_package() {
    let sources = vec![
        RuleSource::new(source_id("z"), "rules/z.ts").unwrap(),
        RuleSource::new(source_id("a"), "rules/a.ts").unwrap(),
    ];
    let provenance = vec![
        RuleProvenance::new(subject_id("z.subject"), source_id("z"), Some(9), None).unwrap(),
        RuleProvenance::new(subject_id("a.subject"), source_id("a"), Some(3), Some(7)).unwrap(),
    ];
    let direct = admit_rule_package(candidate(
        "consumer",
        3,
        vec![dependency("z-dependency", 2), dependency("a-dependency", 1)],
        sources,
        provenance,
        json!({
            "z": ["é", -9_007_199_254_740_991_i64],
            "a": {"quoted": "line\nbreak"}
        }),
    ))
    .unwrap();

    let canonical = encode_rule_package(&direct);
    assert_eq!(canonical.last(), Some(&b'\n'));
    assert_eq!(decode_canonical_rule_package(&canonical).unwrap(), direct);

    let noncanonical =
        serde_json::to_vec_pretty(&serde_json::from_slice::<Value>(&canonical).unwrap()).unwrap();
    let decoded = decode_rule_package(&noncanonical).unwrap();
    assert_eq!(decoded, direct);
    assert_eq!(
        decoded
            .correlated_source(&subject_id("a.subject"))
            .unwrap()
            .1
            .path(),
        "rules/a.ts"
    );
    assert!(matches!(
        decode_canonical_rule_package(&noncanonical),
        Err(RulePackageError::NonCanonicalArtifact { .. })
    ));
}

#[test]
fn checked_fixture_locks_canonical_bytes_and_fingerprint() {
    let fixture = include_bytes!("../../../../fixtures/gameplay-rules/package-v1.canonical.json");
    let package = decode_canonical_rule_package(fixture).unwrap();
    assert_eq!(package.identity().domain().as_str(), "fixture");
    assert_eq!(package.identity().package().as_str(), "core");
    assert_eq!(
        package.fingerprint().as_str(),
        "8ef484b4505310b757c59133985608c29d38b421e02488797cf7df9a999d57b2"
    );
}

#[test]
fn unicode_fixture_locks_cross_language_key_order_and_escaping() {
    let fixture =
        include_bytes!("../../../../fixtures/gameplay-rules/package-v1-unicode.canonical.json");
    let package = decode_canonical_rule_package(fixture).unwrap();
    assert_eq!(package.identity().package().as_str(), "unicode");
    assert_eq!(encode_rule_package(&package), fixture);
    assert_eq!(
        package.payload()["a"],
        Value::String("line\nquote\"slash\\\u{1}".to_string())
    );
}

#[test]
fn strict_decode_rejects_ambiguous_or_nonportable_json_before_admission() {
    let duplicate = br#"{"kind":"rusty.gameplay-rules.package","kind":"again"}"#;
    assert!(matches!(
        decode_rule_package(duplicate),
        Err(RulePackageError::DuplicateJsonKey { path, key })
            if path == "$" && key == "kind"
    ));

    let invalid_utf8 = [b'{', b'"', 0xff, b'"', b':', b'0', b'}'];
    assert!(matches!(
        decode_rule_package(&invalid_utf8),
        Err(RulePackageError::MalformedUtf8 { .. })
    ));

    let invalid_surrogate = valid_artifact_with_payload(br#""\ud800""#);
    assert!(matches!(
        decode_rule_package(&invalid_surrogate),
        Err(RulePackageError::MalformedJson { path, .. }) if path == "$/payload"
    ));

    for payload in [
        b"1.5".as_slice(),
        b"1e2".as_slice(),
        b"9007199254740992".as_slice(),
        b"-9007199254740992".as_slice(),
    ] {
        assert!(matches!(
            decode_rule_package(&valid_artifact_with_payload(payload)),
            Err(RulePackageError::JsonIntegerOutOfRange { path, .. })
                if path == "$/payload"
        ));
    }

    let mut unknown =
        serde_json::from_slice::<Value>(&encode_rule_package(&admit("plain", vec![]))).unwrap();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("runtimeSession".to_string(), json!({}));
    assert!(matches!(
        decode_rule_package(&serde_json::to_vec(&unknown).unwrap()),
        Err(RulePackageError::UnknownField { path }) if path == "$/runtimeSession"
    ));
}

#[test]
fn binary64_schema_normalizes_and_canonicalizes_portable_float_values() {
    let promoted_f32 = f64::from(0.1_f32);
    let direct = admit_rule_package(RulePackageCandidate::new_with_schema(
        RulePackageSchemaVersion::Binary64V2,
        domain("fixture"),
        package_id("binary64"),
        version(1),
        vec![],
        vec![],
        vec![],
        json!({
            "values": [-0.0, 1.0, 1.5, 1e-6, 1e20, 1e21, 5e-324, f64::MAX]
        }),
    ))
    .unwrap();
    let expected =
        include_bytes!("../../../../fixtures/gameplay-rules/package-v2-binary64.canonical.json");
    assert_eq!(encode_rule_package(&direct), expected);
    assert_eq!(
        direct.schema_version(),
        RulePackageSchemaVersion::Binary64V2
    );
    assert_eq!(
        direct.fingerprint().as_str(),
        "03a4a6f2c65e10beaa8f689297394dcf6362fcc5c83c2b8920f199ebc0c50670"
    );
    assert_eq!(decode_canonical_rule_package(expected).unwrap(), direct);

    let promoted = admit_rule_package(RulePackageCandidate::new_with_schema(
        RulePackageSchemaVersion::Binary64V2,
        domain("fixture"),
        package_id("promoted-f32"),
        version(1),
        vec![],
        vec![],
        vec![],
        json!({ "value": promoted_f32 }),
    ))
    .unwrap();
    let round_tripped = decode_canonical_rule_package(&encode_rule_package(&promoted)).unwrap();
    assert_eq!(
        round_tripped.payload()["value"].as_f64().unwrap().to_bits(),
        promoted_f32.to_bits()
    );
}

#[test]
fn binary64_schema_rejects_non_finite_underflow_and_unsafe_bare_integers() {
    for payload in ["1e400", "1e-400"] {
        assert!(matches!(
            decode_rule_package(&valid_v2_artifact_with_payload(payload.as_bytes())),
            Err(RulePackageError::JsonNumberOutOfRange { path, .. })
                if path == "$/payload"
        ));
    }
    assert!(matches!(
        decode_rule_package(&valid_v2_artifact_with_payload(b"9007199254740993")),
        Err(RulePackageError::JsonIntegerOutOfRange { path, .. })
            if path == "$/payload"
    ));
    assert!(matches!(
        admit_rule_package(RulePackageCandidate::new_with_schema(
            RulePackageSchemaVersion::Binary64V2,
            domain("fixture"),
            package_id("unsafe-direct-integer"),
            version(1),
            vec![],
            vec![],
            vec![],
            Value::Number(serde_json::Number::from(9_007_199_254_740_993_u64)),
        )),
        Err(RulePackageError::JsonIntegerOutOfRange { path, .. })
            if path == "$/payload"
    ));
    let zero = decode_rule_package(&valid_v2_artifact_with_payload(b"0e999")).unwrap();
    assert_eq!(zero.payload().as_f64(), Some(0.0));
}

#[test]
fn malformed_binary64_tokens_remain_malformed_json() {
    for payload in ["1.", "1e", "1e+", "1e-"] {
        assert!(matches!(
            decode_rule_package(&valid_v2_artifact_with_payload(payload.as_bytes())),
            Err(RulePackageError::MalformedJson { path, .. })
                if path == "$/payload"
        ));
    }
}

#[test]
fn package_admission_rejects_duplicate_and_malformed_metadata() {
    let duplicated_dependency = dependency("base", 1);
    assert!(matches!(
        admit_rule_package(candidate(
            "consumer",
            1,
            vec![duplicated_dependency.clone(), duplicated_dependency],
            vec![],
            vec![],
            Value::Null,
        )),
        Err(RulePackageError::DuplicateDependency { .. })
    ));

    assert!(matches!(
        admit_rule_package(candidate(
            "self",
            1,
            vec![dependency("self", 2)],
            vec![],
            vec![],
            Value::Null,
        )),
        Err(RulePackageError::SelfDependency { .. })
    ));

    let source = RuleSource::new(source_id("one"), "rules/one.ts").unwrap();
    assert!(matches!(
        admit_rule_package(candidate(
            "sources",
            1,
            vec![],
            vec![source.clone(), source],
            vec![],
            Value::Null,
        )),
        Err(RulePackageError::DuplicateSource { .. })
    ));

    let first =
        RuleProvenance::new(subject_id("same"), source_id("one"), Some(1), Some(1)).unwrap();
    let duplicate =
        RuleProvenance::new(subject_id("same"), source_id("one"), Some(2), Some(1)).unwrap();
    assert!(matches!(
        admit_rule_package(candidate(
            "provenance",
            1,
            vec![],
            vec![RuleSource::new(source_id("one"), "one.ts").unwrap()],
            vec![first, duplicate],
            Value::Null,
        )),
        Err(RulePackageError::DuplicateProvenance { .. })
    ));

    let unknown =
        RuleProvenance::new(subject_id("subject"), source_id("missing"), Some(1), None).unwrap();
    assert!(matches!(
        admit_rule_package(candidate(
            "provenance",
            1,
            vec![],
            vec![],
            vec![unknown],
            Value::Null,
        )),
        Err(RulePackageError::UnknownProvenanceSource { .. })
    ));

    let invalid_location = valid_artifact_with_metadata(
        br#""sources":[{"id":"one","path":"one.ts"}],"provenance":[{"subject":"subject","source":"one","line":0}]"#,
    );
    assert!(matches!(
        decode_rule_package(&invalid_location),
        Err(RulePackageError::InvalidSourceLocation { path, .. })
            if path == "$/provenance/0/line"
    ));

    let oversized_dependencies = oversized_dependency_artifact();
    assert!(matches!(
        decode_rule_package(oversized_dependencies.as_bytes()),
        Err(RulePackageError::QuotaExceeded {
            path,
            actual,
            maximum,
        }) if path == "$/dependencies"
            && actual == MAX_DEPENDENCIES_PER_RULE_PACKAGE + 1
            && maximum == MAX_DEPENDENCIES_PER_RULE_PACKAGE
    ));
}

#[test]
fn deterministic_resolution_validates_the_complete_exact_dependency_set() {
    let a = admit("a", vec![]);
    let a_fingerprint = a.fingerprint().clone();
    let pinned_a = RulePackageDependency::new(
        domain("test"),
        package_id("a"),
        version(1),
        Some(a_fingerprint),
    );
    let b = admit("b", vec![]);
    let c = admit("c", vec![dependency("b", 1), pinned_a]);
    let d = admit("d", vec![]);
    let resolved = resolve_rule_packages(vec![d, c, b, a]).unwrap();
    assert_eq!(
        resolved
            .packages()
            .iter()
            .map(|package| package.identity().package().as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c", "d"]
    );
    assert_eq!(resolved.dependency_count(), 2);

    let duplicate = admit("duplicate", vec![]);
    assert!(matches!(
        resolve_rule_packages(vec![duplicate.clone(), duplicate]),
        Err(RulePackageSetError::DuplicatePackage { .. })
    ));

    let version_one = admit("versioned", vec![]);
    let version_two = admit_rule_package(candidate(
        "versioned",
        2,
        vec![],
        vec![],
        vec![],
        Value::Null,
    ))
    .unwrap();
    assert!(matches!(
        resolve_rule_packages(vec![version_two, version_one]),
        Err(RulePackageSetError::ConflictingVersions { first, second, .. })
            if first == version(1) && second == version(2)
    ));

    assert!(matches!(
        resolve_rule_packages(vec![admit("consumer", vec![dependency("missing", 1)])]),
        Err(RulePackageSetError::MissingDependency { .. })
    ));

    let available = admit("base", vec![]);
    let wrong_version = admit("consumer", vec![dependency("base", 2)]);
    assert!(matches!(
        resolve_rule_packages(vec![wrong_version, available.clone()]),
        Err(RulePackageSetError::DependencyVersionMismatch { available, .. })
            if available == version(1)
    ));

    let wrong_pin = RulePackageDependency::new(
        domain("test"),
        package_id("base"),
        version(1),
        Some(RuleFingerprint::parse("0".repeat(64)).unwrap()),
    );
    assert!(matches!(
        resolve_rule_packages(vec![admit("consumer", vec![wrong_pin]), available]),
        Err(RulePackageSetError::DependencyFingerprintMismatch { .. })
    ));
}

#[test]
fn cycle_identity_is_deterministic_for_any_input_order() {
    let cycle = || {
        vec![
            admit("a", vec![dependency("b", 1)]),
            admit("b", vec![dependency("c", 1)]),
            admit("c", vec![dependency("a", 1)]),
        ]
    };
    let first = resolve_rule_packages(cycle()).unwrap_err();
    let mut reversed = cycle();
    reversed.reverse();
    let second = resolve_rule_packages(reversed).unwrap_err();
    assert_eq!(first, second);
    assert!(matches!(
        first,
        RulePackageSetError::DependencyCycle { packages }
            if packages
                .iter()
                .map(|identity| identity.package().as_str())
                .collect::<Vec<_>>()
                == ["a", "b", "c", "a"]
    ));
}

#[test]
fn diagnostics_are_bounded_source_correlated_and_sorted_stably() {
    let package = RulePackageIdentity::new(domain("test"), package_id("rules"), version(1));
    let correlation = RuleDiagnosticCorrelation::new(
        subject_id("subject"),
        source_id("source"),
        Some(2),
        Some(4),
    )
    .unwrap();
    let warning = RuleDiagnostic::new(
        "Z_WARNING",
        RuleDiagnosticSeverity::Warning,
        "$/z",
        "warning",
        Some(package.clone()),
        None,
    )
    .unwrap();
    let error = RuleDiagnostic::new(
        "A_ERROR",
        RuleDiagnosticSeverity::Error,
        "$/a",
        "error",
        Some(package),
        Some(correlation.clone()),
    )
    .unwrap();
    let report = RuleDiagnosticReport::new(vec![warning, error]).unwrap();
    assert!(report.has_errors());
    assert_eq!(report.diagnostics()[0].code(), "A_ERROR");
    assert_eq!(
        report.diagnostics()[0]
            .correlation()
            .unwrap()
            .subject()
            .as_str(),
        "subject"
    );

    assert!(RuleDiagnosticCorrelation::new(
        subject_id("subject"),
        source_id("source"),
        Some(0),
        None,
    )
    .is_err());
    assert!(RuleDiagnostic::new(
        "bad code",
        RuleDiagnosticSeverity::Error,
        "$",
        "message",
        None,
        None,
    )
    .is_ok());
    assert!(RuleDiagnostic::new(
        "\n",
        RuleDiagnosticSeverity::Error,
        "$",
        "message",
        None,
        None,
    )
    .is_err());

    assert!(RuleDiagnostic::new(
        "x".repeat(MAX_DIAGNOSTIC_CODE_BYTES),
        RuleDiagnosticSeverity::Warning,
        "x".repeat(MAX_DIAGNOSTIC_LOGICAL_PATH_BYTES),
        "x".repeat(MAX_DIAGNOSTIC_MESSAGE_BYTES),
        None,
        None,
    )
    .is_ok());
    assert!(RuleDiagnostic::new(
        "x".repeat(MAX_DIAGNOSTIC_CODE_BYTES + 1),
        RuleDiagnosticSeverity::Warning,
        "$",
        "message",
        None,
        None,
    )
    .is_err());
    assert!(RuleDiagnostic::new(
        "CODE",
        RuleDiagnosticSeverity::Warning,
        "x".repeat(MAX_DIAGNOSTIC_LOGICAL_PATH_BYTES + 1),
        "message",
        None,
        None,
    )
    .is_err());
    assert!(RuleDiagnostic::new(
        "CODE",
        RuleDiagnosticSeverity::Warning,
        "$",
        "x".repeat(MAX_DIAGNOSTIC_MESSAGE_BYTES + 1),
        None,
        None,
    )
    .is_err());
    assert!(RuleDiagnosticCorrelation::new(
        subject_id("subject"),
        source_id("source"),
        Some(MAX_SAFE_JSON_INTEGER),
        Some(MAX_SAFE_JSON_INTEGER),
    )
    .is_ok());
    assert!(RuleDiagnosticCorrelation::new(
        subject_id("subject"),
        source_id("source"),
        Some(MAX_SAFE_JSON_INTEGER + 1),
        None,
    )
    .is_err());

    let exact = (0..MAX_RULE_DIAGNOSTICS)
        .map(|index| {
            RuleDiagnostic::new(
                format!("E{index:03}"),
                RuleDiagnosticSeverity::Warning,
                format!("$/item/{index:03}"),
                "bounded",
                None,
                None,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        RuleDiagnosticReport::new(exact.clone())
            .unwrap()
            .diagnostics()
            .len(),
        MAX_RULE_DIAGNOSTICS
    );
    let mut one_over = exact;
    one_over.push(
        RuleDiagnostic::new(
            "OVER",
            RuleDiagnosticSeverity::Error,
            "$/over",
            "bounded",
            None,
            None,
        )
        .unwrap(),
    );
    assert!(matches!(
        RuleDiagnosticReport::new(one_over),
        Err(RuleDiagnosticError::QuotaExceeded {
            actual,
            maximum
        }) if actual == MAX_RULE_DIAGNOSTICS + 1 && maximum == MAX_RULE_DIAGNOSTICS
    ));
}

#[test]
fn per_package_bounds_accept_the_exact_limit_and_reject_one_over() {
    assert!(RulePackageId::parse("x".repeat(MAX_RULE_ID_BYTES)).is_ok());
    assert!(RulePackageId::parse("x".repeat(MAX_RULE_ID_BYTES + 1)).is_err());
    assert!(RuleVersion::new(MAX_SAFE_JSON_INTEGER).is_ok());
    assert!(RuleVersion::new(MAX_SAFE_JSON_INTEGER + 1).is_err());

    assert!(RuleSource::new(source_id("source"), "x".repeat(MAX_SOURCE_PATH_BYTES)).is_ok());
    assert!(RuleSource::new(source_id("source"), "x".repeat(MAX_SOURCE_PATH_BYTES + 1)).is_err());

    let exact_dependencies = (0..MAX_DEPENDENCIES_PER_RULE_PACKAGE)
        .map(|index| dependency(&format!("dependency-{index:02}"), 1))
        .collect::<Vec<_>>();
    assert!(admit_rule_package(candidate(
        "dependencies",
        1,
        exact_dependencies.clone(),
        vec![],
        vec![],
        Value::Null,
    ))
    .is_ok());
    let mut one_over_dependencies = exact_dependencies;
    one_over_dependencies.push(dependency("dependency-over", 1));
    assert!(matches!(
        admit_rule_package(candidate(
            "dependencies",
            1,
            one_over_dependencies,
            vec![],
            vec![],
            Value::Null,
        )),
        Err(RulePackageError::QuotaExceeded {
            actual,
            maximum,
            ..
        }) if actual == MAX_DEPENDENCIES_PER_RULE_PACKAGE + 1
            && maximum == MAX_DEPENDENCIES_PER_RULE_PACKAGE
    ));

    let exact_sources = (0..MAX_SOURCES_PER_RULE_PACKAGE)
        .map(|index| {
            RuleSource::new(
                source_id(&format!("source-{index:02}")),
                format!("source-{index:02}.ts"),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(admit_rule_package(candidate(
        "sources",
        1,
        vec![],
        exact_sources.clone(),
        vec![],
        Value::Null,
    ))
    .is_ok());
    let mut one_over_sources = exact_sources;
    one_over_sources.push(RuleSource::new(source_id("source-over"), "over.ts").unwrap());
    assert!(matches!(
        admit_rule_package(candidate(
            "sources",
            1,
            vec![],
            one_over_sources,
            vec![],
            Value::Null,
        )),
        Err(RulePackageError::QuotaExceeded {
            actual,
            maximum,
            ..
        }) if actual == MAX_SOURCES_PER_RULE_PACKAGE + 1
            && maximum == MAX_SOURCES_PER_RULE_PACKAGE
    ));

    let source = RuleSource::new(source_id("source"), "source.ts").unwrap();
    let exact_provenance = (0..MAX_PROVENANCE_PER_RULE_PACKAGE)
        .map(|index| {
            RuleProvenance::new(
                subject_id(&format!("subject-{index:04}")),
                source_id("source"),
                Some(1),
                None,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(admit_rule_package(candidate(
        "provenance",
        1,
        vec![],
        vec![source.clone()],
        exact_provenance.clone(),
        Value::Null,
    ))
    .is_ok());
    let mut one_over_provenance = exact_provenance;
    one_over_provenance.push(
        RuleProvenance::new(
            subject_id("subject-over"),
            source_id("source"),
            Some(1),
            None,
        )
        .unwrap(),
    );
    assert!(matches!(
        admit_rule_package(candidate(
            "provenance",
            1,
            vec![],
            vec![source],
            one_over_provenance,
            Value::Null,
        )),
        Err(RulePackageError::QuotaExceeded {
            actual,
            maximum,
            ..
        }) if actual == MAX_PROVENANCE_PER_RULE_PACKAGE + 1
            && maximum == MAX_PROVENANCE_PER_RULE_PACKAGE
    ));
}

#[test]
fn json_and_artifact_bounds_accept_exact_and_reject_one_over_without_expansion() {
    let exact_string = "x".repeat(MAX_JSON_STRING_BYTES);
    assert!(admit_rule_package(candidate(
        "string",
        1,
        vec![],
        vec![],
        vec![],
        Value::String(exact_string),
    ))
    .is_ok());
    assert!(matches!(
        admit_rule_package(candidate(
            "string",
            1,
            vec![],
            vec![],
            vec![],
            Value::String("x".repeat(MAX_JSON_STRING_BYTES + 1)),
        )),
        Err(RulePackageError::QuotaExceeded {
            actual,
            maximum,
            ..
        }) if actual == MAX_JSON_STRING_BYTES + 1 && maximum == MAX_JSON_STRING_BYTES
    ));

    assert!(admit_rule_package(candidate(
        "depth",
        1,
        vec![],
        vec![],
        vec![],
        nested_arrays(MAX_JSON_NESTING_DEPTH - 2),
    ))
    .is_ok());
    assert!(matches!(
        admit_rule_package(candidate(
            "depth",
            1,
            vec![],
            vec![],
            vec![],
            nested_arrays(MAX_JSON_NESTING_DEPTH - 1),
        )),
        Err(RulePackageError::JsonDepthExceeded {
            actual,
            maximum,
            ..
        }) if actual == MAX_JSON_NESTING_DEPTH + 1
            && maximum == MAX_JSON_NESTING_DEPTH
    ));

    let exact_nodes = package_with_json_nodes("nodes", MAX_JSON_NODES_PER_RULE_PACKAGE);
    assert_eq!(exact_nodes.json_nodes(), MAX_JSON_NODES_PER_RULE_PACKAGE);
    assert!(matches!(
        admit_rule_package(candidate(
            "nodes-over",
            1,
            vec![],
            vec![],
            vec![],
            Value::Array(vec![
                Value::Null;
                MAX_JSON_NODES_PER_RULE_PACKAGE - base_json_nodes("nodes-over") + 1
            ]),
        )),
        Err(RulePackageError::JsonNodeQuotaExceeded {
            actual,
            maximum,
            ..
        }) if actual == MAX_JSON_NODES_PER_RULE_PACKAGE + 1
            && maximum == MAX_JSON_NODES_PER_RULE_PACKAGE
    ));

    let exact_artifact = package_with_canonical_bytes("artifact", MAX_ENCODED_RULE_PACKAGE_BYTES);
    assert_eq!(
        exact_artifact.canonical_bytes().len(),
        MAX_ENCODED_RULE_PACKAGE_BYTES
    );
    assert!(matches!(
        package_candidate_with_canonical_bytes(
            "artifact-over",
            MAX_ENCODED_RULE_PACKAGE_BYTES + 1
        )
        .and_then(admit_rule_package),
        Err(RulePackageError::ArtifactQuotaExceeded {
            actual,
            maximum,
        }) if actual == MAX_ENCODED_RULE_PACKAGE_BYTES + 1
            && maximum == MAX_ENCODED_RULE_PACKAGE_BYTES
    ));

    let oversized_input = vec![b' '; MAX_ENCODED_RULE_PACKAGE_BYTES + 1];
    assert!(matches!(
        decode_rule_package(&oversized_input),
        Err(RulePackageError::ArtifactQuotaExceeded {
            actual,
            maximum,
        }) if actual == MAX_ENCODED_RULE_PACKAGE_BYTES + 1
            && maximum == MAX_ENCODED_RULE_PACKAGE_BYTES
    ));
}

#[test]
fn aggregate_bounds_are_checked_before_dependency_graph_work() {
    let exact_bytes = (0..4)
        .map(|index| {
            package_with_canonical_bytes(
                &format!("artifact-set-{index}"),
                MAX_ENCODED_RULE_PACKAGE_BYTES,
            )
        })
        .collect::<Vec<_>>();
    let resolved = resolve_rule_packages(exact_bytes.clone()).unwrap();
    assert_eq!(
        resolved.canonical_bytes(),
        MAX_CANONICAL_RULE_PACKAGE_SET_BYTES
    );
    drop(resolved);
    let mut bytes_over = exact_bytes;
    bytes_over.push(admit("artifact-set-over", vec![]));
    assert!(matches!(
        resolve_rule_packages(bytes_over),
        Err(RulePackageSetError::AggregateQuotaExceeded {
            field: "canonical bytes",
            ..
        })
    ));

    let bases = (0..MAX_DEPENDENCIES_PER_RULE_PACKAGE)
        .map(|index| admit(&format!("base-{index:02}"), vec![]))
        .collect::<Vec<_>>();
    let all_dependencies = (0..MAX_DEPENDENCIES_PER_RULE_PACKAGE)
        .map(|index| dependency(&format!("base-{index:02}"), 1))
        .collect::<Vec<_>>();
    let dependents = (0..(MAX_DEPENDENCIES_PER_RULE_PACKAGE_SET
        / MAX_DEPENDENCIES_PER_RULE_PACKAGE))
        .map(|index| admit(&format!("consumer-{index:02}"), all_dependencies.clone()))
        .collect::<Vec<_>>();
    let mut exact_dependencies = bases;
    exact_dependencies.extend(dependents);
    let resolved = resolve_rule_packages(exact_dependencies.clone()).unwrap();
    assert_eq!(
        resolved.dependency_count(),
        MAX_DEPENDENCIES_PER_RULE_PACKAGE_SET
    );
    drop(resolved);
    exact_dependencies.push(admit("consumer-over", vec![dependency("base-00", 1)]));
    assert!(matches!(
        resolve_rule_packages(exact_dependencies),
        Err(RulePackageSetError::AggregateQuotaExceeded {
            field: "dependencies",
            ..
        })
    ));

    assert_aggregate_source_bound();
    assert_aggregate_provenance_bound();

    let exact_nodes = (0..4)
        .map(|index| {
            package_with_json_nodes(
                &format!("node-set-{index}"),
                MAX_JSON_NODES_PER_RULE_PACKAGE,
            )
        })
        .collect::<Vec<_>>();
    let resolved = resolve_rule_packages(exact_nodes.clone()).unwrap();
    assert_eq!(resolved.json_nodes(), MAX_JSON_NODES_PER_RULE_PACKAGE_SET);
    drop(resolved);
    let mut nodes_over = exact_nodes;
    nodes_over.push(admit("node-set-over", vec![]));
    assert!(matches!(
        resolve_rule_packages(nodes_over),
        Err(RulePackageSetError::AggregateQuotaExceeded {
            field: "JSON nodes",
            ..
        })
    ));

    let exact_packages = (0..MAX_RULE_PACKAGES_PER_SET)
        .map(|index| admit(&format!("package-{index:02}"), vec![]))
        .collect::<Vec<_>>();
    assert_eq!(
        resolve_rule_packages(exact_packages.clone())
            .unwrap()
            .packages()
            .len(),
        MAX_RULE_PACKAGES_PER_SET
    );
    let mut packages_over = exact_packages;
    packages_over.push(admit("package-over", vec![]));
    assert!(matches!(
        resolve_rule_packages(packages_over),
        Err(RulePackageSetError::AggregateQuotaExceeded {
            field: "packages",
            actual,
            maximum,
        }) if actual == MAX_RULE_PACKAGES_PER_SET + 1
            && maximum == MAX_RULE_PACKAGES_PER_SET
    ));
}

fn assert_aggregate_source_bound() {
    let exact = (0..(MAX_SOURCES_PER_RULE_PACKAGE_SET / MAX_SOURCES_PER_RULE_PACKAGE))
        .map(|package_index| {
            let sources = (0..MAX_SOURCES_PER_RULE_PACKAGE)
                .map(|source_index| {
                    RuleSource::new(
                        source_id(&format!("source-{source_index:02}")),
                        format!("rules/{source_index:02}.ts"),
                    )
                    .unwrap()
                })
                .collect();
            admit_rule_package(candidate(
                &format!("source-set-{package_index:02}"),
                1,
                vec![],
                sources,
                vec![],
                Value::Null,
            ))
            .unwrap()
        })
        .collect::<Vec<_>>();
    let resolved = resolve_rule_packages(exact.clone()).unwrap();
    assert_eq!(resolved.source_count(), MAX_SOURCES_PER_RULE_PACKAGE_SET);
    drop(resolved);
    let mut one_over = exact;
    one_over.push(
        admit_rule_package(candidate(
            "source-set-over",
            1,
            vec![],
            vec![RuleSource::new(source_id("over"), "over.ts").unwrap()],
            vec![],
            Value::Null,
        ))
        .unwrap(),
    );
    assert!(matches!(
        resolve_rule_packages(one_over),
        Err(RulePackageSetError::AggregateQuotaExceeded {
            field: "sources",
            ..
        })
    ));
}

fn assert_aggregate_provenance_bound() {
    let exact = (0..(MAX_PROVENANCE_PER_RULE_PACKAGE_SET / MAX_PROVENANCE_PER_RULE_PACKAGE))
        .map(|package_index| {
            let provenance = (0..MAX_PROVENANCE_PER_RULE_PACKAGE)
                .map(|subject_index| {
                    RuleProvenance::new(
                        subject_id(&format!("subject-{subject_index:04}")),
                        source_id("source"),
                        Some(1),
                        None,
                    )
                    .unwrap()
                })
                .collect();
            admit_rule_package(candidate(
                &format!("provenance-set-{package_index:02}"),
                1,
                vec![],
                vec![RuleSource::new(source_id("source"), "source.ts").unwrap()],
                provenance,
                Value::Null,
            ))
            .unwrap()
        })
        .collect::<Vec<_>>();
    let resolved = resolve_rule_packages(exact.clone()).unwrap();
    assert_eq!(
        resolved.provenance_count(),
        MAX_PROVENANCE_PER_RULE_PACKAGE_SET
    );
    drop(resolved);
    let mut one_over = exact;
    one_over.push(
        admit_rule_package(candidate(
            "provenance-set-over",
            1,
            vec![],
            vec![RuleSource::new(source_id("source"), "source.ts").unwrap()],
            vec![
                RuleProvenance::new(subject_id("over"), source_id("source"), Some(1), None)
                    .unwrap(),
            ],
            Value::Null,
        ))
        .unwrap(),
    );
    assert!(matches!(
        resolve_rule_packages(one_over),
        Err(RulePackageSetError::AggregateQuotaExceeded {
            field: "provenance",
            ..
        })
    ));
}

fn nested_arrays(depth: usize) -> Value {
    (0..depth).fold(Value::Null, |value, _| Value::Array(vec![value]))
}

fn base_json_nodes(package: &str) -> usize {
    admit(package, vec![]).json_nodes()
}

fn package_with_json_nodes(package: &str, target: usize) -> AdmittedRulePackage {
    let overhead = base_json_nodes(package);
    let package = admit_rule_package(candidate(
        package,
        1,
        vec![],
        vec![],
        vec![],
        Value::Array(vec![Value::Null; target - overhead]),
    ))
    .unwrap();
    assert_eq!(package.json_nodes(), target);
    package
}

fn package_candidate_with_canonical_bytes(
    package: &str,
    target: usize,
) -> Result<RulePackageCandidate, RulePackageError> {
    let full = "x".repeat(MAX_JSON_STRING_BYTES);
    let base = admit_rule_package(candidate(
        package,
        1,
        vec![],
        vec![],
        vec![],
        json!([
            full,
            "x".repeat(MAX_JSON_STRING_BYTES),
            "x".repeat(MAX_JSON_STRING_BYTES),
            ""
        ]),
    ))?;
    let remaining = target
        .checked_sub(base.canonical_bytes().len())
        .ok_or_else(|| RulePackageError::ArithmeticOverflow {
            path: "$/payload/3".to_string(),
        })?;
    assert!(remaining <= MAX_JSON_STRING_BYTES);
    Ok(candidate(
        package,
        1,
        vec![],
        vec![],
        vec![],
        json!([
            "x".repeat(MAX_JSON_STRING_BYTES),
            "x".repeat(MAX_JSON_STRING_BYTES),
            "x".repeat(MAX_JSON_STRING_BYTES),
            "x".repeat(remaining)
        ]),
    ))
}

fn package_with_canonical_bytes(package: &str, target: usize) -> AdmittedRulePackage {
    let package =
        admit_rule_package(package_candidate_with_canonical_bytes(package, target).unwrap())
            .unwrap();
    assert_eq!(package.canonical_bytes().len(), target);
    package
}

fn valid_artifact_with_payload(payload: &[u8]) -> Vec<u8> {
    let mut artifact = br#"{"kind":"rusty.gameplay-rules.package","schemaVersion":1,"domain":"test","package":"decode","version":1,"dependencies":[],"sources":[],"provenance":[],"payload":"#.to_vec();
    artifact.extend_from_slice(payload);
    artifact.extend_from_slice(b"}");
    artifact
}

fn valid_v2_artifact_with_payload(payload: &[u8]) -> Vec<u8> {
    let mut artifact = br#"{"kind":"rusty.gameplay-rules.package","schemaVersion":2,"domain":"test","package":"decode","version":1,"dependencies":[],"sources":[],"provenance":[],"payload":"#.to_vec();
    artifact.extend_from_slice(payload);
    artifact.extend_from_slice(b"}");
    artifact
}

fn valid_artifact_with_metadata(metadata: &[u8]) -> Vec<u8> {
    let mut artifact = br#"{"kind":"rusty.gameplay-rules.package","schemaVersion":1,"domain":"test","package":"decode","version":1,"dependencies":[],"#.to_vec();
    artifact.extend_from_slice(metadata);
    artifact.extend_from_slice(br#","payload":null}"#);
    artifact
}

fn oversized_dependency_artifact() -> String {
    let mut artifact = String::from(
        r#"{"kind":"rusty.gameplay-rules.package","schemaVersion":1,"domain":"test","package":"decode","version":1,"dependencies":["#,
    );
    for index in 0..MAX_DEPENDENCIES_PER_RULE_PACKAGE {
        if index != 0 {
            artifact.push(',');
        }
        artifact.push_str(&format!(
            r#"{{"domain":"test","package":"dependency-{index:02}","version":1}}"#
        ));
    }
    // The quota must reject before attempting to expand or even finish item 33.
    artifact.push_str(r#",{"deeply":"unterminated"#);
    artifact
}
