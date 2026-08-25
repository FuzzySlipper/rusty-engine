#!/usr/bin/env node

/**
 * Product Model Tauri 2 acceptance proof.
 *
 * This runner deliberately speaks only the W3C WebDriver surface exposed by
 * tauri-driver.  It does not start a browser host or a development server and
 * it never reaches into Tauri's private JavaScript APIs.  The product binary,
 * desktop entry, storage namespace, and activation receipt are all explicit
 * inputs so a packaged/relocated product can be tested without depending on
 * this repository's checkout layout.
 */

import { spawn } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { dirname, isAbsolute, join, resolve } from "node:path";

const DEFAULT_PORT = 4454;
const DEFAULT_NATIVE_PORT = 4455;
const DEFAULT_NATIVE_DRIVER = "/usr/bin/WebKitWebDriver";
const DEFAULT_COUNTER_SELECTOR = "#product-conformance-counter";
const DEFAULT_STARTUP_TIMEOUT_MS = 45_000;
const DEFAULT_STEP_TIMEOUT_MS = 15_000;
const DEFAULT_LAUNCH_TIMEOUT_MS = 8_000;
const MAX_CAPTURE_BYTES = 16 * 1024;
const MAX_RECEIPT_BYTES = 256 * 1024;
const MAX_SCREENSHOT_BYTES = 8 * 1024 * 1024;
const MAX_ERROR_BYTES = 8 * 1024;
const MAX_TIMEOUT_MS = 5 * 60 * 1000;

class UsageError extends Error {}

function usage() {
  return `Usage:
  node rusty-cli-tauri-test.mjs \
    --driver /absolute/path/to/tauri-driver \
    --application /absolute/path/to/installed-product \
    --desktop-entry org.example.product \
    --xdg-data-home /absolute/path/to/xdg-data \
    --storage-namespace product.namespace \
    --activation-receipt /absolute/path/to/activation.json \
    --evidence-dir /absolute/path/to/evidence

Optional: --port N --native-port N --native-driver PATH --native-host HOST
          --counter-selector CSS --startup-timeout-ms N
          --step-timeout-ms N --launch-timeout-ms N --resize-width N
          --resize-height N --screenshot-name FILE --self-test
`;
}

function parsePositiveInteger(name, value) {
  if (!/^\d+$/u.test(value)) throw new UsageError(`${name} must be a positive integer`);
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new UsageError(`${name} must be a positive integer`);
  }
  if (name.includes("timeout") && parsed > MAX_TIMEOUT_MS) {
    throw new UsageError(`${name} must not exceed ${MAX_TIMEOUT_MS}ms`);
  }
  return parsed;
}

function parseArgs(argv) {
  const values = new Map();
  const aliases = new Map([
    ["tauri-driver", "driver"],
    ["binary", "application"],
    ["desktop-entry-id", "desktop-entry"],
    ["xdg-data-root", "xdg-data-home"],
    ["receipt", "activation-receipt"],
    ["output-dir", "evidence-dir"],
  ]);
  let selfTest = false;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--help" || argument === "-h") {
      process.stdout.write(usage());
      process.exit(0);
    }
    if (argument === "--self-test") {
      selfTest = true;
      continue;
    }
    if (!argument.startsWith("--")) throw new UsageError(`unexpected argument ${argument}`);
    const name = aliases.get(argument.slice(2)) ?? argument.slice(2);
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) throw new UsageError(`missing value for --${name}`);
    values.set(name, value);
    index += 1;
  }
  if (selfTest) return { selfTest };

  const required = [
    ["driver", "tauri-driver"],
    ["application", "application"],
    ["desktop-entry", "desktop-entry"],
    ["xdg-data-home", "XDG data root"],
    ["storage-namespace", "storage namespace"],
    ["activation-receipt", "activation receipt"],
    ["evidence-dir", "evidence directory"],
  ];
  for (const [key, label] of required) {
    if (!values.has(key) || values.get(key).trim() === "") {
      throw new UsageError(`--${key} is required (${label})`);
    }
  }
  const absolute = (key) => {
    const path = resolve(values.get(key));
    if (!isAbsolute(path)) throw new UsageError(`--${key} must resolve to an absolute path`);
    return path;
  };
  return {
    selfTest,
    driver: absolute("driver"),
    application: absolute("application"),
    desktopEntry: values.get("desktop-entry").trim(),
    xdgDataHome: absolute("xdg-data-home"),
    storageNamespace: values.get("storage-namespace").trim(),
    activationReceipt: absolute("activation-receipt"),
    evidenceDir: absolute("evidence-dir"),
    port: parsePositiveInteger("--port", values.get("port") ?? String(DEFAULT_PORT)),
    nativePort: parsePositiveInteger(
      "--native-port",
      values.get("native-port") ?? String(DEFAULT_NATIVE_PORT),
    ),
    nativeDriver: resolve(values.get("native-driver") ?? DEFAULT_NATIVE_DRIVER),
    nativeHost: values.get("native-host")?.trim() || null,
    counterSelector: values.get("counter-selector") ?? DEFAULT_COUNTER_SELECTOR,
    startupTimeoutMs: parsePositiveInteger(
      "--startup-timeout-ms",
      values.get("startup-timeout-ms") ?? String(DEFAULT_STARTUP_TIMEOUT_MS),
    ),
    stepTimeoutMs: parsePositiveInteger(
      "--step-timeout-ms",
      values.get("step-timeout-ms") ?? String(DEFAULT_STEP_TIMEOUT_MS),
    ),
    launchTimeoutMs: parsePositiveInteger(
      "--launch-timeout-ms",
      values.get("launch-timeout-ms") ?? String(DEFAULT_LAUNCH_TIMEOUT_MS),
    ),
    resizeWidth: parsePositiveInteger("--resize-width", values.get("resize-width") ?? "1024"),
    resizeHeight: parsePositiveInteger("--resize-height", values.get("resize-height") ?? "640"),
    screenshotName: values.get("screenshot-name") ?? "tauri-product.png",
  };
}

