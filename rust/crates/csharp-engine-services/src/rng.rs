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
