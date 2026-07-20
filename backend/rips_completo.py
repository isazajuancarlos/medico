"""
Generador RIPS Completo - Resolución 2275 de 2023
Ministerio de Salud y Protección Social de Colombia

Actualizado para 2026:
- Formato JSON según Resolución 2275/2023 (FEV_RIPS)
- CUPS 2026: Resolución 2706 de 2025
- Plan de Beneficios 2026: Resolución 2765 de 2025
- UPC 2026: Resolución 2764 de 2025
- Integración con MUV (Mecanismo Único de Validación) del MinSalud

Archivos generados:
- AC: Consultas (atenciones ambulatorias)
- AP: Procedimientos (laboratorios, imágenes, terapias, cirugías)
- AM: Medicamentos (fórmulas dispensadas)
- AU: Urgencias (atenciones en servicio de urgencias)
- AH: Hospitalización (ingresos hospitalarios)
- CT: Control (resumen de facturación - recuento de registros)

Notas de cumplimiento (Res. 2275/2023):
- Formato JSON obligatorio desde diciembre 2023 (no TXT)
- RIPS es soporte obligatorio de la Factura Electrónica de Venta (FEV)
- Validación en MUV del MinSalud (requiere integración externa en producción)
- CUV (Código Único de Validación) asignado por el MUV tras validación exitosa
- MIPRES: integración requerida para prescripciones de tecnologías no PBS
"""

import io
import json
import zipfile
from datetime import datetime, date
from typing import Any, Dict, List, Optional

from sqlalchemy.orm import joinedload
from sqlmodel import Session, select

from database import engine
from models import (
    AtencionUrgencias, CausaExterna, FinalidadConsulta, Formula,
    Historia, Hospitalizacion, Institucion, Orden, Paciente,
    TipoDiagnostico, Usuario,
)


# ==============================================================================
# UTILIDADES INTERNAS
# ==============================================================================

class InstitucionRequerida(ValueError):
    """No se puede generar un RIPS sin saber de qué institución es.

    Antes `institucion_id` era opcional y, al omitirlo, cada generador dejaba de
    filtrar: el archivo salía con las atenciones de TODAS las instituciones,
    etiquetado con el NIT de la primera fila de la tabla. Dos daños a la vez:
    datos clínicos de terceros fuera del sistema, y una factura que no
    corresponde a lo que nadie prestó.

    El parámetro opcional era la fuga. Un filtro que por omisión abre todo
    espera a que alguien olvide pasarlo, y alguien siempre lo olvida:
    `routers/rips.py` lo llevaba olvidado desde el principio.
    """


class DatosIncompletos(ValueError):
    """Falta un dato que el RIPS exige y que no se puede deducir.

    Antes cada hueco se rellenaba con una constante: el CUPS `"890201"`, el CUM
    `"00000000"`, el diagnóstico `"R69"`, el valor del servicio en `0`. El
    archivo salía siempre, pasaba la validación de estructura, y el problema
    aparecía después —como glosa, como rechazo del MUV, o como un pago que
    nunca llega porque el valor iba en cero.

    Un RIPS que no sale se arregla en una tarde. Uno que sale con datos
    inventados no se nota nunca.
    """

    def __init__(self, faltantes: List["Falta"]):
        self.faltantes = faltantes
        super().__init__(self.resumen())

    def resumen(self) -> str:
        por_campo: Dict[str, int] = {}
        for f in self.faltantes:
            por_campo[f.etiqueta] = por_campo.get(f.etiqueta, 0) + 1
        detalle = "; ".join(f"{e}: {n}" for e, n in
                            sorted(por_campo.items(), key=lambda kv: -kv[1]))
        return (
            f"No se puede generar el RIPS: {len(self.faltantes)} dato(s) "
            f"obligatorios no están registrados ({detalle}). "
            f"Corrígelos y vuelve a generar, o pide explícitamente "
            f"`permitir_supuestos=True` si aceptas reportarlos por defecto."
        )


class Falta:
    """Un dato obligatorio que no está, con dónde buscarlo."""

    __slots__ = ("archivo", "registro_id", "campo", "etiqueta", "supuesto")

    def __init__(self, archivo, registro_id, campo, etiqueta, supuesto):
        self.archivo = archivo
        self.registro_id = registro_id
        self.campo = campo
        self.etiqueta = etiqueta
        self.supuesto = supuesto

    def __repr__(self) -> str:
        return (f"{self.archivo} #{self.registro_id}: falta {self.etiqueta} "
                f"({self.campo})")


class _Recolector:
    """Reúne TODO lo que falta antes de decidir si se puede generar.

    Falla al final y no en el primer hueco a propósito: quien cierra el mes
    necesita la lista completa para corregirla de una vez. Reventar en la
    primera consulta de mil convierte el cierre en mil intentos.

    `permitir_supuestos` es el camino permisivo que exige
    [[directiva-fallar-en-vez-de-suponer]]: existe, pero lo pide el llamante,
    nunca se activa solo, y lo supuesto queda registrado igual.
    """

    def __init__(self, permitir_supuestos: bool = False):
        self.permitir_supuestos = permitir_supuestos
        self.faltantes: List[Falta] = []

    @staticmethod
    def _vacio(valor: Any) -> bool:
        """Ausente es `None` o cadena en blanco. **`0` no es ausente**: un
        servicio puede valer cero de verdad, y confundir «no se registró» con
        «vale cero» es el error que este módulo existe para no cometer."""
        if valor is None:
            return True
        return isinstance(valor, str) and not valor.strip()

    def dato(self, valor: Any, *, archivo: str, registro_id: Any, campo: str,
             etiqueta: str, supuesto: Any = None) -> Any:
        if not self._vacio(valor):
            return valor
        self.faltantes.append(Falta(archivo, registro_id, campo, etiqueta, supuesto))
        return supuesto if self.permitir_supuestos else None

    def cerrar(self) -> None:
        if self.faltantes and not self.permitir_supuestos:
            raise DatosIncompletos(self.faltantes)


