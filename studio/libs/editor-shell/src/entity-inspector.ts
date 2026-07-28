import type { InputSignal, Type } from '@angular/core';
import {
  MAX_STUDIO_ENTITY_INSPECTOR_IDENTITY_BYTES,
  type AdapterDescription,
  type StudioEntityComponentReference,
  type StudioEntityInspectorContractIdentity,
  type StudioProjectIdentity,
} from '@rusty-engine/studio-adapter-client';

export type StudioEntityInspectorContributionErrorCode =
  | 'inspectorContribution.invalid'
  | 'inspectorContribution.duplicate';

export class StudioEntityInspectorContributionError extends Error {
  readonly code: StudioEntityInspectorContributionErrorCode;

  constructor(code: StudioEntityInspectorContributionErrorCode, message: string) {
    super(message);
    this.name = 'StudioEntityInspectorContributionError';
    this.code = code;
  }
}

export type StudioEntityInspectorMutationErrorCode =
  | 'inspectorMutation.busy'
  | 'inspectorMutation.stale'
  | 'inspectorMutation.hashMismatch'
  | 'inspectorMutation.closed';

export class StudioEntityInspectorMutationError extends Error {
  readonly code: StudioEntityInspectorMutationErrorCode;

  constructor(code: StudioEntityInspectorMutationErrorCode, message: string) {
    super(message);
    this.name = 'StudioEntityInspectorMutationError';
    this.code = code;
  }
}

export interface StudioEntityInspectorContext {
  readonly ownerEntityId: number;
  readonly componentTypeId: string;
  readonly inspectorContract: StudioEntityInspectorContractIdentity;
  readonly project: StudioProjectIdentity;
  readonly projectGeneration: number;
  readonly selectionGeneration: number;
  readonly contractGeneration: number;
  readonly adapterId: string;
  readonly busy: boolean;
}

export interface StudioEntityInspectorMutationReceipt {
  readonly beforeProjectHash: string;
  readonly afterProjectHash: string;
}

export type StudioEntityInspectorMutationSettlement =
  | { readonly kind: 'accepted'; readonly projectHash: string }
  | { readonly kind: 'rejected'; readonly message: string }
  | { readonly kind: 'stale' };

export interface StudioEntityInspectorMutationLease {
  readonly context: StudioEntityInspectorContext;
  settle(
    receipt: StudioEntityInspectorMutationReceipt,
  ): Promise<StudioEntityInspectorMutationSettlement>;
  reject(error?: unknown): StudioEntityInspectorMutationSettlement;
}

export interface StudioEntityInspectorMutationPort {
  acquire(context: StudioEntityInspectorContext): StudioEntityInspectorMutationLease;
}

export interface StudioEntityInspectorPanel {
  readonly context: InputSignal<StudioEntityInspectorContext>;
  readonly mutationPort: InputSignal<StudioEntityInspectorMutationPort>;
}

export interface StudioEntityInspectorContribution {
  readonly componentTypeId: string;
  readonly contract: StudioEntityInspectorContractIdentity;
  readonly title: string;
  readonly order: number;
  readonly panel: Type<StudioEntityInspectorPanel>;
  readonly dataVisualId?: string;
}

export interface AdmittedStudioEntityInspectorContribution
  extends StudioEntityInspectorContribution {
  readonly key: string;
}

export interface StudioEntityInspectorMatch {
  readonly contribution: AdmittedStudioEntityInspectorContribution;
  readonly reference: StudioEntityComponentReference;
}

export interface StudioEntityInspectorRenderMatch extends StudioEntityInspectorMatch {
  readonly context: StudioEntityInspectorContext;
  readonly instanceKey: string;
}

