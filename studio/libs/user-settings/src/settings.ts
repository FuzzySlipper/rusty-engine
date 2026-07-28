export const RUSTY_STUDIO_HOST_USER_SETTINGS_VERSION =
  'rusty-engine-studio-host-user-settings.v1' as const;

// Persisted identifiers and bindings remain compact even when authored with
// multibyte text; measure their serialized UTF-8 representation.
const MAX_PROJECT_KEY_BYTES = 160;
const MAX_KEYBOARD_BINDING_BYTES = 64;

export interface StudioKeyboardBindings {
  readonly moveForward: string;
  readonly moveBackward: string;
  readonly moveLeft: string;
  readonly moveRight: string;
  readonly moveDown: string;
  readonly moveUp: string;
  readonly boost: string;
}

export interface StudioHostUserSettingsArtifact {
  readonly schemaVersion: 1;
  readonly artifactKind: 'rusty_engine_studio_host_user_settings';
  readonly settingsVersion: typeof RUSTY_STUDIO_HOST_USER_SETTINGS_VERSION;
  readonly projectKey: string;
  readonly theme: 'graphite' | 'highContrast';
  readonly editor: {
    readonly snappingEnabled: boolean;
    readonly translationSnap: number;
    readonly translationSnapAxes: readonly [number, number, number];
    readonly rotationSnapDegrees: number;
    readonly scaleSnapAxes: readonly [number, number, number];
    readonly fineMultiplier: number;
    readonly transformOrientation: 'world' | 'local';
  };
  readonly sceneView: {
    readonly lightingMode: 'work_light' | 'authored_lights';
    readonly gridVisible: boolean;
    readonly minorColor: readonly [number, number, number, number];
    readonly majorColor: readonly [number, number, number, number];
    readonly xAxisColor: readonly [number, number, number, number];
    readonly yAxisColor: readonly [number, number, number, number];
    readonly zAxisColor: readonly [number, number, number, number];
    readonly majorLineEvery: number;
    readonly opacity: number;
    readonly fadeStart: number;
    readonly fadeEnd: number;
    readonly cameraMoveSpeed: number;
    readonly cameraBoostMultiplier: number;
    readonly invertLookY: boolean;
    readonly invertPanY: boolean;
  };
  readonly keyboard: StudioKeyboardBindings;
}

export type StudioUserSettingsParseResult =
  | {
      readonly status: 'loaded';
      readonly artifact: StudioHostUserSettingsArtifact;
      readonly preservedRawText: null;
      readonly diagnostic: null;
    }
  | {
      readonly status: 'unsupported_future_version' | 'invalid';
      readonly artifact: null;
      readonly preservedRawText: string;
      readonly diagnostic: string;
    };

const DEFAULT_KEYBOARD_BINDINGS: StudioKeyboardBindings = {
  moveForward: 'KeyW',
  moveBackward: 'KeyS',
  moveLeft: 'KeyA',
  moveRight: 'KeyD',
  moveDown: 'KeyQ',
  moveUp: 'KeyE',
  boost: 'ShiftLeft',
};

export function buildDefaultStudioHostUserSettings(
  projectKey: string,
): StudioHostUserSettingsArtifact {
  return {
    schemaVersion: 1,
    artifactKind: 'rusty_engine_studio_host_user_settings',
    settingsVersion: RUSTY_STUDIO_HOST_USER_SETTINGS_VERSION,
    projectKey: requireNonEmpty(projectKey, 'host project key'),
    theme: 'graphite',
    editor: {
      snappingEnabled: true,
      translationSnap: 0.5,
      translationSnapAxes: [0.5, 0.5, 0.5],
      rotationSnapDegrees: 15,
      scaleSnapAxes: [0.1, 0.1, 0.1],
      fineMultiplier: 0.1,
      transformOrientation: 'world',
    },
    sceneView: {
      lightingMode: 'work_light',
      gridVisible: true,
      minorColor: [0.24, 0.4, 0.42, 0.36],
      majorColor: [0.32, 0.58, 0.58, 0.62],
      xAxisColor: [0.86, 0.28, 0.26, 0.92],
      yAxisColor: [0.28, 0.82, 0.46, 0.92],
      zAxisColor: [0.28, 0.5, 0.9, 0.92],
      majorLineEvery: 4,
      opacity: 0.82,
      fadeStart: 22,
      fadeEnd: 62,
      cameraMoveSpeed: 6,
      cameraBoostMultiplier: 4,
      invertLookY: false,
      invertPanY: false,
    },
    keyboard: { ...DEFAULT_KEYBOARD_BINDINGS },
  };
}