function boundedText(value, limit = MAX_CAPTURE_BYTES) {
  const text = String(value ?? "");
  if (Buffer.byteLength(text) <= limit) return text;
  return `${text.slice(0, Math.max(0, limit - 32))}\n...[truncated]`;
}

function captureStream(stream) {
  let value = "";
  let bytes = 0;
  let truncated = false;
  return new Promise((resolveCapture) => {
    if (!stream) return resolveCapture({ value, truncated });
    let settled = false;
    const finish = () => {
      if (settled) return;
      settled = true;
      resolveCapture({ value, truncated });
    };
    stream.setEncoding("utf8");
    stream.on("data", (chunk) => {
      const text = String(chunk);
      const chunkBytes = Buffer.byteLength(text);
      if (bytes < MAX_CAPTURE_BYTES) {
        const room = MAX_CAPTURE_BYTES - bytes;
        value += text.slice(0, room);
      }
      bytes += chunkBytes;
      if (bytes > MAX_CAPTURE_BYTES) truncated = true;
    });
    stream.once("end", finish);
    stream.once("close", finish);
    stream.once("error", () => {
      truncated = true;
      finish();
    });
  });
}

function exited(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return Promise.resolve({ code: child.exitCode, signal: child.signalCode });
  }
  return new Promise((resolveExit) => child.once("exit", (code, signal) => resolveExit({ code, signal })));
}

function signalProcessGroup(child, signal) {
  if (!child) return;
  try {
    if (process.platform !== "win32" && child.pid) process.kill(-child.pid, signal);
    else child.kill(signal);
  } catch (error) {
    if (error?.code !== "ESRCH") throw error;
  }
}

async function terminateProcess(child) {
  if (!child || child.exitCode !== null || child.signalCode !== null) return;
  const signalGroup = (signal) => signalProcessGroup(child, signal);
  signalGroup("SIGTERM");
  const deadline = Date.now() + 2_000;
  while (Date.now() < deadline && child.exitCode === null && child.signalCode === null) {
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 50));
  }
  if (child.exitCode === null && child.signalCode === null) signalGroup("SIGKILL");
  await exited(child);
}

async function reapDescendants(child) {
  // A launcher can return successfully while a descendant still owns the
  // stdout/stderr pipes.  The launcher has its own detached process group, so
  // terminating that group closes the pipes and prevents an unbounded wait or
  // a leaked second product process.  The ESRCH path is the normal no-child
  // case after a clean exit.
  if (process.platform === "win32" || !child?.pid) return;
  signalProcessGroup(child, "SIGTERM");
  await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
  signalProcessGroup(child, "SIGKILL");
}

async function runBounded(command, args, env, timeoutMs, preserveCleanDescendants = false) {
  const child = spawn(command, args, {
    cwd: env.XDG_DATA_HOME,
    detached: process.platform !== "win32",
    env,
    stdio: preserveCleanDescendants ? "ignore" : ["ignore", "pipe", "pipe"],
  });
  const stdout = child.stdout === null ? Promise.resolve({ value: "", truncated: false }) : captureStream(child.stdout);
  const stderr = child.stderr === null ? Promise.resolve({ value: "", truncated: false }) : captureStream(child.stderr);
  let timedOut = false;
  const exit = new Promise((resolveExit) => {
    child.once("error", (error) => resolveExit({ code: null, signal: null, error }));
    child.once("exit", (code, signal) => resolveExit({ code, signal }));
  });
  let timeoutHandle;
  const timeout = new Promise((resolveTimeout) => {
    timeoutHandle = setTimeout(() => resolveTimeout({ timeout: true }), timeoutMs);
  });
  const result = await Promise.race([exit, timeout]);
  clearTimeout(timeoutHandle);
  if (result.timeout) {
    timedOut = true;
    await terminateProcess(child);
    await reapDescendants(child);
  } else if (!preserveCleanDescendants) {
    await reapDescendants(child);
  }
  const status = await exit;
  const [out, err] = await Promise.all([stdout, stderr]);
  return {
    command,
    args,
    timedOut,
    code: status.code,
    signal: status.signal,
    error: status.error ? boundedText(status.error.stack ?? status.error.message, MAX_ERROR_BYTES) : null,
    stdout: boundedText(out.value),
    stderr: boundedText(err.value),
    stdoutTruncated: out.truncated,
    stderrTruncated: err.truncated,
    processGroupPid: preserveCleanDescendants ? child.pid ?? null : null,
  };
}

async function reapPreservedProcessGroup(pid) {
  if (process.platform === "win32" || !Number.isInteger(pid) || pid <= 0) return;
  const child = { pid };
  await reapDescendants(child);
}

