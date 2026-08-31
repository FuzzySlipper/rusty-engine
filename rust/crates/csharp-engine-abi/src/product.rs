use crate::*;
use std::ffi::c_void;
pub type NativeIntegrateLook =
    unsafe extern "C" fn(*mut c_void, NativeLookRequest, *mut NativeLookReceipt) -> i32;
pub type NativeResetLook =
    unsafe extern "C" fn(*mut c_void, NativeLookResetRequest, *mut NativeLookReceipt) -> i32;
pub type NativeRebaseLook =
    unsafe extern "C" fn(*mut c_void, NativeLookRebaseRequest, *mut NativeLookReceipt) -> i32;
pub type NativeDiagnoseLook =
    unsafe extern "C" fn(*mut c_void, NativeLookRequest, *mut NativeLookDiagnostic) -> i32;
pub type NativeCreateDynamicsWorld = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsWorldConfig,
    *mut NativeDynamicsWorldHandle,
) -> i32;
pub type NativeDestroyDynamicsWorld =
    unsafe extern "C" fn(*mut c_void, NativeDynamicsWorldHandle) -> i32;
pub type NativeCreateDynamicsBody = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsCreateBodyRequest,
    *mut NativeDynamicsBodyHandle,
) -> i32;
pub type NativeCreateDynamicsSphereBody = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsCreateSphereBodyRequest,
    *mut NativeDynamicsBodyHandle,
) -> i32;
pub type NativeCreateDynamicsCuboidBody = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsCreateCuboidBodyRequest,
    *mut NativeDynamicsBodyHandle,
) -> i32;
pub type NativeCreateDynamicsSphereBodyWithProperties = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsCreateSphereBodyPropertiesRequest,
    *mut NativeDynamicsBodyHandle,
) -> i32;
pub type NativeCreateDynamicsCapsuleBody = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsCreateCapsuleBodyRequest,
    *mut NativeDynamicsBodyHandle,
) -> i32;
pub type NativeBindDynamicsWorldCollision =
    unsafe extern "C" fn(*mut c_void, NativeDynamicsWorldCollisionBindingRequest) -> i32;
pub type NativeRebaseDynamicsWorldOrigin = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsRebaseWorldOriginRequest,
    *mut NativeDynamicsRebaseWorldOriginReceipt,
) -> i32;
pub type NativeDestroyDynamicsBody =
    unsafe extern "C" fn(*mut c_void, NativeDynamicsBodyHandle) -> i32;
pub type NativeStepDynamics = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsStepRequest,
    *mut NativeDynamicsStepReceipt,
) -> i32;
pub type NativeStepAndReadDynamics = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsStepAndReadRequest,
    *mut NativeDynamicsStepAndReadLease,
) -> i32;
pub type NativeDestroyDynamicsStepAndReadLease =
    unsafe extern "C" fn(*mut c_void, NativeDynamicsStepAndReadLeaseHandle) -> i32;
pub type NativeReadDynamics =
    unsafe extern "C" fn(*mut c_void, NativeDynamicsReadRequest, *mut NativeDynamicsReadout) -> i32;
pub type NativeResetDynamics = unsafe extern "C" fn(*mut c_void, NativeDynamicsResetRequest) -> i32;
pub type NativeUpdateDynamicsBody =
    unsafe extern "C" fn(*mut c_void, NativeDynamicsUpdateBodyRequest) -> i32;
pub type NativeReadDynamicsWorld = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsWorldReadRequest,
    *mut NativeDynamicsWorldReadout,
) -> i32;
pub type NativeReadDynamicsBodyAt = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsBodyAtRequest,
    *mut NativeDynamicsBodyAtReceipt,
) -> i32;
pub type NativeReadDynamicsContactAt = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsContactAtRequest,
    *mut NativeDynamicsContactAtReceipt,
) -> i32;
pub type NativeReplaceDynamicsBody = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsReplaceBodyRequest,
    *mut NativeDynamicsBodyHandle,
) -> i32;
pub type NativeReplaceDynamicsCuboidBody = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsReplaceCuboidBodyRequest,
    *mut NativeDynamicsBodyHandle,
) -> i32;
pub type NativeReplaceDynamicsSphereBody = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsReplaceSphereBodyRequest,
    *mut NativeDynamicsBodyHandle,
) -> i32;
pub type NativeReplaceDynamicsCapsuleBody = unsafe extern "C" fn(
    *mut c_void,
    NativeDynamicsReplaceCapsuleBodyRequest,
    *mut NativeDynamicsBodyHandle,
) -> i32;
pub type NativeCreateSpatialSession = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialSessionConfig,
    *mut NativeSpatialSessionHandle,
) -> i32;
pub type NativeDestroySpatialSession =
    unsafe extern "C" fn(*mut c_void, NativeSpatialSessionHandle) -> i32;
