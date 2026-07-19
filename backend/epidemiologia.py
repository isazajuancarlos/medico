"""Informes epidemiológicos, migrados del sistema en Visual FoxPro.

Reemplaza seis formularios de `repoNuevo` que el consultorio corre hoy a mano:
repomorbi, repocve, repoira, repoits, repocobertura y repoprocedimiento.

No son funciones opcionales: son obligaciones de reporte a la Secretaría de
Salud. Por eso el criterio aquí es FIDELIDAD, aunque algo del original parezca
mejorable — si los números no coinciden con los que el prestador viene
reportando, el informe nuevo no sirve para nada.

=== LAS DOS DECISIONES DE FIDELIDAD QUE HAY QUE CONOCER ===

**1. La edad se calcula como el original: `int(días / 365)`.**

No es la edad calendario. El VFP hacía `lnanos = INT((fechaEvento - fechaNac)/365)`
y eso ignora los bisiestos: quien tenga 20 años cumplidos puede clasificarse como
de 19 si acumuló suficientes 29 de febrero. Se conserva A PROPÓSITO, porque los
rangos etarios son la variable de corte de todos estos informes y "corregirlo"
movería personas de casilla y rompería la serie histórica del prestador.
`edad_calendario()` queda disponible para cuando se decida cambiar, y el cambio
debe ser una decisión explícita, no un arreglo silencioso.

**2. El evento de vigilancia se DERIVA del diagnóstico.**

El original guardaba un campo aparte (`exa_fisico_gen.cve1`) que el médico
diligenciaba. El sistema nuevo no lo tiene, pero el catálogo CVE mapea
CIE-10 -> evento, así que se deriva del diagnóstico principal. Es equivalente
mientras el médico registrara el CVE coherente con el diagnóstico. SI ALGUNA VEZ
REGISTRÓ UN CVE DISTINTO DEL DIAGNÓSTICO, los números cambiarán: hay que
preguntárselo al prestador antes de dar los informes por buenos.

=== FECHAS ===
El original filtra por la fecha del MOVIMIENTO DE HISTORIA (`mvto_historia.fecha`),
no por la del examen. Aquí equivale a `Historia.fecha`.
"""

from __future__ import annotations

import json
from collections import Counter, defaultdict
from datetime import date
from functools import lru_cache
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence

RUTA_CATALOGOS = Path(__file__).parent / "datos" / "catalogos_epidemiologia.json"

#: Conjuntos de rangos etarios del original. Cada informe usa el suyo:
#: rango1 (7 tramos) para IRA, rango2 (6) para cobertura, rango3 (18) para CVE.
CONJUNTO_IRA = "rango1"
CONJUNTO_COBERTURA = "rango2"
CONJUNTO_CVE = "rango3"

#: Código de evento de la Infección Respiratoria Aguda en el catálogo CVE.
EVENTO_IRA = "995"


# ==============================================================================
# CATÁLOGOS
# ==============================================================================

@lru_cache(maxsize=1)
def catalogos() -> Dict[str, Any]:
    """Catálogos extraídos del sistema original. Solo listas de códigos."""
    with open(RUTA_CATALOGOS, encoding="utf-8") as f:
        return json.load(f)


@lru_cache(maxsize=1)
def _cie_a_evento() -> Dict[str, str]:
    """CIE-10 -> código de evento de vigilancia."""
    return {c["cie"].strip().upper(): c["evento"].strip() for c in catalogos()["cve"]}


@lru_cache(maxsize=1)
def _nombre_evento() -> Dict[str, str]:
    nombres = {e["codigo"].strip(): e["nombre"].strip() for e in catalogos()["eventos"]}
    # El propio CVE trae el nombre; sirve de respaldo si falta en EVENTOS.
    for c in catalogos()["cve"]:
        nombres.setdefault(c["evento"].strip(), c["nombre"].strip())
    return nombres


@lru_cache(maxsize=1)
def _cie_de_its() -> Dict[str, str]:
    """CIE-10 de las infecciones de transmisión sexual -> nombre."""
    return {r["cie"].strip().upper(): r["nombre"].strip() for r in catalogos()["its"]}


def evento_de(cie: Optional[str]) -> Optional[str]:
    """Evento de vigilancia que corresponde a un diagnóstico, si alguno."""
    if not cie:
        return None
    return _cie_a_evento().get(cie.strip().upper())


def es_its(cie: Optional[str]) -> bool:
    if not cie:
        return False
    return cie.strip().upper() in _cie_de_its()


