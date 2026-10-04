//! Forward-only inspection time, admitted by the ordinary lifecycle owner.
use super::*;

use product_host::ProductHostTimeAnswer;
pub(super) use product_host::ProductHostTimeMode as TimeMode;

impl CsharpProductRuntime {
    pub(super) fn execute_time_debug(
        &mut self,
        command: &str,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostDebugResult>, ProductHostRuntimeError> {
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
                self.host_elapsed_ns = 0;
                self.playtest_time = mode;
                self.follow_world_time();
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
        let message = serde_json::to_string(&ProductHostTimeAnswer {
            mode: self.playtest_time,
            simulation_step: CanonicalU64::new(
                self.lifecycle.readout().admitted_simulation_steps(),
            ),
            fixed_step_hz: hz,
            advanced_ms: f64::from(advanced) * 1000.0 / f64::from(hz),
            world_held: self.playtest_time != TimeMode::Realtime,
        })
        .map_err(|error| {
            ProductHostRuntimeError::new(
                "CSHARP_TIME_ENCODE",
                format!("time answer could not be encoded: {error}"),
            )
        })?;
        ProductHostRuntimeReceipt::new(
            ProductHostDebugResult::new(true, message).with_readout(self.readout()),
            outputs,
        )
        .map_err(host_runtime_error)
    }
}

fn time_error(
    message: &str,
) -> Result<ProductHostRuntimeReceipt<ProductHostDebugResult>, ProductHostRuntimeError> {
    ProductHostRuntimeReceipt::new(
        ProductHostDebugResult::new(false, message.to_owned()),
        Vec::new(),
    )
    .map_err(host_runtime_error)
}
