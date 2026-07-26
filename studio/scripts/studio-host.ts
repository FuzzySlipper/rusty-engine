import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { createReadStream } from 'node:fs';
import { stat } from 'node:fs/promises';
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import {
  extname,
  isAbsolute,
  relative,
  resolve,
} from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  MAX_STUDIO_ADAPTER_REQUEST_BYTES,
  MAX_STUDIO_ADAPTER_RESPONSE_BYTES,
} from '../libs/adapter-client/src/index.js';
import {
  MAX_STUDIO_USER_SETTINGS_BYTES,
  defaultStudioUserSettingsRoot,
  readStudioUserSettings,
  writeStudioUserSettings,
} from './studio-user-settings-service.js';
import { listStudioHostDirectory } from './studio-host-files-service.js';
import { readStudioRenderResource } from './studio-render-resource-service.js';

const DEFAULT_STATIC_ROOT = fileURLToPath(
  new URL('../dist/apps/studio-app/browser/', import.meta.url),
);
const MAX_ERROR_BYTES = 64 * 1024;

interface HostOptions {
  readonly adapterBinary: string;
  readonly staticRoot: string;
  readonly host: string;
  readonly port: number;
  readonly settingsRoot: string;
}

interface PendingExchange {
  readonly resolve: (line: string) => void;
  readonly reject: (error: Error) => void;
}

class AdapterProcess {
  readonly #child: ChildProcessWithoutNullStreams;
  #pending: PendingExchange | null = null;
  #stdoutBuffer = Buffer.alloc(0);
  #stderr = '';
  #serial: Promise<void> = Promise.resolve();
  #closedError: Error | null = null;

