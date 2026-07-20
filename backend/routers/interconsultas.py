from typing import Optional
from datetime import datetime
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from models import Interconsulta, Paciente, Usuario, RolUsuario, PrioridadInterconsulta, EstadoInterconsulta, ModuloSistema
from core.deps import get_db, get_current_active_user, requiere_modulo
from schemas.interconsulta import InterconsultaCreate, InterconsultaResponder, InterconsultaRead
from core.aislamiento import obtener

router = APIRouter(prefix="/interconsultas", tags=["interconsultas"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.INTERCONSULTAS))],
)


def _enrich(ic: Interconsulta, db: Session) -> InterconsultaRead:
    data = InterconsultaRead.model_validate(ic)
    p = db.get(Paciente, ic.paciente_id)  # aislamiento: ya autorizado por el padre
    if p:
        data.paciente_nombre = p.nombre_completo.strip()
        data.paciente_identificacion = p.identificacion
    ms = db.get(Usuario, ic.medico_solicita_id)  # aislamiento: ya autorizado por el padre
    if ms:
        data.medico_solicita_nombre = ms.nombre_completo
    if ic.medico_responde_id:
        mr = db.get(Usuario, ic.medico_responde_id)  # aislamiento: ya autorizado por el padre
        if mr:
            data.medico_responde_nombre = mr.nombre_completo
    # Especialidad
    try:
        from models import Especialidad
        esp = db.get(Especialidad, ic.especialidad_solicitada_id)  # aislamiento: ya autorizado por el padre
        if esp:
            data.especialidad_nombre = esp.nombre
    except Exception:
        pass
    return data


@router.get("/", response_model=list[InterconsultaRead])
def list_interconsultas(
    estado: Optional[str] = Query(None),
    prioridad: Optional[str] = Query(None),
    solo_mias: bool = Query(False),
    solo_para_mi: bool = Query(False),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(Interconsulta)
    if estado:
        query = query.where(Interconsulta.estado == estado)
    if prioridad:
        query = query.where(Interconsulta.prioridad == prioridad)
    if solo_mias:
        query = query.where(Interconsulta.medico_solicita_id == current_user.id)
    if solo_para_mi:
        query = query.where(Interconsulta.medico_responde_id == current_user.id)
    # Multi-tenancy
    if current_user.rol != RolUsuario.SUPERADMIN and current_user.institucion_id:
        query = query.where(Interconsulta.institucion_id == current_user.institucion_id)
    query = query.order_by(Interconsulta.fecha_solicitud.desc())
    return [_enrich(ic, db) for ic in db.exec(query).all()]


@router.get("/{ic_id}", response_model=InterconsultaRead)
def get_interconsulta(
    ic_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    ic = obtener(db, Interconsulta, ic_id, current_user, que="interconsulta")
    return _enrich(ic, db)


@router.post("/", response_model=InterconsultaRead, status_code=201)
def create_interconsulta(
    payload: InterconsultaCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    if not db.get(Paciente, payload.paciente_id):  # aislamiento: ya autorizado por el padre
        raise HTTPException(status_code=404, detail="Paciente no encontrado")
    try:
        prioridad = PrioridadInterconsulta(payload.prioridad)
    except ValueError:
        raise HTTPException(status_code=400, detail=f"Prioridad inválida: {payload.prioridad}")
    ic = Interconsulta(
        medico_solicita_id=current_user.id,
        institucion_id=current_user.institucion_id,
        prioridad=prioridad,
        **{k: v for k, v in payload.model_dump().items() if k != "prioridad"},
    )
    db.add(ic)
    db.commit()
    db.refresh(ic)
    return _enrich(ic, db)


@router.put("/{ic_id}/aceptar", response_model=InterconsultaRead)
def aceptar_interconsulta(
    ic_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    ic = obtener(db, Interconsulta, ic_id, current_user, que="interconsulta")
    if ic.estado != EstadoInterconsulta.SOLICITADA:
        raise HTTPException(status_code=400, detail="Solo se pueden aceptar interconsultas en estado SOLICITADA")
    ic.estado = EstadoInterconsulta.ACEPTADA
    ic.medico_responde_id = current_user.id
    db.add(ic)
    db.commit()
    db.refresh(ic)
    return _enrich(ic, db)


@router.put("/{ic_id}/responder", response_model=InterconsultaRead)
def responder_interconsulta(
    ic_id: int,
    payload: InterconsultaResponder,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    ic = obtener(db, Interconsulta, ic_id, current_user, que="interconsulta")
    if ic.estado == EstadoInterconsulta.CANCELADA:
        raise HTTPException(status_code=400, detail="No se puede responder una interconsulta cancelada")
    ic.estado = EstadoInterconsulta.RESPONDIDA
    ic.medico_responde_id = current_user.id
    ic.respuesta = payload.respuesta
    ic.recomendaciones = payload.recomendaciones
    ic.fecha_respuesta = datetime.utcnow()
    db.add(ic)
    db.commit()
    db.refresh(ic)
    return _enrich(ic, db)


@router.put("/{ic_id}/cancelar", response_model=InterconsultaRead)
def cancelar_interconsulta(
    ic_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    ic = obtener(db, Interconsulta, ic_id, current_user, que="interconsulta")
    if ic.estado == EstadoInterconsulta.RESPONDIDA:
        raise HTTPException(status_code=400, detail="No se puede cancelar una interconsulta ya respondida")
    ic.estado = EstadoInterconsulta.CANCELADA
    db.add(ic)
    db.commit()
    db.refresh(ic)
    return _enrich(ic, db)
