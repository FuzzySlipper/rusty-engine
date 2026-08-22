// Generated from the Rust gameplay-standard contract. Do not edit by hand.
// Run: pnpm --dir rules run generate

export const STANDARD_CONTRACT_DESCRIPTOR_VERSION = 1 as const;
export const STANDARD_FAMILIES = Object.freeze({
  exact: Object.freeze({ schemaVersion: 1, evaluatorSemanticsVersion: 1, operations: Object.freeze(["literal","input","add","subtract","multiply","floorDivide","truncatingDivide","fixedPower","min","max"] as const), inputKinds: Object.freeze([{"fields":["kind","role","id"],"tag":"parameter"},{"fields":["kind","role","id"],"tag":"fact"},{"fields":["kind","role","id"],"tag":"roll"},{"fields":["kind","role","id","minimum","maximum"],"tag":"boundedRoll"},{"fields":["kind","role","id"],"tag":"choice"},{"fields":["kind","role","stat"],"tag":"standardStat"},{"fields":["kind","role","track"],"tag":"standardTrackCurrent"},{"fields":["kind","role","track"],"tag":"standardTrackMaximum"}] as const), literal: Object.freeze({"encoding":"safe-integer","field":"value","negativeZero":false} as const) }),
  continuous: Object.freeze({ schemaVersion: 2, evaluatorSemanticsVersion: 1, operations: Object.freeze(["literal","input","add","subtract","multiply","divide","min","max"] as const), inputKinds: Object.freeze([{"fields":["kind","role","id"],"tag":"parameter"},{"fields":["kind","role","id"],"tag":"fact"},{"fields":["kind","role","id"],"tag":"roll"},{"fields":["kind","role","id"],"tag":"choice"}] as const), literal: Object.freeze({"alphabet":"0123456789abcdef","encoding":"binary64-bits","exponentLeadingHexDigits":3,"exponentMask":2047,"field":"bits","finite":true,"lowercase":true,"negativeZero":false,"negativeZeroBits":"8000000000000000","width":16} as const) }),
} as const);
export const STANDARD_FIELD_ORDER = Object.freeze({"Aggregate":["op","values"],"Binary":["op","left","right"],"ContinuousLiteral":["op","bits"],"Definition":["family","roles","semanticsVersion","source","subject","tree"],"Input":["op","input"],"Literal":["op","value"],"Role":["role","capabilities"]} as const);
export const STANDARD_IDENTITIES = Object.freeze({"capability":{"maximumBytes":96,"pattern":"^[a-z][a-z0-9._-]*$"},"extensionKind":{"maximumBytes":96,"pattern":"^[a-z][a-z0-9._-]*$"},"input":{"maximumBytes":96,"pattern":"^[a-z][a-z0-9._-]*$"},"mechanicsStat":{"maximumBytes":96,"pattern":"^[a-z][a-z0-9._-]*$"},"mechanicsTrack":{"maximumBytes":96,"pattern":"^[a-z][a-z0-9._-]*$"},"role":{"maximumBytes":96,"pattern":"^[a-z][a-z0-9._-]*$"},"source":{"maximumBytes":128,"pattern":"^[ -~]+$","trimmed":true},"subject":{"maximumBytes":128,"pattern":"^[ -~]+$","trimmed":true}} as const);
export const STANDARD_FAILURE_CODES = Object.freeze(["unknown-field","wrong-family","unsupported-semantics-version","invalid-identity","non-canonical-roles","undeclared-input-role","invalid-literal","invalid-node","depth-quota-exceeded","node-quota-exceeded","input-quota-exceeded","arity-quota-exceeded","work-quota-exceeded","source-correlation-mismatch","extension-schema-mismatch","extension-payload-too-large","missing-product-capability"] as const);
export const STANDARD_LIMITS = Object.freeze({
  maxRoleIdBytes: 96,
  maxCapabilitiesPerRole: 32,
  exact: Object.freeze({"maximumArity":16,"maximumDepth":32,"maximumInputs":64,"maximumNodes":256,"maximumScalar":1000000000000,"maximumWork":512,"minimumScalar":-1000000000000} as const),
  continuous: Object.freeze({"maximumArity":16,"maximumDepth":32,"maximumInputs":64,"maximumNodes":256,"maximumWork":512} as const),
  maxExtensionBytes: 65536,
} as const);
export const STANDARD_EXTENSION = Object.freeze({"family":"standardExtension","fieldOrder":["family","kind","namespace","payload","schemaVersion","source","subject"],"maximumBytes":65536,"namespaceMaximumBytes":96,"namespacePattern":"^[a-z][a-z0-9.-]*$","runtime":"downstream-rust-closed-enum","schemaVersionMaximum":4294967295} as const);
export const STANDARD_COMPOSED_EXACT = Object.freeze({"definitionFieldOrder":["family","roles","semanticsVersion","source","subject","extension","tree"],"extensionFieldOrder":["namespace","schemaVersion"],"family":"composedExact","productFieldOrder":["op","kind","payload","source","subject"],"productOp":"product","runtime":"downstream-rust-static-codec","schemaVersion":1,"semanticsVersion":1} as const);

