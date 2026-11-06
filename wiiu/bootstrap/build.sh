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

if [[ -n "${CAFEGLSL_ROOT:-}" ]]; then
   if [[ ! -x "${CAFEGLSL_ROOT}/bin/glslcompiler" ]]; then
      printf 'CAFEGLSL_ROOT is set but bin/glslcompiler is missing: %s\n' "${CAFEGLSL_ROOT}" >&2
      exit 1
   fi
else
   CAFEGLSL_VERSION="1.0.0-rc1"
   CAFEGLSL_ROOT="${BUILD_DIR}/deps/cafeglsl-${CAFEGLSL_VERSION}-linux-x86_64"
   if [[ ! -x "${CAFEGLSL_ROOT}/bin/glslcompiler" ]]; then
      CAFEGLSL_ARCHIVE="${BUILD_DIR}/deps/cafeglsl-${CAFEGLSL_VERSION}-linux-x86_64.tar.xz"
      mkdir -p "${BUILD_DIR}/deps"
      curl -fsSL -o "${CAFEGLSL_ARCHIVE}" \
         "https://github.com/Exzap/CafeGLSL/releases/download/v${CAFEGLSL_VERSION}/cafeglsl-${CAFEGLSL_VERSION}-linux-x86_64.tar.xz"
      printf '%s  %s\n' \
         'fdbd561539080f1db74ec3f248edee4e1988d329a4d20f2273c72adf55015f97' \
         "${CAFEGLSL_ARCHIVE}" | sha256sum --check
      tar -xJf "${CAFEGLSL_ARCHIVE}" -C "${BUILD_DIR}/deps"
   fi
fi

export PATH="${CAFEGLSL_ROOT}/bin:${PATH}"
powerpc-eabi-cmake -S "${SCRIPT_DIR}" -B "${BUILD_DIR}" -G Ninja \
   -DCMAKE_BUILD_TYPE=Release \
   -DHORIZON_WIIU_CAPTURE_DIR="${HORIZON_WIIU_CAPTURE_DIR:-}" \
   -DHORIZON_WIIU_CAPTURE_MAX_VERTICES="${HORIZON_WIIU_CAPTURE_MAX_VERTICES:-65536}" \
   -DCAFEGLSL_COMPILER_EXECUTABLE="${CAFEGLSL_ROOT}/bin/glslcompiler"
cmake --build "${BUILD_DIR}" --parallel "$(nproc)"

printf 'RPX: %s\n' "${BUILD_DIR}/horizon_wiiu_bootstrap.rpx"
printf 'WUHB: %s\n' "${BUILD_DIR}/horizon_wiiu_bootstrap.wuhb"
