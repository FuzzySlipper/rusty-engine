use std::{convert::Infallible, marker::PhantomData};

use developer_command::{
    CommandBindings, CommandBindingsError, CommandDescriptor, CommandId, CommandLane,
    DeveloperCommand, ParameterDescriptor, TypeDescriptor,
};
use engine_inspector::{EntityInspection, MechanicsStructuralEntityInspection};
use gameplay_mechanics::{
    EffectApplyRequest, EffectMutationReceipt, EffectRemovalRequest, MechanicsError,
    StatBaseMutationReceipt, StatBaseMutationRequest, TrackSetReceipt, TrackSetRequest,
};

/// A closed standard command marker. Product-generic commands retain all of
/// their product-owned request/reply/error types; named-owner commands use the
/// exact existing types below. Descriptor types are bounded discovery/help
/// summaries, not a complete host DTO codec for those Rust owner types.
pub trait StandardCommand: DeveloperCommand {}

macro_rules! exact_command {
    ($name:ident, $request:ty, $reply:ty, $error:ty, $descriptor:ident) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name;

        impl DeveloperCommand for $name {
            type Request = $request;
            type Reply = $reply;
            type Error = $error;

            fn descriptor() -> CommandDescriptor {
                $descriptor()
            }
        }

        impl StandardCommand for $name {}
    };
}

exact_command!(
    InspectEntity,
    core_ids::EntityId,
    Option<EntityInspection>,
    Infallible,
    inspect_entity_descriptor
);
exact_command!(
    InspectMechanics,
    core_ids::EntityId,
    MechanicsStructuralEntityInspection,
    MechanicsError,
    inspect_mechanics_descriptor
);
exact_command!(
    AdminSetStatBase,
    StatBaseMutationRequest,
    StatBaseMutationReceipt,
    MechanicsError,
    admin_set_stat_base_descriptor
);
exact_command!(
    AdminSetTrack,
    TrackSetRequest,
    TrackSetReceipt,
    MechanicsError,
    admin_set_track_descriptor
);
exact_command!(
    AdminApplyEffect,
    EffectApplyRequest,
    EffectMutationReceipt,
    MechanicsError,
    admin_apply_effect_descriptor
);
exact_command!(
    AdminRemoveEffect,
    EffectRemovalRequest,
    EffectMutationReceipt,
    MechanicsError,
    admin_remove_effect_descriptor
);

macro_rules! product_command {
    ($name:ident, $id:literal, $lane:expr, $summary:literal, $request:ty) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name<Request, Reply, Error>(PhantomData<fn() -> (Request, Reply, Error)>);

        impl<Request, Reply, Error> DeveloperCommand for $name<Request, Reply, Error>
        where
            Request: Send + 'static,
            Reply: Send + 'static,
            Error: Send + 'static,
        {
            type Request = $request;
            type Reply = Reply;
            type Error = Error;

            fn descriptor() -> CommandDescriptor {
                product_descriptor($id, $lane, $summary)
            }
        }

        impl<Request, Reply, Error> StandardCommand for $name<Request, Reply, Error>
        where
            Request: Send + 'static,
            Reply: Send + 'static,
            Error: Send + 'static,
        {
        }
    };
}

product_command!(
    InspectStandard,
    "standard.inspect.gameplay",
    CommandLane::Inspect,
    "Present caller-supplied standard projections and a product-typed explanation without reevaluation.",
    Request
);
macro_rules! product_attempt_command {
    ($name:ident, $id:literal, $lane:expr, $summary:literal) => {
        #[derive(Debug, Clone, Copy, Default)]
        pub struct $name<RawIntent, Evidence, Reply, Error>(
            PhantomData<fn() -> (RawIntent, Evidence, Reply, Error)>,
        );

        impl<RawIntent, Evidence, Reply, Error> DeveloperCommand
            for $name<RawIntent, Evidence, Reply, Error>
        where
            RawIntent: Send + 'static,
            Evidence: Send + 'static,
            Reply: Send + 'static,
            Error: Send + 'static,
        {
            type Request = crate::StandardAttempt<RawIntent, Evidence>;
            type Reply = Reply;
            type Error = Error;

            fn descriptor() -> CommandDescriptor {
                product_descriptor($id, $lane, $summary)
            }
        }

        impl<RawIntent, Evidence, Reply, Error> StandardCommand
            for $name<RawIntent, Evidence, Reply, Error>
        where
            RawIntent: Send + 'static,
            Evidence: Send + 'static,
            Reply: Send + 'static,
            Error: Send + 'static,
        {
        }
    };
}

