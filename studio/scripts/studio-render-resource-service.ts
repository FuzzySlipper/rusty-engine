import { createHash } from 'node:crypto';
import { constants } from 'node:fs';
import { lstat, open } from 'node:fs/promises';
import {
  isAbsolute,
  join,
  normalize,
  parse,
  relative,
  resolve,
} from 'node:path';

// Render resources are read into one host Buffer and one browser ArrayBuffer.
// Keep that allocation bounded and aligned with the browser transport.
export const MAX_STUDIO_RENDER_RESOURCE_BYTES = 64 * 1024 * 1024;
// Real host paths stay far below this ceiling; it guards malformed inputs
// before path normalization and filesystem traversal.
export const MAX_STUDIO_RENDER_RESOURCE_PATH_BYTES = 4096;

export interface StudioRenderResourceRequest {
  readonly projectRoot: string;
  readonly sourcePath: string;
  readonly contentHash: string;
}

/**
 * Resolve one owner-declared renderer resource through the trusted Node host.
 * The browser never receives general filesystem access: callers must provide a
 * project-relative path and the exact SHA-256 already admitted by Rust.
 */
export async function readStudioRenderResource(
  request: StudioRenderResourceRequest,
): Promise<Buffer> {
  const projectRoot = checkedProjectRoot(request.projectRoot);
  const sourcePath = checkedSourcePath(request.sourcePath);
  const expectedHash = checkedContentHash(request.contentHash);
  const file = resolve(projectRoot, sourcePath);
  const fromRoot = relative(projectRoot, file);
  if (fromRoot.startsWith('..') || isAbsolute(fromRoot)) {
    throw new TypeError('Studio render resource must stay inside the project root.');
  }

  await requireExistingChainWithoutSymlinks(projectRoot);
  await requireExistingChainWithoutSymlinks(file);
  const handle = await open(file, constants.O_RDONLY | constants.O_NOFOLLOW);
  try {
    const metadata = await handle.stat();
    if (!metadata.isFile()) throw new TypeError('Studio render resource must be a regular file.');
    if (metadata.size > MAX_STUDIO_RENDER_RESOURCE_BYTES) {
      throw new TypeError('Studio render resource exceeds the byte bound.');
    }
    const bytes = await handle.readFile();
    if (bytes.byteLength > MAX_STUDIO_RENDER_RESOURCE_BYTES) {
      throw new TypeError('Studio render resource exceeds the byte bound.');
    }
    const actualHash = `sha256:${createHash('sha256').update(bytes).digest('hex')}`;
    if (actualHash !== expectedHash) {
      throw new TypeError('Studio render resource does not match its owner-admitted content hash.');
    }
    return bytes;
  } finally {
    await handle.close();
  }
}

function checkedProjectRoot(requested: string): string {
  if (requested.trim().length === 0
    || Buffer.byteLength(requested, 'utf8') > MAX_STUDIO_RENDER_RESOURCE_PATH_BYTES
    || requested.includes('\0')
    || !isAbsolute(requested)
    || normalize(requested) !== requested) {
    throw new TypeError('Project root must be a bounded, absolute, lexically normalized path.');
  }
  return requested;
}

function checkedSourcePath(requested: string): string {
  if (requested.trim().length === 0
    || Buffer.byteLength(requested, 'utf8') > MAX_STUDIO_RENDER_RESOURCE_PATH_BYTES
    || requested.includes('\0')
    || isAbsolute(requested)
    || normalize(requested) !== requested
    || requested === '..'
    || requested.startsWith('../')
    || !requested.toLocaleLowerCase().endsWith('.glb')) {
    throw new TypeError('Studio render resource must be a normalized project-relative GLB path.');
  }
  return requested;
}

function checkedContentHash(requested: string): string {
  if (!/^sha256:[0-9a-f]{64}$/u.test(requested)) {
    throw new TypeError('Studio render resource requires a lowercase SHA-256 content hash.');
  }
  return requested;
}

async function requireExistingChainWithoutSymlinks(path: string): Promise<void> {
  const root = parse(path).root;
  const relativeParts = path.slice(root.length).split('/').filter((part) => part.length > 0);
  let current = root;
  for (const part of relativeParts) {
    current = join(current, part);
    if ((await lstat(current)).isSymbolicLink()) {
      throw new TypeError(`Symbolic links are not accepted in Studio render paths: ${current}`);
    }
  }
}
