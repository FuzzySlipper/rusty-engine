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
import type { EditorGridDescriptor, RenderFrameDiff } from '@rusty-engine/render-contracts';
import {
  mountRendererInspectionSurface,
  type RendererInspectionSurface,
  type RendererInspectionSurfaceControlPreferences,
} from '@rusty-engine/renderer-host';

import {
  STUDIO_EDITOR_GRID,
  canvasPoint,
  movedPastPickThreshold,
  presentStudioSelection,
  type StudioVoxelPreview,
} from './viewport-model.js';

type ViewportStatus = 'mounting' | 'ready' | 'error' | 'disposed';

export interface VoxelViewportPickCandidate {
  readonly instanceId: string;
  readonly cameraOrigin: readonly [number, number, number];
  readonly direction: readonly [number, number, number];
  readonly worldPoint: readonly [number, number, number];
  readonly worldNormal: readonly [number, number, number];
  readonly maxDistance: number;
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
    '[attr.data-selected-render-handle]': 'selectedRenderHandle()',
    '[attr.data-camera-move-speed]': 'controlPreferences().moveSpeed',
    '[attr.data-camera-move-forward]': 'controlPreferences().keyboard.moveForward',
  },
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class StudioViewportComponent implements AfterViewInit, OnDestroy {
  readonly frame = input<RenderFrameDiff | null>(null);
  readonly frameGeneration = input(0);
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
  readonly previewTranslation = input<readonly [number, number, number] | null>(null);
  readonly voxelPreview = input<StudioVoxelPreview | null>(null);

  readonly entityPicked = output<number | null>();
  readonly voxelPicked = output<VoxelViewportPickCandidate>();
  readonly rendererError = output<string>();

  readonly status = signal<ViewportStatus>('mounting');
  readonly retainedOpCount = signal(0);
  readonly cameraRevision = signal(0);
  readonly pickRevision = signal(0);
  readonly retainedFrameHash = signal('');
  readonly previewApplied = signal(false);
  readonly voxelPreviewKind = signal<StudioVoxelPreview['kind'] | null>(null);
  readonly selectedRenderHandle = signal<number | null>(null);

  @ViewChild('canvas', { static: true })
  private canvasElement!: ElementRef<HTMLCanvasElement>;

  #surface: RendererInspectionSurface | null = null;
  #destroyed = false;
  #lastPresentationKey = '';
  #pointerStart: readonly [number, number] | null = null;
  #pointerDragged = false;

  constructor() {
    effect(() => {
      const generation = this.frameGeneration();
      const frame = this.frame();
      const selectedEntityId = this.selectedEntityId();
      const previewEntityId = this.previewEntityId();
      const previewTranslation = this.previewTranslation();
      const voxelPreview = this.voxelPreview();
      if (frame !== null) {
        this.#replaceFrame(
          frame,
          generation,
          selectedEntityId,
          previewEntityId,
          previewTranslation,
          voxelPreview,
        );
      }
    });
    effect(() => {
      this.#setGrid(this.grid());
    });
    effect(() => {
      const preferences = this.controlPreferences();
      this.#configureControls(preferences);
    });
  }

  ngAfterViewInit(): void {
    void this.#mount();
  }

  ngOnDestroy(): void {
    this.#destroyed = true;
    this.#surface?.dispose();
    this.#surface = null;
    this.status.set('disposed');
  }

  pointerDown(event: PointerEvent): void {
    if (event.button !== 0) return;
    this.#pointerStart = [event.clientX, event.clientY];
    this.#pointerDragged = false;
  }

  pointerMove(event: PointerEvent): void {
    if (this.#pointerStart === null || this.#pointerDragged) return;
    this.#pointerDragged = movedPastPickThreshold(
      this.#pointerStart,
      [event.clientX, event.clientY],
    );
  }

  pointerUp(): void {
    this.#pointerStart = null;
  }

  pick(event: MouseEvent): void {
    if (this.#pointerDragged) {
      this.#pointerDragged = false;
      return;
    }
    const surface = this.#surface;
    if (surface === null) return;
    const receipt = surface.pick({
      point: canvasPoint(
        [event.clientX, event.clientY],
        this.canvasElement.nativeElement.getBoundingClientRect(),
      ),
      filter: { channels: ['authored'] },
    });
    if (receipt.diagnostics.length > 0) {
      this.#report(receipt.diagnostics.map((entry) => entry.message).join('; '));
      return;
    }
    this.pickRevision.update((revision) => revision + 1);
    const hint = receipt.hint;
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
    try {
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
        },
      );
      if (this.#destroyed) {
        surface.dispose();
        return;
      }
      this.#surface = surface;
      this.status.set('ready');
      const frame = this.frame();
      if (frame !== null) {
        this.#replaceFrame(
          frame,
          this.frameGeneration(),
          this.selectedEntityId(),
          this.previewEntityId(),
          this.previewTranslation(),
          this.voxelPreview(),
        );
      }
      this.#syncReadout();
    } catch (error) {
      this.#fail(error instanceof Error ? error.message : 'shared renderer failed to mount');
    }
  }

  #replaceFrame(
    frame: RenderFrameDiff,
    generation: number,
    selectedEntityId: number | null,
    previewEntityId: number | null,
    previewTranslation: readonly [number, number, number] | null,
    voxelPreview: StudioVoxelPreview | null,
  ): void {
    const surface = this.#surface;
    const presentationKey = JSON.stringify([
      generation,
      selectedEntityId,
      previewEntityId,
      previewTranslation,
      voxelPreview,
    ]);
    if (surface === null || presentationKey === this.#lastPresentationKey) return;
    const presentation = presentStudioSelection(
      frame,
      selectedEntityId,
      previewEntityId,
      previewTranslation,
      voxelPreview,
    );
    const receipt = surface.replaceFrame(presentation.frame);
    if (!receipt.applied) {
      this.#fail(receipt.diagnostics.map((entry) => entry.message).join('; '));
      return;
    }
    this.#lastPresentationKey = presentationKey;
    this.previewApplied.set(presentation.previewApplied);
    this.voxelPreviewKind.set(presentation.voxelPreviewKind);
    this.selectedRenderHandle.set(presentation.selectedHandle);
    this.#syncReadout();
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
