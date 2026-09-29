use serde::{Deserialize, Serialize};

use crate::trigger::diagnostic;
use crate::{
    KinematicTriggerDefinition, TriggerOverlapPair, TriggerVolumeDiagnosticCode,
    TriggerVolumeError, TriggerVolumeSystem,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TriggerVolumeSnapshot {
    pub schema_version: u32,
    pub revision: u64,
    pub definitions: Vec<KinematicTriggerDefinition>,
    #[serde(default)]
    pub inactive_triggers: Vec<u64>,
    pub active_overlaps: Vec<TriggerOverlapPair>,
}

pub fn encode_trigger_snapshot(system: &TriggerVolumeSystem) -> Result<String, TriggerVolumeError> {
    let mut encoded =
        serde_json::to_string_pretty(&system.snapshot()).map_err(|error| TriggerVolumeError {
            diagnostics: vec![diagnostic(
                TriggerVolumeDiagnosticCode::SnapshotDecode,
                None,
                error.to_string(),
            )],
        })?;
    encoded.push('\n');
    Ok(encoded)
}

pub fn decode_trigger_snapshot(input: &str) -> Result<TriggerVolumeSystem, TriggerVolumeError> {
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let snapshot: TriggerVolumeSnapshot = serde_path_to_error::deserialize(&mut deserializer)
        .map_err(|error| TriggerVolumeError {
            diagnostics: vec![diagnostic(
                TriggerVolumeDiagnosticCode::SnapshotDecode,
                None,
                format!("{}: {}", error.path(), error.inner()),
            )],
        })?;
    deserializer.end().map_err(|error| TriggerVolumeError {
        diagnostics: vec![diagnostic(
            TriggerVolumeDiagnosticCode::SnapshotDecode,
            None,
            error.to_string(),
        )],
    })?;
    TriggerVolumeSystem::from_snapshot(snapshot)
}