def _resolver_recolector(
    recibido: Optional[_Recolector], permitir_supuestos: bool
) -> tuple[_Recolector, bool]:
    """El recolector propio del generador, o el compartido del paquete.

    Cuando se generan los seis archivos de una, uno solo los recorre todos: así
    el cierre de mes ve la lista COMPLETA de lo que falta, y no la del primer
    archivo que reventó. El booleano dice si toca cerrarlo aquí.
    """
    if recibido is not None:
        return recibido, False
    return _Recolector(permitir_supuestos), True


def _get_institucion(session: Session, institucion_id: Optional[int]) -> Institucion:
    """La institución del reporte. No hay respaldo ni «la primera disponible».

    El respaldo anterior era lo que convertía un id ausente en un archivo con el
    NIT de otro. Ver [[directiva-fallar-en-vez-de-suponer]]: ante la ausencia del
    dato, excepción; nunca un sustituto con la forma correcta.
    """
    if not institucion_id:
        raise InstitucionRequerida(
            "Falta la institución del reporte. Sin ella el RIPS saldría con las "
            "atenciones de todas las instituciones del sistema."
        )
    institucion = session.get(Institucion, institucion_id)
    if institucion is None:
        raise InstitucionRequerida(
            f"La institución {institucion_id} no existe. Generar el reporte "
            f"igualmente produciría un archivo vacío a nombre de otra."
        )
    return institucion


def _bloque_economico(obj: Any, rec: "_Recolector", archivo: str) -> Dict:
    """Los campos de plata, que son los mismos en los cinco archivos.

    Iban quemados en cada generador —`vrServicio: 0`, `conceptoRecaudo: "05"`—
    y no salían de ningún sitio porque el modelo no tenía dónde guardarlos. Ver
    `models.CamposRips`, que ahora los declara una vez para los cinco.
    """
    def exigir(campo, etiqueta, supuesto):
        return rec.dato(getattr(obj, campo, None), archivo=archivo,
                        registro_id=getattr(obj, "id", None), campo=campo,
                        etiqueta=etiqueta, supuesto=supuesto)

    return {
        "vrServicio": exigir("valor_servicio", "valor del servicio", 0),
        "conceptoRecaudo": exigir("concepto_recaudo", "concepto de recaudo", "05"),
        # El copago SÍ admite cero de verdad: la mayoría de atenciones no llevan.
        "valorPagoModerador": getattr(obj, "valor_pago_moderador", None) or 0,
        "numFEVPagoModerador": getattr(obj, "num_fev_pago_moderador", None) or "",
    }


def _valor_enum(v: Any) -> Any:
    """El `.value` de un Enum, o el valor tal cual. No sustituye ausencias."""
    return v.value if hasattr(v, "value") else v


def _usuario_base(p: Paciente, consecutivo: int, rec: "_Recolector") -> Dict:
    """Identificación del usuario en el RIPS.

    `codMunicipioResidencia` traía `"05001"` de supuesto: reportaba como
    residente de Medellín a todo paciente sin municipio, en el país entero. El
    campo tiene la forma correcta, así que el validador lo acepta y el dato
    falso entra en la estadística de residencia sin que nadie lo note. Ver
    `divipola.py`, que ya lo valida al capturar.
    """
    def exigir(valor, campo, etiqueta, supuesto=None):
        return rec.dato(valor, archivo="usuarios", registro_id=p.identificacion,
                        campo=campo, etiqueta=etiqueta, supuesto=supuesto)

    return {
        "tipoDocumentoIdentificacion": p.tipo_id,
        "numDocumentoIdentificacion": p.identificacion,
        "tipoUsuario": exigir(_valor_enum(getattr(p, "tipo_usuario", None)),
                              "tipo_usuario", "tipo de usuario", "01"),
        "fechaNacimiento": p.fecha_nacimiento.strftime('%Y-%m-%d'),
        "codSexo": p.sexo,
        "codPaisResidencia": exigir(getattr(p, "cod_pais", None),
                                    "cod_pais", "país de residencia", "170"),
        "codMunicipioResidencia": exigir(getattr(p, "cod_municipio", None),
                                         "cod_municipio",
                                         "municipio de residencia", "05001"),
        "codZonaTerritorialResidencia": exigir(
            _valor_enum(getattr(p, "zona", None)),
            "zona", "zona territorial", "01"),
        "codPaisOrigen": exigir(getattr(p, "cod_pais_origen", None),
                                "cod_pais_origen", "país de origen", "170"),
        "consecutivo": consecutivo,
    }


def _fecha_dt(f: datetime) -> str:
    return f.strftime('%Y-%m-%d %H:%M:%S')


def _fecha_d(f: date) -> str:
    return f.strftime('%Y-%m-%d')


