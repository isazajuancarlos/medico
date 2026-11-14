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

# Que el binario lleve la interfaz DENTRO se comprueba antes de subir 27 MB.
#
# ESTA COMPROBACIÓN ESTABA MUERTA Y HABRÍA PARADO EL DESPLIEGUE (arreglado el
# 2026-08-12). Buscaba `ng-version`, que es la marca que Angular deja en el
# `index.html` — y Angular salió del producto el 2026-08-08. O sea que el guion
# llevaba cuatro días abortando aquí sobre cualquier binario nuevo, con el
# mensaje exactamente al revés de lo que pasaba: la interfaz SÍ está, lo que ya
# no está es el marcador que buscaba. Es el modo de fallo de la directiva 31 —un
# veredicto anclado a un artefacto que cambió— aplicado a una salvaguarda.
#
# Y el PORQUÉ de la salvaguarda también cambió. El caso del «binario rancio» y
# el del «binario vacío» eran de la era de `rust-embed`: la interfaz se copiaba
# a `estaticos/` en un paso aparte, así que podía faltar o quedarse vieja. Con
# `maud` las plantillas las compila `rustc` y los tres activos entran por
# `include_str!` desde un directorio versionado: si faltaran, no compilaría.
# Esa clase de fallo desapareció.
#
# Lo que sigue mereciendo un grep es lo que el compilador NO puede cazar: que
# `$BIN` sea el binario que se acaba de construir y no otro que quedó ahí. Se
# busca una cadena que solo puede estar si el panel viaja dentro.
echo "==> 2/6  comprobar que el binario lleva la interfaz dentro"
for marca in "htmx" "boi —" "medico-api"; do
    grep -qa "$marca" "$BIN" || {
        echo "El binario no lleva «$marca» dentro: ¿es el que acaba de construir" >&2
        echo "\`construir.sh\`? No se subió nada." >&2
        exit 4
    }
done

echo "==> 3/6  usuario, directorios y binario"
ssh "$DESTINO" "set -e
    sudo -n id -u medico >/dev/null 2>&1 || \
        sudo -n useradd --system --no-create-home --shell /usr/sbin/nologin medico
    sudo -n mkdir -p $REMOTO /etc/medico-demo
    sudo -n chown medico:medico $REMOTO"
# `install` ABRE el destino con O_TRUNC, y sobre un binario que se está
# ejecutando eso es ETXTBSY: el guion funcionaba porque hasta hoy solo se había
# corrido contra un servidor donde no había nada corriendo. Se escribe al lado y
# se renombra encima: `rename` sustituye la entrada del directorio, el proceso
# vivo conserva su inodo hasta el `restart` de más abajo, y no hay un instante en
# que la ruta no exista.
# EL PUNTO DE VUELTA: una carpeta por despliegue, con las TRES piezas dentro.
#
# Aquí hubo dos versiones malas antes de ésta, y las dos merecen quedar escritas
# porque las dos parecían una red y no lo eran:
#
#  1. Solo el binario. Deja volver al código y NO a los datos — se vio el
#     2026-08-16 desplegando nueve migraciones de una pasada (0060–0068).
#  2. El binario y un `cp` de `medico.db` con su `sha256`. Peor, porque
#     tranquiliza: la base va en **modo WAL** (comprobado en producción, con un
#     `medico.db-wal` de 6,2 MB en disco), así que copiar el archivo suelto deja
#     fuera las transacciones confirmadas que aún viven en el WAL — y el `sha256`
#     sale VERDE igual, porque compara la copia con el original y los dos están
#     igual de incompletos. El control validaba el artefacto equivocado.
#
# La forma buena YA ESTABA EN EL REPOSITORIO: `medico-api respaldar` (B4.a,
# #733) usa `VACUUM INTO` —instantánea consistente, en caliente, sin parar el
# servicio—, se lleva las IMÁGENES diagnósticas (que viven fuera de la base, y
# que ninguna de las dos versiones anteriores tocaba) y verifica la copia
# ABRIÉNDOLA: `integrity_check`, recuentos por tabla e imagen por imagen. El
# módulo lo dice en su cabecera desde que nació: «copiar `medico.db` con el
# servidor vivo produce un archivo que se abre, que pasa por bueno, y al que le
# faltan las últimas transacciones». Reimplementarlo peor aquí fue la directiva 1
# al revés.
#
# Y va TODO EN LA MISMA CARPETA porque dos listas con dos marcas de tiempo y dos
# cupos distintos (5 y 6) se desalinean solas: basta un despliegue que aborte a
# mitad para que el n-ésimo binario deje de ser de la misma época que la n-ésima
# base. Una carpeta se poda entera y no hay nada que emparejar a ojo.
#
# La MARCA se calcula UNA vez, aquí, en local: dos `date` remotos en dos
# invocaciones `ssh` separadas dan segundos distintos.
MARCA="$(date +%Y%m%d-%H%M%S)"
RESPALDO="$REMOTO/respaldos/$MARCA"

