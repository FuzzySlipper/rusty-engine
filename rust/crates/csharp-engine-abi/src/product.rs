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
pub type NativeSpatialRegisterTrigger =
    unsafe extern "C" fn(*mut c_void, *const NativeSpatialTriggerRegisterRequest) -> i32;
pub type NativeSpatialReconcileTriggers = unsafe extern "C" fn(
    *mut c_void,
    *const NativeSpatialTriggerReconcileRequest,
    *mut NativeSpatialTriggerReceipt,
) -> i32;
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
pub type NativeReadPresentation =
    unsafe extern "C" fn(*mut c_void, *mut NativePresentationReadout) -> i32;
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
) -> i32;
pub type NativeDefineMechanicsStat =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsStatDefinitionRequest) -> i32;
pub type NativeDefineMechanicsTrack =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsTrackDefinitionRequest) -> i32;
pub type NativeDefineMechanicsContribution =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsContributionDefinitionRequest) -> i32;
pub type NativeDefineMechanicsSource =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsSourceDefinitionRequest) -> i32;
pub type NativeDefineMechanicsDamageKind =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsDamageKindDefinitionRequest) -> i32;
pub type NativeDefineMechanicsDamageResponse =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsDamageResponseDefinitionRequest) -> i32;
pub type NativeDefineMechanicsEffect =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsEffectDefinitionRequest) -> i32;
pub type NativeDefineMechanicsCapacityMetric =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsCapacityMetricDefinitionRequest) -> i32;
pub type NativeDefineMechanicsItem =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsItemDefinitionRequest) -> i32;
pub type NativeDefineMechanicsEquipmentSlot =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsEquipmentSlotDefinitionRequest) -> i32;
pub type NativeAdmitMechanicsCatalog =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsCatalogHandle) -> i32;
pub type NativeDestroyMechanicsCatalog =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsCatalogHandle) -> i32;
pub type NativeReadMechanicsCatalogIdentity = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsCatalogIdentityLease,
) -> i32;
pub type NativeReadMechanicsCatalogStats = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsStatCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogTracks = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsTrackCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogSources = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsSourceCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogStatContributions = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsStatContributionCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogDamageKinds = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsDamageKindCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogDamageResponses = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsDamageResponseCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogEffects = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsEffectCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogEffectSources = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsEffectSourceCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogCapacityMetrics = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsCapacityMetricCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogItems = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsItemCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogItemClassifications = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsItemClassificationCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogItemCapacityCosts = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsItemCapacityCostCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogItemEquipmentPolicies = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsItemEquipmentPolicyCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogItemSources = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsItemSourceCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogEquipmentSlots = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsEquipmentSlotCatalogLease,
) -> i32;
pub type NativeReadMechanicsCatalogSlotClassifications = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsCatalogHandle,
    *mut NativeMechanicsSlotClassificationCatalogLease,
) -> i32;
pub type NativeDestroyMechanicsCatalogLease =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsCatalogLeaseHandle) -> i32;
pub type NativeReadMechanicsStatComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsStatComponentLease,
) -> i32;
pub type NativeReadMechanicsTrackComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsTrackComponentLease,
) -> i32;
pub type NativeReadMechanicsIntrinsicSourceComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsIntrinsicSourceComponentLease,
) -> i32;
pub type NativeReadMechanicsActiveEffectComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsActiveEffectComponentLease,
) -> i32;
pub type NativeReadMechanicsInventoryStackComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsInventoryStackComponentLease,
) -> i32;
pub type NativeReadMechanicsInventoryCapacityLimitComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsInventoryCapacityLimitComponentLease,
) -> i32;
pub type NativeReadMechanicsItemComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsItemComponentLease,
) -> i32;
pub type NativeReadMechanicsEquipmentAssignmentComponent = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsEquipmentAssignmentComponentLease,
) -> i32;
pub type NativeDestroyMechanicsComponentLease =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsComponentLeaseHandle) -> i32;
pub type NativeBindMechanicsEntity = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEntityBindRequest,
    *mut NativeMechanicsEntityHandle,
) -> i32;
pub type NativeRebindMechanicsEntity = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEntityRebindRequest,
    *mut NativeMechanicsEntityHandle,
) -> i32;
pub type NativeSetMechanicsInitialStat =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsInitialStatRequest) -> i32;
pub type NativeSetMechanicsInitialTrack =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsInitialTrackRequest) -> i32;
pub type NativeBindMechanicsIntrinsicSource =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsIntrinsicSourceRequest) -> i32;
pub type NativeSetMechanicsInitialComponents =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsInitialComponentsRequest) -> i32;
pub type NativeStageMechanicsInitialContainment =
    unsafe extern "C" fn(*mut c_void, *const NativeMechanicsInitialContainmentRequest) -> i32;