  constructor(binary: string) {
    this.#child = spawn(binary, [], { stdio: ['pipe', 'pipe', 'pipe'] });
    this.#child.stdout.on('data', (chunk: Buffer) => this.#receive(chunk));
    this.#child.stderr.setEncoding('utf8');
    this.#child.stderr.on('data', (chunk: string) => {
      this.#stderr = `${this.#stderr}${chunk}`.slice(-MAX_ERROR_BYTES);
    });
    this.#child.on('error', (error) => this.#fail(error));
    this.#child.on('exit', (code, signal) => {
      this.#fail(new Error(
        `Studio adapter exited code=${String(code)} signal=${String(signal)}${
          this.#stderr.length === 0 ? '' : `: ${this.#stderr}`
        }`,
      ));
    });
  }

  exchange(requestLine: string): Promise<string> {
    const exchange = this.#serial.then(() => this.#exchangeOne(requestLine));
    this.#serial = exchange.then(() => undefined, () => undefined);
    return exchange;
  }

  close(): void {
    if (!this.#child.stdin.destroyed) this.#child.stdin.end();
  }

  #exchangeOne(requestLine: string): Promise<string> {
    if (this.#closedError !== null) return Promise.reject(this.#closedError);
    if (this.#pending !== null) {
      return Promise.reject(new Error('Studio adapter exchange overlap'));
    }
    return new Promise((resolvePromise, rejectPromise) => {
      this.#pending = { resolve: resolvePromise, reject: rejectPromise };
      this.#child.stdin.write(`${requestLine}\n`, (error) => {
        if (error !== null && error !== undefined) this.#fail(error);
      });
    });
  }

  #receive(chunk: Buffer): void {
    this.#stdoutBuffer = Buffer.concat([this.#stdoutBuffer, chunk]);
    const newline = this.#stdoutBuffer.indexOf(0x0a);
    if (newline === -1) {
      if (this.#stdoutBuffer.byteLength > MAX_STUDIO_ADAPTER_RESPONSE_BYTES) {
        this.#fail(new Error('Studio adapter response exceeds the protocol byte bound'));
        this.#child.kill();
      }
      return;
    }

    const responseBytes = this.#stdoutBuffer.subarray(0, newline);
    this.#stdoutBuffer = this.#stdoutBuffer.subarray(newline + 1);
    if (responseBytes.byteLength > MAX_STUDIO_ADAPTER_RESPONSE_BYTES) {
      this.#fail(new Error('Studio adapter response exceeds the protocol byte bound'));
      this.#child.kill();
      return;
    }
    const pending = this.#pending;
    this.#pending = null;
    if (pending === null) {
      this.#fail(new Error('Studio adapter emitted an unsolicited response'));
      this.#child.kill();
      return;
    }
    if (this.#stdoutBuffer.byteLength !== 0) {
      pending.reject(new Error('Studio adapter emitted more than one response'));
      this.#fail(new Error('Studio adapter emitted more than one response'));
      this.#child.kill();
      return;
    }
    pending.resolve(responseBytes.toString('utf8').replace(/\r$/, ''));
  }

  #fail(error: Error): void {
    if (this.#closedError === null) this.#closedError = error;
    const pending = this.#pending;
    this.#pending = null;
    pending?.reject(error);
  }
}

async function readBoundedBody(request: IncomingMessage): Promise<string> {
  const declared = Number(request.headers['content-length'] ?? 0);
  if (Number.isFinite(declared) && declared > MAX_STUDIO_ADAPTER_REQUEST_BYTES) {
    throw new Error('Studio adapter request exceeds the protocol byte bound');
  }
  const chunks: Buffer[] = [];
  let bytes = 0;
  for await (const chunk of request) {
    const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    bytes += buffer.byteLength;
    if (bytes > MAX_STUDIO_ADAPTER_REQUEST_BYTES) {
      throw new Error('Studio adapter request exceeds the protocol byte bound');
    }
    chunks.push(buffer);
  }
  return Buffer.concat(chunks).toString('utf8');
}

async function readSettingsBody(request: IncomingMessage): Promise<string> {
  const declared = Number(request.headers['content-length'] ?? 0);
  if (Number.isFinite(declared) && declared > MAX_STUDIO_USER_SETTINGS_BYTES * 2) {
    throw new Error('Studio user-settings request exceeds the protocol byte bound');
  }
  const chunks: Buffer[] = [];
  let bytes = 0;
  for await (const chunk of request) {
    const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    bytes += buffer.byteLength;
    if (bytes > MAX_STUDIO_USER_SETTINGS_BYTES * 2) {
      throw new Error('Studio user-settings request exceeds the protocol byte bound');
    }
    chunks.push(buffer);
  }
  return Buffer.concat(chunks).toString('utf8');
}

async function exchangeUserSettings(
  request: IncomingMessage,
  response: ServerResponse,
  url: URL,
  settingsRoot: string,
): Promise<void> {
  if (request.method === 'GET') {
    const projectRoots = url.searchParams.getAll('projectRoot');
    if (projectRoots.length !== 1) {
      sendError(response, 400, 'Exactly one projectRoot is required');
      return;
    }
    sendJson(response, 200, await readStudioUserSettings({
      projectRoot: projectRoots[0] as string,
      settingsRoot,
    }));
    return;
  }
  if (request.method !== 'PUT') {
    response.writeHead(405, { allow: 'GET, PUT' });
    response.end();
    return;
  }
  let decoded: unknown;
  try {
    decoded = JSON.parse(await readSettingsBody(request)) as unknown;
  } catch {
    sendError(response, 400, 'Studio user-settings request is malformed JSON');
    return;
  }
  if (!isRecord(decoded)
    || typeof decoded['projectRoot'] !== 'string'
    || typeof decoded['text'] !== 'string'
    || (decoded['expectedHash'] !== null && typeof decoded['expectedHash'] !== 'string')) {
    sendError(response, 400, 'Studio user-settings request has an invalid shape');
    return;
  }
  const result = await writeStudioUserSettings({
    projectRoot: decoded['projectRoot'],
    text: decoded['text'],
    expectedHash: decoded['expectedHash'],
    settingsRoot,
  });
  sendJson(response, result.ok ? 200 : result.diagnostic === 'stale_user_settings' ? 409 : 400, result);
}

async function exchangeWithAdapter(
  request: IncomingMessage,
  response: ServerResponse,
  adapter: AdapterProcess,
): Promise<void> {
  if (request.method !== 'POST') {
    response.writeHead(405, { allow: 'POST' });
    response.end();
    return;
  }
  const body = await readBoundedBody(request);
  let decoded: unknown;
  try {
    decoded = JSON.parse(body) as unknown;
  } catch {
    sendError(response, 400, 'Studio adapter request is malformed JSON');
    return;
  }
  if (decoded === null || typeof decoded !== 'object' || Array.isArray(decoded)) {
    sendError(response, 400, 'Studio adapter request must be a JSON object');
    return;
  }
  const responseLine = await adapter.exchange(JSON.stringify(decoded));
  response.writeHead(200, {
    'cache-control': 'no-store',
    'content-type': 'application/json; charset=utf-8',
    'content-length': String(Buffer.byteLength(responseLine)),
  });
  response.end(responseLine);
}

async function exchangeHostFiles(
  request: IncomingMessage,
  response: ServerResponse,
  url: URL,
): Promise<void> {
  if (request.method !== 'GET') {
    response.writeHead(405, { allow: 'GET' });
    response.end();
    return;
  }
  const directories = url.searchParams.getAll('directory');
  if (directories.length !== 1) {
    sendError(response, 400, 'Exactly one host directory is required');
    return;
  }
  sendJson(response, 200, await listStudioHostDirectory({
    directory: directories[0] as string,
    extensions: url.searchParams.getAll('extension'),
  }));
}

async function exchangeRenderResource(
  request: IncomingMessage,
  response: ServerResponse,
  url: URL,
): Promise<void> {
  if (request.method !== 'GET') {
    response.writeHead(405, { allow: 'GET' });
    response.end();
    return;
  }
  const projectRoots = url.searchParams.getAll('projectRoot');
  const sourcePaths = url.searchParams.getAll('sourcePath');
  const contentHashes = url.searchParams.getAll('contentHash');
  if (projectRoots.length !== 1 || sourcePaths.length !== 1 || contentHashes.length !== 1) {
    sendError(response, 400, 'Exactly one projectRoot, sourcePath, and contentHash are required');
    return;
  }
  const bytes = await readStudioRenderResource({
    projectRoot: projectRoots[0] as string,
    sourcePath: sourcePaths[0] as string,
    contentHash: contentHashes[0] as string,
  });
  response.writeHead(200, {
    'cache-control': 'no-store',
    'content-type': 'model/gltf-binary',
    'content-length': String(bytes.byteLength),
  });
  response.end(bytes);
}

async function serveStatic(
  request: IncomingMessage,
  response: ServerResponse,
  staticRoot: string,
  pathname: string,
): Promise<void> {
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    response.writeHead(405, { allow: 'GET, HEAD' });
    response.end();
    return;
  }
  let decodedPath: string;
  try {
    decodedPath = decodeURIComponent(pathname);
  } catch {
    sendError(response, 400, 'Malformed static path');
    return;
  }
  if (decodedPath.includes('\0')) {
    sendError(response, 400, 'Malformed static path');
    return;
  }
  const relativePath = decodedPath === '/' ? 'index.html' : decodedPath.replace(/^\/+/, '');
  const file = resolve(staticRoot, relativePath);
  const fromRoot = relative(staticRoot, file);
  if (fromRoot.startsWith('..') || isAbsolute(fromRoot)) {
    sendError(response, 404, 'Not found');
    return;
  }
  let metadata;
  try {
    metadata = await stat(file);
  } catch {
    sendError(response, 404, 'Not found');
    return;
  }
  if (!metadata.isFile()) {
    sendError(response, 404, 'Not found');
    return;
  }
  response.writeHead(200, {
    'content-type': contentType(file),
    'content-length': String(metadata.size),
    'cache-control': relativePath === 'index.html' ? 'no-cache' : 'public, max-age=31536000, immutable',
  });
  if (request.method === 'HEAD') {
    response.end();
    return;
  }
  createReadStream(file).pipe(response);
}

function sendError(response: ServerResponse, status: number, message: string): void {
  sendJson(response, status, { ok: false, message });
}

function sendJson(response: ServerResponse, status: number, value: unknown): void {
  const body = JSON.stringify(value);
  response.writeHead(status, {
    'cache-control': 'no-store',
    'content-type': 'application/json; charset=utf-8',
    'content-length': String(Buffer.byteLength(body)),
  });
  response.end(body);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function contentType(file: string): string {
  switch (extname(file)) {
    case '.css': return 'text/css; charset=utf-8';
    case '.html': return 'text/html; charset=utf-8';
    case '.js': return 'text/javascript; charset=utf-8';
    case '.json': return 'application/json; charset=utf-8';
    case '.svg': return 'image/svg+xml';
    case '.woff2': return 'font/woff2';
    default: return 'application/octet-stream';
  }
}

function argumentValue(name: string, fallback?: string): string {
  const index = process.argv.indexOf(name);
  if (index === -1) {
    if (fallback !== undefined) return fallback;
    throw new Error(`${name} is required`);
  }
  const value = process.argv[index + 1];
  if (value === undefined || value.startsWith('--')) throw new Error(`${name} requires a value`);
  return value;
}

function options(): HostOptions {
  const adapterBinary = argumentValue('--adapter-binary');
  const staticRoot = argumentValue('--static-root', DEFAULT_STATIC_ROOT);
  const host = argumentValue('--host', '127.0.0.1');
  const port = Number(argumentValue('--port', '4300'));
  const settingsRoot = argumentValue('--settings-root', defaultStudioUserSettingsRoot());
  if (!isAbsolute(adapterBinary)) throw new Error('--adapter-binary must be absolute');
  if (!isAbsolute(staticRoot)) throw new Error('--static-root must be absolute');
  if (!isAbsolute(settingsRoot)) throw new Error('--settings-root must be absolute');
  if (!Number.isSafeInteger(port) || port < 1 || port > 65_535) {
    throw new Error('--port must be an integer from 1 through 65535');
  }
  return { adapterBinary, staticRoot, host, port, settingsRoot };
}

async function main(): Promise<void> {
  const configured = options();
  const [binaryMetadata, indexMetadata] = await Promise.all([
    stat(configured.adapterBinary),
    stat(resolve(configured.staticRoot, 'index.html')),
  ]);
  if (!binaryMetadata.isFile()) throw new Error('--adapter-binary must name a file');
  if (!indexMetadata.isFile()) throw new Error('--static-root must contain index.html');

  const adapter = new AdapterProcess(configured.adapterBinary);
  const server = createServer((request, response) => {
    void (async () => {
      const url = new URL(request.url ?? '/', `http://${configured.host}:${String(configured.port)}`);
      if (url.pathname === '/health') {
        const body = JSON.stringify({ status: 'ok' });
        response.writeHead(200, {
          'cache-control': 'no-store',
          'content-type': 'application/json; charset=utf-8',
          'content-length': String(Buffer.byteLength(body)),
        });
        response.end(body);
        return;
      }
      if (url.pathname === '/api/studio-adapter') {
        await exchangeWithAdapter(request, response, adapter);
        return;
      }
      if (url.pathname === '/api/studio-user-settings') {
        await exchangeUserSettings(request, response, url, configured.settingsRoot);
        return;
      }
      if (url.pathname === '/api/studio-host-files') {
        await exchangeHostFiles(request, response, url);
        return;
      }
      if (url.pathname === '/api/studio-render-resource') {
        await exchangeRenderResource(request, response, url);
        return;
      }
      await serveStatic(request, response, configured.staticRoot, url.pathname);
    })().catch((error: unknown) => {
      if (!response.headersSent) {
        sendError(response, 502, error instanceof Error ? error.message : 'Studio host failure');
      } else {
        response.destroy(error instanceof Error ? error : undefined);
      }
    });
  });
  await new Promise<void>((resolvePromise, rejectPromise) => {
    server.once('error', rejectPromise);
    server.listen(configured.port, configured.host, resolvePromise);
  });
  const shutdown = (): void => {
    adapter.close();
    server.close();
  };
  process.once('SIGINT', shutdown);
  process.once('SIGTERM', shutdown);
  process.stdout.write(
    `Rusty Engine Studio listening on http://${configured.host}:${String(configured.port)}\n`,
  );
}

await main();
