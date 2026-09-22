use std::{collections::BTreeMap, ffi::c_void};

use csharp_engine_abi::*;
use svc_rng::{KeyedRngV1, RngSeed, ScopedRng};

use crate::{composition::borrowed_utf8, composition::ABI_OK};

pub(crate) struct RuntimeRngBridge {
    streams: BTreeMap<u64, ScopedRng>,
    next_stream: u64,
}

impl RuntimeRngBridge {
    pub(crate) fn new() -> Self {
        Self {
            streams: BTreeMap::new(),
            next_stream: 1,
        }
    }

    fn insert(&mut self, stream: ScopedRng) -> Option<NativeRngHandle> {
        let handle = self.next_stream;
        self.next_stream = handle.checked_add(1)?;
        self.streams.insert(handle, stream);
        Some(NativeRngHandle { value: handle })
    }

    fn stream_mut(&mut self, handle: NativeRngHandle) -> Option<&mut ScopedRng> {
        self.streams.get_mut(&handle.value)
    }
}

pub(crate) fn api(bridge: &mut RuntimeRngBridge) -> NativeRngApi {
    NativeRngApi {
        context: (bridge as *mut RuntimeRngBridge).cast(),
        draw_keyed: draw_keyed_rng,
        draw_lcg15: draw_lcg15,
        create_scoped: create_scoped_rng,
        fork_scoped: fork_scoped_rng,
        destroy_scoped: destroy_scoped_rng,
        next_u64: next_scoped_rng_u64,
        next_bounded_u32: next_scoped_rng_bounded,
        next_bool: next_scoped_rng_bool,
    }
}

unsafe extern "C" fn draw_keyed_rng(
    _context: *mut c_void,
    request: *const NativeKeyedRngRequest,
    receipt: *mut NativeKeyedRngReceipt,
) -> i32 {
    if request.is_null() || receipt.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let scope = match unsafe { borrowed_utf8(request.scope.bytes, request.scope.len, "RNG scope") }
    {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let key = match unsafe { borrowed_utf8(request.key.bytes, request.key.len, "RNG key") } {
        Ok(value) => value,
        Err(_) => return 0,
    };
    match KeyedRngV1::draw_i64_inclusive(
        RngSeed::new(request.seed),
        scope,
        key.as_bytes(),
        request.minimum,
        request.maximum,
    ) {
        Ok(value) => {
            unsafe { *receipt = NativeKeyedRngReceipt { value } };
            ABI_OK
        }
        Err(_) => 0,
    }
}

unsafe extern "C" fn draw_lcg15(
    _context: *mut c_void,
    request: NativeLcg15Request,
    receipt: *mut NativeLcg15Receipt,
) -> i32 {
    if receipt.is_null() || request.upper_exclusive == 0 {
        return 0;
    }

    let state = request
        .state
        .wrapping_mul(1_103_515_245)
        .wrapping_add(12_345);
    let sample = (state >> 16) & 0x7fff;
    unsafe {
        *receipt = NativeLcg15Receipt {
            state,
            value: sample % request.upper_exclusive,
        }
    };
    ABI_OK
}

unsafe extern "C" fn create_scoped_rng(
    context: *mut c_void,
    request: *const NativeScopedRngCreateRequest,
    result: *mut NativeRngHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let scope = match unsafe { borrowed_utf8(request.scope.bytes, request.scope.len, "RNG scope") }
    {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeRngBridge>() };
    match bridge.insert(ScopedRng::new(RngSeed::new(request.seed), scope)) {
        Some(handle) => {
            unsafe { *result = handle };
            ABI_OK
        }
        None => 0,
    }
}

unsafe extern "C" fn fork_scoped_rng(
    context: *mut c_void,
    request: *const NativeScopedRngForkRequest,
    result: *mut NativeRngHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let request = unsafe { &*request };
    let scope = match unsafe { borrowed_utf8(request.scope.bytes, request.scope.len, "RNG scope") }
    {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let bridge = unsafe { &mut *context.cast::<RuntimeRngBridge>() };
    let Some(child) = bridge
        .streams
        .get(&request.parent.value)
        .map(|parent| parent.fork(scope))
    else {
        return 0;
    };
    match bridge.insert(child) {
        Some(handle) => {
            unsafe { *result = handle };
            ABI_OK
        }
        None => 0,
    }
}

unsafe extern "C" fn destroy_scoped_rng(context: *mut c_void, handle: NativeRngHandle) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeRngBridge>() };
    if bridge.streams.remove(&handle.value).is_some() {
        ABI_OK
    } else {
        0
    }
}

