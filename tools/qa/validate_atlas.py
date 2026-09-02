#!/usr/bin/env python3
"""Validate the dependency-free, machine-readable Ori documentation Atlas.

The Atlas intentionally uses a small YAML subset so release packages do not
need a third-party YAML dependency. This validator checks that subset's schema
and every referenced path; it is not a general YAML parser.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REQUIRED_FEATURE_KEYS = {
    "name",
    "status",
    "implementation",
    "tests",
    "user_docs",
    "reference",
    "examples",
}
ALLOWED_STATUSES = {"stable", "partial", "implemented", "experimental", "planned"}
ARRAY_RE = re.compile(r"^\[(.*)\]$")
FEATURE_RE = re.compile(r"^  - id: ([A-Za-z0-9_.-]+)$")
KEY_RE = re.compile(r"^    ([a-z_]+):\s*(.*)$")


def parse_array(raw: str, line_number: int, key: str) -> list[str]:
    match = ARRAY_RE.fullmatch(raw)
    if not match:
        raise ValueError(f"line {line_number}: {key} must use a one-line array")
    values = []
    for item in match.group(1).split(","):
        value = item.strip()
        if not value:
            continue
        if len(value) >= 2 and value[0] == '"' and value[-1] == '"':
            path = value[1:-1]
        elif re.fullmatch(r"[A-Za-z0-9_./-]+", value):
            path = value
        else:
            raise ValueError(
                f"line {line_number}: {key} entries must be quoted or plain paths"
            )
        if path.startswith("/") or ".." in Path(path).parts:
            raise ValueError(f"line {line_number}: {key} contains unsafe path {path!r}")
        values.append(path)
    return values


def validate(atlas_path: Path, repo: Path) -> int:
    lines = atlas_path.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0].strip() != "schema: ori-doc-atlas/v1":
        raise ValueError("Atlas must declare schema ori-doc-atlas/v1 on its first line")

    features: list[tuple[int, dict[str, object]]] = []
    current: dict[str, object] | None = None
    current_line = 0
    in_features = False
    for line_number, line in enumerate(lines, start=1):
        if line == "features:":
            in_features = True
            continue
        feature_match = FEATURE_RE.fullmatch(line)
        if feature_match:
            if not in_features:
                raise ValueError(f"line {line_number}: feature appears before features:")
            if current is not None:
                features.append((current_line, current))
            current = {"id": feature_match.group(1)}
            current_line = line_number
            continue
        if current is None:
            continue
        key_match = KEY_RE.fullmatch(line)
        if not key_match:
            continue
        key, raw = key_match.groups()
        if key in current:
            raise ValueError(f"line {line_number}: duplicate feature key {key!r}")
        if key in {"implementation", "tests", "user_docs", "reference", "examples"}:
            current[key] = parse_array(raw, line_number, key)
        else:
            current[key] = raw.strip().strip('"')
    if current is not None:
        features.append((current_line, current))

    if not features:
        raise ValueError("Atlas must contain at least one feature")

    seen_ids: set[str] = set()
    for line_number, feature in features:
        feature_id = str(feature["id"])
        if feature_id in seen_ids:
            raise ValueError(f"line {line_number}: duplicate feature id {feature_id!r}")
        seen_ids.add(feature_id)
        missing = REQUIRED_FEATURE_KEYS.difference(feature)
        if missing:
            raise ValueError(f"line {line_number}: {feature_id} missing {sorted(missing)}")
        status = str(feature["status"])
        if status not in ALLOWED_STATUSES:
            raise ValueError(f"line {line_number}: {feature_id} has unknown status {status!r}")
        for key in REQUIRED_FEATURE_KEYS - {"name", "status"}:
            for path in feature[key]:  # type: ignore[index]
                if not (repo / path).exists():
                    raise ValueError(f"line {line_number}: {feature_id} references missing {path}")

    print(f"atlas_schema: OK ({len(features)} features, {len(seen_ids)} unique ids)")
    return 0


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: validate_atlas.py PATH/TO/features.yaml", file=sys.stderr)
        return 2
    atlas_path = Path(sys.argv[1]).resolve()
    repo = atlas_path.parents[2]
    try:
        return validate(atlas_path, repo)
    except (OSError, ValueError) as error:
        print(f"atlas_schema: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
