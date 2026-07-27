#!/usr/bin/env python3
"""Generate machine-readable and concise historical documentation inventories."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REPORT = ROOT / ".ai/generated/documentation-history-inventory.json"
SUMMARY = ROOT / ".ai/generated/documentation-history-inventory.tsv"

ARCHIVE_CATEGORY_NAMES = {"plans", "audits", "investigations", "sessions", "legacy"}
SKIP_PARTS = {".git", "target", "node_modules", "dist", "build", ".ori"}

PLANNING_EXCLUSIONS = {
    "docs/planning/README.md",
    "docs/planning/BACKLOG.md",
    "docs/planning/PENDENTES.md",
    "docs/planning/adr-ori-surface-s3-auk9.md",
    "docs/planning/adr-arc-single-cascade-owner.md",
    "docs/planning/adr-arc-cow-collections.md",
    "docs/planning/repo-and-project-layout.md",
    "docs/planning/qa/test-matrix-ori.md",
}


def tracked_text_files() -> list[Path]:
    files: list[Path] = []
    for path in ROOT.rglob("*"):
        if not path.is_file() or any(part in SKIP_PARTS for part in path.parts):
            continue
        try:
            path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        files.append(path)
    return sorted(files)


def candidate_files() -> list[Path]:
    candidates: set[Path] = set()

    planning = ROOT / "docs/planning"
    if planning.exists():
        for path in planning.rglob("*.md"):
            rel = path.relative_to(ROOT).as_posix()
            if rel not in PLANNING_EXCLUSIONS:
                candidates.add(path)

    archive = ROOT / "docs/archive"
    if archive.exists():
        for path in archive.iterdir():
            if path.is_file() and path.name != "README.md":
                candidates.add(path)

    return sorted(candidates)


def title_of(text: str, fallback: str) -> str:
    for line in text.splitlines():
        if line.startswith("# "):
            return line[2:].strip()
    return fallback


def suggested_category(path: Path, title: str, text: str) -> tuple[str, str]:
    haystack = f"{path.name} {title} {text[:3000]}".casefold()

    if any(token in haystack for token in (
        "sessão", "sessao", "session", "resume point", "retomar",
        "máquina", "machine switch", "checkpoint de sessão",
    )):
        return "sessions", "session/resume terminology"

    if any(token in haystack for token in (
        "audit", "auditoria", "gap analysis", "lacunas", "assessment",
        "avaliação", "avaliacao", "parity", "closure report",
        "relatório de fechamento", "relatorio de fechamento",
    )):
        return "audits", "audit/assessment terminology"

    if any(token in haystack for token in (
        "bugcheck", "bug check", "investigation", "investigação",
        "investigacao", "benchmark", "study", "estudo", "análise",
        "analise", "experiment", "experimento", "prototype", "protótipo",
        "prompt", "discussion", "discussão", "discussao", "ideas", "ideias",
    )):
        return "investigations", "investigation/experiment terminology"

    if any(token in haystack for token in (
        "legacy", "retired", "obsolete", "obsoleto", "surface s3", "auk9",
        "old syntax", "sintaxe antiga", "predecessor", "compatibility pointer",
    )):
        return "legacy", "legacy/superseded terminology"

    if any(token in haystack for token in (
        "plan", "plano", "roadmap", "implementation", "implementação",
        "implementacao", "pr-", "milestone", "maturity", "maturidade",
        "migration", "migração", "migracao", "backlog", "freeze",
    )):
        return "plans", "plan/implementation terminology"

    return "investigations", "default historical evidence classification"


def reference_index(files: list[Path], candidates: list[Path]) -> dict[str, list[str]]:
    refs: dict[str, set[str]] = {path.relative_to(ROOT).as_posix(): set() for path in candidates}
    aliases: dict[str, list[str]] = {}
    for path in candidates:
        rel = path.relative_to(ROOT).as_posix()
        aliases[rel] = [rel, path.name]

    for source in files:
        source_rel = source.relative_to(ROOT).as_posix()
        text = source.read_text(encoding="utf-8")
        for target, patterns in aliases.items():
            if source_rel == target:
                continue
            if any(pattern in text for pattern in patterns):
                refs[target].add(source_rel)

    return {key: sorted(value) for key, value in refs.items()}


def main() -> int:
    files = tracked_text_files()
    candidates = candidate_files()
    refs = reference_index(files, candidates)

    records = []
    for path in candidates:
        text = path.read_text(encoding="utf-8")
        rel = path.relative_to(ROOT).as_posix()
        title = title_of(text, path.stem)
        category, reason = suggested_category(path, title, text)
        records.append(
            {
                "path": rel,
                "title": title,
                "lines": len(text.splitlines()),
                "bytes": len(text.encode("utf-8")),
                "sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(),
                "suggested_category": category,
                "classification_reason": reason,
                "inbound_references": refs[rel],
            }
        )

    report = {
        "schema_version": 1,
        "generated_for": "DOC-MIGRATE-1",
        "candidate_count": len(records),
        "excluded_current_paths": sorted(PLANNING_EXCLUSIONS),
        "categories": {
            category: sum(1 for record in records if record["suggested_category"] == category)
            for category in sorted(ARCHIVE_CATEGORY_NAMES)
        },
        "records": records,
    }

    REPORT.parent.mkdir(parents=True, exist_ok=True)
    rendered = json.dumps(report, ensure_ascii=False, indent=2, sort_keys=False) + "\n"
    REPORT.write_text(rendered, encoding="utf-8")

    rows = ["path\tcategory\ttitle\tlines\tinbound_references"]
    for record in records:
        title = record["title"].replace("\t", " ").replace("\n", " ")
        rows.append(
            f'{record["path"]}\t{record["suggested_category"]}\t{title}\t'
            f'{record["lines"]}\t{len(record["inbound_references"])}'
        )
    SUMMARY.write_text("\n".join(rows) + "\n", encoding="utf-8")

    print(f"wrote {len(records)} records")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
