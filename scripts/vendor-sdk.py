#!/usr/bin/env python3
"""Stage byte-identical SDK binaries and upstream source archives for packaging.

Run fetch-sdk.py first. No native binaries are downloaded during Cargo builds.
Upstream source snapshots are included for inspection; correspondence to Orbbec's
private builds is explicitly unverified in provenance.json and THIRD_PARTY.md.
"""
import hashlib
import json
from pathlib import Path
import shutil
import tarfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
LOCK = json.loads((ROOT / "scripts/sdk-lock.json").read_text())
SOURCES = [
    {
        "name": "libusb-1.0.26.tar.bz2",
        "url": "https://github.com/libusb/libusb/releases/download/v1.0.26/libusb-1.0.26.tar.bz2",
        "sha256": "12ce7a61fc9854d1d2a1ffe095f7b5fac19ddba095c259e6067a46500381b5a5",
    },
    {
        "name": "live.2022.04.26.tar.gz",
        "url": "https://download.videolan.org/contrib/live555/live.2022.04.26.tar.gz",
        "sha256": "24fd24d53da1ac8f8282173039e99c9952186c18404b1fc3b1d4c0e9f49414e7",
    },
]
LIBRARIES = ["libOrbbecSDK.so.1.10.37", "libob_usb.so", "liblive555.so", "libdepthengine.so.2.0"]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    cache = ROOT / ".sdk/sources"
    cache.mkdir(parents=True, exist_ok=True)
    for source in SOURCES:
        archive = cache / source["name"]
        if not archive.exists():
            partial = archive.with_suffix(".partial")
            urllib.request.urlretrieve(source["url"], partial)
            partial.rename(archive)
        if digest(archive) != source["sha256"]:
            raise SystemExit(f"Source checksum mismatch: {archive}")
    for arch, suffix in [("x64", "x86_64"), ("arm64", "aarch64")]:
        asset = LOCK["assets"][arch]
        archive = ROOT / ".sdk" / asset["name"]
        if not archive.exists() or digest(archive) != asset["sha256"]:
            raise SystemExit(f"Run python3 scripts/fetch-sdk.py {arch} first")
        source = ROOT / ".sdk" / arch / "OrbbecSDK_v1.10.37"
        crate = ROOT / "crates" / f"orbbec-v1-bin-{suffix}"
        (crate / "sdk/lib").mkdir(parents=True, exist_ok=True)
        # Check extracted files against the hashed release before copying them.
        with zipfile.ZipFile(archive) as z:
            for name in LIBRARIES:
                path = source / "SDK/lib" / name
                original = z.read(f"OrbbecSDK_v1.10.37/SDK/lib/{name}")
                if path.read_bytes() != original:
                    raise SystemExit(f"Extracted SDK was modified: {path}")
                shutil.copyfile(path, crate / "sdk/lib" / name)
        shutil.copytree(source / "SDK/config", crate / "sdk/config", dirs_exist_ok=True)
        shutil.copyfile(source / "LICENSE.txt", crate / "LICENSE.txt")
        (crate / "sources").mkdir(exist_ok=True)
        (crate / "licenses").mkdir(exist_ok=True)
        shutil.copyfile(ROOT / "LICENSE", crate / "licenses/RUST-MIT.txt")
        for s in SOURCES:
            shutil.copyfile(cache / s["name"], crate / "sources" / s["name"])
        for archive_name, member, dest in [
            (SOURCES[0]["name"], "libusb-1.0.26/COPYING", "LIBUSB-LGPL-2.1.txt"),
            (SOURCES[1]["name"], "live/COPYING", "LIVE555-GPL-3.0.txt"),
            (SOURCES[1]["name"], "live/COPYING.LESSER", "LIVE555-LGPL-3.0.txt"),
        ]:
            with tarfile.open(cache / archive_name) as tar:
                (crate / "licenses" / dest).write_bytes(tar.extractfile(member).read())
        files = {}
        for directory in ["sdk", "sources", "licenses"]:
            for path in sorted((crate / directory).rglob("*")):
                if path.is_file():
                    files[str(path.relative_to(crate))] = digest(path)
        files["LICENSE.txt"] = digest(crate / "LICENSE.txt")
        provenance = {
            "sdk_version": LOCK["version"], "target": f"{suffix}-unknown-linux-gnu",
            "release": LOCK["release"], "release_archive": asset,
            "binaries_modified": False, "sources": SOURCES,
            "source_correspondence_verified": False,
            "source_note": "Upstream source versions match observed version identifiers, but Orbbec-specific patches and build scripts have not been established. See THIRD_PARTY.md.",
            "sha256": files,
        }
        (crate / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
        print(f"Staged {crate.name}: {len(files)} files, original binaries unchanged")


if __name__ == "__main__":
    main()
