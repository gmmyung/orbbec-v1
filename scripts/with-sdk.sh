#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
# Cargo supplies the bundled SDK and runtime loader path. An explicit installation
# can still be used for native commands and existing workflows.
if [[ -n ${ORBBEC_V1_SDK_DIR:-} ]]; then
    export LD_LIBRARY_PATH="$ORBBEC_V1_SDK_DIR/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi
export PATH="$HOME/.cargo/bin:$PATH"
# This workspace can use user-local Clang headers extracted from Ubuntu packages.
# A normal system Clang installation is discovered automatically by bindgen.
clang_resource="$repo_dir/.tools/sysroot/usr/lib/llvm-21/lib/clang/21"
if [[ -f "$clang_resource/include/stdbool.h" ]]; then
    export BINDGEN_EXTRA_CLANG_ARGS="${BINDGEN_EXTRA_CLANG_ARGS:-} -resource-dir=$clang_resource"
fi
exec "$@"
