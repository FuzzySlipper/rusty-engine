import {
  decodeStudioAdapterResponse,
  STUDIO_ADAPTER_PROTOCOL_VERSION,
  type AdapterRejection,
  type DescribedResponse,
  type EntityTranslationAppliedResponse,
  type ProjectMutationAppliedResponse,
  type ProjectClosedResponse,
  type ProjectOpenedResponse,
  type ProjectReadResponse,
  type StudioAdapterRequest,
  type StudioAdapterResponse,
  type VoxelConversionDiscardedResponse,
  type VoxelConversionPreparedResponse,
  type VoxelPickValidatedResponse,
  type VoxelReadResponse,
} from './protocol.js';

type RequestInput<Type extends StudioAdapterRequest['type']> = Omit<
  Extract<StudioAdapterRequest, { readonly type: Type }>,
  'type' | 'protocolVersion' | 'requestId'
>;

export interface StudioAdapterTransport {
  exchange(request: StudioAdapterRequest): Promise<unknown>;
}

export interface SetEntityTranslationInput {
  readonly expectedProjectHash: string;
  readonly expectedSceneRevision: number;
  readonly entityId: number;
  readonly translation: readonly [number, number, number];
}

export class StudioAdapterOperationRejected extends Error {
  readonly rejection: AdapterRejection;

  constructor(rejection: AdapterRejection) {
    super(`${rejection.code}: ${rejection.message}`);
    this.name = 'StudioAdapterOperationRejected';
    this.rejection = rejection;
  }
}

export class StudioAdapterClient {
  readonly #transport: StudioAdapterTransport;
  #nextRequestId = 1;

  constructor(transport: StudioAdapterTransport) {
    this.#transport = transport;
  }

