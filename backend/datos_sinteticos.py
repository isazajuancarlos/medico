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

import os
import random
from datetime import date, datetime, timedelta
from functools import lru_cache
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

#: CUPS reales de procedimiento, para el archivo AP.
PROCEDIMIENTOS_CUPS = [
    "902210",  # hemograma IV
    "903841",  # glucosa en suero
    "871121",  # radiografía de tórax
    "893801",  # electrocardiograma de ritmo
]

#: Diagnósticos de urgencia y hospitalización. Se separan de DIAGNOSTICOS
#: porque una rinofaringitis no lleva a nadie a urgencias, y un archivo que
#: nunca ejercita un caso realista no prueba lo que dice probar.
DIAGNOSTICOS_AGUDOS = [
    "I219",  # infarto agudo de miocardio
    "J189",  # neumonía
    "K358",  # apendicitis aguda
    "S060",  # concusión
]

MUNICIPIOS = ["70001", "70110", "70215", "05001"]

#: Documentos del PACIENTE. Incluye los de menores a propósito: son válidos en
#: el bloque de usuarios y son los que destaparon que el documento del
#: profesional se estaba copiando de aquí.
TIPOS_DOCUMENTO = ["CC", "TI", "RC"]

#: Documentos del PROFESIONAL. Deliberadamente distintos: la Resolución 948
#: (Documento Técnico 1, P11) solo admite estos ocho, y ninguno es de menor.
#: Que las dos listas no se solapen del todo es lo que hace que una prueba
#: pueda distinguir un documento del otro.
TIPOS_DOCUMENTO_PROFESIONAL = ["CC", "CE", "PA"]

SEXOS = ["M", "F"]


def _del_entorno(variable: str, que_es: str) -> str:
    """Un dato del prestador, o un error que dice cuál falta.

    Se resuelve UNA vez, arriba, y se pasa hacia abajo. Leerlo desde el fondo
    de la pila obligaba a montar variables de entorno para ejercitar cosas que
    nada tienen que ver con el MUV, y una prueba que necesita el entorno para
    comprobar la forma de un JSON está midiendo dos cosas a la vez.
    """
    valor = os.environ.get(variable)
    if not valor:
        raise RuntimeError(
            f"Falta {variable}: {que_es}. No hay valor por defecto — uno "
            f"inventado produce un rechazo que parece un error del archivo.")
    return valor


def _documento_falso(i: int) -> str:
    """Identificación claramente inventada, para que nadie la confunda con una
    real ni siquiera de un vistazo."""
    return f"9999{i:06d}"


@lru_cache(maxsize=1)
def _cums_activos() -> tuple:
    """Unos cuantos CUM reales y vigentes del catálogo del INVIMA.

    Se toman del mismo catálogo contra el que valida el MUV. Inventar un código
    con forma de CUM haría que el archivo se rechazara por el motivo
    equivocado, y el rechazo equivocado enseña lo contrario de lo que se busca.
    """
    import cum
    activos = [m for m in cum._cargar().values() if m.activo]
    return tuple(activos[:200])


def _codigo_tableta() -> int:
    """La presentación más común, tomada del maestro y no escrita a mano."""
    import maestros_muv
    codigo = maestros_muv.codigo_presentacion("TABLETA")
    assert codigo is not None, "el maestro de presentaciones no trae TABLETA"
    return codigo


def _cie11(cie10: str, prefijo: str) -> Dict[str, str]:
    """El par CIE-11 que la 948 pide junto a cada CIE-10.

    Misma regla que `rips_completo._par_cie11` —vacío si la equivalencia no es
    exacta y única— y se deja igual a propósito: un archivo de prueba que
    rellenara lo que el generador de verdad deja vacío probaría otro programa.
    """
    import cie11
    destino = cie11.derivar(cie10)
    return {
        # `codDiagnosticoPrincipal` -> `codDiagnosticoPrincipalCIE11` y
        # `nomCodDiagnosticoPrincipalCIE11`. El prefijo YA empieza por «cod»:
        # añadirle otro daba `nomCodCodDiagnostico…`, un campo que no existe y
        # que el MUV habría ignorado en silencio.
        f"{prefijo}CIE11": destino.codigo if destino else "",
        f"nom{prefijo[0].upper()}{prefijo[1:]}CIE11": destino.titulo if destino else "",
    }


def profesional_sintetico(i: int, semilla: random.Random) -> Dict[str, Any]:
    """El médico que atiende. Va en los campos P11/P12 de CADA servicio.

    No es el paciente, y por eso su documento se genera aparte y en otro rango:
    `8888…` frente a `9999…`. Si alguien vuelve a copiar el del paciente, la
    diferencia salta a la vista en el archivo y no hay que esperar al MUV.
    """
    return {
        "tipoDocumentoIdentificacion": semilla.choice(TIPOS_DOCUMENTO_PROFESIONAL),
        "numDocumentoIdentificacion": f"8888{i:06d}",
    }


