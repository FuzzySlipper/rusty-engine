import { rendererResourceContentHash as rendererHostResourceContentHash } from '@rusty-engine/renderer-host';

export {
  RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION,
  RustyApplicationHostError,
  mountRustyApplication,
} from './application-host.js';
export {
  RUSTY_APPLICATION_AUDIO_RESOURCE_MAX_BYTES,
  RUSTY_APPLICATION_AUDIO_RESOURCE_MAX_COUNT,
  RUSTY_APPLICATION_AUDIO_RESOURCE_MAX_TOTAL_BYTES,
  RustyApplicationContentError,
} from './application-content.js';
export function rendererResourceContentHash(
  data: ArrayBuffer,
  expected: string,
): Promise<string> {
  return rendererHostResourceContentHash(data, expected);
}
export type {
  RustyApplicationAudioResumeReceipt,
  RustyApplicationAudioDiagnostic,
  RustyApplicationAudioDiagnosticCode,
  RustyApplicationAudioRealizedFact,
  RustyApplicationAudioRealizedFactsReadout,
  RustyApplicationAnimationDiagnostic,
  RustyApplicationAnimationDiagnosticCode,
  RustyApplicationAnimationCueDefinition,
  RustyApplicationAnimationRealizedFact,
  RustyApplicationAnimationRealizedFactsReadout,
  RustyApplicationCameraPose,
  RustyApplicationFrame,
  RustyApplicationFrameDiagnostic,
  RustyApplicationFrameReceipt,
  RustyApplicationFogOptions,
  RustyApplicationGhostPlateReadout,
  RustyApplicationHost,
  RustyApplicationHostOptions,
  RustyApplicationHostReadout,
  RustyApplicationInteractionMode,
  RustyApplicationLightingOptions,
  RustyApplicationPresentationDiagnostic,
  RustyApplicationPresentationFrame,
  RustyApplicationPresentationReceipt,
  RustyApplicationViewComposition,
  RustyApplicationViewCompositionCamera,
  RustyApplicationViewCompositionPresentation,
  RustyApplicationViewCompositionReceipt,
  RustyApplicationViewCompositionTarget,
  RustyApplicationViewCompositionView,
  RustyApplicationViewCompositionViewport,
  RustyApplicationRendererOptions,
  RustyApplicationRendererPort,
  RustyApplicationUiContext,
  RustyApplicationUiIntentsPort,
  RustyApplicationUiMount,
  RustyApplicationUiOwner,
  RustyApplicationUiPort,
} from './application-host.js';
export {
  RUSTY_APPLICATION_UI_PROJECTION_ARTIFACT,
  RUSTY_APPLICATION_UI_PROJECTION_DEFAULT_STREAM,
  RUSTY_APPLICATION_UI_PROJECTION_MAX_ARRAY_LENGTH,
  RUSTY_APPLICATION_UI_PROJECTION_MAX_BYTES,
  RUSTY_APPLICATION_UI_PROJECTION_MAX_DEPTH,
  RUSTY_APPLICATION_UI_PROJECTION_MAX_NODES,
  RUSTY_APPLICATION_UI_PROJECTION_MAX_OBJECT_KEYS,
  RUSTY_APPLICATION_UI_PROJECTION_MAX_STRING_BYTES,
  RUSTY_APPLICATION_UI_PROJECTION_MAX_SUBSCRIBERS,
  RUSTY_APPLICATION_UI_PROJECTION_MAX_WIRE_BYTES,
  RustyApplicationUiProjectionError,
  createRustyApplicationUiProjection,
} from './ui-projection.js';
export type {
  RustyApplicationUiProjectionEnvelope,
  RustyApplicationUiProjectionErrorCode,
  RustyApplicationUiProjectionJson,
  RustyApplicationUiProjectionOptions,
  RustyApplicationUiProjectionPort,
  RustyApplicationUiProjectionReadout,
  RustyApplicationUiProjectionView,
} from './ui-projection.js';
export {
  RUSTY_APPLICATION_INPUT_POINTER_DELTA_MAXIMUM,
  RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_BYTES_MAXIMUM,
  RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_COLLECTION_MAXIMUM,
  RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_DEPTH_MAXIMUM,
  RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_NODES_MAXIMUM,
  RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_SAFE_INTEGER_MAXIMUM,
  RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_STRING_BYTES_MAXIMUM,
  RUSTY_APPLICATION_INPUT_QUEUE_MAXIMUM,
  RUSTY_APPLICATION_INPUT_SELECTED_CONTROLLER_MAXIMUM,
  RUSTY_APPLICATION_INPUT_U64_MAXIMUM,
  RUSTY_APPLICATION_INPUT_WHEEL_DELTA_MAXIMUM,
} from './input-ingress.js';
export type {
  RustyApplicationControllerAxis,
  RustyApplicationControllerButton,
  RustyApplicationInputClearReason,
  RustyApplicationInputEdge,
  RustyApplicationInputPort,
  RustyApplicationKeyboardControl,
  RustyApplicationPointerButton,
  RustyApplicationProductPayloadJson,
  RustyApplicationProductPayloadJsonObject,
  RustyApplicationRuntimeDirectIntentClaim,
  RustyApplicationRuntimeIdentity,
  RustyApplicationRuntimeInputBinding,
  RustyApplicationRuntimeInputEnvelope,
  RustyApplicationRuntimeInputFact,
  RustyApplicationRuntimeInputIngress,
  RustyApplicationRuntimeInputOptions,
  RustyApplicationRuntimeIntentValue,
  RustyApplicationSelectedControllerOptions,
} from './input-ingress.js';
export type {
  RustyApplicationPresentationAspectBounds,
} from './presentation-frame.js';
export type {
  RustyApplicationContent,
  RustyApplicationContentDiagnosticCode,
  RustyApplicationResource,
  RustyApplicationResourceKind,
} from './application-content.js';
