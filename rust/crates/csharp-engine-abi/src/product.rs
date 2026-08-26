use crate::*;
use std::ffi::c_void;
pub type NativeIntegrateLook =
    unsafe extern "C" fn(*mut c_void, NativeLookRequest, *mut NativeLookReceipt) -> i32;
pub type NativeCreateSpatialSession = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialSessionConfig,
    *mut NativeSpatialSessionHandle,
) -> i32;
pub type NativeDestroySpatialSession =
    unsafe extern "C" fn(*mut c_void, NativeSpatialSessionHandle) -> i32;
pub type NativeReplaceCollision = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCollisionReplaceRequest,
    *mut NativeCollisionReplaceReceipt,
) -> i32;
pub type NativeReplaceNavigation = unsafe extern "C" fn(
    *mut c_void,
    *const NativeNavigationReplaceRequest,
    *mut NativeNavigationReplaceReceipt,
) -> i32;
pub type NativeProposeCharacterStep = unsafe extern "C" fn(
    *mut c_void,
    NativeCharacterStepRequest,
    *mut NativeCharacterStepReceipt,
) -> i32;
pub type NativeProposeNavigationStep = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationStepRequest,
    *mut NativeNavigationStepReceipt,
) -> i32;
pub type NativeOpenRenderResource = unsafe extern "C" fn(
    *mut c_void,
    *const NativeRenderResourceRequest,
    *mut NativeRenderResourceInfo,
) -> i32;
pub type NativeCreatePrimitiveAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativePrimitiveAppearanceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeCreateStaticMeshAppearance = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStaticMeshAppearanceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeCreateStaticMeshContentAppearance = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStaticMeshContentAppearanceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeCreateSpriteAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativeSpriteAppearanceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativePublishAppearanceSnapshot =
    unsafe extern "C" fn(*mut c_void, *const NativeAppearanceFact, usize) -> i32;
pub type NativeOpenUiStream = unsafe extern "C" fn(
    *mut c_void,
    *const NativeUiStreamRequest,
    *mut NativeUiStreamHandle,
) -> i32;
pub type NativePublishUiProjection =
    unsafe extern "C" fn(*mut c_void, *const NativeUiProjection) -> i32;
pub type NativeDrawKeyedRng = unsafe extern "C" fn(
    *mut c_void,
    *const NativeKeyedRngRequest,
    *mut NativeKeyedRngReceipt,
) -> i32;
pub type NativeCreateScopedRng = unsafe extern "C" fn(
    *mut c_void,
    *const NativeScopedRngCreateRequest,
    *mut NativeRngHandle,
) -> i32;
pub type NativeForkScopedRng = unsafe extern "C" fn(
    *mut c_void,
    *const NativeScopedRngForkRequest,
    *mut NativeRngHandle,
) -> i32;
pub type NativeDestroyScopedRng = unsafe extern "C" fn(*mut c_void, NativeRngHandle) -> i32;
pub type NativeNextScopedRng =
    unsafe extern "C" fn(*mut c_void, NativeRngHandle, *mut NativeRngValue) -> i32;
pub type NativeNextBoundedScopedRng =
    unsafe extern "C" fn(*mut c_void, NativeScopedRngBoundedRequest, *mut NativeRngValue) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeLookApi {
    pub context: *mut c_void,
    pub integrate: NativeIntegrateLook,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialApi {
    pub context: *mut c_void,
    pub create_session: NativeCreateSpatialSession,
    pub destroy_session: NativeDestroySpatialSession,
    pub replace_collision: NativeReplaceCollision,
    pub replace_navigation: NativeReplaceNavigation,
    pub propose_character_step: NativeProposeCharacterStep,
    pub propose_navigation_step: NativeProposeNavigationStep,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeUiApi {
    pub context: *mut c_void,
    pub open_stream: NativeOpenUiStream,
    pub publish_projection: NativePublishUiProjection,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAppearanceApi {
    pub context: *mut c_void,
    pub open_resource: NativeOpenRenderResource,
    pub create_primitive: NativeCreatePrimitiveAppearance,
    pub create_static_mesh: NativeCreateStaticMeshAppearance,
    pub create_static_mesh_from_content: NativeCreateStaticMeshContentAppearance,
    pub create_sprite: NativeCreateSpriteAppearance,
    pub publish_snapshot: NativePublishAppearanceSnapshot,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRngApi {
    pub context: *mut c_void,
    pub draw_keyed: NativeDrawKeyedRng,
    pub create_scoped: NativeCreateScopedRng,
    pub fork_scoped: NativeForkScopedRng,
    pub destroy_scoped: NativeDestroyScopedRng,
    pub next_u64: NativeNextScopedRng,
    pub next_bounded_u32: NativeNextBoundedScopedRng,
    pub next_bool: NativeNextScopedRng,
}

/// Direct named Engine service families available to trusted NativeAOT code.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeEngineApi {
    pub look: NativeLookApi,
    pub spatial: NativeSpatialApi,
    pub appearance: NativeAppearanceApi,
    pub rng: NativeRngApi,
    pub ui: NativeUiApi,
}

/// Borrowed creation inputs plus the direct Engine API.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeProductCreateArgs {
    pub content: *const NativeContentFile,
    pub content_len: usize,
    pub engine: NativeEngineApi,
}

pub type NativeProductCreate =
    unsafe extern "C" fn(*const NativeProductCreateArgs, *mut *mut c_void) -> i32;
pub type NativeProductAction = unsafe extern "C" fn(*mut c_void) -> i32;
pub type NativeProductTurn = unsafe extern "C" fn(*mut c_void, *const NativeTurnArgs) -> i32;
pub type NativeProductDestroy = unsafe extern "C" fn(*mut c_void);

/// Product functions supplied to Rust by the one NativeAOT bootstrap export.
/// Nullable fields let Rust receive and inspect an initially empty table safely.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeProductApi {
    pub create:
        Option<unsafe extern "C" fn(*const NativeProductCreateArgs, *mut *mut c_void) -> i32>,
    pub start: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub turn: Option<unsafe extern "C" fn(*mut c_void, *const NativeTurnArgs) -> i32>,
    pub pause: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub resume: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub shutdown: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

pub type NativeProductBind = unsafe extern "C" fn(*mut NativeProductApi) -> i32;
