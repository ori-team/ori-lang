#!/usr/bin/env python3
"""Create a reproducible tar.gz archive for a staged Ori package."""

from __future__ import annotations

import argparse
import gzip
import os
import tarfile
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--epoch", type=int, default=0)
    return parser.parse_args()


def normalized_info(tar: tarfile.TarFile, path: Path, arcname: str, epoch: int) -> tarfile.TarInfo:
    info = tar.gettarinfo(str(path), arcname=arcname)
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    info.mtime = epoch
    # Preserve executable/readability bits but remove process-specific bits.
    info.mode &= 0o777
    return info


def package_entries(root: Path) -> list[Path]:
    """Return stable package entries, excluding compiler-local caches."""

    entries = [root]
    # Prune caches during traversal instead of discovering every cached object
    # and filtering it afterwards. A staged package can contain hundreds of
    # megabytes under `.ori/`; walking those files made archive creation both
    # slow and needlessly close to disk quotas.
    for current, directories, files in os.walk(root):
        directories[:] = sorted(name for name in directories if name != ".ori")
        for name in sorted(directories + files):
            entries.append(Path(current) / name)
    entries[1:] = sorted(
        entries[1:], key=lambda item: item.relative_to(root).as_posix()
    )
    return entries


def create_archive(root: Path, archive: Path, epoch: int) -> None:
    root = root.resolve()
    archive.parent.mkdir(parents=True, exist_ok=True)
    temporary = archive.with_name(f".{archive.name}.tmp-{os.getpid()}")
    if temporary.exists():
        temporary.unlink()
    try:
        with temporary.open("wb") as raw:
            with gzip.GzipFile(fileobj=raw, mode="wb", mtime=epoch, filename="") as compressed:
                with tarfile.open(fileobj=compressed, mode="w") as tar:
                    for path in package_entries(root):
                        relative = path.relative_to(root.parent).as_posix()
                        info = normalized_info(tar, path, relative, epoch)
                        if info.isreg():
                            with path.open("rb") as source:
                                tar.addfile(info, source)
                        else:
                            tar.addfile(info)
            raw.flush()
            os.fsync(raw.fileno())
        os.replace(temporary, archive)
    finally:
        temporary.unlink(missing_ok=True)


if __name__ == "__main__":
    args = parse_args()
    if args.epoch < 0:
        raise SystemExit("--epoch must not be negative")
    create_archive(args.root, args.archive, args.epoch)