# ==============================================================================
# AC - CONSULTAS
# ==============================================================================

def generar_archivo_ac(
    fecha_inicio: date,
    fecha_fin: date,
    institucion_id: int,
    *,
    permitir_supuestos: bool = False,
    _recolector: Optional["_Recolector"] = None,
) -> Dict:
    """
    AC - Archivo de Consultas (Obligatorio)
    Incluye todas las atenciones ambulatorias y consultas de control.

    Levanta `DatosIncompletos` si alguna atención no tiene los datos que el
    RIPS exige, salvo que se pida `permitir_supuestos=True`.
    """
    rec, propio = _resolver_recolector(_recolector, permitir_supuestos)
    with Session(engine) as session:
        institucion = _get_institucion(session, institucion_id)
        nit_institucion = institucion.nit
        cod_habilitacion = institucion.codigo_habilitacion

        q = (
            select(Historia)
            .options(
                joinedload(Historia.paciente),
                joinedload(Historia.medico),
                joinedload(Historia.cita),
            )
            .join(Paciente, Historia.paciente_id == Paciente.id)
            .where(
                Historia.fecha >= datetime.combine(fecha_inicio, datetime.min.time()),
                Historia.fecha <= datetime.combine(fecha_fin, datetime.max.time()),
            )
        )
        q = q.where(Paciente.institucion_id == institucion_id)

        historias = session.exec(q).all()

        usuarios_map: Dict[int, Dict] = {}
        consecutivo_usuario = 1

        for h in historias:
            p = h.paciente
            if not p:
                continue

            if p.id not in usuarios_map:
                base = _usuario_base(p, consecutivo_usuario, rec)
                base["incapacidad"] = "SI" if h.genera_incapacidad else "NO"
                base["servicios"] = {"consultas": []}
                usuarios_map[p.id] = base
                consecutivo_usuario += 1

            num_autorizacion = ""
            if h.cita and hasattr(h.cita, 'num_autorizacion') and h.cita.num_autorizacion:
                num_autorizacion = h.cita.num_autorizacion

            def exigir(valor, campo, etiqueta, supuesto=None):
                return rec.dato(valor, archivo="AC", registro_id=h.id,
                                campo=campo, etiqueta=etiqueta, supuesto=supuesto)

            # Estos campos IBAN QUEMADOS: la misma constante para todo paciente.
            # Después salieron de la historia, pero conservando el quemado como
            # valor por defecto «para no romper lo que ya funcionaba». Eso deja
            # el archivo igual de falso, solo que con la falsedad más repartida:
            # `auditoria_rips` sabía cuáles eran supuestos y se enviaban igual.
            #
            # Ahora el supuesto se declara pero NO se usa salvo que el llamante
            # pida `permitir_supuestos`. Saber que un dato es inventado y
            # mandarlo de todos modos no es auditar: es documentar la mentira.
            consulta = {
                "codPrestador": cod_habilitacion,
                "fechaInicioAtencion": _fecha_dt(h.fecha),
                "numAutorizacion": num_autorizacion,
                "codConsulta": exigir(getattr(h, 'cod_consulta_cups', None),
                                      "cod_consulta_cups", "CUPS de la consulta",
                                      "890201"),
                "modalidadGrupoServicioTecSal": exigir(
                    getattr(h, 'modalidad_grupo_servicio', None),
                    "modalidad_grupo_servicio", "modalidad del servicio", "01"),
                "grupoServicios": exigir(getattr(h, 'grupo_servicios', None),
                                         "grupo_servicios", "grupo de servicios", "01"),
                "codServicio": exigir(getattr(h, 'cod_servicio', None),
                                      "cod_servicio", "código de servicio", 328),
                "finalidadTecnologiaSalud": exigir(
                    _valor_enum(getattr(h, 'finalidad', None)),
                    "finalidad", "finalidad de la consulta", "10"),
                "causaMotivoAtencion": exigir(
                    _valor_enum(getattr(h, 'causa_externa', None)),
                    "causa_externa", "causa o motivo de la atención", "13"),
                "codDiagnosticoPrincipal": exigir(
                    h.diagnostico_principal, "diagnostico_principal",
                    "diagnóstico principal"),
                # Los relacionados SÍ pueden ir vacíos: son opcionales, y una
                # cadena vacía aquí significa «no hay», no «no se registró».
                "codDiagnosticoRelacionado1": h.diagnostico_relacionado1 or "",
                "codDiagnosticoRelacionado2": getattr(h, 'diagnostico_relacionado2', None) or "",
                "codDiagnosticoRelacionado3": getattr(h, 'diagnostico_relacionado3', None) or "",
                "tipoDiagnosticoPrincipal": exigir(
                    _valor_enum(getattr(h, 'tipo_diagnostico', None)),
                    "tipo_diagnostico", "tipo de diagnóstico principal", "01"),
                "tipoDocumentoIdentificacion": p.tipo_id,
                "numDocumentoIdentificacion": p.identificacion,
                # El valor del servicio: de él depende el pago. Iba en 0 SIEMPRE.
                # Ojo: `0` es un valor legítimo —un servicio puede ser gratuito—
                # y por eso el recolector solo considera ausente el `None`.
                "vrServicio": exigir(getattr(h, 'valor_servicio', None),
                                     "valor_servicio", "valor del servicio", 0),
                "conceptoRecaudo": exigir(getattr(h, 'concepto_recaudo', None),
                                          "concepto_recaudo",
                                          "concepto de recaudo", "05"),
                # El pago moderador SÍ admite cero de verdad: la mayoría de las
                # atenciones no lo llevan. Cero aquí es un dato, no un hueco.
                "valorPagoModerador": getattr(h, 'valor_pago_moderador', None) or 0,
                "numFEVPagoModerador": getattr(h, 'num_fev_pago_moderador', None) or "",
                "consecutivo": len(usuarios_map[p.id]["servicios"]["consultas"]) + 1,
            }
            usuarios_map[p.id]["servicios"]["consultas"].append(consulta)

        if propio:
            rec.cerrar()
        return {
            "numDocumentoIdObligado": nit_institucion,
            "numFactura": "",
            "tipoNota": "",
            "numNota": "",
            "usuarios": list(usuarios_map.values()),
        }