  describe(): Promise<DescribedResponse> {
    return this.#exchange(
      {
        type: 'describe',
        protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
        requestId: this.#requestId('describe'),
      },
      'described',
    );
  }

  openProject(root: string, projectFile: string): Promise<ProjectOpenedResponse> {
    return this.#exchange(
      {
        type: 'openProject',
        protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
        requestId: this.#requestId('open'),
        root,
        projectFile,
      },
      'projectOpened',
    );
  }

  readProject(): Promise<ProjectReadResponse> {
    return this.#exchange(
      {
        type: 'readProject',
        protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
        requestId: this.#requestId('read'),
      },
      'projectRead',
    );
  }

  setEntityTranslation(
    input: SetEntityTranslationInput,
  ): Promise<EntityTranslationAppliedResponse> {
    return this.#exchange(
      {
        type: 'setEntityTranslation',
        protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
        requestId: this.#requestId('translate'),
        expectedProjectHash: input.expectedProjectHash,
        expectedSceneRevision: input.expectedSceneRevision,
        entityId: input.entityId,
        translation: input.translation,
      },
      'entityTranslationApplied',
    );
  }

  upsertMaterial(input: RequestInput<'upsertMaterial'>): Promise<ProjectMutationAppliedResponse> {
    return this.#mutation('upsertMaterial', input);
  }

  initializeVoxelAsset(
    input: RequestInput<'initializeVoxelAsset'>,
  ): Promise<ProjectMutationAppliedResponse> {
    return this.#mutation('initializeVoxelAsset', input);
  }

  duplicateVoxelAsset(
    input: RequestInput<'duplicateVoxelAsset'>,
  ): Promise<ProjectMutationAppliedResponse> {
    return this.#mutation('duplicateVoxelAsset', input);
  }

  attachVoxelInstance(
    input: RequestInput<'attachVoxelInstance'>,
  ): Promise<ProjectMutationAppliedResponse> {
    return this.#mutation('attachVoxelInstance', input);
  }

  setVoxelInstanceTransform(
    input: RequestInput<'setVoxelInstanceTransform'>,
  ): Promise<ProjectMutationAppliedResponse> {
    return this.#mutation('setVoxelInstanceTransform', input);
  }

  removeVoxelInstance(
    input: RequestInput<'removeVoxelInstance'>,
  ): Promise<ProjectMutationAppliedResponse> {
    return this.#mutation('removeVoxelInstance', input);
  }

  replaceVoxelPalette(
    input: RequestInput<'replaceVoxelPalette'>,
  ): Promise<ProjectMutationAppliedResponse> {
    return this.#mutation('replaceVoxelPalette', input);
  }

  validateVoxelPick(
    input: RequestInput<'validateVoxelPick'>,
  ): Promise<VoxelPickValidatedResponse> {
    return this.#exchange(this.#request('validateVoxelPick', input), 'voxelPickValidated');
  }

  applyVoxelBrush(
    input: RequestInput<'applyVoxelBrush'>,
  ): Promise<ProjectMutationAppliedResponse> {
    return this.#mutation('applyVoxelBrush', input);
  }

  undoVoxelEdit(input: RequestInput<'undoVoxelEdit'>): Promise<ProjectMutationAppliedResponse> {
    return this.#mutation('undoVoxelEdit', input);
  }

  redoVoxelEdit(input: RequestInput<'redoVoxelEdit'>): Promise<ProjectMutationAppliedResponse> {
    return this.#mutation('redoVoxelEdit', input);
  }

  revertVoxelHistory(
    input: RequestInput<'revertVoxelHistory'>,
  ): Promise<ProjectMutationAppliedResponse> {
    return this.#mutation('revertVoxelHistory', input);
  }

  createVoxelAnnotationLayer(
    input: RequestInput<'createVoxelAnnotationLayer'>,
  ): Promise<ProjectMutationAppliedResponse> {
    return this.#mutation('createVoxelAnnotationLayer', input);
  }

  editVoxelAnnotation(
    input: RequestInput<'editVoxelAnnotation'>,
  ): Promise<ProjectMutationAppliedResponse> {
    return this.#mutation('editVoxelAnnotation', input);
  }

  queryVoxelAnnotation(
    input: RequestInput<'queryVoxelAnnotation'>,
  ): Promise<VoxelReadResponse> {
    return this.#exchange(this.#request('queryVoxelAnnotation', input), 'voxelRead');
  }

  exportVoxelAnnotation(
    input: RequestInput<'exportVoxelAnnotation'>,
  ): Promise<VoxelReadResponse> {
    return this.#exchange(this.#request('exportVoxelAnnotation', input), 'voxelRead');
  }

  queryVoxelModel(input: RequestInput<'queryVoxelModel'>): Promise<VoxelReadResponse> {
    return this.#exchange(this.#request('queryVoxelModel', input), 'voxelRead');
  }

  prepareVoxelConversion(
    input: RequestInput<'prepareVoxelConversion'>,
  ): Promise<VoxelConversionPreparedResponse> {
    return this.#exchange(
      this.#request('prepareVoxelConversion', input),
      'voxelConversionPrepared',
    );
  }

  applyVoxelConversion(
    input: RequestInput<'applyVoxelConversion'>,
  ): Promise<ProjectMutationAppliedResponse> {
    return this.#mutation('applyVoxelConversion', input);
  }

  discardVoxelConversion(
    input: RequestInput<'discardVoxelConversion'>,
  ): Promise<VoxelConversionDiscardedResponse> {
    return this.#exchange(
      this.#request('discardVoxelConversion', input),
      'voxelConversionDiscarded',
    );
  }

  closeProject(): Promise<ProjectClosedResponse> {
    return this.#exchange(
      {
        type: 'closeProject',
        protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
        requestId: this.#requestId('close'),
      },
      'projectClosed',
    );
  }

  #mutation<Type extends MutationRequestType>(
    type: Type,
    input: RequestInput<Type>,
  ): Promise<ProjectMutationAppliedResponse> {
    return this.#exchange(this.#request(type, input), 'projectMutationApplied');
  }

  #request<Type extends StudioAdapterRequest['type']>(
    type: Type,
    input: RequestInput<Type>,
  ): Extract<StudioAdapterRequest, { readonly type: Type }> {
    return {
      ...input,
      type,
      protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
      requestId: this.#requestId(type),
    } as Extract<StudioAdapterRequest, { readonly type: Type }>;
  }

  async #exchange<Type extends Exclude<StudioAdapterResponse['type'], 'rejected'>>(
    request: StudioAdapterRequest,
    expectedType: Type,
  ): Promise<Extract<StudioAdapterResponse, { readonly type: Type }>> {
    const response = decodeStudioAdapterResponse(await this.#transport.exchange(request));
    if (response.type === 'rejected') {
      throw new StudioAdapterOperationRejected(response.error);
    }
    if (response.requestId !== request.requestId) {
      throw new Error(
        `Studio adapter response requestId ${JSON.stringify(response.requestId)} did not match ${JSON.stringify(request.requestId)}`,
      );
    }
    if (response.type !== expectedType) {
      throw new Error(
        `Studio adapter returned ${response.type} for ${request.type}; expected ${expectedType}`,
      );
    }
    return response as Extract<StudioAdapterResponse, { readonly type: Type }>;
  }

  #requestId(operation: string): string {
    const id = this.#nextRequestId;
    this.#nextRequestId += 1;
    return `studio-${operation}-${String(id)}`;
  }
}

type MutationRequestType =
  | 'upsertMaterial'
  | 'initializeVoxelAsset'
  | 'duplicateVoxelAsset'
  | 'attachVoxelInstance'
  | 'setVoxelInstanceTransform'
  | 'removeVoxelInstance'
  | 'replaceVoxelPalette'
  | 'applyVoxelBrush'
  | 'undoVoxelEdit'
  | 'redoVoxelEdit'
  | 'revertVoxelHistory'
  | 'createVoxelAnnotationLayer'
  | 'editVoxelAnnotation'
  | 'applyVoxelConversion';
