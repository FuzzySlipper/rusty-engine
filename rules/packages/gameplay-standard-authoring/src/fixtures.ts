import type { CanonicalRuleArtifact } from '@rusty-engine/gameplay-rules-authoring';
import type { JsonValue } from '@rusty-engine/gameplay-rules-contracts';

import {
  authorBinary64StandardExtension,
  authorComposedExactDefinition,
  authorContinuousDefinition,
  authorExactDefinition,
  authorStandardExtension,
  declareStandardExtensionSchema,
} from './author.js';
import type { StrictComposedExactProductCodec } from '@rusty-engine/gameplay-standard-contracts';

/** The committed Rust/TypeScript convergence vectors. They contain no product runtime behavior. */
export function standardFixtureArtifacts(): Readonly<Record<string, CanonicalRuleArtifact<JsonValue>>> {
  const exact = authorExactDefinition({
    domain: 'game', package: 'standard', version: 1,
    sources: [{ id: 'rules', path: 'rules.json' }],
    provenance: [{ subject: 'health_formula', source: 'rules' }],
    definition: {
      family: 'exact', roles: [{ role: 'self', capabilities: ['read.stat'] }], semanticsVersion: 1,
      source: 'rules', subject: 'health_formula',
      tree: {
        op: 'add',
        left: { op: 'input', input: { kind: 'standardStat', role: 'self', stat: 'health' } },
        right: { op: 'max', values: [
          { op: 'literal', value: 3 },
          { op: 'multiply', left: { op: 'literal', value: 2 }, right: { op: 'input', input: { kind: 'parameter', role: 'self', id: 'bonus' } } },
        ] },
      },
    },
  });
  const continuous = authorContinuousDefinition({
    domain: 'game', package: 'standard', version: 2,
    sources: [{ id: 'rules', path: 'rules.json' }],
    provenance: [{ subject: 'wind_formula', source: 'rules' }],
    definition: {
      family: 'continuous', roles: [{ role: 'caster', capabilities: ['read.wind'] }], semanticsVersion: 1,
      source: 'rules', subject: 'wind_formula',
      tree: {
        op: 'subtract',
        left: { op: 'add', left: { op: 'literal', bits: '0000000000000000' }, right: { op: 'input', input: { kind: 'parameter', role: 'caster', id: 'wind' } } },
        right: { op: 'literal', bits: '0000000000000001' },
      },
    },
  });
  const fixedPowerBoundedRoll = authorExactDefinition({
    domain: 'game', package: 'numeric', version: 1,
    sources: [{ id: 'rules', path: 'rules.json' }],
    provenance: [{ subject: 'numeric_formula', source: 'rules' }],
    definition: {
      family: 'exact', roles: [{ role: 'self', capabilities: [] }], semanticsVersion: 1,
      source: 'rules', subject: 'numeric_formula',
      tree: {
        op: 'fixedPower', scale: 1000,
        base: { op: 'input', input: { kind: 'boundedRoll', role: 'self', id: 'attack', minimum: 1, maximum: 20 } },
        exponent: { op: 'literal', value: 2 },
      },
    },
  });
  const schema = declareStandardExtensionSchema<{ readonly option: string }>('example.combat', 1);
  const extensionSchema1 = authorStandardExtension({
    domain: 'game', package: 'combat-extension', version: 1,
    sources: [{ id: 'rules', path: 'rules.json' }],
    provenance: [{ subject: 'guard', source: 'rules' }],
    schema, kind: 'combat.option', subject: 'guard', source: 'rules', payload: { option: 'guard' },
  });
  const binary64Schema = declareStandardExtensionSchema<{ readonly weight: number }>('example.combat', 1);
  const extensionSchema2 = authorBinary64StandardExtension({
    domain: 'game', package: 'combat-extension', version: 2,
    sources: [{ id: 'rules', path: 'rules.json' }],
    provenance: [{ subject: 'guard-weight', source: 'rules' }],
    schema: binary64Schema, kind: 'combat.weight', subject: 'guard-weight', source: 'rules', payload: { weight: 1.5 },
  });
  const composedCodec: StrictComposedExactProductCodec<{ readonly slot: 'combat.equipped-tool' | 'combat.protection' }> = {
    schema: { namespace: 'example.combat', schemaVersion: 1 },
    decode(payload: unknown) {
      if (typeof payload !== 'object' || payload === null || Array.isArray(payload)) throw new Error('product leaf must be an object');
      const object = payload as Record<string, unknown>;
      if (Object.keys(object).length !== 1 || (object['slot'] !== 'combat.equipped-tool' && object['slot'] !== 'combat.protection')) throw new Error('product leaf has unknown fields or an invalid slot');
      return { slot: object['slot'] };
    },
    encode(payload) { return { slot: payload.slot }; },
  };
  const composedExact = authorComposedExactDefinition({
    domain: 'game', package: 'composed', version: 1,
    sources: [{ id: 'rules', path: 'rules.json' }],
    provenance: [{ subject: 'damage_check', source: 'rules' }, { subject: 'tool_leaf', source: 'rules' }, { subject: 'protection_leaf', source: 'rules' }],
    codec: composedCodec,
    definition: {
      family: 'composedExact', semanticsVersion: 1, subject: 'damage_check', source: 'rules', extension: composedCodec.schema,
      roles: [{ role: 'attacker', capabilities: ['read.equipped-tool'] }, { role: 'defender', capabilities: ['read.protection'] }],
      tree: { op: 'max', values: [
        { op: 'literal', value: 1 },
        { op: 'min', values: [{ op: 'floorDivide', left: { op: 'multiply', left: { op: 'product', kind: 'combat.equipped-tool', subject: 'tool_leaf', source: 'rules', payload: { slot: 'combat.equipped-tool' } }, right: { op: 'literal', value: 2 } }, right: { op: 'add', left: { op: 'product', kind: 'combat.protection', subject: 'protection_leaf', source: 'rules', payload: { slot: 'combat.protection' } }, right: { op: 'literal', value: 1 } } }, { op: 'literal', value: 7 }] },
      ] },
    },
  });
  return Object.freeze({ exact, continuous, fixedPowerBoundedRoll, extensionSchema1, extensionSchema2, composedExact });
}
