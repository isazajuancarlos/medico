"""Informes epidemiológicos, migrados del sistema en Visual FoxPro.

Reemplaza seis formularios de `repoNuevo` que el consultorio corre hoy a mano:
repomorbi, repocve, repoira, repoits, repocobertura y repoprocedimiento.

No son funciones opcionales: son obligaciones de reporte a la Secretaría de
Salud. Por eso el criterio aquí es FIDELIDAD, aunque algo del original parezca
mejorable — si los números no coinciden con los que el prestador viene
reportando, el informe nuevo no sirve para nada.

=== LAS DOS DECISIONES QUE HAY QUE CONOCER ===

**1. La edad se calcula BIEN, apartándose del original a propósito.**

El VFP hacía `lnanos = INT(días/365)` y `lnmeses = INT(resto/30)`: años de 365
días y meses de 30. Eso ENVEJECE a la gente — alguien nacido en 1950 aparecía
con 71 dos semanas antes de cumplirlos —, y el desfase crece con la edad.

Aquí `edad()` usa aritmética de calendario real y devuelve años, meses y días.
Los tres importan: en pediatría el tramo «0 años» va del recién nacido al de
once meses, y para vigilancia de IRA no es lo mismo; RIPS y las fichas de
notificación además exigen la unidad de medida cuando el paciente es menor de un
año.

Medido antes de cambiarlo: la edad en años difiere del original en ~5 % de los
casos, pero solo el 0,16 %–0,47 % de las clasificaciones cambia de RANGO, y 0 %
en los tramos de cobertura. Es decir, la corrección mueve muy pocos números.
`edad_vfp()` se conserva para poder EXPLICAR una diferencia contra los reportes
históricos del prestador, no para producirla.

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
from collections import Counter
from dataclasses import dataclass
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

@dataclass(frozen=True)
class Edad:
    """Edad exacta en años, meses y días de calendario.

    Los tres importan. En pediatría, «0 años» va del recién nacido al de once
    meses, y para vigilancia de IRA no es lo mismo: sin meses y días ese tramo
    es ciego. RIPS y las fichas de notificación también piden la unidad de
    medida cuando el paciente es menor de un año.
    """

    años: int
    meses: int
    días: int
    total_días: int

    @property
    def total_meses(self) -> int:
        return self.años * 12 + self.meses

    @property
    def unidad_rips(self) -> str:
        """Unidad con que se reporta la edad: años, meses o días."""
        if self.años >= 1:
            return "años"
        if self.total_meses >= 1:
            return "meses"
        return "días"

    @property
    def valor_rips(self) -> int:
        if self.años >= 1:
            return self.años
        if self.total_meses >= 1:
            return self.total_meses
        return self.total_días

    def __str__(self) -> str:
        return f"{self.años}a {self.meses}m {self.días}d"


def _días_del_mes(año: int, mes: int) -> int:
    if mes == 2:
        bisiesto = (año % 4 == 0 and año % 100 != 0) or año % 400 == 0
        return 29 if bisiesto else 28
    return 30 if mes in (4, 6, 9, 11) else 31


def _sumar_meses(f: date, n: int) -> date:
    """`f` más `n` meses, RECORTANDO al último día si el mes es más corto.

    Es la convención habitual (la misma de `relativedelta`): un mes después del
    31 de enero es el 29 de febrero en año bisiesto, no el 2 ó 3 de marzo.
    """
    indice = f.month - 1 + n
    año = f.year + indice // 12
    mes = indice % 12 + 1
    return date(año, mes, min(f.day, _días_del_mes(año, mes)))


def edad(fecha_evento: date, fecha_nacimiento: date) -> Edad:
    """Edad exacta, con aritmética de calendario real.

    Reemplaza el cálculo del original, que usaba años de 365 días y meses de 30
    —`INT(días/365)` y `INT(resto/30)`—, y por eso envejecía a la gente: alguien
    nacido en 1950 aparecía con 71 años dos semanas antes de cumplirlos.

    Se calcula buscando el ÚLTIMO ANIVERSARIO MENSUAL cumplido y contando los
    días desde ahí. Restar campo a campo y «pedir prestado» al mes anterior no
    basta: quien nace un día 31 produce días negativos, porque el mes prestado
    puede ser más corto que su día de nacimiento.
    """
    if fecha_evento < fecha_nacimiento:
        raise ValueError(
            f"la fecha del evento ({fecha_evento}) es anterior al nacimiento "
            f"({fecha_nacimiento})"
        )

    meses_totales = (
        (fecha_evento.year - fecha_nacimiento.year) * 12
        + fecha_evento.month - fecha_nacimiento.month
    )
    if _sumar_meses(fecha_nacimiento, meses_totales) > fecha_evento:
        meses_totales -= 1

    aniversario = _sumar_meses(fecha_nacimiento, meses_totales)
    años, meses = divmod(meses_totales, 12)

    return Edad(
        años=años,
        meses=meses,
        días=(fecha_evento - aniversario).days,
        total_días=(fecha_evento - fecha_nacimiento).days,
    )


def edad_calendario(fecha_evento: date, fecha_nacimiento: date) -> int:
    """Años cumplidos, sin más. Atajo sobre `edad`."""
    return edad(fecha_evento, fecha_nacimiento).años


def edad_vfp(fecha_evento: date, fecha_nacimiento: date) -> int:
    """Años como los calculaba el original: `INT(días/365)`.

    YA NO SE USA EN LOS INFORMES. Se conserva únicamente para poder comparar
    contra los reportes históricos del prestador y explicar una diferencia si
    aparece — no para producirla.
    """
    return int((fecha_evento - fecha_nacimiento).days / 365)


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
    """Rango etario del paciente, con la edad CALCULADA CORRECTAMENTE.

    Equivale a `Clasificaredad` del original salvo en la aritmética: aquí los
    años son de calendario, no de 365 días. Ver la nota del encabezado.

    Fuera de todo rango devuelve None en vez de una cadena vacía: así el
    llamante puede contarlos aparte en lugar de perderlos en un grupo mudo.
    """
    años = edad(fecha_evento, fecha_nacimiento).años
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
