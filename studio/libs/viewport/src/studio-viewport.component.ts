import {
  ChangeDetectionStrategy,
  Component,
  ViewChild,
  effect,
  input,
  output,
  signal,
  type AfterViewInit,
  type ElementRef,
  type OnDestroy,
} from '@angular/core';
import type { EditorGridDescriptor, RenderFrameDiff, Transform } from '@rusty-engine/render-contracts';
import {
  mountRendererInspectionSurface,
  type RendererAnimatedMeshResourceManifest,
  type RendererAnimatedMeshResourceResolver,
  type RendererInspectionSurface,
  type RendererInspectionSurfaceControlPreferences,
  type RendererMeshResourceManifest,
  type RendererMeshResourceResolver,
} from '@rusty-engine/renderer-host';

import {
  STUDIO_EDITOR_GRID,
  canvasPoint,
  movedPastPickThreshold,
  presentStudioLighting,
  presentStudioSelection,
  type StudioLightingMode,
  type StudioVoxelPreview,
} from './viewport-model.js';
import {
  beginStudioTransformManipulatorDrag,
  cancelStudioTransformManipulatorDrag,
  projectStudioTransformManipulator,
  studioTransformHandleFromId,
  updateStudioTransformManipulatorDrag,
  type StudioTransformHandle,
  type StudioTransformManipulatorCamera,
  type StudioTransformManipulatorDrag,
  type StudioTransformOrientation,
  type StudioTransformSnapping,
  type StudioTransformTool,
} from './transform-manipulator.js';

type ViewportStatus = 'mounting' | 'ready' | 'error' | 'disposed';

export interface VoxelViewportPickCandidate {
  readonly instanceId: string;
  readonly cameraOrigin: readonly [number, number, number];
  readonly direction: readonly [number, number, number];
  readonly worldPoint: readonly [number, number, number];
  readonly worldNormal: readonly [number, number, number];
  readonly maxDistance: number;
}

export interface StudioTransformGizmoDragFinished {
  readonly cancelled: boolean;
}

export interface StudioVoxelObjectPlacementPick {
  readonly worldPoint: readonly [number, number, number];
  readonly worldNormal: readonly [number, number, number];
  readonly sourceEntity: number | null;
}