export function parseStudioHostUserSettings(text: string): StudioUserSettingsParseResult {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text) as unknown;
  } catch {
    return rejected(
      'invalid',
      text,
      'Settings are not valid JSON; the original text was preserved and writes are disabled.',
    );
  }
  if (!isRecord(parsed) || parsed['artifactKind'] !== 'rusty_engine_studio_host_user_settings') {
    return rejected(
      'invalid',
      text,
      'Settings do not contain a Rusty Engine Studio host-user artifact.',
    );
  }
  if (parsed['settingsVersion'] !== RUSTY_STUDIO_HOST_USER_SETTINGS_VERSION) {
    return rejected(
      'unsupported_future_version',
      text,
      `Unsupported settings version ${String(parsed['settingsVersion'])}; the original text was preserved and writes are disabled.`,
    );
  }
  try {
    return {
      status: 'loaded',
      artifact: validateStudioHostUserSettings(parsed),
      preservedRawText: null,
      diagnostic: null,
    };
  } catch (error) {
    return rejected(
      'invalid',
      text,
      error instanceof Error ? error.message : 'Host-user settings are invalid.',
    );
  }
}

export function serializeStudioHostUserSettings(
  artifact: StudioHostUserSettingsArtifact,
): string {
  validateStudioHostUserSettings(artifact);
  return `${JSON.stringify(artifact, null, 2)}\n`;
}

export function validateStudioHostUserSettings(
  value: unknown,
): StudioHostUserSettingsArtifact {
  if (!isRecord(value)
    || value['schemaVersion'] !== 1
    || value['artifactKind'] !== 'rusty_engine_studio_host_user_settings'
    || value['settingsVersion'] !== RUSTY_STUDIO_HOST_USER_SETTINGS_VERSION) {
    throw new TypeError('Host-user settings must use the supported Rusty Engine Studio v1 schema.');
  }
  const projectKey = requireNonEmpty(value['projectKey'], 'host project key');
  if (new TextEncoder().encode(projectKey).byteLength > MAX_PROJECT_KEY_BYTES) {
    throw new TypeError(`Host project key exceeds its ${String(MAX_PROJECT_KEY_BYTES)}-byte bound.`);
  }
  const theme = value['theme'];
  if (theme !== 'graphite' && theme !== 'highContrast') {
    throw new TypeError('Studio theme must be graphite or highContrast.');
  }
  const editor = requireRecord(value['editor'], 'editor settings');
  const snappingEnabled = requireBoolean(editor['snappingEnabled'], 'snapping enabled');
  const translationSnap = requirePositiveFinite(editor['translationSnap'], 'translation snap');
  const translationSnapAxes = editor['translationSnapAxes'] === undefined
    ? [translationSnap, translationSnap, translationSnap] as const
    : requirePositiveVector3(editor['translationSnapAxes'], 'translation snap axes');
  const rotationSnapDegrees = editor['rotationSnapDegrees'] === undefined
    ? 15
    : requirePositiveFinite(editor['rotationSnapDegrees'], 'rotation snap');
  const scaleSnapAxes = editor['scaleSnapAxes'] === undefined
    ? [0.1, 0.1, 0.1] as const
    : requirePositiveVector3(editor['scaleSnapAxes'], 'scale snap axes');
  const fineMultiplier = editor['fineMultiplier'] === undefined
    ? 0.1
    : requirePositiveUnit(editor['fineMultiplier'], 'fine transform multiplier');
  const transformOrientation = editor['transformOrientation'] === undefined
    ? 'world'
    : editor['transformOrientation'];
  if (transformOrientation !== 'world' && transformOrientation !== 'local') {
    throw new TypeError('Transform orientation must be world or local.');
  }
  const sceneView = requireRecord(value['sceneView'], 'scene-view settings');
  const lightingMode = sceneView['lightingMode'] === undefined
    ? 'work_light'
    : sceneView['lightingMode'];
  if (lightingMode !== 'work_light' && lightingMode !== 'authored_lights') {
    throw new TypeError('Studio lighting mode must be work_light or authored_lights.');
  }
  const gridVisible = requireBoolean(sceneView['gridVisible'], 'grid visibility');
  const minorColor = requireColor(sceneView['minorColor'], 'minor grid color');
  const majorColor = requireColor(sceneView['majorColor'], 'major grid color');
  const xAxisColor = requireColor(sceneView['xAxisColor'], 'X-axis grid color');
  const yAxisColor = requireColor(sceneView['yAxisColor'], 'Y-axis grid color');
  const zAxisColor = requireColor(sceneView['zAxisColor'], 'Z-axis grid color');
  const majorLineEvery = requirePositiveInteger(sceneView['majorLineEvery'], 'major line interval');
  const opacity = requireUnitInterval(sceneView['opacity'], 'grid opacity');
  const fadeStart = requireNonNegativeFinite(sceneView['fadeStart'], 'grid fade start');
  const fadeEnd = requirePositiveFinite(sceneView['fadeEnd'], 'grid fade end');
  if (fadeEnd <= fadeStart) throw new TypeError('Grid fade end must be greater than fade start.');
  const cameraMoveSpeed = requirePositiveFinite(sceneView['cameraMoveSpeed'], 'camera move speed');
  const cameraBoostMultiplier = requirePositiveFinite(
    sceneView['cameraBoostMultiplier'],
    'camera boost multiplier',
  );
  if (cameraBoostMultiplier < 1) {
    throw new TypeError('Camera boost multiplier must be at least one.');
  }
  const invertLookY = requireBoolean(sceneView['invertLookY'], 'look inversion');
  const invertPanY = requireBoolean(sceneView['invertPanY'], 'pan inversion');
  const keyboard = requireRecord(value['keyboard'], 'keyboard settings');
  const bindings: StudioKeyboardBindings = {
    moveForward: requireBinding(keyboard['moveForward'], 'move forward'),
    moveBackward: requireBinding(keyboard['moveBackward'], 'move backward'),
    moveLeft: requireBinding(keyboard['moveLeft'], 'move left'),
    moveRight: requireBinding(keyboard['moveRight'], 'move right'),
    moveDown: requireBinding(keyboard['moveDown'], 'move down'),
    moveUp: requireBinding(keyboard['moveUp'], 'move up'),
    boost: requireBinding(keyboard['boost'], 'boost'),
  };
  return {
    schemaVersion: 1,
    artifactKind: 'rusty_engine_studio_host_user_settings',
    settingsVersion: RUSTY_STUDIO_HOST_USER_SETTINGS_VERSION,
    projectKey,
    theme,
    editor: {
      snappingEnabled,
      translationSnap,
      translationSnapAxes,
      rotationSnapDegrees,
      scaleSnapAxes,
      fineMultiplier,
      transformOrientation,
    },
    sceneView: {
      lightingMode,
      gridVisible,
      minorColor,
      majorColor,
      xAxisColor,
      yAxisColor,
      zAxisColor,
      majorLineEvery,
      opacity,
      fadeStart,
      fadeEnd,
      cameraMoveSpeed,
      cameraBoostMultiplier,
      invertLookY,
      invertPanY,
    },
    keyboard: bindings,
  };
}

