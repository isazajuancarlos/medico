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
from typing import Dict, List, Optional

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

def _get_institucion(session: Session, institucion_id: Optional[int]) -> Optional[Institucion]:
    """Obtiene institución por ID o la primera disponible (fallback single-tenant)."""
    if institucion_id:
        return session.get(Institucion, institucion_id)
    return session.exec(select(Institucion).limit(1)).first()


def _cod_zona(p: Paciente) -> str:
    """Extrae código de zona territorial del paciente."""
    zona = getattr(p, 'zona', None)
    if zona is None:
        return "01"
    if hasattr(zona, 'value'):
        return zona.value
    return str(zona) or "01"


def _usuario_base(p: Paciente, consecutivo: int) -> Dict:
    """Construye estructura de identificación de usuario RIPS."""
    tipo_usuario = getattr(p, 'tipo_usuario', None)
    return {
        "tipoDocumentoIdentificacion": p.tipo_id,
        "numDocumentoIdentificacion": p.identificacion,
        "tipoUsuario": tipo_usuario.value if hasattr(tipo_usuario, 'value') else "01",
        "fechaNacimiento": p.fecha_nacimiento.strftime('%Y-%m-%d'),
        "codSexo": p.sexo,
        "codPaisResidencia": getattr(p, 'cod_pais', "170"),
        "codMunicipioResidencia": getattr(p, 'cod_municipio', "05001"),
        "codZonaTerritorialResidencia": _cod_zona(p),
        "codPaisOrigen": getattr(p, 'cod_pais_origen', "170"),
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
    institucion_id: Optional[int] = None
) -> Dict:
    """
    AC - Archivo de Consultas (Obligatorio)
    Incluye todas las atenciones ambulatorias y consultas de control.
    """
    with Session(engine) as session:
        institucion = _get_institucion(session, institucion_id)
        nit_institucion = institucion.nit if institucion else "000000000"
        cod_habilitacion = institucion.codigo_habilitacion if institucion else "000000000000"

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
        if institucion_id:
            q = q.where(Paciente.institucion_id == institucion_id)

        historias = session.exec(q).all()

        usuarios_map: Dict[int, Dict] = {}
        consecutivo_usuario = 1

        for h in historias:
            p = h.paciente
            if not p:
                continue

            if p.id not in usuarios_map:
                base = _usuario_base(p, consecutivo_usuario)
                base["incapacidad"] = "SI" if h.genera_incapacidad else "NO"
                base["servicios"] = {"consultas": []}
                usuarios_map[p.id] = base
                consecutivo_usuario += 1

            finalidad = getattr(h, 'finalidad', None)
            causa_externa = getattr(h, 'causa_externa', None)
            tipo_dx = getattr(h, 'tipo_diagnostico', None)
            num_autorizacion = ""
            if h.cita and hasattr(h.cita, 'num_autorizacion') and h.cita.num_autorizacion:
                num_autorizacion = h.cita.num_autorizacion

            consulta = {
                "codPrestador": cod_habilitacion,
                "fechaInicioAtencion": _fecha_dt(h.fecha),
                "numAutorizacion": num_autorizacion,
                "codConsulta": getattr(h, 'cod_consulta_cups', "890201"),
                "modalidadGrupoServicioTecSal": "01",  # Intramural
                "grupoServicios": "01",  # Consulta Externa
                "codServicio": 328,
                "finalidadTecnologiaSalud": finalidad.value if hasattr(finalidad, 'value') else "10",
                "causaMotivoAtencion": causa_externa.value if hasattr(causa_externa, 'value') else "13",
                "codDiagnosticoPrincipal": h.diagnostico_principal,
                "codDiagnosticoRelacionado1": h.diagnostico_relacionado1 or "",
                "codDiagnosticoRelacionado2": "",
                "codDiagnosticoRelacionado3": "",
                "tipoDiagnosticoPrincipal": tipo_dx.value if hasattr(tipo_dx, 'value') else "01",
                "tipoDocumentoIdentificacion": p.tipo_id,
                "numDocumentoIdentificacion": p.identificacion,
                "vrServicio": 0,
                "conceptoRecaudo": "05",
                "valorPagoModerador": 0,
                "numFEVPagoModerador": "",
                "consecutivo": len(usuarios_map[p.id]["servicios"]["consultas"]) + 1,
            }
            usuarios_map[p.id]["servicios"]["consultas"].append(consulta)

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
    institucion_id: Optional[int] = None
) -> Dict:
    """
    AP - Archivo de Procedimientos
    Incluye laboratorios, imágenes diagnósticas, terapias, cirugías.
    """
    with Session(engine) as session:
        institucion = _get_institucion(session, institucion_id)
        nit_institucion = institucion.nit if institucion else "000000000"
        cod_habilitacion = institucion.codigo_habilitacion if institucion else "000000000000"

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
        if institucion_id:
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
                base = _usuario_base(p, consecutivo_usuario)
                base["servicios"] = {"procedimientos": []}
                usuarios_map[p.id] = base
                consecutivo_usuario += 1

            finalidad = getattr(h, 'finalidad', None)
            tipo_orden_val = orden.tipo.value if hasattr(orden.tipo, 'value') else str(orden.tipo)

            procedimiento = {
                "codPrestador": cod_habilitacion,
                "fechaInicioAtencion": _fecha_dt(h.fecha),
                "idMIPRES": "",
                "numAutorizacion": "",
                "codProcedimiento": orden.codigo_cups or "999999",
                "viaIngresoServicioSalud": "01",
                "modalidadGrupoServicioTecSal": modalidad_map.get(tipo_orden_val, "02"),
                "grupoServicios": "02",
                "codServicio": 0,
                "finalidadTecnologiaSalud": finalidad.value if hasattr(finalidad, 'value') else "10",
                "tipoDocumentoIdentificacion": p.tipo_id,
                "numDocumentoIdentificacion": p.identificacion,
                "codDiagnosticoPrincipal": h.diagnostico_principal,
                "codDiagnosticoRelacionado": h.diagnostico_relacionado1 or "",
                "codComplicacion": "",
                "vrServicio": 0,
                "conceptoRecaudo": "05",
                "valorPagoModerador": 0,
                "numFEVPagoModerador": "",
                "consecutivo": len(usuarios_map[p.id]["servicios"]["procedimientos"]) + 1,
            }
            usuarios_map[p.id]["servicios"]["procedimientos"].append(procedimiento)

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
    institucion_id: Optional[int] = None
) -> Dict:
    """
    AM - Archivo de Medicamentos
    Incluye todas las fórmulas dispensadas en el período.
    """
    with Session(engine) as session:
        institucion = _get_institucion(session, institucion_id)
        nit_institucion = institucion.nit if institucion else "000000000"
        cod_habilitacion = institucion.codigo_habilitacion if institucion else "000000000000"

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
        if institucion_id:
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
                base = _usuario_base(p, consecutivo_usuario)
                base["servicios"] = {"medicamentos": []}
                usuarios_map[p.id] = base
                consecutivo_usuario += 1

            # Tipo medicamento según si es controlado (INVIMA clasificación básica)
            # 01=Medicamento, 02=Biológico, 03=Magistral, 04=Fitoterapéutico
            tipo_med = "01"
            if hasattr(formula, 'es_controlado') and formula.es_controlado:
                tipo_med = "01"  # Controlados siguen siendo "01" (medicamento)

            medicamento = {
                "codPrestador": cod_habilitacion,
                "fechaDispensacion": _fecha_d(h.fecha.date() if isinstance(h.fecha, datetime) else h.fecha),
                "idMIPRES": "",
                "numAutorizacion": "",
                "codMedicamento": formula.codigo_cum or "00000000",
                "tipoMedicamento": tipo_med,
                "nombreMedicamento": formula.medicamento,
                "cantidadMedicamento": float(formula.cantidad),
                "tipoDocumentoIdentificacion": p.tipo_id,
                "numDocumentoIdentificacion": p.identificacion,
                "vrUnitMedicamento": 0,
                "vrServicio": 0,
                "conceptoRecaudo": "05",
                "valorPagoModerador": 0,
                "numFEVPagoModerador": "",
                "consecutivo": len(usuarios_map[p.id]["servicios"]["medicamentos"]) + 1,
            }
            usuarios_map[p.id]["servicios"]["medicamentos"].append(medicamento)

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
    institucion_id: Optional[int] = None
) -> Dict:
    """
    AU - Archivo de Urgencias
    Incluye todas las atenciones en el servicio de urgencias.
    Res. 2275/2023 - obligatorio para IPS con servicio de urgencias habilitado.
    """
    with Session(engine) as session:
        institucion = _get_institucion(session, institucion_id)
        nit_institucion = institucion.nit if institucion else "000000000"
        cod_habilitacion = institucion.codigo_habilitacion if institucion else "000000000000"

        q = (
            select(AtencionUrgencias)
            .options(joinedload(AtencionUrgencias.paciente))
            .join(Paciente, AtencionUrgencias.paciente_id == Paciente.id)
            .where(
                AtencionUrgencias.fecha_ingreso >= datetime.combine(fecha_inicio, datetime.min.time()),
                AtencionUrgencias.fecha_ingreso <= datetime.combine(fecha_fin, datetime.max.time()),
            )
        )
        if institucion_id:
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

            if p.id not in usuarios_map:
                base = _usuario_base(p, consecutivo_usuario)
                base["servicios"] = {"urgencias": []}
                usuarios_map[p.id] = base
                consecutivo_usuario += 1

            destino = au.destino_salida
            destino_val = destino.value if hasattr(destino, 'value') else str(destino or "")
            cod_destino = destino_map.get(destino_val.upper(), "01")

            fecha_egreso = au.fecha_egreso or datetime.now()

            urgencia = {
                "codPrestador": cod_habilitacion,
                "viaIngresoServicioSalud": "02",  # 02 = Urgencias
                "fechaInicioAtencion": _fecha_dt(au.fecha_ingreso),
                "numAutorizacion": "",
                "causaMotivo": "17",  # 17 = Urgencia (causa externa genérica)
                "codDiagnosticoPrincipal": au.diagnostico_ingreso or "R69",
                "codDiagnosticoRelacionado": au.diagnostico_egreso or "",
                "codComplicacion": "",
                "condicionDestinoUsuarioEgreso": cod_destino,
                "codDiagnosticoCausaMuerte": "",
                "fechaEgresoAtencion": _fecha_dt(fecha_egreso),
                "tipoDocumentoIdentificacion": p.tipo_id,
                "numDocumentoIdentificacion": p.identificacion,
                "vrServicio": 0,
                "conceptoRecaudo": "05",
                "valorPagoModerador": 0,
                "numFEVPagoModerador": "",
                "consecutivo": len(usuarios_map[p.id]["servicios"]["urgencias"]) + 1,
            }
            usuarios_map[p.id]["servicios"]["urgencias"].append(urgencia)

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
    institucion_id: Optional[int] = None
) -> Dict:
    """
    AH - Archivo de Hospitalización
    Incluye todos los ingresos hospitalarios en el período.
    Res. 2275/2023 - obligatorio para IPS con servicio de hospitalización.
    """
    with Session(engine) as session:
        institucion = _get_institucion(session, institucion_id)
        nit_institucion = institucion.nit if institucion else "000000000"
        cod_habilitacion = institucion.codigo_habilitacion if institucion else "000000000000"

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
        if institucion_id:
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

            if p.id not in usuarios_map:
                base = _usuario_base(p, consecutivo_usuario)
                base["servicios"] = {"hospitalizacion": []}
                usuarios_map[p.id] = base
                consecutivo_usuario += 1

            fecha_egreso = h.fecha_egreso or datetime.now()
            dias_estancia = max(1, (fecha_egreso - h.fecha_ingreso).days)
            via_ingreso = (h.via_ingreso or "PROGRAMADA").upper()
            estado = (h.estado or "ALTA_MEDICA").upper()

            hospitalizacion_rec = {
                "codPrestador": cod_habilitacion,
                "viaIngresoServicioSalud": via_map.get(via_ingreso, "03"),
                "fechaInicioAtencion": _fecha_dt(h.fecha_ingreso),
                "numAutorizacion": "",
                "causaMotivo": "13",  # Enfermedad general
                "codDiagnosticoPrincipal": h.diagnostico_ingreso or "Z00",
                "codDiagnosticoRelacionado": h.diagnostico_egreso or "",
                "codComplicacion": "",
                "condicionDestinoUsuarioEgreso": estado_egreso_map.get(estado, "01"),
                "codDiagnosticoCausaMuerte": "",
                "fechaEgresoAtencion": _fecha_dt(fecha_egreso),
                "diasEstancia": dias_estancia,
                "tipoDocumentoIdentificacion": p.tipo_id,
                "numDocumentoIdentificacion": p.identificacion,
                "vrServicio": 0,
                "conceptoRecaudo": "05",
                "valorPagoModerador": 0,
                "numFEVPagoModerador": "",
                "consecutivo": len(usuarios_map[p.id]["servicios"]["hospitalizacion"]) + 1,
            }
            usuarios_map[p.id]["servicios"]["hospitalizacion"].append(hospitalizacion_rec)

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
    institucion_id: Optional[int] = None,
    conteos: Optional[Dict[str, int]] = None
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
        nit_institucion = institucion.nit if institucion else "000000000"
        cod_habilitacion = institucion.codigo_habilitacion if institucion else "000000000000"

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

            if institucion_id:
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
    institucion_id: Optional[int] = None
) -> Dict[str, Dict]:
    """
    Genera todos los archivos RIPS en un solo llamado.

    Returns:
        {"AC": {...}, "AP": {...}, "AM": {...}, "AU": {...}, "AH": {...}, "CT": {...}}
    """
    ac = generar_archivo_ac(fecha_inicio, fecha_fin, institucion_id)
    ap = generar_archivo_ap(fecha_inicio, fecha_fin, institucion_id)
    am = generar_archivo_am(fecha_inicio, fecha_fin, institucion_id)
    au = generar_archivo_au(fecha_inicio, fecha_fin, institucion_id)
    ah = generar_archivo_ah(fecha_inicio, fecha_fin, institucion_id)

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
    institucion_id: Optional[int] = None
) -> bytes:
    """
    Genera todos los archivos RIPS y los empaqueta en un ZIP en memoria.

    Returns:
        bytes del archivo ZIP listo para descargar.

    Uso desde NiceGUI:
        zip_bytes = exportar_rips_zip(f_ini, f_fin, inst_id)
        ui.download(zip_bytes, f'RIPS_{f_ini}_{f_fin}.zip')
    """
    archivos = exportar_rips_completo(fecha_inicio, fecha_fin, institucion_id)

    buf = io.BytesIO()
    with zipfile.ZipFile(buf, 'w', zipfile.ZIP_DEFLATED) as zf:
        for nombre, contenido in archivos.items():
            json_str = json.dumps(contenido, indent=2, ensure_ascii=False)
            zf.writestr(f"{nombre}.json", json_str)

    buf.seek(0)
    return buf.read()
