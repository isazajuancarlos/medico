from typing import Optional
from datetime import datetime
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from models import Hospitalizacion, Cama, Paciente, Usuario, RolUsuario
from core.deps import get_db, get_current_active_user, filtrar_por_institucion
from schemas.hospitalizacion import (
    HospitalizacionRead, CamaRead, IngresoCreate, EgresoUpdate, MedicoBasic,
)

router = APIRouter(prefix="/hospitalizacion", tags=["hospitalizacion"])


def _enrich(hosp: Hospitalizacion, db: Session) -> HospitalizacionRead:
    data = HospitalizacionRead.model_validate(hosp)
    p = db.get(Paciente, hosp.paciente_id)
    if p:
        data.paciente_nombre = p.nombre_completo.strip()
        data.paciente_identificacion = p.identificacion
    cama = db.get(Cama, hosp.cama_id)
    if cama:
        data.cama_numero = cama.numero
        data.cama_piso = cama.piso
        data.cama_servicio = cama.servicio
    m = db.get(Usuario, hosp.medico_tratante_id)
    if m:
        data.medico_nombre = m.nombre_completo
    return data


@router.get("/", response_model=list[HospitalizacionRead])
def list_hospitalizaciones(
    estado: Optional[str] = Query(None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(Hospitalizacion)
    if estado:
        query = query.where(Hospitalizacion.estado == estado)
    else:
        query = query.where(Hospitalizacion.estado == "ACTIVA")
    try:
        query = filtrar_por_institucion(query, Hospitalizacion, current_user)
    except Exception:
        pass
    query = query.order_by(Hospitalizacion.fecha_ingreso.desc())
    return [_enrich(h, db) for h in db.exec(query).all()]


@router.get("/camas", response_model=list[CamaRead])
def list_camas(
    estado: Optional[str] = Query(None),
    servicio: Optional[str] = Query(None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(Cama)
    if estado:
        query = query.where(Cama.estado == estado)
    if servicio:
        query = query.where(Cama.servicio == servicio)
    query = query.order_by(Cama.piso, Cama.numero)
    return db.exec(query).all()


@router.get("/medicos", response_model=list[MedicoBasic])
def list_medicos(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    roles_medico = [RolUsuario.MEDICO, RolUsuario.ANESTESIOLOGO]
    query = (
        select(Usuario)
        .where(Usuario.rol.in_(roles_medico))
        .where(Usuario.activo == True)
    )
    if current_user.rol != RolUsuario.SUPERADMIN and current_user.institucion_id:
        query = query.where(Usuario.institucion_id == current_user.institucion_id)
    return db.exec(query).all()


@router.post("/ingresar", response_model=HospitalizacionRead, status_code=201)
def ingresar_paciente(
    payload: IngresoCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    cama = db.get(Cama, payload.cama_id)
    if not cama:
        raise HTTPException(status_code=404, detail="Cama no encontrada")
    if cama.estado != "DISPONIBLE":
        raise HTTPException(status_code=400, detail=f"La cama no está disponible (estado: {cama.estado})")
    hosp = Hospitalizacion(
        paciente_id=payload.paciente_id,
        cama_id=payload.cama_id,
        medico_tratante_id=payload.medico_id,
        diagnostico_ingreso=payload.diagnostico_ingreso,
        via_ingreso=payload.via_ingreso,
        motivo_ingreso=payload.motivo_ingreso,
        observaciones_ingreso=payload.observaciones_ingreso,
    )
    db.add(hosp)
    cama.estado = "OCUPADA"
    db.add(cama)
    db.commit()
    db.refresh(hosp)
    return _enrich(hosp, db)


@router.put("/{hosp_id}/egreso", response_model=HospitalizacionRead)
def egreso_paciente(
    hosp_id: int,
    payload: EgresoUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    hosp = db.get(Hospitalizacion, hosp_id)
    if not hosp:
        raise HTTPException(status_code=404, detail="Hospitalización no encontrada")
    if hosp.estado != "ACTIVA":
        raise HTTPException(status_code=400, detail="La hospitalización ya tiene egreso registrado")
    hosp.estado = payload.tipo_egreso
    hosp.fecha_egreso = datetime.utcnow()
    hosp.diagnostico_egreso = payload.diagnostico_egreso
    hosp.resumen_clinico = payload.resumen_clinico
    cama = db.get(Cama, hosp.cama_id)
    if cama:
        cama.estado = "DISPONIBLE"
        db.add(cama)
    db.add(hosp)
    db.commit()
    db.refresh(hosp)
    return _enrich(hosp, db)
