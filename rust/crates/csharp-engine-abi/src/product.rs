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
pub type NativeDestroyDynamicsBody =
    unsafe extern "C" fn(*mut c_void, NativeDynamicsBodyHandle) -> i32;
pub type NativeStepDynamics = unsafe extern "C" fn(
    *mut c_void,
    *const NativeDynamicsStepRequest,
    *mut NativeDynamicsStepReceipt,
) -> i32;
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
pub type NativeReplaceVoxelNavigation = unsafe extern "C" fn(
    *mut c_void,
    *const NativeNavigationVoxelReplaceRequest,
    *mut NativeNavigationReplaceReceipt,
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
pub type NativeProposeCharacterStep = unsafe extern "C" fn(
    *mut c_void,
    NativeCharacterStepRequest,
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
pub type NativeEmitAudio = unsafe extern "C" fn(*mut c_void, *const NativeAudioEmitRequest) -> i32;
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
pub type NativeReadAudio = unsafe extern "C" fn(*mut c_void, *mut NativeAudioReadout) -> i32;
pub type NativeReadAudioDiagnosticAt = unsafe extern "C" fn(
    *mut c_void,
    NativeAudioDiagnosticAtRequest,
    *mut NativeAudioDiagnosticAtReceipt,
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
pub type NativePublishUiProjection = unsafe extern "C" fn(
    *mut c_void,
    *const NativeUiProjection,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyUiOperationDiagnosticLease =
    unsafe extern "C" fn(*mut c_void, NativeEngineDiagnosticLeaseHandle) -> i32;
pub type NativeAdmitRulesPackage = unsafe extern "C" fn(
    *mut c_void,
    *const NativeRulesPackageAdmitRequest,
    *mut NativeRulesPackageHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyRulesPackage =
    unsafe extern "C" fn(*mut c_void, NativeRulesPackageHandle) -> i32;
pub type NativeReadRulesPackage = unsafe extern "C" fn(
    *mut c_void,
    NativeRulesPackageHandle,
    *mut NativeRulesPackageReadoutLease,
) -> i32;
pub type NativeDestroyRulesPackageReadoutLease =
    unsafe extern "C" fn(*mut c_void, NativeRulesPackageReadoutLeaseHandle) -> i32;
pub type NativeResolveRulesPackages = unsafe extern "C" fn(
    *mut c_void,
    *const NativeRulesResolvePackagesRequest,
    *mut NativeRulesResolvedPackageSetLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyRulesResolvedPackageSetLease =
    unsafe extern "C" fn(*mut c_void, NativeRulesResolvedPackageSetLeaseHandle) -> i32;
pub type NativeSelectRulesPayload = unsafe extern "C" fn(
    *mut c_void,
    *const NativeRulesSelectPayloadRequest,
    *mut NativeRulesPayloadSelectionLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyRulesPayloadSelectionLease =
    unsafe extern "C" fn(*mut c_void, NativeRulesPayloadSelectionLeaseHandle) -> i32;
pub type NativeDestroyRulesOperationDiagnosticLease =
    unsafe extern "C" fn(*mut c_void, NativeEngineDiagnosticLeaseHandle) -> i32;
pub type NativeAdmitStandardExact = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStandardExactAdmitRequest,
    *mut NativeStandardExactDefinitionHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyStandardExactDefinition =
    unsafe extern "C" fn(*mut c_void, NativeStandardExactDefinitionHandle) -> i32;
pub type NativeReadStandardExactDefinition = unsafe extern "C" fn(
    *mut c_void,
    NativeStandardExactDefinitionHandle,
    *mut NativeStandardExactReadoutLease,
) -> i32;
pub type NativeDestroyStandardExactReadoutLease =
    unsafe extern "C" fn(*mut c_void, NativeStandardExactReadoutLeaseHandle) -> i32;
pub type NativeEvaluateStandardExact = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStandardExactEvaluateRequest,
    *mut NativeStandardExactEvaluationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyStandardExactEvaluationLease =
    unsafe extern "C" fn(*mut c_void, NativeStandardExactEvaluationLeaseHandle) -> i32;
pub type NativeDestroyStandardExactOperationDiagnosticLease =
    unsafe extern "C" fn(*mut c_void, NativeEngineDiagnosticLeaseHandle) -> i32;
pub type NativeAdmitStandardExactPredicate = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStandardExactPredicateAdmitRequest,
    *mut NativeStandardExactPredicateHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyStandardExactPredicate =
    unsafe extern "C" fn(*mut c_void, NativeStandardExactPredicateHandle) -> i32;
pub type NativeReadStandardExactPredicate = unsafe extern "C" fn(
    *mut c_void,
    NativeStandardExactPredicateHandle,
    *mut NativeStandardExactPredicateReadoutLease,
) -> i32;
pub type NativeDestroyStandardExactPredicateReadoutLease =
    unsafe extern "C" fn(*mut c_void, NativeStandardExactPredicateReadoutLeaseHandle) -> i32;
pub type NativeEvaluateStandardExactPredicate = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStandardExactEvaluatePredicateRequest,
    *mut NativeStandardExactPredicateEvaluationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyStandardExactPredicateEvaluationLease =
    unsafe extern "C" fn(*mut c_void, NativeStandardExactPredicateEvaluationLeaseHandle) -> i32;
pub type NativeAdmitStandardContinuous = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStandardContinuousAdmitRequest,
    *mut NativeStandardContinuousDefinitionHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyStandardContinuousDefinition =
    unsafe extern "C" fn(*mut c_void, NativeStandardContinuousDefinitionHandle) -> i32;
pub type NativeReadStandardContinuousDefinition = unsafe extern "C" fn(
    *mut c_void,
    NativeStandardContinuousDefinitionHandle,
    *mut NativeStandardContinuousReadoutLease,
) -> i32;
pub type NativeDestroyStandardContinuousReadoutLease =
    unsafe extern "C" fn(*mut c_void, NativeStandardContinuousReadoutLeaseHandle) -> i32;
pub type NativeEvaluateStandardContinuous = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStandardContinuousEvaluateRequest,
    *mut NativeStandardContinuousEvaluationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyStandardContinuousEvaluationLease =
    unsafe extern "C" fn(*mut c_void, NativeStandardContinuousEvaluationLeaseHandle) -> i32;
pub type NativeDestroyStandardContinuousOperationDiagnosticLease =
    unsafe extern "C" fn(*mut c_void, NativeEngineDiagnosticLeaseHandle) -> i32;
pub type NativeAdmitStandardContinuousPredicate = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStandardContinuousPredicateAdmitRequest,
    *mut NativeStandardContinuousPredicateHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyStandardContinuousPredicate =
    unsafe extern "C" fn(*mut c_void, NativeStandardContinuousPredicateHandle) -> i32;
pub type NativeReadStandardContinuousPredicate = unsafe extern "C" fn(
    *mut c_void,
    NativeStandardContinuousPredicateHandle,
    *mut NativeStandardContinuousPredicateReadoutLease,
) -> i32;
pub type NativeDestroyStandardContinuousPredicateReadoutLease =
    unsafe extern "C" fn(*mut c_void, NativeStandardContinuousPredicateReadoutLeaseHandle) -> i32;
pub type NativeEvaluateStandardContinuousPredicate = unsafe extern "C" fn(
    *mut c_void,
    *const NativeStandardContinuousEvaluatePredicateRequest,
    *mut NativeStandardContinuousPredicateEvaluationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyStandardContinuousPredicateEvaluationLease = unsafe extern "C" fn(
    *mut c_void,
    NativeStandardContinuousPredicateEvaluationLeaseHandle,
) -> i32;
pub type NativeCreateContinuousMechanicsCatalog = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContinuousMechanicsCatalogCreateRequest,
    *mut NativeContinuousMechanicsCatalogHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyContinuousMechanicsCatalog = unsafe extern "C" fn(
    *mut c_void,
    NativeContinuousMechanicsCatalogHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadContinuousMechanicsCatalog = unsafe extern "C" fn(
    *mut c_void,
    NativeContinuousMechanicsCatalogHandle,
    *mut NativeContinuousMechanicsCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyContinuousMechanicsCatalogLease =
    unsafe extern "C" fn(*mut c_void, NativeContinuousMechanicsCatalogLeaseHandle) -> i32;
pub type NativeSetContinuousMechanicsInitialComponents = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContinuousMechanicsInitialComponentsRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadContinuousMechanicsComponents = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContinuousMechanicsComponentReadRequest,
    *mut NativeContinuousMechanicsComponentLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyContinuousMechanicsComponentLease =
    unsafe extern "C" fn(*mut c_void, NativeContinuousMechanicsComponentLeaseHandle) -> i32;
pub type NativeExportContinuousMechanicsWorld = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContinuousMechanicsWorldExportRequest,
    *mut NativeContinuousMechanicsWorldExportLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyContinuousMechanicsWorldExportLease =
    unsafe extern "C" fn(*mut c_void, NativeContinuousMechanicsWorldExportLeaseHandle) -> i32;
pub type NativeStageContinuousMechanicsWorldImport = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContinuousMechanicsWorldImportStageRequest,
    *mut NativeContinuousMechanicsWorldImportLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyContinuousMechanicsWorldImportLease =
    unsafe extern "C" fn(*mut c_void, NativeContinuousMechanicsWorldImportLeaseHandle) -> i32;
pub type NativeEvaluateContinuousMechanicsStat = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContinuousMechanicsStatEvaluateRequest,
    *mut NativeContinuousMechanicsStatEvaluationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetContinuousMechanicsStatBase = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContinuousMechanicsStatBaseMutationRequest,
    *mut NativeContinuousMechanicsStatMutationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadContinuousMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContinuousMechanicsTrackReadRequest,
    *mut NativeContinuousMechanicsTrackLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetContinuousMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContinuousMechanicsTrackSetRequest,
    *mut NativeContinuousMechanicsTrackLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSpendContinuousMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContinuousMechanicsTrackAdjustmentRequest,
    *mut NativeContinuousMechanicsTrackLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRestoreContinuousMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContinuousMechanicsTrackAdjustmentRequest,
    *mut NativeContinuousMechanicsTrackLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeApplyContinuousMechanicsEffect = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContinuousMechanicsEffectApplyRequest,
    *mut NativeContinuousMechanicsEffectLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRemoveContinuousMechanicsEffect = unsafe extern "C" fn(
    *mut c_void,
    *const NativeContinuousMechanicsEffectRemoveRequest,
    *mut NativeContinuousMechanicsEffectLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyContinuousMechanicsOperationLease =
    unsafe extern "C" fn(*mut c_void, NativeContinuousMechanicsOperationLeaseHandle) -> i32;
pub type NativeDestroyContinuousMechanicsOperationDiagnosticLease =
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
pub type NativeCreateMechanicsCatalog = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsCatalogCreateRequest,
    *mut NativeMechanicsCatalogHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefineMechanicsStat = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsStatDefinitionRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefineMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsTrackDefinitionRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefineMechanicsContribution = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsContributionDefinitionRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefineMechanicsSource = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsSourceDefinitionRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefineMechanicsDamageKind = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsDamageKindDefinitionRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefineMechanicsDamageResponse = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsDamageResponseDefinitionRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefineMechanicsEffect = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEffectDefinitionRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefineMechanicsCapacityMetric = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsCapacityMetricDefinitionRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefineMechanicsItem = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsItemDefinitionRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDefineMechanicsEquipmentSlot = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEquipmentSlotDefinitionRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeAdmitMechanicsCatalog = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMechanicsCatalog = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogIdentity = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsCatalogIdentityLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogStats = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsStatCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogTracks = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsTrackCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogSources = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsSourceCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogStatContributions = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsStatContributionCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogDamageKinds = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsDamageKindCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogDamageResponses = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsDamageResponseCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogEffects = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsEffectCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogEffectSources = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsEffectSourceCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogCapacityMetrics = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsCapacityMetricCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogItems = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsItemCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogItemClassifications = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsItemClassificationCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogItemCapacityCosts = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsItemCapacityCostCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogItemEquipmentPolicies = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsItemEquipmentPolicyCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogItemSources = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsItemSourceCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogEquipmentSlots = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsEquipmentSlotCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsCatalogSlotClassifications = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsSlotClassificationCatalogLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMechanicsCatalogLease =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsCatalogLeaseHandle) -> i32;
pub type NativeReadMechanicsStatComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsStatComponentLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsTrackComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsTrackComponentLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsIntrinsicSourceComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsIntrinsicSourceComponentLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsActiveEffectComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsActiveEffectComponentLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsInventoryStackComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsInventoryStackComponentLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsInventoryCapacityLimitComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsInventoryCapacityLimitComponentLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsItemComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsItemComponentLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsEquipmentAssignmentComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsEquipmentAssignmentComponentLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMechanicsComponentLease =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsComponentLeaseHandle) -> i32;
pub type NativeCaptureMechanicsWorldSnapshot = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsWorldSnapshotHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMechanicsWorldSnapshot =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsWorldSnapshotHandle) -> i32;
pub type NativeReadMechanicsWorldSnapshot = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsWorldSnapshotHandle,
    *mut NativeMechanicsWorldSnapshotLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMechanicsWorldSnapshotLease =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsWorldSnapshotLeaseHandle) -> i32;
pub type NativePrepareMechanicsWorldRestore = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsWorldRestoreRequest,
    *mut NativeMechanicsWorldRestoreHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMechanicsWorldRestore =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsWorldRestoreHandle) -> i32;
pub type NativeReadMechanicsWorldRestore = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsWorldRestoreHandle,
    *mut NativeMechanicsWorldRestoreLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMechanicsWorldRestoreLease =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsWorldRestoreLeaseHandle) -> i32;
pub type NativePublishMechanicsWorldRestore =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsWorldRestoreHandle) -> i32;
pub type NativeExportMechanicsWorld = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsWorldExportLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMechanicsWorldExportLease =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsWorldExportLeaseHandle) -> i32;
pub type NativePrepareMechanicsWorldImport = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsWorldImportRequest,
    *mut NativeMechanicsWorldImportHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMechanicsWorldImport =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsWorldImportHandle) -> i32;
pub type NativeReadMechanicsWorldImport = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsWorldImportHandle,
    *mut NativeMechanicsWorldImportLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMechanicsWorldImportLease =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsWorldImportLeaseHandle) -> i32;
pub type NativePublishMechanicsWorldImport =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsWorldImportHandle) -> i32;
pub type NativeClaimMechanicsWorldImportEntity = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsWorldImportEntityClaimRequest,
    *mut NativeMechanicsEntityHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeBindMechanicsEntity = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEntityBindRequest,
    *mut NativeMechanicsEntityHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRebindMechanicsEntity = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEntityRebindRequest,
    *mut NativeMechanicsEntityHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetMechanicsInitialStat = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsInitialStatRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetMechanicsInitialTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsInitialTrackRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeBindMechanicsIntrinsicSource = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsIntrinsicSourceRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetMechanicsInitialComponents = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsInitialComponentsRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeStageMechanicsInitialContainment = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsInitialContainmentRequest,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsContainment = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsContainmentReadRequest,
    *mut NativeMechanicsContainmentReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeCommitMechanicsEntity = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsEntityReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMechanicsEntity = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetMechanicsEntityLifecycle = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsLifecycleRequest,
    *mut NativeMechanicsLifecycleReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsStat = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsStatReadRequest,
    *mut NativeMechanicsStatReadReceipt,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeEvaluateMechanicsStat = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsStatOperationRequest,
    *mut NativeMechanicsStatEvaluationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsTrackReadRequest,
    *mut NativeMechanicsTrackReadLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReadMechanicsInventoryView = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsInventoryViewLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeGrantMechanicsInventory = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsInventoryMutationRequest,
    *mut NativeMechanicsInventoryMutationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeConsumeMechanicsInventory = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsInventoryMutationRequest,
    *mut NativeMechanicsInventoryMutationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeTransferMechanicsInventory = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsInventoryTransferRequest,
    *mut NativeMechanicsInventoryTransferLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeTransferMechanicsUniqueItem = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsUniqueItemTransferRequest,
    *mut NativeMechanicsUniqueItemTransferLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeMaterializeMechanicsUniqueItem = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsUniqueItemMaterializationRequest,
    *mut NativeMechanicsUniqueItemMaterializationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMechanicsUniqueItem = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsUniqueItemDestroyRequest,
    *mut NativeMechanicsUniqueItemDestroyLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeEquipMechanicsEquipment = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEquipmentEquipRequest,
    *mut NativeMechanicsEquipmentMutationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeUnequipMechanicsEquipment = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEquipmentUnequipRequest,
    *mut NativeMechanicsEquipmentMutationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSwapMechanicsEquipment = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEquipmentSwapRequest,
    *mut NativeMechanicsEquipmentMutationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSetMechanicsStatBase = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsStatBaseMutationRequest,
    *mut NativeMechanicsStatMutationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeDestroyMechanicsOperationLease =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsOperationLeaseHandle) -> i32;