# ==============================================================================
# EDAD Y RANGOS
# ==============================================================================

def edad_vfp(fecha_evento: date, fecha_nacimiento: date) -> int:
    """Edad como la calculaba el original: años de 365 días exactos.

    Se conserva por fidelidad. Ver la nota del encabezado del módulo.
    """
    return int((fecha_evento - fecha_nacimiento).days / 365)


def edad_calendario(fecha_evento: date, fecha_nacimiento: date) -> int:
    """Edad real, contando bisiestos. NO se usa todavía en los informes.

    Existe para poder medir la diferencia contra `edad_vfp` antes de decidir
    si se cambia, en vez de cambiarlo a ciegas.
    """
    años = fecha_evento.year - fecha_nacimiento.year
    if (fecha_evento.month, fecha_evento.day) < (fecha_nacimiento.month, fecha_nacimiento.day):
        años -= 1
    return años


def rangos(conjunto: str) -> List[Dict[str, Any]]:
    definidos = catalogos()["rangos_etarios"]
    if conjunto not in definidos:
        raise ValueError(
            f"conjunto de rangos desconocido: {conjunto!r}; "
            f"hay {sorted(definidos)}"
        )
    return definidos[conjunto]


def clasificar_edad(
    fecha_evento: date,
    fecha_nacimiento: date,
    conjunto: str = CONJUNTO_CVE,
) -> Optional[str]:
    """Devuelve el nombre del rango etario, como `Clasificaredad` del original.

    Fuera de todo rango devuelve None en vez de una cadena vacía: así el
    llamante puede contarlos aparte en lugar de perderlos en un grupo mudo.
    """
    años = edad_vfp(fecha_evento, fecha_nacimiento)
    for r in rangos(conjunto):
        if r["inicial"] <= años <= r["final"]:
            return r["nombre"]
    return None


# ==============================================================================
# INFORMES
# ==============================================================================

def morbilidad(atenciones: Sequence[Any], nombres_cie: Optional[Dict[str, str]] = None) -> List[Dict[str, Any]]:
    """repomorbi: conteo de diagnósticos, de mayor a menor.

    `atenciones` es cualquier secuencia de objetos con `diagnostico_principal`.
    El original descartaba los diagnósticos vacíos y solo mostraba cantidad > 0.
    """
    nombres_cie = nombres_cie or {}
    conteo: Counter[str] = Counter()
    for a in atenciones:
        cie = (getattr(a, "diagnostico_principal", None) or "").strip().upper()
        if cie:
            conteo[cie] += 1

    return [
        {"codigo": cie, "nombre": nombres_cie.get(cie, ""), "cantidad": n}
        for cie, n in sorted(conteo.items(), key=lambda kv: (-kv[1], kv[0]))
    ]


def _tabular(
    filas: Sequence[Dict[str, Any]],
    conjunto: str,
) -> List[Dict[str, Any]]:
    """Tabla cruzada: una fila por clave, columnas por rango etario y sexo.

    Es la forma común de repocve, repoira y repoits en el original.
    """
    nombres_rango = [r["nombre"] for r in rangos(conjunto)]
    acumulado: Dict[str, Dict[str, Any]] = {}

    for f in filas:
        clave = f["clave"]
        if clave not in acumulado:
            acumulado[clave] = {
                "codigo": clave,
                "nombre": f.get("nombre", ""),
                **{n: 0 for n in nombres_rango},
                "sin_rango": 0,
                "totalM": 0,
                "totalF": 0,
                "total": 0,
            }
        fila = acumulado[clave]

        rango = clasificar_edad(f["fecha"], f["fecha_nacimiento"], conjunto)
        fila[rango if rango else "sin_rango"] += 1

        if (f.get("sexo") or "").strip().upper().startswith("M"):
            fila["totalM"] += 1
        else:
            fila["totalF"] += 1
        fila["total"] += 1

    return sorted(acumulado.values(), key=lambda x: x["codigo"])


def _filas_de(atenciones: Sequence[Any]) -> List[Dict[str, Any]]:
    """Aplana atenciones a lo mínimo que necesitan los informes cruzados."""
    filas = []
    for a in atenciones:
        paciente = getattr(a, "paciente", None)
        if paciente is None or not getattr(paciente, "fecha_nacimiento", None):
            continue
        filas.append({
            "cie": (getattr(a, "diagnostico_principal", None) or "").strip().upper(),
            "fecha": getattr(a, "fecha").date() if hasattr(getattr(a, "fecha"), "date") else getattr(a, "fecha"),
            "fecha_nacimiento": paciente.fecha_nacimiento,
            "sexo": getattr(paciente, "sexo", ""),
            "identificacion": getattr(paciente, "identificacion", ""),
        })
    return filas


