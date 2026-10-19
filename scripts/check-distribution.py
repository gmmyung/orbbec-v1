#!/usr/bin/env python3
"""Verify crate payloads and run a consumer using only unpacked published files.

Requires Cargo with `cargo package --workspace` support (validated with 1.99.1).
This script never uploads packages or declares license/source correspondence.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-dirty", action="store_true")
    args = parser.parse_args()
    cargo = shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo")
    env = os.environ.copy()
    for key in ["ORBBEC_V1_SDK_DIR", "ORBBEC_V1_SYSROOT", "LD_LIBRARY_PATH", "BINDGEN_EXTRA_CLANG_ARGS", "LIBCLANG_PATH"]:
        env.pop(key, None)
    cmd = [cargo, "package", "--workspace"]
    if args.allow_dirty:
        cmd.append("--allow-dirty")
    subprocess.run(cmd, cwd=ROOT, env=env, check=True)
    metadata = json.loads(subprocess.check_output([cargo, "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT, env=env))
    target = Path(metadata["target_directory"])
    archives = sorted((target / "package").glob("orbbec-*.crate"))
    expected = {f'{p["name"]}-{p["version"]}' for p in metadata["packages"]}
    archives = [p for p in archives if p.stem in expected]
    assert len(archives) == len(expected), "Missing crate archive"
    sizes = {}
    with tempfile.TemporaryDirectory(prefix="orbbec-packages-") as tmp:
        scratch = Path(tmp)
        for archive in archives:
            assert archive.stat().st_size < 10_000_000, f"Package exceeds conservative 10 MB limit: {archive}"
            sizes[archive.name] = archive.stat().st_size
            with tarfile.open(archive) as tar:
                tar.extractall(scratch, filter="data")
            unpacked = scratch / archive.stem
            if archive.name.startswith("orbbec-v1-bin-"):
                provenance = json.loads((unpacked / "provenance.json").read_text())
                for name, expected_hash in provenance["sha256"].items():
                    actual = hashlib.sha256((unpacked / name).read_bytes()).hexdigest()
                    assert actual == expected_hash, f"Payload checksum mismatch: {name}"
        members = sorted(p.parent for p in scratch.glob("*/Cargo.toml") if not p.parent.name.startswith("orbbec-v1-viewer-"))
        manifest = '[workspace]\nresolver = "2"\nmembers = ' + json.dumps([p.name for p in members]) + '\n\n[patch.crates-io]\n'
        for p in members:
            name = p.name.rsplit("-", 1)[0]
            manifest += f'{name} = {{ path = "{p.name}" }}\n'
        (scratch / "Cargo.toml").write_text(manifest)
        # Reuse cached third-party dependencies, but none of the checkout's code or SDK.
        env["CARGO_TARGET_DIR"] = str(target / "distribution-check")
        subprocess.run([cargo, "test", "--offline", "-p", "orbbec-v1"], cwd=scratch, env=env, check=True)
        for triple, selected, excluded in [
            ("x86_64-unknown-linux-gnu", "orbbec-v1-bin-x86_64", "orbbec-v1-bin-aarch64"),
            ("aarch64-unknown-linux-gnu", "orbbec-v1-bin-aarch64", "orbbec-v1-bin-x86_64"),
        ]:
            tree = subprocess.check_output([cargo, "tree", "--offline", "-p", "orbbec-v1", "--target", triple], cwd=scratch, env=env, text=True)
            assert selected in tree and excluded not in tree, f"Wrong architecture dependency: {triple}"
    (target / "distribution-check").mkdir(exist_ok=True)
    (target / "distribution-check/report.json").write_text(json.dumps({"package_sizes_bytes": sizes, "isolated_consumer_tests": "passed", "target_dependency_selection": "passed", "published": False}, indent=2) + "\n")
    for name, size in sizes.items():
        print(f"{name}: {size:,} bytes")
    print("Packaged consumer passed; no SDK installation, loader environment, or libclang needed.")


if __name__ == "__main__":
    main()
