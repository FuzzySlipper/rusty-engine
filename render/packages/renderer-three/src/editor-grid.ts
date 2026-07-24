import * as THREE from 'three';
import type {
  CameraBasis,
  CameraPose,
  EditorGridDescriptor,
  EditorGridPlane,
  EditorGridProjectionReadout,
  PerspectiveProjection,
} from '@rusty-engine/render-contracts';

export interface EditorGridProjectionCamera {
  readonly basis: CameraBasis;
  readonly pose: CameraPose;
  readonly projection: PerspectiveProjection;
}

export interface EditorGridProjectionSize {
  readonly height: number;
  readonly width: number;
}

interface ProjectedGridLine {
  readonly a: readonly [number, number, number];
  readonly alpha: number;
  readonly b: readonly [number, number, number];
  readonly color: readonly [number, number, number, number];
}

export interface EditorGridProjection {
  readonly lines: readonly ProjectedGridLine[];
  readonly readout: EditorGridProjectionReadout;
}

interface PlaneAxes {
  readonly a: 0 | 1 | 2;
  readonly b: 0 | 1 | 2;
  readonly normal: 0 | 1 | 2;
}

const MAX_MINOR_LINES_PER_AXIS = 256;

/** Pure camera-to-grid projection used by the backend and conformance tests. */
export function projectEditorGrid(
  descriptor: EditorGridDescriptor,
  camera: EditorGridProjectionCamera,
  size: EditorGridProjectionSize,
): EditorGridProjection {
  if (!descriptor.visible) {
    return {
      lines: [],
      readout: {
        descriptor,
        bounds: null,
        minorLineStep: 1,
        renderedLineCount: 0,
      },
    };
  }
  const axes = planeAxes(descriptor.plane);
  const position = camera.pose.position;
  const forward = camera.basis.forward;
  const planeCoordinate = descriptor.grid.origin[axes.normal];
  const denominator = forward[axes.normal];
  const unclampedIntersection = Math.abs(denominator) < 1e-6
    ? 0
    : (planeCoordinate - position[axes.normal]) / denominator;
  const intersectionDistance = Number.isFinite(unclampedIntersection) && unclampedIntersection > 0
    ? Math.min(unclampedIntersection, descriptor.style.fadeEnd)
    : 0;
  const center = [...position] as [number, number, number];
  center[axes.a] += forward[axes.a] * intersectionDistance;
  center[axes.b] += forward[axes.b] * intersectionDistance;
  center[axes.normal] = planeCoordinate;

  const aspect = Math.max(1e-6, size.width / Math.max(1, size.height));
  const cameraDistance = Math.abs(position[axes.normal] - planeCoordinate);
  const halfVertical = cameraDistance * Math.tan(camera.projection.fovYDegrees * Math.PI / 360);
  const minimumExtent = Math.max(
    descriptor.grid.spacing[axes.a],
    descriptor.grid.spacing[axes.b],
  ) * descriptor.style.majorLineEvery * 4;
  const viewExtent = Math.max(halfVertical, halfVertical * aspect, intersectionDistance * 0.35);
  const extent = Math.min(
    descriptor.style.fadeEnd,
    Math.max(minimumExtent, viewExtent * 1.75),
  );

  const boundsMin = [...descriptor.grid.origin] as [number, number, number];
  const boundsMax = [...descriptor.grid.origin] as [number, number, number];
  boundsMin[axes.a] = gridBoundaryAtOrBefore(
    center[axes.a] - extent,
    descriptor.grid.origin[axes.a],
    descriptor.grid.spacing[axes.a],
  );
  boundsMax[axes.a] = gridBoundaryAtOrAfter(
    center[axes.a] + extent,
    descriptor.grid.origin[axes.a],
    descriptor.grid.spacing[axes.a],
  );
  boundsMin[axes.b] = gridBoundaryAtOrBefore(
    center[axes.b] - extent,
    descriptor.grid.origin[axes.b],
    descriptor.grid.spacing[axes.b],
  );
  boundsMax[axes.b] = gridBoundaryAtOrAfter(
    center[axes.b] + extent,
    descriptor.grid.origin[axes.b],
    descriptor.grid.spacing[axes.b],
  );
  boundsMin[axes.normal] = planeCoordinate;
  boundsMax[axes.normal] = planeCoordinate;

  const aStart = boundaryIndex(boundsMin[axes.a], descriptor.grid.origin[axes.a], descriptor.grid.spacing[axes.a]);
  const aEnd = boundaryIndex(boundsMax[axes.a], descriptor.grid.origin[axes.a], descriptor.grid.spacing[axes.a]);
  const bStart = boundaryIndex(boundsMin[axes.b], descriptor.grid.origin[axes.b], descriptor.grid.spacing[axes.b]);
  const bEnd = boundaryIndex(boundsMax[axes.b], descriptor.grid.origin[axes.b], descriptor.grid.spacing[axes.b]);
  const maximumMinorCount = Math.max(aEnd - aStart + 1, bEnd - bStart + 1);
  const minorLineStep = nextPowerOfTwo(Math.max(1, Math.ceil(maximumMinorCount / MAX_MINOR_LINES_PER_AXIS)));
  const lines: ProjectedGridLine[] = [];

  appendLines(lines, {
    descriptor,
    fixedAxis: axes.a,
    lineAxis: axes.b,
    normalAxis: axes.normal,
    startIndex: aStart,
    endIndex: aEnd,
    lineMin: boundsMin[axes.b],
    lineMax: boundsMax[axes.b],
    center,
    minorLineStep,
  });
  appendLines(lines, {
    descriptor,
    fixedAxis: axes.b,
    lineAxis: axes.a,
    normalAxis: axes.normal,
    startIndex: bStart,
    endIndex: bEnd,
    lineMin: boundsMin[axes.a],
    lineMax: boundsMax[axes.a],
    center,
    minorLineStep,
  });

  return {
    lines,
    readout: {
      descriptor,
      bounds: { min: boundsMin, max: boundsMax },
      minorLineStep,
      renderedLineCount: lines.length,
    },
  };
}

