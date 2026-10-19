#!/usr/bin/env python3
"""Download pinned official SDK packages, verify hashes, preserve ZIP symlinks."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import stat
import urllib.parse
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
LOCK = json.loads((ROOT / "scripts/sdk-lock.json").read_text())


def fetch(arch):
    asset = LOCK["assets"][arch]
    cache = ROOT / ".sdk"
    cache.mkdir(exist_ok=True)
    archive = cache / asset["name"]
    if not archive.exists():
        url = "https://github.com/orbbec/OrbbecSDK/releases/download/v" + LOCK["version"] + "/" + urllib.parse.quote(asset["name"])
        partial = archive.with_suffix(".partial")
        urllib.request.urlretrieve(url, partial)
        partial.rename(archive)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    if digest != asset["sha256"]:
        raise SystemExit(f"SHA256 mismatch for {archive}; remove it and retry")
    dest = cache / arch
    dest.mkdir(exist_ok=True)
    with zipfile.ZipFile(archive) as z:
        for info in z.infolist():
            path = dest / info.filename
            if not path.resolve().is_relative_to(dest.resolve()):
                raise SystemExit(f"Unsafe ZIP path: {info.filename}")
            if info.is_dir():
                path.mkdir(parents=True, exist_ok=True)
                continue
            path.parent.mkdir(parents=True, exist_ok=True)
            # Remove an old extracted file/link before replacing it.
            path.unlink(missing_ok=True)
            if stat.S_ISLNK(info.external_attr >> 16):
                target = z.read(info).decode()
                if not (path.parent / target).resolve().is_relative_to(dest.resolve()):
                    raise SystemExit(f"Unsafe ZIP symlink: {target}")
                path.symlink_to(target)
            else:
                with z.open(info) as src, path.open("wb") as out:
                    shutil.copyfileobj(src, out)
                mode = (info.external_attr >> 16) & 0o777
                if mode:
                    path.chmod(mode)
    print(f"{arch}: SHA256 verified; SDK at {dest}/OrbbecSDK_v{LOCK['version']}/SDK")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("architectures", nargs="*", choices=["x64", "arm64"])
    args = parser.parse_args()
    for architecture in args.architectures or ["x64", "arm64"]:
        fetch(architecture)
