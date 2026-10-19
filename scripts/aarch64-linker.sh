#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
sdk_dir=${ORBBEC_V1_SDK_DIR:-"$repo_dir/.sdk/arm64/OrbbecSDK_v1.10.37/SDK"}
if command -v aarch64-linux-gnu-gcc >/dev/null; then
    exec aarch64-linux-gnu-gcc -Wl,-rpath-link,"$sdk_dir/lib" "$@"
fi
tools_dir="$repo_dir/.tools/sysroot/usr"
export LD_LIBRARY_PATH="$tools_dir/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$tools_dir/bin/aarch64-linux-gnu-gcc-15" --sysroot="$repo_dir/.tools/sysroot" -B"$tools_dir/aarch64-linux-gnu/bin/" \
    -Wl,-rpath-link,"$sdk_dir/lib" -Wl,-rpath-link,"$tools_dir/aarch64-linux-gnu/lib" "$@"
