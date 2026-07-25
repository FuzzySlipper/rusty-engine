import {
  decodeStudioAdapterResponse,
  STUDIO_ADAPTER_PROTOCOL_VERSION,
  type AdapterRejection,
  type DescribedResponse,
  type EntityTranslationAppliedResponse,
  type ProjectClosedResponse,
  type ProjectOpenedResponse,
  type ProjectReadResponse,
  type StudioAdapterRequest,
  type StudioAdapterResponse,
} from './protocol.js';

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