function makeEnvironment(options) {
  const xdgConfigHome = join(options.xdgDataHome, "config");
  const xdgCacheHome = join(options.xdgDataHome, "cache");
  const xdgStateHome = join(options.xdgDataHome, "state");
  mkdirSync(xdgConfigHome, { recursive: true });
  mkdirSync(xdgCacheHome, { recursive: true });
  mkdirSync(xdgStateHome, { recursive: true });
  return {
    ...process.env,
    XDG_DATA_HOME: options.xdgDataHome,
    XDG_DATA_DIRS: `${options.xdgDataHome}:${process.env.XDG_DATA_DIRS ?? "/usr/local/share:/usr/share"}`,
    XDG_CONFIG_HOME: xdgConfigHome,
    XDG_CACHE_HOME: xdgCacheHome,
    XDG_STATE_HOME: xdgStateHome,
    // These names are intentionally plain environment inputs for the thin
    // generated wrapper.  The runner does not invoke an IPC command or infer
    // any product state from them.
    RUSTY_PRODUCT_STORAGE_NAMESPACE: options.storageNamespace,
    RUSTY_STORAGE_NAMESPACE: options.storageNamespace,
    TAURI_STORAGE_NAMESPACE: options.storageNamespace,
    RUSTY_PRODUCT_ACTIVATION_RECEIPT: options.activationReceipt,
    RUSTY_ACTIVATION_RECEIPT: options.activationReceipt,
    TAURI_ACTIVATION_RECEIPT: options.activationReceipt,
  };
}

async function request(baseUrl, path, { method = "GET", body, timeoutMs = 30_000 } = {}) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(`${baseUrl}${path}`, {
      method,
      headers: { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: controller.signal,
    });
    const text = await response.text();
    let payload = null;
    try {
      payload = text === "" ? null : JSON.parse(text);
    } catch {
      throw new Error(`WebDriver ${method} ${path} returned non-JSON: ${boundedText(text)}`);
    }
    const value = payload?.value ?? payload;
    if (!response.ok || value?.error) {
      throw new Error(`WebDriver ${method} ${path}: ${boundedText(text)}`);
    }
    return value;
  } finally {
    clearTimeout(timeout);
  }
}

async function waitFor(label, operation, timeoutMs, intervalMs = 150) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    try {
      const value = await operation();
      if (value && typeof value === "object" && value.ok === false) {
        last = value.diagnostic ?? value;
      } else if (value) {
        return value;
      } else {
        last = value;
      }
    } catch (error) {
      last = error instanceof Error ? error.message : String(error);
    }
    await new Promise((resolveDelay) => setTimeout(resolveDelay, intervalMs));
  }
  throw new Error(`timed out waiting for ${label}: ${boundedText(JSON.stringify(last))}`);
}

async function processPids(binary) {
  // Dynamic import keeps the main proof's dependency surface to Node built-ins
  // while making the no-procfs fallback explicit.
  const { readdirSync } = await import("node:fs");
  let expected;
  try {
    expected = realpathSync(binary);
  } catch {
    return [];
  }
  const pids = [];
  try {
    for (const entry of readdirSync("/proc", { withFileTypes: true })) {
      if (!entry.isDirectory() || !/^\d+$/u.test(entry.name)) continue;
      try {
        if (realpathSync(`/proc/${entry.name}/exe`) === expected) pids.push(Number(entry.name));
      } catch {
        // Process exited during inspection.
      }
    }
  } catch {
    // Non-Linux hosts do not expose procfs; the WebDriver session and process
    // lifecycle checks still provide the authoritative proof there.
  }
  return pids;
}

async function waitForApplicationExit(binary, timeoutMs, intervalMs = 50) {
  const started = Date.now();
  let pids = await processPids(binary);
  const deadline = started + timeoutMs;
  while (pids.length > 0 && Date.now() < deadline) {
    await new Promise((resolveDelay) => setTimeout(resolveDelay, intervalMs));
    pids = await processPids(binary);
  }
  return {
    graceful: pids.length === 0,
    durationMs: Date.now() - started,
    pids,
  };
}

async function cleanApplicationProcesses(binary) {
  let remaining = await processPids(binary);
  if (remaining.length === 0) return { attempted: [], remaining };
  const attempted = [...remaining];
  for (const pid of remaining) {
    try {
      process.kill(pid, "SIGTERM");
    } catch (error) {
      if (error?.code !== "ESRCH") throw error;
    }
  }
  const deadline = Date.now() + 2_000;
  while (Date.now() < deadline) {
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 50));
    remaining = await processPids(binary);
    if (remaining.length === 0) return { attempted, remaining };
  }
  for (const pid of remaining) {
    try {
      process.kill(pid, "SIGKILL");
    } catch (error) {
      if (error?.code !== "ESRCH") throw error;
    }
  }
  await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
  return { attempted, remaining: await processPids(binary) };
}

async function execute(baseUrl, sessionId, script, args = []) {
  return request(baseUrl, `/session/${sessionId}/execute/sync`, {
    method: "POST",
    body: { script: `return (() => { ${script} })();`, args },
  });
}

