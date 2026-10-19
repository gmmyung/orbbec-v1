#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
case "$(uname -m)" in
    x86_64) sdk_arch=x64 ;;
    aarch64|arm64) sdk_arch=arm64 ;;
    *) echo "Unsupported host architecture" >&2; exit 1 ;;
esac
sdk_dir=${ORBBEC_V1_SDK_DIR:-"$repo_dir/.sdk/$sdk_arch/OrbbecSDK_v1.10.37/SDK"}
if [[ ! -f "$sdk_dir/lib/libOrbbecSDK.so.1.10.37" ]]; then
    echo "SDK not found. Run python3 scripts/fetch-sdk.py $sdk_arch" >&2
    exit 1
fi
export LD_LIBRARY_PATH="$sdk_dir/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PATH="$HOME/.cargo/bin:$PATH"
# This workspace can use user-local Clang headers extracted from Ubuntu packages.
# A normal system Clang installation is discovered automatically by bindgen.
clang_resource="$repo_dir/.tools/sysroot/usr/lib/llvm-21/lib/clang/21"
if [[ -f "$clang_resource/include/stdbool.h" ]]; then
    export BINDGEN_EXTRA_CLANG_ARGS="${BINDGEN_EXTRA_CLANG_ARGS:-} -resource-dir=$clang_resource"
fi
exec "$@"
