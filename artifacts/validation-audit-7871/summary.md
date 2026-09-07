# Validation and limit candidate inventory

This is a heuristic review queue, not a semantic-completeness claim or a validity verdict.

Generated: `2026-09-07T12:07:00.282369+00:00`
Discovery: `git ls-files --cached --others --exclude-standard -z`

## Coverage

- Discovered paths: 1006
- Supported first-party source/config files scanned: 846
- Candidates: 34699 (4005 limit, 22009 validation, 11318 broad guard)
- Test/fixture candidates: 11553

## Exclusions

- audit_output: 5
- generated_artifact: 23
- inventory_tool: 1
- lockfile: 4
- unsupported_file_type: 112
- vendor_or_build: 15

Generated files:

- `inventory.jsonl` is the full raw candidate inventory.
- `limits.tsv` is the limit-only spreadsheet-friendly subset.
- `index.html` is a standalone filterable review view with source and copy links.
- `summary.json` contains machine-readable coverage and exclusion counts.

Candidates carry `review_status=unreviewed`. Keep human dispositions in a separate annotation file or system; rerunning this script replaces only its raw generated outputs.

Known survey limits: text patterns cannot prove runtime reachability, data-flow ownership, whether a comparison is a guard, or whether a limit is intrinsic versus policy. Broad `if`/`match` capture is a low-confidence fallback intended to expose those gaps.