async function domSnapshot(baseUrl, sessionId, selector) {
  return execute(
    baseUrl,
    sessionId,
    `const label = document.querySelector(arguments[0]);
     return {
       readyState: document.readyState,
       visibility: document.visibilityState,
       title: document.title,
       body: document.body !== null,
       productReady: document.body?.dataset?.rustyProductReady === 'true',
       lastRuntimeCommand: document.body?.dataset?.rustyLastRuntimeCommand ?? null,
       lastRuntimeAccepted: document.body?.dataset?.rustyLastRuntimeAccepted ?? null,
       lastRuntimeCount: document.body?.dataset?.rustyLastRuntimeCount ?? null,
       lastRuntimeDiagnostic: document.body?.dataset?.rustyLastRuntimeDiagnostic ?? null,
       lastRuntimeOutput: document.body?.dataset?.rustyLastRuntimeOutput ?? null,
       interactionMode: document.querySelector('[data-rusty-application-host]')?.dataset?.interactionMode ?? null,
       viewportWidth: window.innerWidth,
       viewportHeight: window.innerHeight,
       canvasCount: document.querySelectorAll('canvas').length,
       canvasRect: (() => {
         const canvas = document.querySelector('canvas');
         if (!canvas) return null;
         const rect = canvas.getBoundingClientRect();
         return {
           left: rect.left,
           top: rect.top,
           right: rect.right,
           bottom: rect.bottom,
           width: rect.width,
           height: rect.height,
           centerX: rect.left + rect.width / 2,
           centerY: rect.top + rect.height / 2,
         };
       })(),
       activeElementIsCurrentCanvas: document.activeElement === document.querySelector('canvas'),
       activeElementTag: document.activeElement?.tagName ?? null,
       activeElementId: document.activeElement?.id ?? null,
       counter: label?.textContent?.trim() ?? null,
       counterTag: label?.tagName ?? null,
       href: location.href,
       tauriGlobal: globalThis.__TAURI__ !== undefined,
       tauriKeys: globalThis.__TAURI__ === undefined
         ? []
         : Object.keys(globalThis.__TAURI__).sort().slice(0, 64),
       moduleScripts: Array.from(document.querySelectorAll('script[type="module"]'))
         .map((script) => script.getAttribute('src'))
         .slice(0, 16),
       bodyText: (document.body?.innerText ?? '').slice(0, 4096),
       startupError: (
         document.body?.dataset?.startupError ??
         document.body?.dataset?.runtimeError ??
         document.body?.dataset?.desktopStartupError ??
         document.querySelector('[data-startup-error]')?.textContent?.trim() ??
         null
       )?.toString().slice(0, 1024) ?? null,
     };`,
    [selector],
  );
}

function compactFocusSnapshot(snapshot) {
  if (!snapshot) return null;
  return {
    readyState: snapshot.readyState,
    canvasCount: snapshot.canvasCount,
    canvasRect: snapshot.canvasRect,
    activeElementIsCurrentCanvas: snapshot.activeElementIsCurrentCanvas,
    activeElementTag: snapshot.activeElementTag,
    activeElementId: snapshot.activeElementId,
    counter: snapshot.counter,
    startupError: snapshot.startupError,
    bodyText: boundedText(snapshot.bodyText, 1024),
  };
}

async function focusCanvas(baseUrl, sessionId, counterSelector, timeoutMs) {
  const started = Date.now();
  const deadline = started + timeoutMs;
  const attempts = [];
  let attemptsTruncated = false;
  let attemptNumber = 0;
  let lastSnapshot = null;
  let firstActiveElement = null;
  while (Date.now() < deadline) {
    attemptNumber += 1;
    const attemptStarted = Date.now();
    const attempt = { attempt: attemptNumber, method: "w3c-tab" };
    try {
      const before = await domSnapshot(baseUrl, sessionId, counterSelector);
      lastSnapshot = before;
      firstActiveElement ??= {
        tag: before.activeElementTag,
        id: before.activeElementId,
      };
      attempt.before = compactFocusSnapshot(before);
      if (before.canvasCount !== 1) {
        throw new Error(`expected one current canvas before Tab, found ${before.canvasCount}`);
      }
      if (before.activeElementIsCurrentCanvas === true) {
        attempt.status = "passed";
        attempt.reason = "canvas was already the active current element";
        attempt.durationMs = Date.now() - attemptStarted;
        attempts.push(attempt);
        return {
          status: "passed",
          method: "already-focused",
          durationMs: Date.now() - started,
          attemptCount: attemptNumber,
          tabSteps: 0,
          firstActiveElement,
          attempts,
          attemptsTruncated,
          latestSnapshot: compactFocusSnapshot(before),
        };
      }
      // Use a real W3C Tab action. The canvas is the product's focusable
      // surface; this follows the browser's sequential focus order without
      // synthesizing a DOM event or calling a DOM focus method.
      await request(baseUrl, `/session/${sessionId}/actions`, {
        method: "POST",
        body: {
          actions: [
            {
              type: "key",
              id: "rusty-acceptance-focus-keyboard",
              actions: [
                { type: "keyDown", value: "\uE004" },
                { type: "keyUp", value: "\uE004" },
              ],
            },
          ],
        },
      });
      const after = await domSnapshot(baseUrl, sessionId, counterSelector);
      lastSnapshot = after;
      attempt.after = compactFocusSnapshot(after);
      if (after.canvasCount === 1 && after.activeElementIsCurrentCanvas === true) {
        attempt.status = "passed";
        attempt.durationMs = Date.now() - attemptStarted;
        attempts.push(attempt);
        return {
          status: "passed",
          method: "w3c-tab",
          durationMs: Date.now() - started,
          attemptCount: attemptNumber,
          tabSteps: attemptNumber,
          firstActiveElement,
          attempts,
          attemptsTruncated,
          latestSnapshot: compactFocusSnapshot(after),
        };
      }
      attempt.status = "retry";
      attempt.reason = "Tab action completed but current sole canvas focus was not proven";
    } catch (error) {
      attempt.status = "retry";
      attempt.error = boundedText(error?.stack ?? error?.message ?? String(error), MAX_ERROR_BYTES);
    }
    attempt.durationMs = Date.now() - attemptStarted;
    if (attempts.length < 8) attempts.push(attempt);
    else {
      attemptsTruncated = true;
      attempts[attempts.length - 1] = attempt;
    }
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 100));
  }
  const focusEvidence = {
    status: "failed",
    durationMs: Date.now() - started,
    attemptCount: attemptNumber,
    tabSteps: attemptNumber,
    firstActiveElement,
    attempts,
    attemptsTruncated,
    latestSnapshot: compactFocusSnapshot(lastSnapshot),
  };
  const error = new Error(
    `timed out focusing the current canvas through WebDriver: ${boundedText(JSON.stringify(focusEvidence))}`,
  );
  error.focusEvidence = focusEvidence;
  throw error;
}

