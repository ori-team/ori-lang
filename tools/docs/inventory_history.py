#!/usr/bin/env python3
"""Generate a machine-readable inventory of historical documentation.

This is a migration helper for DOC-MIGRATE-1. It does not modify source
files. The generated report is deterministic and intended for review before
moving files.
"""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REPORT = ROOT / ".ai/generated/documentation-history-inventory.json"

HISTORICAL_ROOTS = [
    ROOT / "docs/planning/historico",
]

ARCHIVE_CATEGORY_NAMES = {"plans", "audits", "investigations", "sessions", "legacy"}

SKIP_PARTS = {".git", "target", "node_modules", "dist", "build", ".ori"}


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

    for root in HISTORICAL_ROOTS:
        if root.exists():
            candidates.update(path for path in root.rglob("*") if path.is_file())

    archive = ROOT / "docs/archive"
    if archive.exists():
        for path in archive.iterdir():
            if not path.is_file() or path.name == "README.md":
                continue
            candidates.add(path)

    planning = ROOT / "docs/planning"
    if planning.exists():
        for path in planning.iterdir():
            if not path.is_file() or path.suffix.lower() != ".md":
                continue
            lower = path.name.casefold()
            if lower in {"readme.md", "backlog.md", "pendentes.md"}:
                continue
            if lower.startswith("adr-"):
                continue
            if any(token in lower for token in (
                "plan", "plano", "audit", "auditoria", "check", "bug",
                "report", "relatorio", "investig", "study", "estudo",
                "analysis", "analise", "prompt", "session", "sessao",
                "resume", "migration", "migracao", "roadmap", "status",
                "complete", "implement", "conclu", "history", "historico",
            )):
                candidates.add(path)

    return sorted(candidates)


def title_of(text: str, fallback: str) -> str:
    for line in text.splitlines():
        if line.startswith("# "):
            return line[2:].strip()
    return fallback


def suggested_category(path: Path, title: str, text: str) -> tuple[str, str]:
    haystack = f"{path.name} {title} {text[:3000]}".casefold()

    if any(token in haystack for token in ("sessão", "sessao", "session", "resume point", "retomar", "máquina", "machine switch")):
        return "sessions", "session/resume terminology"

    if any(token in haystack for token in ("audit", "auditoria", "gap analysis", "lacunas", "assessment", "avaliação", "avaliacao")):
        return "audits", "audit/assessment terminology"

    if any(token in haystack for token in ("bugcheck", "bug check", "investigation", "investigação", "investigacao", "benchmark", "study", "estudo", "análise", "analise", "experiment", "experimento", "prototype", "protótipo", "prompt")):
        return "investigations", "investigation/experiment terminology"

    if any(token in haystack for token in ("legacy", "retired", "obsolete", "obsoleto", "surface s3", "auk9", "old syntax", "sintaxe antiga", "predecessor")):
        return "legacy", "legacy/superseded terminology"

    if any(token in haystack for token in ("plan", "plano", "roadmap", "implementation", "implementação", "implementacao", "pr-", "milestone", "maturity", "maturidade", "migration", "migração", "migracao")):
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
        "categories": {
            category: sum(1 for record in records if record["suggested_category"] == category)
            for category in sorted(ARCHIVE_CATEGORY_NAMES)
        },
        "records": records,
    }

    REPORT.parent.mkdir(parents=True, exist_ok=True)
    rendered = json.dumps(report, ensure_ascii=False, indent=2, sort_keys=False) + "\n"
    if REPORT.exists() and REPORT.read_text(encoding="utf-8") == rendered:
        print(f"inventory unchanged: {REPORT.relative_to(ROOT)}")
        return 0

    REPORT.write_text(rendered, encoding="utf-8")
    print(f"wrote {len(records)} records to {REPORT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
