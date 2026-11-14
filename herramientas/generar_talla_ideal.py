#!/usr/bin/env python3
"""Transcribe `PESOTALLA1.DBF` del sistema en Visual FoxPro a JSON para el Rust.

    python3 herramientas/generar_talla_ideal.py

Deja `crates/medico-dominio/datos/talla_ideal.json`, que `antropometria.rs`
empotra con `include_str!`. Se ejecuta a mano: la tabla tiene 62 filas y no
cambia — es una referencia antropométrica del producto original, no un catálogo
que MinSalud publique.

=== POR QUÉ ES `PESOTALLA1` Y NO `PESOTALLA` ===

Son dos tablas distintas y el sistema vivo abre la equivocada:

    PESOTALLA1.DBF   EDAD, TIPO, TALLAIDEAL, PESOIDEAL, SEXO   62 filas  (+ .CDX)
    PESOTALLA.DBF    DIASINI, DIASFIN, TALLAIDEAL, PESOIDEAL, SEXO   47 filas

El formulario hace `SEEK` sobre EDAD, y `PESOTALLA` no tiene columna EDAD ni ese
índice, así que la búsqueda NO PUEDE acertar y la talla ideal sale siempre en 0.
O sea que aquí no hay un número del prestador con el que cuadrar: es la única
pieza del port que no se contrasta contra el sistema vivo, porque en el sistema
vivo nunca funcionó. Va escrito también en `antropometria.rs`, que es donde se
leería la duda.

=== POR QUÉ EN PYTHON, habiendo directiva 38 ===

Porque lee un DBF de Visual FoxPro, y el lector de DBF que este repositorio ya
tiene y ya ha usado es Python (`herramientas/vfp_lector.py`, `dbfread`). Escribir
esto en Rust obligaría a portar antes el lector de DBF para una tabla de 62 filas
que no vuelve a cambiar. Ésa es la frase que la directiva 38 pide que quede
escrita donde se lea.

=== LO QUE NO SE TOCA ===

La columna PESOIDEAL se transcribe también, aunque hoy no la consuma nadie: el
archivo es la copia verbatim de una tabla, y editar una tabla al copiarla es
justo lo que la regla «ningún código de catálogo se escribe a mano» prohíbe.
Quién la use es decisión aparte — ver la nota de `peso_ideal` sobre los menores
de 12 años.
"""
import json
import struct
from pathlib import Path

ORIGEN = Path("/mnt/data/aprograms/medico/data/PESOTALLA1.DBF")
RAIZ = Path(__file__).resolve().parent.parent
SALIDA = RAIZ / "crates" / "medico-dominio" / "datos" / "talla_ideal.json"

#: Lo que la columna TIPO significa. El original no lo documenta en ninguna
#: parte; sale de mirar los rangos: TIPO='M' va de 0 a 11 y TIPO='A' de 1 a 19,
#: y las tallas de 'M' (0,48–0,77 m) son de lactante.
TIPOS = {"M": "meses", "A": "anios"}


def leer_dbf(ruta: Path):
    """Las filas de un DBF de VFP, sin dependencias.

    Se parsea la cabecera a mano en vez de usar `dbfread` porque esta tabla no
    tiene memos ni campos lógicos —los dos motivos por los que `vfp_lector.py`
    necesita su parser tolerante—, y así la herramienta no arrastra nada.
    """
    b = ruta.read_bytes()
    n_registros, cabecera, largo = struct.unpack("<IHH", b[4:12])
    campos = []
    desp = 32
    while b[desp] != 0x0D:
        c = b[desp : desp + 32]
        campos.append((c[:11].split(b"\0")[0].decode("latin-1"), c[16]))
        desp += 32

    for i in range(n_registros):
        r = b[cabecera + i * largo : cabecera + (i + 1) * largo]
        if r[:1] == b"*":  # marcado como borrado
            continue
        pos = 1
        fila = {}
        for nombre, tam in campos:
            fila[nombre] = r[pos : pos + tam].decode("latin-1").strip()
            pos += tam
        yield fila


def main() -> None:
    if not ORIGEN.exists():
        raise SystemExit(
            f"no está {ORIGEN}. Es la fuente autoritativa del sistema original; "
            f"NO se sustituye por la copia aplanada de /mnt/data/medfuente."
        )

    filas = []
    for f in leer_dbf(ORIGEN):
        tipo = f["TIPO"]
        if tipo not in TIPOS:
            raise SystemExit(
                f"TIPO desconocido «{tipo}»: la tabla cambió y esta herramienta "
                f"no sabe leerla. No se adivina."
            )
        filas.append(
            {
                "edad": int(f["EDAD"]),
                "unidad": TIPOS[tipo],
                "sexo": f["SEXO"],
                "talla_ideal_m": float(f["TALLAIDEAL"]),
                "peso_ideal_kg": float(f["PESOIDEAL"]),
            }
        )

    # Orden estable, para que regenerar no produzca un diff de ruido.
    filas.sort(key=lambda x: (x["unidad"], x["sexo"], x["edad"]))

    claves = {(x["unidad"], x["sexo"], x["edad"]) for x in filas}
    if len(claves) != len(filas):
        raise SystemExit(
            "hay dos filas para la misma (unidad, sexo, edad): la tabla no "
            "puede resolverse a un solo valor y habría que elegir, que es lo "
            "que no se hace solo."
        )

    SALIDA.parent.mkdir(parents=True, exist_ok=True)
    SALIDA.write_text(
        json.dumps(
            {
                "fuente": "PESOTALLA1.DBF del sistema en Visual FoxPro "
                "(/mnt/data/aprograms/medico/data), transcrita por "
                "herramientas/generar_talla_ideal.py",
                "advertencia": "En el original esta tabla NUNCA se consultó: el "
                "formulario abre PESOTALLA, que no tiene columna EDAD ni ese "
                "índice, así que el SEEK no puede acertar y la talla ideal sale "
                "siempre en 0. No hay número del prestador con el que cuadrar.",
                "filas": filas,
            },
            ensure_ascii=False,
            indent=1,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"{len(filas)} filas -> {SALIDA.relative_to(RAIZ)}")


if __name__ == "__main__":
    main()
