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
import type { RenderFrameDiff } from '@rusty-engine/render-contracts';
import {
  mountRendererInspectionSurface,
  type RendererInspectionSurface,
} from '@rusty-engine/renderer-host';

import {
  STUDIO_EDITOR_GRID,
  canvasPoint,
  movedPastPickThreshold,
} from './viewport-model.js';

type ViewportStatus = 'mounting' | 'ready' | 'error' | 'disposed';

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
  },
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class StudioViewportComponent implements AfterViewInit, OnDestroy {
  readonly frame = input<RenderFrameDiff | null>(null);
  readonly frameGeneration = input(0);
  readonly gridVisible = input(true);
  readonly selectedEntityId = input<number | null>(null);
  readonly previewEntityId = input<number | null>(null);
  readonly previewTranslation = input<readonly [number, number, number] | null>(null);

  readonly entityPicked = output<number | null>();
  readonly rendererError = output<string>();

  readonly status = signal<ViewportStatus>('mounting');
  readonly retainedOpCount = signal(0);
  readonly cameraRevision = signal(0);
  readonly pickRevision = signal(0);

  @ViewChild('canvas', { static: true })
  private canvasElement!: ElementRef<HTMLCanvasElement>;

  #surface: RendererInspectionSurface | null = null;
  #destroyed = false;
  #lastAppliedFrameGeneration = -1;
  #pointerStart: readonly [number, number] | null = null;
  #pointerDragged = false;

  constructor() {
    effect(() => {
      const generation = this.frameGeneration();
      const frame = this.frame();
      if (frame !== null) this.#replaceFrame(frame, generation);
    });
    effect(() => {
      const visible = this.gridVisible();
      this.#setGrid(visible);
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
    this.entityPicked.emit(receipt.hint?.sourceTrace?.entity ?? null);
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
            moveSpeed: 8,
          },
          initialGrid: this.gridVisible() ? STUDIO_EDITOR_GRID : null,
        },
      );
      if (this.#destroyed) {
        surface.dispose();
        return;
      }
      this.#surface = surface;
      this.status.set('ready');
      const frame = this.frame();
      if (frame !== null) this.#replaceFrame(frame, this.frameGeneration());
      this.#syncReadout();
    } catch (error) {
      this.#fail(error instanceof Error ? error.message : 'shared renderer failed to mount');
    }
  }

  #replaceFrame(frame: RenderFrameDiff, generation: number): void {
    const surface = this.#surface;
    if (surface === null || generation === this.#lastAppliedFrameGeneration) return;
    const receipt = surface.replaceFrame(frame);
    if (!receipt.applied) {
      this.#fail(receipt.diagnostics.map((entry) => entry.message).join('; '));
      return;
    }
    this.#lastAppliedFrameGeneration = generation;
    this.#syncReadout();
  }

  #setGrid(visible: boolean): void {
    const surface = this.#surface;
    if (surface === null) return;
    const receipt = surface.setGrid(visible ? STUDIO_EDITOR_GRID : null);
    if (!receipt.applied) {
      this.#fail(receipt.diagnostics.map((entry) => entry.message).join('; '));
      return;
    }
    this.#syncReadout();
  }

  #syncReadout(): void {
    const readout = this.#surface?.readout();
    if (readout === undefined) return;
    this.retainedOpCount.set(readout.retainedOpCount);
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
