//! Forward-only inspection time, admitted by the ordinary lifecycle owner.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum TimeMode {
    Realtime,
    Manual,
    ActionDriven,
}

impl TimeMode {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Realtime => "realtime",
            Self::Manual => "manual",
            Self::ActionDriven => "action-driven",
        }
    }
}
impl CsharpProductRuntime {
    pub(super) fn execute_time_debug(
        &mut self,
        command: &str,
    ) -> Result<ProductDevRuntimeReceipt<ProductDevDebugResult>, ProductDevRuntimeError> {
        let words: Vec<_> = command.split_whitespace().collect();
        let mut outputs = Vec::new();
        let mut advanced = 0_u32;
        let hz = match self.lifecycle.configuration() {
            RuntimeLifecycleConfig::Realtime(config) => config.fixed_step_hz(),
            _ => return time_error("manual time requires a configured fixed-step cadence"),
        };
        match words.as_slice() {
            ["engine.time"] => {}
            ["engine.time.mode", mode] => {
                let mode = match *mode {
                    "realtime" => TimeMode::Realtime,
                    "manual" => TimeMode::Manual,
                    "action-driven" => TimeMode::ActionDriven,
                    _ => return time_error("mode must be realtime, manual or action-driven"),
                };
                self.lifecycle.reset_realtime_baseline();
                self.playtest_time = mode;
            }
            ["engine.time.advance", milliseconds] => {
                if self.playtest_time == TimeMode::Realtime {
                    return time_error("select manual or action-driven time before advancing");
                }
                let milliseconds: f64 = match milliseconds.parse() {
                    Ok(value) if value > 0.0 && value <= 2000.0 => value,
                    _ => return time_error("advance duration must be finite and in (0, 2000] ms"),
                };
                let count = (milliseconds * f64::from(hz) / 1000.0).ceil() as u32;
                for _ in 0..count {
                    let admission = self
                        .lifecycle
                        .admit_manual_step()
                        .map_err(|error| self.lifecycle_runtime_error(error))?;
                    outputs.extend(
                        // Manual admission changes who advances the clock, not the
                        // realtime product contract or its animation step semantics.
                        self.update_admitted(REALTIME_UPDATE_MODE, None, admission, 0)
                            .map_err(|error| self.runtime_error(error))?,
                    );
                    advanced += 1;
                }
            }
            _ => {
                return time_error(
                    "expected engine.time, engine.time.mode MODE or engine.time.advance MS",
                )
            }
        }
        let message = serde_json::json!({
            "mode": self.playtest_time,
            "simulationStep": self.lifecycle.readout().admitted_simulation_steps().to_string(),
            "fixedStepHz": hz,
            "advancedMs": f64::from(advanced) * 1000.0 / f64::from(hz),
            "worldHeld": self.playtest_time != TimeMode::Realtime,
        })
        .to_string();
        ProductDevRuntimeReceipt::new(
            ProductDevDebugResult::new(true, message)
                .map_err(host_runtime_error)?
                .with_readout(self.readout()),
            outputs,
        )
        .map_err(host_runtime_error)
    }
}

fn time_error(
    message: &str,
) -> Result<ProductDevRuntimeReceipt<ProductDevDebugResult>, ProductDevRuntimeError> {
    ProductDevRuntimeReceipt::new(
        ProductDevDebugResult::new(false, message.to_owned()).map_err(host_runtime_error)?,
        Vec::new(),
    )
    .map_err(host_runtime_error)
}