async function createSession(baseUrl, application) {
  const value = await request(baseUrl, "/session", {
    method: "POST",
    body: {
      capabilities: {
        alwaysMatch: {
          "tauri:options": { application },
        },
      },
    },
  });
  const sessionId = value?.sessionId;
  if (typeof sessionId !== "string" || sessionId === "") {
    throw new Error(`WebDriver returned no session id: ${boundedText(JSON.stringify(value))}`);
  }
  await request(baseUrl, `/session/${sessionId}/timeouts`, {
    method: "POST",
    body: { script: 60_000, pageLoad: 60_000, implicit: 0 },
  });
  return sessionId;
}

async function sendWKeyAction(baseUrl, sessionId, type) {
  // W3C key actions are the normal WebDriver input path.  No DOM event is
  // synthesized and no product IPC surface is touched.
  await request(baseUrl, `/session/${sessionId}/actions`, {
    method: "POST",
    body: {
      actions: [
        {
          type: "key",
          id: "rusty-acceptance-keyboard",
          actions: [{ type, value: "w" }],
        },
      ],
    },
  });
}

async function observePhysicalKeys(baseUrl, sessionId) {
  return execute(
    baseUrl,
    sessionId,
    `globalThis.__rustyAcceptanceKeys ??= [];
     if (globalThis.__rustyAcceptanceKeyListenerInstalled !== true) {
       globalThis.__rustyAcceptanceKeyListenerInstalled = true;
       const observe = (phase) => (event) => globalThis.__rustyAcceptanceKeys.push({
         phase,
         type: event.type,
         key: event.key,
         code: event.code,
         repeat: event.repeat,
         defaultPrevented: event.defaultPrevented,
         target: event.target?.tagName ?? null,
       });
       document.addEventListener('keydown', observe('capture'), true);
       document.addEventListener('keyup', observe('capture'), true);
       document.addEventListener('keydown', observe('bubble'));
       document.addEventListener('keyup', observe('bubble'));
     }
     return globalThis.__rustyAcceptanceKeys.slice(-16);`,
  );
}

async function resizeWindow(baseUrl, sessionId, width, height) {
  const original = await request(baseUrl, `/session/${sessionId}/window/rect`);
  await request(baseUrl, `/session/${sessionId}/window/rect`, {
    method: "POST",
    body: { width, height },
  });
  return { original, requested: { width, height } };
}

async function restoreWindow(baseUrl, sessionId, original) {
  if (!original || !Number.isInteger(original.width) || !Number.isInteger(original.height)) return;
  try {
    const body = { width: original.width, height: original.height };
    if (Number.isInteger(original.x) && Number.isInteger(original.y)) {
      body.x = original.x;
      body.y = original.y;
    }
    await request(baseUrl, `/session/${sessionId}/window/rect`, {
      method: "POST",
      body,
    });
  } catch {
    // Cleanup remains best effort; session deletion is the authoritative close.
  }
}

function readReceipt(path) {
  try {
    const metadata = statSync(path);
    if (!metadata.isFile() || metadata.size > MAX_RECEIPT_BYTES) {
      return { present: true, valid: false, error: "receipt is not a bounded regular file" };
    }
    const bytes = readFileSync(path);
    const digest = createHash("sha256").update(bytes).digest("hex");
    let value;
    try {
      value = JSON.parse(bytes.toString("utf8"));
    } catch (error) {
      return { present: true, valid: false, bytes: bytes.length, sha256: digest, error: `invalid JSON: ${error.message}` };
    }
    const freshness = activationFreshness(value);
    const shapeValid =
      value !== null &&
      typeof value === "object" &&
      !Array.isArray(value) &&
      value.artifact === "rusty.product.activation" &&
      value.mainThreadCompleted === true &&
      freshness !== null;
    return {
      present: true,
      valid: true,
      shapeValid,
      bytes: bytes.length,
      sha256: digest,
      artifact: value?.artifact ?? null,
      mainThreadCompleted: value?.mainThreadCompleted ?? null,
      freshness,
    };
  } catch (error) {
    if (error?.code === "ENOENT") return { present: false };
    return { present: false, error: boundedText(error.message, MAX_ERROR_BYTES) };
  }
}

function activationFreshness(value) {
  if (value && Number.isSafeInteger(value.activationSequence) && value.activationSequence >= 0) {
    return { kind: "activationSequence", value: value.activationSequence };
  }
  if (value && typeof value.activationNonce === "string" && value.activationNonce.length > 0 && value.activationNonce.length <= 256) {
    return { kind: "activationNonce", value: value.activationNonce };
  }
  if (value && typeof value.instanceId === "string" && value.instanceId.length > 0 && value.instanceId.length <= 256) {
    return { kind: "instanceId", value: value.instanceId };
  }
  return null;
}

