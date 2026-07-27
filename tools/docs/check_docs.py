#!/usr/bin/env python3
"""Validate Ori's canonical documentation framework using the Python stdlib."""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[2]
CATALOG = ROOT / "docs/catalog.yaml"
ATLAS = ROOT / "docs/ATLAS.md"
CARGO = ROOT / "compiler/Cargo.toml"

RETIRED_NAME = "zeni" + "th"
RETIRED_REPOSITORY = RETIRED_NAME + "lang"

REQUIRED_FILES = [
    "README.md",
    "README.pt-BR.md",
    "README.ja.md",
    "PROJECT_START.md",
    "AGENTS.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "docs/README.md",
    "docs/README.pt-BR.md",
    "docs/ATLAS.md",
    "docs/catalog.yaml",
]

CURRENT_VERSION_DOCS = [
    "README.md",
    "README.pt-BR.md",
    "README.ja.md",
    "PROJECT_START.md",
    "AGENTS.md",
    "docs/README.md",
    "docs/README.pt-BR.md",
    "docs/product/status.md",
    "docs/product/versioning.md",
    "docs/product/support-matrix.md",
    "docs/spec/README.md",
    "docs/spec/01-overview.md",
    "docs/spec/15-stdlib-maintenance.md",
    "docs/spec/18-stability-and-compatibility.md",
]

SKIP_PARTS = {
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    ".ori",
}

MARKDOWN_LINK = re.compile(r"!?(?:\[[^\]]*\])\(([^)]+)\)")


def fail(errors: list[str], message: str) -> None:
    errors.append(message)


def workspace_version() -> str:
    text = CARGO.read_text(encoding="utf-8")
    workspace = text.split("[workspace.package]", 1)
    if len(workspace) != 2:
        raise ValueError("compiler/Cargo.toml has no [workspace.package] section")
    match = re.search(r'^version\s*=\s*"([^"]+)"', workspace[1], re.MULTILINE)
    if not match:
        raise ValueError("workspace package version was not found")
    return match.group(1)


def catalog_project_version(text: str) -> str:
    project = text.split("project:", 1)
    if len(project) != 2:
        raise ValueError("docs/catalog.yaml has no project section")
    match = re.search(r"^\s+version:\s*([^\s#]+)", project[1], re.MULTILINE)
    if not match:
        raise ValueError("catalog project version was not found")
    return match.group(1).strip('"\'')


def canonical_paths(text: str) -> list[str]:
    section = text.split("canonical_documents:", 1)
    if len(section) != 2:
        raise ValueError("docs/catalog.yaml has no canonical_documents section")
    body = section[1].split("normative_roots:", 1)[0]
    return re.findall(r"^\s{4}path:\s*([^\s#]+)", body, re.MULTILINE)


def iter_utf8_files() -> list[Path]:
    files: list[Path] = []
    for path in ROOT.rglob("*"):
        if not path.is_file() or any(part in SKIP_PARTS for part in path.parts):
            continue
        try:
            path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        files.append(path)
    return files


def check_identity(errors: list[str]) -> None:
    patterns = [RETIRED_NAME.casefold(), RETIRED_REPOSITORY.casefold()]
    for path in iter_utf8_files():
        folded = path.read_text(encoding="utf-8").casefold()
        if any(pattern in folded for pattern in patterns):
            fail(errors, f"retired project identity remains in {path.relative_to(ROOT)}")


def normalized_link_target(source: Path, raw_target: str) -> Path | None:
    target = raw_target.strip().split(maxsplit=1)[0].strip("<>")
    if not target or target.startswith(("#", "http://", "https://", "mailto:")):
        return None
    target = unquote(target.split("#", 1)[0].split("?", 1)[0])
    if not target:
        return None
    return (source.parent / target).resolve()


def check_markdown_links(errors: list[str], paths: list[str]) -> None:
    checked = set(paths + REQUIRED_FILES)
    for relative in sorted(checked):
        source = ROOT / relative
        if source.suffix.lower() != ".md" or not source.is_file():
            continue
        text = source.read_text(encoding="utf-8")
        for raw_target in MARKDOWN_LINK.findall(text):
            target = normalized_link_target(source, raw_target)
            if target is None:
                continue
            try:
                target.relative_to(ROOT)
            except ValueError:
                fail(errors, f"link escapes repository in {relative}: {raw_target}")
                continue
            if not target.exists():
                fail(errors, f"broken relative link in {relative}: {raw_target}")


def main() -> int:
    errors: list[str] = []

    for relative in REQUIRED_FILES:
        if not (ROOT / relative).is_file():
            fail(errors, f"required file is missing: {relative}")

    try:
        version = workspace_version()
    except (OSError, ValueError) as exc:
        fail(errors, str(exc))
        version = ""

    try:
        catalog_text = CATALOG.read_text(encoding="utf-8")
        catalog_version = catalog_project_version(catalog_text)
        paths = canonical_paths(catalog_text)
    except (OSError, ValueError) as exc:
        fail(errors, str(exc))
        catalog_version = ""
        paths = []

    if version and catalog_version and version != catalog_version:
        fail(errors, f"workspace version {version} != catalog version {catalog_version}")

    for relative in paths:
        if not (ROOT / relative).is_file():
            fail(errors, f"canonical catalog path is missing: {relative}")

    if version:
        for relative in CURRENT_VERSION_DOCS:
            path = ROOT / relative
            if not path.is_file():
                fail(errors, f"current-version document is missing: {relative}")
                continue
            if version not in path.read_text(encoding="utf-8"):
                fail(errors, f"current-version document does not name {version}: {relative}")

    if ATLAS.is_file() and "catalog.yaml" not in ATLAS.read_text(encoding="utf-8"):
        fail(errors, "ATLAS does not route to docs/catalog.yaml")

    retired_history_root = ROOT / "docs/planning/historico"
    if retired_history_root.exists() and any(retired_history_root.rglob("*.md")):
        fail(errors, "retired historical root contains Markdown files: docs/planning/historico")

    loose_archive = ROOT / "docs/archive"
    allowed_loose = {"README.md", "MIGRATION_REPORT.md"}
    if loose_archive.exists():
        for archive_file in loose_archive.glob("*.md"):
            if archive_file.name not in allowed_loose:
                fail(errors, f"uncategorized archive document: {archive_file.relative_to(ROOT)}")

    check_identity(errors)
    check_markdown_links(errors, paths)

    if errors:
        print("documentation validation failed:")
        for error in errors:
            print(f"- {error}")
        return 1

    print(f"documentation validation passed for Ori {version}")
    print(f"validated {len(paths)} canonical document paths")
    return 0


if __name__ == "__main__":
    sys.exit(main())
