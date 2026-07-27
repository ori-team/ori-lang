#!/usr/bin/env python3
"""Remove retired project-identity terms from UTF-8 repository text files.

This is a one-time migration helper. It preserves all unrelated content and is
idempotent. Remove it after the documentation migration is complete.
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RETIRED_NAME = "zeni" + "th"
RETIRED_REPOSITORY = RETIRED_NAME + "lang"

SKIP_PARTS = {
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    ".ori",
}


def iter_text_files() -> list[Path]:
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


def migrate(text: str) -> str:
    # Replace the repository identifier first so URLs and paths remain valid.
    text = re.sub(
        re.escape(RETIRED_REPOSITORY),
        "ori-lang",
        text,
        flags=re.IGNORECASE,
    )
    # Preserve historical meaning without retaining the retired proper name.
    text = re.sub(
        rf"\b{re.escape(RETIRED_NAME)}\b",
        "retired predecessor project",
        text,
        flags=re.IGNORECASE,
    )
    return text


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="report files that still require migration without writing",
    )
    args = parser.parse_args()

    changed: list[Path] = []
    for path in iter_text_files():
        original = path.read_text(encoding="utf-8")
        updated = migrate(original)
        if updated == original:
            continue
        changed.append(path.relative_to(ROOT))
        if not args.check:
            path.write_text(updated, encoding="utf-8")

    for path in changed:
        print(path.as_posix())

    if args.check and changed:
        print(f"{len(changed)} file(s) still contain retired identity terms")
        return 1

    print(f"migrated {len(changed)} file(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
