//! Gameplay time requests. The runtime lifecycle owns the rate and admits the
//! steps; this bridge validates a product's request and stages it for the
//! runtime to settle when the callback returns.
use std::ffi::c_void;
use std::num::NonZeroU32;

use csharp_engine_abi::*;
use runtime_lifecycle::{GameplayRate, GameplayTime, GAMEPLAY_RATE_REALTIME_PPM};

use crate::composition::ABI_OK;
use crate::operation_diagnostics::{clear_receipt, refuse};
use crate::CsharpEngineServicesError;

/// Longest bounded advance one request may ask for.
pub const MAX_GAMEPLAY_ADVANCE_SECONDS: f64 = 3600.0;
/// Rounding slack so that, say, 0.1 s at 60 Hz is 6 steps rather than 7.
const STEP_ROUNDING_TOLERANCE: f64 = 1e-6;

/// One validated selection a callback staged. The latest in a callback wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameplayTimeRequest {
    Rate(GameplayRate),
    Advance {
        steps: NonZeroU32,
        rate: GameplayRate,
    },
}

pub(crate) struct RuntimeGameplayTimeBridge {
    fixed_step_hz: Option<u32>,
    effective: GameplayTime,
    staged: Option<GameplayTimeRequest>,
}

impl RuntimeGameplayTimeBridge {
    pub(crate) fn new() -> Self {
        Self {
            fixed_step_hz: None,
            effective: GameplayTime::default(),
            staged: None,
        }
    }

    /// The lifecycle's current selection; `None` cadence refuses requests
    /// (demand and external runtimes have no realtime rate).
    pub(crate) fn set(&mut self, fixed_step_hz: Option<u32>, effective: GameplayTime) {
        self.fixed_step_hz = fixed_step_hz;
        self.effective = effective;
    }

    pub(crate) fn begin_call(&mut self) {
        self.staged = None;
    }

    pub(crate) fn take_call(&mut self) -> Option<GameplayTimeRequest> {
        self.staged.take()
    }

    fn readout(&self) -> NativeGameplayTimeReadout {
        let (rate, remaining) = match self.staged {
            None if !self.effective.selected() => (GameplayRate::REALTIME, 0),
            None => (
                self.effective.rate(),
                self.effective.advance_remaining_steps(),
            ),
            Some(GameplayTimeRequest::Rate(rate)) => (rate, 0),
            Some(GameplayTimeRequest::Advance { steps, rate }) => (rate, steps.get()),
        };
        NativeGameplayTimeReadout {
            selected: self.effective.selected() || self.staged.is_some(),
            rate: rate_value(rate),
            advance_remaining_steps: remaining,
            fixed_step_hz: self.fixed_step_hz.unwrap_or(0),
        }
    }

    fn stage(
        &mut self,
        request: impl FnOnce(u32) -> Result<GameplayTimeRequest, CsharpEngineServicesError>,
    ) -> Result<NativeGameplayTimeReadout, CsharpEngineServicesError> {
        let hz = self.fixed_step_hz.ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_GAMEPLAY_TIME_MODE",
                "gameplay time needs a realtime runtime; demand and external runtimes have no rate",
            )
        })?;
        self.staged = Some(request(hz)?);
        Ok(self.readout())
    }
}

pub(crate) fn api(bridge: &mut RuntimeGameplayTimeBridge) -> NativeGameplayTimeApi {
    NativeGameplayTimeApi {
        context: (bridge as *mut RuntimeGameplayTimeBridge).cast(),
        select_rate,
        advance,
        read,
    }
}

/// The product-facing rate a lifecycle rate stands for.
pub fn rate_value(rate: GameplayRate) -> f64 {
    f64::from(rate.parts_per_million()) / f64::from(GAMEPLAY_RATE_REALTIME_PPM)
}

fn gameplay_rate(value: f64, allow_hold: bool) -> Result<GameplayRate, CsharpEngineServicesError> {
    let lowest_ok = if allow_hold {
        value >= 0.0
    } else {
        value > 0.0
    };
    if !value.is_finite() || !lowest_ok || value > 1.0 {
        let range = if allow_hold { "[0, 1]" } else { "(0, 1]" };
        return Err(CsharpEngineServicesError::new(
            "CSHARP_GAMEPLAY_TIME_RATE",
            format!("gameplay rate {value} is outside {range}"),
        ));
    }
    let parts = (value * f64::from(GAMEPLAY_RATE_REALTIME_PPM)).round() as u32;
    // A positive rate below a millionth still moves.
    let parts = if value > 0.0 { parts.max(1) } else { parts };
    Ok(GameplayRate::from_parts_per_million(parts).expect("a rate within [0, 1]"))
}

fn advance_steps(seconds: f64, hz: u32) -> Result<NonZeroU32, CsharpEngineServicesError> {
    if !seconds.is_finite() || seconds <= 0.0 || seconds > MAX_GAMEPLAY_ADVANCE_SECONDS {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_GAMEPLAY_TIME_ADVANCE",
            format!(
                "gameplay advance of {seconds} s is outside (0, {MAX_GAMEPLAY_ADVANCE_SECONDS}] s"
            ),
        ));
    }
    let steps = (seconds * f64::from(hz) - STEP_ROUNDING_TOLERANCE)
        .ceil()
        .max(1.0);
    Ok(NonZeroU32::new(steps as u32).expect("at least one step"))
}

