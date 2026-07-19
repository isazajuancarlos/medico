from datetime import date, datetime, time, timedelta
from typing import Optional, List, Any
from fastapi import APIRouter, Depends, HTTPException
from sqlmodel import Session, select
from pydantic import BaseModel
from models import (
    AgendaMedico, BloqueHorario, BloqueoAgenda, Cita, Usuario,
    TipoBloqueo, EstadoCita, RolUsuario,
)
from core.deps import get_db, get_current_active_user

router = APIRouter(prefix="/agenda", tags=["agenda"])

DIAS_SEMANA = ["Lunes", "Martes", "Miércoles", "Jueves", "Viernes", "Sábado", "Domingo"]
DURACION_SLOT_MIN = 30  # minutes per slot


# ── Schemas ───────────────────────────────────────────────────────────────────

class BloqueCreate(BaseModel):
    dia_semana: int  # 0=Lunes … 6=Domingo
    hora_inicio: str  # "08:00"
    hora_fin: str     # "12:00"
    activo: bool = True


class AgendaCreate(BaseModel):
    medico_id: int
    año: int
    mes: int  # 1-12
    observaciones: Optional[str] = None
    bloques: List[BloqueCreate] = []


class AgendaUpdate(BaseModel):
    activa: Optional[bool] = None
    observaciones: Optional[str] = None


class BloqueoCreate(BaseModel):
    medico_id: int
    fecha_inicio: datetime
    fecha_fin: datetime
    motivo: str
    tipo: TipoBloqueo = TipoBloqueo.OTRO


class SlotDisponible(BaseModel):
    hora: str
    disponible: bool = True
    motivo_no_disponible: Optional[str] = None


# ── Core slot calculation ─────────────────────────────────────────────────────

def get_slots_para_fecha(medico_id: int, fecha: date, db: Session) -> List[SlotDisponible]:
    """
    Returns available appointment slots for a doctor on a given date.
    Respects: AgendaMedico blocks, BloqueoAgenda periods, and existing Citas.
    Returns empty list if no agenda is configured (no forced defaults).
    """
    agenda = db.exec(
        select(AgendaMedico)
        .where(AgendaMedico.medico_id == medico_id)
        .where(AgendaMedico.año == fecha.year)
        .where(AgendaMedico.mes == fecha.month)
        .where(AgendaMedico.activa == True)
    ).first()

    if not agenda:
        return []

    dia = fecha.weekday()  # 0=Monday, same as model convention
    bloques = db.exec(
        select(BloqueHorario)
        .where(BloqueHorario.agenda_id == agenda.id)
        .where(BloqueHorario.dia_semana == dia)
        .where(BloqueHorario.activo == True)
    ).all()

    if not bloques:
        return []

    fecha_ini_dt = datetime.combine(fecha, time(0, 0))
    fecha_fin_dt = datetime.combine(fecha, time(23, 59))

    # Active bloqueos that overlap this day
    bloqueos = db.exec(
        select(BloqueoAgenda)
        .where(BloqueoAgenda.medico_id == medico_id)
        .where(BloqueoAgenda.fecha_inicio <= fecha_fin_dt)
        .where(BloqueoAgenda.fecha_fin >= fecha_ini_dt)
    ).all()

    # Existing non-cancelled citas
    citas_del_dia = db.exec(
        select(Cita)
        .where(Cita.medico_id == medico_id)
        .where(Cita.fecha_hora >= fecha_ini_dt)
        .where(Cita.fecha_hora <= fecha_fin_dt)
        .where(Cita.estado != EstadoCita.CANCELADA)
    ).all()
    citas_horas = {c.fecha_hora.strftime('%H:%M') for c in citas_del_dia}

    slots: List[SlotDisponible] = []
    for bloque in bloques:
        h_ini_h, h_ini_m = map(int, bloque.hora_inicio.split(':'))
        h_fin_h, h_fin_m = map(int, bloque.hora_fin.split(':'))
        current = datetime.combine(fecha, time(h_ini_h, h_ini_m))
        fin = datetime.combine(fecha, time(h_fin_h, h_fin_m))

        while current < fin:
            hora_str = current.strftime('%H:%M')

            bloqueo_activo = None
            for bloqueo in bloqueos:
                if bloqueo.fecha_inicio <= current <= bloqueo.fecha_fin:
                    bloqueo_activo = bloqueo
                    break

            if bloqueo_activo:
                disponible = False
                motivo = f"Bloqueado: {bloqueo_activo.tipo.value}"
            elif hora_str in citas_horas:
                disponible = False
                motivo = "Cita agendada"
            else:
                disponible = True
                motivo = None

            slots.append(SlotDisponible(hora=hora_str, disponible=disponible, motivo_no_disponible=motivo))
            current += timedelta(minutes=DURACION_SLOT_MIN)

    return slots