# ==============================================================================
# AP - PROCEDIMIENTOS
# ==============================================================================

def generar_archivo_ap(
    fecha_inicio: date,
    fecha_fin: date,
    institucion_id: int,
    *,
    permitir_supuestos: bool = False,
    _recolector: Optional["_Recolector"] = None,
) -> Dict:
    """
    AP - Archivo de Procedimientos
    Incluye laboratorios, imágenes diagnósticas, terapias, cirugías.
    """
    rec, propio = _resolver_recolector(_recolector, permitir_supuestos)
    with Session(engine) as session:
        institucion = _get_institucion(session, institucion_id)
        nit_institucion = institucion.nit
        cod_habilitacion = institucion.codigo_habilitacion

        q = (
            select(Orden)
            .options(
                joinedload(Orden.historia).joinedload(Historia.paciente),
                joinedload(Orden.historia).joinedload(Historia.medico),
            )
            .join(Historia, Orden.historia_id == Historia.id)
            .join(Paciente, Historia.paciente_id == Paciente.id)
            .where(
                Historia.fecha >= datetime.combine(fecha_inicio, datetime.min.time()),
                Historia.fecha <= datetime.combine(fecha_fin, datetime.max.time()),
            )
        )
        q = q.where(Paciente.institucion_id == institucion_id)

        ordenes = session.exec(q).all()

        usuarios_map: Dict[int, Dict] = {}
        consecutivo_usuario = 1

        modalidad_map = {
            "Laboratorio": "02",
            "Imagenología": "02",
            "Remisión Especialista": "01",
            "Cirugía": "01",
            "Terapia": "02",
            "Otro": "02",
        }

        for orden in ordenes:
            h = orden.historia
            if not h or not h.paciente:
                continue

            p = h.paciente

            if p.id not in usuarios_map:
                base = _usuario_base(p, consecutivo_usuario, rec)
                base["servicios"] = {"procedimientos": []}
                usuarios_map[p.id] = base
                consecutivo_usuario += 1

            tipo_orden_val = _valor_enum(orden.tipo) or str(orden.tipo)

            def exigir(valor, campo, etiqueta, supuesto=None):
                return rec.dato(valor, archivo="AP", registro_id=orden.id,
                                campo=campo, etiqueta=etiqueta, supuesto=supuesto)

            procedimiento = {
                "codPrestador": cod_habilitacion,
                "fechaInicioAtencion": _fecha_dt(h.fecha),
                "idMIPRES": "",
                "numAutorizacion": "",
                # `"999999"` no es un CUPS: es un procedimiento que no existe.
                # El validador lo rechaza y, si no lo rechazara, sería peor.
                "codProcedimiento": exigir(orden.codigo_cups, "codigo_cups",
                                           "CUPS del procedimiento", "999999"),
                "viaIngresoServicioSalud": exigir(
                    getattr(orden, 'via_ingreso_servicio', None),
                    "via_ingreso_servicio", "vía de ingreso al servicio", "01"),
                # La modalidad SÍ se deduce del tipo de orden, y eso es legítimo:
                # un laboratorio es apoyo diagnóstico por definición. Se deriva,
                # no se supone. Lo que no se puede es rellenar el que no encaje.
                "modalidadGrupoServicioTecSal": exigir(
                    modalidad_map.get(tipo_orden_val),
                    "tipo", f"modalidad del procedimiento ({tipo_orden_val})", "02"),
                "grupoServicios": exigir(getattr(orden, 'grupo_servicios', None),
                                         "grupo_servicios", "grupo de servicios", "02"),
                "codServicio": exigir(getattr(orden, 'cod_servicio', None),
                                      "cod_servicio", "código de servicio", 0),
                "finalidadTecnologiaSalud": exigir(
                    _valor_enum(getattr(h, 'finalidad', None)),
                    "finalidad", "finalidad del procedimiento", "10"),
                "tipoDocumentoIdentificacion": p.tipo_id,
                "numDocumentoIdentificacion": p.identificacion,
                "codDiagnosticoPrincipal": exigir(
                    h.diagnostico_principal, "diagnostico_principal",
                    "diagnóstico principal"),
                "codDiagnosticoRelacionado": h.diagnostico_relacionado1 or "",
                "codComplicacion": "",
                **_bloque_economico(orden, rec, "AP"),
                "consecutivo": len(usuarios_map[p.id]["servicios"]["procedimientos"]) + 1,
            }
            usuarios_map[p.id]["servicios"]["procedimientos"].append(procedimiento)

        if propio:
            rec.cerrar()
        return {
            "numDocumentoIdObligado": nit_institucion,
            "numFactura": "",
            "tipoNota": "",
            "numNota": "",
            "usuarios": list(usuarios_map.values()),
        }


