export type RuleContractErrorCode =
  | 'artifact-quota-exceeded'
  | 'malformed-utf8'
  | 'malformed-json'
  | 'duplicate-json-key'
  | 'wrong-artifact-kind'
  | 'unsupported-schema-version'
  | 'missing-field'
  | 'unknown-field'
  | 'invalid-field-type'
  | 'invalid-identity'
  | 'invalid-version'
  | 'invalid-source-path'
  | 'invalid-source-location'
  | 'invalid-fingerprint'
  | 'json-integer-out-of-range'
  | 'json-number-out-of-range'
  | 'quota-exceeded'
  | 'json-depth-exceeded'
  | 'json-node-quota-exceeded'
  | 'duplicate-dependency'
  | 'duplicate-source'
  | 'duplicate-provenance'
  | 'unknown-provenance-source'
  | 'self-dependency'
  | 'noncanonical-value'
  | 'diagnostic-invalid';

export class RuleContractError extends Error {
  public constructor(
    public readonly code: RuleContractErrorCode,
    public readonly logicalPath: string,
    message: string,
    public readonly details: Readonly<Record<string, string | number>> = {},
  ) {
    super(message);
    this.name = 'RuleContractError';
  }
}