echo "==> 3b/6  el punto de vuelta: base + imágenes + binario en $RESPALDO"
# El cuerpo remoto va por STDIN con un heredoc CITADO (<<'REMOTO_SH'), y eso no
# es estilo: es lo que quita una clase entera de fallos que ya mordió dos veces
# en una tarde. Dentro de `ssh "$DESTINO" "…"` con comillas dobles, el shell
# LOCAL expande la cadena antes de mandarla, así que un backtick escrito en un
# COMENTARIO —`cp`, `medico`, `respaldar`— es sustitución de comandos y se
# ejecuta aquí. Pasó las dos veces, y las dos fueron inocuas por suerte, no por
# diseño. Con el heredoc citado no se expande nada: el texto viaja literal y lo
# interpreta el servidor. Lo que hace falta de aquí se pasa como ARGUMENTOS
# posicionales, que es explícito y no depende de escapes.
ssh "$DESTINO" "sudo -n bash -s -- '$REMOTO' '$RESPALDO' '$ENTORNO'" <<'REMOTO_SH'
set -eu
REMOTO="$1"; RESPALDO="$2"; ENTORNO="$3"

if [ ! -f "$REMOTO/medico-api" ]; then
    echo '    no había instalación anterior que respaldar (primera vez)'
    exit 0
fi

# El respaldo lo toma el binario que HAY INSTALADO, que es el viejo, y eso es lo
# correcto: lee su propia base sin migrarla (`respaldar` abre con
# `SqliteConnection::connect`, no con el migrador). Pero un binario anterior a
# #733 no conoce el subcomando, y entonces `medico-api respaldar …` NO falla:
# cae al camino por defecto y ARRANCA EL SERVIDOR. Se comprueba contra el
# ARTEFACTO antes de invocarlo (comprobado con su control: el binario del
# 2026-08-15 no lleva la cadena, el de hoy sí).
if ! grep -qa 'uso: medico-api respaldar' "$REMOTO/medico-api"; then
    echo '    el binario instalado NO conoce «respaldar» (es anterior a #733).' >&2
    echo '    No se sigue: migrar sin punto de vuelta no se hace.' >&2
    exit 6
fi

# 0700, y NO 0755. En esta máquina hay otras cuentas con shell (`quantum`, uid
# 1001) y otros servicios con usuario propio: un respaldo de historias clínicas
# legible por «otros» es exposición de dato de salud (Ley 1581), y además
# PERSISTENTE — conserva lo que un paciente pidió suprimir de la base viva.
mkdir -p "$REMOTO/respaldos"
chown medico:medico "$REMOTO/respaldos"
chmod 0700 "$REMOTO/respaldos"

# Y la base VIVA estaba en 0644, que es de dónde salía la exposición: la copia
# solo la heredaba. El proceso corre como `medico`, así que 0640 no le quita nada.
[ -f "$REMOTO/medico.db" ] && chmod 0640 "$REMOTO/medico.db"

# Corre como `medico`: la base es suya y la copia tiene que nacer suya.
sudo -n -u medico sh -c 'set -a; . "$1"; set +a; exec "$2/medico-api" respaldar "$3"' \
    _ "$ENTORNO" "$REMOTO" "$RESPALDO" | sed 's/^/    /'

# El binario viejo entra en la MISMA carpeta. `cp`, no `mv`: el de arriba sigue
# sirviendo hasta el `restart` del paso 5.
cp -a "$REMOTO/medico-api" "$RESPALDO/medico-api"
chown -R medico:medico "$RESPALDO"
chmod -R go-rwx "$RESPALDO"

