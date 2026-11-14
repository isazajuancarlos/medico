#!/usr/bin/env bash
# Vuelve medico al punto de vuelta de un despliegue anterior: la BASE, las
# IMÁGENES y el BINARIO, en ese orden y de una vez.
#
#   ./despliegue/volver-atras.sh debian@192.99.42.108            # lista los puntos
#   ./despliegue/volver-atras.sh debian@192.99.42.108 20260816-231430
#
# Existe porque hasta el 2026-08-16 la vuelta atrás no estaba escrita en ninguna
# parte. Lo único que había era la receta genérica de `conocimiento/despliegue/
# VPS.md` —restaurar el archivo del servicio y reiniciar—, y aplicada a medico
# ESO NO FUNCIONA. Tres razones, las tres medidas:
#
#  1. `sqlx::migrate!` valida al arrancar que toda migración aplicada en la base
#     la conozca el binario (`ignore_missing` va en false y nadie lo cambia). Un
#     binario anterior contra una base ya migrada aborta con `VersionMissing`,
#     `abrir` devuelve Err y el servicio queda en `failed` — no degradado:
#     caído. Así que restaurar SOLO el binario nunca es una vuelta atrás si hubo
#     migraciones, sean aditivas o no.
#  2. La base va en modo WAL. Restaurarla con `cp` dejando el `-wal` al lado no
#     restaura nada: SQLite reproduce ese WAL sobre el archivo recién puesto y
#     devuelve el estado que se quería abandonar, con `integrity_check` en `ok`.
#     `medico-api restaurar` borra `-wal`/`-shm` a propósito.
#  3. Las imágenes diagnósticas viven FUERA de la base. Volver la base sin ellas
#     deja filas que prometen una placa que el disco no tiene.
#
# Es idempotente y no destruye sin red: antes de pisar nada toma un punto de
# vuelta del estado ACTUAL, que es el que hace falta si la vuelta atrás resulta
# ser peor que el problema.
set -euo pipefail

DESTINO="${1:-}"
MARCA="${2:-}"
if [ -z "$DESTINO" ]; then
    echo "Uso: $0 usuario@servidor [marca]" >&2
    echo "     sin marca, lista los puntos de vuelta disponibles." >&2
    exit 2
fi

REMOTO=/opt/medico-demo
ENTORNO=/etc/medico-demo/medico.env
SVC=medico-demo

if [ -z "$MARCA" ]; then
    echo "Puntos de vuelta en $DESTINO:$REMOTO/respaldos (el más reciente arriba)"
    # Por NOMBRE y no por mtime: los nombres son marcas de tiempo, y el mtime de
    # una carpeta restaurada o copiada con `-a` es de otra época. Se midió: `ls
    # -1t` enseñaba el punto más nuevo entre dos viejos.
    ssh "$DESTINO" "sudo -n ls -1 $REMOTO/respaldos 2>/dev/null | sort -r | while read -r d; do
        printf '  %-20s %s\n' \"\$d\" \"\$(sudo -n du -sh $REMOTO/respaldos/\$d | cut -f1)\"
    done" || echo "  (ninguno)"
    echo
    echo "Volver a uno:  $0 $DESTINO <marca>"
    exit 0
fi

CARPETA="$REMOTO/respaldos/$MARCA"

echo "==> 1/5  comprobar que el punto de vuelta está COMPLETO"
# Las tres piezas, o no se para el servicio. Un servicio parado y un respaldo a
# medias es peor que el problema que se venía a arreglar.
ssh "$DESTINO" "set -e
    sudo -n test -d $CARPETA || { echo 'no existe $CARPETA' >&2; exit 3; }
    for pieza in medico.db medico-api; do
        sudo -n test -f $CARPETA/\$pieza || {
            echo \"al punto de vuelta le falta \$pieza: no se toca nada\" >&2; exit 3; }
    done
    printf '    base   %s\n' \"\$(sudo -n du -h $CARPETA/medico.db | cut -f1)\"
    printf '    binario %s\n' \"\$(sudo -n du -h $CARPETA/medico-api | cut -f1)\"
    sudo -n test -d $CARPETA/imagenes && \\
        printf '    imágenes %s\n' \"\$(sudo -n du -sh $CARPETA/imagenes | cut -f1)\" || \\
        echo '    (sin carpeta de imágenes en este punto)'"

echo "==> 2/5  punto de vuelta del estado ACTUAL, antes de pisarlo"
AHORA="$REMOTO/respaldos/antes-de-volver-$(date +%Y%m%d-%H%M%S)"
ssh "$DESTINO" "set -e
    sudo -n -u medico sh -c 'set -a; . $ENTORNO; set +a; exec $REMOTO/medico-api respaldar $AHORA' | sed 's/^/    /'
    sudo -n cp -a $REMOTO/medico-api $AHORA/medico-api"

echo "==> 3/5  parar el servicio"
# Se para ANTES de tocar la base: restaurar bajo un proceso vivo deja al proceso
# con su inodo viejo y a SQLite con dos verdades.
ssh "$DESTINO" "sudo -n systemctl stop $SVC && sleep 2 && systemctl is-active $SVC || true"

echo "==> 4/5  restaurar base + imágenes, y el binario de esa época"
# `--forzar` es obligatorio y consciente: la base de destino TIENE datos, y sin
# la bandera `restaurar` se niega —esa negativa es correcta y por eso no se le
# quita, se le pasa la bandera aquí, donde el operador ya eligió la marca.
ssh "$DESTINO" "set -e
    sudo -n -u medico sh -c 'set -a; . $ENTORNO; set +a; exec $REMOTO/medico-api restaurar $CARPETA --forzar' | sed 's/^/    /'
    sudo -n install -o medico -g medico -m 0755 $CARPETA/medico-api $REMOTO/medico-api.volviendo
    sudo -n mv -f $REMOTO/medico-api.volviendo $REMOTO/medico-api"

echo "==> 5/5  arrancar y comprobar que responde de verdad"
# `is-active` no prueba nada: el fallo característico de una vuelta atrás mal
# hecha es justamente que el binario viejo no arranca contra la base, y systemd
# lo relanza en bucle. Se mira el código HTTP y, si no responde, el diario.
ssh "$DESTINO" "set -e
    sudo -n systemctl start $SVC
    sleep 8
    codigo=\$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8010/ || echo 000)
    printf '    raiz  %s\n' \"\$codigo\"
    if [ \"\$codigo\" = 000 ]; then
        echo '    NO responde. Las últimas líneas del diario:' >&2
        journalctl -u $SVC -n 20 --no-pager >&2
        echo \"    el estado de antes de volver está en $AHORA\" >&2
        exit 5
    fi"

cat <<FIN

Vuelto a $MARCA. El estado que había justo antes quedó en:

    $AHORA

FIN
