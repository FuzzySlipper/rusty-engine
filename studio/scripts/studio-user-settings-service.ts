import { createHash, randomUUID } from 'node:crypto';
import {
  access,
  lstat,
  mkdir,
  open,
  readFile,
  realpath,
  rename,
  unlink,
} from 'node:fs/promises';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';

import {
  parseStudioHostUserSettings,
} from '../libs/user-settings/src/index.js';

export const MAX_STUDIO_USER_SETTINGS_BYTES = 64 * 1024;

export interface StudioUserSettingsLocation {
  readonly canonicalProjectRoot: string;
  readonly projectKey: string;
  readonly settingsRoot: string;
  readonly path: string;
}

export interface StudioUserSettingsReadResult extends StudioUserSettingsLocation {
  readonly ok: true;
  readonly exists: boolean;
  readonly text: string | null;
  readonly sha256: string | null;
}

export type StudioUserSettingsWriteResult =
  | (StudioUserSettingsLocation & {
      readonly ok: true;
      readonly sha256: string;
    })
  | {
      readonly ok: false;
      readonly diagnostic:
        | 'invalid_user_settings'
        | 'project_key_mismatch'
        | 'stale_user_settings';
      readonly message: string;
    };

export function defaultStudioUserSettingsRoot(): string {
  const configured = process.env['XDG_CONFIG_HOME']?.trim();
  return resolve(
    configured === undefined || configured.length === 0
      ? join(homedir(), '.config')
      : configured,
    'rusty-engine-studio',
    'projects',
  );
}

export async function resolveStudioUserSettingsLocation(options: {
  readonly projectRoot: string;
  readonly settingsRoot?: string;
}): Promise<StudioUserSettingsLocation> {
  const requestedRoot = options.projectRoot.trim();
  if (requestedRoot.length === 0 || requestedRoot.length > 4096 || requestedRoot.includes('\0')) {
    throw new TypeError('A bounded, non-empty project root is required for host-user settings.');
  }
  const absoluteRoot = resolve(requestedRoot);
  let canonicalProjectRoot = absoluteRoot;
  try {
    canonicalProjectRoot = await realpath(absoluteRoot);
  } catch {
    // A root selected immediately before project creation still receives a stable normalized key.
  }
  const digest = createHash('sha256').update(canonicalProjectRoot).digest('hex');
  const settingsRoot = resolve(options.settingsRoot ?? defaultStudioUserSettingsRoot());
  return {
    canonicalProjectRoot,
    projectKey: `rusty-studio-project:${digest}`,
    settingsRoot,
    path: join(settingsRoot, `${digest}.json`),
  };
}

export async function readStudioUserSettings(options: {
  readonly projectRoot: string;
  readonly settingsRoot?: string;
}): Promise<StudioUserSettingsReadResult> {
  const location = await resolveStudioUserSettingsLocation(options);
  try {
    await access(location.path);
  } catch {
    return { ok: true, exists: false, ...location, text: null, sha256: null };
  }
  const metadata = await lstat(location.path);
  if (metadata.isSymbolicLink() || !metadata.isFile()) {
    throw new TypeError('Host-user settings path must be a regular non-symlink file.');
  }
  if (metadata.size > MAX_STUDIO_USER_SETTINGS_BYTES) {
    throw new TypeError('Host-user settings exceed the supported byte bound.');
  }
  const bytes = await readFile(location.path);
  if (bytes.byteLength > MAX_STUDIO_USER_SETTINGS_BYTES) {
    throw new TypeError('Host-user settings exceed the supported byte bound.');
  }
  return {
    ok: true,
    exists: true,
    ...location,
    text: bytes.toString('utf8'),
    sha256: sha256(bytes),
  };
}

export async function writeStudioUserSettings(options: {
  readonly projectRoot: string;
  readonly text: string;
  readonly expectedHash: string | null;
  readonly settingsRoot?: string;
}): Promise<StudioUserSettingsWriteResult> {
  const bytes = Buffer.from(options.text, 'utf8');
  if (bytes.byteLength > MAX_STUDIO_USER_SETTINGS_BYTES) {
    return {
      ok: false,
      diagnostic: 'invalid_user_settings',
      message: 'Host-user settings exceed the supported byte bound.',
    };
  }
  const location = await resolveStudioUserSettingsLocation(options);
  const parsed = parseStudioHostUserSettings(options.text);
  if (parsed.status !== 'loaded') {
    return {
      ok: false,
      diagnostic: 'invalid_user_settings',
      message: parsed.diagnostic,
    };
  }
  if (parsed.artifact.projectKey !== location.projectKey) {
    return {
      ok: false,
      diagnostic: 'project_key_mismatch',
      message: 'Host-user settings must be bound to the canonical project root key.',
    };
  }
  const before = await readStudioUserSettings(options);
  if (before.sha256 !== options.expectedHash) {
    return {
      ok: false,
      diagnostic: 'stale_user_settings',
      message: 'Host-user settings changed since they were loaded; reload before saving.',
    };
  }
  await mkdir(location.settingsRoot, { recursive: true, mode: 0o700 });
  const temporaryPath = join(location.settingsRoot, `.${randomUUID()}.tmp`);
  let candidateCreated = false;
  try {
    const candidate = await open(temporaryPath, 'wx', 0o600);
    candidateCreated = true;
    try {
      await candidate.writeFile(bytes);
      await candidate.sync();
    } finally {
      await candidate.close();
    }
    const current = await readStudioUserSettings(options);
    if (current.sha256 !== options.expectedHash) {
      return {
        ok: false,
        diagnostic: 'stale_user_settings',
        message: 'Host-user settings changed while a replacement was staged.',
      };
    }
    await rename(temporaryPath, location.path);
    candidateCreated = false;
    const readback = await readStudioUserSettings(options);
    if (readback.sha256 !== sha256(bytes)) {
      throw new Error('Host-user settings readback did not match the staged candidate.');
    }
    return { ok: true, ...location, sha256: readback.sha256 };
  } finally {
    if (candidateCreated) await unlink(temporaryPath).catch(() => undefined);
  }
}

function sha256(bytes: Uint8Array): string {
  return createHash('sha256').update(bytes).digest('hex');
}