# ==============================================================================
# AM - MEDICAMENTOS
# ==============================================================================

def generar_archivo_am(
    fecha_inicio: date,
    fecha_fin: date,
    institucion_id: int,
    *,
    permitir_supuestos: bool = False,
    _recolector: Optional["_Recolector"] = None,
) -> Dict:
    """
    AM - Archivo de Medicamentos
    Incluye todas las fórmulas dispensadas en el período.
    """
    rec, propio = _resolver_recolector(_recolector, permitir_supuestos)
    with Session(engine) as session:
        institucion = _get_institucion(session, institucion_id)
        nit_institucion = institucion.nit
        cod_habilitacion = institucion.codigo_habilitacion

        q = (
            select(Formula)
            .options(
                joinedload(Formula.historia).joinedload(Historia.paciente),
                joinedload(Formula.historia).joinedload(Historia.medico),
            )
            .join(Historia, Formula.historia_id == Historia.id)
            .join(Paciente, Historia.paciente_id == Paciente.id)
            .where(
                Historia.fecha >= datetime.combine(fecha_inicio, datetime.min.time()),
                Historia.fecha <= datetime.combine(fecha_fin, datetime.max.time()),
            )
        )
        q = q.where(Paciente.institucion_id == institucion_id)

        formulas = session.exec(q).all()

        usuarios_map: Dict[int, Dict] = {}
        consecutivo_usuario = 1

        for formula in formulas:
            h = formula.historia
            if not h or not h.paciente:
                continue

            p = h.paciente

            if p.id not in usuarios_map:
                base = _usuario_base(p, consecutivo_usuario, rec)
                base["servicios"] = {"medicamentos": []}
                usuarios_map[p.id] = base
                consecutivo_usuario += 1

            def exigir(valor, campo, etiqueta, supuesto=None):
                return rec.dato(valor, archivo="AM", registro_id=formula.id,
                                campo=campo, etiqueta=etiqueta, supuesto=supuesto)

            medicamento = {
                "codPrestador": cod_habilitacion,
                "fechaDispensacion": _fecha_d(h.fecha.date() if isinstance(h.fecha, datetime) else h.fecha),
                "idMIPRES": "",
                "numAutorizacion": "",
                # `"00000000"` no es un CUM. Es el campo que la Resolución 948
                # valida contra el listado del INVIMA, así que un CUM inventado
                # no solo miente: hace rechazar la factura entera.
                "codMedicamento": exigir(formula.codigo_cum, "codigo_cum",
                                         "CUM del medicamento", "00000000"),
                # 01 medicamento, 02 biológico, 03 magistral, 04 fitoterapéutico.
                # El `if` que había aquí devolvía "01" en las dos ramas: parecía
                # que clasificaba y no clasificaba nada.
                "tipoMedicamento": exigir(getattr(formula, 'tipo_medicamento', None),
                                          "tipo_medicamento",
                                          "tipo de medicamento", "01"),
                "nombreMedicamento": formula.medicamento,
                "cantidadMedicamento": float(formula.cantidad),
                "tipoDocumentoIdentificacion": p.tipo_id,
                "numDocumentoIdentificacion": p.identificacion,
                "vrUnitMedicamento": exigir(getattr(formula, 'valor_unitario', None),
                                            "valor_unitario",
                                            "valor unitario del medicamento", 0),
                **_bloque_economico(formula, rec, "AM"),
                "consecutivo": len(usuarios_map[p.id]["servicios"]["medicamentos"]) + 1,
            }
            usuarios_map[p.id]["servicios"]["medicamentos"].append(medicamento)

        if propio:
            rec.cerrar()
        return {
            "numDocumentoIdObligado": nit_institucion,
            "numFactura": "",
            "tipoNota": "",
            "numNota": "",
            "usuarios": list(usuarios_map.values()),
        }


# ==============================================================================
# AU - URGENCIAS
# ==============================================================================

