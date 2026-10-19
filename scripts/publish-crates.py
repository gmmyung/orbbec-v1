#!/usr/bin/env python3
"""Plan an ordered crates.io release; upload only with the explicit --execute flag.

Credentials are read by Cargo from CARGO_REGISTRY_TOKEN, never from CLI arguments.
An existing version is skipped only when its package payload matches this release
(excluding Cargo's lockfile and VCS metadata, which vary with the workspace).
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
ORDER = (
    "orbbec-v1-bin-x86_64",
    "orbbec-v1-bin-aarch64",
    "orbbec-v1-sys",
    "orbbec-v1",
    "orbbec-v1-viewer",
)
USER_AGENT = "orbbec-v1-release (https://github.com/gmmyung/orbbec-v1)"


def fetch(url):
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=30) as response:
        return response.read()


def registry_version(name, version):
    try:
        return json.loads(fetch(f"https://crates.io/api/v1/crates/{name}/{version}"))["version"]
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise


def payload(path):
    files = {}
    with tarfile.open(path) as archive:
        for member in archive.getmembers():
            name = member.name.partition("/")[2]
            if member.isdir() or name in {"Cargo.lock", ".cargo_vcs_info.json"}:
                continue
            if not member.isfile() or not name or name in files:
                raise ValueError(f"Unexpected archive entry: {member.name}")
            files[name] = hashlib.sha256(archive.extractfile(member).read()).hexdigest()
    return files


def build_plan(packages, target):
    plan = []
    for name in ORDER:
        version = packages[name]["version"]
        archive = target / "package" / f"{name}-{version}.crate"
        if not archive.is_file():
            raise ValueError(f"Missing prepared package: {archive}")
        if archive.stat().st_size >= 10_000_000:
            raise ValueError(f"Package exceeds the conservative 10 MB limit: {archive.name}")
        checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
        existing = registry_version(name, version)
        action = "publish"
        if existing is not None:
            if existing["yanked"]:
                raise ValueError(f"{name} {version} is yanked; choose a new version")
            if existing["checksum"] != checksum:
                cached = target / "published-cache" / archive.name
                cached.parent.mkdir(parents=True, exist_ok=True)
                data = fetch(f"https://static.crates.io/crates/{name}/{archive.name}")
                if hashlib.sha256(data).hexdigest() != existing["checksum"]:
                    raise ValueError(f"Published archive checksum mismatch: {name} {version}")
                cached.write_bytes(data)
                if payload(archive) != payload(cached):
                    raise ValueError(f"{name} {version} exists with different contents; bump its version")
            action = "skip-matching-version"
        plan.append({"name": name, "version": version, "action": action, "sha256": checksum})
    return plan


def publish(plan, cargo):
    pending = [item for item in plan if item["action"] == "publish"]
    if pending and not os.environ.get("CARGO_REGISTRY_TOKEN"):
        raise ValueError("No publish token: use trusted publishing or the crates-io environment secret")
    for item in pending:
        # Cargo polls the index after uploading, before a dependent crate is published.
        subprocess.run([cargo, "publish", "--registry", "crates-io", "--locked", "-p", item["name"]], cwd=ROOT, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True, help="Expected orbbec-v1 version")
    parser.add_argument("--prepared", action="store_true", help="Reuse packages already verified in this checkout")
    parser.add_argument("--execute", action="store_true", help="Actually publish pending packages")
    args = parser.parse_args()
    cargo = shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo")
    dirty = subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True)
    if dirty:
        parser.error("Commit changes before preparing a release")
    metadata = json.loads(subprocess.check_output([cargo, "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=ROOT))
    packages = {p["name"]: p for p in metadata["packages"]}
    if set(packages) != set(ORDER) or any(not name.startswith("orbbec-v1") for name in packages):
        parser.error("Unexpected workspace package names; every package must use the orbbec-v1 prefix")
    if packages["orbbec-v1"]["version"] != args.version:
        parser.error("Requested version does not match orbbec-v1/Cargo.toml")
    target = Path(metadata["target_directory"])
    if not args.prepared:
        subprocess.run([cargo, "package", "--workspace", "--locked"], cwd=ROOT, check=True)
    # Check VCS identity even when reusing previously prepared artifacts.
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    for name in ORDER:
        version = packages[name]["version"]
        archive_path = target / "package" / f"{name}-{version}.crate"
        with tarfile.open(archive_path) as archive:
            info = json.load(archive.extractfile(f"{name}-{version}/.cargo_vcs_info.json"))["git"]
            if info["sha1"] != revision or info.get("dirty", False):
                parser.error(f"Repackage {name}: archive must match the clean current commit")
    plan = build_plan(packages, target)
    report = {"commit": revision, "requested_version": args.version, "packages": plan}
    (target / "release-plan.json").write_text(json.dumps(report, indent=2) + "\n")
    for item in plan:
        print(f'{item["action"]}: {item["name"]} {item["version"]}', flush=True)
    if args.execute:
        publish(plan, cargo)
    else:
        print("Verification only; no packages uploaded.")


if __name__ == "__main__":
    main()