interface AppendLinesInput {
  readonly center: readonly [number, number, number];
  readonly descriptor: EditorGridDescriptor;
  readonly endIndex: number;
  readonly fixedAxis: 0 | 1 | 2;
  readonly lineAxis: 0 | 1 | 2;
  readonly lineMax: number;
  readonly lineMin: number;
  readonly minorLineStep: number;
  readonly normalAxis: 0 | 1 | 2;
  readonly startIndex: number;
}

function appendLines(lines: ProjectedGridLine[], input: AppendLinesInput): void {
  const { descriptor } = input;
  for (let index = input.startIndex; index <= input.endIndex; index += 1) {
    const isAxis = index === 0;
    const isMajor = Math.abs(index) % descriptor.style.majorLineEvery === 0;
    if (!isAxis && !isMajor && Math.abs(index) % input.minorLineStep !== 0) {
      continue;
    }
    const fixed = descriptor.grid.origin[input.fixedAxis]
      + index * descriptor.grid.spacing[input.fixedAxis];
    const a = [...descriptor.grid.origin] as [number, number, number];
    const b = [...descriptor.grid.origin] as [number, number, number];
    a[input.fixedAxis] = fixed;
    b[input.fixedAxis] = fixed;
    a[input.lineAxis] = input.lineMin;
    b[input.lineAxis] = input.lineMax;
    a[input.normalAxis] = descriptor.grid.origin[input.normalAxis];
    b[input.normalAxis] = descriptor.grid.origin[input.normalAxis];
    const color = isAxis
      ? axisColor(descriptor, input.lineAxis)
      : isMajor
        ? descriptor.style.majorColor
        : descriptor.style.minorColor;
    const distance = Math.abs(fixed - input.center[input.fixedAxis]);
    const fade = distance <= descriptor.style.fadeStart
      ? 1
      : 1 - Math.min(1, (distance - descriptor.style.fadeStart)
        / (descriptor.style.fadeEnd - descriptor.style.fadeStart));
    lines.push({ a, b, color, alpha: color[3] * descriptor.style.opacity * fade });
  }
}

function planeAxes(plane: EditorGridPlane): PlaneAxes {
  switch (plane) {
    case 'xy': return { a: 0, b: 1, normal: 2 };
    case 'yz': return { a: 1, b: 2, normal: 0 };
    case 'xz': return { a: 0, b: 2, normal: 1 };
  }
}

