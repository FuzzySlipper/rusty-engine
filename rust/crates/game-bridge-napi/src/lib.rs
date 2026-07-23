//! Narrow N-API transport for the trusted TypeScript game-code comparison.
//!
//! Each gameplay invocation crosses as one JSON batch in and one JSON batch
//! out. JSON is deliberately an early measurement format, not a permanent ABI
//! decision.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use core_ids::EntityId;
use napi::{Error, Result};
use napi_derive::napi;
use project_code_host::{
    decode_project_snapshot, encode_project_snapshot, ProjectCodeRuntime, ProjectDecisionBatch,
};
use serde::Serialize;

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BridgeStats {
    gameplay_calls: u64,
    bytes_in: u64,
    bytes_out: u64,
}

struct BridgeSession {
    runtime: ProjectCodeRuntime,
    stats: BridgeStats,
}

#[derive(Default)]
struct Registry {
    next_handle: u32,
    sessions: BTreeMap<u32, BridgeSession>,
}

impl Registry {
    fn insert(&mut self, runtime: ProjectCodeRuntime) -> u32 {
        self.next_handle = self.next_handle.saturating_add(1).max(1);
        let handle = self.next_handle;
        self.sessions.insert(
            handle,
            BridgeSession {
                runtime,
                stats: BridgeStats::default(),
            },
        );
        handle
    }
}

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

fn registry() -> &'static Mutex<Registry> {
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}

fn with_session<T>(
    handle: u32,
    operation: impl FnOnce(&mut BridgeSession) -> Result<T>,
) -> Result<T> {
    let mut guard = registry()
        .lock()
        .map_err(|_| Error::from_reason("project runtime registry lock poisoned"))?;
    let session = guard
        .sessions
        .get_mut(&handle)
        .ok_or_else(|| Error::from_reason(format!("unknown project runtime handle {handle}")))?;
    operation(session)
}

fn json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|error| Error::from_reason(error.to_string()))
}

fn record_call(session: &mut BridgeSession, bytes_in: usize, output: String) -> String {
    session.stats.gameplay_calls = session.stats.gameplay_calls.saturating_add(1);
    session.stats.bytes_in = session.stats.bytes_in.saturating_add(bytes_in as u64);
    session.stats.bytes_out = session.stats.bytes_out.saturating_add(output.len() as u64);
    output
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateReceipt {
    handle: u32,
    actor: u64,
    switch: u64,
    door: u64,
}

#[napi]
pub fn create_project_door_runtime(initial_state_json: String) -> Result<String> {
    let initial_state = serde_json::from_str(&initial_state_json)
        .map_err(|error| Error::from_reason(error.to_string()))?;
    let (ids, runtime) = ProjectCodeRuntime::security_door(initial_state)
        .map_err(|error| Error::from_reason(error.to_string()))?;
    let handle = registry()
        .lock()
        .map_err(|_| Error::from_reason("project runtime registry lock poisoned"))?
        .insert(runtime);
    json(&CreateReceipt {
        handle,
        actor: ids.actor.raw(),
        switch: ids.switch.raw(),
        door: ids.door.raw(),
    })
}

#[napi]
pub fn begin_project_interaction(handle: u32, actor: u32, target: u32) -> Result<String> {
    with_session(handle, |session| {
        let wave = session
            .runtime
            .begin_interaction(EntityId::new(actor as u64), EntityId::new(target as u64))
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let output = json(&wave)?;
        Ok(record_call(session, 8, output))
    })
}

#[napi]
pub fn apply_project_decisions(handle: u32, decision_json: String) -> Result<String> {
    with_session(handle, |session| {
        let decision: ProjectDecisionBatch = serde_json::from_str(&decision_json)
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let receipt = session
            .runtime
            .apply_decisions(decision)
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let output = json(&receipt)?;
        Ok(record_call(session, decision_json.len(), output))
    })
}

#[napi]
pub fn advance_project_time(handle: u32, ticks: u32) -> Result<String> {
    with_session(handle, |session| {
        let wave = session
            .runtime
            .advance_by(ticks as u64)
            .map_err(|error| Error::from_reason(error.to_string()))?;
        let output = json(&wave)?;
        Ok(record_call(session, 4, output))
    })
}

#[napi]
pub fn read_project_runtime(handle: u32) -> Result<String> {
    with_session(handle, |session| {
        let output = json(&session.runtime.readout())?;
        Ok(record_call(session, 0, output))
    })
}

#[napi]
pub fn save_project_runtime(handle: u32) -> Result<String> {
    with_session(handle, |session| {
        let output = encode_project_snapshot(&session.runtime)
            .map_err(|error| Error::from_reason(error.to_string()))?;
        Ok(record_call(session, 0, output))
    })
}

#[napi]
pub fn restore_project_runtime(snapshot_json: String) -> Result<u32> {
    let runtime = decode_project_snapshot(&snapshot_json)
        .map_err(|error| Error::from_reason(error.to_string()))?;
    Ok(registry()
        .lock()
        .map_err(|_| Error::from_reason("project runtime registry lock poisoned"))?
        .insert(runtime))
}

#[napi]
pub fn read_bridge_stats(handle: u32) -> Result<String> {
    with_session(handle, |session| json(&session.stats))
}

#[napi]
pub fn close_project_runtime(handle: u32) -> Result<bool> {
    let mut guard = registry()
        .lock()
        .map_err(|_| Error::from_reason("project runtime registry lock poisoned"))?;
    Ok(guard.sessions.remove(&handle).is_some())
}