product_attempt_command!(
    PreviewStandardAttempt,
    "standard.preview.attempt",
    CommandLane::Preview,
    "Resolve a product-supplied ordinary attempt in preview mode; the selected transaction aborts."
);
product_attempt_command!(
    PlayStandardAttempt,
    "standard.play.attempt",
    CommandLane::Play,
    "Resolve a product-supplied ordinary attempt in apply mode through the product transaction."
);

fn command(
    id: &str,
    lane: CommandLane,
    summary: &str,
    parameters: Vec<ParameterDescriptor>,
    result: TypeDescriptor,
    error: TypeDescriptor,
) -> CommandDescriptor {
    CommandDescriptor::new(
        CommandId::parse(id).expect("fixed standard command id is valid"),
        Vec::new(),
        lane,
        summary,
        parameters,
        result,
        error,
    )
    .expect("fixed standard command descriptor is valid")
}

fn field(name: &str, summary: &str, required: bool, value: TypeDescriptor) -> ParameterDescriptor {
    ParameterDescriptor::new(name, summary, required, value)
}

fn entity_parameter() -> ParameterDescriptor {
    field(
        "entity",
        "Stable entity identifier.",
        true,
        TypeDescriptor::UnsignedInteger,
    )
}

fn owner_error() -> TypeDescriptor {
    TypeDescriptor::Record {
        fields: vec![
            field(
                "owner",
                "Named owner that rejected the request.",
                true,
                TypeDescriptor::String { maximum_bytes: 96 },
            ),
            field(
                "kind",
                "Stable owner error variant identity.",
                true,
                TypeDescriptor::Identifier { maximum_bytes: 96 },
            ),
        ],
    }
}

fn inspection_result() -> TypeDescriptor {
    TypeDescriptor::Record {
        fields: vec![
            field(
                "entity",
                "Inspected entity identifier.",
                true,
                TypeDescriptor::UnsignedInteger,
            ),
            field(
                "components",
                "Registered component identities.",
                true,
                TypeDescriptor::List {
                    item: Box::new(TypeDescriptor::Identifier { maximum_bytes: 96 }),
                    maximum_items: 32,
                },
            ),
        ],
    }
}

fn inspect_entity_descriptor() -> CommandDescriptor {
    command(
        "standard.inspect.entity",
        CommandLane::Inspect,
        "Read one entity through engine-inspector without changing it.",
        vec![entity_parameter()],
        inspection_result(),
        TypeDescriptor::Unit,
    )
}

fn inspect_mechanics_descriptor() -> CommandDescriptor {
    command(
        "standard.inspect.mechanics",
        CommandLane::Inspect,
        "Read structural mechanics facts through engine-inspector without evaluating or changing them.",
        vec![entity_parameter()],
        TypeDescriptor::Record { fields: vec![
            field("entity", "Inspected entity identifier.", true, TypeDescriptor::UnsignedInteger),
            field("stats", "Stored stat facts.", true, TypeDescriptor::List { item: Box::new(TypeDescriptor::Record { fields: vec![field("id", "Authored stat identity.", true, TypeDescriptor::Identifier { maximum_bytes: 96 })] }), maximum_items: 32 }),
            field("tracks", "Stored track facts.", true, TypeDescriptor::List { item: Box::new(TypeDescriptor::Record { fields: vec![field("id", "Authored track identity.", true, TypeDescriptor::Identifier { maximum_bytes: 96 })] }), maximum_items: 32 }),
        ] },
        owner_error(),
    )
}

fn mutation_parameters(
    include_source: bool,
    fields: Vec<ParameterDescriptor>,
) -> Vec<ParameterDescriptor> {
    let mut parameters = vec![field(
        "operation",
        "Caller-owned mechanics operation identity.",
        true,
        TypeDescriptor::Identifier { maximum_bytes: 96 },
    )];
    if include_source {
        parameters.push(field(
            "source",
            "Caller-owned attributed source identity.",
            true,
            TypeDescriptor::Record {
                fields: vec![field(
                    "kind",
                    "Source identity variant.",
                    true,
                    TypeDescriptor::Identifier { maximum_bytes: 96 },
                )],
            },
        ));
    }
    parameters.extend(fields);
    parameters
}