@Component({
  selector: 'rusty-studio-viewport',
  standalone: true,
  templateUrl: './studio-viewport.component.html',
  styleUrl: './studio-viewport.component.css',
  host: {
    '[attr.data-renderer-status]': 'status()',
    '[attr.data-retained-ops]': 'retainedOpCount()',
    '[attr.data-selected-entity]': 'selectedEntityId()',
    '[attr.data-pick-revision]': 'pickRevision()',
    '[attr.data-authored-frame-hash]': 'retainedFrameHash()',
    '[attr.data-preview-applied]': 'previewApplied()',
    '[attr.data-voxel-preview-kind]': 'voxelPreviewKind()',
    '[attr.data-object-placement-interactive]': 'objectPlacementInteractive()',
    '[attr.data-selected-render-handle]': 'selectedRenderHandle()',
    '[attr.data-animated-mesh-resources]': 'animatedMeshManifest()?.resources?.length ?? 0',
    '[attr.data-mesh-resources]': 'meshResourceManifest()?.resources?.length ?? 0',
    '[attr.data-voxel-object-definitions]': 'voxelObjectDefinitionCount()',
    '[attr.data-voxel-object-instances]': 'voxelObjectInstanceCount()',
    '[attr.data-voxel-object-placement-ghosts]': 'voxelObjectPlacementGhostCount()',
    '[attr.data-camera-move-speed]': 'controlPreferences().moveSpeed',
    '[attr.data-camera-move-forward]': 'controlPreferences().keyboard.moveForward',
    '[attr.data-camera-revision]': 'cameraRevision()',
    '[attr.data-renderer-error]': 'lastRendererError()',
    '[attr.data-lighting-mode]': 'lightingMode()',
    '[attr.data-work-light-active]': 'workLightActive()',
    '[attr.data-transform-gizmo-visible]': 'manipulatorTransform() !== null && transformTool() !== null',
    '[attr.data-transform-tool]': 'transformTool()',
    '[attr.data-transform-orientation]': 'transformOrientation()',
    '[attr.data-active-transform-handle]': 'activeTransformHandleLabel()',
    '[attr.data-hovered-transform-handle]': 'hoveredTransformHandleLabel()',
  },
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class StudioViewportComponent implements AfterViewInit, OnDestroy {
  readonly frame = input<RenderFrameDiff | null>(null);
  readonly framePatch = input<RenderFrameDiff | null>(null);
  readonly frameGeneration = input(0);
  readonly lightingMode = input<StudioLightingMode>('work_light');
  readonly grid = input<EditorGridDescriptor | null>(STUDIO_EDITOR_GRID);
  readonly controlPreferences = input<RendererInspectionSurfaceControlPreferences>({
    moveSpeed: 6,
    boostMultiplier: 4,
    invertLookY: false,
    invertPanY: false,
    keyboard: {
      moveForward: 'KeyW',
      moveBackward: 'KeyS',
      moveLeft: 'KeyA',
      moveRight: 'KeyD',
      moveDown: 'KeyQ',
      moveUp: 'KeyE',
      boost: 'ShiftLeft',
    },
  });
  readonly selectedEntityId = input<number | null>(null);
  readonly previewEntityId = input<number | null>(null);
  readonly previewTransform = input<Transform | null>(null);
  readonly manipulatorTransform = input<Transform | null>(null);
  readonly transformTool = input<StudioTransformTool | null>(null);
  readonly transformOrientation = input<StudioTransformOrientation>('world');
  readonly transformSnapping = input<StudioTransformSnapping>({
    enabled: true,
    rotationDegrees: 15,
    scale: [0.1, 0.1, 0.1],
    translation: [0.25, 0.25, 0.25],
  });
  readonly voxelPreview = input<StudioVoxelPreview | null>(null);
  readonly voxelObjectPlacementResourceFrame = input<RenderFrameDiff | null>(null);
  readonly objectPlacementInteractive = input(true);
  readonly animatedMeshManifest = input<RendererAnimatedMeshResourceManifest | null>(null);
  readonly resolveAnimatedMeshResource = input<RendererAnimatedMeshResourceResolver | null>(null);
  readonly animatedMeshResourceKey = input('');
  readonly meshResourceManifest = input<RendererMeshResourceManifest | null>(null);
  readonly resolveMeshResource = input<RendererMeshResourceResolver | null>(null);
  readonly meshResourceKey = input('');

  readonly entityPicked = output<number | null>();
  readonly frameApplied = output<number>();
  readonly voxelPicked = output<VoxelViewportPickCandidate>();
  readonly rendererError = output<string>();
  readonly transformDragStarted = output<void>();
  readonly transformCandidate = output<Transform>();
  readonly transformDragFinished = output<StudioTransformGizmoDragFinished>();
  readonly transformRevertRequested = output<void>();
  readonly voxelObjectPlacementPicked = output<StudioVoxelObjectPlacementPick>();
  readonly voxelObjectPlacementCommitRequested = output<void>();
  readonly voxelObjectPlacementCancelRequested = output<void>();

  readonly status = signal<ViewportStatus>('mounting');
  readonly retainedOpCount = signal(0);
  readonly cameraRevision = signal(0);
  readonly pickRevision = signal(0);
  readonly retainedFrameHash = signal('');
  readonly previewApplied = signal(false);
  readonly voxelPreviewKind = signal<StudioVoxelPreview['kind'] | null>(null);
  readonly selectedRenderHandle = signal<number | null>(null);
  readonly lastRendererError = signal('');
  readonly workLightActive = signal(true);
  readonly voxelObjectDefinitionCount = signal(0);
  readonly voxelObjectInstanceCount = signal(0);
  readonly voxelObjectPlacementGhostCount = signal(0);
  readonly activeTransformHandleLabel = signal<string | null>(null);
  readonly hoveredTransformHandleLabel = signal<string | null>(null);

  @ViewChild('canvas', { static: true })
  private canvasElement!: ElementRef<HTMLCanvasElement>;

  #surface: RendererInspectionSurface | null = null;
  #destroyed = false;
  #viewReady = false;
  #mountRevision = 0;
  #lastResourceKey = '';
  #lastPresentationKey = '';
  #lastAppliedFrameGeneration = -1;
  #authoredFrameReady = false;
  #lastManipulatorKey = '';
  #pointerStart: readonly [number, number] | null = null;
  #pointerDragged = false;
  #suppressNextClick = false;
  #suppressClickTimer: ReturnType<typeof setTimeout> | null = null;
  #activeManipulatorDrag: {
    readonly pointerId: number;
    readonly drag: StudioTransformManipulatorDrag;
  } | null = null;
  #activeManipulatorHandle: StudioTransformHandle | null = null;
  #hoveredManipulatorHandle: StudioTransformHandle | null = null;

  constructor() {
    effect(() => {
      const generation = this.frameGeneration();
      const frame = this.frame();
      const framePatch = this.framePatch();
      const selectedEntityId = this.selectedEntityId();
      const previewEntityId = this.previewEntityId();
      const previewTransform = this.previewTransform();
      const voxelPreview = this.voxelPreview();
      const voxelObjectPlacementResourceFrame = this.voxelObjectPlacementResourceFrame();
      const lightingMode = this.lightingMode();
      if (frame !== null) {
        this.#replaceFrame(
          frame,
          framePatch,
          generation,
          selectedEntityId,
          previewEntityId,
          previewTransform,
          voxelPreview,
          voxelObjectPlacementResourceFrame,
          lightingMode,
        );
      }
    });
    effect(() => {
      this.#setGrid(this.grid());
    });
    effect(() => {
      this.#replaceManipulatorOverlay(
        this.manipulatorTransform(),
        this.transformTool(),
        this.transformOrientation(),
        this.activeTransformHandleLabel(),
        this.hoveredTransformHandleLabel(),
      );
    });
    effect(() => {
      const preferences = this.controlPreferences();
      this.#configureControls(preferences);
    });
    effect(() => {
      const manifest = this.animatedMeshManifest();
      const resolver = this.resolveAnimatedMeshResource();
      const resourceKey = this.animatedMeshResourceKey();
      const meshManifest = this.meshResourceManifest();
      const meshResolver = this.resolveMeshResource();
      const meshResourceKey = this.meshResourceKey();
      const key = JSON.stringify([
        resourceKey,
        manifest?.kind ?? null,
        manifest?.resources ?? [],
        resolver === null,
        meshResourceKey,
        meshManifest?.kind ?? null,
        meshManifest?.resources ?? [],
        meshResolver === null,
      ]);
      if (key === this.#lastResourceKey) return;
      this.#lastResourceKey = key;
      if (this.#viewReady) void this.#mount();
    });
  }

  ngAfterViewInit(): void {
    this.#viewReady = true;
    void this.#mount();
  }

  ngOnDestroy(): void {
    this.#destroyed = true;
    if (this.#suppressClickTimer !== null) clearTimeout(this.#suppressClickTimer);
    this.#surface?.dispose();
    this.#surface = null;
    this.status.set('disposed');
  }

  focusTarget(target: readonly [number, number, number]): boolean {
    const surface = this.#surface;
    if (surface === null || !surface.focusTarget(target)) return false;
    this.canvasElement.nativeElement.focus({ preventScroll: true });
    this.#syncReadout();
    return true;
  }

  pointerDown(event: PointerEvent): void {
    if (event.button !== 0) return;
    if (this.voxelPreview()?.kind !== 'objectPlacement' && this.#beginManipulatorDrag(event)) return;
    this.#pointerStart = [event.clientX, event.clientY];
    this.#pointerDragged = false;
  }

  pointerMove(event: PointerEvent): void {
    const active = this.#activeManipulatorDrag;
    if (active !== null && active.pointerId === event.pointerId) {
      event.preventDefault();
      event.stopImmediatePropagation();
      this.#pointerDragged = true;
      this.#emitManipulatorCandidate(active.drag, event);
      return;
    }
    if (event.buttons === 0) this.#updateManipulatorHover(event);
    if (this.#pointerStart === null || this.#pointerDragged) return;
    this.#pointerDragged = movedPastPickThreshold(
      this.#pointerStart,
      [event.clientX, event.clientY],
    );
  }

  pointerUp(event: PointerEvent): void {
    const active = this.#activeManipulatorDrag;
    if (active !== null && active.pointerId === event.pointerId) {
      event.preventDefault();
      event.stopImmediatePropagation();
      this.#emitManipulatorCandidate(active.drag, event);
      this.#finishManipulatorDrag(event.pointerId, false);
      return;
    }
    this.#pointerStart = null;
  }

  pointerCancel(event: PointerEvent): void {
    const active = this.#activeManipulatorDrag;
    if (active !== null && active.pointerId === event.pointerId) {
      event.preventDefault();
      event.stopImmediatePropagation();
      this.transformCandidate.emit(cancelStudioTransformManipulatorDrag(active.drag).transform);
      this.#finishManipulatorDrag(event.pointerId, true);
      return;
    }
    this.#pointerStart = null;
  }

  keyDown(event: KeyboardEvent): void {
    if (this.voxelPreview()?.kind === 'objectPlacement') {
      if (!this.objectPlacementInteractive()) return;
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopImmediatePropagation();
        this.voxelObjectPlacementCancelRequested.emit();
      } else if (event.key === 'Enter') {
        event.preventDefault();
        event.stopImmediatePropagation();
        this.voxelObjectPlacementCommitRequested.emit();
      }
      return;
    }
    if (event.key !== 'Escape' || this.previewTransform() === null) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    const active = this.#activeManipulatorDrag;
    if (active !== null) {
      this.transformCandidate.emit(cancelStudioTransformManipulatorDrag(active.drag).transform);
      this.#finishManipulatorDrag(active.pointerId, true);
    } else {
      this.transformRevertRequested.emit();
    }
  }

  pick(event: MouseEvent): void {
    if (this.#suppressNextClick) {
      this.#suppressNextClick = false;
      if (this.#suppressClickTimer !== null) clearTimeout(this.#suppressClickTimer);
      this.#suppressClickTimer = null;
      return;
    }
    if (this.#pointerDragged) {
      this.#pointerDragged = false;
      return;
    }
    const surface = this.#surface;
    if (surface === null) return;
    const placingObject = this.voxelPreview()?.kind === 'objectPlacement';
    if (placingObject && !this.objectPlacementInteractive()) return;
    const receipt = surface.pick({
      point: canvasPoint(
        [event.clientX, event.clientY],
        this.canvasElement.nativeElement.getBoundingClientRect(),
      ),
      filter: placingObject
        ? { channels: ['authored'], layers: ['scene'] }
        : { channels: ['authored'] },
    });
    if (receipt.diagnostics.length > 0) {
      this.#report(receipt.diagnostics.map((entry) => entry.message).join('; '));
      return;
    }
    this.pickRevision.update((revision) => revision + 1);
    const hint = receipt.hint;
    if (placingObject) {
      if (hint === null) {
        this.#report('Voxel-object placement requires a visible authored surface or a numeric transform.');
        return;
      }
      this.voxelObjectPlacementPicked.emit({
        worldPoint: hint.position,
        worldNormal: hint.normal,
        sourceEntity: hint.sourceTrace?.entity ?? null,
      });
      return;
    }
    if (hint === null) {
      this.entityPicked.emit(null);
      return;
    }
    const instanceTag = hint.tags.find((tag) => tag.startsWith('voxel-instance:'));
    if (instanceTag !== undefined) {
      const cameraOrigin = surface.camera().pose.position;
      const direction = normalize([
        hint.position[0] - cameraOrigin[0],
        hint.position[1] - cameraOrigin[1],
        hint.position[2] - cameraOrigin[2],
      ]);
      if (direction === null) {
        this.#report('voxel pick could not derive a finite camera ray');
        return;
      }
      this.voxelPicked.emit({
        instanceId: instanceTag.slice('voxel-instance:'.length),
        cameraOrigin,
        direction,
        worldPoint: hint.position,
        worldNormal: hint.normal,
        maxDistance: Math.max(0.01, hint.distance + 0.01),
      });
      return;
    }
    this.entityPicked.emit(hint.sourceTrace?.entity ?? null);
  }

  async #mount(): Promise<void> {
    const mountRevision = ++this.#mountRevision;
    const previous = this.#surface;
    this.#surface = null;
    previous?.dispose();
    this.#lastPresentationKey = '';
    this.#lastAppliedFrameGeneration = -1;
    this.#authoredFrameReady = false;
    this.#lastManipulatorKey = '';
    this.lastRendererError.set('');
    this.status.set('mounting');
    try {
      const animatedMeshManifest = this.animatedMeshManifest();
      const resolveAnimatedMeshResource = this.resolveAnimatedMeshResource();
      const meshResourceManifest = this.meshResourceManifest();
      const resolveMeshResource = this.resolveMeshResource();
      if ((animatedMeshManifest === null) !== (resolveAnimatedMeshResource === null)) {
        throw new Error('animated mesh manifest and resolver must be supplied together');
      }
      if ((meshResourceManifest === null) !== (resolveMeshResource === null)) {
        throw new Error('mesh resource manifest and resolver must be supplied together');
      }
      const surface = await mountRendererInspectionSurface(
        this.canvasElement.nativeElement,
        {
          autoStart: true,
          clearColor: 0x0d1418,
          controls: {
            enabled: true,
            initialPosition: [15, 13, 22],
            initialTarget: [4.5, 1.5, 7],
            ...this.controlPreferences(),
          },
          initialGrid: this.grid(),
          ...(animatedMeshManifest === null || resolveAnimatedMeshResource === null
            ? {}
            : { animatedMeshManifest, resolveAnimatedMeshResource }),
          ...(meshResourceManifest === null || resolveMeshResource === null
            ? {}
            : { meshResourceManifest, resolveMeshResource }),
        },
      );
      if (this.#destroyed || mountRevision !== this.#mountRevision) {
        surface.dispose();
        return;
      }
      this.#surface = surface;
      this.status.set('ready');
      const frame = this.frame();
      if (frame !== null) {
        this.#replaceFrame(
          frame,
          this.framePatch(),
          this.frameGeneration(),
          this.selectedEntityId(),
          this.previewEntityId(),
          this.previewTransform(),
          this.voxelPreview(),
          this.voxelObjectPlacementResourceFrame(),
          this.lightingMode(),
        );
      }
      this.#replaceManipulatorOverlay(
        this.manipulatorTransform(),
        this.transformTool(),
        this.transformOrientation(),
        this.activeTransformHandleLabel(),
        this.hoveredTransformHandleLabel(),
      );
      this.#syncReadout();
    } catch (error) {
      if (mountRevision !== this.#mountRevision) return;
      this.#fail(error instanceof Error ? error.message : 'shared renderer failed to mount');
    }
  }

  #replaceFrame(
    frame: RenderFrameDiff,
    framePatch: RenderFrameDiff | null,
    generation: number,
    selectedEntityId: number | null,
    previewEntityId: number | null,
    previewTransform: Transform | null,
    voxelPreview: StudioVoxelPreview | null,
    voxelObjectPlacementResourceFrame: RenderFrameDiff | null,
    lightingMode: StudioLightingMode,
  ): void {
    const surface = this.#surface;
    const presentationKey = JSON.stringify([
      generation,
      selectedEntityId,
      previewEntityId,
      previewTransform,
      voxelPreview,
      voxelObjectPlacementResourceFrame,
      lightingMode,
    ]);
    if (surface === null) return;
    const generationChanged = generation !== this.#lastAppliedFrameGeneration;
    if (generationChanged && framePatch !== null && this.#authoredFrameReady) {
      const receipt = surface.applyAuthoredFrame(framePatch);
      if (!receipt.applied) {
        this.#fail(receipt.diagnostics.map((entry) => entry.message).join('; '));
        return;
      }
      this.#lastAppliedFrameGeneration = generation;
      this.#lastPresentationKey = presentationKey;
      this.#syncReadout();
      this.frameApplied.emit(generation);
      return;
    }
    if (this.#authoredFrameReady && presentationKey === this.#lastPresentationKey) return;
    const presentation = presentStudioSelection(
      frame,
      selectedEntityId,
      previewEntityId,
      previewTransform,
      voxelPreview,
      voxelObjectPlacementResourceFrame,
    );
    const lighting = presentStudioLighting(presentation.frame, lightingMode);
    const receipt = surface.replaceFrame(lighting.frame);
    if (!receipt.applied) {
      this.#fail(receipt.diagnostics.map((entry) => entry.message).join('; '));
      return;
    }
    this.#lastPresentationKey = presentationKey;
    this.#lastAppliedFrameGeneration = generation;
    this.#authoredFrameReady = true;
    this.previewApplied.set(presentation.previewApplied);
    this.voxelPreviewKind.set(presentation.voxelPreviewKind);
    this.selectedRenderHandle.set(presentation.selectedHandle);
    this.workLightActive.set(lighting.workLightActive);
    this.voxelObjectDefinitionCount.set(lighting.frame.ops.filter(
      (operation) => operation.op === 'defineVoxelObject',
    ).length);
    this.voxelObjectInstanceCount.set(lighting.frame.ops.filter(
      (operation) => operation.op === 'createVoxelObjectInstance'
        && operation.instance.metadata.sourceEntity !== null,
    ).length);
    this.voxelObjectPlacementGhostCount.set(lighting.frame.ops.filter(
      (operation) => operation.op === 'createVoxelObjectInstance'
        && operation.instance.metadata.tags.includes('voxel-object-placement-ghost'),
    ).length);
    this.#syncReadout();
    if (generationChanged) this.frameApplied.emit(generation);
  }

  #beginManipulatorDrag(event: PointerEvent): boolean {
    const surface = this.#surface;
    const transform = this.manipulatorTransform();
    const tool = this.transformTool();
    if (surface === null || transform === null || tool === null) return false;
    const pointer = this.#canvasPoint(event);
    const receipt = surface.pick({
      point: pointer,
      filter: {
        channels: ['overlay'],
        layers: ['debug'],
        tags: ['studio-transform-manipulator'],
      },
    });
    if (receipt.diagnostics.length > 0) {
      this.#report(receipt.diagnostics.map((entry) => entry.message).join('; '));
      return false;
    }
    const handle = receipt.hint === null
      ? null
      : studioTransformHandleFromId(receipt.hint.handle);
    if (handle === null || handle.tool !== tool) return false;
    try {
      const drag = beginStudioTransformManipulatorDrag({
        camera: this.#manipulatorCamera(),
        handle,
        orientation: this.transformOrientation(),
        pointer,
        revision: `${String(this.frameGeneration())}:${String(
          this.previewEntityId() ?? this.selectedEntityId(),
        )}`,
        snapping: this.transformSnapping(),
        source: transform,
      });
      event.preventDefault();
      event.stopImmediatePropagation();
      this.canvasElement.nativeElement.focus({ preventScroll: true });
      this.canvasElement.nativeElement.setPointerCapture(event.pointerId);
      this.#activeManipulatorDrag = { pointerId: event.pointerId, drag };
      this.#activeManipulatorHandle = handle;
      this.activeTransformHandleLabel.set(handleLabel(handle));
      this.transformDragStarted.emit();
      this.#pointerStart = [event.clientX, event.clientY];
      this.#pointerDragged = true;
      return true;
    } catch (error) {
      this.#report(error instanceof Error ? error.message : 'transform drag could not begin');
      return false;
    }
  }

  #emitManipulatorCandidate(
    drag: StudioTransformManipulatorDrag,
    event: PointerEvent,
  ): void {
    try {
      const candidate = updateStudioTransformManipulatorDrag(
        drag,
        this.#manipulatorCamera(),
        this.#canvasPoint(event),
        {
          fine: event.shiftKey,
          snapping: event.ctrlKey || event.metaKey
            ? !drag.snapping.enabled
            : drag.snapping.enabled,
        },
      );
      this.transformCandidate.emit(candidate.transform);
    } catch (error) {
      this.#report(error instanceof Error ? error.message : 'transform drag could not update');
    }
  }

  #finishManipulatorDrag(pointerId: number, cancelled: boolean): void {
    this.#activeManipulatorDrag = null;
    this.#activeManipulatorHandle = null;
    this.activeTransformHandleLabel.set(null);
    this.#pointerStart = null;
    this.#pointerDragged = true;
    this.#suppressNextClick = true;
    if (this.#suppressClickTimer !== null) clearTimeout(this.#suppressClickTimer);
    this.#suppressClickTimer = setTimeout(() => {
      this.#suppressNextClick = false;
      this.#suppressClickTimer = null;
    }, 0);
    try {
      if (this.canvasElement.nativeElement.hasPointerCapture(pointerId)) {
        this.canvasElement.nativeElement.releasePointerCapture(pointerId);
      }
    } catch {
      // Pointer capture may already be gone after cancellation or host teardown.
    }
    this.transformDragFinished.emit({ cancelled });
  }

  #updateManipulatorHover(event: PointerEvent): void {
    const surface = this.#surface;
    if (surface === null || this.manipulatorTransform() === null || this.transformTool() === null) {
      this.#setHoveredManipulator(null);
      return;
    }
    const receipt = surface.pick({
      point: this.#canvasPoint(event),
      filter: {
        channels: ['overlay'],
        layers: ['debug'],
        tags: ['studio-transform-manipulator'],
      },
    });
    const handle = receipt.diagnostics.length === 0 && receipt.hint !== null
      ? studioTransformHandleFromId(receipt.hint.handle)
      : null;
    this.#setHoveredManipulator(handle);
  }

  #setHoveredManipulator(handle: StudioTransformHandle | null): void {
    const label = handle === null ? null : handleLabel(handle);
    if (label === this.hoveredTransformHandleLabel()) return;
    this.#hoveredManipulatorHandle = handle;
    this.hoveredTransformHandleLabel.set(label);
  }

  #replaceManipulatorOverlay(
    transform: Transform | null,
    tool: StudioTransformTool | null,
    orientation: StudioTransformOrientation,
    activeLabel: string | null,
    hoveredLabel: string | null,
  ): void {
    const surface = this.#surface;
    if (surface === null) return;
    const key = JSON.stringify([transform, tool, orientation, activeLabel, hoveredLabel]);
    if (key === this.#lastManipulatorKey) return;
    const receipt = transform === null || tool === null
      ? surface.clearOverlayProjection()
      : surface.replaceOverlayFrame(projectStudioTransformManipulator({
          active: this.#activeManipulatorHandle,
          hovered: this.#hoveredManipulatorHandle,
          orientation,
          tool,
          transform,
          visible: true,
        }));
    if (!receipt.applied) {
      this.#fail(receipt.diagnostics.map((entry) => entry.message).join('; '));
      return;
    }
    this.#lastManipulatorKey = key;
  }

  #canvasPoint(event: Pick<PointerEvent, 'clientX' | 'clientY'>): readonly [number, number] {
    return canvasPoint(
      [event.clientX, event.clientY],
      this.canvasElement.nativeElement.getBoundingClientRect(),
    );
  }

  #manipulatorCamera(): StudioTransformManipulatorCamera {
    const surface = this.#surface;
    if (surface === null) throw new Error('shared renderer surface is unavailable');
    const camera = surface.camera();
    const bounds = this.canvasElement.nativeElement.getBoundingClientRect();
    return {
      position: camera.pose.position,
      basis: camera.basis,
      fovYDegrees: camera.projection.fovYDegrees,
      viewport: {
        width: Math.max(1, bounds.width),
        height: Math.max(1, bounds.height),
      },
    };
  }

  #setGrid(descriptor: EditorGridDescriptor | null): void {
    const surface = this.#surface;
    if (surface === null) return;
    const receipt = surface.setGrid(descriptor);
    if (!receipt.applied) {
      this.#fail(receipt.diagnostics.map((entry) => entry.message).join('; '));
      return;
    }
    this.#syncReadout();
  }

  #configureControls(preferences: RendererInspectionSurfaceControlPreferences): void {
    const surface = this.#surface;
    if (surface === null) return;
    try {
      surface.configureControlPreferences(preferences);
      this.#syncReadout();
    } catch (error) {
      this.#fail(error instanceof Error ? error.message : 'camera preferences were rejected');
    }
  }

  #syncReadout(): void {
    const readout = this.#surface?.readout();
    if (readout === undefined) return;
    this.retainedOpCount.set(readout.retainedOpCount);
    this.retainedFrameHash.set(readout.retainedFrameHash);
    this.cameraRevision.set(readout.cameraRevision);
  }

  #fail(message: string): void {
    this.status.set('error');
    this.#report(message);
  }

  #report(message: string): void {
    this.lastRendererError.set(message);
    this.rendererError.emit(`Shared renderer: ${message}`);
  }
}

function normalize(
  vector: readonly [number, number, number],
): readonly [number, number, number] | null {
  const length = Math.hypot(...vector);
  if (!Number.isFinite(length) || length <= Number.EPSILON) return null;
  return [vector[0] / length, vector[1] / length, vector[2] / length];
}

function handleLabel(handle: StudioTransformHandle): string {
  const target = handle.kind === 'axis'
    ? ['x', 'y', 'z'][handle.axis]
    : handle.kind === 'plane'
      ? handle.plane
      : 'uniform';
  return `${handle.tool}:${String(target)}`;
}