unsafe extern "C" fn select_rate(
    context: *mut c_void,
    request: *const NativeGameplayTimeRateRequest,
    output: *mut NativeGameplayTimeReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeGameplayTimeBridge>() };
    let request = unsafe { *request };
    match bridge.stage(|_| gameplay_rate(request.rate, true).map(GameplayTimeRequest::Rate)) {
        Ok(readout) => {
            unsafe { *output = readout };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

unsafe extern "C" fn advance(
    context: *mut c_void,
    request: *const NativeGameplayTimeAdvanceRequest,
    output: *mut NativeGameplayTimeReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeGameplayTimeBridge>() };
    let request = unsafe { *request };
    let staged = bridge.stage(|hz| {
        Ok(GameplayTimeRequest::Advance {
            rate: gameplay_rate(request.rate, false)?,
            steps: advance_steps(request.seconds, hz)?,
        })
    });
    match staged {
        Ok(readout) => {
            unsafe { *output = readout };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

unsafe extern "C" fn read(
    context: *mut c_void,
    output: *mut NativeGameplayTimeReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &*context.cast::<RuntimeGameplayTimeBridge>() };
    unsafe { *output = bridge.readout() };
    ABI_OK
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation_diagnostics::receipt_codes;

    fn bridge(hz: Option<u32>) -> RuntimeGameplayTimeBridge {
        let mut bridge = RuntimeGameplayTimeBridge::new();
        bridge.set(hz, GameplayTime::default());
        bridge.begin_call();
        bridge
    }

    fn rate(
        bridge: &mut RuntimeGameplayTimeBridge,
        rate: f64,
    ) -> Result<NativeGameplayTimeReadout, Vec<String>> {
        let mut output = NativeGameplayTimeReadout::default();
        let mut error = crate::operation_diagnostics::empty_receipt();
        let status = unsafe {
            select_rate(
                (bridge as *mut RuntimeGameplayTimeBridge).cast(),
                &NativeGameplayTimeRateRequest { rate },
                &mut output,
                &mut error,
            )
        };
        if status == ABI_OK {
            Ok(output)
        } else {
            Err(receipt_codes(&error))
        }
    }

    fn advance_for(
        bridge: &mut RuntimeGameplayTimeBridge,
        seconds: f64,
        rate: f64,
    ) -> Result<NativeGameplayTimeReadout, Vec<String>> {
        let mut output = NativeGameplayTimeReadout::default();
        let mut error = crate::operation_diagnostics::empty_receipt();
        let status = unsafe {
            advance(
                (bridge as *mut RuntimeGameplayTimeBridge).cast(),
                &NativeGameplayTimeAdvanceRequest { seconds, rate },
                &mut output,
                &mut error,
            )
        };
        if status == ABI_OK {
            Ok(output)
        } else {
            Err(receipt_codes(&error))
        }
    }

    #[test]
    fn requests_stage_the_latest_valid_selection_and_refuse_invalid_ones() {
        let mut bridge = bridge(Some(60));
        let held = rate(&mut bridge, 0.0).unwrap();
        assert!(held.selected);
        assert_eq!(held.rate, 0.0);
        let slow = rate(&mut bridge, 0.1).unwrap();
        assert_eq!(slow.rate, 0.1);
        for invalid in [-0.1, 1.5, f64::NAN, f64::INFINITY] {
            assert_eq!(
                rate(&mut bridge, invalid),
                Err(vec!["CSHARP_GAMEPLAY_TIME_RATE".to_owned()])
            );
        }
        // A refusal leaves the earlier valid request staged.
        assert_eq!(
            bridge.take_call(),
            Some(GameplayTimeRequest::Rate(
                GameplayRate::from_parts_per_million(100_000).unwrap()
            ))
        );
        assert_eq!(bridge.take_call(), None);
    }

    #[test]
    fn an_advance_rounds_up_to_whole_steps() {
        let mut bridge = bridge(Some(60));
        assert_eq!(
            advance_for(&mut bridge, 0.1, 1.0)
                .unwrap()
                .advance_remaining_steps,
            6
        );
        assert_eq!(
            advance_for(&mut bridge, 2.0, 0.5)
                .unwrap()
                .advance_remaining_steps,
            120
        );
        assert_eq!(
            advance_for(&mut bridge, 0.001, 1.0)
                .unwrap()
                .advance_remaining_steps,
            1
        );
        for (seconds, rate) in [(0.0, 1.0), (-1.0, 1.0), (3601.0, 1.0), (f64::NAN, 1.0)] {
            assert_eq!(
                advance_for(&mut bridge, seconds, rate),
                Err(vec!["CSHARP_GAMEPLAY_TIME_ADVANCE".to_owned()])
            );
        }
        assert_eq!(
            advance_for(&mut bridge, 1.0, 0.0),
            Err(vec!["CSHARP_GAMEPLAY_TIME_RATE".to_owned()])
        );
    }

    #[test]
    fn a_runtime_without_a_realtime_cadence_refuses() {
        let mut bridge = bridge(None);
        assert_eq!(
            rate(&mut bridge, 0.5),
            Err(vec!["CSHARP_GAMEPLAY_TIME_MODE".to_owned()])
        );
        assert_eq!(bridge.take_call(), None);
    }
}