pub type NativeMotionResolve = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMotionResolveRequest,
    *mut NativeMotionResolveReceipt,
) -> i32;
pub type NativeReplaceCollision = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCollisionReplaceRequest,
    *mut NativeCollisionReplaceReceipt,
) -> i32;
pub type NativeReplaceSpatialContentArtifact = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialContentArtifactReplaceRequest,
    *mut NativeSpatialContentArtifactReplaceReceipt,
) -> i32;
pub type NativeReadSpatialContentArtifact = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialContentArtifactReadRequest,
    *mut NativeSpatialContentArtifactReadout,
) -> i32;
pub type NativeReplaceNavigation = unsafe extern "C" fn(
    *mut c_void,
    *const NativeNavigationReplaceRequest,
    *mut NativeNavigationReplaceReceipt,
) -> i32;
pub type NativeReplaceVoxelNavigation = unsafe extern "C" fn(
    *mut c_void,
    *const NativeNavigationVoxelReplaceRequest,
    *mut NativeNavigationReplaceReceipt,
) -> i32;
pub type NativeReplaceNavigationTraversal = unsafe extern "C" fn(
    *mut c_void,
    *const NativeNavigationTraversalReplaceRequest,
    *mut NativeNavigationTraversalReplaceReceipt,
) -> i32;
pub type NativeClearNavigationTraversal = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationTraversalClearRequest,
    *mut NativeNavigationTraversalReplaceReceipt,
) -> i32;
pub type NativeReplaceVolumetricNavigationTraversal = unsafe extern "C" fn(
    *mut c_void,
    *const NativeNavigationVolumetricTraversalReplaceRequest,
    *mut NativeNavigationVolumetricTraversalReplaceReceipt,
) -> i32;
pub type NativeClearVolumetricNavigationTraversal = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationVolumetricTraversalClearRequest,
    *mut NativeNavigationVolumetricTraversalReplaceReceipt,
) -> i32;
pub type NativeReadNavigationProjection = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationProjectionReadRequest,
    *mut NativeNavigationProjectionReadout,
) -> i32;
pub type NativeRequestNavigationPath = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationPathRequest,
    *mut NativeNavigationPathReadout,
) -> i32;
pub type NativeRequestWeightedNavigationPath = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationWeightedPathRequest,
    *mut NativeNavigationWeightedPathReadout,
) -> i32;
pub type NativeRequestWeightedVolumetricNavigationPath = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationVolumetricWeightedPathRequest,
    *mut NativeNavigationVolumetricWeightedPathReadout,
) -> i32;
pub type NativeReadNavigationPathCellAt = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationPathCellAtRequest,
    *mut NativeNavigationPathCellAtReceipt,
) -> i32;
pub type NativeRequestVolumetricNavigationPath = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationVolumetricPathRequest,
    *mut NativeNavigationPathReadout,
) -> i32;
pub type NativeClearNavigation =
    unsafe extern "C" fn(*mut c_void, NativeNavigationClearRequest) -> i32;
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
) -> i32;
pub type NativeReadCharacterController = unsafe extern "C" fn(
    *mut c_void,
    NativeCharacterControllerReadRequest,
    *mut NativeCharacterControllerReadout,
) -> i32;
pub type NativeReadCharacterContactAt = unsafe extern "C" fn(
    *mut c_void,
    NativeCharacterContactAtRequest,
    *mut NativeCharacterContactAtReceipt,
) -> i32;
pub type NativeReadCharacterDynamicImpulseAt = unsafe extern "C" fn(
    *mut c_void,
    NativeCharacterDynamicImpulseAtRequest,
    *mut NativeCharacterDynamicImpulseAtReceipt,
) -> i32;
pub type NativeProposeNavigationStep = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationStepRequest,
    *mut NativeNavigationStepReceipt,
) -> i32;
pub type NativeEvaluateNavigationStep = unsafe extern "C" fn(
    *mut c_void,
    NativeNavigationStepRequest,
    *mut NativeNavigationStepReceipt,
) -> i32;
pub type NativeReadSpatialProjection = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialProjectionReadRequest,
    *mut NativeSpatialProjectionReadout,
) -> i32;
pub type NativeSpatialContainsPoint = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialContainsPointRequest,
    *mut NativeSpatialQueryReceipt,
) -> i32;
pub type NativeSpatialRaycast = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialRaycastRequest,
    *mut NativeSpatialHit,
) -> i32;
pub type NativeQueryPerception = unsafe extern "C" fn(
    *mut c_void,
    *const NativePerceptionQueryRequest,
    *mut NativePerceptionReadoutLease,
) -> i32;
pub type NativeDestroyPerceptionReadoutLease =
    unsafe extern "C" fn(*mut c_void, NativePerceptionReadoutLeaseHandle) -> i32;
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
pub type NativeSpatialPickVoxel =
    unsafe extern "C" fn(*mut c_void, NativeSpatialPickRequest, *mut NativeSpatialHit) -> i32;
pub type NativeSpatialRegisterTrigger = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialTriggerRegisterRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSpatialReconcileTriggers = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialTriggerReconcileRequest,
    *mut NativeSpatialTriggerReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroySpatialOperationDiagnosticLease =
    unsafe extern "C" fn(*mut c_void, NativeEngineDiagnosticLeaseHandle) -> i32;
