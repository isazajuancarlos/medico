#!/usr/bin/env python3
"""Extrae el código fuente del sistema original en Visual FoxPro.

Los formularios (.scx), librerías de clases (.vcx) e informes (.frx) de VFP son
tablas DBF con un archivo de memos aparte (.sct/.vct/.frt). No hace falta tener
Visual FoxPro: basta con renombrar el par a .dbf/.fpt y leerlo con cualquier
lector de DBF. El código de cada objeto vive en el campo memo METHODS.

    pip install dbfread
    python vfp_lector.py /mnt/data/medfuente --salida ./extraido
    python vfp_lector.py /mnt/data/medfuente --resumen

NO TOCA DATOS. Lee únicamente los contenedores de código y diseño. Los .dbf de
datos del sistema quedan fuera a propósito: son historias clínicas.
"""
import argparse
import glob
import os
import re
import shutil
import tempfile

from dbfread import DBF, FieldParser

PAREJA = {
    ".scx": ".sct",   # formularios
    ".vcx": ".vct",   # librerías de clases
    ".frx": ".frt",   # informes
    ".mnx": ".mnt",   # menús
    ".pjx": ".pjt",   # proyecto
}


class ParserTolerante(FieldParser):
    """dbfread es más estricto que Visual FoxPro, y por eso se atraganta.

    VFP deja los campos lógicos SIN INICIALIZAR escritos como `\\x00`, que no es
    ni verdadero ni falso ni espacio. dbfread levanta
    `ValueError: Illegal value for logical field` y se pierde el archivo
    entero — los menús (.mnx) no se pueden leer sin esto. Mismo criterio para
    fechas y números corruptos: un campo ilegible no debe costar el contenedor
    completo, que es lo que venimos a leer.
    """

    def parseL(self, field, data):
        if data in (b"\x00", b" ", b"", b"?"):
            return None
        return data.strip().upper() in (b"Y", b"T")

    def parseD(self, field, data):
        try:
            return super().parseD(field, data)
        except Exception:
            return None

    def parseN(self, field, data):
        try:
            return super().parseN(field, data)
        except Exception:
            return None


def abrir(ruta):
    """Devuelve los registros de un contenedor VFP como lista de dicts."""
    base, ext = os.path.splitext(ruta)
    memo_ext = PAREJA[ext.lower()]
    memo = base + memo_ext
    if not os.path.exists(memo):
        memo = base + memo_ext.upper()
    if not os.path.exists(memo):
        raise FileNotFoundError(f"falta el archivo de memos de {ruta}")

    tmp = tempfile.mkdtemp()
    try:
        d = os.path.join(tmp, "x.dbf")
        f = os.path.join(tmp, "x.fpt")
        shutil.copy(ruta, d)
        shutil.copy(memo, f)
        # latin-1 y replace: son archivos de 2007, con acentos de DOS/Windows
        # y bytes sueltos. Preferimos leer todo con algún carácter dudoso a
        # perder un formulario entero por un byte.
        return list(DBF(d, encoding="latin-1", char_decode_errors="replace",
                        parserclass=ParserTolerante))
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def codigo_de(registro) -> str:
    return (registro.get("METHODS") or "").strip()


def contenedores(carpeta):
    for nombre in sorted(os.listdir(carpeta)):
        if os.path.splitext(nombre)[1].lower() in PAREJA:
            yield os.path.join(carpeta, nombre)


def extraer(carpeta, salida):
    os.makedirs(salida, exist_ok=True)
    total_obj = total_bytes = 0
    for ruta in contenedores(carpeta):
        nombre = os.path.basename(ruta)
        try:
            regs = abrir(ruta)
        except Exception as e:
            print(f"  ⚠️  {nombre}: {e}")
            continue

        partes = []
        for r in regs:
            cod = codigo_de(r)
            if not cod:
                continue
            obj = (r.get("OBJNAME") or "").strip()
            clase = (r.get("BASECLASS") or r.get("CLASS") or "").strip()
            padre = (r.get("PARENT") or "").strip()
            ruta_obj = f"{padre}.{obj}" if padre else obj
            partes.append(f"* ===== {ruta_obj}  [{clase}] =====\n{cod}\n")
            total_obj += 1

        if partes:
            destino = os.path.join(salida, nombre.rsplit(".", 1)[0] + ".prg.txt")
            texto = f"* Extraído de {nombre} — {len(partes)} objetos con código\n\n"
            texto += "\n".join(partes)
            with open(destino, "w", encoding="utf-8") as fh:
                fh.write(texto)
            total_bytes += len(texto)
    print(f"\n✅ {total_obj} objetos con código -> {salida} ({total_bytes//1024} KB)")


def resumen(carpeta):
    filas = []
    for ruta in contenedores(carpeta):
        try:
            regs = abrir(ruta)
        except Exception:
            continue
        cod = sum(len(codigo_de(r)) for r in regs)
        filas.append((os.path.basename(ruta), len(regs), cod))

    filas.sort(key=lambda x: -x[2])
    print(f"{'contenedor':34} {'objetos':>8} {'código':>12}")
    print("-" * 56)
    for nombre, objs, cod in filas:
        print(f"{nombre:34} {objs:8} {cod:11,}b")
    print("-" * 56)
    print(f"{'TOTAL':34} {sum(f[1] for f in filas):8} "
          f"{sum(f[2] for f in filas):11,}b")


