"""Vuelca las tablas de referencia del validador FEV-RIPS a maestros locales.

    python herramientas/extraer_maestros_muv.py            # lista lo disponible
    python herramientas/extraer_maestros_muv.py UPR        # una
    python herramientas/extraer_maestros_muv.py --todas

=== POR QUÉ DE AQUÍ Y NO DE UN CSV ===

El contenedor del MUV sincroniza estas tablas desde SISPRO y valida contra
ellas. Cualquier otra copia —un CSV descargado a mano, una hoja de cálculo de
hace dos años— es una fuente paralela que se desincroniza en silencio. Aquí se
toma exactamente lo que el validador va a usar para juzgarnos.

Es un paso DELIBERADO, igual que `generar_cum.py`: un consultorio en zona rural
no puede depender de que el contenedor esté arriba el día del cierre de mes.

=== LO QUE ESTA HERRAMIENTA VIENE A ARREGLAR ===

El 2026-07-26 se cargó a mano un mapa de presentaciones con códigos
INVENTADOS —tableta=1 cuando el catálogo dice 66—, y el MUV lo aceptó porque
ese campo lo valida como rango (1 a 999) y no contra la tabla. Un dato falso
que pasa la validación es peor que uno que la rompe: nadie lo nota.

Nada de lo que sale de aquí se escribe a mano.
"""
from __future__ import annotations

import gzip
import json
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
DESTINO = RAIZ / "datos" / "maestros_muv"

CONTENEDOR = "fevrips-db"
BASE = "FevRipsApiLocal"
SQLCMD = "/opt/mssql-tools/bin/sqlcmd"

#: Clave fijada por MinSalud en el compose que reparte. No es un secreto
#: nuestro y el 1433 no se publica al host: lo que protege la base es que solo
#: se llegue a ella por la red interna de docker.
CLAVE_SA = "FEVRIPS1*"

#: nombre corto -> (tabla, columnas, filtro). El filtro NO es cosmético:
#: `REFs_UPR` trae FRASCO dos veces —13 habilitado y 40 deshabilitado— y
#: quedarse con el primero daría el que ya no vale.
TABLAS = {
    "UPR": ("Rips.REFs_UPR", "Codigo, Nombre", "Habilitado = 1"),
    "CUPS": ("Rips.REFs_CUPS", "Codigo, Nombre", "Habilitado = 1"),
    "CUPS_CIE10": ("Rips.REFs_CUPS_CIE10", "*", None),
    "FINALIDAD_CUPS": ("Rips.REFs_FinalidadCups", "*", None),
    # Estas tres traen columnas AplicaAC/AplicaAP/AplicaAU/AplicaAH y, la
    # primera, Sexo y rango de edad: un mismo código vale en unos archivos y no
    # en otros. Se traen enteras porque ahí está lo que decide RVC059, y
    # recortarlas a «Codigo, Nombre» sería tirar justo la parte útil.
    "FINALIDAD_TEC": ("Rips.REFs_FinalidadTecnologiaSalud", "*", None),
    "CAUSA_MOTIVO": ("Rips.REFs_CausaMotivoAtencion", "*", None),
    "VIA_INGRESO": ("Rips.REFs_ViaIngresoServicioSalud", "*", None),
    "TIPO_DOC": ("Rips.REFs_TipoDoc", "*", None),
}


class ContenedorNoDisponible(RuntimeError):
    """El validador no está levantado. No se usa una copia vieja en su lugar."""


class ConsultaMal(RuntimeError):
    """La consulta está mal escrita. El contenedor respondió perfectamente.

    Se separa de `ContenedorNoDisponible` porque decirle a alguien «levanta el
    validador» cuando el validador está arriba y lo que falla es un nombre de
    columna manda a buscar donde no es. Es el mismo error que el cliente del
    MUV cometía con el HTTP 400: tomar una respuesta legítima por una avería.
    """


