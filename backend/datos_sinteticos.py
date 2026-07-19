"""Generador de datos SINTÉTICOS para validar contra el MUV.

Existe por una razón de protección de datos, no de comodidad. El ambiente de
pruebas de MinSalud tiene controles más débiles que producción y una finalidad
distinta de aquella para la que se recogieron las historias clínicas. Enviar
pacientes reales ahí es exactamente lo que la Ley 1581 no quiere para datos de
salud, que son categoría especial.

Así que el ciclo se prueba con personas que no existen. Lo que se valida es la
ESTRUCTURA del archivo que produce el sistema, y para eso los datos inventados
sirven igual.

Los códigos SÍ son reales —CIE-10, CUPS, municipios— porque el validador los
contrasta contra tablas de referencia y con valores inventados rechazaría por el
motivo equivocado, ocultando los errores de estructura que sí buscamos.
"""

from __future__ import annotations

import random
from datetime import date, datetime, timedelta
from typing import Any, Dict, List

#: CIE-10 reales de consulta general. Sacados del catálogo del sistema.
DIAGNOSTICOS = [
    "J00X",  # rinofaringitis aguda
    "J039",  # amigdalitis aguda
    "I10X",  # hipertensión esencial
    "E119",  # diabetes tipo 2 sin complicaciones
    "M545",  # lumbago
    "K297",  # gastritis
    "N390",  # infección de vías urinarias
    "R51X",  # cefalea
]

#: CUPS reales de consulta.
CONSULTAS_CUPS = [
    "890201",  # consulta de primera vez por medicina general
    "890301",  # consulta de control por medicina general
]

MUNICIPIOS = ["70001", "70110", "70215", "05001"]
TIPOS_DOCUMENTO = ["CC", "TI", "RC"]
SEXOS = ["M", "F"]


def _documento_falso(i: int) -> str:
    """Identificación claramente inventada, para que nadie la confunda con una
    real ni siquiera de un vistazo."""
    return f"9999{i:06d}"


def paciente_sintetico(i: int, semilla: random.Random) -> Dict[str, Any]:
    nacimiento = date(1950, 1, 1) + timedelta(days=semilla.randint(0, 27000))
    return {
        "tipoDocumentoIdentificacion": semilla.choice(TIPOS_DOCUMENTO),
        "numDocumentoIdentificacion": _documento_falso(i),
        "tipoUsuario": "12",
        "fechaNacimiento": nacimiento.isoformat(),
        "codSexo": semilla.choice(SEXOS),
        "codPaisResidencia": "170",
        "codMunicipioResidencia": semilla.choice(MUNICIPIOS),
        "codZonaTerritorialResidencia": semilla.choice(["01", "02"]),
        "incapacidad": "NO",
        "codPaisOrigen": "170",
        "consecutivo": i,
    }


def consulta_sintetica(paciente: Dict[str, Any], momento: datetime,
                       semilla: random.Random) -> Dict[str, Any]:
    return {
        "codPrestador": "700010000101",
        "fechaInicioAtencion": momento.strftime("%Y-%m-%d %H:%M:%S"),
        "numAutorizacion": "",
        "codConsulta": semilla.choice(CONSULTAS_CUPS),
        "modalidadGrupoServicioTecSal": "01",
        "grupoServicios": "01",
        "codServicio": 328,
        "finalidadTecnologiaSalud": "10",
        "causaMotivoAtencion": "38",
        "codDiagnosticoPrincipal": semilla.choice(DIAGNOSTICOS),
        "codDiagnosticoRelacionado1": "",
        "codDiagnosticoRelacionado2": "",
        "codDiagnosticoRelacionado3": "",
        "tipoDiagnosticoPrincipal": "01",
        "tipoDocumentoIdentificacion": paciente["tipoDocumentoIdentificacion"],
        "numDocumentoIdentificacion": paciente["numDocumentoIdentificacion"],
        "vrServicio": 0,
        "conceptoRecaudo": "05",
        "valorPagoModerador": 0,
        "numFEVPagoModerador": "",
        "consecutivo": 1,
    }


def documento_sintetico(
    n_pacientes: int = 10,
    nit_obligado: str = "999999999",
    semilla: int = 20260719,
) -> Dict[str, Any]:
    """Un documento con la misma forma que produce `rips_completo.generar_archivo_ac`.

    Determinista por defecto: la misma semilla da el mismo archivo, para poder
    comparar dos corridas.
    """
    rnd = random.Random(semilla)
    inicio = datetime(2026, 6, 1, 8, 0)

    usuarios = []
    for i in range(1, n_pacientes + 1):
        p = paciente_sintetico(i, rnd)
        momento = inicio + timedelta(days=rnd.randint(0, 29), minutes=rnd.randint(0, 480))
        p["servicios"] = {"consultas": [consulta_sintetica(p, momento, rnd)]}
        usuarios.append(p)

    return {"numDocumentoIdObligado": nit_obligado, "usuarios": usuarios}


def paquete_sintetico(n_pacientes: int = 10, num_nota: str = "PRUEBA1") -> Dict[str, Any]:
    """Paquete listo para enviar al MUV de PRUEBAS."""
    from fev_rips import consolidar, sobre_rips_sin_factura

    doc = documento_sintetico(n_pacientes)
    return sobre_rips_sin_factura(
        doc["numDocumentoIdObligado"], consolidar([doc]), num_nota=num_nota
    )