fn stat_mutation_result() -> TypeDescriptor {
    TypeDescriptor::Record {
        fields: vec![
            field(
                "entity",
                "Mutated entity identifier.",
                true,
                TypeDescriptor::UnsignedInteger,
            ),
            field(
                "stat",
                "Mutated stat identity.",
                true,
                TypeDescriptor::Identifier { maximum_bytes: 96 },
            ),
            field(
                "before",
                "Prior mechanics scalar.",
                true,
                TypeDescriptor::Integer,
            ),
            field(
                "after",
                "Committed mechanics scalar.",
                true,
                TypeDescriptor::Integer,
            ),
            field(
                "committedStatsRevision",
                "Committed owner revision.",
                true,
                TypeDescriptor::UnsignedInteger,
            ),
        ],
    }
}

fn track_mutation_result() -> TypeDescriptor {
    TypeDescriptor::Record {
        fields: vec![
            field(
                "entity",
                "Mutated entity identifier.",
                true,
                TypeDescriptor::UnsignedInteger,
            ),
            field(
                "track",
                "Mutated track identity.",
                true,
                TypeDescriptor::Identifier { maximum_bytes: 96 },
            ),
            field(
                "before",
                "Prior mechanics scalar.",
                true,
                TypeDescriptor::Integer,
            ),
            field(
                "after",
                "Committed mechanics scalar.",
                true,
                TypeDescriptor::Integer,
            ),
            field(
                "committedTracksRevision",
                "Committed owner revision.",
                true,
                TypeDescriptor::UnsignedInteger,
            ),
        ],
    }
}

fn effect_mutation_result() -> TypeDescriptor {
    TypeDescriptor::Record {
        fields: vec![
            field(
                "entity",
                "Mutated entity identifier.",
                true,
                TypeDescriptor::UnsignedInteger,
            ),
            field(
                "kind",
                "Effect mutation kind.",
                true,
                TypeDescriptor::Identifier { maximum_bytes: 96 },
            ),
            field(
                "committedEffectsRevision",
                "Committed owner revision.",
                true,
                TypeDescriptor::UnsignedInteger,
            ),
        ],
    }
}

fn admin_set_stat_base_descriptor() -> CommandDescriptor {
    command(
        "standard.admin.stat.set-base",
        CommandLane::Admin,
        "Set an existing mechanics stat base through StatService.",
        mutation_parameters(
            true,
            vec![
                entity_parameter(),
                field(
                    "stat",
                    "Authored stat identity.",
                    true,
                    TypeDescriptor::Identifier { maximum_bytes: 96 },
                ),
                field(
                    "base",
                    "Requested mechanics scalar base.",
                    true,
                    TypeDescriptor::Integer,
                ),
                field(
                    "expectedRevision",
                    "Optional expected stats revision.",
                    false,
                    TypeDescriptor::UnsignedInteger,
                ),
            ],
        ),
        stat_mutation_result(),
        owner_error(),
    )
}

fn admin_set_track_descriptor() -> CommandDescriptor {
    command(
        "standard.admin.track.set",
        CommandLane::Admin,
        "Set an existing mechanics track through TrackService under an explicit policy.",
        mutation_parameters(
            true,
            vec![
                entity_parameter(),
                field(
                    "track",
                    "Authored track identity.",
                    true,
                    TypeDescriptor::Identifier { maximum_bytes: 96 },
                ),
                field(
                    "value",
                    "Requested mechanics scalar current value.",
                    true,
                    TypeDescriptor::Integer,
                ),
                field(
                    "policy",
                    "reject-out-of-bounds or clamp-to-bounds.",
                    true,
                    TypeDescriptor::Identifier { maximum_bytes: 96 },
                ),
                field(
                    "expectedRevision",
                    "Optional expected tracks revision.",
                    false,
                    TypeDescriptor::UnsignedInteger,
                ),
            ],
        ),
        track_mutation_result(),
        owner_error(),
    )
}

fn admin_apply_effect_descriptor() -> CommandDescriptor {
    command(
        "standard.admin.effect.apply",
        CommandLane::Admin,
        "Apply an existing mechanics effect through EffectService.",
        mutation_parameters(
            false,
            vec![
                entity_parameter(),
                field(
                    "instance",
                    "Live effect instance identity.",
                    true,
                    TypeDescriptor::Identifier { maximum_bytes: 96 },
                ),
                field(
                    "definition",
                    "Authored effect definition identity.",
                    true,
                    TypeDescriptor::Identifier { maximum_bytes: 96 },
                ),
                field(
                    "provenance",
                    "Attributed source identity for the effect application.",
                    true,
                    TypeDescriptor::Record {
                        fields: vec![field(
                            "kind",
                            "Source identity variant.",
                            true,
                            TypeDescriptor::Identifier { maximum_bytes: 96 },
                        )],
                    },
                ),
                field(
                    "stacks",
                    "Requested effect stack count.",
                    true,
                    TypeDescriptor::UnsignedInteger,
                ),
                field(
                    "expectedRevision",
                    "Optional expected effects revision.",
                    false,
                    TypeDescriptor::UnsignedInteger,
                ),
            ],
        ),
        effect_mutation_result(),
        owner_error(),
    )
}

