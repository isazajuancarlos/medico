from datetime import date
from typing import Optional
from fastapi import APIRouter, Depends, Query
from sqlmodel import Session, select, func
from sqlalchemy import extract, and_, cast, Date
from models import (
    Cita, Paciente, Historia, Usuario, AtencionUrgencias,
    Factura, Cama, Hospitalizacion, EstadoCita, TipoTriage,
)
from models import ModuloSistema
from core.deps import get_db, get_current_active_user, requiere_modulo
# La condición de institución de una factura y de una glosa vive en UN sitio
# (`routers/facturacion.py`), y se importa en vez de copiarse: dos escrituras
# de la misma regla de aislamiento es una que se arregla en un sitio y se
# olvida en el otro — que es exactamente cómo este endpoint acabó sin filtro
# mientras el de al lado sí lo tenía (directiva 1).
from routers.facturacion import _de_mi_institucion

router = APIRouter(prefix="/estadisticas", tags=["estadisticas"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.ESTADISTICAS))],
)


def _pac_sub(db: Session, user: Usuario):
    """Subquery of paciente IDs scoped to user's institution (None = no filter)."""
    if user.institucion_id:
        return select(Paciente.id).where(Paciente.institucion_id == user.institucion_id).subquery()
    return None


# ─────────────────────────────────────────────────────────────────────────────
# RESUMEN GENERAL (KPI cards)
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/resumen")
def get_resumen(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    hoy = date.today()
    inicio_mes = hoy.replace(day=1)
    inst_id = current_user.institucion_id
    sub = _pac_sub(db, current_user)

    # Pacientes
    q_pac = select(func.count(Paciente.id))
    if inst_id:
        q_pac = q_pac.where(Paciente.institucion_id == inst_id)
    total_pacientes = db.exec(q_pac).one()

    # Citas
    def _cita_count(extra=None):
        q = select(func.count(Cita.id))
        if sub is not None:
            q = q.where(Cita.paciente_id.in_(sub))
        if extra is not None:
            q = q.where(extra)
        return db.exec(q).one()

    citas_hoy = _cita_count(cast(Cita.fecha_hora, Date) == hoy)
    citas_mes = _cita_count(cast(Cita.fecha_hora, Date) >= inicio_mes)
    citas_atendidas_mes = _cita_count(and_(
        cast(Cita.fecha_hora, Date) >= inicio_mes,
        Cita.estado == EstadoCita.ATENDIDA
    ))

    # Consultas (historias) este mes
    q_hist = select(func.count(Historia.id)).where(cast(Historia.fecha, Date) >= inicio_mes)
    if sub is not None:
        q_hist = q_hist.where(Historia.paciente_id.in_(sub))
    consultas_mes = db.exec(q_hist).one()

    # Urgencias activas
    q_urg = select(func.count(AtencionUrgencias.id)).where(AtencionUrgencias.en_atencion == True)
    if sub is not None:
        q_urg = q_urg.where(AtencionUrgencias.paciente_id.in_(sub))
    urgencias_activas = db.exec(q_urg).one()

    # Hospitalizaciones activas
    q_hosp = select(func.count(Hospitalizacion.id)).where(Hospitalizacion.activo == True)
    if sub is not None:
        q_hosp = q_hosp.where(Hospitalizacion.paciente_id.in_(sub))
    hospitalizados = db.exec(q_hosp).one()

    # Camas
    total_camas = db.exec(select(func.count(Cama.id))).one()
    camas_ocupadas = db.exec(select(func.count(Cama.id)).where(Cama.estado == "OCUPADA")).one()
    camas_disponibles = db.exec(select(func.count(Cama.id)).where(Cama.estado == "DISPONIBLE")).one()
    tasa_ocupacion = round((camas_ocupadas / total_camas * 100) if total_camas else 0, 1)

    # Facturación pendiente
    q_fact = select(func.count(Factura.id), func.sum(Factura.valor_neto)).where(
        Factura.estado.in_(["BORRADOR", "RADICADA"])
    )
    fact_row = db.exec(q_fact).one()

    return {
        "fecha": hoy.isoformat(),
        "total_pacientes": total_pacientes,
        "citas_hoy": citas_hoy,
        "citas_mes": citas_mes,
        "citas_atendidas_mes": citas_atendidas_mes,
        "tasa_asistencia": round((citas_atendidas_mes / citas_mes * 100) if citas_mes else 0, 1),
        "consultas_mes": consultas_mes,
        "urgencias_activas": urgencias_activas,
        "hospitalizados": hospitalizados,
        "total_camas": total_camas,
        "camas_disponibles": camas_disponibles,
        "camas_ocupadas": camas_ocupadas,
        "tasa_ocupacion": tasa_ocupacion,
        "facturas_pendientes": fact_row[0] or 0,
        "valor_pendiente": float(fact_row[1] or 0),
    }


# ─────────────────────────────────────────────────────────────────────────────
# CITAS
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/citas")
def get_citas(
    fecha_inicio: Optional[date] = Query(default=None),
    fecha_fin: Optional[date] = Query(default=None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    if not fecha_inicio:
        fecha_inicio = date.today().replace(day=1)
    if not fecha_fin:
        fecha_fin = date.today()

    sub = _pac_sub(db, current_user)

    def _q(cols):
        q = select(*cols).where(
            cast(Cita.fecha_hora, Date) >= fecha_inicio,
            cast(Cita.fecha_hora, Date) <= fecha_fin,
        )
        if sub is not None:
            q = q.where(Cita.paciente_id.in_(sub))
        return q

    # Por estado
    by_estado = {r[0]: r[1] for r in db.exec(_q([Cita.estado, func.count(Cita.id)]).group_by(Cita.estado)).all()}

    # Por tipo
    by_tipo = {r[0]: r[1] for r in db.exec(_q([Cita.tipo, func.count(Cita.id)]).group_by(Cita.tipo)).all()}

    # Tendencia (diaria si <= 60 días, mensual si más)
    delta = (fecha_fin - fecha_inicio).days
    if delta <= 60:
        q_trend = _q([cast(Cita.fecha_hora, Date).label("d"), func.count(Cita.id)]).group_by("d").order_by("d")
        trend = [{"fecha": str(r[0]), "cantidad": r[1]} for r in db.exec(q_trend).all()]
    else:
        q_trend = (
            _q([
                extract("year", Cita.fecha_hora).label("y"),
                extract("month", Cita.fecha_hora).label("m"),
                func.count(Cita.id)
            ])
            .group_by("y", "m").order_by("y", "m")
        )
        trend = [{"fecha": f"{int(r[0])}-{int(r[1]):02d}", "cantidad": r[2]} for r in db.exec(q_trend).all()]

    # Por médico (top 10)
    q_med = _q([Cita.medico_id, func.count(Cita.id).label("c")]).group_by(Cita.medico_id).order_by(func.count(Cita.id).desc()).limit(10)
    by_medico = []
    for r in db.exec(q_med).all():
        u = db.get(Usuario, r[0])  # aislamiento: el id sale de una agregación ya filtrada, y solo se usa para poner el nombre
        nombre = u.nombre_completo if u else f"ID {r[0]}"
        by_medico.append({"medico": nombre, "cantidad": r[1]})

    return {
        "periodo": {"inicio": fecha_inicio.isoformat(), "fin": fecha_fin.isoformat()},
        "total": sum(by_estado.values()),
        "by_estado": by_estado,
        "by_tipo": by_tipo,
        "tendencia": trend,
        "by_medico": by_medico,
    }


# ─────────────────────────────────────────────────────────────────────────────
# PACIENTES
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/pacientes")
def get_pacientes(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    inst_id = current_user.institucion_id

    def _w(q):
        if inst_id:
            q = q.where(Paciente.institucion_id == inst_id)
        return q

    # Por sexo
    by_sexo = {r[0]: r[1] for r in db.exec(_w(select(Paciente.sexo, func.count(Paciente.id)).group_by(Paciente.sexo))).all()}

    # Por grupo edad
    fechas = db.exec(_w(select(Paciente.fecha_nacimiento))).all()
    hoy = date.today()
    grupos = {"0-10": 0, "11-17": 0, "18-29": 0, "30-44": 0, "45-59": 0, "60+": 0}
    for (fn,) in fechas:
        if not fn:
            continue
        edad = hoy.year - fn.year - ((hoy.month, hoy.day) < (fn.month, fn.day))
        if edad <= 10:
            grupos["0-10"] += 1
        elif edad <= 17:
            grupos["11-17"] += 1
        elif edad <= 29:
            grupos["18-29"] += 1
        elif edad <= 44:
            grupos["30-44"] += 1
        elif edad <= 59:
            grupos["45-59"] += 1
        else:
            grupos["60+"] += 1

    # Por tipo usuario (régimen)
    tipo_map = {"01": "Contributivo", "02": "Subsidiado", "03": "Vinculado", "04": "Particular", "05": "Otro"}
    by_tipo = {
        tipo_map.get(str(r[0]), str(r[0])): r[1]
        for r in db.exec(_w(select(Paciente.tipo_usuario, func.count(Paciente.id)).group_by(Paciente.tipo_usuario))).all()
    }

    # Por EPS (top 8)
    from models import Entidad
    q_eps = _w(
        select(Paciente.entidad_id, func.count(Paciente.id).label("c"))
        .where(Paciente.entidad_id.isnot(None))
        .group_by(Paciente.entidad_id)
        .order_by(func.count(Paciente.id).desc())
        .limit(8)
    )
    by_eps = []
    for r in db.exec(q_eps).all():
        ent = db.get(Entidad, r[0])
        by_eps.append({"eps": ent.nombre if ent else f"EPS {r[0]}", "cantidad": r[1]})

    # Por zona
    zona_map = {"01": "Urbana", "02": "Rural"}
    by_zona = {
        zona_map.get(str(r[0]), str(r[0])): r[1]
        for r in db.exec(_w(select(Paciente.zona, func.count(Paciente.id)).group_by(Paciente.zona))).all()
    }

    return {
        "total": sum(grupos.values()),
        "by_sexo": by_sexo,
        "by_grupo_edad": grupos,
        "by_tipo_usuario": by_tipo,
        "by_eps": by_eps,
        "by_zona": by_zona,
    }


# ─────────────────────────────────────────────────────────────────────────────
# TOP DIAGNÓSTICOS
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/diagnosticos")
def get_diagnosticos(
    fecha_inicio: Optional[date] = Query(default=None),
    fecha_fin: Optional[date] = Query(default=None),
    top: int = Query(default=10, le=30),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    if not fecha_inicio:
        fecha_inicio = date.today().replace(month=1, day=1)
    if not fecha_fin:
        fecha_fin = date.today()

    sub = _pac_sub(db, current_user)

    q = (
        select(Historia.diagnostico_principal, func.count(Historia.id).label("cnt"))
        .where(cast(Historia.fecha, Date) >= fecha_inicio, cast(Historia.fecha, Date) <= fecha_fin)
        .group_by(Historia.diagnostico_principal)
        .order_by(func.count(Historia.id).desc())
        .limit(top)
    )
    if sub is not None:
        q = q.where(Historia.paciente_id.in_(sub))

    from models import Cie10
    result = []
    for codigo, cnt in db.exec(q).all():
        cie = db.exec(select(Cie10).where(Cie10.codigo == codigo)).first()
        result.append({
            "codigo": codigo,
            "descripcion": cie.descripcion if cie else codigo,
            "cantidad": cnt,
        })

    return {
        "periodo": {"inicio": fecha_inicio.isoformat(), "fin": fecha_fin.isoformat()},
        "diagnosticos": result,
    }


# ─────────────────────────────────────────────────────────────────────────────
# PRODUCCIÓN MÉDICA
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/produccion")
def get_produccion(
    fecha_inicio: Optional[date] = Query(default=None),
    fecha_fin: Optional[date] = Query(default=None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    if not fecha_inicio:
        fecha_inicio = date.today().replace(day=1)
    if not fecha_fin:
        fecha_fin = date.today()

    sub = _pac_sub(db, current_user)

    q_citas = (
        select(Cita.medico_id, func.count(Cita.id))
        .where(cast(Cita.fecha_hora, Date) >= fecha_inicio, cast(Cita.fecha_hora, Date) <= fecha_fin)
        .where(Cita.estado == EstadoCita.ATENDIDA)
        .group_by(Cita.medico_id)
    )
    if sub is not None:
        q_citas = q_citas.where(Cita.paciente_id.in_(sub))
    citas_map = {r[0]: r[1] for r in db.exec(q_citas).all()}

    q_hist = (
        select(Historia.medico_id, func.count(Historia.id))
        .where(cast(Historia.fecha, Date) >= fecha_inicio, cast(Historia.fecha, Date) <= fecha_fin)
        .group_by(Historia.medico_id)
    )
    if sub is not None:
        q_hist = q_hist.where(Historia.paciente_id.in_(sub))
    hist_map = {r[0]: r[1] for r in db.exec(q_hist).all()}

    medico_ids = set(list(citas_map.keys()) + list(hist_map.keys()))
    produccion = []
    for mid in medico_ids:
        u = db.get(Usuario, mid)  # aislamiento: el id sale de una agregación ya filtrada, y solo se usa para poner el nombre
        if not u:
            continue
        produccion.append({
            "medico_id": mid,
            "nombre": u.nombre_completo,
            "especialidad": u.especialidad or "",
            "citas_atendidas": citas_map.get(mid, 0),
            "historias_creadas": hist_map.get(mid, 0),
        })
    produccion.sort(key=lambda x: x["citas_atendidas"], reverse=True)

    return {
        "periodo": {"inicio": fecha_inicio.isoformat(), "fin": fecha_fin.isoformat()},
        "medicos": produccion,
    }


# ─────────────────────────────────────────────────────────────────────────────
# FINANCIERO
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/financiero")
def get_financiero(
    fecha_inicio: Optional[date] = Query(default=None),
    fecha_fin: Optional[date] = Query(default=None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    if not fecha_inicio:
        fecha_inicio = date.today().replace(month=1, day=1)
    if not fecha_fin:
        fecha_fin = date.today()

    # EL `_de_mi_institucion` DE LAS TRES CONSULTAS NO ES ADORNO. Hasta el
    # 2026-08-06 este endpoint devolvía el valor facturado, el reparto por estado,
    # la tendencia mensual y las glosas de TODAS las instituciones — medido con
    # dos prestadores: los dos veían `total_valor = 3000` cuando cada uno había
    # facturado 1000 y 2000, mientras `/api/facturacion/stats`, con los mismos
    # datos y la misma sesión, respondía bien.
    #
    # Es el QUINTO sitio de la misma fuga, y no se pudo arreglar antes porque
    # `Factura` no tenía `institucion_id`: lo trae esta rama. Lo cazó la quinta
    # revisión de seguridad, después de que las cuatro anteriores arreglaran los
    # otros cuatro. Que sean agregados no lo salva: este proyecto ya decidió lo
    # contrario al arreglar el contador de glosas de `/stats` —«es la fuga que
    # menos se veía, porque un contador no parece un dato»—, y aquí además va el
    # dinero.
    q_estado = (
        select(Factura.estado, func.count(Factura.id), func.sum(Factura.valor_neto))
        .where(Factura.fecha_factura >= fecha_inicio, Factura.fecha_factura <= fecha_fin)
        .where(_de_mi_institucion(current_user))
        .group_by(Factura.estado)
    )
    by_estado = []
    total_valor = 0.0
    for r in db.exec(q_estado).all():
        val = float(r[2] or 0)
        total_valor += val
        by_estado.append({"estado": r[0], "cantidad": r[1], "valor": val})

    q_trend = (
        select(
            extract("year", Factura.fecha_factura).label("y"),
            extract("month", Factura.fecha_factura).label("m"),
            func.count(Factura.id),
            func.sum(Factura.valor_neto),
        )
        .where(Factura.fecha_factura >= fecha_inicio, Factura.fecha_factura <= fecha_fin)
        .where(_de_mi_institucion(current_user))
        .group_by("y", "m").order_by("y", "m")
    )
    trend = [
        {"periodo": f"{int(r[0])}-{int(r[1]):02d}", "cantidad": r[2], "valor": float(r[3] or 0)}
        for r in db.exec(q_trend).all()
    ]

    from models import Glosa
    # La glosa NO lleva institución propia a propósito —cuelga de una factura con
    # `factura_id` obligatorio, así que su dueño es demostrable en un salto—, de
    # modo que acotarla exige el JOIN. Esta consulta además no filtraba ni por
    # fecha, así que sumaba el histórico entero de todos los prestadores.
    q_glosas = (
        select(Glosa.estado, func.count(Glosa.id), func.sum(Glosa.valor_glosado))
        .join(Factura, Glosa.factura_id == Factura.id)
        .where(Factura.fecha_factura >= fecha_inicio, Factura.fecha_factura <= fecha_fin)
        .where(_de_mi_institucion(current_user))
        .group_by(Glosa.estado)
    )
    glosas = [{"estado": r[0], "cantidad": r[1], "valor": float(r[2] or 0)} for r in db.exec(q_glosas).all()]

    return {
        "periodo": {"inicio": fecha_inicio.isoformat(), "fin": fecha_fin.isoformat()},
        "total_valor": total_valor,
        "by_estado": by_estado,
        "tendencia_mensual": trend,
        "glosas": glosas,
        "valor_glosado_pendiente": sum(g["valor"] for g in glosas if g["estado"] == "PENDIENTE"),
    }


# ─────────────────────────────────────────────────────────────────────────────
# URGENCIAS
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/urgencias")
def get_urgencias(
    fecha_inicio: Optional[date] = Query(default=None),
    fecha_fin: Optional[date] = Query(default=None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    if not fecha_inicio:
        fecha_inicio = date.today().replace(day=1)
    if not fecha_fin:
        fecha_fin = date.today()

    sub = _pac_sub(db, current_user)

    def _w(q):
        q = q.where(
            cast(AtencionUrgencias.fecha_ingreso, Date) >= fecha_inicio,
            cast(AtencionUrgencias.fecha_ingreso, Date) <= fecha_fin,
        )
        if sub is not None:
            q = q.where(AtencionUrgencias.paciente_id.in_(sub))
        return q

    triage_labels = {
        TipoTriage.ROJO: "Rojo (I)", TipoTriage.NARANJA: "Naranja (II)",
        TipoTriage.AMARILLO: "Amarillo (III)", TipoTriage.VERDE: "Verde (IV)",
        TipoTriage.AZUL: "Azul (V)",
    }
    q_triage = _w(select(AtencionUrgencias.triage, func.count(AtencionUrgencias.id)).group_by(AtencionUrgencias.triage))
    by_triage = {triage_labels.get(r[0], str(r[0])): r[1] for r in db.exec(q_triage).all()}

    q_dest = _w(
        select(AtencionUrgencias.destino_salida, func.count(AtencionUrgencias.id))
        .where(AtencionUrgencias.destino_salida.isnot(None))
        .group_by(AtencionUrgencias.destino_salida)
    )
    by_destino = {str(r[0]): r[1] for r in db.exec(q_dest).all()}

    avg_espera = db.exec(_w(
        select(func.avg(AtencionUrgencias.tiempo_espera_minutos))
        .where(AtencionUrgencias.tiempo_espera_minutos.isnot(None))
    )).one()

    total = db.exec(_w(select(func.count(AtencionUrgencias.id)))).one()

    return {
        "periodo": {"inicio": fecha_inicio.isoformat(), "fin": fecha_fin.isoformat()},
        "total": total,
        "by_triage": by_triage,
        "by_destino": by_destino,
        "tiempo_espera_promedio_min": round(float(avg_espera), 1) if avg_espera else 0,
    }


# Legacy endpoint — keep for dashboard compatibility
@router.get("/kpis")
def get_kpis(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    r = get_resumen(db=db, current_user=current_user)
    return {
        "citas_hoy": r["citas_hoy"],
        "citas_mes": r["citas_mes"],
        "total_pacientes": r["total_pacientes"],
        "fecha": r["fecha"],
    }
