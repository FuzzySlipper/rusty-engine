import {
  RULE_LIMITS,
  type JsonValue,
  type RulePackage,
} from './generated.js';
import { RuleContractError } from './error.js';
import { parseStrictJson } from './json.js';
import { admitRulePackageValue } from './validation.js';

export function decodeRulePackage(
  bytes: Uint8Array,
): RulePackage<JsonValue> {
  if (bytes.byteLength > RULE_LIMITS.maxEncodedRulePackageBytes) {
    throw new RuleContractError(
      'artifact-quota-exceeded',
      '$',
      'encoded artifact exceeds the package byte limit',
      {
        actual: bytes.byteLength,
        maximum: RULE_LIMITS.maxEncodedRulePackageBytes,
      },
    );
  }
  const parsed = parseStrictJson(bytes);
  return admitRulePackageValue(parsed.value);
}
