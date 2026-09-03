#!/usr/bin/env python3
"""Generate a minimal SPDX 2.3 dependency SBOM from Cargo.lock."""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import pathlib
import re
import sys
import tomllib


def package_spdx_id(name: str, version: str, index: int) -> str:
    safe = re.sub(r"[^A-Za-z0-9.-]", "-", f"{name}-{version}")
    return f"SPDXRef-Package-{safe}-{index}"


def reproducible_creation_time() -> str:
    raw_epoch = os.environ.get("SOURCE_DATE_EPOCH")
    if raw_epoch is None:
        # Local invocations do not necessarily have a source commit. Epoch zero
        # is an explicit, deterministic fallback; release CI always provides the
        # tagged commit timestamp.
        print(
            "warning: SOURCE_DATE_EPOCH is unset; using 1970-01-01T00:00:00Z",
            file=sys.stderr,
        )
        epoch = 0
    else:
        try:
            epoch = int(raw_epoch, 10)
        except ValueError as error:
            raise SystemExit("SOURCE_DATE_EPOCH must be an integer") from error
        if epoch < 0:
            raise SystemExit("SOURCE_DATE_EPOCH must not be negative")

    try:
        created = datetime.datetime.fromtimestamp(epoch, datetime.timezone.utc)
    except (OverflowError, OSError, ValueError) as error:
        raise SystemExit("SOURCE_DATE_EPOCH is outside the supported range") from error
    return created.replace(microsecond=0).isoformat().replace("+00:00", "Z")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--lock", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--name", required=True)
    args = parser.parse_args()

    lock_bytes = args.lock.read_bytes()
    lock = tomllib.loads(lock_bytes.decode("utf-8"))
    packages = []
    relationships = []
    for index, dependency in enumerate(lock.get("package", []), start=1):
        name = dependency["name"]
        version = dependency["version"]
        spdx_id = package_spdx_id(name, version, index)
        package = {
            "SPDXID": spdx_id,
            "name": name,
            "versionInfo": version,
            "downloadLocation": dependency.get("source", "NOASSERTION"),
            "filesAnalyzed": False,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": "NOASSERTION",
            "copyrightText": "NOASSERTION",
        }
        checksum = dependency.get("checksum")
        if checksum and len(checksum) == 64:
            package["checksums"] = [
                {"algorithm": "SHA256", "checksumValue": checksum.lower()}
            ]
        packages.append(package)
        relationships.append(
            {
                "spdxElementId": "SPDXRef-DOCUMENT",
                "relationshipType": "DESCRIBES",
                "relatedSpdxElement": spdx_id,
            }
        )

    source_identity = os.environ.get("GITHUB_SHA") or hashlib.sha256(lock_bytes).hexdigest()
    document = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": args.name,
        "documentNamespace": f"https://github.com/ori-lang/ori/sbom/{source_identity}",
        "creationInfo": {
            "created": reproducible_creation_time(),
            "creators": ["Tool: ori-generate-sbom"],
        },
        "packages": packages,
        "relationships": relationships,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