pub type NativeSpatialReadTrigger = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialTriggerReadRequest,
    *mut NativeSpatialTriggerReadReceipt,
) -> i32;
pub type NativeSpatialReadTriggerOverlapAt = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialTriggerOverlapAtRequest,
    *mut NativeSpatialTriggerOverlapAtReceipt,
) -> i32;
pub type NativeSpatialReadTriggerFactAt = unsafe extern "C" fn(
    *mut c_void,
    NativeSpatialTriggerFactAtRequest,
    *mut NativeSpatialTriggerFactAtReceipt,
) -> i32;
pub type NativeWorldOriginPrepare = unsafe extern "C" fn(
    *mut c_void,
    *const NativeWorldOriginPrepareRequest,
    *mut NativeWorldOriginPreparedHandle,
) -> i32;
pub type NativeWorldOriginRead = unsafe extern "C" fn(
    *mut c_void,
    NativeWorldOriginReadRequest,
    *mut NativeWorldOriginReadout,
) -> i32;
pub type NativeWorldOriginReadPrepared = unsafe extern "C" fn(
    *mut c_void,
    NativeWorldOriginPreparedReadRequest,
    *mut NativeWorldOriginPreparedReadout,
) -> i32;
pub type NativeWorldOriginReadAffectedAt = unsafe extern "C" fn(
    *mut c_void,
    NativeWorldOriginAffectedAtRequest,
    *mut NativeWorldOriginAffectedAtReceipt,
) -> i32;
pub type NativeWorldOriginCommit = unsafe extern "C" fn(
    *mut c_void,
    NativeWorldOriginCommitRequest,
    *mut NativeWorldOriginCommitReceipt,
) -> i32;
pub type NativeDestroyWorldOriginPrepared =
    unsafe extern "C" fn(*mut c_void, NativeWorldOriginPreparedHandle) -> i32;
pub type NativeOpenRenderResource = unsafe extern "C" fn(
    *mut c_void,
    *const NativeRenderResourceRequest,
    *mut NativeRenderResourceInfo,
) -> i32;
pub type NativeOpenAudioClip = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioClipRequest,
    *mut NativeAudioClipHandle,
) -> i32;
pub type NativeEmitAudio = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioEmitRequest,
    *mut NativeAudioSignalHandle,
) -> i32;
pub type NativeCreateAudioVoice = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioSourceDescriptor,
    *mut NativeAudioVoiceHandle,
) -> i32;
pub type NativeUpdateAudioVoice =
    unsafe extern "C" fn(*mut c_void, *const NativeAudioVoiceUpdateRequest) -> i32;
pub type NativeReplaceAudioVoice = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioVoiceReplaceRequest,
    *mut NativeAudioVoiceHandle,
) -> i32;
pub type NativeDestroyAudioVoice = unsafe extern "C" fn(*mut c_void, NativeAudioVoiceHandle) -> i32;
pub type NativeControlAudioVoice =
    unsafe extern "C" fn(*mut c_void, *const NativeAudioVoiceControlRequest) -> i32;
pub type NativeSetAudioBusVolume =
    unsafe extern "C" fn(*mut c_void, *const NativeAudioBusVolumeRequest) -> i32;
pub type NativeSetAudioBusMuted =
    unsafe extern "C" fn(*mut c_void, *const NativeAudioBusMutedRequest) -> i32;
pub type NativeReadAudio = unsafe extern "C" fn(*mut c_void, *mut NativeAudioReadout) -> i32;
pub type NativeReadAudioVoice = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioVoiceReadRequest,
    *mut NativeAudioVoiceReadout,
) -> i32;
pub type NativeReadAudioBus = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAudioBusReadRequest,
    *mut NativeAudioBusReadout,
) -> i32;
pub type NativeReadAudioDiagnosticAt = unsafe extern "C" fn(
    *mut c_void,
    NativeAudioDiagnosticAtRequest,
    *mut NativeAudioDiagnosticAtReceipt,
) -> i32;
pub type NativeReadAudioRealization =
    unsafe extern "C" fn(*mut c_void, *mut NativeAudioRealizationReadout) -> i32;
pub type NativeReadAudioRealizationFactAt = unsafe extern "C" fn(
    *mut c_void,
    NativeAudioRealizationFactAtRequest,
    *mut NativeAudioRealizationFactAtReceipt,
) -> i32;
pub type NativeCreateMaterial =
    unsafe extern "C" fn(*mut c_void, NativeMaterialRequest, *mut NativeMaterialHandle) -> i32;
pub type NativeUpdateMaterial =
    unsafe extern "C" fn(*mut c_void, NativeMaterialUpdateRequest) -> i32;
pub type NativeReplaceMaterial = unsafe extern "C" fn(
    *mut c_void,
    NativeMaterialUpdateRequest,
    *mut NativeMaterialHandle,
) -> i32;
pub type NativeDestroyMaterial = unsafe extern "C" fn(*mut c_void, NativeMaterialHandle) -> i32;
pub type NativeCreatePrimitiveAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativePrimitiveAppearanceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeReplacePrimitiveAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativePrimitiveAppearanceReplaceRequest,
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
pub type NativeReplaceStaticMeshAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativeAppearanceHandle,
    *const NativeStaticMeshAppearanceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeReplaceStaticMeshContentAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativeAppearanceHandle,
    *const NativeStaticMeshContentAppearanceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeUpdateStaticMeshMaterials =
    unsafe extern "C" fn(*mut c_void, *const NativeStaticMeshMaterialUpdateRequest) -> i32;
pub type NativeCreateSpriteAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativeSpriteAppearanceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeReplaceSpriteAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativeSpriteAppearanceReplaceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeCreateSpriteAtlas = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpriteAtlasCreateRequest,
    *mut NativeSpriteAtlasHandle,
) -> i32;
pub type NativeDestroySpriteAtlas =
    unsafe extern "C" fn(*mut c_void, NativeSpriteAtlasHandle) -> i32;
