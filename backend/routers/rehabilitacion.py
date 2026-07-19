from typing import Optional
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from models import PlanRehabilitacion, SesionTerapia, Paciente, Usuario, RolUsuario, ModuloSistema
from core.deps import get_db, get_current_active_user, requiere_modulo
from schemas.rehabilitacion import (
    PlanCreate, PlanUpdate, PlanRead,
    SesionCreate, SesionUpdate, SesionRead,
)

router = APIRouter(prefix="/rehabilitacion", tags=["rehabilitacion"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.REHABILITACION))],
)


def _enrich_plan(p: PlanRehabilitacion, db: Session) -> PlanRead:
    data = PlanRead.model_validate(p)
    pac = db.get(Paciente, p.paciente_id)
    if pac:
        data.paciente_nombre = pac.nombre_completo.strip()
        data.paciente_identificacion = pac.identificacion
    ter = db.get(Usuario, p.terapeuta_id)
    if ter:
        data.terapeuta_nombre = ter.nombre_completo
    return data


def _enrich_sesion(s: SesionTerapia, db: Session) -> SesionRead:
    data = SesionRead.model_validate(s)
    ter = db.get(Usuario, s.terapeuta_id)
    if ter:
        data.terapeuta_nombre = ter.nombre_completo
    return data


# ─── Planes ──────────────────────────────────────────────────────

@router.get("/planes", response_model=list[PlanRead])
def list_planes(
    paciente_id: Optional[int] = Query(None),
    tipo_terapia: Optional[str] = Query(None),
    activo: Optional[bool] = Query(None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(PlanRehabilitacion)
    if paciente_id:
        query = query.where(PlanRehabilitacion.paciente_id == paciente_id)
    if tipo_terapia:
        query = query.where(PlanRehabilitacion.tipo_terapia == tipo_terapia)
    if activo is not None:
        query = query.where(PlanRehabilitacion.activo == activo)
    if current_user.rol != RolUsuario.SUPERADMIN and current_user.institucion_id:
        query = query.where(PlanRehabilitacion.institucion_id == current_user.institucion_id)
    query = query.order_by(PlanRehabilitacion.fecha_creacion.desc())
    return [_enrich_plan(p, db) for p in db.exec(query).all()]


@router.get("/planes/{plan_id}", response_model=PlanRead)
def get_plan(
    plan_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    p = db.get(PlanRehabilitacion, plan_id)
    if not p:
        raise HTTPException(status_code=404, detail="Plan no encontrado")
    return _enrich_plan(p, db)


@router.post("/planes", response_model=PlanRead, status_code=201)
def create_plan(
    payload: PlanCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    if not db.get(Paciente, payload.paciente_id):
        raise HTTPException(status_code=404, detail="Paciente no encontrado")
    p = PlanRehabilitacion(
        terapeuta_id=current_user.id,
        institucion_id=current_user.institucion_id,
        **payload.model_dump(),
    )
    db.add(p)
    db.commit()
    db.refresh(p)
    return _enrich_plan(p, db)


@router.put("/planes/{plan_id}", response_model=PlanRead)
def update_plan(
    plan_id: int,
    payload: PlanUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    p = db.get(PlanRehabilitacion, plan_id)
    if not p:
        raise HTTPException(status_code=404, detail="Plan no encontrado")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(p, field, value)
    db.add(p)
    db.commit()
    db.refresh(p)
    return _enrich_plan(p, db)


# ─── Sesiones ────────────────────────────────────────────────────

@router.get("/planes/{plan_id}/sesiones", response_model=list[SesionRead])
def list_sesiones(
    plan_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    if not db.get(PlanRehabilitacion, plan_id):
        raise HTTPException(status_code=404, detail="Plan no encontrado")
    sesiones = db.exec(
        select(SesionTerapia)
        .where(SesionTerapia.plan_id == plan_id)
        .order_by(SesionTerapia.numero_sesion)
    ).all()
    return [_enrich_sesion(s, db) for s in sesiones]


@router.post("/planes/{plan_id}/sesiones", response_model=SesionRead, status_code=201)
def create_sesion(
    plan_id: int,
    payload: SesionCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    plan = db.get(PlanRehabilitacion, plan_id)
    if not plan:
        raise HTTPException(status_code=404, detail="Plan no encontrado")
    numero = plan.sesiones_realizadas + 1
    s = SesionTerapia(
        plan_id=plan_id,
        paciente_id=plan.paciente_id,
        terapeuta_id=current_user.id,
        numero_sesion=numero,
        institucion_id=current_user.institucion_id,
        **payload.model_dump(),
    )
    db.add(s)
    plan.sesiones_realizadas = numero
    if numero >= plan.sesiones_autorizadas:
        plan.activo = False
    db.add(plan)
    db.commit()
    db.refresh(s)
    return _enrich_sesion(s, db)


@router.put("/sesiones/{sesion_id}", response_model=SesionRead)
def update_sesion(
    sesion_id: int,
    payload: SesionUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    s = db.get(SesionTerapia, sesion_id)
    if not s:
        raise HTTPException(status_code=404, detail="Sesión no encontrada")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(s, field, value)
    db.add(s)
    db.commit()
    db.refresh(s)
    return _enrich_sesion(s, db)
