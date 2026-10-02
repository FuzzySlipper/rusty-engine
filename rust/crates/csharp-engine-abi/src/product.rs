pub type NativeConfigureDynamicsRopes = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsRopeSolverRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
use crate::*;
use std::ffi::c_void;
pub type NativeSetDynamicsFixedTether = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsFixedTetherRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetDynamicsBodyTether = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsBodyTetherRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRemoveDynamicsTether = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsTetherRequest,
    *mut NativeDynamicsTetherReleaseReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadDynamicsTether = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsTetherRequest,
    *mut NativeDynamicsTetherReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateDynamicsWorld = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsWorldConfig,
    *mut NativeDynamicsWorldHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyDynamicsWorld = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsWorldHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateDynamicsBody = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsCreateBodyRequest,
    *mut NativeDynamicsBodyHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateDynamicsSphereBody = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsCreateSphereBodyRequest,
    *mut NativeDynamicsBodyHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateDynamicsCuboidBody = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsCreateCuboidBodyRequest,
    *mut NativeDynamicsBodyHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateDynamicsSphereBodyWithProperties = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsCreateSphereBodyPropertiesRequest,
    *mut NativeDynamicsBodyHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateDynamicsCapsuleBody = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsCreateCapsuleBodyRequest,
    *mut NativeDynamicsBodyHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeBindDynamicsWorldCollision = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsWorldCollisionBindingRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRebaseDynamicsWorldOrigin = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsRebaseWorldOriginRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyDynamicsBody = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsBodyHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeStepDynamics = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsStepRequest,
    *mut NativeDynamicsStepReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeStepAndReadDynamics = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsStepAndReadRequest,
    *mut NativeDynamicsStepAndReadResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadDynamics = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsReadRequest,
    *mut NativeDynamicsReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeResetDynamics = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsResetRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeUpdateDynamicsBody = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsUpdateBodyRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadDynamicsWorld = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsWorldReadRequest,
    *mut NativeDynamicsWorldResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceDynamicsBody = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsReplaceBodyRequest,
    *mut NativeDynamicsBodyHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceDynamicsCuboidBody = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsReplaceCuboidBodyRequest,
    *mut NativeDynamicsBodyHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceDynamicsSphereBody = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsReplaceSphereBodyRequest,
    *mut NativeDynamicsBodyHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceDynamicsCapsuleBody = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsReplaceCapsuleBodyRequest,
    *mut NativeDynamicsBodyHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateSpatialSession = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialSessionConfig,
    *mut NativeSpatialSessionHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroySpatialSession =
    unsafe extern "C" fn(*mut c_void, NativeSpatialSessionHandle) -> i32;