function rejected(
  status: 'unsupported_future_version' | 'invalid',
  text: string,
  diagnostic: string,
): StudioUserSettingsParseResult {
  return { status, artifact: null, preservedRawText: text, diagnostic };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function requireRecord(value: unknown, label: string): Record<string, unknown> {
  if (!isRecord(value)) throw new TypeError(`${label} must be an object.`);
  return value;
}

function requireNonEmpty(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.trim().length === 0 || value.includes('\0')) {
    throw new TypeError(`${label} must be a non-empty string.`);
  }
  return value;
}

function requireBinding(value: unknown, label: string): string {
  const binding = requireNonEmpty(value, `${label} keyboard binding`);
  if (new TextEncoder().encode(binding).byteLength > MAX_KEYBOARD_BINDING_BYTES) {
    throw new TypeError(
      `${label} keyboard binding exceeds its ${String(MAX_KEYBOARD_BINDING_BYTES)}-byte bound.`,
    );
  }
  return binding;
}

function requireBoolean(value: unknown, label: string): boolean {
  if (typeof value !== 'boolean') throw new TypeError(`${label} must be boolean.`);
  return value;
}

function requirePositiveFinite(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value <= 0) {
    throw new TypeError(`${label} must be finite and positive.`);
  }
  return value;
}

function requireNonNegativeFinite(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0) {
    throw new TypeError(`${label} must be finite and non-negative.`);
  }
  return value;
}

function requirePositiveInteger(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 1) {
    throw new TypeError(`${label} must be a positive integer.`);
  }
  return value;
}

function requireUnitInterval(value: unknown, label: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > 1) {
    throw new TypeError(`${label} must be between zero and one.`);
  }
  return value;
}

function requirePositiveUnit(value: unknown, label: string): number {
  const result = requirePositiveFinite(value, label);
  if (result > 1) throw new TypeError(`${label} must be no greater than one.`);
  return result;
}

function requirePositiveVector3(
  value: unknown,
  label: string,
): readonly [number, number, number] {
  if (!Array.isArray(value) || value.length !== 3) {
    throw new TypeError(`${label} must contain three finite positive values.`);
  }
  return [
    requirePositiveFinite(value[0], `${label} X`),
    requirePositiveFinite(value[1], `${label} Y`),
    requirePositiveFinite(value[2], `${label} Z`),
  ];
}

function requireColor(
  value: unknown,
  label: string,
): readonly [number, number, number, number] {
  if (!Array.isArray(value) || value.length !== 4 || !value.every(
    (entry) => typeof entry === 'number' && Number.isFinite(entry) && entry >= 0 && entry <= 1,
  )) {
    throw new TypeError(`${label} must contain four finite values from zero through one.`);
  }
  return [value[0] as number, value[1] as number, value[2] as number, value[3] as number];
}
