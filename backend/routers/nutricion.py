from typing import Optional
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from models import ValoracionNutricional, PlanAlimentacion, Paciente, Usuario, RolUsuario
from core.deps import get_db, get_current_active_user
from schemas.nutricion import (
    ValoracionCreate, ValoracionUpdate, ValoracionRead,
    PlanCreate, PlanUpdate, PlanRead,
)

router = APIRouter(prefix="/nutricion", tags=["nutricion"])


def _enrich_val(v: ValoracionNutricional, db: Session) -> ValoracionRead:
    data = ValoracionRead.model_validate(v)
    p = db.get(Paciente, v.paciente_id)
    if p:
        data.paciente_nombre = p.nombre_completo.strip()
        data.paciente_identificacion = p.identificacion
    n = db.get(Usuario, v.nutricionista_id)
    if n:
        data.nutricionista_nombre = n.nombre_completo
    return data


def _enrich_plan(p: PlanAlimentacion, db: Session) -> PlanRead:
    data = PlanRead.model_validate(p)
    n = db.get(Usuario, p.nutricionista_id)
    if n:
        data.nutricionista_nombre = n.nombre_completo
    return data


# ─── Valoraciones ────────────────────────────────────────────────

@router.get("/valoraciones", response_model=list[ValoracionRead])
def list_valoraciones(
    paciente_id: Optional[int] = Query(None),
    riesgo: Optional[str] = Query(None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(ValoracionNutricional)
    if paciente_id:
        query = query.where(ValoracionNutricional.paciente_id == paciente_id)
    if riesgo:
        query = query.where(ValoracionNutricional.riesgo_nutricional == riesgo)
    if current_user.rol != RolUsuario.SUPERADMIN and current_user.institucion_id:
        query = query.where(ValoracionNutricional.institucion_id == current_user.institucion_id)
    query = query.order_by(ValoracionNutricional.fecha_valoracion.desc())
    return [_enrich_val(v, db) for v in db.exec(query).all()]


@router.get("/valoraciones/{val_id}", response_model=ValoracionRead)
def get_valoracion(
    val_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    v = db.get(ValoracionNutricional, val_id)
    if not v:
        raise HTTPException(status_code=404, detail="Valoración no encontrada")
    return _enrich_val(v, db)


@router.post("/valoraciones", response_model=ValoracionRead, status_code=201)
def create_valoracion(
    payload: ValoracionCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    if not db.get(Paciente, payload.paciente_id):
        raise HTTPException(status_code=404, detail="Paciente no encontrado")
    v = ValoracionNutricional(
        nutricionista_id=current_user.id,
        institucion_id=current_user.institucion_id,
        **payload.model_dump(),
    )
    db.add(v)
    db.commit()
    db.refresh(v)
    return _enrich_val(v, db)


@router.put("/valoraciones/{val_id}", response_model=ValoracionRead)
def update_valoracion(
    val_id: int,
    payload: ValoracionUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    v = db.get(ValoracionNutricional, val_id)
    if not v:
        raise HTTPException(status_code=404, detail="Valoración no encontrada")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(v, field, value)
    db.add(v)
    db.commit()
    db.refresh(v)
    return _enrich_val(v, db)


@router.delete("/valoraciones/{val_id}", status_code=204)
def delete_valoracion(
    val_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    v = db.get(ValoracionNutricional, val_id)
    if not v:
        raise HTTPException(status_code=404, detail="Valoración no encontrada")
    db.delete(v)
    db.commit()


# ─── Planes de alimentación ──────────────────────────────────────

@router.get("/valoraciones/{val_id}/plan", response_model=PlanRead)
def get_plan(
    val_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    plan = db.exec(
        select(PlanAlimentacion).where(PlanAlimentacion.valoracion_id == val_id)
    ).first()
    if not plan:
        raise HTTPException(status_code=404, detail="Plan no encontrado")
    return _enrich_plan(plan, db)


@router.post("/valoraciones/{val_id}/plan", response_model=PlanRead, status_code=201)
def create_plan(
    val_id: int,
    payload: PlanCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    v = db.get(ValoracionNutricional, val_id)
    if not v:
        raise HTTPException(status_code=404, detail="Valoración no encontrada")
    existing = db.exec(
        select(PlanAlimentacion).where(PlanAlimentacion.valoracion_id == val_id)
    ).first()
    if existing:
        raise HTTPException(status_code=400, detail="Ya existe un plan. Use PUT para actualizarlo.")
    plan = PlanAlimentacion(
        valoracion_id=val_id,
        paciente_id=v.paciente_id,
        nutricionista_id=current_user.id,
        institucion_id=current_user.institucion_id,
        **payload.model_dump(),
    )
    db.add(plan)
    db.commit()
    db.refresh(plan)
    return _enrich_plan(plan, db)


@router.put("/planes/{plan_id}", response_model=PlanRead)
def update_plan(
    plan_id: int,
    payload: PlanUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    plan = db.get(PlanAlimentacion, plan_id)
    if not plan:
        raise HTTPException(status_code=404, detail="Plan no encontrado")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(plan, field, value)
    db.add(plan)
    db.commit()
    db.refresh(plan)
    return _enrich_plan(plan, db)
