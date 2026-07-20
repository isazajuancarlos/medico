from typing import Optional
from datetime import date, datetime
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from models import EvolucionMedica, Paciente, Usuario, RolUsuario, ModuloSistema
from core.deps import get_db, get_current_active_user, requiere_modulo
from schemas.evolucion import EvolucionCreate, EvolucionUpdate, EvolucionRead
from core.aislamiento import obtener

router = APIRouter(prefix="/evoluciones", tags=["evoluciones"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.EVOLUCIONES))],
)


def _enrich(e: EvolucionMedica, db: Session) -> EvolucionRead:
    data = EvolucionRead.model_validate(e)
    p = db.get(Paciente, e.paciente_id)  # aislamiento: ya autorizado por el padre
    if p:
        data.paciente_nombre = p.nombre_completo.strip()
        data.paciente_identificacion = p.identificacion
    m = db.get(Usuario, e.medico_id)  # aislamiento: ya autorizado por el padre
    if m:
        data.medico_nombre = m.nombre_completo
    return data


@router.get("/", response_model=list[EvolucionRead])
def list_evoluciones(
    paciente_id: Optional[int] = Query(None),
    medico_id: Optional[int] = Query(None),
    fecha_desde: Optional[date] = Query(None),
    fecha_hasta: Optional[date] = Query(None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(EvolucionMedica)
    if paciente_id:
        query = query.where(EvolucionMedica.paciente_id == paciente_id)
    if medico_id:
        query = query.where(EvolucionMedica.medico_id == medico_id)
    if fecha_desde:
        query = query.where(EvolucionMedica.fecha >= datetime.combine(fecha_desde, datetime.min.time()))
    if fecha_hasta:
        query = query.where(EvolucionMedica.fecha <= datetime.combine(fecha_hasta, datetime.max.time()))
    # Multi-tenancy: filter via Paciente.institucion_id
    if current_user.rol != RolUsuario.SUPERADMIN and current_user.institucion_id:
        query = query.join(Paciente, EvolucionMedica.paciente_id == Paciente.id).where(
            Paciente.institucion_id == current_user.institucion_id
        )
    query = query.order_by(EvolucionMedica.fecha.desc())
    return [_enrich(e, db) for e in db.exec(query).all()]


@router.get("/paciente/{paciente_id}", response_model=list[EvolucionRead])
def list_evoluciones_paciente(
    paciente_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    rows = db.exec(
        select(EvolucionMedica)
        .where(EvolucionMedica.paciente_id == paciente_id)
        .order_by(EvolucionMedica.fecha.desc())
    ).all()
    return [_enrich(e, db) for e in rows]


@router.get("/{evolucion_id}", response_model=EvolucionRead)
def get_evolucion(
    evolucion_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    e = obtener(db, EvolucionMedica, evolucion_id, current_user, que="evolución")
    return _enrich(e, db)


@router.post("/", response_model=EvolucionRead, status_code=201)
def create_evolucion(
    payload: EvolucionCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    paciente = db.get(Paciente, payload.paciente_id)  # aislamiento: ya autorizado por el padre
    if not paciente:
        raise HTTPException(status_code=404, detail="Paciente no encontrado")
    e = EvolucionMedica(
        medico_id=current_user.id,
        **payload.model_dump(),
    )
    db.add(e)
    db.commit()
    db.refresh(e)
    return _enrich(e, db)


@router.put("/{evolucion_id}", response_model=EvolucionRead)
def update_evolucion(
    evolucion_id: int,
    payload: EvolucionUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    e = obtener(db, EvolucionMedica, evolucion_id, current_user, que="evolución")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(e, field, value)
    db.add(e)
    db.commit()
    db.refresh(e)
    return _enrich(e, db)


@router.delete("/{evolucion_id}", status_code=204)
def delete_evolucion(
    evolucion_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    e = obtener(db, EvolucionMedica, evolucion_id, current_user, que="evolución")
    db.delete(e)
    db.commit()
