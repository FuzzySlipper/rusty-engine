#!/usr/bin/env python3
"""Build a review queue of validation and limit candidates from first-party files.

This is a text survey, not a semantic analyzer.  Its broad `if`/`match` pass is
intentional: it makes omissions visible while keeping confidence and hints
separate from a review decision.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import html
import json
import re
import subprocess
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


TOOL_VERSION = "1"
DEFAULT_OUTPUT = "artifacts/validation-audit-7871"
SUPPORTED_SUFFIXES = {
    ".rs": "Rust",
    ".cs": "C#",
    ".ts": "TypeScript",
    ".tsx": "TypeScript",
    ".js": "JavaScript",
    ".mjs": "JavaScript",
    ".cjs": "JavaScript",
    ".py": "Python",
    ".sh": "Shell",
    ".bash": "Shell",
    ".zsh": "Shell",
    ".json": "JSON",
    ".jsonc": "JSON",
    ".yaml": "YAML",
    ".yml": "YAML",
    ".toml": "TOML",
}
LOCK_NAMES = {
    "Cargo.lock",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lockb",
    "composer.lock",
}
EXCLUDED_COMPONENTS = {
    ".git",
    ".nx",
    ".venv",
    "__pycache__",
    "build",
    "coverage",
    "dist",
    "node_modules",
    "obj",
    "out",
    "playwright-report",
    "target",
    "test-results",
    "vendor",
}
GENERATED_COMPONENTS = {"artifacts", "generated", "obj"}
TEST_COMPONENTS = {"test", "tests", "__tests__", "fixture", "fixtures", "__fixtures__"}

VALIDATION_CALL = re.compile(
    r"\b(?:assert(?:_\w+)?|debug_assert|ensure|bail|guard|require|"
    r"validat\w*|verif\w*|ensure\w*|require\w*|check\w*|try_from|TryParse|is_finite|IsFinite|invariant|contract|"
    r"Argument(?:Null|OutOfRange)?Exception|ArgumentException|"
    r"throw|panic!|unreachable!|todo!|not_implemented)\b",
    re.IGNORECASE,
)
ERROR_RETURN = re.compile(
    r"\b(?:return\s+(?:Err|None|false|nil)\b|(?:Err|Result\.Err|Task\.FromException)\s*\(|"
    r"raise\s+|throw\s+|return\s+new\s+Error\b)",
    re.IGNORECASE,
)
ABI_POINTER_GUARD = re.compile(
    r"\b(?:is_null\s*\(|(?:pointer|ptr|bytes|utf8|buffer)\b.*\b(?:null|nil)\b|"
    r"\b(?:null|nil)\b.*\b(?:pointer|ptr|bytes|utf8|buffer)\b)",
    re.IGNORECASE,
)
BROAD_GUARD = re.compile(r"\b(?:if\b|match\b|switch\s*\()")
LIMIT_WORD = re.compile(
    r"(?<![A-Za-z0-9])(?i:max(?:imum)?|min(?:imum)?|limit|budget|quota|capacity|timeout|"
    r"deadline|duration|size|length|count|bytes?|rows?|cols?|vertices|indices|"
    r"items?|entries|pages|depth|width|height|rate|concurrency)(?=$|[A-Z_\W])"
)
LIMIT_COMPARISON = re.compile(r"(?:<=|>=|(?<![=-])<(?![=<])|(?<![!=>])>(?![=>]))")
LIMIT_OPERATION = re.compile(r"\b(?:clamp(?:ed)?|min|max|take|truncate|reserve|with_capacity)\s*(?:\(|\b)", re.IGNORECASE)
SCHEMA_CONSTRAINT = re.compile(
    r"\b(?:min(?:imum)?(?:Length|Items|Properties)?|max(?:imum)?(?:Length|Items|Properties)?|"
    r"pattern|enum|const|multipleOf|exclusiveMinimum|exclusiveMaximum|required)\b",
    re.IGNORECASE,
)
NUMERIC_LITERAL = re.compile(r"\b\d[\d_]*(?:\.\d+)?\b")
INTRINSIC_WORD = re.compile(
    r"(?<![A-Za-z0-9])(?i:null|nil|pointer|ptr|length|len|utf-?8|alignment|overflow|underflow|"
    r"finite|nan|infinity|index|bounds|layout|abi|lifetime|release)(?=$|[A-Z_\W])"
)
POLICY_WORD = re.compile(
    r"(?<![A-Za-z0-9])(?i:budget|quota|timeout|deadline|rate|capacity|concurrency|limit|"
    r"maximum|max(?:imum)?|minimum|min(?:imum)?|message|request|transport)(?=$|[A-Z_\W])"
)


def run_git_paths(root: Path) -> list[str]:
    result = subprocess.run(
        ["git", "-C", str(root), "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        check=True,
        capture_output=True,
    )
    return sorted(path for path in result.stdout.decode("utf-8", "surrogateescape").split("\0") if path)


def language_for(path: Path) -> str | None:
    language = SUPPORTED_SUFFIXES.get(path.suffix.lower())
    if language:
        return language
    if not path.suffix and path.parts and path.parts[0] == "scripts":
        return "Shell"
    return None


def exclusion_reason(relative: Path, output_relative: Path | None) -> str | None:
    parts = set(relative.parts)
    name = relative.name
    if output_relative is not None and (relative == output_relative or output_relative in relative.parents):
        return "audit_output"
    if relative == Path("scripts/inventory-validation.py"):
        return "inventory_tool"
    if name in LOCK_NAMES or name.endswith(".lock"):
        return "lockfile"
    if parts & EXCLUDED_COMPONENTS:
        return "vendor_or_build" if "vendor" in parts else "build_or_dependency"
    if parts & GENERATED_COMPONENTS or name.endswith((".g.cs", ".generated.cs", ".min.js")):
        return "generated_artifact"
    return None


def owner_for(relative: Path) -> str:
    parts = relative.parts
    if len(parts) >= 3 and parts[:2] == ("rust", "crates"):
        return f"crate:{parts[2]}"
    if len(parts) >= 2 and parts[0] == "csharp":
        return f"csharp:{parts[1]}"
    if len(parts) >= 3 and parts[:2] == ("render", "packages"):
        return f"render-package:{parts[2]}"
    if len(parts) >= 3 and parts[:2] == ("studio", "libs"):
        return f"studio-lib:{parts[2]}"
    if len(parts) >= 3 and parts[:2] == ("studio", "apps"):
        return f"studio-app:{parts[2]}"
    return parts[0] if parts else "root"


def path_is_test_or_fixture(relative: Path) -> bool:
    lowered = [part.lower() for part in relative.parts]
    name = relative.name.lower()
    return bool(set(lowered) & TEST_COMPONENTS) or bool(re.search(r"(?:^|[._-])(test|tests|fixture)(?:[._-]|$)", name))


def rust_test_lines(lines: list[str], language: str) -> set[int]:
    """Return line indexes inside ordinary cfg(test) modules and test functions.

    This is deliberately a small brace-based helper, not a Rust parser.  It
    covers the usual inline test layout without treating an entire source file
    as test code merely because it contains one test module.
    """
    if language != "Rust":
        return set()
    marked: set[int] = set()
    pending = False
    for index, line in enumerate(lines):
        if re.search(r"#\[(?:cfg\s*\(\s*test\s*\)|test)\]", line):
            pending = True
            continue
        if not pending or not re.search(r"\b(?:mod\s+\w+|fn\s+\w+)\b", line):
            continue
        depth = 0
        opened = False
        for nested_index in range(index, len(lines)):
            nested = lines[nested_index]
            depth += nested.count("{") - nested.count("}")
            if "{" in nested:
                opened = True
            if opened:
                marked.add(nested_index)
            if opened and depth <= 0:
                break
        pending = False
    return marked


def hint_for(text: str) -> tuple[str, str]:
    numeric = "possible_numeric_limit" if NUMERIC_LITERAL.search(text) else "no_numeric_literal_seen"
    intrinsic = bool(INTRINSIC_WORD.search(text))
    policy = bool(POLICY_WORD.search(text))
    if intrinsic and policy:
        scope = "possible_intrinsic_and_policy"
    elif intrinsic:
        scope = "possible_representation_lifetime_or_math_invariant"
    elif policy:
        scope = "possible_policy_or_budget"
    else:
        scope = "ambiguous"
    return numeric, scope


def context_for(lines: list[str], line_index: int) -> str:
    start = max(0, line_index - 2)
    end = min(len(lines), line_index + 3)
    return "\n".join(f"{number + 1}: {lines[number]}" for number in range(start, end))


def candidate_signals(line: str, language: str, schema_source: bool = False) -> list[tuple[str, str]]:
    signals: list[tuple[str, str]] = []
    if schema_source and re.search(r"\b(?:record|list|integer|finite|range|enumeration|vec[234])\s*\(", line):
        signals.append(("validation", "possible_schema_helper"))
    validation = VALIDATION_CALL.search(line)
    if validation:
        signals.append(("validation", "validation_call_or_assert"))
    if ERROR_RETURN.search(line):
        signals.append(("validation", "error_return_or_throw"))
    if ABI_POINTER_GUARD.search(line):
        signals.append(("validation", "abi_pointer_or_length_guard"))
    if BROAD_GUARD.search(line):
        signals.append(("broad_guard", "all_if_or_match_guard"))
    has_limit_word = bool(LIMIT_WORD.search(line))
    if has_limit_word and LIMIT_COMPARISON.search(line):
        signals.append(("limit", "named_limit_comparison"))
    if LIMIT_OPERATION.search(line) and (has_limit_word or NUMERIC_LITERAL.search(line)):
        signals.append(("limit", "clamp_min_max_take_or_truncate"))
    if language in {"JSON", "YAML", "TOML"} and SCHEMA_CONSTRAINT.search(line):
        signals.append(("limit", "declarative_schema_constraint"))
    return signals


def scan_file(root: Path, relative: Path, language: str) -> tuple[list[dict[str, Any]], str | None]:
    try:
        text = (root / relative).read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return [], "non_utf8"
    except OSError as error:
        return [], f"read_error:{error.__class__.__name__}"
    lines = text.splitlines()
    path_test = path_is_test_or_fixture(relative)
    rust_tests = rust_test_lines(lines, language)
    records: list[dict[str, Any]] = []
    for index, line in enumerate(lines):
        signals = candidate_signals(line, language, bool(re.search(r"validat|schema", str(relative), re.IGNORECASE)))
        if not signals:
            continue
        categories = sorted({category for category, _ in signals})
        detector_signals = sorted({signal for _, signal in signals})
        is_test = path_test or index in rust_tests
        numeric_hint, scope_hint = hint_for(line)
        stable_source = f"{relative.as_posix()}:{index + 1}:{','.join(detector_signals)}:{line.strip()}"
        records.append(
            {
                "id": hashlib.sha256(stable_source.encode("utf-8")).hexdigest()[:20],
                "path": relative.as_posix(),
                "line": index + 1,
                "language": language,
                "owner_module": owner_for(relative),
                "categories": categories,
                "detector_signals": detector_signals,
                "candidate_scope": "test_or_fixture" if is_test else "production_or_config",
                "is_test_or_fixture": is_test,
                "expression": line.strip(),
                "context": context_for(lines, index),
                "numeric_hint": numeric_hint,
                "scope_hint": scope_hint,
                "hint_note": "Heuristic hint only; it is not a validity, ownership, or removal verdict.",
                "review_status": "unreviewed",
            }
        )
    return records, None


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, records: list[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8", newline="") as stream:
        for record in records:
            stream.write(json.dumps(record, sort_keys=True) + "\n")


def write_limits_tsv(path: Path, records: list[dict[str, Any]]) -> None:
    fields = ["id", "path", "line", "language", "owner_module", "candidate_scope", "categories", "detector_signals", "numeric_hint", "scope_hint", "expression", "review_status"]
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, delimiter="\t", extrasaction="ignore")
        writer.writeheader()
        for record in records:
            if "limit" not in record["categories"]:
                continue
            row = dict(record)
            row["categories"] = ",".join(record["categories"])
            row["detector_signals"] = ",".join(record["detector_signals"])
            writer.writerow(row)


def html_report(records: list[dict[str, Any]], summary: dict[str, Any], root: Path) -> str:
    data = json.dumps(records, separators=(",", ":")).replace("<", "\\u003c")
    summary_json = html.escape(json.dumps(summary["candidate_counts"], sort_keys=True))
    root_url = root.as_uri()
    return f"""<!doctype html>