pub type NativeCreateSpriteFromAtlas = unsafe extern "C" fn(
    *mut c_void,
    NativeSpriteFromAtlasRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeReplaceSpriteFromAtlas = unsafe extern "C" fn(
    *mut c_void,
    NativeSpriteFromAtlasReplaceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeSetSpriteFrame =
    unsafe extern "C" fn(*mut c_void, NativeSpriteFrameUpdateRequest) -> i32;
pub type NativeReadSprite =
    unsafe extern "C" fn(*mut c_void, NativeAppearanceHandle, *mut NativeSpriteReadout) -> i32;
pub type NativeCreateSpritePlayback = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpritePlaybackCreateRequest,
    *mut NativeSpritePlaybackHandle,
) -> i32;
pub type NativeDestroySpritePlayback =
    unsafe extern "C" fn(*mut c_void, NativeSpritePlaybackHandle) -> i32;
pub type NativeControlSpritePlayback = unsafe extern "C" fn(
    *mut c_void,
    NativeSpritePlaybackControlRequest,
    *mut NativeSpritePlaybackReadout,
) -> i32;
pub type NativeAdvanceSpritePlayback = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpritePlaybackAdvanceRequest,
    *mut NativeSpritePlaybackAdvanceLease,
) -> i32;
pub type NativeDestroySpritePlaybackAdvanceLease =
    unsafe extern "C" fn(*mut c_void, NativeSpritePlaybackAdvanceLeaseHandle) -> i32;
pub type NativeSampleSpritePlayback = unsafe extern "C" fn(
    *mut c_void,
    NativeSpritePlaybackSampleRequest,
    *mut NativeSpritePlaybackSample,
) -> i32;
pub type NativeReadSpritePlayback = unsafe extern "C" fn(
    *mut c_void,
    NativeSpritePlaybackHandle,
    *mut NativeSpritePlaybackReadout,
) -> i32;
pub type NativeDestroyAppearance = unsafe extern "C" fn(*mut c_void, NativeAppearanceHandle) -> i32;
pub type NativePublishAppearanceSnapshot =
    unsafe extern "C" fn(*mut c_void, *const NativeAppearanceFact, usize) -> i32;
pub type NativeCreateLight =
    unsafe extern "C" fn(*mut c_void, NativeLightRequest, *mut NativeLightHandle) -> i32;
pub type NativeUpdateLight = unsafe extern "C" fn(*mut c_void, NativeLightUpdateRequest) -> i32;
pub type NativeReplaceLight =
    unsafe extern "C" fn(*mut c_void, NativeLightUpdateRequest, *mut NativeLightHandle) -> i32;
pub type NativeDestroyLight = unsafe extern "C" fn(*mut c_void, NativeLightHandle) -> i32;
pub type NativeReadLight =
    unsafe extern "C" fn(*mut c_void, NativeLightHandle, *mut NativeLightReadout) -> i32;
pub type NativeReadPresentation =
    unsafe extern "C" fn(*mut c_void, *mut NativePresentationReadout) -> i32;
pub type NativeCreatePresentationBillboard = unsafe extern "C" fn(
    *mut c_void,
    *const NativePresentationBillboardDescriptor,
    *mut NativePresentationBillboardHandle,
) -> i32;
pub type NativeUpdatePresentationBillboard = unsafe extern "C" fn(
    *mut c_void,
    NativePresentationBillboardHandle,
    *const NativePresentationBillboardDescriptor,
) -> i32;
pub type NativeCreatePresentationStructuredBillboard = unsafe extern "C" fn(
    *mut c_void,
    *const NativePresentationStructuredBillboardDescriptor,
    *mut NativePresentationBillboardHandle,
) -> i32;
pub type NativeUpdatePresentationStructuredBillboard = unsafe extern "C" fn(
    *mut c_void,
    NativePresentationBillboardHandle,
    *const NativePresentationStructuredBillboardDescriptor,
) -> i32;
pub type NativeDestroyPresentationBillboard =
    unsafe extern "C" fn(*mut c_void, NativePresentationBillboardHandle) -> i32;
pub type NativeEmitPresentationParticles =
    unsafe extern "C" fn(*mut c_void, *const NativePresentationParticleDescriptor) -> i32;
pub type NativeCreatePresentationEmitter = unsafe extern "C" fn(
    *mut c_void,
    *const NativePresentationParticleDescriptor,
    *mut NativePresentationEmitterHandle,
) -> i32;
pub type NativeUpdatePresentationEmitter = unsafe extern "C" fn(
    *mut c_void,
    NativePresentationEmitterHandle,
    *const NativePresentationParticleDescriptor,
) -> i32;
pub type NativeDestroyPresentationEmitter =
    unsafe extern "C" fn(*mut c_void, NativePresentationEmitterHandle) -> i32;
pub type NativeReadPresentationFacts =
    unsafe extern "C" fn(*mut c_void, *mut NativePresentationFactsReadout) -> i32;
pub type NativeReadPresentationDiagnosticAt = unsafe extern "C" fn(
    *mut c_void,
    NativePresentationDiagnosticAtRequest,
    *mut NativePresentationDiagnosticAtReceipt,
) -> i32;
pub type NativeOpenAnimatedMesh = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimatedMeshResourceRequest,
    *mut NativeRenderResourceHandle,
) -> i32;
pub type NativeOpenAnimationClipPack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationClipPackResourceRequest,
    *mut NativeRenderResourceHandle,
) -> i32;
pub type NativeAssociateAnimationClipPack =
    unsafe extern "C" fn(*mut c_void, *const NativeAnimationClipPackAssociationRequest) -> i32;
