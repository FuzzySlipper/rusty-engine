// Generated from the Rust gameplay-rules contract. Do not edit by hand.
// Run: pnpm --dir rules run generate

export const RULE_CONTRACT_DESCRIPTOR_VERSION = 1 as const;
export const RULE_PACKAGE_ARTIFACT_KIND = "rusty.gameplay-rules.package" as const;
export const RULE_PACKAGE_SCHEMA_VERSION = 1 as const;

export const RULE_LIMITS = Object.freeze({
  maxCanonicalRulePackageSetBytes: 16777216,
  maxDependenciesPerRulePackage: 32,
  maxDependenciesPerRulePackageSet: 512,
  maxDiagnosticCodeBytes: 64,
  maxDiagnosticLogicalPathBytes: 512,
  maxDiagnosticMessageBytes: 2048,
  maxEncodedRulePackageBytes: 4194304,
  maxJsonNestingDepth: 64,
  maxJsonNodesPerRulePackage: 100000,
  maxJsonNodesPerRulePackageSet: 400000,
  maxJsonStringBytes: 1048576,
  maxProvenancePerRulePackage: 4096,
  maxProvenancePerRulePackageSet: 16384,
  maxRuleDiagnostics: 256,
  maxRuleIdBytes: 128,
  maxRulePackagesPerSet: 64,
  maxSafeJsonInteger: 9007199254740991,
  maxSourcePathBytes: 512,
  maxSourcesPerRulePackage: 64,
  maxSourcesPerRulePackageSet: 1024,
} as const);

export const RULE_FIELD_ORDER = Object.freeze({
  RulePackage: Object.freeze(["kind","schemaVersion","domain","package","version","dependencies","sources","provenance","payload"] as const),
  RulePackageDependency: Object.freeze(["domain","package","version","fingerprint"] as const),
  RuleProvenance: Object.freeze(["subject","source","line","column"] as const),
  RuleSource: Object.freeze(["id","path"] as const),
} as const);

export type JsonPrimitive = null | boolean | number | string;
export type JsonValue = JsonPrimitive | readonly JsonValue[] | { readonly [key: string]: JsonValue };

declare const RuleDomainIdBrand: unique symbol;
export type RuleDomainId = string & { readonly [RuleDomainIdBrand]: true };

declare const RulePackageIdBrand: unique symbol;
export type RulePackageId = string & { readonly [RulePackageIdBrand]: true };

declare const RuleSourceIdBrand: unique symbol;
export type RuleSourceId = string & { readonly [RuleSourceIdBrand]: true };

declare const RuleSubjectIdBrand: unique symbol;
export type RuleSubjectId = string & { readonly [RuleSubjectIdBrand]: true };

declare const RuleFingerprintBrand: unique symbol;
export type RuleFingerprint = string & { readonly [RuleFingerprintBrand]: true };

export type RuleDiagnosticSeverity = "error" | "warning";

export interface RulePackageDependency {
  readonly domain: RuleDomainId;
  readonly package: RulePackageId;
  readonly version: number;
  readonly fingerprint?: RuleFingerprint;
}

export interface RuleSource {
  readonly id: RuleSourceId;
  readonly path: string;
}

export interface RuleProvenance {
  readonly subject: RuleSubjectId;
  readonly source: RuleSourceId;
  readonly line?: number;
  readonly column?: number;
}

export interface RulePackage<Payload extends JsonValue = JsonValue> {
  readonly kind: typeof RULE_PACKAGE_ARTIFACT_KIND;
  readonly schemaVersion: typeof RULE_PACKAGE_SCHEMA_VERSION;
  readonly domain: RuleDomainId;
  readonly package: RulePackageId;
  readonly version: number;
  readonly dependencies: readonly RulePackageDependency[];
  readonly sources: readonly RuleSource[];
  readonly provenance: readonly RuleProvenance[];
  readonly payload: Payload;
}

export interface RuleDiagnosticCorrelation {
  readonly subject: RuleSubjectId;
  readonly source: RuleSourceId;
  readonly line?: number;
  readonly column?: number;
}

export interface RuleDiagnostic {
  readonly code: string;
  readonly severity: RuleDiagnosticSeverity;
  readonly logicalPath: string;
  readonly message: string;
  readonly package?: RulePackageIdentity;
  readonly correlation?: RuleDiagnosticCorrelation;
}

export interface RulePackageIdentity {
  readonly domain: RuleDomainId;
  readonly package: RulePackageId;
  readonly version: number;
}
