import { Buffer } from 'node:buffer';
import { createHash } from 'node:crypto';

import {
  RULE_LIMITS,
  RuleContractError,
  admitRulePackage,
  parseRuleFingerprint,
  type JsonValue,
  type RuleFingerprint,
  type RulePackage,
} from '@rusty-engine/gameplay-rules-contracts';

export function canonicalizeRulePackage<Payload extends JsonValue>(
  value: RulePackage<Payload>,
): string {
  const packageValue = admitRulePackage(value);
  const output = new BoundedUtf8Writer(
    RULE_LIMITS.maxEncodedRulePackageBytes,
  );
  output.write('{"kind":', '$/kind');
  output.writeString(packageValue.kind, '$/kind');
  output.write(
    `,"schemaVersion":${String(packageValue.schemaVersion)},"domain":`,
    '$/domain',
  );
  output.writeString(packageValue.domain, '$/domain');
  output.write(',"package":', '$/package');
  output.writeString(packageValue.package, '$/package');
  output.write(',"version":', '$/version');
  output.write(String(packageValue.version), '$/version');
  output.write(',"dependencies":[', '$/dependencies');
  packageValue.dependencies.forEach((dependency, index) => {
    const path = `$/dependencies/${String(index)}`;
    if (index !== 0) output.write(',', '$/dependencies');
    output.write('{"domain":', `${path}/domain`);
    output.writeString(dependency.domain, `${path}/domain`);
    output.write(',"package":', `${path}/package`);
    output.writeString(dependency.package, `${path}/package`);
    output.write(',"version":', `${path}/version`);
    output.write(String(dependency.version), `${path}/version`);
    if (dependency.fingerprint !== undefined) {
      output.write(',"fingerprint":', `${path}/fingerprint`);
      output.writeString(dependency.fingerprint, `${path}/fingerprint`);
    }
    output.write('}', path);
  });
  output.write('],"sources":[', '$/sources');
  packageValue.sources.forEach((source, index) => {
    const path = `$/sources/${String(index)}`;
    if (index !== 0) output.write(',', '$/sources');
    output.write('{"id":', `${path}/id`);
    output.writeString(source.id, `${path}/id`);
    output.write(',"path":', `${path}/path`);
    output.writeString(source.path, `${path}/path`);
    output.write('}', path);
  });
  output.write('],"provenance":[', '$/provenance');
  packageValue.provenance.forEach((entry, index) => {
    const path = `$/provenance/${String(index)}`;
    if (index !== 0) output.write(',', '$/provenance');
    output.write('{"subject":', `${path}/subject`);
    output.writeString(entry.subject, `${path}/subject`);
    output.write(',"source":', `${path}/source`);
    output.writeString(entry.source, `${path}/source`);
    if (entry.line !== undefined) {
      output.write(',"line":', `${path}/line`);
      output.write(String(entry.line), `${path}/line`);
    }
    if (entry.column !== undefined) {
      output.write(',"column":', `${path}/column`);
      output.write(String(entry.column), `${path}/column`);
    }
    output.write('}', path);
  });
  output.write('],"payload":', '$/payload');
  output.writeValue(packageValue.payload, '$/payload');
  output.write('}\n', '$');
  return output.finish();
}

export function fingerprintCanonicalRulePackage(
  canonicalJson: string,
): RuleFingerprint {
  return parseRuleFingerprint(
    createHash('sha256').update(canonicalJson, 'utf8').digest('hex'),
  );
}

class BoundedUtf8Writer {
  private readonly chunks: string[] = [];
  private bytes = 0;

  public constructor(private readonly maximum: number) {}

  public write(value: string, logicalPath: string): void {
    const next = this.bytes + Buffer.byteLength(value, 'utf8');
    if (!Number.isSafeInteger(next)) {
      throw new RuleContractError(
        'artifact-quota-exceeded',
        logicalPath,
        'canonical artifact byte accounting overflowed',
      );
    }
    if (next > this.maximum) {
      throw new RuleContractError(
        'artifact-quota-exceeded',
        logicalPath,
        'canonical artifact exceeds the package byte limit',
        { actual: next, maximum: this.maximum },
      );
    }
    this.bytes = next;
    this.chunks.push(value);
  }

  public writeString(value: string, logicalPath: string): void {
    const escaped = value.replace(
      /["\\\u0000-\u001f]/g,
      (character) => {
        const code = character.codePointAt(0) as number;
        switch (character) {
          case '"':
            return '\\"';
          case '\\':
            return '\\\\';
          case '\b':
            return '\\b';
          case '\f':
            return '\\f';
          case '\n':
            return '\\n';
          case '\r':
            return '\\r';
          case '\t':
            return '\\t';
          default:
            return `\\u${code.toString(16).padStart(4, '0')}`;
        }
      },
    );
    this.write(`"${escaped}"`, logicalPath);
  }

  public writeValue(value: JsonValue, logicalPath: string): void {
    if (value === null) {
      this.write('null', logicalPath);
    } else if (typeof value === 'boolean' || typeof value === 'number') {
      this.write(String(value), logicalPath);
    } else if (typeof value === 'string') {
      this.writeString(value, logicalPath);
    } else if (Array.isArray(value)) {
      this.write('[', logicalPath);
      value.forEach((entry, index) => {
        if (index !== 0) this.write(',', logicalPath);
        this.writeValue(entry, `${logicalPath}/${String(index)}`);
      });
      this.write(']', logicalPath);
    } else {
      this.write('{', logicalPath);
      const entries = Object.entries(value).sort(([left], [right]) =>
        Buffer.compare(Buffer.from(left, 'utf8'), Buffer.from(right, 'utf8')),
      );
      entries.forEach(([key, entry], index) => {
        if (index !== 0) this.write(',', logicalPath);
        this.writeString(key, `${logicalPath}/<key>`);
        this.write(':', logicalPath);
        this.writeValue(
          entry,
          `${logicalPath}/${key.replaceAll('~', '~0').replaceAll('/', '~1')}`,
        );
      });
      this.write('}', logicalPath);
    }
  }

  public finish(): string {
    return this.chunks.join('');
  }
}