pub type NativeReadMechanicsContainment = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsContainmentReadRequest,
    *mut NativeMechanicsContainmentReceipt,
) -> i32;
pub type NativeCommitMechanicsEntity = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsEntityReceipt,
) -> i32;
pub type NativeDestroyMechanicsEntity =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsEntityHandle) -> i32;
pub type NativeSetMechanicsEntityLifecycle = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsLifecycleRequest,
    *mut NativeMechanicsLifecycleReceipt,
) -> i32;
pub type NativeReadMechanicsStat = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsStatReadRequest,
    *mut NativeMechanicsStatReadReceipt,
) -> i32;
pub type NativeEvaluateMechanicsStat = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsStatOperationRequest,
    *mut NativeMechanicsStatEvaluationLease,
) -> i32;
pub type NativeReadMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsTrackReadRequest,
    *mut NativeMechanicsTrackReadLease,
) -> i32;
pub type NativeReadMechanicsInventoryView = unsafe extern "C" fn(
    *mut c_void,
    NativeMechanicsEntityHandle,
    *mut NativeMechanicsInventoryViewLease,
) -> i32;
pub type NativeGrantMechanicsInventory = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsInventoryMutationRequest,
    *mut NativeMechanicsInventoryMutationLease,
) -> i32;
pub type NativeConsumeMechanicsInventory = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsInventoryMutationRequest,
    *mut NativeMechanicsInventoryMutationLease,
) -> i32;
pub type NativeTransferMechanicsInventory = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsInventoryTransferRequest,
    *mut NativeMechanicsInventoryTransferLease,
) -> i32;
pub type NativeTransferMechanicsUniqueItem = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsUniqueItemTransferRequest,
    *mut NativeMechanicsUniqueItemTransferLease,
) -> i32;
pub type NativeMaterializeMechanicsUniqueItem = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsUniqueItemMaterializationRequest,
    *mut NativeMechanicsUniqueItemMaterializationLease,
) -> i32;
pub type NativeDestroyMechanicsUniqueItem = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsUniqueItemDestroyRequest,
    *mut NativeMechanicsUniqueItemDestroyLease,
) -> i32;
pub type NativeEquipMechanicsEquipment = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEquipmentEquipRequest,
    *mut NativeMechanicsEquipmentMutationLease,
) -> i32;
pub type NativeUnequipMechanicsEquipment = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEquipmentUnequipRequest,
    *mut NativeMechanicsEquipmentMutationLease,
) -> i32;
pub type NativeSwapMechanicsEquipment = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEquipmentSwapRequest,
    *mut NativeMechanicsEquipmentMutationLease,
) -> i32;
pub type NativeSetMechanicsStatBase = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsStatBaseMutationRequest,
    *mut NativeMechanicsStatMutationLease,
) -> i32;
pub type NativeDestroyMechanicsOperationLease =
    unsafe extern "C" fn(*mut c_void, NativeMechanicsOperationLeaseHandle) -> i32;
pub type NativeSetMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsTrackSetRequest,
    *mut NativeMechanicsTrackSetLease,
) -> i32;
pub type NativeSpendMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsTrackMutationRequest,
    *mut NativeMechanicsTrackMutationLease,
) -> i32;
pub type NativeRestoreMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsTrackMutationRequest,
    *mut NativeMechanicsTrackMutationLease,
) -> i32;
pub type NativeReconcileMechanicsTrack = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsTrackReconciliationRequest,
    *mut NativeMechanicsTrackReconciliationLease,
) -> i32;
pub type NativeApplyMechanicsEffect = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEffectMutationRequest,
    *mut NativeMechanicsEffectOperationLease,
) -> i32;
pub type NativeRefreshMechanicsEffect = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEffectRefreshRequest,
    *mut NativeMechanicsEffectOperationLease,
) -> i32;
pub type NativeReplaceMechanicsEffect = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEffectMutationRequest,
    *mut NativeMechanicsEffectOperationLease,
) -> i32;
pub type NativeRemoveMechanicsEffect = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEffectRemovalRequest,
    *mut NativeMechanicsEffectOperationLease,
) -> i32;
pub type NativeExpireMechanicsEffect = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsEffectRemovalRequest,
    *mut NativeMechanicsEffectOperationLease,
) -> i32;
pub type NativePreviewMechanicsDamage = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsDamageRequest,
    *mut NativeMechanicsDamageLease,
) -> i32;
pub type NativeApplyMechanicsDamage = unsafe extern "C" fn(
    *mut c_void,
    *const NativeMechanicsDamageRequest,
    *mut NativeMechanicsDamageLease,
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
    pub read_presentation: NativeReadPresentation,
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

/// Direct named Engine service families available to trusted NativeAOT code.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeEngineApi {
    pub look: NativeLookApi,
    pub dynamics: NativeDynamicsApi,
    pub spatial: NativeSpatialApi,
    pub voxel: NativeVoxelApi,
    pub voxel_content: NativeVoxelContentApi,
    pub appearance: NativeAppearanceApi,
    pub animation: NativeAnimationApi,
    pub audio: NativeAudioApi,
    pub camera_view: NativeCameraViewApi,
    pub rng: NativeRngApi,
    pub mechanics: NativeMechanicsApi,
    pub persistence: NativePersistenceApi,
    pub rules: NativeRulesApi,
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