export function admitStudioEntityInspectorContributions(
  contributions: readonly StudioEntityInspectorContribution[],
): readonly AdmittedStudioEntityInspectorContribution[] {
  const admitted = contributions.map((contribution, index) => {
    validateIdentity(
      contribution.componentTypeId,
      `contributions[${String(index)}].componentTypeId`,
    );
    validateIdentity(
      contribution.contract.contractId,
      `contributions[${String(index)}].contract.contractId`,
    );
    if (
      !Number.isSafeInteger(contribution.contract.contractVersion)
      || contribution.contract.contractVersion <= 0
    ) {
      invalid(
        `contributions[${String(index)}].contract.contractVersion must be a positive safe integer`,
      );
    }
    if (!Number.isSafeInteger(contribution.order)) {
      invalid(`contributions[${String(index)}].order must be a safe integer`);
    }
    if (
      contribution.title.trim().length === 0
      || contribution.title.length > MAX_STUDIO_ENTITY_INSPECTOR_IDENTITY_BYTES
    ) {
      invalid(
        `contributions[${String(index)}].title must contain 1..=${String(MAX_STUDIO_ENTITY_INSPECTOR_IDENTITY_BYTES)} characters`,
      );
    }
    if (typeof contribution.panel !== 'function') {
      invalid(`contributions[${String(index)}].panel must be an Angular component type`);
    }
    const key = contributionKey(contribution);
    return Object.freeze({
      ...contribution,
      contract: Object.freeze({ ...contribution.contract }),
      key,
    });
  });

  const seen = new Set<string>();
  for (const contribution of admitted) {
    if (seen.has(contribution.key)) {
      throw new StudioEntityInspectorContributionError(
        'inspectorContribution.duplicate',
        `duplicate Entity inspector contribution ${contribution.key}`,
      );
    }
    seen.add(contribution.key);
  }

  admitted.sort((left, right) =>
    left.order - right.order
    || compareText(left.title, right.title)
    || compareText(left.componentTypeId, right.componentTypeId)
    || compareText(left.contract.contractId, right.contract.contractId)
    || left.contract.contractVersion - right.contract.contractVersion);
  return Object.freeze(admitted);
}

export function matchStudioEntityInspectorContributions(
  contributions: readonly AdmittedStudioEntityInspectorContribution[],
  references: readonly StudioEntityComponentReference[],
  adapter: AdapterDescription,
  ownerEntityId: number,
): readonly StudioEntityInspectorMatch[] {
  const advertised = new Set(
    adapter.entityInspectorContracts.map(contractKey),
  );
  const byReferenceKey = new Map<string, StudioEntityComponentReference>();
  for (const reference of references) {
    const contract = reference.inspectorContract;
    if (
      reference.ownerEntityId !== ownerEntityId
      || contract === null
      || !advertised.has(contractKey(contract))
    ) {
      continue;
    }
    byReferenceKey.set(contributionKey({
      componentTypeId: reference.componentTypeId,
      contract,
    }), reference);
  }
  return Object.freeze(contributions.flatMap((contribution) => {
    const reference = byReferenceKey.get(contribution.key);
    return reference === undefined
      ? []
      : [Object.freeze({ contribution, reference })];
  }));
}

export function studioEntityInspectorInstanceKey(
  contributionKeyValue: string,
  context: StudioEntityInspectorContext,
): string {
  return [
    contributionKeyValue,
    String(context.ownerEntityId),
    String(context.projectGeneration),
    String(context.selectionGeneration),
    String(context.contractGeneration),
  ].join('\u0000');
}

export function sameStudioEntityInspectorContext(
  left: StudioEntityInspectorContext,
  right: StudioEntityInspectorContext,
): boolean {
  return left.ownerEntityId === right.ownerEntityId
    && left.componentTypeId === right.componentTypeId
    && sameContract(left.inspectorContract, right.inspectorContract)
    && left.project.projectId === right.project.projectId
    && left.project.projectHash === right.project.projectHash
    && left.projectGeneration === right.projectGeneration
    && left.selectionGeneration === right.selectionGeneration
    && left.contractGeneration === right.contractGeneration
    && left.adapterId === right.adapterId;
}

function sameContract(
  left: StudioEntityInspectorContractIdentity,
  right: StudioEntityInspectorContractIdentity,
): boolean {
  return left.contractId === right.contractId
    && left.contractVersion === right.contractVersion;
}

function contributionKey(
  contribution: Pick<StudioEntityInspectorContribution, 'componentTypeId' | 'contract'>,
): string {
  return `${contribution.componentTypeId}\u0000${contractKey(contribution.contract)}`;
}

function contractKey(contract: StudioEntityInspectorContractIdentity): string {
  return `${contract.contractId}\u0000${String(contract.contractVersion)}`;
}

function validateIdentity(value: string, path: string): void {
  if (
    value.length === 0
    || value.length > MAX_STUDIO_ENTITY_INSPECTOR_IDENTITY_BYTES
    || !/^[a-z][a-z0-9._-]*$/u.test(value)
  ) {
    invalid(
      `${path} must use 1..=${String(MAX_STUDIO_ENTITY_INSPECTOR_IDENTITY_BYTES)} bytes of stable lowercase ASCII identity`,
    );
  }
}

function compareText(left: string, right: string): number {
  if (left < right) return -1;
  if (left > right) return 1;
  return 0;
}

function invalid(message: string): never {
  throw new StudioEntityInspectorContributionError(
    'inspectorContribution.invalid',
    message,
  );
}
