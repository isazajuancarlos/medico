#!/usr/bin/env bash
# Publica la demo YA DESPLEGADA en https://medico.xiliux.com.
#
#   ./despliegue/publicar-demo.sh debian@192.99.42.108
#
# Se ejecuta DESPUÉS de `desplegar-demo.sh` y DESPUÉS de que exista el registro
# DNS. Hace las tres cosas que aquél deja fuera —habilitar el sitio, pedir el
# certificado y recargar nginx— y comprueba el resultado contra el dominio real.
#
# === POR QUÉ ESTO ES UN GUION Y NO TRES LÍNEAS EN UN README ===
#
# Las tres líneas obvias NO funcionan en ese orden, y el fallo aparece con el
# sitio a medio publicar:
#
#   1. `medico.xiliux.com.nginx` trae `listen 443 ssl` con los `ssl_certificate`
#      apuntando a un certificado que aún no existe: `nginx -t` FALLA.
#   2. Pero certbot necesita que alguien conteste en el puerto 80 por ese nombre
#      para resolver el reto HTTP-01, y el sitio por defecto de este servidor
#      cierra con 444 a todo host desconocido. Sin sitio, no hay reto.
#
# O sea: no se puede habilitar sin certificado, y no se puede obtener el
# certificado sin habilitar. Se rompe el círculo con un sitio TEMPORAL de
# solo-HTTP que únicamente sirve el reto. Es el mismo problema —y la misma
# solución— que en informes.xiliux.com.
set -euo pipefail

DESTINO="${1:-}"
DOMINIO="${DOMINIO:-medico.xiliux.com}"
CORREO="${CORREO:-isazajuancarlos@gmail.com}"
if [ -z "$DESTINO" ]; then
    echo "Uso: $0 usuario@servidor      (por ejemplo: debian@192.99.42.108)" >&2
    exit 2
fi

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SITIO="$RAIZ/despliegue/$DOMINIO.nginx"
[ -f "$SITIO" ] || { echo "no encuentro $SITIO" >&2; exit 2; }

echo "==> 0/6  comprobar el acceso a $DESTINO"
ssh -o ConnectTimeout=10 "$DESTINO" true || exit 3

# El DNS se comprueba AQUÍ y no se descubre a medio camino: sin él certbot falla
# tras haber dejado el sitio temporal habilitado, que es el peor sitio donde
# parar. Y se compara con la dirección del servidor, no solo «resuelve algo».
echo "==> 1/6  comprobar que $DOMINIO resuelve a este servidor"
IP_SERVIDOR="$(ssh "$DESTINO" 'curl -sf --max-time 10 https://api.ipify.org || hostname -I | awk "{print \$1}"')"
IP_DNS="$(dig +short "$DOMINIO" A | tail -1)"
[ -n "$IP_DNS" ] || { echo "«$DOMINIO» no resuelve. Falta el registro A -> $IP_SERVIDOR" >&2; exit 4; }
[ "$IP_DNS" = "$IP_SERVIDOR" ] || {
    echo "«$DOMINIO» resuelve a $IP_DNS pero el servidor es $IP_SERVIDOR." >&2
    echo "El reto llegaría a otra máquina. No se tocó nada." >&2; exit 4; }
echo "    $DOMINIO -> $IP_DNS  ✔"

echo "==> 2/6  sitio TEMPORAL de solo-HTTP para el reto"
ssh "$DESTINO" "set -e
    sudo -n tee /etc/nginx/sites-available/$DOMINIO >/dev/null <<'TMP'
server {
    listen 80;
    listen [::]:80;
    server_name $DOMINIO;
    location /.well-known/acme-challenge/ { root /var/www/html; }
    location / { return 200 'preparando la demostración\n'; add_header Content-Type text/plain; }
}
TMP
    sudo -n ln -sf /etc/nginx/sites-available/$DOMINIO /etc/nginx/sites-enabled/$DOMINIO
    sudo -n nginx -t && sudo -n systemctl reload nginx"

echo "==> 3/6  certificado (idempotente: si ya existe, certbot no lo repite)"
ssh "$DESTINO" "sudo -n certbot certonly --webroot -w /var/www/html -d $DOMINIO \
    --non-interactive --agree-tos -m $CORREO --keep-until-expiring" | tail -4

echo "==> 4/6  sitio DEFINITIVO (solo si el certificado existe)"
scp -q "$SITIO" "$DESTINO:/tmp/$DOMINIO.nginx"
ssh "$DESTINO" "set -e
    sudo -n test -f /etc/letsencrypt/live/$DOMINIO/fullchain.pem || {
        echo 'sin certificado: no se instala el sitio definitivo' >&2; exit 1; }
    sudo -n install -o root -g root -m 0644 /tmp/$DOMINIO.nginx /etc/nginx/sites-available/$DOMINIO
    rm -f /tmp/$DOMINIO.nginx
    sudo -n nginx -t && sudo -n systemctl reload nginx"

echo "==> 5/6  la demo responde por el dominio"
printf '    https://%s/          %s\n' "$DOMINIO" "$(curl -s -o /dev/null -w '%{http_code}' --max-time 20 "https://$DOMINIO/")"
printf '    /api/demo                        %s\n' "$(curl -s -o /dev/null -w '%{http_code}' --max-time 20 "https://$DOMINIO/api/demo")"
# El acceso por el dominio SÍ debe dar 200: nginx pone X-Real-IP, que es lo que
# el freno necesita. Un 503 aquí significa que falta esa cabecera en el vhost.
CRED="$(curl -s --max-time 20 "https://$DOMINIO/api/demo")"
CORREO_DEMO="$(printf '%s' "$CRED" | sed -n 's/.*"correo":"\([^"]*\)".*/\1/p')"
CLAVE_DEMO="$(printf '%s' "$CRED" | sed -n 's/.*"clave":"\([^"]*\)".*/\1/p')"
printf '    /api/acceso con lo publicado     %s\n' \
    "$(curl -s -o /dev/null -w '%{http_code}' --max-time 20 -X POST "https://$DOMINIO/api/acceso" \
        -H 'Content-Type: application/json' \
        -d "{\"email\":\"$CORREO_DEMO\",\"clave\":\"$CLAVE_DEMO\"}")"

# Y que lo VIEJO siga en pie: una recarga de nginx puede tumbar otro sitio y
# `nginx -t` no lo diría.
echo "==> 6/6  el resto del servidor sigue en pie"
for u in https://xiliux.com/ https://informes.xiliux.com/ https://oprf.xiliux.com/healthz; do
    printf '    %-38s %s\n' "$u" "$(curl -s -o /dev/null -w '%{http_code}' --max-time 20 "$u")"
done
