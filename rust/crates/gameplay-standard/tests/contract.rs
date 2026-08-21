use gameplay_standard::{
    modules, CapabilityIdentity, CapabilityIdentityError, CapabilityMaturity, CapabilityVersion,
    CapabilityVersionError, MAX_CAPABILITY_IDENTITY_BYTES,
};

#[test]
fn each_capability_has_its_exact_incubating_readout() {
    let readouts = [
        &modules::entity_state::READOUT,
        &modules::mechanics::READOUT,
        &modules::resolution::READOUT,
        &modules::rules::READOUT,
    ];
    let identities = ["entity-state", "mechanics", "resolution", "rules"];

    for (readout, identity) in readouts.into_iter().zip(identities) {
        assert_eq!(readout.identity().as_str(), identity);
        assert_eq!(readout.version().get(), 1);
        assert_eq!(readout.maturity(), CapabilityMaturity::Incubating);
    }
}

#[test]
fn identities_and_versions_validate_their_boundaries() {
    assert_eq!(
        CapabilityIdentity::new(""),
        Err(CapabilityIdentityError::Empty)
    );
    let single_byte = CapabilityIdentity::new("x").expect("single lowercase byte is valid");
    assert_eq!(single_byte.as_str(), "x");
    assert_eq!(
        CapabilityIdentity::new("Entity-state"),
        Err(CapabilityIdentityError::InvalidStart)
    );
    assert_eq!(
        CapabilityIdentity::new("entity-state-"),
        Err(CapabilityIdentityError::InvalidEnd)
    );
    assert_eq!(
        CapabilityIdentity::new("entity_state"),
        Err(CapabilityIdentityError::InvalidCharacter)
    );
    let maximum_length =
        CapabilityIdentity::new("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .expect("the maximum length is valid");
    assert_eq!(maximum_length.as_str().len(), MAX_CAPABILITY_IDENTITY_BYTES);
    assert_eq!(MAX_CAPABILITY_IDENTITY_BYTES, 64);
    assert_eq!(
        CapabilityIdentity::new(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ),
        Err(CapabilityIdentityError::TooLong)
    );
    assert_eq!(CapabilityVersion::new(0), Err(CapabilityVersionError::Zero));
    assert_eq!(
        CapabilityVersion::new(1).expect("positive version").get(),
        1
    );
    assert_eq!(
        CapabilityIdentity::new("").unwrap_err().to_string(),
        "identity is empty"
    );
    assert_eq!(
        CapabilityVersion::new(0).unwrap_err().to_string(),
        "capability version must be positive"
    );
}

#[test]
fn mechanics_and_resolution_are_selected_independently() {
    let selected = [&modules::mechanics::READOUT, &modules::resolution::READOUT];

    assert_eq!(selected[0].identity().as_str(), "mechanics");
    assert_eq!(selected[1].identity().as_str(), "resolution");
    assert_ne!(selected[0].identity(), selected[1].identity());
}

#[test]
fn expression_values_are_an_additive_independent_readout() {
    assert_eq!(
        modules::expression_values::READOUT.identity().as_str(),
        "expression-values"
    );
    assert_eq!(modules::expression_values::READOUT.version().get(), 1);
}

#[test]
fn module_namespaces_reach_exact_owner_apis() {
    assert!(modules::entity_state::EntityLifecycle::Active.is_alive());
    assert_eq!(
        modules::mechanics::StatId::parse("health")
            .expect("exact mechanics API is reachable")
            .as_str(),
        "health"
    );
    assert_eq!(
        modules::resolution::ResolutionId::new(1)
            .expect("exact resolution API is reachable")
            .get(),
        1
    );
    assert_eq!(
        modules::rules::RulePackageId::parse("core")
            .expect("exact rules API is reachable")
            .as_str(),
        "core"
    );
}
