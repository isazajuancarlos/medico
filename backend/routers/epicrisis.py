from typing import Optional
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from models import Epicrisis, Paciente, Usuario, RolUsuario
from core.deps import get_db, get_current_active_user
from schemas.epicrisis import EpicrisisCreate, EpicrisisUpdate, EpicrisisRead

router = APIRouter(prefix="/epicrisis", tags=["epicrisis"])


def _enrich(e: Epicrisis, db: Session) -> EpicrisisRead:
    data = EpicrisisRead.model_validate(e)
    p = db.get(Paciente, e.paciente_id)
    if p:
        data.paciente_nombre = p.nombre_completo.strip()
        data.paciente_identificacion = p.identificacion
    m = db.get(Usuario, e.medico_tratante_id)
    if m:
        data.medico_nombre = m.nombre_completo
    return data


@router.get("/", response_model=list[EpicrisisRead])
def list_epicrisis(
    paciente_id: Optional[int] = Query(None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(Epicrisis)
    if paciente_id:
        query = query.where(Epicrisis.paciente_id == paciente_id)
    if current_user.rol != RolUsuario.SUPERADMIN and current_user.institucion_id:
        query = query.join(Paciente, Epicrisis.paciente_id == Paciente.id).where(
            Paciente.institucion_id == current_user.institucion_id
        )
    query = query.order_by(Epicrisis.fecha_egreso.desc())
    return [_enrich(e, db) for e in db.exec(query).all()]


@router.get("/paciente/{paciente_id}", response_model=list[EpicrisisRead])
def list_epicrisis_paciente(
    paciente_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    rows = db.exec(
        select(Epicrisis)
        .where(Epicrisis.paciente_id == paciente_id)
        .order_by(Epicrisis.fecha_egreso.desc())
    ).all()
    return [_enrich(e, db) for e in rows]


@router.get("/{epicrisis_id}", response_model=EpicrisisRead)
def get_epicrisis(
    epicrisis_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    e = db.get(Epicrisis, epicrisis_id)
    if not e:
        raise HTTPException(status_code=404, detail="Epicrisis no encontrada")
    return _enrich(e, db)


@router.post("/", response_model=EpicrisisRead, status_code=201)
def create_epicrisis(
    payload: EpicrisisCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    if not db.get(Paciente, payload.paciente_id):
        raise HTTPException(status_code=404, detail="Paciente no encontrado")
    e = Epicrisis(medico_tratante_id=current_user.id, **payload.model_dump())
    db.add(e)
    db.commit()
    db.refresh(e)
    return _enrich(e, db)


@router.put("/{epicrisis_id}", response_model=EpicrisisRead)
def update_epicrisis(
    epicrisis_id: int,
    payload: EpicrisisUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    e = db.get(Epicrisis, epicrisis_id)
    if not e:
        raise HTTPException(status_code=404, detail="Epicrisis no encontrada")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(e, field, value)
    db.add(e)
    db.commit()
    db.refresh(e)
    return _enrich(e, db)


@router.delete("/{epicrisis_id}", status_code=204)
def delete_epicrisis(
    epicrisis_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    e = db.get(Epicrisis, epicrisis_id)
    if not e:
        raise HTTPException(status_code=404, detail="Epicrisis no encontrada")
    db.delete(e)
    db.commit()
