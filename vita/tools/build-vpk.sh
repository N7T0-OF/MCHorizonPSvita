#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [ -z "${VITASDK:-}" ]; then
  echo "VITASDK n'est pas défini. Installe VitaSDK et exporte VITASDK=/chemin/vers/vitasdk" >&2
  exit 1
fi
if [ ! -f "$VITASDK/share/vita.toolchain.cmake" ]; then
  echo "Toolchain Vita absente : $VITASDK/share/vita.toolchain.cmake" >&2
  exit 1
fi
cmake -S vita-bootstrap -B vita-bootstrap/build -DCMAKE_TOOLCHAIN_FILE="$VITASDK/share/vita.toolchain.cmake" -DCMAKE_BUILD_TYPE=Release
cmake --build vita-bootstrap/build -j "$(nproc)"
find vita-bootstrap/build -name '*.vpk' -print