pub type NativeCreateAnimatedMeshAppearance = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimatedMeshAppearanceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeReplaceAnimatedMeshAppearance = unsafe extern "C" fn(
    *mut c_void,
    NativeAppearanceHandle,
    *const NativeAnimatedMeshAppearanceRequest,
    *mut NativeAppearanceHandle,
) -> i32;
pub type NativeUpdateAnimatedMeshMaterials =
    unsafe extern "C" fn(*mut c_void, *const NativeAnimatedMeshMaterialUpdateRequest) -> i32;
pub type NativeCreateAnimationInstance = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationInstanceRequest,
    *mut NativeAnimationInstanceHandle,
) -> i32;
pub type NativeDestroyAnimationInstance =
    unsafe extern "C" fn(*mut c_void, NativeAnimationInstanceHandle) -> i32;
pub type NativeReplaceAnimationInstance = unsafe extern "C" fn(
    *mut c_void,
    NativeAnimationInstanceHandle,
    *const NativeAnimationInstanceRequest,
    *mut NativeAnimationInstanceHandle,
) -> i32;
pub type NativeSetAnimationPlayback =
    unsafe extern "C" fn(*mut c_void, *const NativeAnimationPlaybackRequest) -> i32;
pub type NativeReplaceAnimationCueDefinitions =
    unsafe extern "C" fn(*mut c_void, *const NativeAnimationCueDefinitionReplaceRequest) -> i32;
pub type NativeCreateAnimationGraph = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationGraphCreateRequest,
    *mut NativeAnimationGraphHandle,
) -> i32;
pub type NativeDestroyAnimationGraph =
    unsafe extern "C" fn(*mut c_void, NativeAnimationGraphHandle) -> i32;
pub type NativeDefineAnimationParameter =
    unsafe extern "C" fn(*mut c_void, *const NativeAnimationParameterDefinitionRequest) -> i32;
pub type NativeDefineAnimationState =
    unsafe extern "C" fn(*mut c_void, *const NativeAnimationStateDefinitionRequest) -> i32;
pub type NativeDefineAnimationTransition = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationTransitionDefinitionRequest,
    *mut NativeAnimationTransitionHandle,
) -> i32;
pub type NativeDefineAnimationCondition =
    unsafe extern "C" fn(*mut c_void, *const NativeAnimationConditionDefinitionRequest) -> i32;
pub type NativeCreateAnimationController = unsafe extern "C" fn(
    *mut c_void,
    *const NativeAnimationControllerCreateRequest,
    *mut NativeAnimationControllerHandle,
) -> i32;
pub type NativeDestroyAnimationController =
    unsafe extern "C" fn(*mut c_void, NativeAnimationControllerHandle) -> i32;
pub type NativeSetAnimationFloat =
    unsafe extern "C" fn(*mut c_void, *const NativeAnimationSetFloatRequest) -> i32;
pub type NativeSetAnimationBool =
    unsafe extern "C" fn(*mut c_void, *const NativeAnimationSetBoolRequest) -> i32;
pub type NativeFireAnimationTrigger =
    unsafe extern "C" fn(*mut c_void, *const NativeAnimationFireTriggerRequest) -> i32;
pub type NativeTickAnimation =
    unsafe extern "C" fn(*mut c_void, *const NativeAnimationTickRequest) -> i32;
pub type NativeReadAnimationController = unsafe extern "C" fn(
    *mut c_void,
    NativeAnimationControllerHandle,
    *mut NativeAnimationControllerReadout,
) -> i32;
pub type NativeReadAnimation =
    unsafe extern "C" fn(*mut c_void, *mut NativeAnimationReadout) -> i32;
pub type NativeReadAnimationRealization =
    unsafe extern "C" fn(*mut c_void, *mut NativeAnimationRealizationReadout) -> i32;
pub type NativeReadAnimationRealizationFactAt = unsafe extern "C" fn(
    *mut c_void,
    NativeAnimationRealizationFactAtRequest,
    *mut NativeAnimationRealizationFactAtReceipt,
) -> i32;
pub type NativeCreateCamera = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCameraDescriptor,
    *mut NativeCameraHandle,
) -> i32;
pub type NativeUpdateCamera =
    unsafe extern "C" fn(*mut c_void, *const NativeCameraUpdateRequest) -> i32;
pub type NativeReplaceCamera = unsafe extern "C" fn(
    *mut c_void,
    *const NativeCameraReplaceRequest,
    *mut NativeCameraHandle,
) -> i32;
pub type NativeDestroyCamera = unsafe extern "C" fn(*mut c_void, NativeCameraHandle) -> i32;
pub type NativeSetActiveCamera = unsafe extern "C" fn(*mut c_void, NativeCameraHandle) -> i32;
pub type NativeClearActiveCamera =
    unsafe extern "C" fn(*mut c_void, *const NativeClearActiveCameraRequest) -> i32;
pub type NativeSetSkyBackground =
    unsafe extern "C" fn(*mut c_void, NativeRenderResourceHandle) -> i32;
pub type NativeClearSkyBackground =
    unsafe extern "C" fn(*mut c_void, *const NativeClearSkyBackgroundRequest) -> i32;
