#!/usr/bin/env python3
"""Check repository files and reachable Git history for crates.io token patterns.

Reports locations only. Never prints matching credential values.
"""
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
TOKEN = re.compile(rb"(?<![A-Za-z0-9])cio[A-Za-z0-9]{20,}(?![A-Za-z0-9])")


def main():
    failures = []
    paths = subprocess.check_output(["git", "ls-files", "-co", "--exclude-standard", "-z"], cwd=ROOT).split(b"\0")
    files = 0
    for name in set(paths) - {b""}:
        path = ROOT / name.decode(errors="surrogateescape")
        if path.is_file():
            files += 1
            if TOKEN.search(path.read_bytes()):
                failures.append(f"working file: {path.relative_to(ROOT)}")
    objects = subprocess.check_output(["git", "rev-list", "--objects", "--all"], cwd=ROOT).splitlines()
    checked = 0
    # A persistent cat-file process avoids a subprocess for each binary blob.
    with subprocess.Popen(["git", "cat-file", "--batch"], cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE) as proc:
        for record in objects:
            oid, _, name = record.partition(b" ")
            proc.stdin.write(oid + b"\n")
            proc.stdin.flush()
            header = proc.stdout.readline().split()
            if len(header) != 3:
                raise RuntimeError("Could not inspect Git object")
            size = int(header[2])
            data = proc.stdout.read(size)
            if len(data) != size or proc.stdout.read(1) != b"\n":
                raise RuntimeError("Incomplete Git object read")
            if header[1] in {b"blob", b"commit", b"tag"}:
                checked += 1
                if TOKEN.search(data):
                    label = name.decode(errors="replace") or oid.decode()
                    failures.append(f"Git {header[1].decode()}: {label}")
        proc.stdin.close()
        if proc.wait() != 0:
            raise RuntimeError("Git history inspection failed")
    if failures:
        for location in failures:
            print(f"Possible crates.io token found in {location}; value redacted")
        raise SystemExit(1)
    print(f"No crates.io token pattern found in {files} repository files or {checked} Git objects.")


if __name__ == "__main__":
    main()
