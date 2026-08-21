import type { CanonicalRuleArtifact } from '@rusty-engine/gameplay-rules-authoring';
import type { JsonValue } from '@rusty-engine/gameplay-rules-contracts';

import {
  authorBinary64StandardExtension,
  authorContinuousDefinition,
  authorExactDefinition,
  authorStandardExtension,
  declareStandardExtensionSchema,
} from './author.js';

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
  return Object.freeze({ exact, continuous, extensionSchema1, extensionSchema2 });
}
