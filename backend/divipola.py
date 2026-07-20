"""DIVIPOLA: los municipios y departamentos de Colombia, para el RIPS.

Hasta ahora `Paciente.cod_municipio` era texto libre con `"05001"` de valor por
defecto. Eso pasaba la validación de estructura y reportaba a todos los
pacientes del país como residentes de Medellín.

=== POR QUÉ VIENE EMPAQUETADO Y NO SE CONSULTA POR RED ===

El archivo `datos/divipola.json` se genera de las publicaciones oficiales y se
versiona con el código. Un consultorio en zona rural no puede depender de que
haya internet para saber si un código de municipio es válido, y menos aún de
que datos.gov.co esté arriba el día del cierre de mes. La lista cambia cada
varios años; regenerarla es un paso deliberado, no una llamada en caliente.

=== EL CERO A LA IZQUIERDA NO ES COSMÉTICO ===

`codMunicipioResidencia` va de CINCO dígitos. Antioquia es el departamento 05 y
Atlántico el 08: **148 municipios —el 13 % del país— tienen código que empieza
por cero**, Medellín entre ellos.

El conjunto de datos de MinSalud los publica SIN ese cero (`5001` en vez de
`05001`), porque en algún punto pasaron por una hoja de cálculo que los leyó
como números. Cargar de ahí sin normalizar deja mal reportado a todo paciente
de Antioquia y Atlántico. Aquí se normaliza a cinco dígitos siempre, y hay una
prueba que lo fija.

=== LAS DOS FUENTES OFICIALES NO COINCIDEN, Y NO ES UN ERROR ===

Comparando el DIVIPOLA del DANE con el de MinSalud quedan dos diferencias
reales, no de formato:

    27086 Belén de Bajirá (MinSalud)  ↔  27493 Nuevo Belén de Bajirá (DANE)
    94663 Mapiripana (MinSalud)       ↔  94885 La Guadalupe (DANE)

La primera es el pleito de límites entre Chocó y Antioquia, que lleva décadas.
La segunda, una antigua área no municipalizada del Guainía.

Se aceptan LOS CUATRO códigos. Rechazar los de MinSalud rompería el envío al
MUV, que valida contra su propia lista; rechazar los del DANE rompería el cruce
con cualquier estadística oficial. Un prestador no tiene por qué arbitrar un
diferendo territorial para poder facturar.
"""

from __future__ import annotations

import json
from functools import lru_cache
from pathlib import Path
from typing import NamedTuple, Optional

RUTA = Path(__file__).resolve().parent / "datos" / "divipola.json"

#: Longitud del código de municipio en el RIPS (Resolución 2275 de 2023).
LARGO_CODIGO = 5


class MunicipioNoValido(ValueError):
    """El código no está en DIVIPOLA. No se sustituye por ninguno."""


class Municipio(NamedTuple):
    codigo: str
    nombre: str
    departamento: str
    nombre_departamento: str
    tipo: str
    #: Si está en la lista de MinSalud, que es contra la que valida el MUV.
    en_minsalud: bool


@lru_cache(maxsize=1)
def _cargar() -> tuple[dict[str, Municipio], dict[str, str]]:
    with open(RUTA, encoding="utf-8") as f:
        datos = json.load(f)
    deptos = datos["departamentos"]
    municipios = {
        m["codigo"]: Municipio(
            codigo=m["codigo"],
            nombre=m["nombre"],
            departamento=m["departamento"],
            nombre_departamento=deptos.get(m["departamento"], "—"),
            tipo=m["tipo"],
            en_minsalud=m["en_minsalud"],
        )
        for m in datos["municipios"]
    }
    return municipios, deptos


def normalizar(codigo: str | int | None) -> str:
    """Deja el código en cinco dígitos.

    Acepta enteros y cadenas cortas porque es exactamente lo que llega de una
    hoja de cálculo o de un sistema viejo: `5001` en vez de `"05001"`. Lo que
    NO hace es inventar: si viene vacío, sale vacío y `validar` lo rechaza.
    """
    if codigo is None:
        return ""
    texto = str(codigo).strip()
    if not texto:
        return ""
    return texto.zfill(LARGO_CODIGO)


def buscar(codigo: str | int | None) -> Optional[Municipio]:
    """El municipio, o `None`. Normaliza antes de buscar."""
    municipios, _ = _cargar()
    return municipios.get(normalizar(codigo))


def validar(codigo: str | int | None) -> str:
    """Devuelve el código normalizado, o levanta.

    No devuelve un valor por defecto a propósito: un municipio equivocado en el
    RIPS no lo rechaza el validador —tiene la forma correcta— y queda como un
    dato falso en la estadística de residencia.
    """
    normalizado = normalizar(codigo)
    if not normalizado:
        raise MunicipioNoValido(
            "Falta el código de municipio de residencia. Es obligatorio en el "
            "RIPS y no tiene valor por defecto: poner uno reportaría al "
            "paciente como residente de un sitio en el que no vive."
        )
    if buscar(normalizado) is None:
        raise MunicipioNoValido(
            f"El código de municipio «{normalizado}» no está en DIVIPOLA. "
            f"Revise el dato: el RIPS lo valida contra la lista oficial."
        )
    return normalizado


def departamentos() -> dict[str, str]:
    """Código de dos dígitos → nombre."""
    _, deptos = _cargar()
    return dict(deptos)


def municipios_de(departamento: str | int) -> list[Municipio]:
    """Los municipios de un departamento, ordenados por nombre.

    Es lo que necesita el formulario: se elige departamento y se filtra, en vez
    de buscar entre 1.124 opciones.
    """
    codigo = str(departamento).strip().zfill(2)
    municipios, _ = _cargar()
    return sorted(
        (m for m in municipios.values() if m.departamento == codigo),
        key=lambda m: m.nombre,
    )


def buscar_por_nombre(texto: str, limite: int = 20) -> list[Municipio]:
    """Búsqueda por nombre, sin tildes ni mayúsculas.

    Quien captura escribe «medellin», no «MEDELLÍN».
    """
    from unicodedata import normalize as _uni

    def plano(s: str) -> str:
        return "".join(
            c for c in _uni("NFD", s.lower()) if c.isalnum() or c.isspace()
        )

    aguja = plano(texto).strip()
    if not aguja:
        return []
    municipios, _ = _cargar()
    hallados = [m for m in municipios.values() if aguja in plano(m.nombre)]
    # Los que empiezan por el texto van primero: «bello» antes que «Rebello».
    hallados.sort(key=lambda m: (not plano(m.nombre).startswith(aguja), m.nombre))
    return hallados[:limite]


def total() -> int:
    municipios, _ = _cargar()
    return len(municipios)