<html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
<title>Validation inventory</title>
<style>
body{{font:14px system-ui,sans-serif;margin:1.25rem;color:#18212b;background:#f8fafc}} header{{max-width:1400px}} .controls{{display:flex;flex-wrap:wrap;gap:.7rem;align-items:center;margin:1rem 0}} input[type=search]{{min-width:20rem;padding:.45rem}} button{{padding:.4rem .7rem}} table{{border-collapse:collapse;width:100%;background:white}} th,td{{border:1px solid #d9e0e7;padding:.45rem;vertical-align:top;text-align:left}} th{{position:sticky;top:0;background:#eaf0f6}} code,pre{{white-space:pre-wrap;font:12px ui-monospace,monospace}} .test{{opacity:.65}} .tags{{color:#475569}} .count{{font-weight:600}} a{{color:#075985}} small{{color:#52616b}}</style>
<header><h1>Validation and limit candidate inventory</h1><p>Heuristic survey for one-by-one review. It deliberately includes broad <code>if</code>/<code>match</code> candidates and does not classify a candidate as valid, invalid, intrinsic, or policy.</p><p class=\"count\">{len(records):,} candidates · {summary_json}</p></header>
<section class=\"controls\"><label><input id=\"limits\" type=\"checkbox\" checked> limits</label><label><input id=\"validation\" type=\"checkbox\" checked> validation</label><label><input id=\"guards\" type=\"checkbox\" checked> broad guards</label><label><input id=\"tests\" type=\"checkbox\" checked> tests/fixtures</label><input id=\"query\" type=\"search\" placeholder=\"filter path, owner, expression, signal\"><button id=\"copy\">Copy filtered JSON</button><button id=\"prev\">Previous</button><button id=\"next\">Next</button><span id=\"shown\"></span></section>
<table><thead><tr><th>source</th><th>categories/signals</th><th>owner/scope/hints</th><th>expression and context</th><th>copy</th></tr></thead><tbody id=\"rows\"></tbody></table>
<script id=\"records\" type=\"application/json\">{data}</script><script>
const records=JSON.parse(document.getElementById('records').textContent), rows=document.getElementById('rows'), shown=document.getElementById('shown');
const esc=s=>String(s).replace(/[&<>\"]/g,c=>({{'&':'&amp;','<':'&lt;','>':'&gt;','\\"':'&quot;'}}[c]));
let page=0; const pageSize=100; const searchText=new Map(records.map(r=>[r.id,JSON.stringify(r).toLowerCase()]));
function allowed(r){{const q=document.getElementById('query').value.toLowerCase(); const wants=(id,cat)=>document.getElementById(id).checked&&r.categories.includes(cat); if(!wants('limits','limit')&&!wants('validation','validation')&&!wants('guards','broad_guard')) return false; if(!document.getElementById('tests').checked&&r.is_test_or_fixture)return false; return !q||searchText.get(r.id).includes(q)}}
function render(){{const filtered=records.filter(allowed); page=Math.min(page,Math.max(0,Math.ceil(filtered.length/pageSize)-1)); shown.textContent=`${{filtered.length.toLocaleString()}} matches · page ${{page+1}} / ${{Math.max(1,Math.ceil(filtered.length/pageSize))}}`; document.getElementById('prev').disabled=page===0; document.getElementById('next').disabled=(page+1)*pageSize>=filtered.length; rows.innerHTML=filtered.slice(page*pageSize,(page+1)*pageSize).map(r=>{{const source=`{root_url}/${{r.path}}#L${{r.line}}`; return `<tr class="${{r.is_test_or_fixture?'test':''}}"><td><a href="${{esc(source)}}">${{esc(r.path)}}:${{r.line}}</a><br><small>${{esc(r.id)}}</small></td><td>${{esc(r.categories.join(', '))}}<br><span class="tags">${{esc(r.detector_signals.join(', '))}}</span></td><td>${{esc(r.owner_module)}}<br>${{esc(r.candidate_scope)}}<br><small>${{esc(r.numeric_hint)}}; ${{esc(r.scope_hint)}}</small></td><td><code>${{esc(r.expression)}}</code><details><summary>context</summary><pre>${{esc(r.context)}}</pre></details></td><td><button data-id="${{r.id}}">Copy</button></td></tr>`}}).join(''); document.querySelectorAll('[data-id]').forEach(button=>button.onclick=()=>navigator.clipboard.writeText(JSON.stringify(records.find(r=>r.id===button.dataset.id),null,2))); return filtered}}
document.querySelectorAll('input').forEach(input=>input.addEventListener('input',()=>{{page=0;render()}})); document.getElementById('prev').onclick=()=>{{page--;render()}}; document.getElementById('next').onclick=()=>{{page++;render()}}; document.getElementById('copy').onclick=()=>navigator.clipboard.writeText(JSON.stringify(records.filter(allowed),null,2)); render();
</script></html>"""


def markdown_summary(summary: dict[str, Any]) -> str:
    excluded = summary["excluded_counts"]
    counts = summary["candidate_counts"]
    lines = [
        "# Validation and limit candidate inventory",
        "",
        "This is a heuristic review queue, not a semantic-completeness claim or a validity verdict.",
        "",
        f"Generated: `{summary['generated_at']}`", 
        f"Discovery: `{summary['discovery_command']}`", 
        "",
        "## Coverage",
        "",
        f"- Discovered paths: {summary['discovered_paths']}",
        f"- Supported first-party source/config files scanned: {summary['scanned_files']}",
        f"- Candidates: {summary['record_count']} ({counts.get('limit', 0)} limit, {counts.get('validation', 0)} validation, {counts.get('broad_guard', 0)} broad guard)",
        f"- Test/fixture candidates: {summary['test_or_fixture_candidates']}",
        "",
        "## Exclusions",
        "",
    ]
    lines.extend(f"- {reason}: {count}" for reason, count in sorted(excluded.items()))
    lines.extend([
        "",
        "Generated files:",
        "",
        "- `inventory.jsonl` is the full raw candidate inventory.",
        "- `limits.tsv` is the limit-only spreadsheet-friendly subset.",
        "- `index.html` is a standalone filterable review view with source and copy links.",
        "- `summary.json` contains machine-readable coverage and exclusion counts.",
        "",
        "Candidates carry `review_status=unreviewed`. Keep human dispositions in a separate annotation file or system; rerunning this script replaces only its raw generated outputs.",
        "",
        "Known survey limits: text patterns cannot prove runtime reachability, data-flow ownership, whether a comparison is a guard, or whether a limit is intrinsic versus policy. Broad `if`/`match` capture is a low-confidence fallback intended to expose those gaps.",
    ])
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--output", type=Path, default=Path(DEFAULT_OUTPUT))
    args = parser.parse_args()
    root = args.root.resolve()
    output = (root / args.output).resolve() if not args.output.is_absolute() else args.output.resolve()
    try:
        output_relative = output.relative_to(root)
    except ValueError:
        output_relative = None
    output.mkdir(parents=True, exist_ok=True)
    exclusions: Counter[str] = Counter()
    excluded_paths = []
    scanned_files = 0
    records: list[dict[str, Any]] = []
    source_paths = run_git_paths(root)
    for raw_path in source_paths:
        relative = Path(raw_path)
        reason = exclusion_reason(relative, output_relative)
        if reason:
            excluded_paths.append({"path": raw_path, "reason": reason})
            exclusions[reason] += 1
            continue
        language = language_for(relative)
        if language is None:
            excluded_paths.append({"path": raw_path, "reason": "unsupported_file_type"})
            exclusions["unsupported_file_type"] += 1
            continue
        file_records, read_issue = scan_file(root, relative, language)
        if read_issue:
            excluded_paths.append({"path": raw_path, "reason": read_issue})
            exclusions[read_issue] += 1
            continue
        scanned_files += 1
        records.extend(file_records)
    records.sort(key=lambda record: (record["path"], record["line"], record["id"]))
    category_counts = Counter(category for record in records for category in record["categories"])
    language_counts = Counter(record["language"] for record in records)
    summary = {
        "tool": "scripts/inventory-validation.py",
        "tool_version": TOOL_VERSION,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "root": str(root),
        "output": str(output),
        "discovery_command": "git ls-files --cached --others --exclude-standard -z",
        "discovered_paths": len(source_paths),
        "scanned_files": scanned_files,
        "record_count": len(records),
        "candidate_counts": dict(sorted(category_counts.items())),
        "candidate_language_counts": dict(sorted(language_counts.items())),
        "test_or_fixture_candidates": sum(record["is_test_or_fixture"] for record in records),
        "excluded_counts": dict(sorted(exclusions.items())),
        "notes": [
            "Raw candidates are heuristics; review status is intentionally unreviewed.",
            "Broad if/match guards are low-confidence fallback coverage, not proof of validation.",
            "Hint fields are not ownership, validity, or removal verdicts.",
        ],
    }
    write_jsonl(output / "excluded-paths.jsonl", excluded_paths)
    write_jsonl(output / "inventory.jsonl", records)
    write_limits_tsv(output / "limits.tsv", records)
    write_json(output / "summary.json", summary)
    (output / "summary.md").write_text(markdown_summary(summary), encoding="utf-8")
    (output / "index.html").write_text(html_report(records, summary, root), encoding="utf-8")
    print(f"wrote {len(records)} candidates from {scanned_files} files to {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
