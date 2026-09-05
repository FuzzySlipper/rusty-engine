use std::collections::{BTreeMap, BTreeSet};

use render_model::{
    AnimatedMeshClipPose, AnimatedMeshPlaybackCommand, RenderAssetError, RenderAssetKind,
    RenderHandle,
};
use serde::{Deserialize, Serialize};

use crate::{
    verify_asset, PresentationAssetError, PresentationAssetLookup, PresentationOp,
    PresentationOpMeta,
};

use super::{
    AnimationControllerState, AnimationTransitionFact, AnimationTransitionFactMoment,
    AnimationTransitionState, ResolvedAnimationMotion, BLEND_WEIGHT_SCALE,
};

const MAX_ANIMATION_DIAGNOSTICS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AnimationProjectionHandle(u64);

impl AnimationProjectionHandle {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationControllerProjectionState {
    pub entity: u64,
    pub graph_id: String,
    pub graph_version: u32,
    pub state_id: String,
    pub revision: u64,
    pub controller_tick: u64,
    /// The current unscaled controller phase supplied by Rust's admitted
    /// presentation timeline. A renderer uses it to seed a fresh realization,
    /// then may interpolate locally until the next Engine publication.
    #[serde(default, skip_serializing_if = "phase_seconds_is_zero")]
    pub phase_seconds: f64,
    /// Per-clip current samples preserve activation semantics across state
    /// changes and transitions. `phase_seconds` remains a legacy fallback for
    /// older frames that did not carry explicit clip phases.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clip_phases: Vec<AnimationControllerClipPhase>,
    pub motion: ResolvedAnimationMotion,
    pub transition: Option<AnimationTransitionState>,
    pub transition_fact: Option<AnimationTransitionFact>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationControllerClipPhase {
    pub clip: String,
    pub time_seconds: f64,
}

fn phase_seconds_is_zero(value: &f64) -> bool {
    *value == 0.0
}

impl From<&AnimationControllerState> for AnimationControllerProjectionState {
    fn from(state: &AnimationControllerState) -> Self {
        Self {
            entity: state.entity,
            graph_id: state.graph_id.clone(),
            graph_version: state.graph_version,
            state_id: state.current_state_id.clone(),
            revision: state.revision,
            controller_tick: state.controller_tick,
            phase_seconds: 0.0,
            clip_phases: Vec::new(),
            motion: state.motion.clone(),
            transition: state.transition.clone(),
            transition_fact: state.transition_fact.clone(),
        }
    }
}

impl AnimationControllerProjectionState {
    fn from_state_with_tick_duration(
        state: &AnimationControllerState,
        tick_duration_millis: u32,
    ) -> Self {
        let mut projection = Self::from(state);
        projection.phase_seconds =
            state.controller_tick as f64 * f64::from(tick_duration_millis) / 1_000.0;
        projection.clip_phases = initial_clip_phases(state);
        projection
    }

