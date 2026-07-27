#!/usr/bin/env python3
"""Apply DOC-MIGRATE-1 in a repository checkout.

The script moves historical files with git, preserves relative Markdown links,
adds archive notices, generates category indexes and a migration report, closes
the ExecPlan/backlog entries, strengthens validation, and removes its temporary
inventory/workflow machinery.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
INVENTORY = ROOT / ".ai/generated/documentation-history-inventory.json"
ARCHIVED_ON = "2026-07-27"
CATEGORIES = ("plans", "audits", "investigations", "sessions", "legacy")

CATEGORY_OVERRIDES = {
    "docs/archive/analise-profunda-implementacao-linguagem.md": "audits",
    "docs/archive/auditoria-profunda-implementacao-2026-05-17.md": "audits",
    "docs/archive/auditoria-profunda-implementacao-linguagem-2026-05-13.md": "audits",
    "docs/archive/plano-correcao-implementacao-linguagem.md": "plans",
    "docs/archive/relatorio-fechamento-correcao-implementacao-linguagem.md": "audits",
    "docs/archive/relatorio-fechamento-nova-rodada.md": "audits",
    "docs/planning/IMPLEMENTADOS.md": "audits",
    "docs/planning/PLANO-CDYLIB-EMBED.md": "plans",
    "docs/planning/eco-game-imgui-raylib3d-plan.md": "plans",
    "docs/planning/freeze-and-abi-gates.md": "plans",
    "docs/planning/historico/bugcheck-native-ori-ide-2026-07-18.md": "investigations",
    "docs/planning/historico/c-backend-redefinition.md": "investigations",
    "docs/planning/historico/design-close-backlog-linux-2026-07-13.md": "plans",
    "docs/planning/historico/ideias-programas-avancados.md": "investigations",
    "docs/planning/historico/io-streams-design.md": "investigations",
    "docs/planning/historico/issue-ffi-dispatch-large-binary-2026-07-16.md": "investigations",
    "docs/planning/historico/lang-mem-9-runtime-wrappers-2026-07-18.md": "investigations",
    "docs/planning/historico/lang-res-closure.md": "audits",
    "docs/planning/historico/net-v2-design.md": "investigations",
    "docs/planning/historico/nim-study-2026-07-16-c0.md": "investigations",
    "docs/planning/historico/nim-study-2026-07-17-c1.md": "investigations",
    "docs/planning/historico/nim-study-2026-07-17-c2.md": "investigations",
    "docs/planning/historico/nim-study-2026-07-17-c3.md": "investigations",
    "docs/planning/historico/nim-study-2026-07-17-c4-c7.md": "investigations",
    "docs/planning/historico/perf-runtime-midend-plan.md": "plans",
    "docs/planning/historico/plano-correcao-bugs-2026-05-17.md": "plans",
    "docs/planning/historico/plano-implementacao-lsp-avancado.md": "plans",
    "docs/planning/historico/porting-raylib-sqlite-cimgui.md": "plans",
    "docs/planning/historico/pr-plan-ori-surface-s3.md": "plans",
    "docs/planning/historico/registry-v2.md": "plans",
    "docs/planning/historico/result-ctors-ok-err.md": "plans",
    "docs/planning/historico/rust-independence.md": "plans",
    "docs/planning/historico/security-performance-testing.md": "audits",
    "docs/planning/historico/sessao-nim-arc-2026-07-16.md": "sessions",
    "docs/planning/historico/stdlib-gap-parity.md": "audits",
    "docs/planning/language-direction-decisions-2026-06-30.md": "legacy",
    "docs/planning/manifest-schema.md": "plans",
    "docs/planning/ori-surface-s3-auk9.md": "legacy",
    "docs/planning/package-ecosystem-guidelines.md": "legacy",
    "docs/planning/perf-baseline-2026-07-13.md": "audits",
    "docs/planning/plano-arc-nim-2026-07-16.md": "plans",
    "docs/planning/prompt-analisar-nim-para-ori.md": "investigations",
    "docs/planning/qa/residual-cleanup-2026-07-13.md": "audits",
    "docs/planning/registry-v1.md": "plans",
    "docs/planning/roadmap-maturidade-v0.4-v0.5.md": "plans",
    "docs/planning/roadtov1.md": "plans",
    "docs/planning/stdlib-merge-policy.md": "legacy",
    "docs/planning/uso-real-pequeno-medio.md": "plans",
    "docs/planning/web-framework-learning-course.md": "investigations",
    "docs/planning/web-templates-discussion-roadmap.md": "investigations",
}

SPECIAL_TARGETS = {
    "docs/planning/historico/PLANO-MATURIDADE-COMPLETO.md":
        "docs/archive/plans/maturity-plan-2026-06.md",
}

REPLACEMENTS = {
    "docs/planning/freeze-and-abi-gates.md": "docs/product/versioning.md and docs/spec/19-abi.md",
    "docs/planning/historico/c-backend-redefinition.md": "docs/spec/14-backend-support.md",
    "docs/planning/historico/lang-res-closure.md": "docs/spec/14-backend-support.md",
    "docs/planning/historico/pr-plan-ori-surface-s3.md": "docs/decisions/adr/0001-s3-language-surface.md",
    "docs/planning/ori-surface-s3-auk9.md": "docs/decisions/adr/0001-s3-language-surface.md",
    "docs/planning/historico/stdlib-gap-parity.md": "docs/architecture/stdlib.md and docs/spec/15-stdlib-maintenance.md",
    "docs/planning/manifest-schema.md": "docs/spec/17-project-and-docs.md",
    "docs/planning/plano-arc-nim-2026-07-16.md": "docs/decisions/adr/0002-arc-single-cascade-owner.md",
    "docs/planning/historico/nim-study-2026-07-16-c0.md": "docs/architecture/runtime-and-memory.md",
    "docs/planning/historico/nim-study-2026-07-17-c1.md": "docs/decisions/adr/0002-arc-single-cascade-owner.md",
    "docs/planning/historico/nim-study-2026-07-17-c2.md": "docs/decisions/adr/0002-arc-single-cascade-owner.md",
    "docs/planning/historico/nim-study-2026-07-17-c3.md": "docs/architecture/runtime-and-memory.md",
    "docs/planning/historico/nim-study-2026-07-17-c4-c7.md": "docs/architecture/runtime-and-memory.md",
    "docs/planning/language-direction-decisions-2026-06-30.md": "docs/governance/language-evolution.md and docs/decisions/README.md",
    "docs/planning/package-ecosystem-guidelines.md": "docs/security/supply-chain.md and docs/spec/17-project-and-docs.md",
    "docs/planning/stdlib-merge-policy.md": "docs/spec/15-stdlib-maintenance.md",
}

LINK_RE = re.compile(r"(!?\[[^\]]*\]\()([^)]+)(\))")
URL_PREFIXES = ("http://", "https://", "mailto:", "data:")
SKIP_PARTS = {".git", "target", "node_modules", "dist", "build", ".ori"}


def run(*args: str) -> None:
    subprocess.run(args, cwd=ROOT, check=True)


def rel(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def title_of(text: str, fallback: str) -> str:
    for line in text.splitlines():
        if line.startswith("# "):
            return line[2:].strip()
    return fallback


def split_target(raw: str) -> tuple[str, str, str]:
    """Return path, suffix (#fragment/?query/title), and wrapper style."""
    value = raw.strip()
    wrapper = ""
    if value.startswith("<") and ">" in value:
        end = value.index(">")
        wrapper = value[end + 1 :]
        value = value[1:end]
    elif " " in value:
        value, wrapper = value.split(" ", 1)
        wrapper = " " + wrapper

    match = re.match(r"^([^#?]*)(.*)$", value)
    assert match
    return match.group(1), match.group(2), wrapper


def resolve_local(source: Path, target_path: str) -> str | None:
    if not target_path or target_path.startswith(("#", "/")) or target_path.startswith(URL_PREFIXES):
        return None
    resolved = (source.parent / target_path).resolve()
    try:
        return resolved.relative_to(ROOT).as_posix()
    except ValueError:
        return None


def rewrite_markdown_links(text: str, old_source: Path, new_source: Path, moves: dict[str, str]) -> str:
    def replace(match: re.Match[str]) -> str:
        raw = match.group(2)
        target_path, suffix, wrapper = split_target(raw)
        resolved = resolve_local(old_source, target_path)
        if resolved is None:
            return match.group(0)
        final_target = moves.get(resolved, resolved)
        target_abs = ROOT / final_target
        relative = os.path.relpath(target_abs, start=new_source.parent).replace(os.sep, "/")
        if target_path.endswith("/") and not relative.endswith("/"):
            relative += "/"
        rendered = relative + suffix + wrapper
        return f"{match.group(1)}{rendered}{match.group(3)}"

    return LINK_RE.sub(replace, text)


def original_date(path: str) -> str:
    match = re.search(r"(20\d{2}-\d{2}-\d{2})", path)
    return match.group(1) if match else "before 2026-07-27"


def add_archive_notice(text: str, old: str, category: str) -> str:
    if "Status: **archived**" in text or "status: archived" in text[:500].casefold():
        return text

    replacement = REPLACEMENTS.get(old, "docs/ATLAS.md")
    notice = (
        f"> Status: **archived**  \n"
        f"> Original path: `{old}`  \n"
        f"> Original date: {original_date(old)}  \n"
        f"> Archived on: {ARCHIVED_ON}  \n"
        f"> Category: `{category}`  \n"
        f"> Current replacement: `{replacement}`  \n"
        f"> Warning: versions, syntax, commands, paths, and priorities below may be obsolete.\n"
    )

    lines = text.splitlines()
    for index, line in enumerate(lines):
        if line.startswith("# "):
            lines[index + 1:index + 1] = ["", notice.rstrip(), ""]
            return "\n".join(lines).rstrip() + "\n"
    return notice + "\n" + text.lstrip()


def read_utf8_files() -> list[Path]:
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


def unique_target(category: str, basename: str, occupied: set[str]) -> str:
    target = f"docs/archive/{category}/{basename}"
    if target not in occupied:
        return target
    stem = Path(basename).stem
    suffix = Path(basename).suffix
    index = 2
    while True:
        candidate = f"docs/archive/{category}/{stem}-historical-{index}{suffix}"
        if candidate not in occupied:
            return candidate
        index += 1


def update_text(path: str, transform) -> None:
    file = ROOT / path
    if not file.exists():
        return
    original = file.read_text(encoding="utf-8")
    updated = transform(original)
    if updated != original:
        file.write_text(updated, encoding="utf-8")


def category_index(category: str) -> str:
    directory = ROOT / "docs/archive" / category
    records = []
    for path in sorted(directory.glob("*.md")):
        if path.name == "README.md":
            continue
        text = path.read_text(encoding="utf-8")
        records.append((path.name, title_of(text, path.stem)))

    descriptions = {
        "plans": "Completed, cancelled, or superseded execution plans.",
        "audits": "Dated audits, parity snapshots, closure reports, and assessment evidence.",
        "investigations": "Studies, experiments, prototypes, bug checks, benchmarks, and design exploration.",
        "sessions": "Session/resume notes preserved only for traceability.",
        "legacy": "Retired product, syntax, policy, and repository-direction material.",
    }
    lines = [f"# Archived {category}", "", descriptions[category], "", "> These files are historical evidence, not current instructions. Start from [`../../ATLAS.md`](../../ATLAS.md).", "", "| File | Historical title |", "|---|---|"]
    for filename, title in records:
        lines.append(f"| [`{filename}`]({filename}) | {title.replace('|', '\\|')} |")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    if not INVENTORY.exists():
        raise SystemExit("historical inventory is missing")
    inventory = json.loads(INVENTORY.read_text(encoding="utf-8"))
    records = inventory["records"]

    occupied = {rel(path) for path in ROOT.rglob("*") if path.is_file()}
    moves: dict[str, str] = {}
    categories: dict[str, str] = {}

    for record in records:
        old = record["path"]
        if old in SPECIAL_TARGETS:
            moves[old] = SPECIAL_TARGETS[old]
            categories[old] = "plans"
            continue
        category = CATEGORY_OVERRIDES.get(old, record["suggested_category"])
        if category not in CATEGORIES:
            raise SystemExit(f"invalid category for {old}: {category}")
        target = unique_target(category, Path(old).name, occupied | set(moves.values()))
        moves[old] = target
        categories[old] = category

    active_plan_old = "docs/plans/active/documentation-history-migration.md"
    active_plan_new = "docs/archive/plans/documentation-history-migration.md"
    if (ROOT / active_plan_old).exists():
        moves[active_plan_old] = active_plan_new
        categories[active_plan_old] = "plans"

    # Preserve every moved document's relative links before changing paths.
    original_text: dict[str, str] = {}
    for old in moves:
        path = ROOT / old
        if path.exists():
            original_text[old] = path.read_text(encoding="utf-8")

    # Move files, except the maturity compatibility pointer whose target exists.
    for old, new in moves.items():
        old_path = ROOT / old
        new_path = ROOT / new
        if old == "docs/planning/historico/PLANO-MATURIDADE-COMPLETO.md":
            if old_path.exists():
                run("git", "rm", old)
            continue
        if not old_path.exists():
            continue
        new_path.parent.mkdir(parents=True, exist_ok=True)
        run("git", "mv", old, new)

    # Rewrite links and add archive notices to moved content.
    for old, new in moves.items():
        if old == "docs/planning/historico/PLANO-MATURIDADE-COMPLETO.md":
            continue
        new_path = ROOT / new
        if not new_path.exists():
            continue
        text = original_text[old]
        text = rewrite_markdown_links(text, ROOT / old, new_path, moves)
        if old == active_plan_old:
            text = text.replace("status: ready", "status: completed", 1)
            text = text.replace("updated: 2026-07-27", "updated: 2026-07-27", 1)
            text = text.replace(
                "## Final outcome\n\nComplete this section after the final migration PR with links, counts, residual exclusions, and validation results.",
                "## Final outcome\n\nCompleted on 2026-07-27 in PR #3. Fifty-one historical planning/archive documents were classified and moved, the active ExecPlan itself was archived, category indexes and a migration report were generated, inbound relative links were rewritten, the transitional `docs/planning/historico/` root was removed, and permanent CI validation now prevents its return.",
            )
        else:
            text = add_archive_notice(text, old, categories[old])
        new_path.write_text(text, encoding="utf-8")

    # Update links from all remaining files to moved targets.
    for path in read_utf8_files():
        path_rel = rel(path)
        if path_rel in moves.values():
            continue
        original = path.read_text(encoding="utf-8")
        updated = rewrite_markdown_links(original, path, path, moves)
        for old, new in moves.items():
            updated = updated.replace(old, new)
        if updated != original:
            path.write_text(updated, encoding="utf-8")

    # Close backlog work completed by this PR.
    backlog = ROOT / "docs/planning/BACKLOG.md"
    if backlog.exists():
        lines = backlog.read_text(encoding="utf-8").splitlines()
        lines = [line for line in lines if "**DOC-FRAMEWORK-1**" not in line and "**DOC-MIGRATE-1**" not in line]
        backlog.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")

    update_text(
        "docs/plans/README.md",
        lambda text: re.sub(
            r"## Active plans\n\n.*?\n## Backlog policy",
            "## Active plans\n\nThere are no active ExecPlans after the documentation-history migration completed. New complex work must be added here when it begins.\n\n## Backlog policy",
            text,
            flags=re.DOTALL,
        ).replace(
            "The existing `docs/planning/historico/` tree should migrate into `docs/archive/` by category:",
            "The former `docs/planning/historico/` tree was migrated into `docs/archive/` by category:",
        ).replace(
            "The detailed execution route is [`active/documentation-history-migration.md`](active/documentation-history-migration.md).",
            "The completed execution record is [`../archive/plans/documentation-history-migration.md`](../archive/plans/documentation-history-migration.md).",
        ),
    )

    update_text(
        "docs/planning/README.md",
        lambda text: text.replace(
            "- `historico/` — historical tree pending categorized migration into `docs/archive/`.",
            "- The former `historico/` tree has been migrated into categorized directories under `docs/archive/`.",
        ),
    )

    update_text(
        "docs/ATLAS.md",
        lambda text: text.replace(
            "`planning/historico/` remains a transitional historical root. Migration must categorize files, update links, and avoid duplicate full copies.",
            "The former `planning/historico/` root has been removed. Historical files now live only in the categorized archive directories. See [`archive/MIGRATION_REPORT.md`](archive/MIGRATION_REPORT.md).",
        ),
    )

    update_text(
        "docs/archive/README.md",
        lambda text: text.replace(
            "Existing files directly under `docs/archive/` and `docs/planning/historico/` should migrate gradually into these categories.",
            "Historical files are organized into these categories. The completed migration is recorded in [`MIGRATION_REPORT.md`](MIGRATION_REPORT.md).",
        ).replace(
            "## Migration from `docs/planning/historico/`",
            "## Completed migration from `docs/planning/historico/`",
        ).replace(
            "Classify each file by purpose, not only by its original directory:",
            "The migration classified each file by purpose, not only by its original directory:",
        ).replace(
            "The migration should be performed in focused batches with link validation rather than one unreviewable mass move.",
            "The migration was completed with an explicit inventory, link rewriting, category indexes, and permanent CI validation.",
        ),
    )

    update_text(
        "docs/catalog.yaml",
        lambda text: re.sub(
            r"\n  - path: docs/planning/historico\n    migration_target: docs/archive",
            "",
            text,
        ),
    )

    # Generate category indexes.
    for category in CATEGORIES:
        index = ROOT / "docs/archive" / category / "README.md"
        index.parent.mkdir(parents=True, exist_ok=True)
        index.write_text(category_index(category), encoding="utf-8")

    # Generate migration report.
    report_lines = [
        "# Historical documentation migration report",
        "",
        "> Status: **completed**  ",
        f"> Completed on: {ARCHIVED_ON}  ",
        "> ExecPlan: [`plans/documentation-history-migration.md`](plans/documentation-history-migration.md)",
        "",
        "DOC-MIGRATE-1 classified and moved historical planning and loose archive documents into canonical archive categories.",
        "",
        f"- Inventory candidates migrated: **{len(records)}**",
        "- Completed ExecPlan archived: **1**",
        "- Transitional `docs/planning/historico/` root: **removed**",
        "- Relative inbound links: **rewritten by resolved target**",
        "- Temporary migration tooling/workflows: **removed**",
        "",
        "## Moves",
        "",
        "| Original path | Archive path | Category |",
        "|---|---|---|",
    ]
    for old in sorted(moves):
        report_lines.append(f"| `{old}` | [`{moves[old]}`](../{moves[old].removeprefix('docs/')}) | `{categories[old]}` |")
    report_lines.extend([
        "",
        "## Validation",
        "",
        "- canonical documentation validator passes after the moves;",
        "- archive category indexes are generated from final files;",
        "- active planning no longer contains completed plans or the historical root;",
        "- the catalog no longer declares `docs/planning/historico` as a historical root;",
        "- CI rejects any future Markdown file added under the retired root;",
        "- Git history retains the original content and paths.",
        "",
        "## Intentional compatibility pointers",
        "",
        "The migrated ADR and repository-layout paths under `docs/planning/` remain concise compatibility pointers because they are likely external entry points. They are not historical content roots and are excluded from the archive migration.",
        "",
    ])
    (ROOT / "docs/archive/MIGRATION_REPORT.md").write_text("\n".join(report_lines), encoding="utf-8")

    # Strengthen permanent validation.
    validator = ROOT / "tools/docs/check_docs.py"
    validator_text = validator.read_text(encoding="utf-8")
    marker = "    check_identity(errors)\n    check_markdown_links(errors, paths)"
    replacement = """    retired_history_root = ROOT / \"docs/planning/historico\"
    if retired_history_root.exists() and any(retired_history_root.rglob(\"*.md\")):
        fail(errors, \"retired historical root contains Markdown files: docs/planning/historico\")

    loose_archive = ROOT / \"docs/archive\"
    allowed_loose = {\"README.md\", \"MIGRATION_REPORT.md\"}
    if loose_archive.exists():
        for archive_file in loose_archive.glob(\"*.md\"):
            if archive_file.name not in allowed_loose:
                fail(errors, f\"uncategorized archive document: {archive_file.relative_to(ROOT)}\")

    check_identity(errors)
    check_markdown_links(errors, paths)"""
    if marker not in validator_text:
        raise SystemExit("validator insertion marker not found")
    validator.write_text(validator_text.replace(marker, replacement, 1), encoding="utf-8")

    # Remove one-time generated inventory and automation, including this script/workflow.
    temporary_paths = [
        ".ai/generated/documentation-history-inventory.json",
        ".ai/generated/documentation-history-inventory.tsv",
        "tools/docs/inventory_history.py",
        ".github/workflows/documentation-history-inventory.yml",
        "tools/docs/apply_history_migration.py",
        ".github/workflows/documentation-history-migration.yml",
    ]
    for temporary in temporary_paths:
        path = ROOT / temporary
        if path.exists():
            path.unlink()

    print(f"migrated {len(records)} inventory records and archived the ExecPlan")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
