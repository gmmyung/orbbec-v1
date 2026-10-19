#!/usr/bin/env python3
"""Report an ELF file's GLIBC symbol versions and enforce a deployment ceiling.

This conservatively includes weak symbol requirements. It checks this ELF file,
not every transitive library, GPU driver, kernel, or USB device requirement.
"""
import argparse
from pathlib import Path
import re
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path)
    parser.add_argument("--max-version", required=True, help="Target GLIBC version, e.g. 2.39")
    args = parser.parse_args()
    if not re.fullmatch(r"\d+\.\d+(?:\.\d+)?", args.max_version):
        parser.error("--max-version must be a numeric version")
    output = subprocess.check_output(["readelf", "--version-info", str(args.executable)], text=True)
    versions = set(re.findall(r"Name: GLIBC_(\d+(?:\.\d+)+)", output))
    if not versions:
        parser.error("No GLIBC version requirements found; expected a dynamically linked GNU/Linux ELF")
    key = lambda version: tuple(map(int, version.split(".")))
    highest = max(versions, key=key)
    print(f"{args.executable.name}: highest referenced GLIBC version {highest}; deployment ceiling {args.max_version}")
    if key(highest) > key(args.max_version):
        parser.exit(1, "Incompatible GLIBC requirements: rebuild natively on the target or use a compatible build sysroot.\n")


if __name__ == "__main__":
    main()