export type StandardFamily = keyof typeof STANDARD_FAMILIES;
export type ExactOperation = (typeof STANDARD_FAMILIES.exact.operations)[number];
export type ContinuousOperation = (typeof STANDARD_FAMILIES.continuous.operations)[number];
export type StandardContractErrorCode = (typeof STANDARD_FAILURE_CODES)[number];
export type JsonValue = null | boolean | number | string | readonly JsonValue[] | { readonly [key: string]: JsonValue };
export type ExactInput = { readonly kind: "parameter"; readonly role: string; readonly id: string; }
  | { readonly kind: "fact"; readonly role: string; readonly id: string; }
  | { readonly kind: "roll"; readonly role: string; readonly id: string; }
  | { readonly kind: "boundedRoll"; readonly role: string; readonly id: string; readonly minimum: number; readonly maximum: number; }
  | { readonly kind: "choice"; readonly role: string; readonly id: string; }
  | { readonly kind: "standardStat"; readonly role: string; readonly stat: string; }
  | { readonly kind: "standardTrackCurrent"; readonly role: string; readonly track: string; }
  | { readonly kind: "standardTrackMaximum"; readonly role: string; readonly track: string; };
export type ContinuousInput = { readonly kind: "parameter"; readonly role: string; readonly id: string; }
  | { readonly kind: "fact"; readonly role: string; readonly id: string; }
  | { readonly kind: "roll"; readonly role: string; readonly id: string; }
  | { readonly kind: "choice"; readonly role: string; readonly id: string; };
export type ExactTree =
  | { readonly op: 'literal'; readonly value: number }
  | { readonly op: 'input'; readonly input: ExactInput }
  | { readonly op: "add" | "subtract" | "multiply" | "floorDivide" | "truncatingDivide"; readonly left: ExactTree; readonly right: ExactTree }
  | { readonly op: 'fixedPower'; readonly base: ExactTree; readonly exponent: ExactTree; readonly scale: number }
  | { readonly op: 'min' | 'max'; readonly values: readonly ExactTree[] };
export type ContinuousTree =
  | { readonly op: 'literal'; readonly bits: string }
  | { readonly op: 'input'; readonly input: ContinuousInput }
  | { readonly op: "add" | "subtract" | "multiply" | "divide"; readonly left: ContinuousTree; readonly right: ContinuousTree }
  | { readonly op: 'min' | 'max'; readonly values: readonly ContinuousTree[] };
export interface StandardRole { readonly role: string; readonly capabilities: readonly string[] }
export interface ExactDefinitionPayload { readonly family: 'exact'; readonly roles: readonly StandardRole[]; readonly semanticsVersion: typeof STANDARD_FAMILIES.exact.evaluatorSemanticsVersion; readonly source: string; readonly subject: string; readonly tree: ExactTree }
export interface ContinuousDefinitionPayload { readonly family: 'continuous'; readonly roles: readonly StandardRole[]; readonly semanticsVersion: typeof STANDARD_FAMILIES.continuous.evaluatorSemanticsVersion; readonly source: string; readonly subject: string; readonly tree: ContinuousTree }
export type StandardDefinitionPayload = ExactDefinitionPayload | ContinuousDefinitionPayload;
export interface StandardExtensionArtifact { readonly family: typeof STANDARD_EXTENSION.family; readonly kind: string; readonly namespace: string; readonly payload: JsonValue; readonly schemaVersion: number; readonly source: string; readonly subject: string }
export interface ComposedExactExtensionSchema { readonly namespace: string; readonly schemaVersion: number }
export type ComposedExactProductLeaf<Payload extends JsonValue> = { readonly op: typeof STANDARD_COMPOSED_EXACT.productOp; readonly kind: string; readonly payload: Payload; readonly source: string; readonly subject: string };
export type ComposedExactTree<Payload extends JsonValue> =
  | { readonly op: 'literal'; readonly value: number }
  | { readonly op: 'input'; readonly input: ExactInput }
  | { readonly op: "add" | "subtract" | "multiply" | "floorDivide" | "truncatingDivide"; readonly left: ComposedExactTree<Payload>; readonly right: ComposedExactTree<Payload> }
  | { readonly op: 'fixedPower'; readonly base: ComposedExactTree<Payload>; readonly exponent: ComposedExactTree<Payload>; readonly scale: number }
  | { readonly op: 'min' | 'max'; readonly values: readonly ComposedExactTree<Payload>[] }
  | ComposedExactProductLeaf<Payload>;
export interface ComposedExactDefinitionPayload<Payload extends JsonValue> { readonly family: typeof STANDARD_COMPOSED_EXACT.family; readonly extension: ComposedExactExtensionSchema; readonly roles: readonly StandardRole[]; readonly semanticsVersion: typeof STANDARD_COMPOSED_EXACT.semanticsVersion; readonly source: string; readonly subject: string; readonly tree: ComposedExactTree<Payload> }
export class StandardContractError extends Error {
  public constructor(public readonly code: StandardContractErrorCode, message: string) { super(message); this.name = 'StandardContractError'; }
}