pub type NativeOpenUiStream = unsafe extern "C" fn(
    *mut c_void,
    *const NativeUiStreamRequest,
    *mut NativeUiStreamHandle,
) -> i32;
pub type NativeDestroyUiStream = unsafe extern "C" fn(*mut c_void, NativeUiStreamHandle) -> i32;
pub type NativePublishUiProjection = unsafe extern "C" fn(
    *mut c_void,
    *const NativeUiProjection,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyUiOperationDiagnosticLease =
    unsafe extern "C" fn(*mut c_void, NativeEngineDiagnosticLeaseHandle) -> i32;
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
pub type NativeOpenPersistenceStore = unsafe extern "C" fn(
    *mut c_void,
    *const NativePersistenceOpenRequest,
    *mut NativePersistenceStoreHandle,
) -> i32;
pub type NativeDestroyPersistenceStore =
    unsafe extern "C" fn(*mut c_void, NativePersistenceStoreHandle) -> i32;
pub type NativeSavePersistence = unsafe extern "C" fn(
    *mut c_void,
    *const NativePersistenceSaveRequest,
    *mut NativePersistenceSaveReceipt,
) -> i32;
pub type NativeLoadPersistence = unsafe extern "C" fn(
    *mut c_void,
    *const NativePersistenceLoadRequest,
    *mut NativePersistenceBlobHandle,
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
pub type NativeOpenContentStore = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContentStoreOpenRequest,
    *mut NativeContentStoreHandle,
) -> i32;
pub type NativeDestroyContentStore =
    unsafe extern "C" fn(*mut c_void, NativeContentStoreHandle) -> i32;
pub type NativeCaptureContentStoreSnapshot = unsafe extern "C" fn(
    *mut c_void,
    NativeContentStoreHandle,
    *mut NativeContentStoreSnapshotHandle,
) -> i32;
pub type NativeDestroyContentStoreSnapshot =
    unsafe extern "C" fn(*mut c_void, NativeContentStoreSnapshotHandle) -> i32;
pub type NativeReadContentStoreSnapshot = unsafe extern "C" fn(
    *mut c_void,
    NativeContentStoreSnapshotHandle,
    *mut NativeContentStoreSnapshotLease,
) -> i32;
pub type NativeDestroyContentStoreSnapshotLease =
    unsafe extern "C" fn(*mut c_void, NativeContentStoreSnapshotLeaseHandle) -> i32;
pub type NativeReadContentStoreBody = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContentStoreBodyRequest,
    *mut NativeByteLease,
) -> i32;
pub type NativeDestroyContentStoreByteLease =
    unsafe extern "C" fn(*mut c_void, NativeByteLeaseHandle) -> i32;
