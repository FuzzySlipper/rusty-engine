import { mountAshaRendererBrowserSurface } from "@asha/renderer-three/backend";

import "./style.css";
import { RuntimeProjectionAdapter, type RuntimeBrowserState } from "./projection.js";

const canvas = requiredElement("viewport", HTMLCanvasElement);
const encounterState = requiredElement("encounter-state", HTMLElement);
const revision = requiredElement("revision", HTMLElement);
const doorCaption = requiredElement("door-caption", HTMLElement);
const enemyList = requiredElement("enemy-list", HTMLElement);
const motionState = requiredElement("motion-state", HTMLElement);
const eventList = requiredElement("event-list", HTMLOListElement);
const rendererStatus = requiredElement("renderer-status", HTMLElement);
const smokeResult = requiredElement("smoke-result", HTMLElement);
const projection = new RuntimeProjectionAdapter();
const eventHistory: string[] = [];

let current = await requestState("/api/state");
const surface = mountAshaRendererBrowserSurface(canvas, {
  autoStart: true,
  camera: {
    initialPose: { position: [0, 4.4, 15], pitchDegrees: -10, yawDegrees: 0 },
    projection: { fovYDegrees: 50, near: 0.1, far: 100 },
  },
  clearColor: 0x071012,
  frame: projection.apply(current),
  pixelRatio: Math.min(globalThis.devicePixelRatio ?? 1, 2),
});
renderReadout(current);
rendererStatus.textContent = `${surface.kind} · ${String(projection.trackedEntityCount)} retained entities`;

for (const button of document.querySelectorAll<HTMLButtonElement>("[data-enemy-id]")) {
  button.addEventListener("click", () => {
    const enemy = Number(button.dataset.enemyId);
    void perform(`/api/defeat/${String(enemy)}`);
  });
}
requiredElement("reset", HTMLButtonElement).addEventListener("click", () => {
  void perform("/api/reset");
});
requiredElement("run-motion", HTMLButtonElement).addEventListener("click", () => {
  void perform("/api/motion-phase");
});

if (new URLSearchParams(location.search).has("smoke")) {
  await perform("/api/reset");
  await perform("/api/defeat/4");
  await perform("/api/defeat/5");
  await perform("/api/motion-phase");
  surface.renderOnce();
  const door = current.projection.find((node) => node.id === 3);
  const passed =
    current.encounterState === "cleared" &&
    current.doorState === "open" &&
    door?.translation?.[1] === 3 &&
    current.enemies.every((enemy) => enemy.state === "defeated") &&
    current.motionState === "blocked" &&
    (current.projection.find((node) => node.id === 10)?.translation?.[0] ?? -4) > 2 &&
    surface.snapshot().includes("loading-bay-exit");
  smokeResult.dataset.status = passed ? "pass" : "fail";
  smokeResult.textContent = passed
    ? "PASS · Rust facts reached retained WebGL projection"
    : "FAIL · Product proof did not converge";
  document.body.dataset.smokeStatus = passed ? "pass" : "fail";
}

async function perform(path: string): Promise<void> {
  current = await requestState(path, "POST");
  if (path === "/api/reset") {
    eventHistory.length = 0;
  }
  eventHistory.push(...current.lastEvents);
  const frame = projection.apply(current);
  if (frame.ops.length > 0) {
    surface.applyFrame(frame);
  }
  surface.renderOnce();
  renderReadout(current);
  rendererStatus.textContent = `${surface.kind} · ${String(projection.trackedEntityCount)} retained entities`;
}

function renderReadout(state: RuntimeBrowserState): void {
  encounterState.textContent = state.encounterState.toUpperCase();
  encounterState.dataset.state = state.encounterState;
  revision.textContent = `REV ${String(state.entityRevision)}`;
  doorCaption.textContent = state.doorState === "open" ? "OPEN" : "LOCKED";
  doorCaption.dataset.state = state.doorState;
  motionState.textContent = state.motionState.toUpperCase();
  motionState.dataset.state = state.motionState;
  enemyList.replaceChildren(
    ...state.enemies.map((enemy) => {
      const row = document.createElement("div");
      row.className = "enemy-row";
      row.dataset.state = enemy.state;
      const name = document.createElement("span");
      name.textContent = enemy.name;
      const status = document.createElement("strong");
      status.textContent = enemy.state.toUpperCase();
      row.append(name, status);
      return row;
    }),
  );
  eventList.replaceChildren(
    ...(eventHistory.length === 0
      ? ["Awaiting action"]
      : eventHistory
    ).map((event) => {
        const item = document.createElement("li");
        item.textContent = event;
        return item;
      }),
  );
}

async function requestState(path: string, method = "GET"): Promise<RuntimeBrowserState> {
  const response = await fetch(path, { method });
  if (!response.ok) {
    throw new Error(`${method} ${path} failed with ${String(response.status)}`);
  }
  return (await response.json()) as RuntimeBrowserState;
}

function requiredElement<T extends Element>(id: string, constructor: { new (): T }): T {
  const element = document.getElementById(id);
  if (!(element instanceof constructor)) {
    throw new Error(`missing required element #${id}`);
  }
  return element;
}
