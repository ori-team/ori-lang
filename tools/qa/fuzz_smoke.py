#!/usr/bin/env python3
"""Deterministic hostile-input smoke for the Ori front end.

This is deliberately dependency-free. It is not a replacement for coverage
guided fuzzing; it is a fast regression gate that ensures malformed bytes,
deep nesting, and truncated constructs terminate without a panic or timeout.
"""

from __future__ import annotations

import os
import random
import subprocess
import sys
import tempfile
from pathlib import Path


def find_ori(repo: Path) -> Path | None:
    configured = os.environ.get("ORI_BIN")
    if configured:
        candidate = Path(configured)
        return candidate if candidate.is_file() and os.access(candidate, os.X_OK) else None
    for candidate in (
        repo / "compiler" / "target" / "debug" / "ori",
        repo / "compiler" / "target" / "release" / "ori",
    ):
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return candidate
    return None


def corpus(case_count: int) -> list[bytes]:
    rng = random.Random(0x4F5249)
    cases = [
        b"",
        b"module app.main\n\nmain()\nend\n",
        b"module app.main\n\nmain(\n",
        b"module app.main\n\nmain()\n    match true\n",
        b"module app.main\n\nmain()\n    const x: result[int, string] = ok(1)\n",
        (b"module app.main\n\nmain()\n" + b"    if true\n" * 512),
        b"module app.main\n\nmain()\n    check 1, 2\nend\n",
    ]
    alphabet = b"()[]{}:,=<>+-*/_|'\" \t\nabcXYZ012"
    while len(cases) < case_count:
        size = rng.randrange(0, 512)
        cases.append(bytes(rng.choice(alphabet) for _ in range(size)))
    return cases[:case_count]


def main() -> int:
    repo = Path(__file__).resolve().parents[2]
    ori = find_ori(repo)
    required = os.environ.get("ORI_REQUIRE_FUZZ") == "1"
    if ori is None:
        message = "fuzz_smoke: INCOMPLETE (ori binary not found; set ORI_BIN)"
        print(message, file=sys.stderr)
        return 2 if required else 0

    try:
        case_count = max(1, int(os.environ.get("ORI_FUZZ_CASES", "32")))
        timeout = max(0.1, float(os.environ.get("ORI_FUZZ_TIMEOUT", "2")))
    except ValueError as error:
        print(f"fuzz_smoke: invalid configuration: {error}", file=sys.stderr)
        return 2

    failures: list[str] = []
    with tempfile.TemporaryDirectory(prefix="ori-fuzz-") as temp_dir:
        root = Path(temp_dir)
        for index, source in enumerate(corpus(case_count)):
            path = root / f"case_{index:03}.orl"
            path.write_bytes(source)
            try:
                completed = subprocess.run(
                    [str(ori), "check", str(path)],
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    timeout=timeout,
                    check=False,
                )
            except subprocess.TimeoutExpired:
                failures.append(f"case {index}: timeout after {timeout:.1f}s")
                continue
            if completed.returncode < 0:
                failures.append(f"case {index}: terminated by signal {-completed.returncode}")
                continue
            output = (completed.stdout + completed.stderr).decode("utf-8", "replace").lower()
            if any(marker in output for marker in ("panicked at", "stack overflow", "abort trap")):
                failures.append(f"case {index}: panic-like output detected")

    if failures:
        print("fuzz_smoke: FAIL", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1
    print(f"fuzz_smoke: OK ({case_count} deterministic hostile inputs)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
