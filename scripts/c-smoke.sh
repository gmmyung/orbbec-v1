#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
case "$(uname -m)" in
    x86_64) sdk_arch=x64 ;;
    aarch64|arm64) sdk_arch=arm64 ;;
    *) echo "Unsupported host architecture" >&2; exit 1 ;;
esac
sdk_dir=${ORBBEC_V1_SDK_DIR:-"$repo_dir/.sdk/$sdk_arch/OrbbecSDK_v1.10.37/SDK"}
mkdir -p "$repo_dir/target"
cc -std=c11 -Wall -Wextra -Werror -I"$sdk_dir/include" "$repo_dir/tools/c-smoke.c" \
    -L"$sdk_dir/lib" -Wl,-rpath,"$sdk_dir/lib" -lOrbbecSDK -o "$repo_dir/target/c-smoke"
exec "$repo_dir/scripts/with-sdk.sh" "$repo_dir/target/c-smoke" "$@"