def eventos_vigilancia(atenciones: Sequence[Any]) -> List[Dict[str, Any]]:
    """repocve: eventos de vigilancia por rango etario y sexo (18 tramos)."""
    filas = []
    for f in _filas_de(atenciones):
        evento = evento_de(f["cie"])
        if evento:
            filas.append({**f, "clave": evento, "nombre": _nombre_evento().get(evento, "")})
    return _tabular(filas, CONJUNTO_CVE)


def ira(atenciones: Sequence[Any]) -> List[Dict[str, Any]]:
    """repoira: solo Infección Respiratoria Aguda, con los 7 tramos del rango1.

    El original filtraba por `ALLTRIM(CVE1) == '995'`; aquí el evento se deriva
    del diagnóstico, así que se compara contra el mismo código.
    """
    filas = []
    for f in _filas_de(atenciones):
        if evento_de(f["cie"]) == EVENTO_IRA:
            filas.append({**f, "clave": EVENTO_IRA, "nombre": _nombre_evento().get(EVENTO_IRA, "")})
    return _tabular(filas, CONJUNTO_IRA)


def its(atenciones: Sequence[Any]) -> List[Dict[str, Any]]:
    """repoits: infecciones de transmisión sexual, por rango etario y sexo.

    El original cruzaba el diagnóstico contra el catálogo ITS, que son 13
    códigos CIE-10.
    """
    filas = []
    for f in _filas_de(atenciones):
        if es_its(f["cie"]):
            filas.append({**f, "clave": f["cie"], "nombre": _cie_de_its().get(f["cie"], "")})
    return _tabular(filas, CONJUNTO_CVE)


def cobertura(atenciones: Sequence[Any]) -> List[Dict[str, Any]]:
    """repocobertura: población atendida por rango etario y sexo (6 tramos).

    Cuenta PERSONAS, no atenciones: quien consultó tres veces cuenta una vez.
    Es la diferencia con los demás informes y viene del original, que indexaba
    por cédula antes de contar.
    """
    vistos: Dict[str, Dict[str, Any]] = {}
    for f in _filas_de(atenciones):
        # Se conserva la PRIMERA atención de cada persona en el periodo, que es
        # la que fija su edad para el corte.
        vistos.setdefault(f["identificacion"], {**f, "clave": "TOTAL", "nombre": "Población atendida"})
    return _tabular(list(vistos.values()), CONJUNTO_COBERTURA)


def procedimientos(atenciones: Sequence[Any]) -> List[Dict[str, Any]]:
    """repoprocedimiento: listado plano con los datos que exige el reporte.

    El original volcaba a XLS con los datos demográficos resueltos desde
    `pacientes` y `ciudades`. Aquí se devuelven las filas; la exportación es
    responsabilidad de quien llame.
    """
    filas = []
    for a in atenciones:
        p = getattr(a, "paciente", None)
        if p is None:
            continue
        filas.append({
            "identificacion": getattr(p, "identificacion", ""),
            "tipo_id": getattr(p, "tipo_id", ""),
            "apellido1": getattr(p, "apellido1", ""),
            "apellido2": getattr(p, "apellido2", ""),
            "nombre1": getattr(p, "nombre1", ""),
            "nombre2": getattr(p, "nombre2", ""),
            "sexo": getattr(p, "sexo", ""),
            "zona": getattr(p, "zona", ""),
            "tipo_usuario": getattr(p, "tipo_usuario", ""),
            "municipio": getattr(p, "cod_municipio", ""),
            "fecha": getattr(a, "fecha", None),
            "diagnostico": getattr(a, "diagnostico_principal", ""),
        })
    return filas


def resumen(atenciones: Sequence[Any]) -> Dict[str, int]:
    """Cuántas filas produce cada informe. Útil para cuadrar antes de enviar."""
    return {
        "morbilidad": len(morbilidad(atenciones)),
        "eventos_vigilancia": len(eventos_vigilancia(atenciones)),
        "ira": len(ira(atenciones)),
        "its": len(its(atenciones)),
        "cobertura": len(cobertura(atenciones)),
        "procedimientos": len(procedimientos(atenciones)),
    }
