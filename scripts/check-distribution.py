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
    for key in ["ORBBEC_V1_SDK_DIR", "ORBBEC_V1_SYSROOT", "LD_LIBRARY_PATH", "BINDGEN_EXTRA_CLANG_ARGS", "LIBCLANG_PATH", "CARGO_REGISTRY_TOKEN", "CARGO_REGISTRIES_CRATES_IO_TOKEN"]:
        env.pop(key, None)
    # Cargo's temporary registry assumes a version's extracted source is immutable.
    # Repackaging an unpublished version can otherwise verify stale cached code.
    # Verify the exact archives below by testing their complete unpacked workspace.
    cmd = [cargo, "package", "--workspace", "--no-verify"]
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
        members = sorted(p.parent for p in scratch.glob("*/Cargo.toml"))
        manifest = '[workspace]\nresolver = "2"\nmembers = ' + json.dumps([p.name for p in members]) + '\n\n[patch.crates-io]\n'
        for p in members:
            name = p.name.rsplit("-", 1)[0]
            manifest += f'{name} = {{ path = "{p.name}" }}\n'
        (scratch / "Cargo.toml").write_text(manifest)
        shutil.copyfile(ROOT / "Cargo.lock", scratch / "Cargo.lock")
        # A dedicated publish runner may have an empty Cargo cache. Fetch only
        # normal registry dependencies before enforcing offline archive tests;
        # none of the checkout's code or SDK is used by the unpacked workspace.
        env["CARGO_TARGET_DIR"] = str(target / "distribution-check")
        subprocess.run([cargo, "fetch", "--locked", "--target", "x86_64-unknown-linux-gnu", "--target", "aarch64-unknown-linux-gnu"], cwd=scratch, env=env, check=True)
        subprocess.run([cargo, "test", "--workspace", "--locked", "--offline"], cwd=scratch, env=env, check=True)
        for triple, selected, excluded in [
            ("x86_64-unknown-linux-gnu", "orbbec-v1-bin-x86_64", "orbbec-v1-bin-aarch64"),
            ("aarch64-unknown-linux-gnu", "orbbec-v1-bin-aarch64", "orbbec-v1-bin-x86_64"),
        ]:
            tree = subprocess.check_output([cargo, "tree", "--offline", "-p", "orbbec-v1", "--target", triple], cwd=scratch, env=env, text=True)
            assert selected in tree and excluded not in tree, f"Wrong architecture dependency: {triple}"
    (target / "distribution-check").mkdir(exist_ok=True)
    (target / "distribution-check/report.json").write_text(json.dumps({"package_sizes_bytes": sizes, "isolated_consumer_tests": "passed", "isolated_workspace_tests": "passed", "target_dependency_selection": "passed", "published": False}, indent=2) + "\n")
    for name, size in sizes.items():
        print(f"{name}: {size:,} bytes")
    print("Packaged workspace passed; no SDK installation, loader environment, or libclang needed.")


if __name__ == "__main__":
    main()
