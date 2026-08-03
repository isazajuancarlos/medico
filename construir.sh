#!/usr/bin/env bash
# Construye el artefacto que recibe el cliente: UN archivo.
#
#   ./construir.sh
#
# Orden obligatorio y por qué: la interfaz se compila primero porque el binario
# la lleva DENTRO (`rust-embed`). Si se invierte, el binario sale con la interfaz
# de la vez anterior y nadie lo nota hasta que alguien pregunta por qué su
# arreglo no aparece.
#
# El orden lo garantiza este guion; que la interfaz esté es lo que garantiza
# `crates/medico-api/build.rs`, que rompe el release si falta. Dos cosas
# distintas: el guion evita el binario RANCIO, el build.rs evita el binario VACÍO.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$RAIZ"

echo "==> 1/3  interfaz (Angular)"
cd frontend
# Sin --legacy-peer-deps desde el 2026-08-02: la bandera existía por los peers
# de PrimeNG, y PrimeNG ya no está. Comprobado con `npm ci` limpio.
npm ci --silent || npm install --silent
npx ng build --configuration production
cd "$RAIZ"

echo "==> 2/3  empotrar la interfaz en el binario"
# Se vacía antes de copiar: si un archivo desaparece de la compilación nueva
# —un fragmento que se dividió distinto— y aquí quedara el viejo, el binario
# viajaría con código muerto que además podría servirse.
rm -rf crates/medico-api/estaticos
mkdir -p crates/medico-api/estaticos
# El `.gitkeep` está versionado para que la carpeta exista en un clon limpio; el
# `rm -rf` de arriba lo borraba, así que cada compilación dejaba el árbol sucio
# con un archivo eliminado. Se repone aquí, junto al vaciado que lo quita.
git -C "$RAIZ" checkout -- crates/medico-api/estaticos/.gitkeep 2>/dev/null \
  || : > crates/medico-api/estaticos/.gitkeep
cp -r frontend/dist/frontend/browser/. crates/medico-api/estaticos/

echo "==> 3/3  binario (Rust)"
cargo build --release --bin medico-api

BIN="target/release/medico-api"
echo
echo "Listo: $BIN  ($(du -h "$BIN" | cut -f1))"