def generar_archivo_au(
    fecha_inicio: date,
    fecha_fin: date,
    institucion_id: int,
    *,
    permitir_supuestos: bool = False,
    _recolector: Optional["_Recolector"] = None,
) -> Dict:
    """
    AU - Archivo de Urgencias
    Incluye todas las atenciones en el servicio de urgencias.
    Res. 2275/2023 - obligatorio para IPS con servicio de urgencias habilitado.

    Las atenciones SIN egreso no entran: no han terminado y no se pueden
    facturar. Antes se les ponía `fecha_egreso = datetime.now()`, o sea que un
    paciente todavía en urgencias se reportaba como dado de alta en el instante
    en que alguien generó el archivo.
    """
    rec, propio = _resolver_recolector(_recolector, permitir_supuestos)
    with Session(engine) as session:
        institucion = _get_institucion(session, institucion_id)
        nit_institucion = institucion.nit
        cod_habilitacion = institucion.codigo_habilitacion

        q = (
            select(AtencionUrgencias)
            .options(joinedload(AtencionUrgencias.paciente))
            .join(Paciente, AtencionUrgencias.paciente_id == Paciente.id)
            .where(
                AtencionUrgencias.fecha_ingreso >= datetime.combine(fecha_inicio, datetime.min.time()),
                AtencionUrgencias.fecha_ingreso <= datetime.combine(fecha_fin, datetime.max.time()),
            )
        )
        q = q.where(Paciente.institucion_id == institucion_id)

        atenciones = session.exec(q).all()

        usuarios_map: Dict[int, Dict] = {}
        consecutivo_usuario = 1

        # Mapa destino de salida → código RIPS
        destino_map = {
            "ALTA": "01",
            "HOSPITALIZADO": "02",
            "REMITIDO": "03",
            "FALLECIDO": "04",
            "FUGA": "05",
            "OBSERVACION": "06",
        }

        for au in atenciones:
            p = au.paciente
            if not p:
                continue
            if au.fecha_egreso is None:
                # Sigue en urgencias: se factura cuando termine, en el RIPS del
                # periodo que corresponda.
                continue

            if p.id not in usuarios_map:
                base = _usuario_base(p, consecutivo_usuario, rec)
                base["servicios"] = {"urgencias": []}
                usuarios_map[p.id] = base
                consecutivo_usuario += 1

            def exigir(valor, campo, etiqueta, supuesto=None):
                return rec.dato(valor, archivo="AU", registro_id=au.id,
                                campo=campo, etiqueta=etiqueta, supuesto=supuesto)

            destino_val = str(_valor_enum(au.destino_salida) or "").upper()

            urgencia = {
                "codPrestador": cod_habilitacion,
                # Esta sí es constante legítima: es el archivo de urgencias, la
                # vía de ingreso ES urgencias. No se supone nada.
                "viaIngresoServicioSalud": "02",
                "fechaInicioAtencion": _fecha_dt(au.fecha_ingreso),
                "numAutorizacion": "",
                # Iba quemada en "17". Un accidente de trabajo, uno de tránsito
                # y una lesión por agresión llegan todos por urgencias y son
                # causas DISTINTAS, con responsables de pago distintos.
                "causaMotivo": exigir(getattr(au, 'causa_motivo', None),
                                      "causa_motivo", "causa o motivo de la atención",
                                      "17"),
                # `"R69"` es «causas de morbilidad desconocidas»: un diagnóstico
                # de verdad, inventado para un paciente real.
                "codDiagnosticoPrincipal": exigir(
                    au.diagnostico_ingreso, "diagnostico_ingreso",
                    "diagnóstico de ingreso", "R69"),
                "codDiagnosticoRelacionado": au.diagnostico_egreso or "",
                "codComplicacion": "",
                "condicionDestinoUsuarioEgreso": exigir(
                    destino_map.get(destino_val), "destino_salida",
                    "destino de salida", "01"),
                "codDiagnosticoCausaMuerte": "",
                "fechaEgresoAtencion": _fecha_dt(au.fecha_egreso),
                "tipoDocumentoIdentificacion": p.tipo_id,
                "numDocumentoIdentificacion": p.identificacion,
                **_bloque_economico(au, rec, "AU"),
                "consecutivo": len(usuarios_map[p.id]["servicios"]["urgencias"]) + 1,
            }
            usuarios_map[p.id]["servicios"]["urgencias"].append(urgencia)

        if propio:
            rec.cerrar()
        return {
            "numDocumentoIdObligado": nit_institucion,
            "numFactura": "",
            "tipoNota": "",
            "numNota": "",
            "usuarios": list(usuarios_map.values()),
        }


# ==============================================================================
# AH - HOSPITALIZACIÓN
# ==============================================================================