fn admin_remove_effect_descriptor() -> CommandDescriptor {
    command(
        "standard.admin.effect.remove",
        CommandLane::Admin,
        "Remove an existing mechanics effect through EffectService.",
        mutation_parameters(
            false,
            vec![
                entity_parameter(),
                field(
                    "instance",
                    "Live effect instance identity.",
                    true,
                    TypeDescriptor::Identifier { maximum_bytes: 96 },
                ),
                field(
                    "expectedRevision",
                    "Optional expected effects revision.",
                    false,
                    TypeDescriptor::UnsignedInteger,
                ),
            ],
        ),
        effect_mutation_result(),
        owner_error(),
    )
}

fn product_descriptor(id: &str, lane: CommandLane, summary: &str) -> CommandDescriptor {
    command(
        id,
        lane,
        summary,
        vec![field(
            "productPayload",
            "Product-owned typed payload; the binding supplies its schema.",
            true,
            TypeDescriptor::Record {
                fields: vec![field(
                    "schema",
                    "Product-owned schema identity.",
                    true,
                    TypeDescriptor::Identifier { maximum_bytes: 96 },
                )],
            },
        )],
        TypeDescriptor::Record {
            fields: vec![field(
                "productResult",
                "Product-owned typed result; the binding supplies its schema.",
                true,
                TypeDescriptor::Record {
                    fields: vec![field(
                        "schema",
                        "Product-owned schema identity.",
                        true,
                        TypeDescriptor::Identifier { maximum_bytes: 96 },
                    )],
                },
            )],
        },
        TypeDescriptor::Record {
            fields: vec![field(
                "productError",
                "Product-owned typed error; the binding supplies its schema.",
                true,
                TypeDescriptor::Record {
                    fields: vec![field(
                        "schema",
                        "Product-owned schema identity.",
                        true,
                        TypeDescriptor::Identifier { maximum_bytes: 96 },
                    )],
                },
            )],
        },
    )
}

/// Declares the complete standard module family. Products may deliberately
/// leave a descriptor unbound; discovery will then report it unavailable.
pub fn declare_standard_commands(
    bindings: &mut CommandBindings,
) -> Result<(), CommandBindingsError> {
    for descriptor in [
        inspect_entity_descriptor(), inspect_mechanics_descriptor(),
        product_descriptor("standard.inspect.gameplay", CommandLane::Inspect, "Present caller-supplied standard projections and a product-typed explanation without reevaluation."),
        product_descriptor("standard.preview.attempt", CommandLane::Preview, "Resolve a product-supplied ordinary attempt in preview mode; the selected transaction aborts."),
        product_descriptor("standard.play.attempt", CommandLane::Play, "Resolve a product-supplied ordinary attempt in apply mode through the product transaction."),
        admin_set_stat_base_descriptor(), admin_set_track_descriptor(),
        admin_apply_effect_descriptor(), admin_remove_effect_descriptor(),
    ] {
        bindings.declare_descriptor(descriptor)?;
    }
    Ok(())
}

/// Retrieves a standard descriptor by stable identity for client generation.
pub fn descriptor_for(id: &str) -> Option<CommandDescriptor> {
    match id {
        "standard.inspect.entity" => Some(inspect_entity_descriptor()),
        "standard.inspect.mechanics" => Some(inspect_mechanics_descriptor()),
        "standard.inspect.gameplay" => Some(product_descriptor(id, CommandLane::Inspect, "Present caller-supplied standard projections and a product-typed explanation without reevaluation.")),
        "standard.preview.attempt" => Some(product_descriptor(id, CommandLane::Preview, "Resolve a product-supplied ordinary attempt in preview mode; the selected transaction aborts.")),
        "standard.play.attempt" => Some(product_descriptor(id, CommandLane::Play, "Resolve a product-supplied ordinary attempt in apply mode through the product transaction.")),
        "standard.admin.stat.set-base" => Some(admin_set_stat_base_descriptor()),
        "standard.admin.track.set" => Some(admin_set_track_descriptor()),
        "standard.admin.effect.apply" => Some(admin_apply_effect_descriptor()),
        "standard.admin.effect.remove" => Some(admin_remove_effect_descriptor()),
        _ => None,
    }
}
