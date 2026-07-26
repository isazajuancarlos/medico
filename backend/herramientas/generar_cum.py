"""Convierte el CatalogoCUMs del INVIMA en el artefacto que se versiona.

    python herramientas/generar_cum.py /home/juan/Documentos/cum.csv

Produce `datos/cum.json.gz`. Es un paso DELIBERADO, no una descarga en caliente:
igual que DIVIPOLA, un consultorio en zona rural no puede depender de que
web.sispro.gov.co esté arriba el día del cierre de mes para saber si un código
de medicamento existe.

# UNA FILA NO ES UN MEDICAMENTO

El CSV trae 157.789 filas y solo **65.997 CUM distintos**. La diferencia no son
duplicados a limpiar: el catálogo publica **una fila por principio activo**, así
que un combinado como CARDIK 160/5/12.5 ocupa tres —valsartán, amlodipino e
hidroclorotiazida—, cada una con su cantidad y su unidad.

Cargar sin agrupar reventaría contra el índice único de `codigo_cum`, y quedarse
con la primera fila daría a un combinado la concentración de uno solo de sus
componentes. Aquí se agrupan y la concentración se compone con todos.

# LOS INACTIVOS SE CONSERVAN, CON SU MARCA

35.325 de los 157.789 registros tienen `estadocum = Inactivo`. Se guardan igual,
marcados. Descartarlos haría que la validación respondiera «ese CUM no existe»
ante un código que sí existe y está inactivo, y son dos problemas distintos: uno
es un error de digitación y el otro es un medicamento que salió del mercado. Al
facturador le mandan a buscar cosas distintas.

# LO QUE ESTE CATÁLOGO NO ES

La Resolución 948 (campo M08, `codTecnologiaSalud`) pide el **IUM** —
Identificador Único de Medicamento — y solo admite el CUM «para los medicamentos
que no cuentan con IUM». La tabla de IUM es otra fuente de SISPRO y NO está
aquí. Mientras no se cargue, este catálogo cubre el camino de respaldo que la
norma permite, no el preferente. Queda dicho para que nadie suponga lo contrario.
"""
import csv
import gzip
import json
import sys
from collections import OrderedDict
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
DESTINO = RAIZ / "datos" / "cum.json.gz"


def _limpiar(texto) -> str:
    return " ".join((texto or "").split())


def _ingrediente(fila) -> str:
    """«HIDROCLOROTIAZIDA 12,5 mg» a partir de las columnas del catálogo."""
    nombre = _limpiar(fila.get("principioactivo"))
    cantidad = _limpiar(fila.get("cantidad"))
    unidad = _limpiar(fila.get("unidadmedida"))
    return " ".join(p for p in (nombre, cantidad, unidad) if p)


def construir(ruta_csv: Path) -> dict:
    productos: "OrderedDict[str, dict]" = OrderedDict()
    filas = 0

    with ruta_csv.open(encoding="utf-8", newline="") as f:
        for fila in csv.DictReader(f):
            filas += 1
            expediente = _limpiar(fila.get("expedientecum"))
            consecutivo = _limpiar(fila.get("consecutivocum"))
            if not expediente or not consecutivo:
                # Sin código no hay nada que catalogar. No se inventa uno.
                continue
            cum = f"{expediente}-{consecutivo}"

            registro = productos.get(cum)
            if registro is None:
                registro = productos[cum] = {
                    "n": _limpiar(fila.get("producto")),
                    "f": _limpiar(fila.get("formafarmaceutica")),
                    "t": _limpiar(fila.get("titular")),
                    "v": _limpiar(fila.get("viaadministracion")),
                    # `a` = activo. Basta un registro activo para que el CUM lo
                    # esté: el estado es del código, no de cada componente.
                    "a": False,
                    "p": [],
                }
            if _limpiar(fila.get("estadocum")).lower() == "activo":
                registro["a"] = True
            ing = _ingrediente(fila)
            if ing and ing not in registro["p"]:
                registro["p"].append(ing)

    return {
        "fuente": "CatalogoCUMs (INVIMA/SISPRO)",
        "filas_leidas": filas,
        "productos": productos,
    }


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    ruta = Path(sys.argv[1])
    if not ruta.exists():
        print(f"No existe {ruta}", file=sys.stderr)
        return 2

    datos = construir(ruta)
    DESTINO.parent.mkdir(exist_ok=True)
    with gzip.open(DESTINO, "wt", encoding="utf-8") as f:
        json.dump(datos, f, ensure_ascii=False, separators=(",", ":"))

    activos = sum(1 for p in datos["productos"].values() if p["a"])
    print(f"filas leídas:      {datos['filas_leidas']:,}")
    print(f"CUM distintos:     {len(datos['productos']):,}")
    print(f"  activos:         {activos:,}")
    print(f"  inactivos:       {len(datos['productos']) - activos:,}")
    print(f"escrito:           {DESTINO} ({DESTINO.stat().st_size / 1e6:.1f} MB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