def _documento_por_edad(nacimiento: date) -> str:
    """El tipo de documento que corresponde a esa edad.

    El MUV lo comprueba (RVC007): «La edad según la fecha de nacimiento debe
    coincidir con el tipo de documento registrado». Elegirlo al azar producía
    recién nacidos con cédula y adultos con registro civil — un archivo de
    prueba imposible, que rechaza por un motivo que ningún prestador real
    tendría.
    """
    anios = (date(2026, 6, 1) - nacimiento).days // 365
    if anios < 7:
        return "RC"      # registro civil
    if anios < 18:
        return "TI"      # tarjeta de identidad
    return "CC"


def paciente_sintetico(i: int, semilla: random.Random) -> Dict[str, Any]:
    nacimiento = date(1950, 1, 1) + timedelta(days=semilla.randint(0, 27000))
    return {
        "tipoDocumentoIdentificacion": _documento_por_edad(nacimiento),
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


def consulta_sintetica(paciente: Dict[str, Any], profesional: Dict[str, Any],
                       momento: datetime, semilla: random.Random,
                       cod_prestador: str) -> Dict[str, Any]:
    """El paciente se recibe para nada más que el consecutivo: su documento NO
    va aquí. Los campos P11/P12 son del profesional, y confundirlos fue el
    fallo que el MUV destapó."""
    return {
        "codPrestador": cod_prestador,
        "fechaInicioAtencion": momento.strftime("%Y-%m-%d %H:%M"),
        "numAutorizacion": "",
        "codConsulta": semilla.choice(CONSULTAS_CUPS),
        "modalidadGrupoServicioTecSal": "01",
        "grupoServicios": "01",
        "codServicio": 328,
        "finalidadTecnologiaSalud": "11",
        "causaMotivoAtencion": "38",
        "codDiagnosticoPrincipal": semilla.choice(DIAGNOSTICOS),
        "codDiagnosticoRelacionado1": None,
        "codDiagnosticoRelacionado2": None,
        "codDiagnosticoRelacionado3": None,
        "tipoDiagnosticoPrincipal": "01",
        **profesional,
        "vrServicio": 0,
        "conceptoRecaudo": "05",
        "valorPagoModerador": 0,
        "numFEVPagoModerador": "",
        "consecutivo": 1,
    }


def procedimiento_sintetico(profesional, momento, semilla, cod_prestador) -> Dict[str, Any]:
    dx = semilla.choice(DIAGNOSTICOS)
    return {
        "codPrestador": cod_prestador,
        "fechaInicioAtencion": momento.strftime("%Y-%m-%d %H:%M"),
        "idMIPRES": "",
        "numAutorizacion": "",
        "codProcedimiento": semilla.choice(PROCEDIMIENTOS_CUPS),
        "viaIngresoServicioSalud": "02",
        "modalidadGrupoServicioTecSal": "01",
        "grupoServicios": "01",
        "codServicio": 328,
        "finalidadTecnologiaSalud": "44",
        "codDiagnosticoPrincipal": dx,
        "codDiagnosticoRelacionado": None,
        "codComplicacion": None,
        **_cie11(dx, "codDiagnosticoPrincipal"),
        **profesional,
        "vrServicio": 25000,
        "conceptoRecaudo": "05",
        "valorPagoModerador": 0,
        "numFEVPagoModerador": "",
        "consecutivo": 1,
    }


def medicamento_sintetico(profesional, momento, semilla, cod_prestador) -> Dict[str, Any]:
    """Con un CUM REAL del catálogo del INVIMA, no uno inventado.

    Un código con forma de CUM que no existe pasa cualquier validación de
    estructura y hace rechazar la factura entera en el MUV. Un archivo de
    prueba que lo lleve estaría midiendo el rechazo equivocado.
    """
    medicamento = semilla.choice(_cums_activos())
    return {
        "codPrestador": cod_prestador,
        "fechaDispensAdmon": momento.strftime("%Y-%m-%d %H:%M"),
        "idMIPRES": "",
        "codDiagnosticoPrincipal": semilla.choice(DIAGNOSTICOS),
        "codTecnologiaSalud": medicamento.codigo,
        "tipoMedicamento": "01",
        # Del catálogo real, no inventado: TABLETA es 66. El 1 es AMPOLLA,
        # que es lo que se estuvo enviando.
        "unidadMinDispensa": _codigo_tableta(),
        "nomTecnologiaSalud": medicamento.nombre[:60],
        "cantidadMedicamento": semilla.randint(1, 30),
        "diasTratamiento": semilla.randint(1, 30),
        "vrUnitMedicamento": 1500.0,
        **profesional,
        "vrServicio": 1500.0,
        "conceptoRecaudo": "05",
        "valorPagoModerador": 0,
        "numFEVPagoModerador": "",
        "consecutivo": 1,
    }


def urgencia_sintetica(profesional, momento, semilla, cod_prestador) -> Dict[str, Any]:
    dx = semilla.choice(DIAGNOSTICOS_AGUDOS)
    return {
        "codPrestador": cod_prestador,
        "fechaInicioAtencion": momento.strftime("%Y-%m-%d %H:%M"),
        "causaMotivoAtencion": "38",
        "codDiagnosticoPrincipal": dx,
        "codDiagnosticoPrincipalE": dx,
        "codDiagnosticoRelacionadoE1": None,
        "condicionDestinoUsuarioEgreso": "01",
        # `null` porque el egreso es vivo. La norma no admite "" —pide entre 4 y
        # 6 caracteres— y poner un código «por si acaso» inventaría una muerte.
        "codDiagnosticoCausaMuerte": None,
        "fechaEgreso": (momento + timedelta(hours=6)).strftime("%Y-%m-%d %H:%M"),
        **_cie11(dx, "codDiagnosticoPrincipal"),
        # Sin documento del profesional: la estructura de urgencias y
        # hospitalización no lo admite. El MUV lo rechaza como propiedad no
        # permitida.
        "consecutivo": 1,
    }


def hospitalizacion_sintetica(profesional, momento, semilla, cod_prestador) -> Dict[str, Any]:
    dx = semilla.choice(DIAGNOSTICOS_AGUDOS)
    return {
        "codPrestador": cod_prestador,
        "viaIngresoServicioSalud": "02",
        "fechaInicioAtencion": momento.strftime("%Y-%m-%d %H:%M"),
        "numAutorizacion": "",
        "causaMotivoAtencion": "38",
        "codDiagnosticoPrincipal": dx,
        "codDiagnosticoPrincipalE": dx,
        "codDiagnosticoRelacionadoE1": None,
        "codComplicacion": None,
        "condicionDestinoUsuarioEgreso": "01",
        "codDiagnosticoCausaMuerte": None,
        "fechaEgreso": (momento + timedelta(days=3)).strftime("%Y-%m-%d %H:%M"),
        **_cie11(dx, "codDiagnosticoPrincipal"),
        # Sin documento del profesional: la estructura de urgencias y
        # hospitalización no lo admite. El MUV lo rechaza como propiedad no
        # permitida.
        "consecutivo": 1,
    }


def documento_sintetico(
    n_pacientes: int = 10,
    nit_obligado: str | None = None,
    cod_prestador: str | None = None,
    semilla: int = 20260719,
) -> Dict[str, Any]:
    """Un documento con la misma forma que produce `rips_completo`.

    Determinista por defecto: la misma semilla da el mismo archivo, para poder
    comparar dos corridas.

    LOS CINCO TIPOS DE ATENCIÓN. Hasta el 2026-07-26 esto solo emitía consultas,
    y validar ese archivo contra el MUV respondía por un quinto del sistema
    mientras se concluía sobre el todo. Ahora cada paciente lleva su consulta
    —que todos tienen— y, repartidos, procedimientos, medicamentos, urgencias y
    hospitalización, para que el archivo ejercite los cinco bloques y los 22
    campos CIE-11 que viven sobre todo en los dos últimos.
    """
    # El MUV compara el NIT del archivo con el de la sesión y rechaza si
    # difieren. Uno inventado hacía fallar todo el paquete por un motivo que
    # nada tiene que ver con lo que se quiere probar.
    if nit_obligado is None:
        nit_obligado = _del_entorno(
            "MUV_NIT", "el RIPS tiene que ir a nombre del prestador que autentica")
    if cod_prestador is None:
        cod_prestador = _del_entorno(
            "MUV_COD_PRESTADOR",
            "el código de habilitación del prestador en el REPS, que el MUV "
            "comprueba contra el NIT (regla RVC012)")

    rnd = random.Random(semilla)
    inicio = datetime(2026, 6, 1, 8, 0)

    usuarios = []
    for i in range(1, n_pacientes + 1):
        p = paciente_sintetico(i, rnd)
        prof = profesional_sintetico(i, rnd)
        momento = inicio + timedelta(days=rnd.randint(0, 29), minutes=rnd.randint(0, 480))

        servicios: Dict[str, Any] = {"consultas": [consulta_sintetica(p, prof, momento, rnd, cod_prestador)]}
        # Repartidos y no todos a todos: un paciente con consulta, urgencias y
        # hospitalización el mismo día no es un caso realista, y un archivo de
        # prueba irreal esconde justo los errores que solo salen en lo normal.
        if i % 2 == 0:
            servicios["procedimientos"] = [procedimiento_sintetico(prof, momento, rnd, cod_prestador)]
        if i % 3 == 0:
            servicios["medicamentos"] = [medicamento_sintetico(prof, momento, rnd, cod_prestador)]
        if i % 4 == 0:
            servicios["urgencias"] = [urgencia_sintetica(prof, momento, rnd, cod_prestador)]
        if i % 5 == 0:
            servicios["hospitalizacion"] = [hospitalizacion_sintetica(prof, momento, rnd, cod_prestador)]

        p["servicios"] = servicios
        usuarios.append(p)

    return {"numDocumentoIdObligado": nit_obligado, "usuarios": usuarios}


def paquete_sintetico(n_pacientes: int = 10, num_nota: str = "PRUEBA1") -> Dict[str, Any]:
    """Paquete listo para enviar al MUV de PRUEBAS."""
    from fev_rips import consolidar, sobre_rips_sin_factura

    doc = documento_sintetico(n_pacientes)
    return sobre_rips_sin_factura(
        doc["numDocumentoIdObligado"], consolidar([doc]), num_nota=num_nota
    )