def _forma_invocada(comando: str):
    """Nombres de formulario dentro de un `DO FORM ...`.

    Hay TRES formas de invocar en este sistema, y hay que cubrir las tres o el
    análisis de alcance miente:

      1. `DO FORM pacientes`
      2. `DO FORM c:\\aprograms\\medico\\FORMS\\pacientes`  (ruta completa: el
         nombre es el ÚLTIMO segmento, no lo que sigue a FORM)
      3. `DO FORM (ruta_forms+"frmbodegas")`  (ruta CONSTRUIDA en ejecución;
         es la forma más usada en los menús internos, y una expresión regular
         que corte en el paréntesis devuelve basura)

    La 3 es la que hace inútil cualquier conteo ingenuo de "formularios
    huérfanos": el nombre solo existe dentro de una cadena.
    """
    texto = comando or ""

    # Caso 3 primero: nombre entre comillas dentro de una expresión.
    for m in re.finditer(r'DO\s+FORM\s*\(([^)]*)\)', texto, re.I):
        for lit in re.findall(r'["\']([^"\']+)["\']', m.group(1)):
            nombre = re.split(r"[\\/]", lit)[-1].lower().replace(".scx", "").strip()
            if nombre:
                yield nombre

    # Casos 1 y 2: literal, sin paréntesis.
    for m in re.finditer(r"DO\s+FORM\s+([^\s(][^\s)]*)", texto, re.I):
        nombre = re.split(r"[\\/]", m.group(1))[-1].lower().replace(".scx", "").strip()
        if nombre:
            yield nombre


def clasificar(carpeta):
    """Separa los formularios en: entradas de menú, sub-formularios y sin uso.

    Es lo que acota el trabajo de una migración: no se reimplementan 99
    pantallas, se reimplementan las que el sistema realmente ofrece más las que
    ellas abren.

    CUIDADO al interpretar "sin referencia": no prueba que estén muertos. VFP
    permite invocar por nombre construido en tiempo de ejecución, y hay .prg
    fuera de este análisis. Confirmar con el dueño del sistema antes de
    descartar nada.
    """
    formularios = {
        os.path.basename(p)[:-4].lower()
        for p in glob.glob(os.path.join(carpeta, "*.scx"))
    }

    en_menu, etiqueta = set(), {}
    for ruta in glob.glob(os.path.join(carpeta, "*.mnx")):
        for r in abrir(ruta):
            prompt = str(r.get("PROMPT") or "").replace("\\<", "").strip()
            for f in _forma_invocada(str(r.get("COMMAND") or "")):
                en_menu.add(f)
                etiqueta[f] = prompt

    # Hay que barrer TODO lo que contiene código, no solo los formularios: las
    # librerías de clases (.vcx) y los programas sueltos (.prg) también abren
    # pantallas. Mirar solo los .scx dejaba a frmbodegas y compañía marcados
    # como huérfanos cuando sí se invocan.
    desde_otro = set()
    for ruta in contenedores(carpeta):
        if os.path.splitext(ruta)[1].lower() == ".mnx":
            continue  # el menú se cuenta aparte, como punto de entrada
        origen = os.path.basename(ruta).rsplit(".", 1)[0].lower()
        try:
            regs = abrir(ruta)
        except Exception:
            continue
        for r in regs:
            for f in _forma_invocada(codigo_de(r)):
                if f != origen:
                    desde_otro.add(f)

    for ruta in glob.glob(os.path.join(carpeta, "*.prg")):
        origen = os.path.basename(ruta)[:-4].lower()
        try:
            with open(ruta, encoding="latin-1", errors="replace") as fh:
                texto = fh.read()
        except Exception:
            continue
        for f in _forma_invocada(texto):
            if f != origen:
                desde_otro.add(f)

    rotas = sorted(en_menu - formularios)
    en_menu &= formularios
    subform = (desde_otro & formularios) - en_menu
    sin_uso = formularios - en_menu - subform

    print(f"formularios: {len(formularios)}")
    print(f"  entradas de menú (el sistema en operación): {len(en_menu)}")
    print(f"  sub-formularios (los abre otro):            {len(subform)}")
    print(f"  sin referencia alguna:                      {len(sin_uso)}")
    if rotas:
        print(f"\n⚠️  el menú llama a formularios que NO existen: {', '.join(rotas)}")

    print("\n=== ENTRADAS DE MENÚ ===")
    for f in sorted(en_menu):
        print(f"  {etiqueta.get(f, ''):28} <- {f}")
    print(f"\n=== SIN REFERENCIA ({len(sin_uso)}) — verificar antes de descartar ===")
    print("  " + ", ".join(sorted(sin_uso)))


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("carpeta", help="carpeta del sistema VFP original")
    ap.add_argument("--salida", default="extraido", help="carpeta destino")
    ap.add_argument("--resumen", action="store_true", help="solo contar, no extraer")
    ap.add_argument("--clasificar", action="store_true",
                    help="separa formularios en menú / sub-formulario / sin uso")
    args = ap.parse_args()

    if args.clasificar:
        clasificar(args.carpeta)
    elif args.resumen:
        resumen(args.carpeta)
    else:
        extraer(args.carpeta, args.salida)
