use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{CommandAlias, CommandId, ProfileId};

pub const MAX_COMMAND_ALIASES: usize = 8;
pub const MAX_PARAMETERS_PER_COMMAND: usize = 32;
pub const MAX_DESCRIPTOR_STRING_BYTES: usize = 256;
pub const MAX_DESCRIPTOR_COLLECTION_ITEMS: usize = 32;
pub const MAX_DESCRIPTOR_DEPTH: usize = 64;
pub const MAX_DESCRIPTOR_NODES: usize = 128;
pub const MAX_DISCOVERED_COMMANDS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CommandLane {
    Inspect,
    Preview,
    Play,
    Admin,
    Session,
    Author,
    Fault,
}

impl CommandLane {
    pub const ALL: [Self; 7] = [
        Self::Inspect,
        Self::Preview,
        Self::Play,
        Self::Admin,
        Self::Session,
        Self::Author,
        Self::Fault,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandProfile {
    id: ProfileId,
    permitted_lanes: BTreeSet<CommandLane>,
}

impl CommandProfile {
    pub fn new(
        id: ProfileId,
        permitted_lanes: impl IntoIterator<Item = CommandLane>,
    ) -> Result<Self, CommandDescriptorError> {
        let permitted_lanes = permitted_lanes.into_iter().collect::<BTreeSet<_>>();
        if permitted_lanes.is_empty() {
            return Err(CommandDescriptorError::EmptyProfile);
        }
        Ok(Self {
            id,
            permitted_lanes,
        })
    }

    pub fn broad(id: ProfileId) -> Self {
        Self {
            id,
            permitted_lanes: CommandLane::ALL.into_iter().collect(),
        }
    }

    pub fn id(&self) -> &ProfileId {
        &self.id
    }

    pub fn permits(&self, lane: CommandLane) -> bool {
        self.permitted_lanes.contains(&lane)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandDescriptor {
    id: CommandId,
    aliases: Vec<CommandAlias>,
    lane: CommandLane,
    summary: String,
    parameters: Vec<ParameterDescriptor>,
    result: TypeDescriptor,
    error: TypeDescriptor,
}

impl CommandDescriptor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: CommandId,
        aliases: Vec<CommandAlias>,
        lane: CommandLane,
        summary: impl Into<String>,
        parameters: Vec<ParameterDescriptor>,
        result: TypeDescriptor,
        error: TypeDescriptor,
    ) -> Result<Self, CommandDescriptorError> {
        let summary = summary.into();
        validate_text(&summary)
            .map_err(|(maximum, actual)| CommandDescriptorError::InvalidText { maximum, actual })?;
        if aliases.len() > MAX_COMMAND_ALIASES {
            return Err(CommandDescriptorError::TooManyAliases {
                maximum: MAX_COMMAND_ALIASES,
                actual: aliases.len(),
            });
        }
        if parameters.len() > MAX_PARAMETERS_PER_COMMAND {
            return Err(CommandDescriptorError::TooManyParameters {
                maximum: MAX_PARAMETERS_PER_COMMAND,
                actual: parameters.len(),
            });
        }
        let mut identities = BTreeSet::new();
        identities.insert(id.as_str());
        if aliases
            .iter()
            .any(|alias| !identities.insert(alias.as_str()))
        {
            return Err(CommandDescriptorError::DuplicateAlias);
        }
        let mut names = BTreeSet::new();
        if parameters
            .iter()
            .any(|parameter| !names.insert(parameter.name.as_str()))
        {
            return Err(CommandDescriptorError::DuplicateParameter);
        }
        let mut nodes = result
            .node_count()
            .map_err(CommandDescriptorError::InvalidType)?;
        nodes = nodes
            .checked_add(
                error
                    .node_count()
                    .map_err(CommandDescriptorError::InvalidType)?,
            )
            .ok_or(CommandDescriptorError::TooManyNodes {
                maximum: MAX_DESCRIPTOR_NODES,
                actual: usize::MAX,
            })?;
        for parameter in &parameters {
            validate_text(&parameter.name).map_err(|(maximum, actual)| {
                CommandDescriptorError::InvalidText { maximum, actual }
            })?;
            validate_text(&parameter.summary).map_err(|(maximum, actual)| {
                CommandDescriptorError::InvalidText { maximum, actual }
            })?;
            nodes = nodes
                .checked_add(
                    parameter
                        .value
                        .node_count()
                        .map_err(CommandDescriptorError::InvalidType)?,
                )
                .ok_or(CommandDescriptorError::TooManyNodes {
                    maximum: MAX_DESCRIPTOR_NODES,
                    actual: usize::MAX,
                })?;
        }
        if nodes > MAX_DESCRIPTOR_NODES {
            return Err(CommandDescriptorError::TooManyNodes {
                maximum: MAX_DESCRIPTOR_NODES,
                actual: nodes,
            });
        }
        Ok(Self {
            id,
            aliases,
            lane,
            summary,
            parameters,
            result,
            error,
        })
    }

    pub fn id(&self) -> &CommandId {
        &self.id
    }
    pub fn aliases(&self) -> &[CommandAlias] {
        &self.aliases
    }
    pub const fn lane(&self) -> CommandLane {
        self.lane
    }
    pub fn summary(&self) -> &str {
        &self.summary
    }
    pub fn parameters(&self) -> &[ParameterDescriptor] {
        &self.parameters
    }
    pub fn result(&self) -> &TypeDescriptor {
        &self.result
    }
    pub fn error(&self) -> &TypeDescriptor {
        &self.error
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParameterDescriptor {
    pub name: String,
    pub summary: String,
    pub required: bool,
    pub value: TypeDescriptor,
}

impl ParameterDescriptor {
    pub fn new(
        name: impl Into<String>,
        summary: impl Into<String>,
        required: bool,
        value: TypeDescriptor,
    ) -> Self {
        Self {
            name: name.into(),
            summary: summary.into(),
            required,
            value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum TypeDescriptor {
    Unit,
    Boolean,
    Integer,
    UnsignedInteger,
    String {
        maximum_bytes: usize,
    },
    Identifier {
        maximum_bytes: usize,
    },
    List {
        item: Box<TypeDescriptor>,
        maximum_items: usize,
    },
    Record {
        fields: Vec<ParameterDescriptor>,
    },
}

impl TypeDescriptor {
    pub fn node_count(&self) -> Result<usize, TypeDescriptorError> {
        let mut nodes = 0usize;
        let mut pending = vec![(self, 1usize)];
        while let Some((descriptor, depth)) = pending.pop() {
            if depth > MAX_DESCRIPTOR_DEPTH {
                return Err(TypeDescriptorError::TooDeep {
                    maximum: MAX_DESCRIPTOR_DEPTH,
                    actual: depth,
                });
            }
            nodes = nodes
                .checked_add(1)
                .ok_or(TypeDescriptorError::NodeOverflow)?;
            if nodes > MAX_DESCRIPTOR_NODES {
                return Err(TypeDescriptorError::TooManyNodes {
                    maximum: MAX_DESCRIPTOR_NODES,
                    actual: nodes,
                });
            }
            match descriptor {
                Self::Unit | Self::Boolean | Self::Integer | Self::UnsignedInteger => {}
                Self::String { maximum_bytes } | Self::Identifier { maximum_bytes } => {
                    if *maximum_bytes == 0 || *maximum_bytes > MAX_DESCRIPTOR_STRING_BYTES {
                        return Err(TypeDescriptorError::InvalidStringLimit {
                            maximum: MAX_DESCRIPTOR_STRING_BYTES,
                            actual: *maximum_bytes,
                        });
                    }
                }
                Self::List {
                    item,
                    maximum_items,
                } => {
                    if *maximum_items == 0 || *maximum_items > MAX_DESCRIPTOR_COLLECTION_ITEMS {
                        return Err(TypeDescriptorError::InvalidCollectionLimit {
                            maximum: MAX_DESCRIPTOR_COLLECTION_ITEMS,
                            actual: *maximum_items,
                        });
                    }
                    pending.push((item, depth + 1));
                }
                Self::Record { fields } => {
                    if fields.len() > MAX_DESCRIPTOR_COLLECTION_ITEMS {
                        return Err(TypeDescriptorError::InvalidCollectionLimit {
                            maximum: MAX_DESCRIPTOR_COLLECTION_ITEMS,
                            actual: fields.len(),
                        });
                    }
                    let mut names = BTreeSet::new();
                    for field in fields {
                        validate_text(&field.name).map_err(|(maximum, actual)| {
                            TypeDescriptorError::InvalidText { maximum, actual }
                        })?;
                        validate_text(&field.summary).map_err(|(maximum, actual)| {
                            TypeDescriptorError::InvalidText { maximum, actual }
                        })?;
                        if !names.insert(field.name.as_str()) {
                            return Err(TypeDescriptorError::DuplicateRecordField);
                        }
                        pending.push((&field.value, depth + 1));
                    }
                }
            }
        }
        Ok(nodes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscoveryEntry {
    pub descriptor: CommandDescriptor,
    pub bound: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscoverySnapshot {
    pub protocol_version: crate::ProtocolVersion,
    pub runtime: crate::RuntimeInstanceId,
    pub profile: ProfileId,
    pub commands: Vec<DiscoveryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandDescriptorError {
    EmptyProfile,
    TooManyAliases { maximum: usize, actual: usize },
    TooManyParameters { maximum: usize, actual: usize },
    TooManyNodes { maximum: usize, actual: usize },
    DuplicateAlias,
    DuplicateParameter,
    InvalidText { maximum: usize, actual: usize },
    InvalidType(TypeDescriptorError),
}

impl fmt::Display for CommandDescriptorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid command descriptor: {self:?}")
    }
}
impl std::error::Error for CommandDescriptorError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDescriptorError {
    InvalidStringLimit { maximum: usize, actual: usize },
    InvalidCollectionLimit { maximum: usize, actual: usize },
    TooManyNodes { maximum: usize, actual: usize },
    TooDeep { maximum: usize, actual: usize },
    DuplicateRecordField,
    InvalidRecordField,
    NodeOverflow,
    InvalidText { maximum: usize, actual: usize },
}
impl fmt::Display for TypeDescriptorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid type descriptor: {self:?}")
    }
}
impl std::error::Error for TypeDescriptorError {}

fn validate_text(value: &str) -> Result<(), (usize, usize)> {
    if value.is_empty() || value.len() > MAX_DESCRIPTOR_STRING_BYTES {
        return Err((MAX_DESCRIPTOR_STRING_BYTES, value.len()));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCommandProfile {
    id: ProfileId,
    permitted_lanes: BTreeSet<CommandLane>,
}

impl<'de> Deserialize<'de> for CommandProfile {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = WireCommandProfile::deserialize(deserializer)?;
        Self::new(wire.id, wire.permitted_lanes)
            .map_err(|error| <D::Error as serde::de::Error>::custom(error.to_string()))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCommandDescriptor {
    id: CommandId,
    aliases: Vec<CommandAlias>,
    lane: CommandLane,
    summary: String,
    parameters: Vec<WireParameterDescriptor>,
    result: WireTypeDescriptor,
    error: WireTypeDescriptor,
}

impl<'de> Deserialize<'de> for CommandDescriptor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = WireCommandDescriptor::deserialize(deserializer)?;
        let parameters = wire
            .parameters
            .into_iter()
            .map(ParameterDescriptor::try_from)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| <D::Error as serde::de::Error>::custom(error.to_string()))?;
        Self::new(
            wire.id,
            wire.aliases,
            wire.lane,
            wire.summary,
            parameters,
            TypeDescriptor::try_from(wire.result)
                .map_err(|error| <D::Error as serde::de::Error>::custom(error.to_string()))?,
            TypeDescriptor::try_from(wire.error)
                .map_err(|error| <D::Error as serde::de::Error>::custom(error.to_string()))?,
        )
        .map_err(|error| <D::Error as serde::de::Error>::custom(error.to_string()))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireParameterDescriptor {
    name: String,
    summary: String,
    required: bool,
    value: WireTypeDescriptor,
}

impl TryFrom<WireParameterDescriptor> for ParameterDescriptor {
    type Error = CommandDescriptorError;

    fn try_from(wire: WireParameterDescriptor) -> Result<Self, Self::Error> {
        let value =
            TypeDescriptor::try_from(wire.value).map_err(CommandDescriptorError::InvalidType)?;
        validate_text(&wire.name)
            .map_err(|(maximum, actual)| CommandDescriptorError::InvalidText { maximum, actual })?;
        validate_text(&wire.summary)
            .map_err(|(maximum, actual)| CommandDescriptorError::InvalidText { maximum, actual })?;
        Ok(Self {
            name: wire.name,
            summary: wire.summary,
            required: wire.required,
            value,
        })
    }
}

impl<'de> Deserialize<'de> for ParameterDescriptor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        WireParameterDescriptor::deserialize(deserializer)?
            .try_into()
            .map_err(|error: CommandDescriptorError| {
                <D::Error as serde::de::Error>::custom(error.to_string())
            })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
enum WireTypeDescriptor {
    Unit,
    Boolean,
    Integer,
    UnsignedInteger,
    String {
        maximum_bytes: usize,
    },
    Identifier {
        maximum_bytes: usize,
    },
    List {
        item: Box<WireTypeDescriptor>,
        maximum_items: usize,
    },
    Record {
        fields: Vec<WireParameterDescriptor>,
    },
}

impl TryFrom<WireTypeDescriptor> for TypeDescriptor {
    type Error = TypeDescriptorError;

    fn try_from(wire: WireTypeDescriptor) -> Result<Self, Self::Error> {
        let descriptor = match wire {
            WireTypeDescriptor::Unit => Self::Unit,
            WireTypeDescriptor::Boolean => Self::Boolean,
            WireTypeDescriptor::Integer => Self::Integer,
            WireTypeDescriptor::UnsignedInteger => Self::UnsignedInteger,
            WireTypeDescriptor::String { maximum_bytes } => Self::String { maximum_bytes },
            WireTypeDescriptor::Identifier { maximum_bytes } => Self::Identifier { maximum_bytes },
            WireTypeDescriptor::List {
                item,
                maximum_items,
            } => Self::List {
                item: Box::new(Self::try_from(*item)?),
                maximum_items,
            },
            WireTypeDescriptor::Record { fields } => Self::Record {
                fields: fields
                    .into_iter()
                    .map(ParameterDescriptor::try_from)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| match error {
                        CommandDescriptorError::InvalidType(error) => error,
                        CommandDescriptorError::InvalidText { maximum, actual } => {
                            TypeDescriptorError::InvalidText { maximum, actual }
                        }
                        _ => TypeDescriptorError::InvalidRecordField,
                    })?,
            },
        };
        let nodes = descriptor.node_count()?;
        if nodes > MAX_DESCRIPTOR_NODES {
            return Err(TypeDescriptorError::TooManyNodes {
                maximum: MAX_DESCRIPTOR_NODES,
                actual: nodes,
            });
        }
        Ok(descriptor)
    }
}

impl<'de> Deserialize<'de> for TypeDescriptor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        WireTypeDescriptor::deserialize(deserializer)?
            .try_into()
            .map_err(|error: TypeDescriptorError| {
                <D::Error as serde::de::Error>::custom(error.to_string())
            })
    }
}