# ── Endpoints ─────────────────────────────────────────────────────────────────

@router.get("/medicos")
def get_medicos(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    """Lista de médicos disponibles para agendar citas."""
    roles_clinicos = [
        RolUsuario.MEDICO, RolUsuario.ANESTESIOLOGO, RolUsuario.PSICOLOGO,
        RolUsuario.NUTRICIONISTA, RolUsuario.TERAPEUTA,
    ]
    query = select(Usuario).where(Usuario.rol.in_(roles_clinicos)).where(Usuario.activo == True)
    if current_user.institucion_id:
        query = query.where(Usuario.institucion_id == current_user.institucion_id)
    medicos = db.exec(query).all()
    return [
        {
            "id": m.id,
            "nombre": m.nombre_completo,
            "rol": m.rol.value,
            "especialidad": getattr(m, 'especialidad_nombre', None),
        }
        for m in medicos
    ]


@router.get("/slots", response_model=List[SlotDisponible])
def get_slots(
    medico_id: int,
    fecha: date,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    return get_slots_para_fecha(medico_id, fecha, db)


# ── Bloqueos (must be declared before /{agenda_id} to avoid path conflicts) ───

@router.get("/bloqueos/lista")
def list_bloqueos(
    medico_id: Optional[int] = None,
    desde: Optional[date] = None,
    hasta: Optional[date] = None,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(BloqueoAgenda)
    if current_user.institucion_id:
        query = query.where(BloqueoAgenda.institucion_id == current_user.institucion_id)
    if medico_id:
        query = query.where(BloqueoAgenda.medico_id == medico_id)
    if desde:
        query = query.where(BloqueoAgenda.fecha_fin >= datetime.combine(desde, time(0, 0)))
    if hasta:
        query = query.where(BloqueoAgenda.fecha_inicio <= datetime.combine(hasta, time(23, 59)))

    bloqueos = db.exec(query.order_by(BloqueoAgenda.fecha_inicio)).all()
    result = []
    for b in bloqueos:
        medico = db.get(Usuario, b.medico_id)
        result.append({
            "id": b.id,
            "medico_id": b.medico_id,
            "medico_nombre": medico.nombre_completo if medico else None,
            "fecha_inicio": b.fecha_inicio.isoformat(),
            "fecha_fin": b.fecha_fin.isoformat(),
            "motivo": b.motivo,
            "tipo": b.tipo.value,
        })
    return result


@router.post("/bloqueos/crear", status_code=201)
def create_bloqueo(
    payload: BloqueoCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    # Warn about conflicting confirmed/pending citas
    citas_conflicto = db.exec(
        select(Cita)
        .where(Cita.medico_id == payload.medico_id)
        .where(Cita.fecha_hora >= payload.fecha_inicio)
        .where(Cita.fecha_hora <= payload.fecha_fin)
        .where(Cita.estado.in_([EstadoCita.PENDIENTE, EstadoCita.CONFIRMADA]))
    ).all()

    bloqueo = BloqueoAgenda(
        medico_id=payload.medico_id,
        fecha_inicio=payload.fecha_inicio,
        fecha_fin=payload.fecha_fin,
        motivo=payload.motivo,
        tipo=payload.tipo,
        creado_por_id=current_user.id,
        institucion_id=current_user.institucion_id,
    )
    db.add(bloqueo)
    db.commit()
    db.refresh(bloqueo)
    return {
        "id": bloqueo.id,
        "citas_conflicto": len(citas_conflicto),
        "advertencia": (
            f"Hay {len(citas_conflicto)} cita(s) en este período que requieren reasignación"
            if citas_conflicto else None
        ),
    }


@router.delete("/bloqueos/{bloqueo_id}")
def delete_bloqueo(
    bloqueo_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    bloqueo = db.get(BloqueoAgenda, bloqueo_id)
    if not bloqueo:
        raise HTTPException(status_code=404, detail="Bloqueo no encontrado")
    db.delete(bloqueo)
    db.commit()
    return {"mensaje": "Bloqueo eliminado"}


# ── Indicadores de calidad (Resolución 256/2016) ──────────────────────────────

@router.get("/indicadores/oportunidad")
def get_indicadores_oportunidad(
    medico_id: Optional[int] = None,
    desde: Optional[date] = None,
    hasta: Optional[date] = None,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    """
    Indicadores de oportunidad de acceso a los servicios de salud.
    Normativa: Resolución 256/2016 - Indicadores de Calidad en Salud.
    Metas: medicina general ≤3 días, especialista ≤5 días.
    """
    if not desde:
        desde = date.today().replace(day=1)
    if not hasta:
        hasta = date.today()

    def q(estado: EstadoCita):
        query = select(Cita).where(
            Cita.fecha_hora >= datetime.combine(desde, time(0, 0)),
            Cita.fecha_hora <= datetime.combine(hasta, time(23, 59)),
            Cita.estado == estado,
        )
        if medico_id:
            query = query.where(Cita.medico_id == medico_id)
        if current_user.institucion_id:
            query = query.where(Cita.institucion_id == current_user.institucion_id)
        return query

    atendidas = db.exec(q(EstadoCita.ATENDIDA)).all()
    canceladas = db.exec(q(EstadoCita.CANCELADA)).all()
    no_asistio = db.exec(q(EstadoCita.NO_ASISTIO)).all()
    total = len(atendidas) + len(canceladas) + len(no_asistio)

    return {
        "periodo": {"desde": desde.isoformat(), "hasta": hasta.isoformat()},
        "total_atendidas": len(atendidas),
        "total_canceladas": len(canceladas),
        "total_no_asistio": len(no_asistio),
        "total_programadas": total,
        "tasa_inasistencia_pct": round(len(no_asistio) / total * 100, 2) if total else 0,
        "tasa_cancelacion_pct": round(len(canceladas) / total * 100, 2) if total else 0,
        "meta_oportunidad_med_general_dias": 3,
        "meta_oportunidad_especialista_dias": 5,
        "normativa": "Resolución 256/2016 - Indicadores de Calidad en Salud",
    }


# ── Agendas CRUD ──────────────────────────────────────────────────────────────

@router.get("/")
def list_agendas(
    medico_id: Optional[int] = None,
    año: Optional[int] = None,
    mes: Optional[int] = None,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(AgendaMedico)
    if current_user.institucion_id:
        query = query.where(AgendaMedico.institucion_id == current_user.institucion_id)
    if medico_id:
        query = query.where(AgendaMedico.medico_id == medico_id)
    if año:
        query = query.where(AgendaMedico.año == año)
    if mes:
        query = query.where(AgendaMedico.mes == mes)

    agendas = db.exec(query).all()
    result = []
    for a in agendas:
        medico = db.get(Usuario, a.medico_id)
        bloques = db.exec(select(BloqueHorario).where(BloqueHorario.agenda_id == a.id)).all()
        result.append({
            "id": a.id,
            "medico_id": a.medico_id,
            "medico_nombre": medico.nombre_completo if medico else None,
            "año": a.año,
            "mes": a.mes,
            "activa": a.activa,
            "observaciones": a.observaciones,
            "bloques": [
                {
                    "id": b.id,
                    "dia_semana": b.dia_semana,
                    "dia_nombre": DIAS_SEMANA[b.dia_semana],
                    "hora_inicio": b.hora_inicio,
                    "hora_fin": b.hora_fin,
                    "activo": b.activo,
                }
                for b in bloques
            ],
        })
    return result


@router.post("/", status_code=201)
def create_agenda(
    payload: AgendaCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    existing = db.exec(
        select(AgendaMedico)
        .where(AgendaMedico.medico_id == payload.medico_id)
        .where(AgendaMedico.año == payload.año)
        .where(AgendaMedico.mes == payload.mes)
    ).first()
    if existing:
        raise HTTPException(status_code=400, detail="Ya existe una agenda para ese médico en ese mes")

    agenda = AgendaMedico(
        medico_id=payload.medico_id,
        año=payload.año,
        mes=payload.mes,
        observaciones=payload.observaciones,
        creada_por_id=current_user.id,
        institucion_id=current_user.institucion_id,
    )
    db.add(agenda)
    db.flush()

    for b in payload.bloques:
        db.add(BloqueHorario(
            agenda_id=agenda.id,
            dia_semana=b.dia_semana,
            hora_inicio=b.hora_inicio,
            hora_fin=b.hora_fin,
            activo=b.activo,
        ))

    db.commit()
    db.refresh(agenda)
    return {"id": agenda.id, "mensaje": "Agenda creada exitosamente"}


@router.get("/{agenda_id}")
def get_agenda(
    agenda_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    agenda = db.get(AgendaMedico, agenda_id)
    if not agenda:
        raise HTTPException(status_code=404, detail="Agenda no encontrada")
    medico = db.get(Usuario, agenda.medico_id)
    bloques = db.exec(select(BloqueHorario).where(BloqueHorario.agenda_id == agenda_id)).all()
    return {
        "id": agenda.id,
        "medico_id": agenda.medico_id,
        "medico_nombre": medico.nombre_completo if medico else None,
        "año": agenda.año,
        "mes": agenda.mes,
        "activa": agenda.activa,
        "observaciones": agenda.observaciones,
        "bloques": [
            {
                "id": b.id,
                "dia_semana": b.dia_semana,
                "dia_nombre": DIAS_SEMANA[b.dia_semana],
                "hora_inicio": b.hora_inicio,
                "hora_fin": b.hora_fin,
                "activo": b.activo,
            }
            for b in bloques
        ],
    }


@router.put("/{agenda_id}")
def update_agenda(
    agenda_id: int,
    payload: AgendaUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    agenda = db.get(AgendaMedico, agenda_id)
    if not agenda:
        raise HTTPException(status_code=404, detail="Agenda no encontrada")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(agenda, field, value)
    db.add(agenda)
    db.commit()
    return {"mensaje": "Agenda actualizada"}


@router.delete("/{agenda_id}")
def delete_agenda(
    agenda_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    agenda = db.get(AgendaMedico, agenda_id)
    if not agenda:
        raise HTTPException(status_code=404, detail="Agenda no encontrada")
    db.delete(agenda)
    db.commit()
    return {"mensaje": "Agenda eliminada"}


@router.post("/{agenda_id}/bloques", status_code=201)
def add_bloque(
    agenda_id: int,
    payload: BloqueCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    agenda = db.get(AgendaMedico, agenda_id)
    if not agenda:
        raise HTTPException(status_code=404, detail="Agenda no encontrada")
    bloque = BloqueHorario(
        agenda_id=agenda_id,
        dia_semana=payload.dia_semana,
        hora_inicio=payload.hora_inicio,
        hora_fin=payload.hora_fin,
        activo=payload.activo,
    )
    db.add(bloque)
    db.commit()
    db.refresh(bloque)
    return {"id": bloque.id, "mensaje": "Bloque agregado"}


@router.put("/{agenda_id}/bloques/{bloque_id}")
def update_bloque(
    agenda_id: int,
    bloque_id: int,
    payload: BloqueCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    bloque = db.get(BloqueHorario, bloque_id)
    if not bloque or bloque.agenda_id != agenda_id:
        raise HTTPException(status_code=404, detail="Bloque no encontrado")
    for field, value in payload.model_dump().items():
        setattr(bloque, field, value)
    db.add(bloque)
    db.commit()
    return {"mensaje": "Bloque actualizado"}


@router.delete("/{agenda_id}/bloques/{bloque_id}")
def delete_bloque(
    agenda_id: int,
    bloque_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    bloque = db.get(BloqueHorario, bloque_id)
    if not bloque or bloque.agenda_id != agenda_id:
        raise HTTPException(status_code=404, detail="Bloque no encontrado")
    db.delete(bloque)
    db.commit()
    return {"mensaje": "Bloque eliminado"}