pub type NativeMotionResolve = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMotionResolveRequest,
    *mut NativeMotionResolveReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceCollision = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCollisionReplaceRequest,
    *mut NativeCollisionReplaceReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeApplyCollisionResidency = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCollisionResidencyRequest,
    *mut NativeCollisionReplaceReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceSpatialContentArtifact = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialContentArtifactReplaceRequest,
    *mut NativeSpatialContentArtifactReplaceReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadSpatialContentArtifact = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialContentArtifactReadRequest,
    *mut NativeSpatialContentArtifactReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceNavigation = unsafe extern "C" fn(
    *mut c_void,
    *const NativeNavigationReplaceRequest,
    *mut NativeNavigationReplaceReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceVoxelNavigation = unsafe extern "C" fn(
    *mut c_void,
    *const NativeNavigationVoxelReplaceRequest,
    *mut NativeNavigationReplaceReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceCollisionNavigation = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCollisionNavigationReplaceRequest,
    *mut NativeCollisionNavigationReplaceReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefaultCollisionNavigationConfig =
    unsafe extern "C" fn(*mut c_void, *mut NativeCollisionNavigationConfig) -> i32;
pub type NativeExplainCollisionNavigationColumn = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCollisionNavigationColumnRequest,
    *mut NativeCollisionNavigationColumnResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeExplainCollisionNavigationEdge = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCollisionNavigationEdgeRequest,
    *mut NativeCollisionNavigationEdgeReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceNavigationTraversal = unsafe extern "C" fn(
    *mut c_void,
    *const NativeNavigationTraversalReplaceRequest,
    *mut NativeNavigationTraversalReplaceReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeClearNavigationTraversal = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationTraversalClearRequest,
    *mut NativeNavigationTraversalReplaceReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceVolumetricNavigationTraversal = unsafe extern "C" fn(
    *mut c_void,
    *const NativeNavigationVolumetricTraversalReplaceRequest,
    *mut NativeNavigationVolumetricTraversalReplaceReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeClearVolumetricNavigationTraversal = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationVolumetricTraversalClearRequest,
    *mut NativeNavigationVolumetricTraversalReplaceReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadSpatialMap = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialMapRequest,
    *mut NativeSpatialMapResult,
) -> i32;
pub type NativeReadNavigationProjection = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationProjectionReadRequest,
    *mut NativeNavigationProjectionReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRequestNavigationPath = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationPathRequest,
    *mut NativeNavigationPathResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRequestWeightedNavigationPath = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationWeightedPathRequest,
    *mut NativeNavigationWeightedPathResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRequestWeightedVolumetricNavigationPath = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationVolumetricWeightedPathRequest,
    *mut NativeNavigationVolumetricWeightedPathResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRequestVolumetricNavigationPath = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationVolumetricPathRequest,
    *mut NativeNavigationPathResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeClearNavigation = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationClearRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefaultCharacterControllerConfig =
    unsafe extern "C" fn(*mut c_void, *mut NativeCharacterControllerConfig) -> i32;
pub type NativeValidateCharacterControllerConfig = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCharacterControllerConfig,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeValidateCharacterControllerCommand = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCharacterControllerValidationRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeProposeCharacterStep = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCharacterStepRequest,
    *mut NativeCharacterStepReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCaptureCharacterContinuation = unsafe extern "C" fn(
    *mut c_void,
    NativeCharacterContinuationCaptureRequest,
    *mut NativeCharacterContinuationCheckpoint,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRestoreCharacterContinuation = unsafe extern "C" fn(
    *mut c_void,
    NativeCharacterContinuationRestoreRequest,
    *mut NativeCharacterContinuationRestoreReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadCharacterController = unsafe extern "C" fn(
    *mut c_void,
    NativeCharacterControllerReadRequest,
    *mut NativeCharacterControllerResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeEvaluateNavigationStep = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationStepRequest,
    *mut NativeNavigationStepResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadSpatialProjection = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialProjectionReadRequest,
    *mut NativeSpatialProjectionReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSpatialContainsPoint = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialContainsPointRequest,
    *mut NativeSpatialQueryReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSpatialRaycast = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialRaycastRequest,
    *mut NativeSpatialHit,
) -> i32;
pub type NativeQueryPerception = unsafe extern "C" fn(
    *mut c_void,
    *const NativePerceptionQueryRequest,
    *mut NativePerceptionReadoutResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSpatialSegmentCast = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialSegmentCastRequest,
    *mut NativeSpatialHit,
) -> i32;
pub type NativeSpatialOverlapAabb = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialAabbQueryRequest,
    *mut NativeSpatialQueryReceipt,
) -> i32;
pub type NativeSpatialSweepAabb = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialAabbQueryRequest,
    *mut NativeSpatialQueryReceipt,
) -> i32;
pub type NativeSpatialCastCapsule = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialCapsuleQueryRequest,
    *mut NativeSpatialHit,
) -> i32;
pub type NativeSpatialOverlapCapsule = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialCapsuleQueryRequest,
    *mut NativeSpatialHit,
) -> i32;
pub type NativeSpatialPickVoxel = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialPickRequest,
    *mut NativeSpatialHit,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSpatialRegisterTrigger = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialTriggerRegisterRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSpatialReconcileTriggers = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialTriggerReconcileRequest,
    *mut NativeSpatialTriggerReconcileResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSpatialSetTriggerActive = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialTriggerSetActiveRequest,
    *mut NativeSpatialTriggerLifecycleResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSpatialRestoreTriggers = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialTriggerRestoreRequest,
    *mut NativeSpatialTriggerRestoreReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSpatialReadTrigger = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialTriggerReadRequest,
    *mut NativeSpatialTriggerReadResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeWorldOriginPrepare = unsafe extern "C" fn(
    *mut c_void,
    *const NativeWorldOriginPrepareRequest,
    *mut NativeWorldOriginPreparedHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeWorldOriginRead = unsafe extern "C" fn(
    *mut c_void,
    NativeWorldOriginReadRequest,
    *mut NativeWorldOriginReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeWorldOriginReadPrepared = unsafe extern "C" fn(
    *mut c_void,
    NativeWorldOriginPreparedReadRequest,
    *mut NativeWorldOriginPreparedResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeWorldOriginCommit = unsafe extern "C" fn(
    *mut c_void,
    NativeWorldOriginCommitRequest,
    *mut NativeWorldOriginCommitReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyWorldOriginPrepared =
    unsafe extern "C" fn(*mut c_void, NativeWorldOriginPreparedHandle) -> i32;
pub type NativeOpenRenderResource = unsafe extern "C" fn(
    *mut c_void,
    *const NativeRenderResourceRequest,
    *mut NativeRenderResourceInfo,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeOpenAudioClip = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioClipRequest,
    *mut NativeAudioClipHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativePlayVideo = unsafe extern "C" fn(
    *mut c_void,
    *const NativePlayVideoRequest,
    *mut NativeVideoPlaybackHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativePlayVideoFromContent = unsafe extern "C" fn(
    *mut c_void,
    *const NativePlayVideoFromContentRequest,
    *mut NativeVideoPlaybackHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeStopVideo = unsafe extern "C" fn(
    *mut c_void,
    NativeVideoPlaybackHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSkipVideo = unsafe extern "C" fn(
    *mut c_void,
    NativeVideoPlaybackHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadVideo = unsafe extern "C" fn(
    *mut c_void,
    *mut NativeVideoReadout,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadVideoRealization = unsafe extern "C" fn(
    *mut c_void,
    *mut NativeVideoRealizationResult,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeOpenAudioClipFromContent = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioClipFromContentRequest,
    *mut NativeAudioClipHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAudioClip = unsafe extern "C" fn(
    *mut c_void,
    NativeAudioClipHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativePreloadOptionalAudioClip = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioClipRequest,
    *mut NativeAudioOptionalPreloadReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeEmitAudio = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioEmitRequest,
    *mut NativeAudioSignalHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateAudioVoice = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioSourceDescriptor,
    *mut NativeAudioVoiceHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeUpdateAudioVoice = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioVoiceUpdateRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceAudioVoice = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioVoiceReplaceRequest,
    *mut NativeAudioVoiceHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAudioVoice = unsafe extern "C" fn(
    *mut c_void,
    NativeAudioVoiceHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeControlAudioVoice = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioVoiceControlRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetAudioBusVolume = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioBusVolumeRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetAudioBusMuted = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioBusMutedRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadAudio = unsafe extern "C" fn(
    *mut c_void,
    *mut NativeAudioResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadAudioVoice = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioVoiceReadRequest,
    *mut NativeAudioVoiceReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadAudioBus = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioBusReadRequest,
    *mut NativeAudioBusReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadAudioRealization = unsafe extern "C" fn(
    *mut c_void,
    *mut NativeAudioRealizationResult,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateMaterial = unsafe extern "C" fn(
    *mut c_void,
    NativeMaterialRequest,
    *mut NativeMaterialHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateAuthoredMaterial = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAuthoredMaterialAppearanceRequest,
    *mut NativeMaterialHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeUpdateMaterial = unsafe extern "C" fn(
    *mut c_void,
    NativeMaterialUpdateRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceMaterial = unsafe extern "C" fn(
    *mut c_void,
    NativeMaterialUpdateRequest,
    *mut NativeMaterialHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMaterial = unsafe extern "C" fn(
    *mut c_void,
    NativeMaterialHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreatePrimitiveAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativePrimitiveAppearanceRequest,
    *mut NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplacePrimitiveAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativePrimitiveAppearanceReplaceRequest,
    *mut NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateMeshResource = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMeshResourceCreateRequest,
    *mut NativeMeshResourceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMeshResource = unsafe extern "C" fn(
    *mut c_void,
    NativeMeshResourceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateMeshAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativeMeshResourceHandle,
    *mut NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativePartitionMesh = unsafe extern "C" fn(
    *mut c_void,
    NativeMeshPartitionRequest,
    *mut NativeMeshPartitionHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMeshPartition = unsafe extern "C" fn(
    *mut c_void,
    NativeMeshPartitionHandle,
    *mut NativeMeshPartitionReadout,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeTakeMeshPartitionPart = unsafe extern "C" fn(
    *mut c_void,
    NativeMeshPartitionPartRequest,
    *mut NativeMeshResourceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMeshPartition = unsafe extern "C" fn(
    *mut c_void,
    NativeMeshPartitionHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;

pub type NativeCreateStaticMeshAppearance = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStaticMeshAppearanceRequest,
    *mut NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateStaticMeshContentAppearance = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStaticMeshContentAppearanceRequest,
    *mut NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceStaticMeshAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativeAppearanceHandle,
    *const NativeStaticMeshAppearanceRequest,
    *mut NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceStaticMeshContentAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativeAppearanceHandle,
    *const NativeStaticMeshContentAppearanceRequest,
    *mut NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeUpdateStaticMeshMaterials = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStaticMeshMaterialUpdateRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateSpriteAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativeSpriteAppearanceRequest,
    *mut NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceSpriteAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativeSpriteAppearanceReplaceRequest,
    *mut NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateSpriteAtlas = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpriteAtlasCreateRequest,
    *mut NativeSpriteAtlasHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroySpriteAtlas = unsafe extern "C" fn(
    *mut c_void,
    NativeSpriteAtlasHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateSpriteFromAtlas = unsafe extern "C" fn(
    *mut c_void,
    NativeSpriteFromAtlasRequest,
    *mut NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceSpriteFromAtlas = unsafe extern "C" fn(
    *mut c_void,
    NativeSpriteFromAtlasReplaceRequest,
    *mut NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetSpriteFrame = unsafe extern "C" fn(
    *mut c_void,
    NativeSpriteFrameUpdateRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetSpriteViewport = unsafe extern "C" fn(
    *mut c_void,
    NativeSpriteViewportUpdateRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadSprite = unsafe extern "C" fn(
    *mut c_void,
    NativeAppearanceHandle,
    *mut NativeSpriteReadout,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateSpritePlayback = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpritePlaybackCreateRequest,
    *mut NativeSpritePlaybackHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroySpritePlayback = unsafe extern "C" fn(
    *mut c_void,
    NativeSpritePlaybackHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeControlSpritePlayback = unsafe extern "C" fn(
    *mut c_void,
    NativeSpritePlaybackControlRequest,
    *mut NativeSpritePlaybackReadout,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSelectSpritePlaybackFrame = unsafe extern "C" fn(
    *mut c_void,
    NativeSpritePlaybackFrameSelectionRequest,
    *mut NativeSpritePlaybackReadout,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeAdvanceSpritePlayback = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpritePlaybackAdvanceRequest,
    *mut NativeSpritePlaybackAdvanceResult,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSampleSpritePlayback = unsafe extern "C" fn(
    *mut c_void,
    NativeSpritePlaybackSampleRequest,
    *mut NativeSpritePlaybackSample,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadSpritePlayback = unsafe extern "C" fn(
    *mut c_void,
    NativeSpritePlaybackHandle,
    *mut NativeSpritePlaybackReadout,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativePublishAppearanceSnapshot = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAppearanceFact,
    usize,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateLight = unsafe extern "C" fn(
    *mut c_void,
    NativeLightRequest,
    *mut NativeLightHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeUpdateLight = unsafe extern "C" fn(
    *mut c_void,
    NativeLightUpdateRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceLight = unsafe extern "C" fn(
    *mut c_void,
    NativeLightUpdateRequest,
    *mut NativeLightHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyLight = unsafe extern "C" fn(
    *mut c_void,
    NativeLightHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadLight = unsafe extern "C" fn(
    *mut c_void,
    NativeLightHandle,
    *mut NativeLightReadout,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadPresentation = unsafe extern "C" fn(
    *mut c_void,
    *mut NativePresentationReadout,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreatePresentationBillboard = unsafe extern "C" fn(
    *mut c_void,
    *const NativePresentationBillboardDescriptor,
    *mut NativePresentationBillboardHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeUpdatePresentationBillboard = unsafe extern "C" fn(
    *mut c_void,
    NativePresentationBillboardHandle,
    *const NativePresentationBillboardDescriptor,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreatePresentationStructuredBillboard = unsafe extern "C" fn(
    *mut c_void,
    *const NativePresentationStructuredBillboardDescriptor,
    *mut NativePresentationBillboardHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeUpdatePresentationStructuredBillboard = unsafe extern "C" fn(
    *mut c_void,
    NativePresentationBillboardHandle,
    *const NativePresentationStructuredBillboardDescriptor,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyPresentationBillboard = unsafe extern "C" fn(
    *mut c_void,
    NativePresentationBillboardHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeEmitPresentationParticles = unsafe extern "C" fn(
    *mut c_void,
    *const NativePresentationParticleDescriptor,
    *mut NativePresentationParticleEmissionReceipt,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreatePresentationEmitter = unsafe extern "C" fn(
    *mut c_void,
    *const NativePresentationParticleDescriptor,
    *mut NativePresentationEmitterHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeUpdatePresentationEmitter = unsafe extern "C" fn(
    *mut c_void,
    NativePresentationEmitterHandle,
    *const NativePresentationParticleDescriptor,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyPresentationEmitter = unsafe extern "C" fn(
    *mut c_void,
    NativePresentationEmitterHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadPresentationFacts = unsafe extern "C" fn(
    *mut c_void,
    *mut NativePresentationFactsResult,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeOpenAnimatedMesh = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimatedMeshResourceRequest,
    *mut NativeRenderResourceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeOpenAnimationClipPack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationClipPackResourceRequest,
    *mut NativeRenderResourceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeAssociateAnimationClipPack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationClipPackAssociationRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateAnimatedMeshAppearance = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimatedMeshAppearanceRequest,
    *mut NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceAnimatedMeshAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativeAppearanceHandle,
    *const NativeAnimatedMeshAppearanceRequest,
    *mut NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetAnimatedMeshInspection = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimatedMeshInspectionRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeUpdateAnimatedMeshMaterials = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimatedMeshMaterialUpdateRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateAnimationInstance = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationInstanceRequest,
    *mut NativeAnimationInstanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAnimationInstance = unsafe extern "C" fn(
    *mut c_void,
    NativeAnimationInstanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceAnimationInstance = unsafe extern "C" fn(
    *mut c_void,
    NativeAnimationInstanceHandle,
    *const NativeAnimationInstanceRequest,
    *mut NativeAnimationInstanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetAnimationPlayback = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationPlaybackRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateAnimationGraph = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationGraphCreateRequest,
    *mut NativeAnimationGraphHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAnimationGraph = unsafe extern "C" fn(
    *mut c_void,
    NativeAnimationGraphHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefineAnimationParameter = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationParameterDefinitionRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefineAnimationState = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationStateDefinitionRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefineAnimationTransition = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationTransitionDefinitionRequest,
    *mut NativeAnimationTransitionHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefineAnimationCondition = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationConditionDefinitionRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateAnimationController = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationControllerCreateRequest,
    *mut NativeAnimationControllerHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyAnimationController = unsafe extern "C" fn(
    *mut c_void,
    NativeAnimationControllerHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetAnimationFloat = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationSetFloatRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetAnimationBool = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationSetBoolRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeFireAnimationTrigger = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationFireTriggerRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeTickAnimation = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationTickRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadAnimationController = unsafe extern "C" fn(
    *mut c_void,
    NativeAnimationControllerHandle,
    *mut NativeAnimationControllerReadout,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadAnimation = unsafe extern "C" fn(
    *mut c_void,
    *mut NativeAnimationReadout,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadAnimationRealization = unsafe extern "C" fn(
    *mut c_void,
    *mut NativeAnimationRealizationResult,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateCamera = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCameraDescriptor,
    *mut NativeCameraHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeUpdateCamera = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCameraUpdateRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeUpdateCameraSample = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCameraSampleRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceCamera = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCameraReplaceRequest,
    *mut NativeCameraHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyCamera = unsafe extern "C" fn(
    *mut c_void,
    NativeCameraHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateCameraTarget = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCameraTargetDescriptor,
    *mut NativeCameraTargetHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeUpdateCameraTarget = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCameraTargetUpdateRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceCameraTarget = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCameraTargetReplaceRequest,
    *mut NativeCameraTargetHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyCameraTarget = unsafe extern "C" fn(
    *mut c_void,
    NativeCameraTargetHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetCameraComposition = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCameraCompositionRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetActiveCamera = unsafe extern "C" fn(
    *mut c_void,
    NativeCameraHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeClearActiveCamera = unsafe extern "C" fn(
    *mut c_void,
    *const NativeClearActiveCameraRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetSkyBackground = unsafe extern "C" fn(
    *mut c_void,
    NativeRenderResourceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetSkyBackgroundBlend = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSkyBackgroundBlendRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetFog = unsafe extern "C" fn(
    *mut c_void,
    *const NativeFogRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetToneMapping = unsafe extern "C" fn(
    *mut c_void,
    *const NativeToneMappingRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeClearSkyBackground = unsafe extern "C" fn(
    *mut c_void,
    *const NativeClearSkyBackgroundRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetBackgroundColor = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSetBackgroundColorRequest,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeOpenUiStream = unsafe extern "C" fn(
    *mut c_void,
    *const NativeUiStreamRequest,
    *mut NativeUiStreamHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyUiStream = unsafe extern "C" fn(
    *mut c_void,
    NativeUiStreamHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeOpenUiImage = unsafe extern "C" fn(
    *mut c_void,
    *const NativeUiImageRequest,
    *mut NativeUiImageHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyUiImage = unsafe extern "C" fn(
    *mut c_void,
    NativeUiImageHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativePublishUiProjection = unsafe extern "C" fn(
    *mut c_void,
    *const NativeUiProjection,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDrawKeyedRng = unsafe extern "C" fn(
    *mut c_void,
    *const NativeKeyedRngRequest,
    *mut NativeKeyedRngReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDrawLcg15 =
    unsafe extern "C" fn(*mut c_void, NativeLcg15Request, *mut NativeLcg15Receipt) -> i32;
pub type NativeCreateScopedRng = unsafe extern "C" fn(
    *mut c_void,
    *const NativeScopedRngCreateRequest,
    *mut NativeRngHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeForkScopedRng = unsafe extern "C" fn(
    *mut c_void,
    *const NativeScopedRngForkRequest,
    *mut NativeRngHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyScopedRng = unsafe extern "C" fn(*mut c_void, NativeRngHandle) -> i32;
pub type NativeNextScopedRng =
    unsafe extern "C" fn(*mut c_void, NativeRngHandle, *mut NativeRngValue) -> i32;
pub type NativeNextBoundedScopedRng =
    unsafe extern "C" fn(*mut c_void, NativeScopedRngBoundedRequest, *mut NativeRngValue) -> i32;
pub type NativeOpenPersistenceStore = unsafe extern "C" fn(
    *mut c_void,
    *const NativePersistenceOpenRequest,
    *mut NativePersistenceStoreHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyPersistenceStore =
    unsafe extern "C" fn(*mut c_void, NativePersistenceStoreHandle) -> i32;
pub type NativeSavePersistence = unsafe extern "C" fn(
    *mut c_void,
    *const NativePersistenceSaveRequest,
    *mut NativePersistenceSaveReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeLoadPersistence = unsafe extern "C" fn(
    *mut c_void,
    *const NativePersistenceLoadRequest,
    *mut NativePersistenceBlobHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyPersistenceBlob =
    unsafe extern "C" fn(*mut c_void, NativePersistenceBlobHandle) -> i32;
pub type NativeDescribePersistenceBlob = unsafe extern "C" fn(
    *mut c_void,
    NativePersistenceBlobHandle,
    *mut NativePersistenceBlobInfo,
) -> i32;
pub type NativeCopyPersistenceBlob =
    unsafe extern "C" fn(*mut c_void, *const NativePersistenceCopyBlobRequest) -> i32;

pub type NativeSetDynamicsChainLength = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsChainLengthRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateDynamicsFixedChain = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsFixedChainRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateDynamicsBodyChain = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsBodyChainRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadDynamicsChain = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsChainRequest,
    *mut NativeDynamicsChainReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadDynamicsChainPoint = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsChainPointRequest,
    *mut NativeDynamicsChainPointReadout,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRemoveDynamicsChain = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsChainRequest,
    *mut NativeDynamicsChainReleaseReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsApi {
    pub context: *mut c_void,
    pub observe_anchor: NativeObserveDynamicsAnchor,
    pub step_with_reactions: NativeStepDynamicsWithReactions,
    pub configure_ropes: NativeConfigureDynamicsRopes,
    pub set_chain_length: NativeSetDynamicsChainLength,
    pub create_fixed_chain: NativeCreateDynamicsFixedChain,
    pub create_body_chain: NativeCreateDynamicsBodyChain,
    pub read_chain: NativeReadDynamicsChain,
    pub read_chain_point: NativeReadDynamicsChainPoint,
    pub remove_chain: NativeRemoveDynamicsChain,
    pub set_fixed_tether: NativeSetDynamicsFixedTether,
    pub set_body_tether: NativeSetDynamicsBodyTether,
    pub remove_tether: NativeRemoveDynamicsTether,
    pub read_tether: NativeReadDynamicsTether,
    pub create_world: NativeCreateDynamicsWorld,
    pub destroy_world: NativeDestroyDynamicsWorld,
    pub create_body: NativeCreateDynamicsBody,
    pub create_sphere_body: NativeCreateDynamicsSphereBody,
    pub create_cuboid_body: NativeCreateDynamicsCuboidBody,
    pub create_sphere_body_with_properties: NativeCreateDynamicsSphereBodyWithProperties,
    pub create_capsule_body: NativeCreateDynamicsCapsuleBody,
    pub bind_world_collision: NativeBindDynamicsWorldCollision,
    pub rebase_world_origin: NativeRebaseDynamicsWorldOrigin,
    pub destroy_body: NativeDestroyDynamicsBody,
    pub step: NativeStepDynamics,
    pub step_and_read: NativeStepAndReadDynamics,
    pub read: NativeReadDynamics,
    pub reset: NativeResetDynamics,
    pub update_body: NativeUpdateDynamicsBody,
    pub read_world: NativeReadDynamicsWorld,
    pub replace_body: NativeReplaceDynamicsBody,
    pub replace_cuboid_body: NativeReplaceDynamicsCuboidBody,
    pub replace_sphere_body: NativeReplaceDynamicsSphereBody,
    pub replace_capsule_body: NativeReplaceDynamicsCapsuleBody,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeMotionApi {
    pub context: *mut c_void,
    pub resolve: NativeMotionResolve,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeSpatialApi {
    pub context: *mut c_void,
    pub create_session: NativeCreateSpatialSession,
    pub destroy_session: NativeDestroySpatialSession,
    pub replace_collision: NativeReplaceCollision,
    pub apply_collision_residency: NativeApplyCollisionResidency,
    pub replace_content_artifact: NativeReplaceSpatialContentArtifact,
    pub read_content_artifact: NativeReadSpatialContentArtifact,
    pub replace_navigation: NativeReplaceNavigation,
    pub replace_voxel_navigation: NativeReplaceVoxelNavigation,
    pub replace_collision_navigation: NativeReplaceCollisionNavigation,
    pub default_collision_navigation_config: NativeDefaultCollisionNavigationConfig,
    pub explain_collision_navigation_column: NativeExplainCollisionNavigationColumn,
    pub explain_collision_navigation_edge: NativeExplainCollisionNavigationEdge,
    pub replace_navigation_traversal: NativeReplaceNavigationTraversal,
    pub clear_navigation_traversal: NativeClearNavigationTraversal,
    pub replace_volumetric_navigation_traversal: NativeReplaceVolumetricNavigationTraversal,
    pub clear_volumetric_navigation_traversal: NativeClearVolumetricNavigationTraversal,
    pub read_navigation_projection: NativeReadNavigationProjection,
    pub read_map: NativeReadSpatialMap,
    pub request_navigation_path: NativeRequestNavigationPath,
    pub request_weighted_navigation_path: NativeRequestWeightedNavigationPath,
    pub request_weighted_volumetric_navigation_path: NativeRequestWeightedVolumetricNavigationPath,
    pub request_volumetric_navigation_path: NativeRequestVolumetricNavigationPath,
    pub clear_navigation: NativeClearNavigation,
    pub default_character_controller_config: NativeDefaultCharacterControllerConfig,
    pub validate_character_controller_config: NativeValidateCharacterControllerConfig,
    pub validate_character_controller_command: NativeValidateCharacterControllerCommand,
    pub propose_character_step: NativeProposeCharacterStep,
    pub capture_character_continuation: NativeCaptureCharacterContinuation,
    pub restore_character_continuation: NativeRestoreCharacterContinuation,
    pub read_character_controller: NativeReadCharacterController,
    pub evaluate_navigation_step: NativeEvaluateNavigationStep,
    pub read_projection: NativeReadSpatialProjection,
    pub contains_point: NativeSpatialContainsPoint,
    pub cast_ray: NativeSpatialRaycast,
    pub cast_segment: NativeSpatialSegmentCast,
    pub overlap_aabb: NativeSpatialOverlapAabb,
    pub sweep_aabb: NativeSpatialSweepAabb,
    pub cast_capsule: NativeSpatialCastCapsule,
    pub overlap_capsule: NativeSpatialOverlapCapsule,
    pub pick_voxel: NativeSpatialPickVoxel,
    pub register_trigger: NativeSpatialRegisterTrigger,
    pub reconcile_triggers: NativeSpatialReconcileTriggers,
    pub set_trigger_active: NativeSpatialSetTriggerActive,
    pub restore_triggers: NativeSpatialRestoreTriggers,
    pub read_trigger: NativeSpatialReadTrigger,
}

/// Origin rebasing is a distinct named service family, but shares the Spatial
/// session context because origin and collision scene commit as one unit.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeWorldOriginApi {
    pub context: *mut c_void,
    pub prepare: NativeWorldOriginPrepare,
    pub read: NativeWorldOriginRead,
    pub read_prepared: NativeWorldOriginReadPrepared,
    pub commit: NativeWorldOriginCommit,
    pub destroy_prepared: NativeDestroyWorldOriginPrepared,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeUiApi {
    pub context: *mut c_void,
    pub open_stream: NativeOpenUiStream,
    pub destroy_stream: NativeDestroyUiStream,
    pub publish_projection: NativePublishUiProjection,
    pub open_image: NativeOpenUiImage,
    pub destroy_image: NativeDestroyUiImage,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeGraphicsApi {
    pub context: *mut c_void,
    pub open_resource: NativeOpenRenderResource,
    pub read_texture_info: NativeReadTextureResourceInfo,
    pub destroy_resource: NativeDestroyRenderResource,
    pub open_resource_from_content: NativeOpenRenderResourceFromContent,
    pub create_static_mesh_from_content_reference: NativeCreateStaticMeshFromContentReference,
    pub create_material: NativeCreateMaterial,
    pub update_material: NativeUpdateMaterial,
    pub replace_material: NativeReplaceMaterial,
    pub destroy_material: NativeDestroyMaterial,
    pub create_primitive: NativeCreatePrimitiveAppearance,
    pub replace_primitive: NativeReplacePrimitiveAppearance,
    pub create_mesh_resource: NativeCreateMeshResource,
    pub destroy_mesh_resource: NativeDestroyMeshResource,
    pub create_mesh_appearance: NativeCreateMeshAppearance,
    pub partition_mesh: NativePartitionMesh,
    pub read_mesh_partition: NativeReadMeshPartition,
    pub take_mesh_partition_part: NativeTakeMeshPartitionPart,
    pub destroy_mesh_partition: NativeDestroyMeshPartition,
    pub create_static_mesh: NativeCreateStaticMeshAppearance,
    pub create_static_mesh_from_content: NativeCreateStaticMeshContentAppearance,
    pub replace_static_mesh: NativeReplaceStaticMeshAppearance,
    pub replace_static_mesh_from_content: NativeReplaceStaticMeshContentAppearance,
    pub update_static_mesh_materials: NativeUpdateStaticMeshMaterials,
    pub create_sprite: NativeCreateSpriteAppearance,
    pub replace_sprite: NativeReplaceSpriteAppearance,
    pub create_sprite_atlas: NativeCreateSpriteAtlas,
    pub destroy_sprite_atlas: NativeDestroySpriteAtlas,
    pub create_sprite_from_atlas: NativeCreateSpriteFromAtlas,
    pub replace_sprite_from_atlas: NativeReplaceSpriteFromAtlas,
    pub set_sprite_frame: NativeSetSpriteFrame,
    pub set_sprite_viewport: NativeSetSpriteViewport,
    pub read_sprite: NativeReadSprite,
    pub create_sprite_playback: NativeCreateSpritePlayback,
    pub destroy_sprite_playback: NativeDestroySpritePlayback,
    pub control_sprite_playback: NativeControlSpritePlayback,
    pub select_sprite_playback_frame: NativeSelectSpritePlaybackFrame,
    pub advance_sprite_playback: NativeAdvanceSpritePlayback,
    pub sample_sprite_playback: NativeSampleSpritePlayback,
    pub read_sprite_playback: NativeReadSpritePlayback,
    pub destroy_appearance: NativeDestroyAppearance,
    pub publish_snapshot: NativePublishAppearanceSnapshot,
    pub publish_changes: NativePublishAppearanceChanges,
    pub create_light: NativeCreateLight,
    pub update_light: NativeUpdateLight,
    pub replace_light: NativeReplaceLight,
    pub destroy_light: NativeDestroyLight,
    pub read_light: NativeReadLight,
    pub read_presentation: NativeReadPresentation,
    pub create_authored_material: NativeCreateAuthoredMaterial,
}

/// Named renderer-neutral facts. Handles identify product-owned billboard and
/// particle facts only; renderer resource/frame ownership remains in Engine.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePresentationApi {
    pub context: *mut c_void,
    pub create_billboard: NativeCreatePresentationBillboard,
    pub update_billboard: NativeUpdatePresentationBillboard,
    pub create_structured_billboard: NativeCreatePresentationStructuredBillboard,
    pub update_structured_billboard: NativeUpdatePresentationStructuredBillboard,
    pub destroy_billboard: NativeDestroyPresentationBillboard,
    pub emit_particles: NativeEmitPresentationParticles,
    pub create_emitter: NativeCreatePresentationEmitter,
    pub update_emitter: NativeUpdatePresentationEmitter,
    pub destroy_emitter: NativeDestroyPresentationEmitter,
    pub read: NativeReadPresentationFacts,
    pub create_ghost_plate: NativeCreateGhostPlatePresentation,
    pub update_ghost_plate: NativeUpdateGhostPlatePresentation,
    pub recapture_ghost_plate: NativeRecaptureGhostPlatePresentation,
    pub read_ghost_plate: NativeReadGhostPlatePresentation,
    pub destroy_ghost_plate: NativeDestroyGhostPlatePresentation,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentApi {
    pub context: *mut c_void,
    pub load_portable_asset: NativeLoadPortableAsset,
    pub destroy_portable_asset: NativeDestroyPortableAsset,
    pub read_portable_asset: NativeReadPortableAsset,
    pub open_portable_asset_member: NativeOpenPortableAssetMember,
    pub admit_reference: NativeAdmitContentReference,
    pub list_bundles: NativeListContentBundles,
    pub open_bundle: NativeOpenContentBundle,
    pub open_container: NativeOpenContentContainer,
    pub pack_container: NativePackContentContainer,
    pub read_bundle_identity: NativeReadContentBundleIdentity,
    pub destroy_bundle: NativeDestroyContentBundle,
    pub read_bundle_files: NativeReadContentBundleFiles,
    pub open_bundle_reference: NativeOpenContentBundleReference,
    pub open_reference: NativeOpenContentReference,
    pub resolve_reference: NativeResolveContentReference,
    pub destroy_reference: NativeDestroyContentReference,
    pub read_reference_info: NativeReadContentReferenceInfo,
    pub read_bytes: NativeReadContentBytes,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAuthoredContentApi {
    pub context: *mut c_void,
    pub admit_catalog: NativeAdmitAuthoredCatalog,
    pub admit_catalog_from_content: NativeAdmitAuthoredCatalogFromContent,
    pub admit_catalog_payload: NativeAdmitAuthoredCatalogPayload,
    pub destroy_catalog: NativeDestroyAuthoredCatalog,
    pub read_catalog: NativeReadAuthoredCatalog,
    pub resolve_reference: NativeResolveAuthoredCatalogReference,
    pub resolve_material: NativeResolveAuthoredMaterial,
    pub resolve_voxel_surface: NativeResolveAuthoredVoxelSurface,
    pub resolve_fallback: NativeResolveAuthoredFallback,
    pub admit_prefab_registry: NativeAdmitAuthoredPrefabRegistry,
    pub admit_prefab_registry_from_content: NativeAdmitAuthoredPrefabRegistryFromContent,
    pub destroy_prefab_registry: NativeDestroyAuthoredPrefabRegistry,
    pub read_prefab_registry: NativeReadAuthoredPrefabRegistry,
    pub resolve_prefab: NativeResolveAuthoredPrefab,
    pub prepare_scene: NativePrepareAuthoredScene,
    pub prepare_scene_from_content: NativePrepareAuthoredSceneFromContent,
    pub destroy_scene_plan: NativeDestroyAuthoredScenePlan,
    pub read_scene_plan: NativeReadAuthoredScenePlan,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationApi {
    pub context: *mut c_void,
    pub read_mesh_info: NativeReadAnimatedMeshInfo,
    pub read_clips: NativeReadAnimationClips,
    pub open_animated_mesh: NativeOpenAnimatedMesh,
    pub open_animated_mesh_from_content: NativeOpenAnimationResourceFromContent,
    pub open_animation_clip_pack_from_content: NativeOpenAnimationResourceFromContent,
    pub open_animation_clip_pack: NativeOpenAnimationClipPack,
    pub associate_animation_clip_pack: NativeAssociateAnimationClipPack,
    pub create_animated_mesh_appearance: NativeCreateAnimatedMeshAppearance,
    pub replace_animated_mesh_appearance: NativeReplaceAnimatedMeshAppearance,
    pub set_mesh_inspection: NativeSetAnimatedMeshInspection,
    pub update_animated_mesh_materials: NativeUpdateAnimatedMeshMaterials,
    pub destroy_appearance: NativeDestroyAppearance,
    pub create_instance: NativeCreateAnimationInstance,
    pub destroy_instance: NativeDestroyAnimationInstance,
    pub replace_instance: NativeReplaceAnimationInstance,
    pub set_playback: NativeSetAnimationPlayback,
    pub create_graph: NativeCreateAnimationGraph,
    pub destroy_graph: NativeDestroyAnimationGraph,
    pub define_parameter: NativeDefineAnimationParameter,
    pub define_state: NativeDefineAnimationState,
    pub define_transition: NativeDefineAnimationTransition,
    pub define_condition: NativeDefineAnimationCondition,
    pub create_controller: NativeCreateAnimationController,
    pub destroy_controller: NativeDestroyAnimationController,
    pub set_float: NativeSetAnimationFloat,
    pub set_bool: NativeSetAnimationBool,
    pub fire_trigger: NativeFireAnimationTrigger,
    pub tick: NativeTickAnimation,
    pub read_controller: NativeReadAnimationController,
    pub read: NativeReadAnimation,
    pub read_realization: NativeReadAnimationRealization,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioApi {
    pub context: *mut c_void,
    pub open_clip: NativeOpenAudioClip,
    pub open_clip_from_content: NativeOpenAudioClipFromContent,
    pub destroy_clip: NativeDestroyAudioClip,
    pub preload_optional: NativePreloadOptionalAudioClip,
    pub emit: NativeEmitAudio,
    pub create_voice: NativeCreateAudioVoice,
    pub update_voice: NativeUpdateAudioVoice,
    pub replace_voice: NativeReplaceAudioVoice,
    pub destroy_voice: NativeDestroyAudioVoice,
    pub control_voice: NativeControlAudioVoice,
    pub set_bus_volume: NativeSetAudioBusVolume,
    pub set_bus_muted: NativeSetAudioBusMuted,
    pub read: NativeReadAudio,
    pub read_voice: NativeReadAudioVoice,
    pub read_bus: NativeReadAudioBus,
    pub read_realization: NativeReadAudioRealization,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeVideoApi {
    pub context: *mut c_void,
    pub play: NativePlayVideo,
    pub play_from_content: NativePlayVideoFromContent,
    pub stop: NativeStopVideo,
    pub skip: NativeSkipVideo,
    pub read: NativeReadVideo,
    pub read_realization: NativeReadVideoRealization,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraViewApi {
    pub context: *mut c_void,
    pub create_camera: NativeCreateCamera,
    pub update_camera: NativeUpdateCamera,
    pub update_camera_sample: NativeUpdateCameraSample,
    pub replace_camera: NativeReplaceCamera,
    pub destroy_camera: NativeDestroyCamera,
    pub create_camera_target: NativeCreateCameraTarget,
    pub update_camera_target: NativeUpdateCameraTarget,
    pub replace_camera_target: NativeReplaceCameraTarget,
    pub destroy_camera_target: NativeDestroyCameraTarget,
    pub set_camera_composition: NativeSetCameraComposition,
    pub set_active_camera: NativeSetActiveCamera,
    pub clear_active_camera: NativeClearActiveCamera,
    pub set_sky_background: NativeSetSkyBackground,
    pub set_sky_background_blend: NativeSetSkyBackgroundBlend,
    pub clear_sky_background: NativeClearSkyBackground,
    pub set_background_color: NativeSetBackgroundColor,
    pub set_fog: NativeSetFog,
    pub set_tone_mapping: NativeSetToneMapping,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRngApi {
    pub context: *mut c_void,
    pub draw_keyed: NativeDrawKeyedRng,
    pub draw_lcg15: NativeDrawLcg15,
    pub create_scoped: NativeCreateScopedRng,
    pub fork_scoped: NativeForkScopedRng,
    pub destroy_scoped: NativeDestroyScopedRng,
    pub next_u64: NativeNextScopedRng,
    pub next_bounded_u32: NativeNextBoundedScopedRng,
    pub next_bool: NativeNextScopedRng,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePersistenceApi {
    pub context: *mut c_void,
    pub open_store: NativeOpenPersistenceStore,
    pub destroy_store: NativeDestroyPersistenceStore,
    pub save: NativeSavePersistence,
    pub delete: NativeDeletePersistence,
    pub load: NativeLoadPersistence,
    pub destroy_blob: NativeDestroyPersistenceBlob,
    pub describe_blob: NativeDescribePersistenceBlob,
    pub copy_blob: NativeCopyPersistenceBlob,
    pub read_blob_bytes: NativeReadPersistenceBlobBytes,
}

pub type NativeReplaceInputMappings = unsafe extern "C" fn(
    *mut c_void,
    *const NativeInputMapping,
    usize,
    *mut NativeInputMappingReplacementOutcome,
    *mut NativeOperationErrorReceipt,
) -> i32;

/// Runtime replacement of physical mappings. The product supplies a complete
/// candidate set; the Engine retains declared semantic intents and settles a
/// valid candidate only after the current callback succeeds.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeInputApi {
    pub context: *mut c_void,
    pub replace_physical_mappings: NativeReplaceInputMappings,
}

/// Direct named Engine service families available to trusted NativeAOT code.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeEngineApi {
    pub input: NativeInputApi,
    pub implicit_surfaces: NativeImplicitSurfacesApi,
    pub diagnostics: NativeDiagnosticsApi,
    pub dynamics: NativeDynamicsApi,
    pub motion: NativeMotionApi,
    pub kinematic: NativeKinematicApi,
    pub spatial: NativeSpatialApi,
    pub perception: NativePerceptionApi,
    pub world_origin: NativeWorldOriginApi,
    pub voxel: NativeVoxelApi,
    pub voxel_content: NativeVoxelContentApi,
    pub voxel_scene_presentation: NativeVoxelScenePresentationApi,
    pub content: NativeContentApi,
    pub authored_content: NativeAuthoredContentApi,
    pub graphics: NativeGraphicsApi,
    pub presentation: NativePresentationApi,
    pub animation: NativeAnimationApi,
    pub audio: NativeAudioApi,
    pub video: NativeVideoApi,
    pub render_output: NativeRenderOutputApi,
    pub camera_view: NativeCameraViewApi,
    pub rng: NativeRngApi,
    pub persistence: NativePersistenceApi,
    pub ui: NativeUiApi,
}

/// Borrowed creation inputs plus the direct Engine API.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeProductCreateArgs {
    pub content: *const NativeContentFile,
    pub content_len: usize,
    pub input: NativeInputConfiguration,
    pub engine: NativeEngineApi,
}

/// Product-owned meaning of one externally completed timeline ticket.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeProductTimelineOutcome {
    Success = 1,
    Failure = 2,
}

/// Borrowed completion data copied by the generated C# product bootstrap.
/// Empty outcome/provenance data slices represent absent optional values.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeProductTimelineCompletion {
    pub ticket: u64,
    pub instance_id: u64,
    pub generation: u64,
    pub control_revision: u64,
    pub correlation: NativeUtf8Slice,
    pub outcome: NativeProductTimelineOutcome,
    pub outcome_data: NativeByteSlice,
    pub provenance_correlation: NativeUtf8Slice,
    pub provenance_detail: NativeByteSlice,
}

/// Creates the product. On failure the product fills `NativeProductCallError`,
/// which Rust copies and releases with `NativeProductReleaseCallError`.
pub type NativeProductCreate = unsafe extern "C" fn(
    *const NativeProductCreateArgs,
    *mut *mut c_void,
    *mut NativeProductCallError,
) -> i32;
pub type NativeProductAction = unsafe extern "C" fn(*mut c_void) -> i32;
pub type NativeProductUpdate = unsafe extern "C" fn(
    *mut c_void,
    *const NativeProductUpdateArgs,
    *mut NativeProductUpdateResult,
) -> i32;
pub type NativeProductCompleteTimeline =
    unsafe extern "C" fn(*mut c_void, *const NativeProductTimelineCompletion, *mut u8) -> i32;
/// Copies the Rust-owned lifecycle state after a host transition has committed.
/// It is notification-only and cannot influence the committed transition.
pub type NativeProductObserveRuntime =
    unsafe extern "C" fn(*mut c_void, *const NativeProductRuntimeFacts);
pub type NativeProductDestroy = unsafe extern "C" fn(*mut c_void);

/// Product-owned outcome for one generated live-debug command execution.
///
/// `message` is allocated by the managed product and stays valid until the
/// matching `NativeProductReleaseDebugResult` callback consumes it.  Rust
/// copies it before release; it does not retain product-owned debug output.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeProductDebugResult {
    /// `1` is a completed command; `0` is a semantic command failure.
    /// ABI failure is reported by the callback's return status instead.
    pub succeeded: u8,
    pub message: NativeUtf8Slice,
}

/// One copied diagnostic describing a failed generated product callback.
/// Every UTF-8 slice is product-owned until `NativeProductReleaseCallError`
/// consumes this result. Rust copies the fields before release; it never keeps
/// the product allocation or an exception object across the ABI.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeProductCallError {
    pub service: NativeUtf8Slice,
    pub operation: NativeUtf8Slice,
    pub status: i32,
    pub message: NativeUtf8Slice,
}

/// Executes one borrowed UTF-8 command through the generated product catalog.
pub type NativeProductExecuteDebug =
    unsafe extern "C" fn(*mut c_void, *const NativeUtf8Slice, *mut NativeProductDebugResult) -> i32;

/// Reads the generated product-owned live-debug catalog as bounded UTF-8
/// descriptor data. This is deliberately separate from command execution:
/// callers may use the returned data for help and completion, but it never
/// participates in dispatch.
pub type NativeProductDescribeDebug =
    unsafe extern "C" fn(*mut c_void, *mut NativeProductDebugResult) -> i32;

/// Releases the exact product-owned UTF-8 buffer returned by
/// [`NativeProductExecuteDebug`].  Rust calls this after every callback that
/// may have initialized the result, including an ABI failure.
pub type NativeProductReleaseDebugResult =
    unsafe extern "C" fn(*mut c_void, NativeProductDebugResult);

/// Reads the last failure recorded by a generated product callback. The
/// callback returns a product-owned UTF-8 result that Rust copies immediately
/// and releases with the matching callback. It has no Engine service access.
pub type NativeProductReadCallError =
    unsafe extern "C" fn(*mut c_void, *mut NativeProductCallError) -> i32;

/// Releases one product-owned callback diagnostic result, including failures
/// where its callback returned a non-success status after initializing it.
pub type NativeProductReleaseCallError = unsafe extern "C" fn(*mut c_void, NativeProductCallError);

/// Product functions supplied to Rust by the generated V1 bootstrap handshake.
/// The generated `ProductBridge` fills every field and the host requires each
/// one; the exact ABI fingerprint rules out an older, partial table. Fields stay
/// nullable only so the host reports a null as an error rather than calling it.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeProductApi {
    pub create: Option<
        unsafe extern "C" fn(
            *const NativeProductCreateArgs,
            *mut *mut c_void,
            *mut NativeProductCallError,
        ) -> i32,
    >,
    pub start: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub update: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const NativeProductUpdateArgs,
            *mut NativeProductUpdateResult,
        ) -> i32,
    >,
    pub pause: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub resume: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub restart: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub shutdown: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
    pub complete_timeline: Option<
        unsafe extern "C" fn(*mut c_void, *const NativeProductTimelineCompletion, *mut u8) -> i32,
    >,
    pub execute_debug: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const NativeUtf8Slice,
            *mut NativeProductDebugResult,
        ) -> i32,
    >,
    pub describe_debug:
        Option<unsafe extern "C" fn(*mut c_void, *mut NativeProductDebugResult) -> i32>,
    pub release_debug_result: Option<unsafe extern "C" fn(*mut c_void, NativeProductDebugResult)>,
    pub observe_runtime:
        Option<unsafe extern "C" fn(*mut c_void, *const NativeProductRuntimeFacts)>,
    pub read_call_error:
        Option<unsafe extern "C" fn(*mut c_void, *mut NativeProductCallError) -> i32>,
    pub release_call_error: Option<unsafe extern "C" fn(*mut c_void, NativeProductCallError)>,
}

/// Fixed SHA-256 ABI fingerprint emitted from BindingGenerator's parsed C ABI
/// model. Four explicit words keep the representation straightforward in both
/// Rust and generated C# without a managed array pinning contract.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeProductAbiFingerprint {
    pub word0: u64,
    pub word1: u64,
    pub word2: u64,
    pub word3: u64,
}

/// The V1 product ABI handshake. The host supplies its expected declaration;
/// the product reports its independently generated declaration and a pointer
/// to its immutable API table. The host copies that table only after exact
/// agreement, so a product never receives host-owned table storage to write.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeProductAbiHandshakeV1 {
    pub protocol_version: u32,
    pub engine_api_size: usize,
    pub product_api_size: usize,
    pub fingerprint: NativeProductAbiFingerprint,
    /// Bounded, copied immediately by the peer, diagnostic provenance only.
    pub build_identity: NativeUtf8Slice,
    /// Set only by the product response. It is ignored until exact agreement.
    pub product_api: *const NativeProductApi,
}

impl Default for NativeProductAbiHandshakeV1 {
    fn default() -> Self {
        Self {
            protocol_version: 0,
            engine_api_size: 0,
            product_api_size: 0,
            fingerprint: NativeProductAbiFingerprint::default(),
            build_identity: NativeUtf8Slice {
                bytes: std::ptr::null(),
                len: 0,
            },
            product_api: std::ptr::null(),
        }
    }
}

/// V1-only generated product bootstrap. There is deliberately no legacy bind
/// alias or compatibility fallback.
pub type NativeProductBindV1 = unsafe extern "C" fn(
    *const NativeProductAbiHandshakeV1,
    *mut NativeProductAbiHandshakeV1,
) -> i32;

pub type NativeOpenRenderResourceFromContent = unsafe extern "C" fn(
    *mut c_void,
    *const NativeRenderResourceContentRequest,
    *mut NativeRenderResourceInfo,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;
pub type NativeOpenAnimationResourceFromContent = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationContentRequest,
    *mut NativeRenderResourceHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCreateStaticMeshFromContentReference = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStaticMeshContentReferenceRequest,
    *mut NativeAppearanceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;

pub type NativeDestroyRenderResource = unsafe extern "C" fn(
    *mut c_void,
    NativeRenderResourceHandle,
    *mut crate::NativeOperationErrorReceipt,
) -> i32;

pub type NativeObserveDynamicsAnchor = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsObserveAnchorRequest,
    *mut NativeDynamicsAnchorObservation,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeStepDynamicsWithReactions = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsStepWithReactionsRequest,
    *mut NativeDynamicsStepReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
