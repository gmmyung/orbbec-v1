#!/usr/bin/env python3
"""Build a native standalone viewer distribution with SDK, notices, and sources."""
import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    arch = {"x86_64": "x86_64", "aarch64": "aarch64"}.get(platform.machine())
    if arch is None or platform.system() != "Linux":
        parser.error("Supported hosts: Linux x86_64 and AArch64")
    if os.environ.get("ORBBEC_V1_SDK_DIR"):
        parser.error("Unset ORBBEC_V1_SDK_DIR: this packager copies notices for the bundled SDK only")
    cargo = shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo")
    profile = ["--release"] if args.release else []
    subprocess.run([cargo, "build", "-p", "orbbec-v1-viewer", *profile], cwd=ROOT, check=True)
    sdk = Path(subprocess.check_output(
        [cargo, "run", "--quiet", "-p", "orbbec-v1", "--example", "sdk_path", *profile],
        cwd=ROOT, text=True,
    ).strip())
    metadata = json.loads(subprocess.check_output([cargo, "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT))
    target = Path(metadata["target_directory"])
    bundle = target / "dist" / f"orbbec-v1-viewer-{arch}"
    bundle.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(target / ("release" if args.release else "debug") / "orbbec-v1-viewer", bundle / "orbbec-v1-viewer")
    (bundle / "orbbec-v1-viewer").chmod(0o755)
    shutil.copytree(sdk, bundle / "orbbec-sdk", dirs_exist_ok=True)
    native = ROOT / "crates" / f"orbbec-v1-bin-{arch}"
    for name in ["LICENSE.txt", "THIRD_PARTY.md", "provenance.json"]:
        shutil.copyfile(native / name, bundle / "orbbec-sdk" / name)
    for name in ["licenses", "sources"]:
        shutil.copytree(native / name, bundle / "orbbec-sdk" / name, dirs_exist_ok=True)
    shutil.copyfile(ROOT / "LICENSE", bundle / "LICENSE")
    print(bundle)


if __name__ == "__main__":
    main()
