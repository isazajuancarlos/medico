#!/usr/bin/env bash
# Construye el artefacto que recibe el cliente: UN archivo.
#
#   ./construir.sh
#
# **Un solo paso desde el 2026-08-08.** Antes eran tres —`ng build`, copiar la
# salida a `estaticos/`, compilar— porque el binario llevaba dentro un SPA de
# Angular con `rust-embed`, y el orden importaba: invertirlo dejaba el binario
# con la interfaz de la vez anterior sin que nadie lo notara.
#
# Con el panel en `maud` no hay nada que copiar ni ordenar: las plantillas las
# compila `rustc`, y htmx, `boi.js` y la hoja de estilos van con `include_str!`
# desde `crates/medico-api/panel/`, que SÍ está versionado. El build dejó de
# necesitar Node.
#
# Este guion se conserva —en vez de decir «use cargo»— porque es el comando que
# nombra el despliegue y la documentación, y porque el día que vuelva a haber un
# paso previo, vuelve aquí y no a la memoria de nadie.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$RAIZ"

cargo build --release --bin medico-api

BIN="target/release/medico-api"
echo "Listo: $BIN  ($(du -h "$BIN" | cut -f1))"
