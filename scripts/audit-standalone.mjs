import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const baselinePath = resolve(repoRoot, "scripts/standalone-dependency-baseline.json");
const baseline = JSON.parse(readFileSync(baselinePath, "utf8"));
if (baseline.schemaVersion !== 2) {
  throw new Error(`unsupported standalone dependency baseline schema ${String(baseline.schemaVersion)}`);
}
const permittedProvenance = new Set(baseline.permittedProvenanceReferences);
const auditControlFiles = new Set([
  "scripts/audit-standalone.mjs",
  "scripts/standalone-dependency-baseline.json",
]);

const tracked = trackedReferences();
const trackedSet = new Set(tracked);
const operationalReferences = new Set([
  ...tracked.filter((entry) => !permittedProvenance.has(entry)),
  ...cargoLocalDependencies(),
  ...pnpmLocalDependencies(),
]);
const actual = [...operationalReferences].sort();
const actualProvenance = tracked.filter((entry) => permittedProvenance.has(entry)).sort();

if (process.argv.includes("--print")) {
  console.log(JSON.stringify({
    operationalReferences: actual,
    permittedProvenanceReferences: actualProvenance,
  }, null, 2));
  process.exit(0);
}

const expected = [...baseline.operationalReferences].sort();
const unexpected = actual.filter((entry) => !expected.includes(entry));
const missing = expected.filter((entry) => !actual.includes(entry));
const staleProvenance = [...permittedProvenance]
  .filter((entry) => !trackedSet.has(entry))
  .sort();
if (unexpected.length > 0 || missing.length > 0 || staleProvenance.length > 0) {
  if (unexpected.length > 0) {
    console.error("unexpected operational Asha references:");
    unexpected.forEach((entry) => console.error(`  + ${entry}`));
  }
  if (missing.length > 0) {
    console.error("baseline operational Asha references no longer present:");
    missing.forEach((entry) => console.error(`  - ${entry}`));
  }
  if (staleProvenance.length > 0) {
    console.error("permitted provenance references no longer present exactly:");
    staleProvenance.forEach((entry) => console.error(`  - ${entry}`));
  }
  console.error(
    "Update the extraction contract and exact provenance baseline deliberately; no whole file is exempt.",
  );
  process.exit(1);
}

console.log(
  `standalone dependency audit matched baseline: ${String(actual.length)} operational Asha references`,
);

function trackedReferences() {
  const tracked = execFileSync(
    "git",
    ["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
    {
      cwd: repoRoot,
      encoding: "utf8",
    },
  ).split("\0").filter(Boolean);
  const references = [];
  for (const file of tracked) {
    const path = resolve(repoRoot, file);
    if (auditControlFiles.has(file) || !existsSync(path)) {
      continue;
    }
    const bytes = readFileSync(path);
    if (bytes.includes(0)) {
      continue;
    }
    for (const line of bytes.toString("utf8").split(/\r?\n/u)) {
      if (!/(?:asha-engine|@asha\/)/iu.test(line)) {
        continue;
      }
      references.push(`tracked|${file}|${normalizeLine(line)}`);
    }
  }
  return references;
}

function cargoLocalDependencies() {
  const metadata = JSON.parse(execFileSync(
    "cargo",
    ["metadata", "--format-version", "1"],
    { cwd: repoRoot, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  ));
  return metadata.packages
    .filter((pkg) => pkg.source === null && isOutsideRepo(pkg.manifest_path))
    .map((pkg) => `cargo-local|${pkg.name}|${normalizeExternalPath(pkg.manifest_path)}`);
}

function pnpmLocalDependencies() {
  const listing = JSON.parse(execFileSync(
    "pnpm",
    ["list", "--recursive", "--depth", "Infinity", "--json"],
    { cwd: repoRoot, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  ));
  const references = new Set();
  visit(listing, "workspace");
  return [...references];

  function visit(value, name) {
    if (Array.isArray(value)) {
      value.forEach((item) => visit(item, name));
      return;
    }
    if (value === null || typeof value !== "object") {
      return;
    }
    if (
      typeof value.path === "string" &&
      isOutsideRepo(value.path) &&
      value.path.replaceAll("\\", "/").includes("/asha-engine/")
    ) {
      references.add(
        `pnpm-local|${String(value.from ?? value.name ?? name)}|${normalizeExternalPath(value.path)}`,
      );
    }
    for (const [key, child] of Object.entries(value)) {
      visit(child, key);
    }
  }
}

function isOutsideRepo(path) {
  const local = relative(repoRoot, resolve(path));
  return local === ".." || local.startsWith(`..${sep}`);
}

function normalizeExternalPath(path) {
  const portable = path.replaceAll("\\", "/");
  const donor = portable.indexOf("/asha-engine/");
  return donor >= 0 ? portable.slice(donor + 1) : portable;
}

function normalizeLine(line) {
  return line.trim().replaceAll(/\s+/gu, " ");
}
