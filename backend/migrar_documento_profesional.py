#!/usr/bin/env python3
"""Añade a `usuario` el documento de identidad del profesional.

    python migrar_documento_profesional.py            # ver qué haría
    python migrar_documento_profesional.py --aplicar

POR QUÉ. El RIPS pide en cada bloque de servicios el documento del PROFESIONAL
que ordenó o realizó el procedimiento (Resolución 948, Documento Técnico 1,
campos P11/P12). Hasta ahora el generador ponía ahí el del PACIENTE, y el MUV
lo destapó rechazando las atenciones de menores: `RC` y `TI` no son documentos
que pueda tener un profesional.

NO SE RELLENA CON NADA. Las dos columnas quedan en NULL. Inventar el documento
de un profesional sería atribuir un acto médico a alguien, y copiar el del
paciente es exactamente el fallo que esta migración viene a arreglar. Quien
aparezca en un RIPS sin tenerlo hará fallar la exportación con un mensaje que
dice qué falta y en qué usuario, que es lo que se quiere.

Al terminar informa cuántos profesionales lo necesitan de verdad —los que han
atendido algo— para que quien cierra el mes sepa el tamaño de la tarea.
"""
from __future__ import annotations

import argparse
import sys

from sqlalchemy import inspect, text
from sqlalchemy.exc import OperationalError

from database import engine

COLUMNAS = {
    "tipo_documento": "VARCHAR",
    "numero_documento": "VARCHAR",
}


def faltantes() -> list[str]:
    existentes = {c["name"] for c in inspect(engine).get_columns("usuario")}
    return [c for c in COLUMNAS if c not in existentes]


def cuantos_lo_necesitan(con) -> int:
    """Profesionales que ya aparecen en alguna atención."""
    total = 0
    for tabla, columna in (
        ("historia", "medico_id"),
        ("atencionurgencias", "medico_id"),
        ("hospitalizacion", "medico_tratante_id"),
    ):
        try:
            fila = con.execute(text(
                f"SELECT COUNT(DISTINCT {columna}) FROM {tabla} "
                f"WHERE {columna} IS NOT NULL"
            )).scalar()
            total = max(total, int(fila or 0))
        except Exception:
            # La tabla puede no existir en una base recién creada. No es un
            # fallo de la migración: solo significa que aún no hay atenciones.
            continue
    return total


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--aplicar", action="store_true",
                    help="ejecuta los ALTER TABLE; sin esto solo informa")
    args = ap.parse_args()

    try:
        pendientes = faltantes()
    except OperationalError as e:
        # Sin esto, «la base no está levantada» salía como cien líneas de traza
        # de SQLAlchemy, y quien corre una migración a las once de la noche
        # necesita leer el problema en la primera línea, no en la última.
        print(f"No hay conexión con la base de datos.\n\n  {e.orig}\n\n"
              f"Levanta PostgreSQL (o apunta DATABASE_URL a donde esté) y "
              f"vuelve a intentarlo. No se ha modificado nada.", file=sys.stderr)
        return 3

    if not pendientes:
        print("Las dos columnas ya existen. Nada que hacer.")
        return 0

    print(f"Columnas por añadir a `usuario`: {', '.join(pendientes)}")
    if not args.aplicar:
        print("\nEn seco. Vuelve a llamar con --aplicar para ejecutarlo.")
        return 0

    with engine.begin() as con:
        for columna in pendientes:
            con.execute(text(
                f"ALTER TABLE usuario ADD COLUMN {columna} {COLUMNAS[columna]}"))
            print(f"  añadida {columna}")
        con.execute(text(
            "CREATE INDEX IF NOT EXISTS ix_usuario_numero_documento "
            "ON usuario (numero_documento)"))
        n = cuantos_lo_necesitan(con)

    print(f"\nListo. Quedan en NULL a propósito: no hay valor que suponer.")
    if n:
        print(f"Hay al menos {n} profesional(es) con atenciones registradas. "
              f"Hasta que tengan documento, generar su RIPS fallará diciendo "
              f"exactamente cuál falta.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
