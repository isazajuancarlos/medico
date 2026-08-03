#!/usr/bin/env bash
# Despliega la demostración pública de medico en el VPS. Se ejecuta DESDE la
# máquina de desarrollo; hace ssh al servidor.
#
#   ./despliegue/desplegar-demo.sh debian@192.99.42.108
#
# Es el guion que documenta lo que se hizo a mano el 2026-08-02, escrito DESPUÉS
# de que funcionara y con los pasos en el orden en que funcionaron — no con el
# orden que uno supondría. Todo idempotente: se puede volver a lanzar.
#
# Lo que NO hace, a propósito, porque no es reversible desde aquí:
#   - crear el registro DNS de medico.xiliux.com (va en el registrador);
#   - pedir el certificado ni habilitar el sitio: eso es `publicar-demo.sh`,
#     que además rompe el círculo «sin certificado no hay sitio / sin sitio no
#     hay reto» con un sitio temporal.
set -euo pipefail

DESTINO="${1:-}"
if [ -z "$DESTINO" ]; then
    echo "Uso: $0 usuario@servidor      (por ejemplo: debian@192.99.42.108)" >&2
    exit 2
fi

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RAIZ"
BIN=target/release/medico-api
REMOTO=/opt/medico-demo
ENTORNO=/etc/medico-demo/medico.env

# El acceso se comprueba ANTES de construir: construir tarda minuto y medio, y
# el ssh es lo primero que puede fallar por algo ajeno al código.
echo "==> 0/6  comprobar el acceso a $DESTINO"
ssh -o ConnectTimeout=10 "$DESTINO" true || {
    echo "No se pudo entrar en «$DESTINO». No se construyó nada." >&2
    exit 3
}

echo "==> 1/6  construir el artefacto completo (interfaz + binario)"
./construir.sh

# Que el binario lleve la interfaz DENTRO se comprueba antes de subir 27 MB:
# `build.rs` ya rompe el release si falta `estaticos/index.html`, pero esto
# cuesta un grep y cierra el caso del binario rancio.
echo "==> 2/6  comprobar que el binario lleva la interfaz dentro"
grep -qa "ng-version" "$BIN" || {
    echo "El binario no parece llevar la interfaz. Revise construir.sh." >&2
    exit 4
}

echo "==> 3/6  usuario, directorios y binario"
ssh "$DESTINO" "set -e
    sudo -n id -u medico >/dev/null 2>&1 || \
        sudo -n useradd --system --no-create-home --shell /usr/sbin/nologin medico
    sudo -n mkdir -p $REMOTO /etc/medico-demo
    sudo -n chown medico:medico $REMOTO"
scp -q "$BIN" "$DESTINO:/tmp/medico-api"
ssh "$DESTINO" "sudo -n install -o medico -g medico -m 0755 /tmp/medico-api $REMOTO/medico-api && rm -f /tmp/medico-api"

# md5 a los dos lados: `scp` puede truncar y salir 0 si el disco se llena.
LOCAL="$(md5sum "$BIN" | cut -d' ' -f1)"
ARRIBA="$(ssh "$DESTINO" "md5sum $REMOTO/medico-api | cut -d' ' -f1")"
[ "$LOCAL" = "$ARRIBA" ] || { echo "el binario no llegó igual ($LOCAL vs $ARRIBA)" >&2; exit 5; }
echo "    md5 $LOCAL  ✔"

# El .env NO se pisa nunca desde aquí: lleva el secreto de sesión en uso, y
# reescribirlo invalidaría todas las sesiones vivas. Se crea si no existe.
echo "==> 4/6  entorno (solo si no existía)"
ssh "$DESTINO" "set -e
    if sudo -n test -f $ENTORNO; then
        echo '    ya existe, no se toca'
    else
        sudo -n tee $ENTORNO >/dev/null <<ENV
# La demostración pública de medico. NO es una instalación de cliente.
#
# MEDICO_DEMO_CORREO y MEDICO_DEMO_CLAVE hacen dos cosas a la vez, y ese es el
# diseño: siembran el usuario (medico-api sembrar-demo) y son lo que /api/demo
# publica en la pantalla de acceso. Así no puede anunciarse una credencial
# distinta de la que existe. Sin las dos, no hay demo y no se publica nada.
SECRET_KEY=\$(openssl rand -hex 32)
DATABASE_URL=sqlite://$REMOTO/medico.db
PORT=8010
MEDICO_ESCUCHA=127.0.0.1
MEDICO_TRAS_PROXY=true
MEDICO_DEMO_CORREO=demo@xiliux.com
MEDICO_DEMO_CLAVE=demo-medico-2026
ENV
        sudo -n chown root:medico $ENTORNO
        sudo -n chmod 0640 $ENTORNO
        echo '    creado con un secreto nuevo'
    fi"

echo "==> 5/6  unidad, semilla y arranque"
scp -q despliegue/medico-demo.service "$DESTINO:/tmp/"
ssh "$DESTINO" "set -e
    sudo -n install -o root -g root -m 0644 /tmp/medico-demo.service /etc/systemd/system/medico-demo.service
    rm -f /tmp/medico-demo.service
    sudo -n systemctl daemon-reload
    sudo -n -u medico env \$(sudo -n grep -v '^#' $ENTORNO | xargs) $REMOTO/medico-api sembrar-demo
    sudo -n systemctl enable --now medico-demo
    sudo -n systemctl restart medico-demo
    sleep 8"

# `is-active` NO prueba nada: Restart relanza el servicio con la versión
# anterior y `restart` sale 0 aunque la aplicación reviente al primer request.
# La verdad es el curl.
echo "==> 6/6  comprobar que responde de verdad"
ssh "$DESTINO" "set -e
    printf '    raiz    %s\n' \"\$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8010/)\"
    printf '    demo    %s\n' \"\$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8010/api/demo)\"
    # /api/acceso responde 503 llamando así, y es CORRECTO: el entorno declara
    # MEDICO_TRAS_PROXY=true y aquí no llega X-Real-IP. El freno al ensayo de
    # contraseñas se niega a meter a todos en el mismo cubo antes que inventar
    # el origen. Por nginx sí llega, y ahí es donde se prueba de verdad.
    printf '    acceso  %s  (503 aquí es correcto: falta X-Real-IP)\n' \\
        \"\$(curl -s -o /dev/null -w '%{http_code}' -X POST http://127.0.0.1:8010/api/acceso \\
            -H 'Content-Type: application/json' -d '{\"email\":\"x\",\"clave\":\"y\"}')\""

cat <<FIN

Desplegado. Falta publicarlo (sitio de nginx + certificado):

    ./despliegue/publicar-demo.sh $DESTINO

FIN
