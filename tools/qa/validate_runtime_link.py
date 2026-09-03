#!/usr/bin/env python3
"""Validate the small, versioned runtime-link metadata contract.

This intentionally uses only the Python standard library so release and local
QA can run it before any compiler crate is available.  Artifact hashes are
checked when ``--check-artifacts`` is supplied; structural validation remains
useful for target metadata that is staged without binaries on the host.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any, NoReturn


TARGET_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]*-[A-Za-z0-9_.-]+$")
VERSION_RE = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$")
HASH_RE = re.compile(r"^[0-9a-fA-F]{64}$")
SCHEMA_PATH = Path(__file__).with_name("runtime-link.schema.json")


def fail(message: str) -> NoReturn:
    raise ValueError(message)


def validate_schema(value: Any, schema: dict[str, Any], path: str = "$") -> None:
    """Validate the small JSON-schema subset used by runtime-link metadata."""
    expected = schema.get("type")
    type_ok = {
        "object": isinstance(value, dict),
        "array": isinstance(value, list),
        "string": isinstance(value, str),
        "integer": isinstance(value, int) and not isinstance(value, bool),
        "boolean": isinstance(value, bool),
    }
    if expected is not None and not type_ok.get(expected, False):
        fail(f"{path} must be a {expected}")
    if "enum" in schema and value not in schema["enum"]:
        fail(f"{path} must be one of {schema['enum']!r}")
    pattern = schema.get("pattern")
    if pattern is not None and isinstance(value, str) and re.fullmatch(pattern, value) is None:
        fail(f"{path} does not match the required pattern")
    if isinstance(value, dict):
        required = schema.get("required", [])
        missing = sorted(set(required) - value.keys())
        if missing:
            fail(f"{path} is missing required fields: {', '.join(missing)}")
        properties = schema.get("properties", {})
        for key, child in value.items():
            if key in properties:
                validate_schema(child, properties[key], f"{path}.{key}")
    elif isinstance(value, list):
        min_items = schema.get("minItems")
        if min_items is not None and len(value) < min_items:
            fail(f"{path} must contain at least {min_items} item(s)")
        if "items" not in schema:
            return
        for index, child in enumerate(value):
            validate_schema(child, schema["items"], f"{path}[{index}]")


def validate(metadata_path: Path, artifact_root: Path | None) -> None:
    try:
        value = json.loads(metadata_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read metadata: {error}")
    try:
        schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read validation schema {SCHEMA_PATH}: {error}")
    if not isinstance(schema, dict):
        fail("runtime-link validation schema root must be an object")
    validate_schema(value, schema)
    if not isinstance(value, dict):
        fail("metadata root must be an object")

    target = value["target"]
    if not isinstance(target, str) or not TARGET_RE.fullmatch(target):
        fail("target must be a non-empty target triple")
    version = value["ori_version"]
    if not isinstance(version, str) or not VERSION_RE.fullmatch(version):
        fail("ori_version must be a semantic version")
    abi = value["abi_version"]
    if not isinstance(abi, str) or not abi.startswith("ori-native-abi-"):
        fail("abi_version must start with ori-native-abi-")
    profile = value["profile"]
    if profile not in {"debug", "release"}:
        fail("profile must be debug or release")

    artifact_names: list[str] = []
    for field in ("runtime", "runtime_cdylib"):
        name = value[field]
        if (
            not isinstance(name, str)
            or not name
            or Path(name).name != name
            or name in {".", ".."}
            or "/" in name
            or "\\" in name
        ):
            fail(f"{field} must be a basename, not a path")
        artifact_names.append(name)

    libraries = value["native_static_libs"]
    if not isinstance(libraries, list) or not libraries or not all(
        isinstance(item, str) and item for item in libraries
    ):
        fail("native_static_libs must be a non-empty string array")

    for field in ("runtime_sha256", "runtime_cdylib_sha256"):
        digest = value.get(field)
        if digest is not None and (not isinstance(digest, str) or not HASH_RE.fullmatch(digest)):
            fail(f"{field} must be a 64-character hexadecimal digest")

    if artifact_root is None:
        return
    if not artifact_root.is_dir():
        fail(f"artifact root does not exist: {artifact_root}")
    for name, hash_field in zip(
        artifact_names, ("runtime_sha256", "runtime_cdylib_sha256")
    ):
        artifact = artifact_root / name
        if artifact.is_symlink() or not artifact.is_file():
            fail(f"declared artifact is missing: {artifact}")
        expected = value.get(hash_field)
        if expected is None:
            continue
        digest = hashlib.sha256(artifact.read_bytes()).hexdigest()
        if digest.lower() != expected.lower():
            fail(f"{hash_field} does not match {artifact}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("metadata", type=Path)
    parser.add_argument(
        "--check-artifacts",
        type=Path,
        metavar="DIR",
        help="also require the declared artifacts and verify their hashes",
    )
    args = parser.parse_args()
    try:
        validate(args.metadata, args.check_artifacts)
    except ValueError as error:
        print(f"runtime_link: {error}", file=sys.stderr)
        return 1
    print(f"runtime_link: OK ({args.metadata})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
