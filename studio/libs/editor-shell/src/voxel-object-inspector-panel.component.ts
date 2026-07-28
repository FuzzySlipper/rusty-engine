import {
  ChangeDetectionStrategy,
  Component,
  InjectionToken,
  computed,
  inject,
  input,
} from '@angular/core';
import {
  VOXEL_OBJECT_COMPONENT_TYPE_ID,
  VOXEL_OBJECT_INSPECTOR_CONTRACT_ID,
  VOXEL_OBJECT_INSPECTOR_CONTRACT_VERSION,
  type VoxelObjectAssetAuthoringReadout,
  type VoxelObjectInstancePlaybackReadout,
  type VoxelObjectInstanceReadout,
} from '@rusty-engine/studio-adapter-client';
import {
  VoxelObjectPlaybackComponent,
  type VoxelEditorAction,
} from '@rusty-engine/studio-voxel-editor';

import {
  admitStudioEntityInspectorContributions,
  type StudioEntityInspectorContext,
  type StudioEntityInspectorMutationPort,
  type StudioEntityInspectorPanel,
} from './entity-inspector.js';

export interface StudioVoxelObjectInspectorReadout {
  readonly instance: VoxelObjectInstanceReadout | null;
  readonly asset: VoxelObjectAssetAuthoringReadout | null;
  readonly playback: VoxelObjectInstancePlaybackReadout | null;
  readonly busy: boolean;
}

export interface StudioVoxelObjectInspectorHost {
  read(ownerEntityId: number): StudioVoxelObjectInspectorReadout;
  run(action: VoxelEditorAction): void;
}

export const STUDIO_VOXEL_OBJECT_INSPECTOR_HOST =
  new InjectionToken<StudioVoxelObjectInspectorHost>('StudioVoxelObjectInspectorHost');

@Component({
  selector: 'rusty-studio-voxel-object-inspector-panel',
  standalone: true,
  imports: [VoxelObjectPlaybackComponent],
  template: `
    <rusty-voxel-object-playback
      [instance]="readout().instance"
      [asset]="readout().asset"
      [playback]="readout().playback"
      [busy]="readout().busy"
      (action)="host.run($event)"
    />
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class StudioVoxelObjectInspectorPanelComponent
implements StudioEntityInspectorPanel {
  readonly context = input.required<StudioEntityInspectorContext>();
  readonly mutationPort = input.required<StudioEntityInspectorMutationPort>();
  readonly host = inject(STUDIO_VOXEL_OBJECT_INSPECTOR_HOST);
  readonly readout = computed(() => this.host.read(this.context().ownerEntityId));
}

export const RUSTY_ENGINE_ENTITY_INSPECTOR_CONTRIBUTIONS =
  admitStudioEntityInspectorContributions([{
    componentTypeId: VOXEL_OBJECT_COMPONENT_TYPE_ID,
    contract: {
      contractId: VOXEL_OBJECT_INSPECTOR_CONTRACT_ID,
      contractVersion: VOXEL_OBJECT_INSPECTOR_CONTRACT_VERSION,
    },
    title: 'Voxel Object',
    order: 100,
    panel: StudioVoxelObjectInspectorPanelComponent,
    dataVisualId: 'entity-voxel-object-component',
  }]);