pub type NativePublishContentStore = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContentStorePublishRequest,
    *mut NativeContentStorePublishReceipt,
) -> i32;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeLookApi {
    pub context: *mut c_void,
    pub integrate: NativeIntegrateLook,
    pub reset: NativeResetLook,
    pub rebase: NativeRebaseLook,
    pub diagnose: NativeDiagnoseLook,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeDynamicsApi {
    pub context: *mut c_void,
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
    pub destroy_step_and_read_lease: NativeDestroyDynamicsStepAndReadLease,
    pub read: NativeReadDynamics,
    pub reset: NativeResetDynamics,
    pub update_body: NativeUpdateDynamicsBody,
    pub read_world: NativeReadDynamicsWorld,
    pub read_body_at: NativeReadDynamicsBodyAt,
    pub read_contact_at: NativeReadDynamicsContactAt,
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
    pub replace_content_artifact: NativeReplaceSpatialContentArtifact,
    pub read_content_artifact: NativeReadSpatialContentArtifact,
    pub replace_navigation: NativeReplaceNavigation,
    pub replace_voxel_navigation: NativeReplaceVoxelNavigation,
    pub replace_navigation_traversal: NativeReplaceNavigationTraversal,
    pub clear_navigation_traversal: NativeClearNavigationTraversal,
    pub replace_volumetric_navigation_traversal: NativeReplaceVolumetricNavigationTraversal,
    pub clear_volumetric_navigation_traversal: NativeClearVolumetricNavigationTraversal,
    pub read_navigation_projection: NativeReadNavigationProjection,
    pub request_navigation_path: NativeRequestNavigationPath,
    pub request_weighted_navigation_path: NativeRequestWeightedNavigationPath,
    pub request_weighted_volumetric_navigation_path: NativeRequestWeightedVolumetricNavigationPath,
    pub read_navigation_path_cell_at: NativeReadNavigationPathCellAt,
    pub request_volumetric_navigation_path: NativeRequestVolumetricNavigationPath,
    pub clear_navigation: NativeClearNavigation,
    pub default_character_controller_config: NativeDefaultCharacterControllerConfig,
    pub validate_character_controller_config: NativeValidateCharacterControllerConfig,
    pub validate_character_controller_command: NativeValidateCharacterControllerCommand,
    pub propose_character_step: NativeProposeCharacterStep,
    pub read_character_controller: NativeReadCharacterController,
    pub read_character_contact_at: NativeReadCharacterContactAt,
    pub read_character_dynamic_impulse_at: NativeReadCharacterDynamicImpulseAt,
    pub propose_navigation_step: NativeProposeNavigationStep,
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
    pub destroy_operation_diagnostic_lease: NativeDestroySpatialOperationDiagnosticLease,
    pub read_trigger: NativeSpatialReadTrigger,
    pub read_trigger_overlap_at: NativeSpatialReadTriggerOverlapAt,
    pub read_trigger_fact_at: NativeSpatialReadTriggerFactAt,
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
    pub read_affected_at: NativeWorldOriginReadAffectedAt,
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
    pub destroy_operation_diagnostic_lease: NativeDestroyUiOperationDiagnosticLease,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAppearanceApi {
    pub context: *mut c_void,
    pub open_resource: NativeOpenRenderResource,
    pub create_material: NativeCreateMaterial,
    pub update_material: NativeUpdateMaterial,
    pub replace_material: NativeReplaceMaterial,
    pub destroy_material: NativeDestroyMaterial,
    pub create_primitive: NativeCreatePrimitiveAppearance,
    pub replace_primitive: NativeReplacePrimitiveAppearance,
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
    pub read_sprite: NativeReadSprite,
    pub create_sprite_playback: NativeCreateSpritePlayback,
    pub destroy_sprite_playback: NativeDestroySpritePlayback,
    pub control_sprite_playback: NativeControlSpritePlayback,
    pub advance_sprite_playback: NativeAdvanceSpritePlayback,
    pub destroy_sprite_playback_advance_lease: NativeDestroySpritePlaybackAdvanceLease,
    pub sample_sprite_playback: NativeSampleSpritePlayback,
    pub read_sprite_playback: NativeReadSpritePlayback,
    pub destroy_appearance: NativeDestroyAppearance,
    pub publish_snapshot: NativePublishAppearanceSnapshot,
    pub create_light: NativeCreateLight,
    pub update_light: NativeUpdateLight,
    pub replace_light: NativeReplaceLight,
    pub destroy_light: NativeDestroyLight,
    pub read_light: NativeReadLight,
    pub read_presentation: NativeReadPresentation,
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
    pub read_diagnostic_at: NativeReadPresentationDiagnosticAt,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentApi {
    pub context: *mut c_void,
    pub open_reference: NativeOpenContentReference,
    pub resolve_reference: NativeResolveContentReference,
    pub destroy_reference: NativeDestroyContentReference,
    pub read_reference_info: NativeReadContentReferenceInfo,
    pub destroy_reference_info_lease: NativeDestroyContentReferenceInfoLease,
    pub read_bytes: NativeReadContentBytes,
    pub destroy_byte_lease: NativeDestroyContentByteLease,
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
    pub destroy_catalog_readout_lease: NativeDestroyAuthoredCatalogReadoutLease,
    pub publish_catalog_to_store: NativePublishAuthoredCatalogToStore,
    pub reopen_catalog_from_store: NativeReopenAuthoredCatalogFromStore,
    pub resolve_reference: NativeResolveAuthoredCatalogReference,
    pub destroy_resolved_entry_lease: NativeDestroyAuthoredResolvedEntryLease,
    pub resolve_material: NativeResolveAuthoredMaterial,
    pub destroy_material_resolution_lease: NativeDestroyAuthoredMaterialResolutionLease,
    pub resolve_voxel_surface: NativeResolveAuthoredVoxelSurface,
    pub destroy_voxel_surface_resolution_lease: NativeDestroyAuthoredVoxelSurfaceResolutionLease,
    pub resolve_fallback: NativeResolveAuthoredFallback,
    pub destroy_fallback_lease: NativeDestroyAuthoredFallbackLease,
    pub admit_prefab_registry: NativeAdmitAuthoredPrefabRegistry,
    pub admit_prefab_registry_from_content: NativeAdmitAuthoredPrefabRegistryFromContent,
    pub destroy_prefab_registry: NativeDestroyAuthoredPrefabRegistry,
    pub read_prefab_registry: NativeReadAuthoredPrefabRegistry,
    pub destroy_prefab_registry_readout_lease: NativeDestroyAuthoredPrefabRegistryReadoutLease,
    pub publish_prefab_registry_to_store: NativePublishAuthoredPrefabRegistryToStore,
    pub reopen_prefab_registry_from_store: NativeReopenAuthoredPrefabRegistryFromStore,
    pub resolve_prefab: NativeResolveAuthoredPrefab,
    pub destroy_resolved_prefab_lease: NativeDestroyAuthoredResolvedPrefabLease,
    pub prepare_scene: NativePrepareAuthoredScene,
    pub prepare_scene_from_content: NativePrepareAuthoredSceneFromContent,
    pub destroy_scene_plan: NativeDestroyAuthoredScenePlan,
    pub read_scene_plan: NativeReadAuthoredScenePlan,
    pub destroy_scene_plan_readout_lease: NativeDestroyAuthoredScenePlanReadoutLease,
    pub publish_scene_to_store: NativePublishAuthoredSceneToStore,
    pub prepare_scene_from_store: NativePrepareAuthoredSceneFromStore,
    pub destroy_operation_diagnostic_lease: NativeDestroyAuthoredContentOperationDiagnosticLease,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAnimationApi {
    pub context: *mut c_void,
    pub open_animated_mesh: NativeOpenAnimatedMesh,
    pub open_animation_clip_pack: NativeOpenAnimationClipPack,
    pub associate_animation_clip_pack: NativeAssociateAnimationClipPack,
    pub create_animated_mesh_appearance: NativeCreateAnimatedMeshAppearance,
    pub replace_animated_mesh_appearance: NativeReplaceAnimatedMeshAppearance,
    pub update_animated_mesh_materials: NativeUpdateAnimatedMeshMaterials,
    pub destroy_appearance: NativeDestroyAppearance,
    pub create_instance: NativeCreateAnimationInstance,
    pub destroy_instance: NativeDestroyAnimationInstance,
    pub replace_instance: NativeReplaceAnimationInstance,
    pub set_playback: NativeSetAnimationPlayback,
    pub replace_cue_definitions: NativeReplaceAnimationCueDefinitions,
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
    pub read_realization_fact_at: NativeReadAnimationRealizationFactAt,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeAudioApi {
    pub context: *mut c_void,
    pub open_clip: NativeOpenAudioClip,
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
    pub read_diagnostic_at: NativeReadAudioDiagnosticAt,
    pub read_realization: NativeReadAudioRealization,
    pub read_realization_fact_at: NativeReadAudioRealizationFactAt,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeCameraViewApi {
    pub context: *mut c_void,
    pub create_camera: NativeCreateCamera,
    pub update_camera: NativeUpdateCamera,
    pub replace_camera: NativeReplaceCamera,
    pub destroy_camera: NativeDestroyCamera,
    pub set_active_camera: NativeSetActiveCamera,
    pub clear_active_camera: NativeClearActiveCamera,
    pub set_sky_background: NativeSetSkyBackground,
    pub clear_sky_background: NativeClearSkyBackground,
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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativePersistenceApi {
    pub context: *mut c_void,
    pub open_store: NativeOpenPersistenceStore,
    pub destroy_store: NativeDestroyPersistenceStore,
    pub save: NativeSavePersistence,
    pub load: NativeLoadPersistence,
    pub destroy_blob: NativeDestroyPersistenceBlob,
    pub describe_blob: NativeDescribePersistenceBlob,
    pub copy_blob: NativeCopyPersistenceBlob,
    pub read_blob_bytes: NativeReadPersistenceBlobBytes,
    pub destroy_byte_lease: NativeDestroyPersistenceByteLease,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContentStoreApi {
    pub context: *mut c_void,
    pub open_store: NativeOpenContentStore,
    pub destroy_store: NativeDestroyContentStore,
    pub capture_snapshot: NativeCaptureContentStoreSnapshot,
    pub destroy_snapshot: NativeDestroyContentStoreSnapshot,
    pub read_snapshot: NativeReadContentStoreSnapshot,
    pub destroy_snapshot_lease: NativeDestroyContentStoreSnapshotLease,
    pub read_body: NativeReadContentStoreBody,
    pub destroy_byte_lease: NativeDestroyContentStoreByteLease,
    pub publish: NativePublishContentStore,
}

/// Direct named Engine service families available to trusted NativeAOT code.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeEngineApi {
    pub look: NativeLookApi,
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
    pub appearance: NativeAppearanceApi,
    pub presentation: NativePresentationApi,
    pub animation: NativeAnimationApi,
    pub audio: NativeAudioApi,
    pub camera_view: NativeCameraViewApi,
    pub rng: NativeRngApi,
    pub persistence: NativePersistenceApi,
    pub content_store: NativeContentStoreApi,
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

pub type NativeProductCreate =
    unsafe extern "C" fn(*const NativeProductCreateArgs, *mut *mut c_void) -> i32;
pub type NativeProductAction = unsafe extern "C" fn(*mut c_void) -> i32;
pub type NativeProductUpdate = unsafe extern "C" fn(
    *mut c_void,
    *const NativeProductUpdateArgs,
    *mut NativeProductUpdateResult,
) -> i32;
pub type NativeProductCompleteTimeline =
    unsafe extern "C" fn(*mut c_void, *const NativeProductTimelineCompletion, *mut u8) -> i32;
/// Acknowledge the final Engine transaction outcome to the managed product.
///
/// Generated C# lease wrappers defer their local disposed state until Rust has
/// committed the complete Engine call. `committed` is `0` for rollback and `1`
/// for commit. `terminal` is `1` only immediately before final product
/// destruction, when managed wrappers must become locally inert without
/// attempting another staged native destroy.
pub type NativeProductCompleteCall = unsafe extern "C" fn(*mut c_void, u8, u8);
/// Copies the Rust-owned lifecycle state after a host transition has committed.
///
/// This observer is optional so products generated before committed lifecycle
/// publication remain loadable. It is notification-only and cannot influence
/// the already committed host transition.
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

/// Product functions supplied to Rust by the one NativeAOT bootstrap export.
/// Nullable fields let Rust receive and inspect an initially empty table safely.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeProductApi {
    pub create:
        Option<unsafe extern "C" fn(*const NativeProductCreateArgs, *mut *mut c_void) -> i32>,
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
    pub complete_call: Option<unsafe extern "C" fn(*mut c_void, u8, u8)>,
    pub execute_debug: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const NativeUtf8Slice,
            *mut NativeProductDebugResult,
        ) -> i32,
    >,
    pub release_debug_result: Option<unsafe extern "C" fn(*mut c_void, NativeProductDebugResult)>,
    /// Appended after the established execute/release pair so products built
    /// against the preceding table keep their release callback offset.
    pub describe_debug:
        Option<unsafe extern "C" fn(*mut c_void, *mut NativeProductDebugResult) -> i32>,
    /// Appended optional committed-state observer. Rust calls this only after
    /// the authoritative lifecycle transition has committed.
    pub observe_runtime:
        Option<unsafe extern "C" fn(*mut c_void, *const NativeProductRuntimeFacts)>,
}

pub type NativeProductBind = unsafe extern "C" fn(*mut NativeProductApi) -> i32;
