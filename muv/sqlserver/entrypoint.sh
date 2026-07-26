#!/bin/bash
# Arranca SQL Server y deja lista la base FevsRipsApix.
#
# Sigue la guía de instalación de MinSalud (sección 11.3.5) con tres cambios
# que se explican donde tocan: la clave no se hornea, sqlcmd se busca en vez de
# suponerse, y no se restaura dos veces.
set -uo pipefail

BD=FevsRipsApix
MARCA=/var/opt/mssql/data/.fevrips-inicializada

# --- La clave llega por entorno, nunca del Dockerfile -----------------------
# La guía hace `ENV SA_PASSWORD=ClaveFuerte123*Alfa2` dentro del Dockerfile.
# Eso deja la clave en una capa de la imagen, visible con `docker history`
# para cualquiera que tenga el .tar. Aquí se exige por entorno y se cae si no
# está: una clave por defecto convertiría un despliegue mal configurado en uno
# que *parece* funcionar.
CLAVE="${MSSQL_SA_PASSWORD:-${SA_PASSWORD:-}}"
if [ -z "$CLAVE" ]; then
    echo "FALTA MSSQL_SA_PASSWORD. No hay valor por defecto a propósito." >&2
    exit 1
fi
export MSSQL_SA_PASSWORD="$CLAVE"

# --- sqlcmd se busca, no se supone ------------------------------------------
# La guía usa /opt/mssql-tools/bin/sqlcmd. En las imágenes 2022 recientes las
# herramientas son mssql-tools18, en OTRA ruta, y exigen -C para aceptar el
# certificado autofirmado del propio servidor. Suponer la ruta vieja da un
# "no such file or directory" que parece un fallo de SQL Server y no lo es.
SQLCMD=""
for c in /opt/mssql-tools18/bin/sqlcmd /opt/mssql-tools/bin/sqlcmd; do
    [ -x "$c" ] && SQLCMD="$c" && break
done
if [ -z "$SQLCMD" ]; then
    echo "No encuentro sqlcmd ni en mssql-tools18 ni en mssql-tools." >&2
    exit 1
fi
case "$SQLCMD" in *tools18*) CONFIAR="-C" ;; *) CONFIAR="" ;; esac

sql() { "$SQLCMD" -S localhost -U sa -P "$CLAVE" $CONFIAR "$@"; }

# --- Arranque ---------------------------------------------------------------
/opt/mssql/bin/sqlservr &
PID_SQL=$!

for i in $(seq 1 120); do
    sql -Q "SELECT 1" > /dev/null 2>&1 && break
    echo "esperando a SQL Server... ($i/120)"
    sleep 1
done
if ! sql -Q "SELECT 1" > /dev/null 2>&1; then
    echo "SQL Server no aceptó conexiones en 120 s." >&2
    exit 1
fi
echo "SQL Server listo."

# --- Inicialización, una sola vez -------------------------------------------
# La marca vive en el volumen de datos, no en la capa del contenedor: si se
# recrea el contenedor sobre el mismo volumen, la base ya está y restaurarla
# otra vez con REPLACE borraría lo validado hasta ahora.
if [ -f "$MARCA" ]; then
    echo "La base ya estaba inicializada ($MARCA). No se toca."
    wait $PID_SQL
    exit $?
fi

BAK=/usr/src/app/FevsRipsLocal.bak
SCRIPT=/usr/src/app/scripts/FEVRIPS-script.sql

if [ -f "$BAK" ]; then
    # El respaldo trae la base entera, catálogos incluidos. Restaurar Y ADEMÁS
    # correr el script encima es lo que sugiere la guía leída de corrido, y es
    # una forma de pisar los datos que acaban de restaurarse.
    echo "Restaurando $BD desde el respaldo del Ministerio..."
    sql -Q "RESTORE DATABASE [$BD] FROM DISK = N'$BAK' WITH FILE = 1,
        MOVE N'FevsRipsLocal'     TO N'/var/opt/mssql/data/${BD}.mdf',
        MOVE N'FevsRipsLocal_log' TO N'/var/opt/mssql/data/${BD}_log.ldf',
        NOUNLOAD, REPLACE, STATS = 5" || exit 1
elif [ -f "$SCRIPT" ]; then
    echo "No hay respaldo; creando $BD y aplicando el script..."
    sql -Q "CREATE DATABASE [$BD]" || exit 1
    sql -d "$BD" -i "$SCRIPT" || exit 1
else
    # Ni respaldo ni script: la imagen se construyó sin los artefactos del
    # Ministerio. Levantar un SQL Server vacío haría que el fallo apareciera
    # mucho después, como un error del API imposible de interpretar.
    echo "No hay ni $BAK ni $SCRIPT. Faltan los artefactos de MinSalud." >&2
    exit 1
fi

touch "$MARCA"
echo "Base $BD lista."
wait $PID_SQL
