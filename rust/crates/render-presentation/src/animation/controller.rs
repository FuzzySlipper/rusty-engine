use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{
    AnimationCondition, AnimationMotionDefinition, AnimationParameterKind, AnimationParameterValue,
    ValidatedAnimationCatalog, ValidatedGraph, BLEND_WEIGHT_SCALE,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedAnimationMotion {
    pub clip_a: String,
    pub clip_b: Option<String>,
    pub blend_weight_milli: i32,
    pub speed_milli: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationTransitionState {
    pub transition_id: String,
    pub from_state_id: String,
    pub to_state_id: String,
    pub elapsed_ticks: u32,
    pub duration_ticks: u32,
    pub target_motion: ResolvedAnimationMotion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnimationTransitionFactMoment {
    Started,
    Completed,
}

/// Typed observation of a controller transition. It carries local animation
/// meaning only; consumers decide whether and how that maps to gameplay facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationTransitionFact {
    pub controller_tick: u64,
    pub transition_id: String,
    pub from_state_id: String,
    pub to_state_id: String,
    pub moment: AnimationTransitionFactMoment,
    pub duration_ticks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationControllerState {
    pub entity: u64,
    pub graph_id: String,
    pub graph_version: u32,
    pub asset_id: String,
    pub current_state_id: String,
    pub revision: u64,
    pub controller_tick: u64,
    pub parameters: BTreeMap<String, AnimationParameterValue>,
    pub motion: ResolvedAnimationMotion,
    pub transition: Option<AnimationTransitionState>,
    pub transition_fact: Option<AnimationTransitionFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationControllerChange {
    pub previous_revision: Option<u64>,
    /// `None` means the controller was detached.
    pub state: Option<AnimationControllerState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AnimationControllerInput {
    Attach {
        graph_id: String,
    },
    SetFloat {
        parameter_id: String,
        value_milli: i32,
    },
    SetBool {
        parameter_id: String,
        value: bool,
    },
    FireTrigger {
        parameter_id: String,
    },
    Tick {
        tick: u64,
    },
    Detach,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimationControllerError {
    UnknownGraph(String),
    ControllerAlreadyAttached(u64),
    ControllerMissing(u64),
    UnknownParameter(String),
    ParameterTypeMismatch(String),
    TickNotContiguous { expected: u64, actual: u64 },
    CorruptGraph(String),
}

impl core::fmt::Display for AnimationControllerError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AnimationControllerError {}

#[derive(Debug, Clone)]
struct ControllerInstance {
    graph_id: String,
    current_state_id: String,
    parameters: BTreeMap<String, AnimationParameterValue>,
    transition: Option<ActiveTransition>,
    last_transition_fact: Option<AnimationTransitionFact>,
    last_tick: u64,
    revision: u64,
}

#[derive(Debug, Clone)]
struct ActiveTransition {
    transition_id: String,
    from_state_id: String,
    to_state_id: String,
    elapsed_ticks: u32,
    duration_ticks: u32,
}

/// Explicit deterministic animation-controller mechanism. It is called by a
/// downstream service; it does not subscribe to entities or run an update loop.
#[derive(Debug, Clone)]
pub struct AnimationControllerService {
    catalog: ValidatedAnimationCatalog,
    controllers: BTreeMap<u64, ControllerInstance>,
}

impl AnimationControllerService {
    pub fn new(catalog: ValidatedAnimationCatalog) -> Self {
        Self {
            catalog,
            controllers: BTreeMap::new(),
        }
    }

    pub fn catalog(&self) -> &ValidatedAnimationCatalog {
        &self.catalog
    }

    pub fn state(&self, entity: u64) -> Result<AnimationControllerState, AnimationControllerError> {
        let controller = self
            .controllers
            .get(&entity)
            .ok_or(AnimationControllerError::ControllerMissing(entity))?;
        let graph = self
            .catalog
            .graphs
            .get(&controller.graph_id)
            .ok_or_else(|| AnimationControllerError::CorruptGraph(controller.graph_id.clone()))?;
        resolved_state(entity, graph, controller)
    }

    pub fn apply(
        &mut self,
        entity: u64,
        input: AnimationControllerInput,
    ) -> Result<AnimationControllerChange, AnimationControllerError> {
        let mut changes = self.apply_batch(vec![(entity, input)])?;
        Ok(changes.pop().expect("one input produces one change"))
    }

    /// Applies an input cluster atomically, which is useful when a downstream
    /// service changes several parameters before advancing the controller.
    pub fn apply_batch(
        &mut self,
        inputs: Vec<(u64, AnimationControllerInput)>,
    ) -> Result<Vec<AnimationControllerChange>, AnimationControllerError> {
        let mut staged = self.clone();
        let mut changes = Vec::with_capacity(inputs.len());
        for (entity, input) in inputs {
            changes.push(staged.apply_inner(entity, input)?);
        }
        *self = staged;
        Ok(changes)
    }

    pub fn attach(
        &mut self,
        entity: u64,
        graph_id: impl Into<String>,
    ) -> Result<AnimationControllerChange, AnimationControllerError> {
        self.apply(
            entity,
            AnimationControllerInput::Attach {
                graph_id: graph_id.into(),
            },
        )
    }

    pub fn set_float(
        &mut self,
        entity: u64,
        parameter_id: impl Into<String>,
        value_milli: i32,
    ) -> Result<AnimationControllerChange, AnimationControllerError> {
        self.apply(
            entity,
            AnimationControllerInput::SetFloat {
                parameter_id: parameter_id.into(),
                value_milli,
            },
        )
    }

    pub fn set_bool(
        &mut self,
        entity: u64,
        parameter_id: impl Into<String>,
        value: bool,
    ) -> Result<AnimationControllerChange, AnimationControllerError> {
        self.apply(
            entity,
            AnimationControllerInput::SetBool {
                parameter_id: parameter_id.into(),
                value,
            },
        )
    }

    pub fn fire_trigger(
        &mut self,
        entity: u64,
        parameter_id: impl Into<String>,
    ) -> Result<AnimationControllerChange, AnimationControllerError> {
        self.apply(
            entity,
            AnimationControllerInput::FireTrigger {
                parameter_id: parameter_id.into(),
            },
        )
    }

    pub fn tick(
        &mut self,
        entity: u64,
        tick: u64,
    ) -> Result<AnimationControllerChange, AnimationControllerError> {
        self.apply(entity, AnimationControllerInput::Tick { tick })
    }

    pub fn detach(
        &mut self,
        entity: u64,
    ) -> Result<AnimationControllerChange, AnimationControllerError> {
        self.apply(entity, AnimationControllerInput::Detach)
    }

    pub fn reset(&mut self) {
        self.controllers.clear();
    }

    fn apply_inner(
        &mut self,
        entity: u64,
        input: AnimationControllerInput,
    ) -> Result<AnimationControllerChange, AnimationControllerError> {
        let previous_revision = self
            .controllers
            .get(&entity)
            .map(|controller| controller.revision);
        match input {
            AnimationControllerInput::Attach { graph_id } => {
                if self.controllers.contains_key(&entity) {
                    return Err(AnimationControllerError::ControllerAlreadyAttached(entity));
                }
                let graph = self
                    .catalog
                    .graphs
                    .get(&graph_id)
                    .ok_or_else(|| AnimationControllerError::UnknownGraph(graph_id.clone()))?;
                let parameters = graph
                    .parameters
                    .iter()
                    .map(|(id, definition)| (id.clone(), definition.default_value.clone()))
                    .collect();
                self.controllers.insert(
                    entity,
                    ControllerInstance {
                        graph_id,
                        current_state_id: graph.definition.initial_state_id.clone(),
                        parameters,
                        transition: None,
                        last_transition_fact: None,
                        last_tick: 0,
                        revision: 0,
                    },
                );
            }
            AnimationControllerInput::SetFloat {
                parameter_id,
                value_milli,
            } => self.set_parameter(
                entity,
                &parameter_id,
                AnimationParameterKind::Float,
                AnimationParameterValue::Float(value_milli),
            )?,
            AnimationControllerInput::SetBool {
                parameter_id,
                value,
            } => self.set_parameter(
                entity,
                &parameter_id,
                AnimationParameterKind::Bool,
                AnimationParameterValue::Bool(value),
            )?,
            AnimationControllerInput::FireTrigger { parameter_id } => self.set_parameter(
                entity,
                &parameter_id,
                AnimationParameterKind::Trigger,
                AnimationParameterValue::Trigger(true),
            )?,
            AnimationControllerInput::Tick { tick } => {
                let controller = self
                    .controllers
                    .get_mut(&entity)
                    .ok_or(AnimationControllerError::ControllerMissing(entity))?;
                let expected = controller.last_tick.saturating_add(1);
                if tick != expected {
                    return Err(AnimationControllerError::TickNotContiguous {
                        expected,
                        actual: tick,
                    });
                }
                let graph = self
                    .catalog
                    .graphs
                    .get(&controller.graph_id)
                    .ok_or_else(|| {
                        AnimationControllerError::CorruptGraph(controller.graph_id.clone())
                    })?;
                controller.last_transition_fact = evaluate_tick(graph, controller, tick)?;
                controller.last_tick = tick;
                controller.revision = controller.revision.saturating_add(1);
            }
            AnimationControllerInput::Detach => {
                if self.controllers.remove(&entity).is_none() {
                    return Err(AnimationControllerError::ControllerMissing(entity));
                }
                return Ok(AnimationControllerChange {
                    previous_revision,
                    state: None,
                });
            }
        }
        Ok(AnimationControllerChange {
            previous_revision,
            state: Some(self.state(entity)?),
        })
    }

    fn set_parameter(
        &mut self,
        entity: u64,
        parameter_id: &str,
        expected_kind: AnimationParameterKind,
        value: AnimationParameterValue,
    ) -> Result<(), AnimationControllerError> {
        let controller = self
            .controllers
            .get_mut(&entity)
            .ok_or(AnimationControllerError::ControllerMissing(entity))?;
        let graph = self
            .catalog
            .graphs
            .get(&controller.graph_id)
            .ok_or_else(|| AnimationControllerError::CorruptGraph(controller.graph_id.clone()))?;
        let definition = graph
            .parameters
            .get(parameter_id)
            .ok_or_else(|| AnimationControllerError::UnknownParameter(parameter_id.to_string()))?;
        if definition.kind != expected_kind {
            return Err(AnimationControllerError::ParameterTypeMismatch(
                parameter_id.to_string(),
            ));
        }
        controller
            .parameters
            .insert(parameter_id.to_string(), value);
        controller.last_transition_fact = None;
        controller.revision = controller.revision.saturating_add(1);
        Ok(())
    }
}

fn evaluate_tick(
    graph: &ValidatedGraph,
    controller: &mut ControllerInstance,
    tick: u64,
) -> Result<Option<AnimationTransitionFact>, AnimationControllerError> {
    if let Some(active) = controller.transition.as_mut() {
        active.elapsed_ticks = active.elapsed_ticks.saturating_add(1);
        if active.elapsed_ticks >= active.duration_ticks {
            let completed = controller
                .transition
                .take()
                .expect("active transition exists");
            controller
                .current_state_id
                .clone_from(&completed.to_state_id);
            return Ok(Some(transition_fact(
                tick,
                &completed,
                AnimationTransitionFactMoment::Completed,
            )));
        }
        return Ok(None);
    }

    let selected = graph
        .transitions
        .get(&controller.current_state_id)
        .and_then(|candidates| {
            candidates
                .iter()
                .find(|candidate| conditions_match(&candidate.conditions, &controller.parameters))
        })
        .cloned();
    let Some(selected) = selected else {
        return Ok(None);
    };
    consume_transition_triggers(&selected.conditions, &mut controller.parameters);
    let active = ActiveTransition {
        transition_id: selected.transition_id,
        from_state_id: selected.from_state_id,
        to_state_id: selected.to_state_id,
        elapsed_ticks: 0,
        duration_ticks: selected.duration_ticks,
    };
    if active.duration_ticks == 0 {
        controller.current_state_id.clone_from(&active.to_state_id);
        return Ok(Some(transition_fact(
            tick,
            &active,
            AnimationTransitionFactMoment::Completed,
        )));
    }
    let fact = transition_fact(tick, &active, AnimationTransitionFactMoment::Started);
    controller.transition = Some(active);
    Ok(Some(fact))
}

fn transition_fact(
    tick: u64,
    transition: &ActiveTransition,
    moment: AnimationTransitionFactMoment,
) -> AnimationTransitionFact {
    AnimationTransitionFact {
        controller_tick: tick,
        transition_id: transition.transition_id.clone(),
        from_state_id: transition.from_state_id.clone(),
        to_state_id: transition.to_state_id.clone(),
        moment,
        duration_ticks: transition.duration_ticks,
    }
}

fn conditions_match(
    conditions: &[AnimationCondition],
    parameters: &BTreeMap<String, AnimationParameterValue>,
) -> bool {
    conditions.iter().all(|condition| match condition {
        AnimationCondition::FloatGreaterThan {
            parameter_id,
            threshold_milli,
        } => matches!(
            parameters.get(parameter_id),
            Some(AnimationParameterValue::Float(value)) if value > threshold_milli
        ),
        AnimationCondition::FloatLessThanOrEqual {
            parameter_id,
            threshold_milli,
        } => matches!(
            parameters.get(parameter_id),
            Some(AnimationParameterValue::Float(value)) if value <= threshold_milli
        ),
        AnimationCondition::BoolEquals {
            parameter_id,
            value,
        } => matches!(
            parameters.get(parameter_id),
            Some(AnimationParameterValue::Bool(actual)) if actual == value
        ),
        AnimationCondition::TriggerSet { parameter_id } => matches!(
            parameters.get(parameter_id),
            Some(AnimationParameterValue::Trigger(true))
        ),
    })
}

fn consume_transition_triggers(
    conditions: &[AnimationCondition],
    parameters: &mut BTreeMap<String, AnimationParameterValue>,
) {
    for condition in conditions {
        if let AnimationCondition::TriggerSet { parameter_id } = condition {
            parameters.insert(
                parameter_id.clone(),
                AnimationParameterValue::Trigger(false),
            );
        }
    }
}

fn resolved_state(
    entity: u64,
    graph: &ValidatedGraph,
    controller: &ControllerInstance,
) -> Result<AnimationControllerState, AnimationControllerError> {
    let motion = resolve_motion(graph, &controller.current_state_id, &controller.parameters)?;
    let transition = controller
        .transition
        .as_ref()
        .map(|active| {
            Ok(AnimationTransitionState {
                transition_id: active.transition_id.clone(),
                from_state_id: active.from_state_id.clone(),
                to_state_id: active.to_state_id.clone(),
                elapsed_ticks: active.elapsed_ticks,
                duration_ticks: active.duration_ticks,
                target_motion: resolve_motion(graph, &active.to_state_id, &controller.parameters)?,
            })
        })
        .transpose()?;
    Ok(AnimationControllerState {
        entity,
        graph_id: controller.graph_id.clone(),
        graph_version: graph.definition.version,
        asset_id: graph.definition.asset_id.clone(),
        current_state_id: controller.current_state_id.clone(),
        revision: controller.revision,
        controller_tick: controller.last_tick,
        parameters: controller.parameters.clone(),
        motion,
        transition,
        transition_fact: controller.last_transition_fact.clone(),
    })
}

fn resolve_motion(
    graph: &ValidatedGraph,
    state_id: &str,
    parameters: &BTreeMap<String, AnimationParameterValue>,
) -> Result<ResolvedAnimationMotion, AnimationControllerError> {
    let state = graph
        .states
        .get(state_id)
        .ok_or_else(|| AnimationControllerError::CorruptGraph(graph.definition.graph_id.clone()))?;
    match &state.motion {
        AnimationMotionDefinition::Clip {
            clip_id,
            speed_milli,
        } => Ok(ResolvedAnimationMotion {
            clip_a: clip_id.clone(),
            clip_b: None,
            blend_weight_milli: 0,
            speed_milli: *speed_milli,
        }),
        AnimationMotionDefinition::LinearBlend {
            parameter_id,
            low_clip_id,
            high_clip_id,
            minimum_milli,
            maximum_milli,
            speed_milli,
        } => {
            let value = match parameters.get(parameter_id) {
                Some(AnimationParameterValue::Float(value)) => *value,
                _ => {
                    return Err(AnimationControllerError::CorruptGraph(
                        graph.definition.graph_id.clone(),
                    ));
                }
            };
            let clamped = value.clamp(*minimum_milli, *maximum_milli);
            let numerator = i64::from(clamped - minimum_milli) * i64::from(BLEND_WEIGHT_SCALE);
            let denominator = i64::from(maximum_milli - minimum_milli);
            let blend_weight_milli = i32::try_from(numerator / denominator)
                .expect("validated fixed-point blend fits i32");
            Ok(ResolvedAnimationMotion {
                clip_a: low_clip_id.clone(),
                clip_b: Some(high_clip_id.clone()),
                blend_weight_milli,
                speed_milli: *speed_milli,
            })
        }
    }
}
