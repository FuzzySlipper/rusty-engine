import { authorBinary64RulePackage, authorRulePackage, type CanonicalRuleArtifact, type RulePackageDraft } from '@rusty-engine/gameplay-rules-authoring';
import type { JsonValue } from '@rusty-engine/gameplay-rules-contracts';
import { StandardContractError, assertStandardExtensionArtifact, assertStandardPayload, decodeComposedExactPayload, type ComposedExactDefinitionPayload, type ComposedExactTree, type ContinuousDefinitionPayload, type ExactDefinitionPayload, type JsonValue as StandardJsonValue, type StandardExtensionArtifact, type StrictComposedExactProductCodec } from '@rusty-engine/gameplay-standard-contracts';

export type StandardDefinitionDraft<P extends ExactDefinitionPayload | ContinuousDefinitionPayload> = Omit<RulePackageDraft<JsonValue>, 'schemaVersion' | 'payload'> & { readonly definition: P };
export function authorExactDefinition(draft: StandardDefinitionDraft<ExactDefinitionPayload>): CanonicalRuleArtifact<JsonValue> {
  assertStandardPayload(draft.definition);
  assertCorrelation(draft.definition.subject, draft.definition.source, draft.provenance);
  return authorRulePackage({ ...envelope(draft), schemaVersion: 1, payload: draft.definition as unknown as JsonValue });
}
export function authorContinuousDefinition(draft: StandardDefinitionDraft<ContinuousDefinitionPayload>): CanonicalRuleArtifact<JsonValue> {
  assertStandardPayload(draft.definition);
  assertCorrelation(draft.definition.subject, draft.definition.source, draft.provenance);
  return authorBinary64RulePackage({ ...envelope(draft), payload: draft.definition as unknown as JsonValue });
}
export interface DeclaredStandardExtensionSchema<Payload extends JsonValue> { readonly namespace: string; readonly version: number; readonly _payload?: Payload }
export function declareStandardExtensionSchema<Payload extends JsonValue>(namespace: string, version: number): DeclaredStandardExtensionSchema<Payload> { assertStandardExtensionArtifact({ family: 'standardExtension', namespace, schemaVersion: version, kind: 'schema', subject: 'schema', source: 'schema', payload: null }); return Object.freeze({ namespace, version }); }
export type StandardExtensionDraft<Payload extends JsonValue> = Omit<RulePackageDraft<JsonValue>, 'schemaVersion' | 'payload'> & { readonly schema: DeclaredStandardExtensionSchema<Payload>; readonly kind: string; readonly subject: string; readonly source: string; readonly payload: Payload };
export function authorStandardExtension<Payload extends JsonValue>(draft: StandardExtensionDraft<Payload>): CanonicalRuleArtifact<JsonValue> {
  return authorExtension(draft, authorRulePackage);
}
/** Authors a schema-2 extension package when its opaque product payload needs binary64 JSON values. */
export function authorBinary64StandardExtension<Payload extends JsonValue>(draft: StandardExtensionDraft<Payload>): CanonicalRuleArtifact<JsonValue> {
  return authorExtension(draft, authorBinary64RulePackage);
}
/** Declares the product-owned, strict authoring codec required for one composed exact schema. */
export function declareComposedExactProductCodec<Payload extends StandardJsonValue>(codec: StrictComposedExactProductCodec<Payload>): StrictComposedExactProductCodec<Payload> {
  if (!codec || typeof codec.decode !== 'function' || typeof codec.encode !== 'function') throw new StandardContractError('extension-schema-mismatch', 'composed exact authoring requires a strict product codec');
  return Object.freeze(codec);
}
export type ComposedExactDefinitionDraft<Payload extends StandardJsonValue> = Omit<RulePackageDraft<JsonValue>, 'schemaVersion' | 'payload'> & { readonly codec: StrictComposedExactProductCodec<Payload>; readonly definition: ComposedExactDefinitionPayload<Payload> };
export type EmbeddedComposedExactDefinitionDraft<Payload extends StandardJsonValue> = Pick<ComposedExactDefinitionDraft<Payload>, 'codec' | 'definition' | 'provenance'> & { readonly parentSchemaVersion: 1 | 2 };
/** Validates an immutable generated subtree under the explicit aggregate numeric policy. */
export function composeEmbeddedComposedExactDefinition<Payload extends StandardJsonValue>(draft: EmbeddedComposedExactDefinitionDraft<Payload>): ComposedExactDefinitionPayload<Payload> {
  const definition = decodeComposedExactPayload(draft.definition, draft.codec, draft.parentSchemaVersion === 1 ? 'integer' : 'binary64');
  assertCorrelation(definition.subject, definition.source, draft.provenance);
  collectLeafProvenance(definition.tree, draft.provenance);
  return deepFreeze(JSON.parse(JSON.stringify(definition)) as ComposedExactDefinitionPayload<Payload>);
}
/** Authors a canonical schema-1 composedExact package. Product payload validation is owned by the explicit strict codec; generic arithmetic stays Rust Standard grammar. */
export function authorComposedExactDefinition<Payload extends StandardJsonValue>(draft: ComposedExactDefinitionDraft<Payload>): CanonicalRuleArtifact<JsonValue> {
  const definition = composeEmbeddedComposedExactDefinition({ ...draft, parentSchemaVersion: 1 });
  const { codec: _codec, definition: _definition, ...envelope } = draft;
  return authorRulePackage({ ...envelope, schemaVersion: 1, payload: definition as unknown as JsonValue });
}
function collectLeafProvenance<Payload extends StandardJsonValue>(tree: ComposedExactTree<Payload>, provenance: RulePackageDraft<JsonValue>['provenance']): void {
  if (tree.op === 'product') { assertCorrelation(tree.subject, tree.source, provenance); return; }
  if (tree.op === 'min' || tree.op === 'max') { for (const child of tree.values) collectLeafProvenance(child, provenance); return; }
  if (tree.op === 'add' || tree.op === 'subtract' || tree.op === 'multiply' || tree.op === 'floorDivide' || tree.op === 'truncatingDivide') { collectLeafProvenance(tree.left, provenance); collectLeafProvenance(tree.right, provenance); }
}
function authorExtension<Payload extends JsonValue>(draft: StandardExtensionDraft<Payload>, emit: (draft: Omit<RulePackageDraft<JsonValue>, 'schemaVersion'>) => CanonicalRuleArtifact<JsonValue>): CanonicalRuleArtifact<JsonValue> {
  const payload: StandardExtensionArtifact = { family: 'standardExtension', namespace: draft.schema.namespace, schemaVersion: draft.schema.version, kind: draft.kind, subject: draft.subject, source: draft.source, payload: draft.payload };
  assertStandardExtensionArtifact(payload);
  assertCorrelation(payload.subject, payload.source, draft.provenance);
  const { schema: _schema, kind: _kind, subject: _subject, source: _source, ...envelope } = draft;
  return emit({ ...envelope, payload: payload as unknown as JsonValue });
}
function envelope<P extends ExactDefinitionPayload | ContinuousDefinitionPayload>(draft: StandardDefinitionDraft<P>): Omit<RulePackageDraft<JsonValue>, 'schemaVersion' | 'payload'> { const { definition: _definition, ...value } = draft; return value; }
function assertCorrelation(subject: string, source: string, provenance: RulePackageDraft<JsonValue>['provenance']): void {
  if (!provenance?.some((entry) => entry.subject === subject && entry.source === source)) throw new StandardContractError('source-correlation-mismatch', `standard artifact ${subject} must correlate to provenance source ${source}`);
}
function deepFreeze<Value>(value: Value): Value {
  if (value && typeof value === 'object') {
    for (const child of Object.values(value as Record<string, unknown>)) deepFreeze(child);
    Object.freeze(value);
  }
  return value;
}