def consultar(sql: str) -> list[dict]:
    """Ejecuta y devuelve filas. Usa `FOR JSON` para no adivinar separadores.

    Un `-s"|"` se rompe en cuanto un nombre trae una barra, y los nombres de
    presentación incluyen cosas como «TAMBOR - BOMBO». El propio SQL Server
    serializa mejor que cualquier partido de texto que escribamos aquí.
    """
    orden = [
        "docker", "exec", CONTENEDOR, SQLCMD,
        "-S", "localhost", "-U", "sa", "-P", CLAVE_SA,
        # `-y 0` = ancho sin límite, para que el JSON no venga cortado.
        # No admite compañía: sqlcmd declara `-y` incompatible con `-W` y con
        # `-h`. La cabecera se quita después, al pegar las líneas.
        "-d", BASE, "-y", "0",
        "-Q", f"SET NOCOUNT ON; {sql} FOR JSON PATH",
    ]
    try:
        r = subprocess.run(orden, capture_output=True, text=True, timeout=180)
    except FileNotFoundError as e:
        raise ContenedorNoDisponible("no hay `docker` en el PATH") from e
    if r.returncode != 0:
        raise ContenedorNoDisponible(
            f"el contenedor `{CONTENEDOR}` no respondió.\n{r.stderr.strip()}")

    # SQL Server informa los errores de consulta por la salida normal y con
    # código 0. Sin esto, un nombre de columna mal escrito se leía como
    # «respuesta ininteligible» y mandaba a revisar el contenedor.
    if "Msg " in r.stdout and "Level " in r.stdout:
        detalle = next((l.strip() for l in r.stdout.splitlines()
                        if "Invalid" in l or "Msg " in l), r.stdout[:200])
        raise ConsultaMal(detalle)

    # Se descarta todo lo que no sea el JSON: sqlcmd antepone la cabecera de
    # la columna y una línea de guiones, y `-h -1` no se puede usar con `-y 0`.
    lineas = [l.strip() for l in r.stdout.splitlines() if l.strip()]
    crudo = "".join(l for l in lineas if not set(l) <= {"-"}
                    and not l.startswith("JSON_"))
    if not crudo:
        return []
    try:
        return json.loads(crudo)
    except json.JSONDecodeError as e:
        raise ContenedorNoDisponible(
            f"respuesta ininteligible del contenedor: {crudo[:120]}") from e


def extraer(nombre: str) -> Path:
    tabla, columnas, filtro = TABLAS[nombre]
    sql = f"SELECT {columnas} FROM {tabla}"
    if filtro:
        sql += f" WHERE {filtro}"
    filas = consultar(sql)
    if not filas:
        # Una tabla vacía no es un maestro: es el contenedor a medio
        # sincronizar. Escribirla dejaría un catálogo que rechaza todo.
        raise ContenedorNoDisponible(
            f"{tabla} vino vacía. ¿Terminó de cargar los catálogos?")

    datos = {
        "fuente": f"{tabla} (contenedor {CONTENEDOR} del validador FEV-RIPS)",
        "filtro": filtro,
        "total": len(filas),
        "filas": filas,
    }
    DESTINO.mkdir(parents=True, exist_ok=True)
    texto = json.dumps(datos, ensure_ascii=False, separators=(",", ":"))

    # Se comprime solo lo que lo merece. 73 presentaciones en gzip son 73
    # presentaciones que nadie puede leer con `less` cuando algo va mal.
    if len(texto) > 100_000:
        ruta = DESTINO / f"{nombre.lower()}.json.gz"
        with gzip.open(ruta, "wt", encoding="utf-8") as f:
            f.write(texto)
    else:
        ruta = DESTINO / f"{nombre.lower()}.json"
        ruta.write_text(texto, encoding="utf-8")
    return ruta


def main() -> int:
    args = [a for a in sys.argv[1:] if not a.startswith("-")]
    todas = "--todas" in sys.argv

    if not args and not todas:
        print(__doc__)
        print("Tablas disponibles:")
        for n, (t, _, f) in TABLAS.items():
            print(f"  {n:<16} {t}{'  [' + f + ']' if f else ''}")
        return 0

    for nombre in (TABLAS if todas else args):
        if nombre not in TABLAS:
            print(f"«{nombre}» no está en la lista. Corre sin argumentos para "
                  f"verlas.", file=sys.stderr)
            return 2
        try:
            ruta = extraer(nombre)
        except ConsultaMal as e:
            print(f"\n{nombre}: la consulta está mal, no el contenedor.\n  {e}\n"
                  f"Revisa las columnas declaradas en TABLAS.", file=sys.stderr)
            return 4
        except ContenedorNoDisponible as e:
            print(f"\n{nombre}: {e}\n\nNo se ha escrito nada. Levanta el "
                  f"validador con `docker compose up -d` en muv/.",
                  file=sys.stderr)
            return 3
        datos = json.loads(
            gzip.open(ruta, "rt", encoding="utf-8").read()
            if ruta.suffix == ".gz" else ruta.read_text(encoding="utf-8"))
        print(f"  {nombre:<16} {datos['total']:>7,} filas  ->  "
              f"{ruta.relative_to(RAIZ)} ({ruta.stat().st_size / 1024:.0f} KB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
