#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BUILD_DIR="${HORIZON_WIIU_BUILD_DIR:-${TMPDIR:-/tmp}/horizonwiiu-bootstrap}"

export DEVKITPRO="${DEVKITPRO:-/opt/devkitpro}"
export DEVKITPPC="${DEVKITPPC:-${DEVKITPRO}/devkitPPC}"
export PATH="${DEVKITPRO}/tools/bin:${DEVKITPRO}/portlibs/wiiu/bin:${DEVKITPPC}/bin:${PATH}"

if [[ ! -f "${SCRIPT_DIR}/.cafe/powerpc-cafe-nintendo.json" ]]; then
   git -C "${REPO_ROOT}" submodule update --init --recursive -- wiiu/bootstrap/.cafe
fi

mkdir -p "${BUILD_DIR}"
powerpc-eabi-cmake -S "${SCRIPT_DIR}" -B "${BUILD_DIR}" -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build "${BUILD_DIR}" --parallel "$(nproc)"

printf 'RPX: %s\n' "${BUILD_DIR}/horizon_wiiu_bootstrap.rpx"
printf 'WUHB: %s\n' "${BUILD_DIR}/horizon_wiiu_bootstrap.wuhb"