pub type NativeDestroyMechanicsOperationDiagnosticLease =
    unsafe extern "C" fn(*mut c_void, NativeEngineDiagnosticLeaseHandle) -> i32;
pub type NativeSetMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsTrackSetRequest,
    *mut NativeMechanicsTrackSetLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeSpendMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsTrackMutationRequest,
    *mut NativeMechanicsTrackMutationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRestoreMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsTrackMutationRequest,
    *mut NativeMechanicsTrackMutationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReconcileMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsTrackReconciliationRequest,
    *mut NativeMechanicsTrackReconciliationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeApplyMechanicsEffect = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEffectMutationRequest,
    *mut NativeMechanicsEffectOperationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRefreshMechanicsEffect = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEffectRefreshRequest,
    *mut NativeMechanicsEffectOperationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeReplaceMechanicsEffect = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEffectMutationRequest,
    *mut NativeMechanicsEffectOperationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeRemoveMechanicsEffect = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEffectRemovalRequest,
    *mut NativeMechanicsEffectOperationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeExpireMechanicsEffect = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEffectRemovalRequest,
    *mut NativeMechanicsEffectOperationLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativePreviewMechanicsDamage = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsDamageRequest,
    *mut NativeMechanicsDamageLease,
    *mut NativeOperationErrorReceipt,
) -> i32;
pub type NativeApplyMechanicsDamage = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsDamageRequest,
    *mut NativeMechanicsDamageLease,
    *mut NativeOperationErrorReceipt,
) -> i32;

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
    pub destroy_body: NativeDestroyDynamicsBody,
    pub step: NativeStepDynamics,
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
pub struct NativeSpatialApi {
    pub context: *mut c_void,
    pub create_session: NativeCreateSpatialSession,
    pub destroy_session: NativeDestroySpatialSession,
    pub replace_collision: NativeReplaceCollision,
    pub replace_navigation: NativeReplaceNavigation,
    pub replace_voxel_navigation: NativeReplaceVoxelNavigation,
    pub read_navigation_projection: NativeReadNavigationProjection,
    pub request_navigation_path: NativeRequestNavigationPath,
    pub read_navigation_path_cell_at: NativeReadNavigationPathCellAt,
    pub request_volumetric_navigation_path: NativeRequestVolumetricNavigationPath,
    pub clear_navigation: NativeClearNavigation,
    pub default_character_controller_config: NativeDefaultCharacterControllerConfig,
    pub propose_character_step: NativeProposeCharacterStep,
    pub read_character_controller: NativeReadCharacterController,
    pub read_character_contact_at: NativeReadCharacterContactAt,
    pub read_character_dynamic_impulse_at: NativeReadCharacterDynamicImpulseAt,
    pub propose_navigation_step: NativeProposeNavigationStep,
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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeUiApi {
    pub context: *mut c_void,
    pub open_stream: NativeOpenUiStream,
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
pub struct NativeAnimationApi {
    pub context: *mut c_void,
    pub open_animated_mesh: NativeOpenAnimatedMesh,
    pub create_animated_mesh_appearance: NativeCreateAnimatedMeshAppearance,
    pub replace_animated_mesh_appearance: NativeReplaceAnimatedMeshAppearance,
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
    pub read: NativeReadAudio,
    pub read_diagnostic_at: NativeReadAudioDiagnosticAt,
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
pub struct NativeMechanicsApi {
    pub context: *mut c_void,
    pub create_catalog: NativeCreateMechanicsCatalog,
    pub define_stat: NativeDefineMechanicsStat,
    pub define_track: NativeDefineMechanicsTrack,
    pub define_contribution: NativeDefineMechanicsContribution,
    pub define_source: NativeDefineMechanicsSource,
    pub define_damage_kind: NativeDefineMechanicsDamageKind,
    pub define_damage_response: NativeDefineMechanicsDamageResponse,
    pub define_effect: NativeDefineMechanicsEffect,
    pub define_capacity_metric: NativeDefineMechanicsCapacityMetric,
    pub define_item: NativeDefineMechanicsItem,
    pub define_equipment_slot: NativeDefineMechanicsEquipmentSlot,
    pub admit_catalog: NativeAdmitMechanicsCatalog,
    pub destroy_catalog: NativeDestroyMechanicsCatalog,
    pub read_catalog_identity: NativeReadMechanicsCatalogIdentity,
    pub read_catalog_stats: NativeReadMechanicsCatalogStats,
    pub read_catalog_tracks: NativeReadMechanicsCatalogTracks,
    pub read_catalog_sources: NativeReadMechanicsCatalogSources,
    pub read_catalog_stat_contributions: NativeReadMechanicsCatalogStatContributions,
    pub read_catalog_damage_kinds: NativeReadMechanicsCatalogDamageKinds,
    pub read_catalog_damage_responses: NativeReadMechanicsCatalogDamageResponses,
    pub read_catalog_effects: NativeReadMechanicsCatalogEffects,
    pub read_catalog_effect_sources: NativeReadMechanicsCatalogEffectSources,
    pub read_catalog_capacity_metrics: NativeReadMechanicsCatalogCapacityMetrics,
    pub read_catalog_items: NativeReadMechanicsCatalogItems,
    pub read_catalog_item_classifications: NativeReadMechanicsCatalogItemClassifications,
    pub read_catalog_item_capacity_costs: NativeReadMechanicsCatalogItemCapacityCosts,
    pub read_catalog_item_equipment_policies: NativeReadMechanicsCatalogItemEquipmentPolicies,
    pub read_catalog_item_sources: NativeReadMechanicsCatalogItemSources,
    pub read_catalog_equipment_slots: NativeReadMechanicsCatalogEquipmentSlots,
    pub read_catalog_slot_classifications: NativeReadMechanicsCatalogSlotClassifications,
    pub destroy_catalog_lease: NativeDestroyMechanicsCatalogLease,
    pub read_stat_component: NativeReadMechanicsStatComponent,
    pub read_track_component: NativeReadMechanicsTrackComponent,
    pub read_intrinsic_source_component: NativeReadMechanicsIntrinsicSourceComponent,
    pub read_active_effect_component: NativeReadMechanicsActiveEffectComponent,
    pub read_inventory_stack_component: NativeReadMechanicsInventoryStackComponent,
    pub read_inventory_capacity_limit_component: NativeReadMechanicsInventoryCapacityLimitComponent,
    pub read_item_component: NativeReadMechanicsItemComponent,
    pub read_equipment_assignment_component: NativeReadMechanicsEquipmentAssignmentComponent,
    pub destroy_component_lease: NativeDestroyMechanicsComponentLease,
    pub capture_world_snapshot: NativeCaptureMechanicsWorldSnapshot,
    pub destroy_world_snapshot: NativeDestroyMechanicsWorldSnapshot,
    pub read_world_snapshot: NativeReadMechanicsWorldSnapshot,
    pub destroy_world_snapshot_lease: NativeDestroyMechanicsWorldSnapshotLease,
    pub prepare_world_restore: NativePrepareMechanicsWorldRestore,
    pub destroy_world_restore: NativeDestroyMechanicsWorldRestore,
    pub read_world_restore: NativeReadMechanicsWorldRestore,
    pub destroy_world_restore_lease: NativeDestroyMechanicsWorldRestoreLease,
    pub publish_world_restore: NativePublishMechanicsWorldRestore,
    pub export_world: NativeExportMechanicsWorld,
    pub destroy_world_export_lease: NativeDestroyMechanicsWorldExportLease,
    pub prepare_world_import: NativePrepareMechanicsWorldImport,
    pub destroy_world_import: NativeDestroyMechanicsWorldImport,
    pub read_world_import: NativeReadMechanicsWorldImport,
    pub destroy_world_import_lease: NativeDestroyMechanicsWorldImportLease,
    pub publish_world_import: NativePublishMechanicsWorldImport,
    pub claim_world_import_entity: NativeClaimMechanicsWorldImportEntity,
    pub bind_entity: NativeBindMechanicsEntity,
    pub rebind_entity: NativeRebindMechanicsEntity,
    pub set_initial_stat: NativeSetMechanicsInitialStat,
    pub set_initial_track: NativeSetMechanicsInitialTrack,
    pub bind_intrinsic_source: NativeBindMechanicsIntrinsicSource,
    pub set_initial_components: NativeSetMechanicsInitialComponents,
    pub stage_initial_containment: NativeStageMechanicsInitialContainment,
    pub read_containment: NativeReadMechanicsContainment,
    pub commit_entity: NativeCommitMechanicsEntity,
    pub set_entity_lifecycle: NativeSetMechanicsEntityLifecycle,
    pub destroy_entity: NativeDestroyMechanicsEntity,
    pub read_stat: NativeReadMechanicsStat,
    pub evaluate_stat: NativeEvaluateMechanicsStat,
    pub read_track: NativeReadMechanicsTrack,
    pub read_inventory_view: NativeReadMechanicsInventoryView,
    pub grant_inventory: NativeGrantMechanicsInventory,
    pub consume_inventory: NativeConsumeMechanicsInventory,
    pub transfer_inventory: NativeTransferMechanicsInventory,
    pub transfer_unique_item: NativeTransferMechanicsUniqueItem,
    pub materialize_unique_item: NativeMaterializeMechanicsUniqueItem,
    pub destroy_unique_item: NativeDestroyMechanicsUniqueItem,
    pub equip_equipment: NativeEquipMechanicsEquipment,
    pub unequip_equipment: NativeUnequipMechanicsEquipment,
    pub swap_equipment: NativeSwapMechanicsEquipment,
    pub set_stat_base: NativeSetMechanicsStatBase,
    pub destroy_operation_lease: NativeDestroyMechanicsOperationLease,
    pub destroy_operation_diagnostic_lease: NativeDestroyMechanicsOperationDiagnosticLease,
    pub set_track: NativeSetMechanicsTrack,
    pub spend_track: NativeSpendMechanicsTrack,
    pub restore_track: NativeRestoreMechanicsTrack,
    pub reconcile_track: NativeReconcileMechanicsTrack,
    pub apply_effect: NativeApplyMechanicsEffect,
    pub refresh_effect: NativeRefreshMechanicsEffect,
    pub replace_effect: NativeReplaceMechanicsEffect,
    pub remove_effect: NativeRemoveMechanicsEffect,
    pub expire_effect: NativeExpireMechanicsEffect,
    pub preview_damage: NativePreviewMechanicsDamage,
    pub apply_damage: NativeApplyMechanicsDamage,
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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeRulesApi {
    pub context: *mut c_void,
    pub admit_package: NativeAdmitRulesPackage,
    pub destroy_package: NativeDestroyRulesPackage,
    pub read_package: NativeReadRulesPackage,
    pub destroy_package_readout_lease: NativeDestroyRulesPackageReadoutLease,
    pub resolve_packages: NativeResolveRulesPackages,
    pub destroy_resolved_package_set_lease: NativeDestroyRulesResolvedPackageSetLease,
    pub select_payload: NativeSelectRulesPayload,
    pub destroy_payload_selection_lease: NativeDestroyRulesPayloadSelectionLease,
    pub destroy_operation_diagnostic_lease: NativeDestroyRulesOperationDiagnosticLease,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardExactApi {
    pub context: *mut c_void,
    pub admit: NativeAdmitStandardExact,
    pub destroy_definition: NativeDestroyStandardExactDefinition,
    pub read_definition: NativeReadStandardExactDefinition,
    pub destroy_readout_lease: NativeDestroyStandardExactReadoutLease,
    pub evaluate: NativeEvaluateStandardExact,
    pub destroy_evaluation_lease: NativeDestroyStandardExactEvaluationLease,
    pub destroy_operation_diagnostic_lease: NativeDestroyStandardExactOperationDiagnosticLease,
    pub admit_predicate: NativeAdmitStandardExactPredicate,
    pub destroy_predicate: NativeDestroyStandardExactPredicate,
    pub read_predicate: NativeReadStandardExactPredicate,
    pub destroy_predicate_readout_lease: NativeDestroyStandardExactPredicateReadoutLease,
    pub evaluate_predicate: NativeEvaluateStandardExactPredicate,
    pub destroy_predicate_evaluation_lease: NativeDestroyStandardExactPredicateEvaluationLease,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStandardContinuousApi {
    pub context: *mut c_void,
    pub admit: NativeAdmitStandardContinuous,
    pub destroy_definition: NativeDestroyStandardContinuousDefinition,
    pub read_definition: NativeReadStandardContinuousDefinition,
    pub destroy_readout_lease: NativeDestroyStandardContinuousReadoutLease,
    pub evaluate: NativeEvaluateStandardContinuous,
    pub destroy_evaluation_lease: NativeDestroyStandardContinuousEvaluationLease,
    pub destroy_operation_diagnostic_lease: NativeDestroyStandardContinuousOperationDiagnosticLease,
    pub admit_predicate: NativeAdmitStandardContinuousPredicate,
    pub destroy_predicate: NativeDestroyStandardContinuousPredicate,
    pub read_predicate: NativeReadStandardContinuousPredicate,
    pub destroy_predicate_readout_lease: NativeDestroyStandardContinuousPredicateReadoutLease,
    pub evaluate_predicate: NativeEvaluateStandardContinuousPredicate,
    pub destroy_predicate_evaluation_lease: NativeDestroyStandardContinuousPredicateEvaluationLease,
}

/// Typed continuous gameplay facts over the existing product entity binding.
/// The catalog has independent lifetime; entities and lifecycle remain owned by
/// the ordinary Mechanics/EntityWorld bridge.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeContinuousMechanicsApi {
    pub context: *mut c_void,
    pub create_catalog: NativeCreateContinuousMechanicsCatalog,
    pub destroy_catalog: NativeDestroyContinuousMechanicsCatalog,
    pub read_catalog: NativeReadContinuousMechanicsCatalog,
    pub destroy_catalog_lease: NativeDestroyContinuousMechanicsCatalogLease,
    pub set_initial_components: NativeSetContinuousMechanicsInitialComponents,
    pub read_components: NativeReadContinuousMechanicsComponents,
    pub destroy_component_lease: NativeDestroyContinuousMechanicsComponentLease,
    pub export_world: NativeExportContinuousMechanicsWorld,
    pub destroy_world_export_lease: NativeDestroyContinuousMechanicsWorldExportLease,
    pub stage_world_import: NativeStageContinuousMechanicsWorldImport,
    pub destroy_world_import_lease: NativeDestroyContinuousMechanicsWorldImportLease,
    pub evaluate_stat: NativeEvaluateContinuousMechanicsStat,
    pub set_stat_base: NativeSetContinuousMechanicsStatBase,
    pub read_track: NativeReadContinuousMechanicsTrack,
    pub set_track: NativeSetContinuousMechanicsTrack,
    pub spend_track: NativeSpendContinuousMechanicsTrack,
    pub restore_track: NativeRestoreContinuousMechanicsTrack,
    pub apply_effect: NativeApplyContinuousMechanicsEffect,
    pub remove_effect: NativeRemoveContinuousMechanicsEffect,
    pub destroy_operation_lease: NativeDestroyContinuousMechanicsOperationLease,
    pub destroy_operation_diagnostic_lease:
        NativeDestroyContinuousMechanicsOperationDiagnosticLease,
}

/// Direct named Engine service families available to trusted NativeAOT code.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeEngineApi {
    pub look: NativeLookApi,
    pub dynamics: NativeDynamicsApi,
    pub spatial: NativeSpatialApi,
    pub voxel: NativeVoxelApi,
    pub voxel_content: NativeVoxelContentApi,
    pub content: NativeContentApi,
    pub appearance: NativeAppearanceApi,
    pub presentation: NativePresentationApi,
    pub animation: NativeAnimationApi,
    pub audio: NativeAudioApi,
    pub camera_view: NativeCameraViewApi,
    pub rng: NativeRngApi,
    pub mechanics: NativeMechanicsApi,
    pub persistence: NativePersistenceApi,
    pub content_store: NativeContentStoreApi,
    pub rules: NativeRulesApi,
    pub resolution: NativeResolutionApi,
    pub state_machine: NativeStateMachineApi,
    pub standard_exact: NativeStandardExactApi,
    pub standard_continuous: NativeStandardContinuousApi,
    pub continuous_mechanics: NativeContinuousMechanicsApi,
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
