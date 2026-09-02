#!/usr/bin/env python3
"""Fail when a Rust file in the incremental formatting baseline drifts."""

from __future__ import annotations

import pathlib
import subprocess
import sys


def main() -> int:
    repo = pathlib.Path(__file__).resolve().parents[2]
    scope_file = repo / "tools" / "qa" / "rustfmt_scope.txt"
    paths = []
    for raw_line in scope_file.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        path = repo / line
        if not path.is_file():
            print(f"rustfmt scope contains missing file: {line}", file=sys.stderr)
            return 2
        paths.append(path)
    if not paths:
        print("rustfmt scope is empty", file=sys.stderr)
        return 2

    failed = False
    for path in paths:
        result = subprocess.run(
            ["rustfmt", "--edition", "2021", "--check", str(path)],
            cwd=repo,
            check=False,
        )
        failed = failed or result.returncode != 0
    if failed:
        print("scoped rustfmt baseline: FAILED", file=sys.stderr)
        return 1
    print(f"scoped rustfmt baseline: OK ({len(paths)} files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