    /// Converts the current controller state into a renderer-neutral frozen
    /// pose. `phase_seconds` must come from the Engine presentation timeline
    /// at capture time; this method neither ticks the controller nor observes
    /// a browser clock.
    pub fn frozen_pose(&self, phase_seconds: f64) -> AnimatedMeshPlaybackCommand {
        let mut clips = BTreeMap::<String, (f64, f32)>::new();
        let transition_progress = self.transition.as_ref().map_or(0.0, |transition| {
            if transition.duration_ticks == 0 {
                1.0
            } else {
                (transition.elapsed_ticks as f32 / transition.duration_ticks as f32).clamp(0.0, 1.0)
            }
        });
        append_frozen_motion(
            &mut clips,
            &self.motion,
            self,
            phase_seconds,
            1.0 - transition_progress,
        );
        if let Some(transition) = &self.transition {
            append_frozen_motion(
                &mut clips,
                &transition.target_motion,
                self,
                phase_seconds,
                transition_progress,
            );
        }
        let clips = clips
            .into_iter()
            .filter_map(|(clip, (time_seconds, weight))| {
                (weight > 0.0).then_some(AnimatedMeshClipPose {
                    clip,
                    time_seconds,
                    weight,
                })
            })
            .collect();
        AnimatedMeshPlaybackCommand::SamplePose { clips }
    }
}

fn append_frozen_motion(
    samples: &mut BTreeMap<String, (f64, f32)>,
    motion: &ResolvedAnimationMotion,
    controller: &AnimationControllerProjectionState,
    fallback_phase_seconds: f64,
    multiplier: f32,
) {
    let blend_b = motion.blend_weight_milli as f32 / BLEND_WEIGHT_SCALE as f32;
    let speed = f64::from(motion.speed_milli) / f64::from(BLEND_WEIGHT_SCALE);
    let mut append = |clip: &str, weight: f32| {
        if weight <= 0.0 {
            return;
        }
        samples
            .entry(clip.to_owned())
            .and_modify(|existing| existing.1 += weight)
            .or_insert((
                controller
                    .clip_phases
                    .iter()
                    .find(|phase| phase.clip == clip)
                    .map(|phase| phase.time_seconds)
                    .unwrap_or(fallback_phase_seconds * speed),
                weight,
            ));
    };
    append(&motion.clip_a, multiplier * (1.0 - blend_b));
    if let Some(clip_b) = &motion.clip_b {
        append(clip_b, multiplier * blend_b);
    }
}

fn initial_clip_phases(state: &AnimationControllerState) -> Vec<AnimationControllerClipPhase> {
    active_motion_speeds(state)
        .into_iter()
        .map(|(clip, _)| AnimationControllerClipPhase {
            clip,
            time_seconds: 0.0,
        })
        .collect()
}

fn advance_clip_phases(
    previous: &AnimationControllerProjectionState,
    state: &AnimationControllerState,
    tick_duration_millis: u32,
) -> Vec<AnimationControllerClipPhase> {
    let elapsed_ticks = state
        .controller_tick
        .saturating_sub(previous.controller_tick);
    let elapsed_seconds = elapsed_ticks as f64 * f64::from(tick_duration_millis) / 1_000.0;
    let previous_phases = previous
        .clip_phases
        .iter()
        .map(|phase| (phase.clip.as_str(), phase.time_seconds))
        .collect::<BTreeMap<_, _>>();
    active_motion_speeds(state)
        .into_iter()
        .map(|(clip, speed)| AnimationControllerClipPhase {
            time_seconds: previous_phases
                .get(clip.as_str())
                .copied()
                .map_or(0.0, |prior| prior + elapsed_seconds * speed),
            clip,
        })
        .collect()
}

fn active_motion_speeds(state: &AnimationControllerState) -> Vec<(String, f64)> {
    let mut clips = BTreeMap::new();
    append_motion_speeds(&mut clips, &state.motion);
    if let Some(transition) = &state.transition {
        append_motion_speeds(&mut clips, &transition.target_motion);
    }
    clips.into_iter().collect()
}

fn append_motion_speeds(clips: &mut BTreeMap<String, f64>, motion: &ResolvedAnimationMotion) {
    let speed = f64::from(motion.speed_milli) / f64::from(BLEND_WEIGHT_SCALE);
    clips.insert(motion.clip_a.clone(), speed);
    if let Some(clip_b) = &motion.clip_b {
        clips.insert(clip_b.clone(), speed);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationProjectionDescriptor {
    pub target: RenderHandle,
    pub asset: String,
    pub content_hash: String,
    pub tick_duration_millis: u32,
    pub controller: AnimationControllerProjectionState,
}

impl AnimationProjectionDescriptor {
    /// Produces one ordinary retained graphics operation for a frozen capture.
    /// The caller appends it only to an isolated captured scene, never to the
    /// live controller-driven world.
    pub fn frozen_pose_operation(&self, phase_seconds: f64) -> render_model::RenderDiff {
        render_model::RenderDiff::SetAnimatedMeshPlayback {
            handle: self.target,
            playback: self.controller.frozen_pose(phase_seconds),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationProjectionTarget {
    pub target: RenderHandle,
    pub content_hash: String,
    pub tick_duration_millis: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AnimationProjectionOp {
    Create {
        handle: AnimationProjectionHandle,
        descriptor: AnimationProjectionDescriptor,
    },
    Update {
        handle: AnimationProjectionHandle,
        controller: AnimationControllerProjectionState,
    },
    Destroy {
        handle: AnimationProjectionHandle,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnimationProjectionDiagnosticCode {
    InvalidDescriptor,
    DuplicateHandle,
    DuplicateController,
    UnknownHandle,
    UnknownController,
    UnknownTarget,
    AssetMissing,
    AssetKindMismatch,
    ContentHashMismatch,
    ClipMissing,
    InvalidBlendWeight,
    InvalidTransition,
    StaleRevision,
    HandleExhausted,
    UnavailableHost,
    IncompatibleRig,
    CompatibilityFallback,
    HostFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationProjectionDiagnostic {
    pub code: AnimationProjectionDiagnosticCode,
    pub sequence: u32,
    pub handle: Option<AnimationProjectionHandle>,
    pub target: Option<RenderHandle>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnimationProjectionReadout {
    pub active_controllers: u32,
    pub referenced_assets: u32,
    pub diagnostics: Vec<AnimationProjectionDiagnostic>,
}

pub trait RenderTargetLookup {
    fn contains_render_target(&self, handle: RenderHandle) -> bool;
}

impl RenderTargetLookup for BTreeSet<RenderHandle> {
    fn contains_render_target(&self, handle: RenderHandle) -> bool {
        self.contains(&handle)
    }
}

#[derive(Debug, Clone)]
pub struct AnimationProjector {
    next_handle: u64,
    active: BTreeMap<AnimationProjectionHandle, AnimationProjectionDescriptor>,
    entity_handles: BTreeMap<u64, AnimationProjectionHandle>,
    referenced_assets: BTreeSet<String>,
    diagnostics: Vec<AnimationProjectionDiagnostic>,
}

impl Default for AnimationProjector {
    fn default() -> Self {
        Self {
            next_handle: 1,
            active: BTreeMap::new(),
            entity_handles: BTreeMap::new(),
            referenced_assets: BTreeSet::new(),
            diagnostics: Vec::new(),
        }
    }
}

impl AnimationProjector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn project(
        &mut self,
        assets: &impl PresentationAssetLookup,
        targets: &impl RenderTargetLookup,
        meta: PresentationOpMeta,
        op: AnimationProjectionOp,
    ) -> Result<PresentationOp, AnimationProjectionDiagnostic> {
        let mut projected = self.project_batch(assets, targets, vec![(meta, op)])?;
        Ok(projected.pop().expect("one input produces one operation"))
    }

    pub fn project_batch(
        &mut self,
        assets: &impl PresentationAssetLookup,
        targets: &impl RenderTargetLookup,
        ops: Vec<(PresentationOpMeta, AnimationProjectionOp)>,
    ) -> Result<Vec<PresentationOp>, AnimationProjectionDiagnostic> {
        let mut staged = self.clone();
        let mut projected = Vec::with_capacity(ops.len());
        for (meta, op) in ops {
            if let Err(code) = staged.validate_and_apply(assets, targets, &op) {
                let diagnostic = AnimationProjectionDiagnostic {
                    code,
                    sequence: meta.sequence,
                    handle: operation_handle(&op),
                    target: operation_target(&op),
                    message: diagnostic_message(code).to_string(),
                };
                self.retain_diagnostic(diagnostic.clone());
                return Err(diagnostic);
            }
            projected.push(PresentationOp::Animation { meta, op });
        }
        *self = staged;
        Ok(projected)
    }

    pub fn create_for_state(
        &mut self,
        assets: &impl PresentationAssetLookup,
        targets: &impl RenderTargetLookup,
        projection: AnimationProjectionTarget,
        state: &AnimationControllerState,
        meta: PresentationOpMeta,
    ) -> Result<PresentationOp, AnimationProjectionDiagnostic> {
        let handle = AnimationProjectionHandle::new(self.next_handle);
        let Some(next_handle) = self.next_handle.checked_add(1) else {
            let diagnostic = AnimationProjectionDiagnostic {
                code: AnimationProjectionDiagnosticCode::HandleExhausted,
                sequence: meta.sequence,
                handle: None,
                target: Some(projection.target),
                message: diagnostic_message(AnimationProjectionDiagnosticCode::HandleExhausted)
                    .to_string(),
            };
            self.retain_diagnostic(diagnostic.clone());
            return Err(diagnostic);
        };
        let result = self.project(
            assets,
            targets,
            meta,
            AnimationProjectionOp::Create {
                handle,
                descriptor: AnimationProjectionDescriptor {
                    target: projection.target,
                    asset: state.asset_id.clone(),
                    content_hash: projection.content_hash,
                    tick_duration_millis: projection.tick_duration_millis,
                    controller: AnimationControllerProjectionState::from_state_with_tick_duration(
                        state,
                        projection.tick_duration_millis,
                    ),
                },
            },
        )?;
        self.next_handle = next_handle;
        Ok(result)
    }

    pub fn update_for_state(
        &mut self,
        assets: &impl PresentationAssetLookup,
        targets: &impl RenderTargetLookup,
        state: &AnimationControllerState,
        meta: PresentationOpMeta,
    ) -> Result<PresentationOp, AnimationProjectionDiagnostic> {
        let Some(handle) = self.entity_handles.get(&state.entity).copied() else {
            let diagnostic = AnimationProjectionDiagnostic {
                code: AnimationProjectionDiagnosticCode::UnknownController,
                sequence: meta.sequence,
                handle: None,
                target: None,
                message: diagnostic_message(AnimationProjectionDiagnosticCode::UnknownController)
                    .to_string(),
            };
            self.retain_diagnostic(diagnostic.clone());
            return Err(diagnostic);
        };
        let descriptor = self
            .active
            .get(&handle)
            .expect("controller handle was checked above");
        let mut controller = AnimationControllerProjectionState::from_state_with_tick_duration(
            state,
            descriptor.tick_duration_millis,
        );
        controller.clip_phases = advance_clip_phases(
            &descriptor.controller,
            state,
            descriptor.tick_duration_millis,
        );
        self.project(
            assets,
            targets,
            meta,
            AnimationProjectionOp::Update { handle, controller },
        )
    }

    pub fn destroy_entity(
        &mut self,
        entity: u64,
        meta: PresentationOpMeta,
    ) -> Result<PresentationOp, AnimationProjectionDiagnostic> {
        let Some(handle) = self.entity_handles.get(&entity).copied() else {
            let diagnostic = AnimationProjectionDiagnostic {
                code: AnimationProjectionDiagnosticCode::UnknownController,
                sequence: meta.sequence,
                handle: None,
                target: None,
                message: diagnostic_message(AnimationProjectionDiagnosticCode::UnknownController)
                    .to_string(),
            };
            self.retain_diagnostic(diagnostic.clone());
            return Err(diagnostic);
        };
        self.active
            .remove(&handle)
            .expect("entity handle and active descriptor stay coherent");
        self.entity_handles.remove(&entity);
        Ok(PresentationOp::Animation {
            meta,
            op: AnimationProjectionOp::Destroy { handle },
        })
    }

    pub fn handle(&self, entity: u64) -> Option<AnimationProjectionHandle> {
        self.entity_handles.get(&entity).copied()
    }

    pub fn descriptor(
        &self,
        handle: AnimationProjectionHandle,
    ) -> Option<&AnimationProjectionDescriptor> {
        self.active.get(&handle)
    }

    /// Iterates retained controller projections in stable handle order for a
    /// fresh realization baseline.
    pub fn active_projections(
        &self,
    ) -> impl Iterator<Item = (AnimationProjectionHandle, &AnimationProjectionDescriptor)> + '_
    {
        self.active
            .iter()
            .map(|(&handle, descriptor)| (handle, descriptor))
    }

    pub fn readout(&self) -> AnimationProjectionReadout {
        AnimationProjectionReadout {
            active_controllers: self.active.len() as u32,
            referenced_assets: self.referenced_assets.len() as u32,
            diagnostics: self.diagnostics.clone(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn retain_diagnostic(&mut self, diagnostic: AnimationProjectionDiagnostic) {
        if let Some(index) = self.diagnostics.iter().position(|existing| {
            existing.code == diagnostic.code
                && existing.handle == diagnostic.handle
                && existing.target == diagnostic.target
                && existing.message == diagnostic.message
        }) {
            self.diagnostics[index] = diagnostic;
            return;
        }
        if self.diagnostics.len() == MAX_ANIMATION_DIAGNOSTICS {
            self.diagnostics.remove(0);
        }
        self.diagnostics.push(diagnostic);
    }

    fn validate_and_apply(
        &mut self,
        assets: &impl PresentationAssetLookup,
        targets: &impl RenderTargetLookup,
        op: &AnimationProjectionOp,
    ) -> Result<(), AnimationProjectionDiagnosticCode> {
        match op {
            AnimationProjectionOp::Create { handle, descriptor } => {
                if self.active.contains_key(handle) {
                    return Err(AnimationProjectionDiagnosticCode::DuplicateHandle);
                }
                if self
                    .entity_handles
                    .contains_key(&descriptor.controller.entity)
                {
                    return Err(AnimationProjectionDiagnosticCode::DuplicateController);
                }
                validate_descriptor(assets, targets, descriptor)?;
                if handle.raw() >= self.next_handle {
                    self.next_handle = handle
                        .raw()
                        .checked_add(1)
                        .ok_or(AnimationProjectionDiagnosticCode::HandleExhausted)?;
                }
                self.referenced_assets.insert(descriptor.asset.clone());
                self.entity_handles
                    .insert(descriptor.controller.entity, *handle);
                self.active.insert(*handle, descriptor.clone());
            }
            AnimationProjectionOp::Update { handle, controller } => {
                let current = self
                    .active
                    .get(handle)
                    .cloned()
                    .ok_or(AnimationProjectionDiagnosticCode::UnknownHandle)?;
                if controller.entity != current.controller.entity
                    || controller.graph_id != current.controller.graph_id
                    || controller.graph_version != current.controller.graph_version
                {
                    return Err(AnimationProjectionDiagnosticCode::InvalidDescriptor);
                }
                if controller.revision <= current.controller.revision {
                    return Err(AnimationProjectionDiagnosticCode::StaleRevision);
                }
                let updated = AnimationProjectionDescriptor {
                    controller: controller.clone(),
                    ..current
                };
                validate_descriptor(assets, targets, &updated)?;
                self.active.insert(*handle, updated);
            }
            AnimationProjectionOp::Destroy { handle } => {
                let removed = self
                    .active
                    .remove(handle)
                    .ok_or(AnimationProjectionDiagnosticCode::UnknownHandle)?;
                self.entity_handles.remove(&removed.controller.entity);
            }
        }
        Ok(())
    }
}

fn validate_descriptor(
    assets: &impl PresentationAssetLookup,
    targets: &impl RenderTargetLookup,
    descriptor: &AnimationProjectionDescriptor,
) -> Result<(), AnimationProjectionDiagnosticCode> {
    if descriptor.asset.is_empty()
        || descriptor.content_hash.is_empty()
        || descriptor.tick_duration_millis == 0
        || descriptor.controller.graph_id.is_empty()
        || descriptor.controller.graph_version == 0
        || descriptor.controller.state_id.is_empty()
        || !descriptor.controller.phase_seconds.is_finite()
        || descriptor.controller.phase_seconds < 0.0
    {
        return Err(AnimationProjectionDiagnosticCode::InvalidDescriptor);
    }
    if !targets.contains_render_target(descriptor.target) {
        return Err(AnimationProjectionDiagnosticCode::UnknownTarget);
    }
    verify_asset(
        assets,
        &descriptor.asset,
        RenderAssetKind::AnimatedMesh,
        Some(&descriptor.content_hash),
    )
    .map_err(asset_diagnostic)?;
    validate_motion(&descriptor.controller.motion)?;
    if descriptor.controller.clip_phases.len() > 4
        || descriptor.controller.clip_phases.iter().any(|phase| {
            phase.clip.is_empty() || !phase.time_seconds.is_finite() || phase.time_seconds < 0.0
        })
        || descriptor
            .controller
            .clip_phases
            .windows(2)
            .any(|pair| pair[0].clip >= pair[1].clip)
    {
        return Err(AnimationProjectionDiagnosticCode::InvalidDescriptor);
    }
    if let Some(transition) = &descriptor.controller.transition {
        if transition.transition_id.is_empty()
            || transition.from_state_id != descriptor.controller.state_id
            || transition.to_state_id.is_empty()
            || transition.duration_ticks == 0
            || transition.elapsed_ticks >= transition.duration_ticks
        {
            return Err(AnimationProjectionDiagnosticCode::InvalidTransition);
        }
        validate_motion(&transition.target_motion)?;
    }
    if let Some(fact) = &descriptor.controller.transition_fact {
        validate_fact(fact, descriptor.controller.transition.as_ref())?;
    }
    Ok(())
}

fn validate_motion(
    motion: &ResolvedAnimationMotion,
) -> Result<(), AnimationProjectionDiagnosticCode> {
    if motion.clip_a.is_empty()
        || motion.speed_milli <= 0
        || motion.clip_b.as_ref().is_some_and(|clip| clip.is_empty())
    {
        return Err(AnimationProjectionDiagnosticCode::ClipMissing);
    }
    if !(0..=BLEND_WEIGHT_SCALE).contains(&motion.blend_weight_milli)
        || (motion.clip_b.is_none() && motion.blend_weight_milli != 0)
    {
        return Err(AnimationProjectionDiagnosticCode::InvalidBlendWeight);
    }
    Ok(())
}

fn validate_fact(
    fact: &AnimationTransitionFact,
    active: Option<&AnimationTransitionState>,
) -> Result<(), AnimationProjectionDiagnosticCode> {
    if fact.transition_id.is_empty()
        || fact.from_state_id.is_empty()
        || fact.to_state_id.is_empty()
        || (fact.moment == AnimationTransitionFactMoment::Started
            && !active.is_some_and(|transition| {
                transition.transition_id == fact.transition_id
                    && transition.from_state_id == fact.from_state_id
                    && transition.to_state_id == fact.to_state_id
                    && transition.duration_ticks == fact.duration_ticks
            }))
        || (fact.moment == AnimationTransitionFactMoment::Completed && active.is_some())
    {
        return Err(AnimationProjectionDiagnosticCode::InvalidTransition);
    }
    Ok(())
}

fn asset_diagnostic(error: PresentationAssetError) -> AnimationProjectionDiagnosticCode {
    match error {
        PresentationAssetError::Missing(_) => AnimationProjectionDiagnosticCode::AssetMissing,
        PresentationAssetError::Invalid(RenderAssetError::ContentHashMismatch { .. }) => {
            AnimationProjectionDiagnosticCode::ContentHashMismatch
        }
        PresentationAssetError::Invalid(_) => AnimationProjectionDiagnosticCode::AssetKindMismatch,
    }
}

fn operation_handle(op: &AnimationProjectionOp) -> Option<AnimationProjectionHandle> {
    Some(match op {
        AnimationProjectionOp::Create { handle, .. }
        | AnimationProjectionOp::Update { handle, .. }
        | AnimationProjectionOp::Destroy { handle } => *handle,
    })
}

fn operation_target(op: &AnimationProjectionOp) -> Option<RenderHandle> {
    match op {
        AnimationProjectionOp::Create { descriptor, .. } => Some(descriptor.target),
        AnimationProjectionOp::Update { .. } | AnimationProjectionOp::Destroy { .. } => None,
    }
}

const fn diagnostic_message(code: AnimationProjectionDiagnosticCode) -> &'static str {
    match code {
        AnimationProjectionDiagnosticCode::InvalidDescriptor => {
            "animation projection descriptor is invalid"
        }
        AnimationProjectionDiagnosticCode::DuplicateHandle => {
            "animation projection handle is already active"
        }
        AnimationProjectionDiagnosticCode::DuplicateController => {
            "entity already has an animation projection"
        }
        AnimationProjectionDiagnosticCode::UnknownHandle => {
            "animation projection handle is not active"
        }
        AnimationProjectionDiagnosticCode::UnknownController => {
            "entity has no animation projection"
        }
        AnimationProjectionDiagnosticCode::UnknownTarget => {
            "animation target render handle is unavailable"
        }
        AnimationProjectionDiagnosticCode::AssetMissing => "animated mesh is unavailable",
        AnimationProjectionDiagnosticCode::AssetKindMismatch => {
            "animation asset has the wrong resource kind"
        }
        AnimationProjectionDiagnosticCode::ContentHashMismatch => {
            "animated mesh content hash does not match"
        }
        AnimationProjectionDiagnosticCode::ClipMissing => {
            "animation motion references an invalid clip"
        }
        AnimationProjectionDiagnosticCode::InvalidBlendWeight => {
            "animation blend weight is invalid"
        }
        AnimationProjectionDiagnosticCode::InvalidTransition => {
            "animation transition projection is invalid"
        }
        AnimationProjectionDiagnosticCode::StaleRevision => {
            "animation controller revision is not newer"
        }
        AnimationProjectionDiagnosticCode::HandleExhausted => {
            "animation projection handles are exhausted"
        }
        AnimationProjectionDiagnosticCode::UnavailableHost => "animation host is unavailable",
        AnimationProjectionDiagnosticCode::IncompatibleRig => {
            "animation clip is incompatible with the target rig"
        }
        AnimationProjectionDiagnosticCode::CompatibilityFallback => {
            "animation host used a compatibility fallback"
        }
        AnimationProjectionDiagnosticCode::HostFailure => "animation host operation failed",
    }
}
