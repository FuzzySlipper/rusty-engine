import { lstat, readdir } from 'node:fs/promises';
import { dirname, isAbsolute, join, normalize, parse } from 'node:path';

// Directory reads are UI projections, not recursive inventory authority. The
// entry cap keeps one host-files response and one rendered list predictable;
// the path cap rejects malformed selections before filesystem traversal.
export const MAX_STUDIO_HOST_FILE_ENTRIES = 512;
export const MAX_STUDIO_HOST_PATH_BYTES = 4096;
export const MAX_STUDIO_HOST_FILE_EXTENSION_FILTERS = 16;
// Extension filters are ASCII-only, so characters, UTF-16 code units, and
// UTF-8 bytes are identical for this bound.
export const MAX_STUDIO_HOST_FILE_EXTENSION_CHARACTERS = 17;

export interface StudioHostFileEntry {
  readonly name: string;
  readonly path: string;
  readonly kind: 'directory' | 'file';
}

export interface StudioHostDirectoryReadout {
  readonly ok: true;
  readonly directory: string;
  readonly parent: string | null;
  readonly entries: readonly StudioHostFileEntry[];
  readonly truncated: boolean;
}

export async function listStudioHostDirectory(options: {
  readonly directory: string;
  readonly extensions?: readonly string[];
}): Promise<StudioHostDirectoryReadout> {
  const directory = checkedAbsolutePath(options.directory);
  await requireExistingChainWithoutSymlinks(directory);
  const metadata = await lstat(directory);
  if (!metadata.isDirectory()) throw new TypeError('Selected host path is not a directory.');
  const extensions = normalizeExtensions(options.extensions ?? []);
  const entries = (await readdir(directory, { withFileTypes: true }))
    .filter((entry) => !entry.isSymbolicLink())
    .flatMap((entry): StudioHostFileEntry[] => {
      if (entry.isDirectory()) {
        return [{ name: entry.name, path: join(directory, entry.name), kind: 'directory' }];
      }
      if (!entry.isFile()) return [];
      if (extensions.length > 0 && !extensions.some(
        (extension) => entry.name.toLocaleLowerCase().endsWith(extension),
      )) {
        return [];
      }
      return [{ name: entry.name, path: join(directory, entry.name), kind: 'file' }];
    })
    .sort((left, right) => left.kind === right.kind
      ? left.name.localeCompare(right.name)
      : left.kind === 'directory' ? -1 : 1);
  return {
    ok: true,
    directory,
    parent: directory === parse(directory).root ? null : dirname(directory),
    entries: entries.slice(0, MAX_STUDIO_HOST_FILE_ENTRIES),
    truncated: entries.length > MAX_STUDIO_HOST_FILE_ENTRIES,
  };
}

function checkedAbsolutePath(requested: string): string {
  if (requested.trim().length === 0
    || Buffer.byteLength(requested, 'utf8') > MAX_STUDIO_HOST_PATH_BYTES
    || requested.includes('\0')
    || !isAbsolute(requested)
    || normalize(requested) !== requested) {
    throw new TypeError('Host directory must be a bounded, absolute, lexically normalized path.');
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
      throw new TypeError(`Symbolic links are not accepted in trusted host paths: ${current}`);
    }
  }
}

function normalizeExtensions(values: readonly string[]): readonly string[] {
  if (values.length > MAX_STUDIO_HOST_FILE_EXTENSION_FILTERS) {
    throw new TypeError('Host file extension filter is too broad.');
  }
  return values.map((value) => {
    const extension = value.trim().toLocaleLowerCase();
    if (extension.length > MAX_STUDIO_HOST_FILE_EXTENSION_CHARACTERS
      || !/^\.[a-z0-9][a-z0-9._-]*$/.test(extension)) {
      throw new TypeError(`Invalid host file extension filter: ${value}`);
    }
    return extension;
  });
}