unsafe extern "C" fn next_scoped_rng_u64(
    context: *mut c_void,
    handle: NativeRngHandle,
    result: *mut NativeRngValue,
) -> i32 {
    next_rng_value(context, result, |bridge| {
        bridge.stream_mut(handle).map(ScopedRng::next_u64)
    })
}

unsafe extern "C" fn next_scoped_rng_bounded(
    context: *mut c_void,
    request: NativeScopedRngBoundedRequest,
    result: *mut NativeRngValue,
) -> i32 {
    next_rng_value(context, result, |bridge| {
        bridge
            .stream_mut(request.stream)?
            .next_bounded_u32(request.upper)
            .map(u64::from)
    })
}

unsafe extern "C" fn next_scoped_rng_bool(
    context: *mut c_void,
    handle: NativeRngHandle,
    result: *mut NativeRngValue,
) -> i32 {
    next_rng_value(context, result, |bridge| {
        bridge
            .stream_mut(handle)
            .map(|stream| u64::from(stream.next_bool()))
    })
}

fn next_rng_value(
    context: *mut c_void,
    result: *mut NativeRngValue,
    action: impl FnOnce(&mut RuntimeRngBridge) -> Option<u64>,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeRngBridge>() };
    match action(bridge) {
        Some(value) => {
            unsafe { *result = NativeRngValue { value } };
            ABI_OK
        }
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(api: &NativeRngApi, state: u32, upper_exclusive: u32) -> NativeLcg15Receipt {
        let mut receipt = NativeLcg15Receipt::default();
        assert_eq!(
            unsafe {
                (api.draw_lcg15)(
                    api.context,
                    NativeLcg15Request {
                        state,
                        upper_exclusive,
                    },
                    &mut receipt,
                )
            },
            ABI_OK
        );
        receipt
    }

    #[test]
    fn lcg15_preserves_the_known_32_bit_compatibility_sequence() {
        let mut bridge = RuntimeRngBridge::new();
        let api = api(&mut bridge);
        let expected = [
            NativeLcg15Receipt {
                state: 1_103_527_590,
                value: 57,
            },
            NativeLcg15Receipt {
                state: 2_524_885_223,
                value: 35,
            },
            NativeLcg15Receipt {
                state: 662_824_084,
                value: 25,
            },
        ];

        let mut state = 1;
        for expected in expected {
            let actual = draw(&api, state, 97);
            assert_eq!(actual.state, expected.state);
            assert_eq!(actual.value, expected.value);
            state = actual.state;
        }
    }

    #[test]
    fn lcg15_wraps_32_bit_state_and_honors_bounds() {
        let mut bridge = RuntimeRngBridge::new();
        let api = api(&mut bridge);
        assert_eq!(
            draw(&api, u32::MAX, 1),
            NativeLcg15Receipt {
                state: 3_191_464_396,
                value: 0,
            }
        );

        let mut receipt = NativeLcg15Receipt::default();
        assert_ne!(
            unsafe {
                (api.draw_lcg15)(
                    api.context,
                    NativeLcg15Request {
                        state: 1,
                        upper_exclusive: 0,
                    },
                    &mut receipt,
                )
            },
            ABI_OK
        );
    }

    #[test]
    fn lcg15_interleavings_are_caller_state_only() {
        let mut bridge = RuntimeRngBridge::new();
        let api = api(&mut bridge);
        let first_a = draw(&api, 0, 97);
        let first_b = draw(&api, u32::MAX, 97);
        let second_a = draw(&api, first_a.state, 97);
        let second_b = draw(&api, first_b.state, 97);

        assert_eq!(
            first_a,
            NativeLcg15Receipt {
                state: 12_345,
                value: 0
            }
        );
        assert_eq!(
            second_a,
            NativeLcg15Receipt {
                state: 3_554_416_254,
                value: 31
            }
        );
        assert_eq!(
            first_b,
            NativeLcg15Receipt {
                state: 3_191_464_396,
                value: 21
            }
        );
        assert_eq!(
            second_b,
            NativeLcg15Receipt {
                state: 288_979_989,
                value: 44
            }
        );
    }
}
