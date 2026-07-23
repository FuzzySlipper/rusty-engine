import { spawn } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { createServer } from "node:net";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const chromium = process.env.CHROMIUM_BIN ?? "/usr/bin/chromium";
if (!existsSync(chromium)) {
  throw new Error(`Chromium is required for the product smoke (${chromium} not found)`);
}

const bundleDirectory = resolve(repoRoot, "ts/packages/browser-shell/dist/assets");
const browserBundle = readdirSync(bundleDirectory)
  .filter((name) => name.endsWith(".js"))
  .map((name) => readFileSync(resolve(bundleDirectory, name), "utf8"))
  .join("\n");
const forbiddenRuntimeSurface = [
  "GameplayRuntimeHost",
  "GameplayFabric",
  "NativeRuntimeBridge",
  "RuntimeSession",
  "AnimationProjectionOp",
  "AudioProjectionOp",
  "BillboardProjectionOp",
  "ParticleProjectionOp",
];
const bundledRuntimeSurface = forbiddenRuntimeSurface.filter((name) => browserBundle.includes(name));
if (bundledRuntimeSurface.length > 0) {
  throw new Error(`browser bundle imported old runtime surface: ${bundledRuntimeSurface.join(", ")}`);
}

const port = await reservePort();
const address = `127.0.0.1:${String(port)}`;
const host = spawn(
  "cargo",
  ["run", "-q", "-p", "game-host", "--bin", "browser-host", "--", "--addr", address],
  { cwd: repoRoot, stdio: ["ignore", "pipe", "pipe"] },
);
let hostOutput = "";
host.stdout.on("data", (chunk) => {
  hostOutput += String(chunk);
});
host.stderr.on("data", (chunk) => {
  hostOutput += String(chunk);
});

try {
  await waitForHealth(`http://${address}/health`, host);
  const result = await run(chromium, [
    "--headless=new",
    "--no-sandbox",
    "--disable-dev-shm-usage",
    "--use-gl=angle",
    "--use-angle=swiftshader",
    "--enable-unsafe-swiftshader",
    "--autoplay-policy=no-user-gesture-required",
    "--virtual-time-budget=10000",
    "--dump-dom",
    `http://${address}/?smoke=1`,
  ]);
  if (result.code !== 0) {
    throw new Error(`Chromium exited ${String(result.code)}\n${result.stderr.slice(-4_000)}`);
  }
  const required = [
    'data-smoke-status="pass"',
    'data-status="pass"',
    'data-held-input="pass"',
    'data-gate-passage="pass"',
    'data-queue-recovery="pass"',
    'data-cooldown="pass"',
    'data-feedback-reset="pass"',
    'data-feedback-families="pass"',
    'data-audio-feedback="pass"',
    'data-feedback-drop="pass"',
    "PASS · Rust facts reached retained WebGL and disposable feedback",
    "EnemyDefeated",
    "EncounterCleared",
    "DoorOpened",
    "KinematicBlocked",
    "NavigationArrived",
    "PlayerMoved",
    "PlayerBlocked",
    "PlayerLookChanged",
    "CombatHit",
    "DamageApplied",
    "CombatEnemyDefeated",
    "CombatRejected",
    "SEED 4",
  ];
  const missing = required.filter((marker) => !result.stdout.includes(marker));
  if (missing.length > 0) {
    throw new Error(`browser smoke missing ${missing.join(", ")}\n${result.stdout.slice(-6_000)}`);
  }
  console.log("browser smoke passed: accepted gameplay -> retained Three/WebGL + disposable feedback shell");
} finally {
  host.kill("SIGTERM");
  await Promise.race([onceExit(host), delay(1_000)]);
  if (host.exitCode === null) {
    host.kill("SIGKILL");
  }
}

async function reservePort() {
  const server = createServer();
  await new Promise((resolveListen, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolveListen);
  });
  const address = server.address();
  if (address === null || typeof address === "string") {
    server.close();
    throw new Error("could not reserve a browser-smoke port");
  }
  const { port } = address;
  await new Promise((resolveClose, reject) =>
    server.close((error) => (error === undefined ? resolveClose() : reject(error))),
  );
  return port;
}

async function waitForHealth(url, process) {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    if (process.exitCode !== null) {
      throw new Error(`browser host exited early (${String(process.exitCode)})\n${hostOutput}`);
    }
    try {
      const response = await fetch(url);
      if (response.ok && (await response.text()).trim() === "ok") {
        return;
      }
    } catch {
      // Compilation and listener startup can take a moment on a clean checkout.
    }
    await delay(100);
  }
  throw new Error(`browser host did not become healthy\n${hostOutput}`);
}

function run(command, args) {
  return new Promise((resolveRun, reject) => {
    const child = spawn(command, args, { cwd: repoRoot, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += String(chunk);
    });
    child.stderr.on("data", (chunk) => {
      stderr += String(chunk);
    });
    child.once("error", reject);
    child.once("exit", (code) => resolveRun({ code, stdout, stderr }));
  });
}

function onceExit(process) {
  if (process.exitCode !== null) {
    return Promise.resolve();
  }
  return new Promise((resolveExit) => process.once("exit", resolveExit));
}

function delay(milliseconds) {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));
}