def generar_archivo_ah(
    fecha_inicio: date,
    fecha_fin: date,
    institucion_id: int,
    *,
    permitir_supuestos: bool = False,
    _recolector: Optional["_Recolector"] = None,
) -> Dict:
    """
    AH - Archivo de Hospitalización
    Incluye todos los ingresos hospitalarios en el período.
    Res. 2275/2023 - obligatorio para IPS con servicio de hospitalización.

    Las hospitalizaciones ACTIVAS no entran. El código ya lo sabía —el mapa de
    estados decía «no debería reportarse»— y las reportaba igual, con egreso
    en `datetime.now()` y estancia calculada hasta ese instante.
    """
    rec, propio = _resolver_recolector(_recolector, permitir_supuestos)
    with Session(engine) as session:
        institucion = _get_institucion(session, institucion_id)
        nit_institucion = institucion.nit
        cod_habilitacion = institucion.codigo_habilitacion

        q = (
            select(Hospitalizacion)
            .options(
                joinedload(Hospitalizacion.paciente),
                joinedload(Hospitalizacion.medico_tratante),
            )
            .join(Paciente, Hospitalizacion.paciente_id == Paciente.id)
            .where(
                Hospitalizacion.fecha_ingreso >= datetime.combine(fecha_inicio, datetime.min.time()),
                Hospitalizacion.fecha_ingreso <= datetime.combine(fecha_fin, datetime.max.time()),
            )
        )
        q = q.where(Paciente.institucion_id == institucion_id)

        hospitalizaciones = session.exec(q).all()

        usuarios_map: Dict[int, Dict] = {}
        consecutivo_usuario = 1

        # Vía de ingreso → código RIPS
        via_map = {
            "URGENCIAS": "02",
            "PROGRAMADA": "03",
            "REMISION": "04",
        }

        # Estado de egreso → condición destino RIPS
        estado_egreso_map = {
            "ALTA_MEDICA": "01",
            "ALTA_VOLUNTARIA": "01",
            "REMISION": "03",
            "DEFUNCION": "04",
            "ACTIVA": "00",  # Aún hospitalizado (no debería reportarse)
        }

        for h in hospitalizaciones:
            p = h.paciente
            if not p:
                continue
            if h.fecha_egreso is None or (h.estado or "").upper() == "ACTIVA":
                # Sigue hospitalizado: no hay estancia que facturar todavía.
                continue

            if p.id not in usuarios_map:
                base = _usuario_base(p, consecutivo_usuario, rec)
                base["servicios"] = {"hospitalizacion": []}
                usuarios_map[p.id] = base
                consecutivo_usuario += 1

            def exigir(valor, campo, etiqueta, supuesto=None):
                return rec.dato(valor, archivo="AH", registro_id=h.id,
                                campo=campo, etiqueta=etiqueta, supuesto=supuesto)

            dias_estancia = max(1, (h.fecha_egreso - h.fecha_ingreso).days)
            via_ingreso = (h.via_ingreso or "").upper()
            estado = (h.estado or "").upper()

            hospitalizacion_rec = {
                "codPrestador": cod_habilitacion,
                "viaIngresoServicioSalud": exigir(
                    via_map.get(via_ingreso), "via_ingreso",
                    "vía de ingreso a la hospitalización", "03"),
                "fechaInicioAtencion": _fecha_dt(h.fecha_ingreso),
                "numAutorizacion": "",
                # Iba quemada en "13", enfermedad general. Con eso, ninguna
                # hospitalización por accidente de tránsito se cobraba al SOAT.
                "causaMotivo": exigir(getattr(h, 'causa_motivo', None),
                                      "causa_motivo",
                                      "causa o motivo de la atención", "13"),
                # `"Z00"` es «examen general»; además no es un código válido del
                # RIPS, que exige cuatro caracteres. Un ingreso hospitalario por
                # examen general no existe.
                "codDiagnosticoPrincipal": exigir(
                    h.diagnostico_ingreso, "diagnostico_ingreso",
                    "diagnóstico de ingreso", "Z00"),
                "codDiagnosticoRelacionado": h.diagnostico_egreso or "",
                "codComplicacion": "",
                "condicionDestinoUsuarioEgreso": exigir(
                    estado_egreso_map.get(estado), "estado",
                    "condición de egreso", "01"),
                "codDiagnosticoCausaMuerte": "",
                "fechaEgresoAtencion": _fecha_dt(h.fecha_egreso),
                "diasEstancia": dias_estancia,
                "tipoDocumentoIdentificacion": p.tipo_id,
                "numDocumentoIdentificacion": p.identificacion,
                **_bloque_economico(h, rec, "AH"),
                "consecutivo": len(usuarios_map[p.id]["servicios"]["hospitalizacion"]) + 1,
            }
            usuarios_map[p.id]["servicios"]["hospitalizacion"].append(hospitalizacion_rec)

        if propio:
            rec.cerrar()

        return {
            "numDocumentoIdObligado": nit_institucion,
            "numFactura": "",
            "tipoNota": "",
            "numNota": "",
            "usuarios": list(usuarios_map.values()),
        }


# ==============================================================================
# CT - CONTROL
# ==============================================================================