function axisColor(
  descriptor: EditorGridDescriptor,
  lineAxis: 0 | 1 | 2,
): readonly [number, number, number, number] {
  if (lineAxis === 0) return descriptor.style.xAxisColor;
  if (lineAxis === 1) return descriptor.style.yAxisColor;
  return descriptor.style.zAxisColor;
}

function gridBoundaryAtOrBefore(value: number, origin: number, spacing: number): number {
  return origin + Math.floor((value - origin) / spacing) * spacing;
}

function gridBoundaryAtOrAfter(value: number, origin: number, spacing: number): number {
  return origin + Math.ceil((value - origin) / spacing) * spacing;
}

function boundaryIndex(value: number, origin: number, spacing: number): number {
  return Math.round((value - origin) / spacing);
}

function nextPowerOfTwo(value: number): number {
  return 2 ** Math.ceil(Math.log2(value));
}

/** Three.js realization kept behind the public renderer backend. */
export class ThreeEditorGridProjection {
  readonly scene = new THREE.Scene();
  #camera: EditorGridProjectionCamera | null = null;
  #descriptor: EditorGridDescriptor | null = null;
  #geometry: THREE.BufferGeometry | null = null;
  #lines: THREE.LineSegments | null = null;
  #material: THREE.ShaderMaterial | null = null;
  #readout: EditorGridProjectionReadout | null = null;
  #size: EditorGridProjectionSize = { width: 800, height: 450 };

  setDescriptor(descriptor: EditorGridDescriptor | null): void {
    this.#descriptor = descriptor === null ? null : structuredClone(descriptor);
    this.#rebuild();
  }

  setCamera(camera: EditorGridProjectionCamera): void {
    this.#camera = structuredClone(camera);
    this.#rebuild();
  }

  resize(size: EditorGridProjectionSize): void {
    this.#size = { ...size };
    this.#rebuild();
  }

  readout(): EditorGridProjectionReadout | null {
    return this.#readout === null ? null : structuredClone(this.#readout);
  }

  snapshot(): string {
    return JSON.stringify(this.#readout);
  }

  dispose(): void {
    this.#removeProjection();
    this.#camera = null;
    this.#descriptor = null;
    this.#readout = null;
  }

  #rebuild(): void {
    this.#removeProjection();
    if (this.#descriptor === null || this.#camera === null) {
      this.#readout = null;
      return;
    }
    const projection = projectEditorGrid(this.#descriptor, this.#camera, this.#size);
    this.#readout = projection.readout;
    if (projection.lines.length === 0) return;
    const positions: number[] = [];
    const colors: number[] = [];
    const alphas: number[] = [];
    for (const line of projection.lines) {
      positions.push(...line.a, ...line.b);
      colors.push(line.color[0], line.color[1], line.color[2]);
      colors.push(line.color[0], line.color[1], line.color[2]);
      alphas.push(line.alpha, line.alpha);
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
    geometry.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
    geometry.setAttribute('lineAlpha', new THREE.Float32BufferAttribute(alphas, 1));
    const material = new THREE.ShaderMaterial({
      transparent: true,
      depthTest: true,
      depthWrite: false,
      vertexShader: `
        attribute vec3 color;
        attribute float lineAlpha;
        varying vec3 vColor;
        varying float vAlpha;
        void main() {
          vColor = color;
          vAlpha = lineAlpha;
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        }
      `,
      fragmentShader: `
        varying vec3 vColor;
        varying float vAlpha;
        void main() {
          gl_FragColor = vec4(vColor, vAlpha);
        }
      `,
    });
    const lines = new THREE.LineSegments(geometry, material);
    lines.frustumCulled = false;
    lines.renderOrder = -1000;
    lines.name = 'rusty-editor-grid';
    this.scene.add(lines);
    this.#geometry = geometry;
    this.#material = material;
    this.#lines = lines;
  }

  #removeProjection(): void {
    if (this.#lines !== null) this.scene.remove(this.#lines);
    this.#geometry?.dispose();
    this.#material?.dispose();
    this.#lines = null;
    this.#geometry = null;
    this.#material = null;
  }
}