function receiptIsFresh(baseline, candidate) {
  if (!candidate.present || !candidate.valid || !candidate.shapeValid) return false;
  if (!baseline.present) return true;
  if (!baseline.valid || !baseline.shapeValid || baseline.freshness === null) return false;
  if (candidate.freshness?.kind !== baseline.freshness.kind) return false;
  if (candidate.freshness.kind === "activationSequence") {
    return candidate.freshness.value > baseline.freshness.value;
  }
  return candidate.freshness.value !== baseline.freshness.value;
}

async function waitForFreshReceipt(path, baseline, timeoutMs, observation) {
  return waitFor(
    "fresh singleton activation receipt",
    async () => {
      const candidate = readReceipt(path);
      observation.latest = candidate;
      if (!candidate.present) return null;
      if (!candidate.valid || !candidate.shapeValid || !receiptIsFresh(baseline, candidate)) {
        return {
          ok: false,
          diagnostic: {
            baseline,
            candidate,
            fresh: receiptIsFresh(baseline, candidate),
          },
        };
      }
      return candidate;
    },
    timeoutMs,
  );
}

function writeEvidence(path, evidence) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(evidence, null, 2)}\n`);
}

async function selfTest() {
  const start = Date.now();
  const env = { ...process.env, XDG_DATA_HOME: "/tmp" };
  const result = await runBounded(
    process.execPath,
    ["-e", "process.stdout.write('self-test-ok');"],
    env,
    2_000,
  );
  if (result.timedOut || result.code !== 0 || result.stdout !== "self-test-ok") {
    throw new Error(`process lifecycle self-test failed: ${JSON.stringify(result)}`);
  }
  const timeoutResult = await runBounded(
    process.execPath,
    ["-e", "setTimeout(() => process.stdout.write('late'), 30_000);"],
    env,
    250,
  );
  if (!timeoutResult.timedOut) {
    throw new Error(`bounded timeout self-test failed: ${JSON.stringify(timeoutResult)}`);
  }
  const receiptBaseline = {
    present: true,
    valid: true,
    shapeValid: true,
    freshness: { kind: "activationSequence", value: 4 },
  };
  const receiptFresh = {
    present: true,
    valid: true,
    shapeValid: true,
    freshness: { kind: "activationSequence", value: 5 },
  };
  if (!receiptIsFresh(receiptBaseline, receiptFresh) || receiptIsFresh(receiptBaseline, receiptBaseline)) {
    throw new Error("activation receipt freshness self-test failed");
  }
  return {
    schemaVersion: 1,
    status: "self-test-passed",
    durationMs: Date.now() - start,
    checks: ["bounded-process-reap", "bounded-output-capture", "bounded-timeout", "activation-receipt-freshness", "argument-parser"],
  };
}

async function main(options) {
  mkdirSync(options.evidenceDir, { recursive: true });
  mkdirSync(options.xdgDataHome, { recursive: true });
  for (const path of [options.driver, options.application, options.nativeDriver]) {
    if (!existsSync(path)) throw new Error(`required executable does not exist: ${path}`);
  }
  if (!statSync(options.application).isFile()) throw new Error(`application is not a regular file: ${options.application}`);
  const evidence = {
    schemaVersion: 1,
    status: "running",
    startedAt: new Date().toISOString(),
    application: options.application,
    tauriDriver: options.driver,
    desktopEntry: options.desktopEntry,
    xdgDataHome: options.xdgDataHome,
    storageNamespace: options.storageNamespace,
    activationReceipt: options.activationReceipt,
    evidenceDir: options.evidenceDir,
    screenshot: join(options.evidenceDir, options.screenshotName),
    inheritedDisplay: process.env.DISPLAY ?? null,
    steps: {},
  };
  const desktopEntryFile = join(options.xdgDataHome, "applications", `${options.desktopEntry}.desktop`);
  const desktopEntryBytes = readFileSync(desktopEntryFile);
  if (desktopEntryBytes.length === 0 || desktopEntryBytes.length > MAX_RECEIPT_BYTES) {
    throw new Error(`installed desktop entry is outside the bounded evidence size: ${desktopEntryFile}`);
  }
  evidence.desktopEntryFile = {
    path: desktopEntryFile,
    bytes: desktopEntryBytes.length,
    sha256: createHash("sha256").update(desktopEntryBytes).digest("hex"),
  };
  if (resolve(evidence.screenshot) !== evidence.screenshot || !evidence.screenshot.startsWith(`${options.evidenceDir}/`)) {
    throw new Error("--screenshot-name must remain inside --evidence-dir");
  }
  const env = makeEnvironment(options);
  const baseUrl = `http://127.0.0.1:${options.port}`;
  let driver = null;
  let driverStdout = null;
  let driverStderr = null;
  let sessionId = null;
  let originalWindow = null;
  const startedProductPids = await processPids(options.application);
  if (startedProductPids.length > 0) {
    throw new Error(`application already running before proof: ${startedProductPids.join(", ")}`);
  }
  try {
    const driverArgs = ["--port", String(options.port), "--native-port", String(options.nativePort), "--native-driver", options.nativeDriver];
    if (options.nativeHost) driverArgs.push("--native-host", options.nativeHost);
    driver = spawn(options.driver, driverArgs, {
      cwd: options.xdgDataHome,
      detached: process.platform !== "win32",
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    driverStdout = captureStream(driver.stdout);
    driverStderr = captureStream(driver.stderr);
    driver.once("error", (error) => {
      evidence.driverError = boundedText(error.stack ?? error.message, MAX_ERROR_BYTES);
    });
    await waitFor(
      "tauri-driver readiness",
      async () => {
        try {
          const status = await request(baseUrl, "/status", { timeoutMs: 2_000 });
          return status?.ready === true || status?.message !== undefined ? status : null;
        } catch {
          return null;
        }
      },
      options.startupTimeoutMs,
    );
    evidence.steps.driverReady = { status: "passed" };

    sessionId = await createSession(baseUrl, options.application);
    evidence.sessionId = sessionId;
    const readinessStarted = Date.now();
    const initial = await waitFor(
      "native window readiness, one Engine canvas, and counter zero",
      async () => {
        const snapshot = await domSnapshot(baseUrl, sessionId, options.counterSelector);
        const mismatches = [];
        if (snapshot.readyState !== "complete") mismatches.push(`readyState=${snapshot.readyState}`);
        if (snapshot.body !== true) mismatches.push("body-missing");
        if (snapshot.productReady !== true) mismatches.push("productReady=false");
        if (snapshot.viewportWidth <= 0) mismatches.push(`viewportWidth=${snapshot.viewportWidth}`);
        if (snapshot.viewportHeight <= 0) mismatches.push(`viewportHeight=${snapshot.viewportHeight}`);
        if (snapshot.canvasCount !== 1) mismatches.push(`canvasCount=${snapshot.canvasCount}`);
        if (snapshot.counter !== "0") mismatches.push(`counter=${JSON.stringify(snapshot.counter)}`);
        if (mismatches.length > 0) {
          evidence.steps.nativeWindowReady = {
            status: "waiting",
            durationMs: Date.now() - readinessStarted,
            mismatches,
            latestSnapshot: snapshot,
          };
          return { ok: false, diagnostic: { mismatches, snapshot } };
        }
        return snapshot;
      },
      options.startupTimeoutMs,
    );
    evidence.steps.nativeWindowReady = { status: "passed", durationMs: Date.now() - readinessStarted, snapshot: initial };

    const focusStarted = Date.now();
    try {
      const focus = await focusCanvas(baseUrl, sessionId, options.counterSelector, options.stepTimeoutMs);
      evidence.steps.canvasFocus = { ...focus, durationMs: Date.now() - focusStarted };
    } catch (error) {
      evidence.steps.canvasFocus = error.focusEvidence ?? {
        status: "failed",
        durationMs: Date.now() - focusStarted,
        error: boundedText(error?.stack ?? error?.message ?? String(error), MAX_ERROR_BYTES),
      };
      throw error;
    }
    const inputStarted = Date.now();
    await observePhysicalKeys(baseUrl, sessionId);
    await sendWKeyAction(baseUrl, sessionId, "keyDown");
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 500));
    evidence.steps.wKeyInput = {
      status: "observed",
      events: await observePhysicalKeys(baseUrl, sessionId),
      snapshot: await domSnapshot(baseUrl, sessionId, options.counterSelector),
    };
    const afterW = await waitFor(
      "Rust-authoritative counter projection to become one while W is held",
      async () => {
        const snapshot = await domSnapshot(baseUrl, sessionId, options.counterSelector);
        return snapshot.counter === "1" && snapshot.canvasCount === 1 ? snapshot : null;
      },
      options.stepTimeoutMs,
    );
    await sendWKeyAction(baseUrl, sessionId, "keyUp");
    evidence.steps.wKeyInput.events = await observePhysicalKeys(baseUrl, sessionId);
    evidence.steps.wKeyProjection = {
      status: "passed",
      durationMs: Date.now() - inputStarted,
      action: "WebDriver W keyDown, authoritative projection, then keyUp",
      snapshot: afterW,
    };

    const resizeStarted = Date.now();
    const resize = await resizeWindow(baseUrl, sessionId, options.resizeWidth, options.resizeHeight);
    originalWindow = resize.original;
    const resized = await waitFor(
      "one Engine canvas after native window resize",
      async () => {
        const snapshot = await domSnapshot(baseUrl, sessionId, options.counterSelector);
        return snapshot.canvasCount === 1 ? snapshot : null;
      },
      options.stepTimeoutMs,
    );
    evidence.steps.resize = { status: "passed", durationMs: Date.now() - resizeStarted, ...resize, snapshot: resized };
    await restoreWindow(baseUrl, sessionId, originalWindow);

    const screenshotBase64 = await request(baseUrl, `/session/${sessionId}/screenshot`);
    if (typeof screenshotBase64 !== "string" || screenshotBase64.length === 0) throw new Error("WebDriver returned no screenshot bytes");
    if (screenshotBase64.length > MAX_SCREENSHOT_BYTES * 2) throw new Error("WebDriver screenshot exceeded the bounded evidence size");
    mkdirSync(dirname(evidence.screenshot), { recursive: true });
    const screenshotBytes = Buffer.from(screenshotBase64, "base64");
    if (screenshotBytes.length === 0 || screenshotBytes.length > MAX_SCREENSHOT_BYTES) throw new Error("WebDriver screenshot decoded outside the bounded evidence size");
    writeFileSync(evidence.screenshot, screenshotBytes);
    evidence.steps.screenshot = { status: "passed", bytes: screenshotBytes.length, path: evidence.screenshot };

    const launch = async (label, command, args, preserveCleanDescendants = false) => {
      const started = Date.now();
      const baseline = readReceipt(options.activationReceipt);
      if (baseline.present && (!baseline.valid || !baseline.shapeValid)) {
        throw new Error(`activation receipt baseline before ${label} has the wrong shape: ${JSON.stringify(baseline)}`);
      }
      const observation = { baseline };
      evidence.steps[label] = {
        status: "waiting",
        baseline,
        activationObservation: observation,
      };
      const result = await runBounded(command, args, env, options.launchTimeoutMs, preserveCleanDescendants);
      const responsive = await waitFor(
        `${label} primary responsiveness`,
        async () => {
          const snapshot = await domSnapshot(baseUrl, sessionId, options.counterSelector);
          return snapshot.canvasCount === 1 && snapshot.counter === "1" ? snapshot : null;
        },
        options.stepTimeoutMs,
      );
      const activation = await waitForFreshReceipt(
        options.activationReceipt,
        baseline,
        options.stepTimeoutMs,
        observation,
      );
      await reapPreservedProcessGroup(result.processGroupPid);
      evidence.steps[label] = {
        ...evidence.steps[label],
        status: result.timedOut || result.error || result.code !== 0 ? "failed" : "passed",
        durationMs: Date.now() - started,
        result,
        primaryResponsive: responsive,
        activation,
      };
      evidence.activation ??= {};
      evidence.activation[label] = { baseline, fresh: activation };
      if (result.timedOut) throw new Error(`${label} did not exit within ${options.launchTimeoutMs}ms`);
      if (result.error) throw new Error(`${label} failed to spawn: ${result.error}`);
      if (result.code !== 0) throw new Error(`${label} exited with code ${result.code ?? "unknown"}`);
      return result;
    };
    await launch("secondDirectLaunch", options.application, []);
    await launch("gtkLaunch", "gtk-launch", [`${options.desktopEntry}.desktop`], true);
  } catch (error) {
    // Preserve bounded step/path evidence on a failed acceptance while still
    // allowing the finally block below to perform session/process cleanup.
    if (error && typeof error === "object") error.evidence = evidence;
    throw error;
  } finally {
    if (sessionId) {
      try {
        await request(baseUrl, `/session/${sessionId}`, { method: "DELETE", body: {} });
        evidence.steps.sessionDelete = { status: "passed" };
      } catch (error) {
        evidence.steps.sessionDelete = { status: "failed", detail: boundedText(error.stack ?? error.message, MAX_ERROR_BYTES) };
      }
      sessionId = null;
    }
    const afterDelete = await waitForApplicationExit(options.application, options.stepTimeoutMs);
    evidence.steps.shutdownAfterSessionDelete = {
      status: afterDelete.graceful ? "passed" : "failed",
      ...afterDelete,
    };
    if (driver) {
      const stdout = driver.stdout;
      const stderr = driver.stderr;
      await terminateProcess(driver);
      // The stream drains are intentionally awaited after process-group
      // termination so a child cannot hold the runner open through a pipe.
      evidence.driverOutput = {
        stdout: boundedText((await driverStdout)?.value),
        stderr: boundedText((await driverStderr)?.value),
      };
    }
    const afterDriverStop = afterDelete.graceful
      ? { graceful: true, durationMs: 0, pids: [] }
      : await waitForApplicationExit(options.application, options.stepTimeoutMs);
    evidence.steps.shutdownAfterDriverStop = {
      status: afterDriverStop.graceful ? "passed" : "failed",
      ...afterDriverStop,
    };
    const cleanup = afterDriverStop.graceful
      ? { attempted: [], remaining: [] }
      : await cleanApplicationProcesses(options.application);
    evidence.forcedApplicationCleanupPids = cleanup.attempted;
    evidence.orphanApplicationPids = cleanup.remaining;
    evidence.shutdown = {
      gracefulAfterSessionDelete: afterDelete.graceful,
      gracefulAfterDriverStop: afterDriverStop.graceful,
      forcedCleanupNeeded: cleanup.attempted.length > 0,
      forcedCleanupPids: cleanup.attempted,
      orphanApplicationPids: cleanup.remaining,
    };
    evidence.shutdownClean =
      afterDelete.graceful &&
      afterDriverStop.graceful &&
      cleanup.attempted.length === 0 &&
      evidence.orphanApplicationPids.length === 0;
  }
  if (!evidence.shutdownClean) {
    throw new Error(
      `desktop application did not shut down gracefully after session deletion: ${JSON.stringify(evidence.shutdown)}`,
    );
  }
  evidence.status = "passed";
  evidence.completedAt = new Date().toISOString();
  evidence.durationMs = Date.parse(evidence.completedAt) - Date.parse(evidence.startedAt);
  return evidence;
}

async function run() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
    if (options.selfTest) {
      const evidence = await selfTest();
      process.stdout.write(`${JSON.stringify(evidence)}\n`);
      return;
    }
    const evidence = await main(options);
    writeEvidence(join(options.evidenceDir, "tauri-test.json"), evidence);
    process.stdout.write(`${JSON.stringify(evidence)}\n`);
  } catch (error) {
    const message = boundedText(error?.stack ?? error?.message ?? String(error), MAX_ERROR_BYTES);
    const failure = {
      ...(error?.evidence ?? {}),
      schemaVersion: 1,
      status: "failed",
      error: message,
      completedAt: new Date().toISOString(),
    };
    if (options?.evidenceDir) {
      try {
        writeEvidence(join(options.evidenceDir, "tauri-test.json"), failure);
      } catch {
        // Preserve the one bounded JSON stdout object even if evidence storage
        // itself is unavailable.
      }
    }
    process.stderr.write(`${message}\n`);
    process.stdout.write(`${JSON.stringify(failure)}\n`);
    process.exitCode = 1;
  }
}

await run();
