#!/usr/bin/env python3
"""Finalize catalog metadata after DOC-MIGRATE-1, then remove temporary tooling."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CATALOG = ROOT / "docs/catalog.yaml"

text = CATALOG.read_text(encoding="utf-8")
old = """  - id: plans.backlog-transitional
    path: docs/planning/BACKLOG.md
    domain: planning
    status: transitional
    audience: [contributor, maintainer, agent]
    owns: [open-work-list]
"""
new = """  - id: plans.backlog
    path: docs/planning/BACKLOG.md
    domain: planning
    status: current
    audience: [contributor, maintainer, agent]
    owns: [open-work-list]
"""
if old not in text:
    raise SystemExit("backlog catalog block not found")
text = text.replace(old, new, 1)

anchor = """  - id: archive.index
    path: docs/archive/README.md
    domain: archive
    status: current
    audience: [contributor, maintainer, agent]
    owns: [archive-policy, historical-migration-routing]
"""
report = """

  - id: archive.migration-report
    path: docs/archive/MIGRATION_REPORT.md
    domain: archive
    status: current
    audience: [contributor, maintainer, agent]
    owns: [historical-migration-report, archive-move-map]
"""
if "id: archive.migration-report" not in text:
    if anchor not in text:
        raise SystemExit("archive index catalog block not found")
    text = text.replace(anchor, anchor + report, 1)

CATALOG.write_text(text, encoding="utf-8")

for relative in (
    "tools/docs/finalize_catalog.py",
    ".github/workflows/documentation-catalog-finalize.yml",
):
    path = ROOT / relative
    if path.exists():
        path.unlink()

print("catalog finalized")
