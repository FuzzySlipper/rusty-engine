use gameplay_rules::{
    admit_rule_package, select_rule_payload_subtree, AdmittedRulePackage, RuleDomainId,
    RuleFingerprint, RulePackageCandidate, RulePackageId, RulePayloadPath, RulePayloadPathError,
    RulePayloadPathSegment, RuleProvenance, RuleSource, RuleSourceId, RuleSubjectId,
    RuleSubtreeSelectionError, RuleVersion,
};

fn package() -> AdmittedRulePackage {
    admit_rule_package(RulePackageCandidate::new(
        RuleDomainId::parse("game").unwrap(),
        RulePackageId::parse("aggregate").unwrap(),
        RuleVersion::new(1).unwrap(),
        vec![],
        vec![RuleSource::new(RuleSourceId::parse("rules").unwrap(), "rules.json").unwrap()],
        vec![RuleProvenance::new(
            RuleSubjectId::parse("formula").unwrap(),
            RuleSourceId::parse("rules").unwrap(),
            None,
            None,
        )
        .unwrap()],
        serde_json::json!({"z":{"entries":[{"value":{"b":2,"a":1}}]}}),
    ))
    .unwrap()
}

#[test]
fn selection_binds_structured_array_path_to_the_exact_parent_and_canonical_value() {
    let package = package();
    let path = RulePayloadPath::new(vec![
        RulePayloadPathSegment::field("z").unwrap(),
        RulePayloadPathSegment::field("entries").unwrap(),
        RulePayloadPathSegment::index(0).unwrap(),
        RulePayloadPathSegment::field("value").unwrap(),
    ])
    .unwrap();
    let selection = select_rule_payload_subtree(&package, package.fingerprint(), path).unwrap();
    assert_eq!(selection.path().display(), "payload.z.entries[0].value");
    assert_eq!(selection.parent_identity(), package.identity());
    assert_eq!(selection.parent_fingerprint(), package.fingerprint());
    assert_eq!(selection.canonical_bytes(), br#"{"a":1,"b":2}"#);
}

#[test]
fn selection_rejects_empty_ambiguous_or_wrong_parent_paths_before_consumer_decode() {
    assert!(matches!(
        RulePayloadPath::new(vec![]),
        Err(RulePayloadPathError::Empty)
    ));
    assert!(matches!(
        RulePayloadPathSegment::field("not.a.key"),
        Err(RulePayloadPathError::InvalidField { .. })
    ));
    assert!(matches!(
        RulePayloadPath::new(
            (0..65)
                .map(|index| RulePayloadPathSegment::index(index).unwrap())
                .collect(),
        ),
        Err(RulePayloadPathError::TooManySegments { .. })
    ));

    let package = package();
    let no_entry = RulePayloadPath::new(vec![
        RulePayloadPathSegment::field("z").unwrap(),
        RulePayloadPathSegment::field("entries").unwrap(),
        RulePayloadPathSegment::index(1).unwrap(),
    ])
    .unwrap();
    assert!(matches!(
        select_rule_payload_subtree(&package, package.fingerprint(), no_entry),
        Err(RuleSubtreeSelectionError::IndexOutOfBounds { path, index: 1, length: 1 })
            if path == "payload.z.entries[1]"
    ));
    let wrong_container = RulePayloadPath::new(vec![
        RulePayloadPathSegment::field("z").unwrap(),
        RulePayloadPathSegment::index(0).unwrap(),
    ])
    .unwrap();
    assert!(matches!(
        select_rule_payload_subtree(&package, package.fingerprint(), wrong_container),
        Err(RuleSubtreeSelectionError::ExpectedArray { path }) if path == "payload.z"
    ));
    let wrong_fingerprint =
        RuleFingerprint::parse("0000000000000000000000000000000000000000000000000000000000000000")
            .unwrap();
    assert!(matches!(
        select_rule_payload_subtree(
            &package,
            &wrong_fingerprint,
            RulePayloadPath::new(vec![RulePayloadPathSegment::field("z").unwrap()]).unwrap(),
        ),
        Err(RuleSubtreeSelectionError::ParentFingerprintMismatch { .. })
    ));
    assert!(matches!(
        RulePayloadPathSegment::index(gameplay_rules::MAX_RULE_PAYLOAD_PATH_INDEX + 1),
        Err(RulePayloadPathError::IndexTooLarge { .. })
    ));
    assert!(RulePayloadPathSegment::index(gameplay_rules::MAX_RULE_PAYLOAD_PATH_INDEX).is_ok());
}
