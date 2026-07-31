#!/bin/bash
# Genera el certificado autofirmado que Kestrel necesita para servir el 9443.
#
# El archivo del Ministerio espera un `fevripsapilocal.pfx` en `C:\Certificates`
# que NO viene con él: se supone que cada quien pone el suyo. Aquí se fabrica,
# con los mismos pasos que documenta `certificado.txt` de la instalación previa.
#
# Autofirmado está bien y no es una concesión: la conexión no sale de la
# máquina. Por eso `validar_muv.py` se llama con `--sin-verificar-tls`.
set -euo pipefail

DESTINO="$(dirname "$(readlink -f "$0")")/certificates"
PFX="$DESTINO/fevrips.pfx"

if [ -f "$PFX" ]; then
    echo "Ya existe $PFX. Bórralo si quieres regenerarlo."
    exit 0
fi

# La clave se pide, no se inventa: un valor por defecto acabaría copiado tal
# cual en el compose de alguien más.
if [ -z "${CERT_PASSWORD:-}" ]; then
    echo "Exporta CERT_PASSWORD con la clave que protegerá el .pfx." >&2
    echo "La MISMA hay que exportarla al levantar: el compose la exige." >&2
    exit 1
fi

mkdir -p "$DESTINO"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

openssl req -x509 -newkey rsa:2048 -sha256 -days 825 -nodes \
    -keyout "$TMP/servidor.key" -out "$TMP/servidor.crt" \
    -subj "/CN=localhost" \
    -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"

openssl pkcs12 -export -out "$PFX" \
    -inkey "$TMP/servidor.key" -in "$TMP/servidor.crt" \
    -passout env:CERT_PASSWORD

chmod 600 "$PFX"
echo "Escrito $PFX"
openssl pkcs12 -in "$PFX" -passin env:CERT_PASSWORD -nokeys -noout \
    && echo "Comprobado: el .pfx abre con esa clave."
