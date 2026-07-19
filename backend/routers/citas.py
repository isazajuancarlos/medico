from typing import Optional
from datetime import date, datetime, time as time_t
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from sqlalchemy import cast
from sqlalchemy.types import Date as SADate
from models import Cita, Paciente, Usuario, EstadoCita, Historia
from schemas.cita import CitaCreate, CitaUpdate, CitaRead
from schemas.common import PaginatedResponse
from core.deps import get_db, get_current_active_user

router = APIRouter(prefix="/citas", tags=["citas"])


def _cita_to_read(cita: Cita, db: Session) -> CitaRead:
    paciente = db.get(Paciente, cita.paciente_id)
    medico = db.get(Usuario, cita.medico_id)
    data = CitaRead.model_validate(cita)
    data.paciente_nombre = f"{paciente.nombre1} {paciente.apellido1}" if paciente else None
    data.medico_nombre = medico.nombre_completo if medico else None
    return data


@router.get("/", response_model=PaginatedResponse[CitaRead])
def list_citas(
    fecha: Optional[date] = None,
    medico_id: Optional[int] = None,
    estado: Optional[EstadoCita] = None,
    page: int = Query(1, ge=1),
    size: int = Query(20, ge=1, le=100),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(Cita)
    if current_user.institucion_id:
        query = query.where(Cita.institucion_id == current_user.institucion_id)
    if fecha:
        query = query.where(cast(Cita.fecha_hora, SADate) == fecha)
    if medico_id:
        query = query.where(Cita.medico_id == medico_id)
    if estado:
        query = query.where(Cita.estado == estado)
    total = len(db.exec(query).all())
    items = db.exec(query.order_by(Cita.fecha_hora).offset((page - 1) * size).limit(size)).all()
    return PaginatedResponse(total=total, items=[_cita_to_read(c, db) for c in items], page=page, size=size)


@router.post("/", response_model=CitaRead, status_code=201)
def create_cita(
    payload: CitaCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    from routers.agenda_medica import get_slots_para_fecha

    fecha_hora = payload.get_fecha_hora()

    # Validate slot availability against doctor's schedule
    slots = get_slots_para_fecha(payload.medico_id, payload.fecha, db)
    if slots:
        slot = next((s for s in slots if s.hora == payload.hora), None)
        if slot is not None and not slot.disponible:
            raise HTTPException(
                status_code=400,
                detail=f"El horario {payload.hora} no está disponible: {slot.motivo_no_disponible}",
            )

    cita = Cita(
        paciente_id=payload.paciente_id,
        medico_id=payload.medico_id,
        fecha_hora=fecha_hora,
        tipo=payload.tipo,
        motivo=payload.motivo or "Consulta médica",
        duracion_minutos=payload.duracion_minutos,
        institucion_id=current_user.institucion_id,
    )
    db.add(cita)
    db.commit()
    db.refresh(cita)
    return _cita_to_read(cita, db)


@router.get("/{cita_id}", response_model=CitaRead)
def get_cita(
    cita_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    cita = db.get(Cita, cita_id)
    if not cita:
        raise HTTPException(status_code=404, detail="Cita no encontrada")
    return _cita_to_read(cita, db)


@router.put("/{cita_id}", response_model=CitaRead)
def update_cita(
    cita_id: int,
    payload: CitaUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    cita = db.get(Cita, cita_id)
    if not cita:
        raise HTTPException(status_code=404, detail="Cita no encontrada")

    update_data = payload.model_dump(exclude_none=True)
    # Handle fecha/hora → fecha_hora conversion
    if 'fecha' in update_data or 'hora' in update_data:
        current_fh = cita.fecha_hora
        new_fecha = update_data.pop('fecha', current_fh.date())
        new_hora = update_data.pop('hora', current_fh.strftime('%H:%M'))
        h, m = map(int, new_hora.split(':'))
        cita.fecha_hora = datetime.combine(new_fecha, time_t(h, m))
    for field, value in update_data.items():
        setattr(cita, field, value)

    db.add(cita)
    db.commit()
    db.refresh(cita)
    return _cita_to_read(cita, db)


@router.delete("/{cita_id}")
def cancel_cita(
    cita_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    cita = db.get(Cita, cita_id)
    if not cita:
        raise HTTPException(status_code=404, detail="Cita no encontrada")
    cita.estado = EstadoCita.CANCELADA
    db.add(cita)
    db.commit()
    return {"message": "Cita cancelada"}


@router.post("/{cita_id}/atender")
def atender_cita(
    cita_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    cita = db.get(Cita, cita_id)
    if not cita:
        raise HTTPException(status_code=404, detail="Cita no encontrada")
    import uuid as _uuid
    historia = Historia(
        paciente_id=cita.paciente_id,
        medico_id=current_user.id,
        cita_id=cita_id,
        codigo_historia=f"HC-{_uuid.uuid4().hex[:8].upper()}",
        motivo_consulta="En proceso de atención",
        enfermedad_actual="En proceso de atención",
        diagnostico_principal="Z00.0",
        plan_tratamiento="Por definir",
    )
    db.add(historia)
    cita.estado = EstadoCita.ATENDIDA
    db.add(cita)
    db.commit()
    db.refresh(historia)
    return {"historia_id": historia.id, "paciente_id": cita.paciente_id}
