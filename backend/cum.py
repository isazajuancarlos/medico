"""CatalogoCUMs: los códigos de medicamento del INVIMA, para el RIPS.

Hasta ahora `codTecnologiaSalud` salía de `Medicamento.codigo_cum` sin
comprobar nada, con `"00000000"` de relleno cuando faltaba. Ese valor tiene la
forma de un CUM y no lo es: pasa la validación de estructura y hace rechazar la
factura en el MUV, que sí valida contra el listado.

=== POR QUÉ VIENE EMPAQUETADO Y NO SE CONSULTA POR RED ===

`datos/cum.json.gz` se genera del CatalogoCUMs con
`herramientas/generar_cum.py` y se versiona con el código. Un consultorio en
zona rural no puede depender de que web.sispro.gov.co esté arriba el día del
cierre de mes para saber si un código existe. Regenerarlo es un paso
deliberado, igual que con DIVIPOLA.

Son 65.997 códigos en 2,6 MB comprimidos: cabe.

=== LO QUE ESTE CATÁLOGO NO ES ===

La Resolución 948, campo **M08** (`codTecnologiaSalud`), pide el **IUM** —
Identificador Único de Medicamento — y solo admite el CUM «para los
medicamentos que no cuentan con IUM». La tabla de IUM es otra fuente de SISPRO
y **no está cargada**. Mientras no lo esté, esto cubre el camino de respaldo que
la norma permite, no el preferente. Se dice aquí para que nadie suponga que
validar contra este catálogo equivale a cumplir M08 en su forma preferida.

=== ACTIVO NO ES LO MISMO QUE EXISTE ===

15.046 de los 65.997 códigos están inactivos: el medicamento salió del mercado
pero el código existió. Se conservan, marcados, porque «ese CUM no existe» y
«ese CUM está inactivo» mandan al facturador a buscar cosas distintas — un
error de digitación o un medicamento descontinuado. `validar` los acepta y
`validar_activo` no; quien llama elige, y la diferencia queda escrita.
"""
from __future__ import annotations

import gzip
import json
from functools import lru_cache
from pathlib import Path
from typing import NamedTuple, Optional

RUTA = Path(__file__).resolve().parent / "datos" / "cum.json.gz"


class CumNoValido(ValueError):
    """El código no está en el catálogo. No se sustituye por ninguno."""


class Medicamento(NamedTuple):
    codigo: str
    nombre: str
    forma_farmaceutica: str
    titular: str
    via_administracion: str
    #: Falso si TODOS sus registros están inactivos en el catálogo.
    activo: bool
    #: Principios activos con su cantidad: «HIDROCLOROTIAZIDA 12,5 mg».
    principios: tuple[str, ...]

    @property
    def concentracion(self) -> str:
        """Los principios activos en una línea, como los pide una etiqueta."""
        return " + ".join(self.principios)


@lru_cache(maxsize=1)
def _cargar() -> dict[str, Medicamento]:
    with gzip.open(RUTA, "rt", encoding="utf-8") as f:
        datos = json.load(f)
    return {
        codigo: Medicamento(
            codigo=codigo,
            nombre=p["n"],
            forma_farmaceutica=p["f"],
            titular=p["t"],
            via_administracion=p["v"],
            activo=p["a"],
            principios=tuple(p["p"]),
        )
        for codigo, p in datos["productos"].items()
    }


def normalizar(codigo: str | None) -> str:
    """Quita espacios y unifica el separador.

    Se acepta `20048021 41` y `20048021_41` además de `20048021-41` porque es
    lo que llega de un sistema viejo o de una hoja de cálculo. Lo que NO se
    hace es completar con ceros ni inventar el consecutivo: un CUM al que le
    falta una parte no es un CUM incompleto, es otro medicamento.
    """
    if codigo is None:
        return ""
    texto = " ".join(str(codigo).split())
    if not texto:
        return ""
    for separador in ("_", " "):
        texto = texto.replace(separador, "-")
    return texto


def buscar(codigo: str | None) -> Optional[Medicamento]:
    """El medicamento, o `None`."""
    return _cargar().get(normalizar(codigo))


def validar(codigo: str | None) -> str:
    """Devuelve el código normalizado, o levanta.

    No devuelve un valor por defecto a propósito. `"00000000"` tiene forma de
    CUM, pasa cualquier comprobación de estructura y hace rechazar la factura
    entera en el MUV, que valida contra el listado del INVIMA. Un dato ausente
    que se rellena deja de notarse.
    """
    normalizado = normalizar(codigo)
    if not normalizado:
        raise CumNoValido(
            "Falta el código del medicamento (CUM). Es obligatorio en el RIPS "
            "y no tiene valor por defecto: uno inventado hace rechazar la "
            "factura completa."
        )
    if buscar(normalizado) is None:
        raise CumNoValido(
            f"El código «{normalizado}» no está en el CatalogoCUMs del INVIMA. "
            f"Revise el dato: el MUV lo valida contra ese listado."
        )
    return normalizado


def validar_activo(codigo: str | None) -> str:
    """Como `validar`, pero además exige que el CUM siga activo.

    Se separa de `validar` porque son dos diagnósticos distintos y llevan a
    acciones distintas: uno se corrige digitando bien, el otro cambiando el
    medicamento. Confundirlos manda al facturador a buscar donde no es.
    """
    normalizado = validar(codigo)
    medicamento = buscar(normalizado)
    if medicamento is not None and not medicamento.activo:
        raise CumNoValido(
            f"El CUM «{normalizado}» ({medicamento.nombre}) está INACTIVO en el "
            f"catálogo del INVIMA: el registro sanitario ya no ampara ese "
            f"producto. Existe, pero no debería dispensarse ni facturarse."
        )
    return normalizado


def buscar_por_nombre(texto: str, limite: int = 20) -> list[Medicamento]:
    """Busca por nombre comercial o por principio activo.

    Por principio activo también porque el médico piensa en «losartán», no en
    la marca, y obligarlo a saber el nombre comercial es la clase de fricción
    que se rodea escribiendo cualquier cosa.
    """
    aguja = " ".join(texto.split()).upper()
    if not aguja:
        return []
    salida = []
    for m in _cargar().values():
        if aguja in m.nombre.upper() or any(aguja in p.upper() for p in m.principios):
            salida.append(m)
            if len(salida) >= limite:
                break
    return salida


def total() -> int:
    """Cuántos códigos trae el catálogo cargado."""
    return len(_cargar())


def total_activos() -> int:
    return sum(1 for m in _cargar().values() if m.activo)
