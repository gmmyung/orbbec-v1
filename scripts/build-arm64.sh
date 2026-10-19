#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_dir"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="$repo_dir/scripts/aarch64-linker.sh"
if [[ -d /usr/aarch64-linux-gnu/include ]]; then
    export ORBBEC_V1_SYSROOT=${ORBBEC_V1_SYSROOT:-/usr/aarch64-linux-gnu}
elif [[ -d "$repo_dir/.tools/sysroot/usr/aarch64-linux-gnu/include" ]]; then
    export ORBBEC_V1_SYSROOT=${ORBBEC_V1_SYSROOT:-"$repo_dir/.tools/sysroot/usr/aarch64-linux-gnu"}
else
    echo "Install gcc-aarch64-linux-gnu and libc6-dev-arm64-cross first" >&2
    exit 1
fi
exec "$repo_dir/scripts/with-sdk.sh" cargo build --target aarch64-unknown-linux-gnu --examples "$@"
