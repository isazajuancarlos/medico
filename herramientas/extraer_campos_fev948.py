"""Extrae del Documento Técnico 1 los campos que la Resolución 948 exige.

Se ejecuta a mano y deja `backend/datos/normativa/campos_fev948.json`, que es
lo que compara `test_campos_fev948.py`. La norma deja de ser prosa que alguien
recuerda y pasa a ser un dato contra el que fallan las pruebas.

=== POR QUÉ DOS FUENTES Y NO UNA ===

El PDF dice lo mismo dos veces y de dos formas:

  - una TABLA numerada por sección (C01…C30 consultas, P## procedimientos,
    R## urgencias, H## hospitalización, N## recién nacidos, M## medicamentos,
    S## otros servicios). Da la CUENTA exacta, pero `pdftotext` parte los
    nombres largos por el ancho de columna: «codTecnologiaSa».
  - un EJEMPLO JSON por sección. Da los nombres COMPLETOS y en orden, pero no
    imprime todos los campos.

Se usa el ejemplo para los nombres y la tabla para la cuenta. Si las dos no
coinciden, el extractor falla en vez de escribir. Una sola fuente ya me engañó
antes: ver [[directiva-medir-lo-que-se-afirma]].

=== LAS CUATRO TRAMPAS DE LA EXTRACCIÓN ===

1. Cada sección incluye ejemplos ANIDADOS de los otros arreglos. Se corta al
   ver la clave de otro arreglo o del sobre.
2. El capítulo 4.3.7 trae un ejemplo GENERAL con los siete anidados. Por eso
   cada arreglo se busca solo dentro de su propia sección.
3. El resumen estructural del 4.3 también parece un ejemplo. Se descarta
   exigiendo que el bloque empiece por `codPrestador`, como todo servicio.
4. Los ejemplos de procedimientos y hospitalización NO imprimen el par de
   complicación en CIE-11, aunque las tablas P y H sí lo listan. Se añade a
   mano tras `codComplicacion`; es el único añadido manual y por eso está aquí
   escrito.

**No filtrar por subcadena.** Se intentó descartar `codSexo` por estar dentro
de `codSexoBiologico` y el filtro se comió `codDiagnosticoPrincipal`, que está
dentro de `codDiagnosticoPrincipalCIE11`. Es el mismo error de prefijos que ya
produjo una vez «once campos inventados» que no existían.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
PDF = RAIZ / "backend" / "datos" / "normativa" / "res948-doc-tecnico-1.pdf"
SALIDA = RAIZ / "backend" / "datos" / "normativa" / "campos_fev948.json"

#: Rangos de línea de cada sección 4.3.x en el texto de `pdftotext -layout`.
SECCIONES = {
    "consultas": (1226, 1983),
    "procedimientos": (1983, 2522),
    "urgencias": (2522, 3050),
    "hospitalizacion": (3050, 3610),
    "recienNacidos": (3610, 4039),
    "medicamentos": (4039, 4763),
    "otrosServicios": (4763, 5283),
}

#: Cuántos campos declara la tabla numerada de cada sección. Es el control.
CUENTA = {
    "consultas": 30, "procedimientos": 27, "urgencias": 25,
    "hospitalizacion": 30, "recienNacidos": 18, "medicamentos": 28,
    "otrosServicios": 18,
}

#: Claves del sobre, que no son campos de servicio.
SOBRE = {"numDocumentoIdObligado", "numFactura", "tipoNota", "numNota"}

CLAVE = re.compile(r'\s*"([A-Za-z0-9]+)"\s*:')


def texto_del_pdf() -> list[str]:
    return subprocess.run(
        ["pdftotext", "-layout", str(PDF), "-"],
        capture_output=True, text=True, check=True,
    ).stdout.split("\n")


def extraer(lineas: list[str]) -> dict[str, list[str]]:
    corte = set(SECCIONES) | SOBRE
    forma: dict[str, list[str]] = {}
    for arreglo, (ini, fin) in SECCIONES.items():
        mejor: list[str] = []
        for i in range(ini, min(fin, len(lineas))):
            if not re.match(rf'\s*"{arreglo}":\s*\[\s*$', lineas[i]):
                continue
            claves: list[str] = []
            for linea in lineas[i + 1:min(i + 200, fin)]:
                m = CLAVE.match(linea)
                if not m:
                    continue
                k = m.group(1)
                if k in corte:
                    break
                if k not in claves:
                    claves.append(k)
            if claves[:1] != ["codPrestador"]:
                continue                      # es el resumen, no un ejemplo
            if len(claves) > len(mejor):
                mejor = claves
        forma[arreglo] = mejor

    for arreglo in ("procedimientos", "hospitalizacion"):
        i = forma[arreglo].index("codComplicacion")
        forma[arreglo][i + 1:i + 1] = ["codComplicacionCIE11",
                                       "nomCodComplicacionCIE11"]
    return forma


def main() -> int:
    forma = extraer(texto_del_pdf())
    malas = {a: (len(forma[a]), CUENTA[a])
             for a in SECCIONES if len(forma[a]) != CUENTA[a]}
    if malas:
        print("ERROR: el ejemplo y la tabla no coinciden en "
              f"{malas}. No se escribe nada.", file=sys.stderr)
        return 1
    SALIDA.write_text(json.dumps({
        "fuente": ("Resolución 948 de 2026 — Documento Técnico 1, "
                   "versión 002 (1 de julio de 2026)"),
        "archivo_fuente": PDF.name,
        "verificacion": ("Cada lista sale del ejemplo JSON de su propia sección "
                         "y su LONGITUD cuadra con la tabla numerada del mismo "
                         "documento. Dos extracciones independientes."),
        "arreglos": forma,
    }, ensure_ascii=False, indent=1), encoding="utf-8")
    cie11 = {c for v in forma.values() for c in v if c.endswith("CIE11")}
    print(f"{sum(len(v) for v in forma.values())} campos en 7 arreglos "
          f"({len(cie11)} distintos en CIE-11) → {SALIDA}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