def generar_archivo_ct(
    fecha_inicio: date,
    fecha_fin: date,
    institucion_id: int,
    conteos: Optional[Dict[str, int]] = None,
    *,
    permitir_supuestos: bool = False,
    _recolector: Optional["_Recolector"] = None,
) -> Dict:
    """
    CT - Archivo de Control
    Resumen de registros enviados. Debe coincidir con los conteos reales.

    Args:
        conteos: Si se proveen, se usan directamente. Si no, se recalculan
                 (útil cuando se llama de forma independiente).
    """
    with Session(engine) as session:
        institucion = _get_institucion(session, institucion_id)
        nit_institucion = institucion.nit
        cod_habilitacion = institucion.codigo_habilitacion

        if conteos is None:
            from sqlalchemy import func

            def _count_q(Model, join_model=None):
                q = select(func.count()).select_from(Model)
                if join_model:
                    q = q.join(join_model)
                return q

            dt_inicio = datetime.combine(fecha_inicio, datetime.min.time())
            dt_fin = datetime.combine(fecha_fin, datetime.max.time())

            q_ac = select(func.count(Historia.id)).where(
                Historia.fecha >= dt_inicio, Historia.fecha <= dt_fin
            )
            q_ap = (
                select(func.count(Orden.id))
                .join(Historia, Orden.historia_id == Historia.id)
                .where(Historia.fecha >= dt_inicio, Historia.fecha <= dt_fin)
            )
            q_am = (
                select(func.count(Formula.id))
                .join(Historia, Formula.historia_id == Historia.id)
                .where(Historia.fecha >= dt_inicio, Historia.fecha <= dt_fin)
            )
            q_au = select(func.count(AtencionUrgencias.id)).where(
                AtencionUrgencias.fecha_ingreso >= dt_inicio,
                AtencionUrgencias.fecha_ingreso <= dt_fin,
            )
            q_ah = select(func.count(Hospitalizacion.id)).where(
                Hospitalizacion.fecha_ingreso >= dt_inicio,
                Hospitalizacion.fecha_ingreso <= dt_fin,
            )

            # El CT declara CUÁNTOS registros lleva el envío. Si contara sin
            # filtrar mientras los otros cinco archivos sí filtran, el conteo no
            # cuadraría con el contenido y el MUV rechaza el paquete entero.
            q_ac = q_ac.join(Paciente, Historia.paciente_id == Paciente.id).where(
                Paciente.institucion_id == institucion_id
            )
            q_ap = q_ap.join(Paciente, Historia.paciente_id == Paciente.id).where(
                Paciente.institucion_id == institucion_id
            )
            q_am = q_am.join(Paciente, Historia.paciente_id == Paciente.id).where(
                Paciente.institucion_id == institucion_id
            )
            q_au = q_au.join(Paciente, AtencionUrgencias.paciente_id == Paciente.id).where(
                Paciente.institucion_id == institucion_id
            )
            q_ah = q_ah.join(Paciente, Hospitalizacion.paciente_id == Paciente.id).where(
                Paciente.institucion_id == institucion_id
            )

            num_ac = session.exec(q_ac).one() or 0
            num_ap = session.exec(q_ap).one() or 0
            num_am = session.exec(q_am).one() or 0
            num_au = session.exec(q_au).one() or 0
            num_ah = session.exec(q_ah).one() or 0
        else:
            num_ac = conteos.get("ac", 0)
            num_ap = conteos.get("ap", 0)
            num_am = conteos.get("am", 0)
            num_au = conteos.get("au", 0)
            num_ah = conteos.get("ah", 0)

        total = num_ac + num_ap + num_am + num_au + num_ah

        return {
            "numDocumentoIdObligado": nit_institucion,
            "numFactura": "",
            "tipoNota": "",
            "numNota": "",
            "control": {
                "codPrestador": cod_habilitacion,
                "fechaRemision": datetime.now().strftime('%Y-%m-%d'),
                "numRegistros": total,
                "registrosConsultas": num_ac,
                "registrosProcedimientos": num_ap,
                "registrosMedicamentos": num_am,
                "registrosUrgencias": num_au,
                "registrosHospitalizacion": num_ah,
                "registrosRecienNacidos": 0,
                "registrosOtrosServicios": 0,
            },
        }


# ==============================================================================
# EXPORTACIÓN COMPLETA
# ==============================================================================

def exportar_rips_completo(
    fecha_inicio: date,
    fecha_fin: date,
    institucion_id: int,
    *,
    permitir_supuestos: bool = False,
) -> Dict[str, Dict]:
    """
    Genera todos los archivos RIPS en un solo llamado.

    Returns:
        {"AC": {...}, "AP": {...}, "AM": {...}, "AU": {...}, "AH": {...}, "CT": {...}}
    """
    # Un solo recolector para los cinco: quien cierra el mes recibe la lista
    # COMPLETA de lo que falta, no la del primer archivo que reventó.
    rec = _Recolector(permitir_supuestos)
    ac = generar_archivo_ac(fecha_inicio, fecha_fin, institucion_id, _recolector=rec)
    ap = generar_archivo_ap(fecha_inicio, fecha_fin, institucion_id, _recolector=rec)
    am = generar_archivo_am(fecha_inicio, fecha_fin, institucion_id, _recolector=rec)
    au = generar_archivo_au(fecha_inicio, fecha_fin, institucion_id, _recolector=rec)
    ah = generar_archivo_ah(fecha_inicio, fecha_fin, institucion_id, _recolector=rec)
    rec.cerrar()

    # Conteos reales desde los archivos ya generados
    conteos = {
        "ac": sum(len(u["servicios"].get("consultas", [])) for u in ac["usuarios"]),
        "ap": sum(len(u["servicios"].get("procedimientos", [])) for u in ap["usuarios"]),
        "am": sum(len(u["servicios"].get("medicamentos", [])) for u in am["usuarios"]),
        "au": sum(len(u["servicios"].get("urgencias", [])) for u in au["usuarios"]),
        "ah": sum(len(u["servicios"].get("hospitalizacion", [])) for u in ah["usuarios"]),
    }

    ct = generar_archivo_ct(fecha_inicio, fecha_fin, institucion_id, conteos=conteos)

    return {"AC": ac, "AP": ap, "AM": am, "AU": au, "AH": ah, "CT": ct}


def exportar_rips_zip(
    fecha_inicio: date,
    fecha_fin: date,
    institucion_id: int,
    *,
    permitir_supuestos: bool = False,
) -> bytes:
    """
    Genera todos los archivos RIPS y los empaqueta en un ZIP en memoria.

    Returns:
        bytes del archivo ZIP listo para descargar.

    Uso desde NiceGUI:
        zip_bytes = exportar_rips_zip(f_ini, f_fin, inst_id)
        ui.download(zip_bytes, f'RIPS_{f_ini}_{f_fin}.zip')
    """
    archivos = exportar_rips_completo(fecha_inicio, fecha_fin, institucion_id,
                                      permitir_supuestos=permitir_supuestos)

    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'w', zipfile.ZIP_DEFLATED) as zf:
        for nombre, contenido in archivos.items():
            json_str = json.dumps(contenido, indent=2, ensure_ascii=False)
            zf.writestr(f"{nombre}.json", json_str)

    buf.seek(0)
    return buf.read()
