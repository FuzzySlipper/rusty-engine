export type RuntimeCompositionAuthoringErrorCode =
  | 'missing-field'
  | 'unknown-field'
  | 'invalid-field-type'
  | 'invalid-identity'
  | 'invalid-capability-target'
  | 'unknown-engine-capability'
  | 'unknown-capability'
  | 'unknown-definition'
  | 'unknown-intent-descriptor'
  | 'invalid-input-value-kind'
  | 'invalid-input-trigger'
  | 'input-trigger-value-kind'
  | 'input-chord-quota-exceeded'
  | 'duplicate-input-chord-control'
  | 'duplicate-entry'
  | 'quota-exceeded'
  | 'json-depth-exceeded'
  | 'json-node-quota-exceeded'
  | 'invalid-json-value'
  | 'artifact-quota-exceeded'
  | 'invalid-operation'
  | 'invalid-schedule-phase'
  | 'invalid-schedule-mode'
  | 'invalid-schedule-cadence'
  | 'unknown-schedule-dependency'
  | 'schedule-cross-phase-dependency'
  | 'schedule-placement-dependency'
  | 'schedule-dependency-cycle'
  | 'schedule-access-ambiguity'
  | 'product-kernel-catalog-invalid'
  | 'product-kernel-catalog-stale'
  | 'product-kernel-catalog-unsorted'
  | 'unknown-product-kernel-capability';

/** A bounded, source-free diagnostic for build-time Runtime Composition authoring. */
export class RuntimeCompositionAuthoringError extends Error {
  public constructor(
    public readonly code: RuntimeCompositionAuthoringErrorCode,
    public readonly logicalPath: string,
    message: string,
    public readonly details: Readonly<Record<string, string | number>> = {},
  ) {
    super(message);
    this.name = 'RuntimeCompositionAuthoringError';
  }
}
