use std::any::{Any, TypeId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    CommandDescriptor, CommandDescriptorError, CommandId, CommandLane, CommandProfile,
    CorrelationId, DiscoveryEntry, DiscoverySnapshot, ProfileId, ProtocolVersion,
    RuntimeInstanceId, CURRENT_PROTOCOL_VERSION, MAX_DISCOVERED_COMMANDS,
};

pub const MAX_COMMAND_HISTORY: usize = 256;
pub const MAX_TRACKED_CORRELATIONS: usize = 1024;

/// A closed command family. Implementors provide their own request, reply, and
/// error types; this crate never serializes them into a generic string error.
pub trait DeveloperCommand: Send + Sync + 'static {
    type Request: Send + 'static;
    type Reply: Send + 'static;
    type Error: Send + 'static;

    fn descriptor() -> CommandDescriptor;
}

/// The explicit adapter from one command family to one downstream owner.
pub trait CommandHandler<C: DeveloperCommand>: Send + 'static {
    fn handle(
        &mut self,
        context: CommandContext,
        request: C::Request,
    ) -> Result<C::Reply, C::Error>;
}

impl<C, F> CommandHandler<C> for F
where
    C: DeveloperCommand,
    F: FnMut(CommandContext, C::Request) -> Result<C::Reply, C::Error> + Send + 'static,
{
    fn handle(
        &mut self,
        context: CommandContext,
        request: C::Request,
    ) -> Result<C::Reply, C::Error> {
        self(context, request)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DispatchFacts {
    pub runtime: RuntimeInstanceId,
    pub revision: u64,
    pub catalog_epoch: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedFacts {
    pub profile: Option<ProfileId>,
    pub revision: Option<u64>,
    pub catalog_epoch: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRequest<P> {
    pub protocol_version: ProtocolVersion,
    pub command: CommandId,
    pub correlation: CorrelationId,
    pub runtime: RuntimeInstanceId,
    pub expected: ExpectedFacts,
    /// Product-provided pre-dispatch cancellation state. This crate owns no clock.
    pub cancelled: bool,
    /// Product-provided pre-dispatch timeout state. This crate owns no timer.
    pub timed_out: bool,
    pub payload: P,
}

impl<P> CommandRequest<P> {
    pub fn new(
        command: CommandId,
        correlation: CorrelationId,
        runtime: RuntimeInstanceId,
        payload: P,
    ) -> Self {
        Self {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            command,
            correlation,
            runtime,
            expected: ExpectedFacts::default(),
            cancelled: false,
            timed_out: false,
            payload,
        }
    }

    pub fn with_expected(mut self, expected: ExpectedFacts) -> Self {
        self.expected = expected;
        self
    }

    pub fn cancelled(mut self) -> Self {
        self.cancelled = true;
        self
    }

    pub fn timed_out(mut self) -> Self {
        self.timed_out = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandProvenance {
    pub command: CommandId,
    pub lane: CommandLane,
    pub correlation: CorrelationId,
    pub runtime: RuntimeInstanceId,
    pub profile: ProfileId,
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandContext {
    provenance: CommandProvenance,
    facts: DispatchFacts,
}

impl CommandContext {
    pub fn provenance(&self) -> &CommandProvenance {
        &self.provenance
    }
    pub fn facts(&self) -> &DispatchFacts {
        &self.facts
    }
}

/// Output-only typed reply envelope. It serializes for an explicit host
/// adapter, but is not an admitted wire input.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct CommandResponse<R, E> {
    pub protocol_version: ProtocolVersion,
    /// Present only after all pre-dispatch envelope checks have passed.
    pub provenance: Option<CommandProvenance>,
    pub facts: DispatchFacts,
    pub result: HandlerResult<R, E>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum HandlerResult<R, E> {
    Success(R),
    Rejected(DispatchError<E>),
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum DispatchError<E> {
    Envelope(EnvelopeError),
    Command(E),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum EnvelopeError {
    UnsupportedProtocol {
        provided: ProtocolVersion,
        supported: ProtocolVersion,
    },
    UnknownCommand {
        command: CommandId,
    },
    CommandUnavailable {
        command: CommandId,
    },
    CommandMismatch {
        expected: CommandId,
        received: CommandId,
    },
    RuntimeMismatch {
        expected: RuntimeInstanceId,
        received: RuntimeInstanceId,
    },
    StaleRevision {
        expected: u64,
        actual: u64,
    },
    StaleCatalogEpoch {
        expected: u64,
        actual: u64,
    },
    StaleProfile {
        expected: ProfileId,
        actual: ProfileId,
    },
    DuplicateCorrelation {
        correlation: CorrelationId,
        command: CommandId,
    },
    CorrelationCapacityExceeded {
        maximum: usize,
    },
    CorrelationMismatch {
        correlation: CorrelationId,
        previous_command: CommandId,
        requested_command: CommandId,
    },
    Cancelled,
    TimedOut,
    SequenceExhausted,
    BindingInvariant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CommandHistoryOutcome {
    Succeeded,
    CommandRejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandHistoryEntry {
    pub provenance: CommandProvenance,
    pub outcome: CommandHistoryOutcome,
}

pub struct CommandBindings {
    profile: CommandProfile,
    facts: DispatchFacts,
    history_capacity: usize,
    history: VecDeque<CommandHistoryEntry>,
    descriptors: BTreeMap<CommandId, CommandDescriptor>,
    aliases: BTreeMap<crate::CommandAlias, CommandId>,
    bindings: BTreeMap<CommandId, Box<dyn ErasedBinding>>,
    correlations: BTreeMap<CorrelationId, CommandId>,
    sequence: u64,
}

impl fmt::Debug for CommandBindings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandBindings")
            .field("profile", &self.profile)
            .field("facts", &self.facts)
            .field("declared_commands", &self.descriptors.len())
            .field("bound_commands", &self.bindings.len())
            .field("history_entries", &self.history.len())
            .finish()
    }
}

impl CommandBindings {
    pub fn new(
        profile: CommandProfile,
        facts: DispatchFacts,
        history_capacity: usize,
    ) -> Result<Self, CommandBindingsError> {
        if history_capacity == 0 || history_capacity > MAX_COMMAND_HISTORY {
            return Err(CommandBindingsError::InvalidHistoryCapacity {
                maximum: MAX_COMMAND_HISTORY,
                actual: history_capacity,
            });
        }
        Ok(Self {
            profile,
            facts,
            history_capacity,
            history: VecDeque::new(),
            descriptors: BTreeMap::new(),
            aliases: BTreeMap::new(),
            bindings: BTreeMap::new(),
            correlations: BTreeMap::new(),
            sequence: 0,
        })
    }

    pub fn profile(&self) -> &CommandProfile {
        &self.profile
    }
    pub fn facts(&self) -> &DispatchFacts {
        &self.facts
    }
    pub fn history(&self) -> &VecDeque<CommandHistoryEntry> {
        &self.history
    }

    /// The product updates observed facts immediately before its chosen safe point.
    pub fn set_facts(&mut self, facts: DispatchFacts) {
        self.facts = facts;
    }

    pub fn declare<C: DeveloperCommand>(&mut self) -> Result<(), CommandBindingsError> {
        self.declare_descriptor(C::descriptor())
    }

    pub fn declare_descriptor(
        &mut self,
        descriptor: CommandDescriptor,
    ) -> Result<(), CommandBindingsError> {
        if self.descriptors.len() >= MAX_DISCOVERED_COMMANDS {
            return Err(CommandBindingsError::TooManyCommands {
                maximum: MAX_DISCOVERED_COMMANDS,
            });
        }
        let id = descriptor.id().clone();
        if self.descriptors.contains_key(&id) {
            return Err(CommandBindingsError::DuplicateCommand { command: id });
        }
        let mut identities = BTreeSet::new();
        identities.insert(id.as_str());
        for alias in descriptor.aliases() {
            if self.descriptors.contains_key(
                &CommandId::parse(alias.as_str()).map_err(CommandBindingsError::InvalidIdentity)?,
            ) || self.aliases.contains_key(alias)
                || self
                    .aliases
                    .values()
                    .any(|existing| existing.as_str() == alias.as_str())
                || !identities.insert(alias.as_str())
            {
                return Err(CommandBindingsError::DuplicateAlias {
                    alias: alias.clone(),
                });
            }
        }
        if self
            .aliases
            .keys()
            .any(|alias| alias.as_str() == id.as_str())
        {
            return Err(CommandBindingsError::DuplicateCommand { command: id });
        }
        for alias in descriptor.aliases() {
            self.aliases.insert(alias.clone(), id.clone());
        }
        self.descriptors.insert(id, descriptor);
        Ok(())
    }

    pub fn bind<C, H>(&mut self, handler: H) -> Result<(), CommandBindingsError>
    where
        C: DeveloperCommand,
        H: CommandHandler<C>,
    {
        let descriptor = C::descriptor();
        let id = descriptor.id().clone();
        if let Some(existing) = self.descriptors.get(&id) {
            if existing != &descriptor {
                return Err(CommandBindingsError::DescriptorMismatch { command: id });
            }
        } else {
            self.declare_descriptor(descriptor)?;
        }
        if self.bindings.contains_key(&id) {
            return Err(CommandBindingsError::DuplicateBinding { command: id });
        }
        let lane = self.descriptors[&id].lane();
        if !self.profile.permits(lane) {
            return Err(CommandBindingsError::ProfileExcludesCommand { command: id, lane });
        }
        self.bindings.insert(
            id,
            Box::new(TypedBinding::<C, H> {
                handler,
                marker: std::marker::PhantomData,
            }),
        );
        Ok(())
    }

    /// Returns every declared command. `bound` is the exact current exposed
    /// surface; declared-but-unbound entries make omitted privileged handlers
    /// visible rather than being mistaken for unknown commands.
    pub fn discover(&self) -> DiscoverySnapshot {
        let commands = self
            .descriptors
            .iter()
            .map(|(id, descriptor)| DiscoveryEntry {
                descriptor: descriptor.clone(),
                bound: self.bindings.contains_key(id) && self.profile.permits(descriptor.lane()),
            })
            .collect();
        DiscoverySnapshot {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            runtime: self.facts.runtime.clone(),
            profile: self.profile.id().clone(),
            commands,
        }
    }

    /// Resolves a discoverable compatibility alias to the canonical request ID.
    /// Typed in-process requests carry canonical IDs only; an adapter explicitly
    /// normalizes aliases before it invokes [`Self::dispatch`].
    pub fn resolve_alias(&self, alias: &crate::CommandAlias) -> Option<&CommandId> {
        self.aliases.get(alias)
    }

    pub fn dispatch<C: DeveloperCommand>(
        &mut self,
        request: CommandRequest<C::Request>,
    ) -> CommandResponse<C::Reply, C::Error> {
        let expected_descriptor = C::descriptor();
        let expected_id = expected_descriptor.id().clone();
        let facts = self.facts.clone();
        let rejection = |error| CommandResponse {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            provenance: None,
            facts: facts.clone(),
            result: HandlerResult::Rejected(DispatchError::Envelope(error)),
        };

        if request.protocol_version != CURRENT_PROTOCOL_VERSION {
            return rejection(EnvelopeError::UnsupportedProtocol {
                provided: request.protocol_version,
                supported: CURRENT_PROTOCOL_VERSION,
            });
        }
        if request.cancelled {
            return rejection(EnvelopeError::Cancelled);
        }
        if request.timed_out {
            return rejection(EnvelopeError::TimedOut);
        }
        if request.command != expected_id {
            return rejection(EnvelopeError::CommandMismatch {
                expected: expected_id,
                received: request.command.clone(),
            });
        }
        if !self.descriptors.contains_key(&request.command) {
            return rejection(EnvelopeError::UnknownCommand {
                command: request.command.clone(),
            });
        }
        let Some(binding) = self.bindings.get(&request.command) else {
            return rejection(EnvelopeError::CommandUnavailable {
                command: request.command.clone(),
            });
        };
        if binding.command_type() != TypeId::of::<C>() {
            return rejection(EnvelopeError::BindingInvariant);
        }
        if request.runtime != self.facts.runtime {
            return rejection(EnvelopeError::RuntimeMismatch {
                expected: self.facts.runtime.clone(),
                received: request.runtime.clone(),
            });
        }
        if let Some(expected) = &request.expected.profile {
            if expected != self.profile.id() {
                return rejection(EnvelopeError::StaleProfile {
                    expected: expected.clone(),
                    actual: self.profile.id().clone(),
                });
            }
        }
        if let Some(expected) = request.expected.revision {
            if expected != self.facts.revision {
                return rejection(EnvelopeError::StaleRevision {
                    expected,
                    actual: self.facts.revision,
                });
            }
        }
        if let Some(expected) = request.expected.catalog_epoch {
            if expected != self.facts.catalog_epoch {
                return rejection(EnvelopeError::StaleCatalogEpoch {
                    expected,
                    actual: self.facts.catalog_epoch,
                });
            }
        }
        if let Some(previous_command) = self.correlations.get(&request.correlation) {
            return if previous_command == &request.command {
                rejection(EnvelopeError::DuplicateCorrelation {
                    correlation: request.correlation.clone(),
                    command: request.command.clone(),
                })
            } else {
                rejection(EnvelopeError::CorrelationMismatch {
                    correlation: request.correlation.clone(),
                    previous_command: previous_command.clone(),
                    requested_command: request.command.clone(),
                })
            };
        }
        if self.correlations.len() >= MAX_TRACKED_CORRELATIONS {
            return rejection(EnvelopeError::CorrelationCapacityExceeded {
                maximum: MAX_TRACKED_CORRELATIONS,
            });
        }

        // Correlation and history state are committed only after all envelope
        // checks pass, immediately before the owner handler receives the payload.
        let Some(sequence) = self.sequence.checked_add(1) else {
            return rejection(EnvelopeError::SequenceExhausted);
        };
        self.sequence = sequence;
        let provenance = CommandProvenance {
            command: request.command.clone(),
            lane: self.descriptors[&request.command].lane(),
            correlation: request.correlation.clone(),
            runtime: self.facts.runtime.clone(),
            profile: self.profile.id().clone(),
            sequence: self.sequence,
        };
        self.correlations
            .insert(request.correlation.clone(), request.command.clone());
        let context = CommandContext {
            provenance: provenance.clone(),
            facts: facts.clone(),
        };
        let binding = self
            .bindings
            .get_mut(&request.command)
            .expect("checked binding exists");
        let boxed = binding.dispatch(Box::new(request.payload), context);
        let result = match boxed {
            Ok(reply) => match reply.downcast::<C::Reply>() {
                Ok(reply) => HandlerResult::Success(*reply),
                Err(_) => HandlerResult::Rejected(DispatchError::Envelope(
                    EnvelopeError::BindingInvariant,
                )),
            },
            Err(error) => match error.downcast::<C::Error>() {
                Ok(error) => HandlerResult::Rejected(DispatchError::Command(*error)),
                Err(_) => HandlerResult::Rejected(DispatchError::Envelope(
                    EnvelopeError::BindingInvariant,
                )),
            },
        };
        let outcome = match &result {
            HandlerResult::Success(_) => CommandHistoryOutcome::Succeeded,
            HandlerResult::Rejected(DispatchError::Command(_)) => {
                CommandHistoryOutcome::CommandRejected
            }
            HandlerResult::Rejected(DispatchError::Envelope(_)) => {
                CommandHistoryOutcome::CommandRejected
            }
        };
        self.history.push_back(CommandHistoryEntry {
            provenance: provenance.clone(),
            outcome,
        });
        if self.history.len() > self.history_capacity {
            self.history.pop_front();
        }
        CommandResponse {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            provenance: Some(provenance),
            facts,
            result,
        }
    }
}

trait ErasedBinding: Send {
    fn command_type(&self) -> TypeId;
    fn dispatch(
        &mut self,
        payload: Box<dyn Any + Send>,
        context: CommandContext,
    ) -> Result<Box<dyn Any + Send>, Box<dyn Any + Send>>;
}

struct TypedBinding<C, H> {
    handler: H,
    marker: std::marker::PhantomData<C>,
}

impl<C, H> ErasedBinding for TypedBinding<C, H>
where
    C: DeveloperCommand,
    H: CommandHandler<C>,
{
    fn command_type(&self) -> TypeId {
        TypeId::of::<C>()
    }

    fn dispatch(
        &mut self,
        payload: Box<dyn Any + Send>,
        context: CommandContext,
    ) -> Result<Box<dyn Any + Send>, Box<dyn Any + Send>> {
        let request = payload
            .downcast::<C::Request>()
            .map_err(|_| Box::new(()) as Box<dyn Any + Send>)?;
        self.handler
            .handle(context, *request)
            .map(|reply| Box::new(reply) as Box<dyn Any + Send>)
            .map_err(|error| Box::new(error) as Box<dyn Any + Send>)
    }
}

#[derive(Debug)]
pub enum CommandBindingsError {
    InvalidHistoryCapacity {
        maximum: usize,
        actual: usize,
    },
    InvalidDescriptor(CommandDescriptorError),
    InvalidIdentity(crate::CommandIdentityError),
    TooManyCommands {
        maximum: usize,
    },
    DuplicateCommand {
        command: CommandId,
    },
    DuplicateAlias {
        alias: crate::CommandAlias,
    },
    DuplicateBinding {
        command: CommandId,
    },
    DescriptorMismatch {
        command: CommandId,
    },
    ProfileExcludesCommand {
        command: CommandId,
        lane: CommandLane,
    },
}

impl fmt::Display for CommandBindingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid command bindings: {self:?}")
    }
}
impl std::error::Error for CommandBindingsError {}
