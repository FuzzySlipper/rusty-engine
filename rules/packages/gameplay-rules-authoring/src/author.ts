import { Buffer } from 'node:buffer';

import {
  RULE_PACKAGE_ARTIFACT_KIND,
  RULE_PACKAGE_SCHEMA_VERSION,
  admitRulePackageValue,
  type JsonValue,
  type RuleFingerprint,
  type RulePackage,
} from '@rusty-engine/gameplay-rules-contracts';

import {
  canonicalizeRulePackage,
  fingerprintCanonicalRulePackage,
} from './canonical.js';

export interface RulePackageDependencyDraft {
  readonly domain: string;
  readonly package: string;
  readonly version: number;
  readonly fingerprint?: string;
}

export interface RuleSourceDraft {
  readonly id: string;
  readonly path: string;
}

export interface RuleProvenanceDraft {
  readonly subject: string;
  readonly source: string;
  readonly line?: number;
  readonly column?: number;
}

export interface RulePackageDraft<Payload extends JsonValue> {
  readonly domain: string;
  readonly package: string;
  readonly version: number;
  readonly dependencies?: readonly RulePackageDependencyDraft[];
  readonly sources?: readonly RuleSourceDraft[];
  readonly provenance?: readonly RuleProvenanceDraft[];
  readonly payload: Payload;
}

export interface CanonicalRuleArtifact<Payload extends JsonValue> {
  readonly package: RulePackage<Payload>;
  readonly canonicalJson: string;
  readonly fingerprint: RuleFingerprint;
}

export function authorRulePackage<Payload extends JsonValue>(
  draft: RulePackageDraft<Payload>,
): CanonicalRuleArtifact<Payload> {
  const packageValue = admitRulePackageValue<Payload>({
    kind: RULE_PACKAGE_ARTIFACT_KIND,
    schemaVersion: RULE_PACKAGE_SCHEMA_VERSION,
    domain: draft.domain,
    package: draft.package,
    version: draft.version,
    dependencies: draft.dependencies ?? [],
    sources: draft.sources ?? [],
    provenance: draft.provenance ?? [],
    payload: draft.payload,
  });
  const canonicalJson = canonicalizeRulePackage(packageValue);
  return Object.freeze({
    package: packageValue,
    canonicalJson,
    fingerprint: fingerprintCanonicalRulePackage(canonicalJson),
  });
}

export function canonicalRuleArtifactBytes<Payload extends JsonValue>(
  artifact: CanonicalRuleArtifact<Payload>,
): Uint8Array {
  return Uint8Array.from(Buffer.from(artifact.canonicalJson, 'utf8'));
}