# Se conservan las 6 últimas carpetas enteras, ordenadas por NOMBRE y no por
# mtime: los nombres son `%Y%m%d-%H%M%S`, así que el orden lexicográfico ES el
# cronológico y no depende de las marcas del sistema de archivos, que un `cp -a`
# o una restauración heredan de otra época (la revisión midió un `ls -t` que
# eligió para borrar el respaldo más NUEVO). Borrado por ruta completa y con
# `--`. Sin `cd`: este bloque corre como root, pero un `cd` que falle dentro de
# una lista `&&` NO dispara `set -e` — así se quedó la poda sin correr y sin que
# nadie lo notara.
ls -1 "$REMOTO/respaldos" | grep -E '^[0-9]{8}-[0-9]{6}$' | sort -r | tail -n +7 |
while read -r vieja; do
    echo "    podada: $vieja"
    rm -rf -- "$REMOTO/respaldos/$vieja"
done

# Y los que deja `volver-atras.sh` al guardar el estado previo, que llevan otro
# prefijo y por tanto NO los recogía el patrón de arriba: crecían sin techo. Se
# conservan los 3 últimos —son la red de la red, no el punto de vuelta normal—.
ls -1 "$REMOTO/respaldos" | grep -E '^antes-de-volver-[0-9]{8}-[0-9]{6}$' | sort -r | tail -n +4 |
while read -r vieja; do
    echo "    podada: $vieja"
    rm -rf -- "$REMOTO/respaldos/$vieja"
done

echo "    punto de vuelta: $RESPALDO  (base verificada + imágenes + binario)"
REMOTO_SH
echo "    para volver: ./despliegue/volver-atras.sh $DESTINO $MARCA"

# El binario sube DESPUÉS del punto de vuelta, no antes: si el respaldo aborta no
# se han movido 36 MB por la red y no queda un `/tmp/medico-api` huérfano que la
# limpieza de más abajo ya no alcanza.
scp -q "$BIN" "$DESTINO:/tmp/medico-api"

ssh "$DESTINO" "set -e
    sudo -n install -o medico -g medico -m 0755 /tmp/medico-api $REMOTO/medico-api.nuevo
    sudo -n mv -f $REMOTO/medico-api.nuevo $REMOTO/medico-api
    rm -f /tmp/medico-api"

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
        # El modo va ANTES del contenido, no después. Un \`tee\` de root crea el
        # archivo con la umask de root —0644— y el \`chmod 0640\` de más abajo
        # llegaba con el secreto ya escrito y legible por todo el mundo. La
        # ventana era de dos órdenes, pero en esta máquina viven otros servicios
        # con su propia cuenta sin privilegios. \`tee\` trunca, no recrea, así
        # que el modo que pone \`install\` es el que queda.
        sudo -n install -o root -g medico -m 0640 /dev/null $ENTORNO
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
        echo '    creado con un secreto nuevo'
    fi"

echo "==> 5/6  unidad, semilla y arranque"
scp -q despliegue/medico-demo.service "$DESTINO:/tmp/"
ssh "$DESTINO" "set -e
    sudo -n install -o root -g root -m 0644 /tmp/medico-demo.service /etc/systemd/system/medico-demo.service
    rm -f /tmp/medico-demo.service
    sudo -n systemctl daemon-reload
    # El secreto NO viaja por la línea de órdenes. Con \`env \$(grep … | xargs)\`
    # el SECRET_KEY acababa en el \`argv\` de tres procesos, y \`/proc/PID/cmdline\`
    # lo lee cualquier cuenta de la máquina —aquí viven otros servicios— con un
    # \`ps auxww\` durante el despliegue. Con ese secreto se firma una cookie de
    # sesión para cualquier usuario, que es saltarse el acceso entero. Lo lee
    # ahora el propio proceso: el archivo es 0640 root:medico y corre como
    # \`medico\`, así que no hace falta un permiso más.
    sudo -n -u medico sh -c 'set -a; . $ENTORNO; set +a; exec $REMOTO/medico-api sembrar-demo'
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
