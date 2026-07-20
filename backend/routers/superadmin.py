from fastapi import APIRouter, Depends, HTTPException
from sqlmodel import Session, select
from models import Institucion, Licencia, Usuario, RolUsuario, AuditoriaSuperAdmin
from core.deps import get_db, get_current_active_user, require_role
from core.aislamiento import obtener
from pydantic import BaseModel
from typing import Optional
from datetime import datetime

router = APIRouter(prefix="/superadmin", tags=["superadmin"])


class InstitucionCreate(BaseModel):
    nombre: str
    nit: str
    codigo_habilitacion: Optional[str] = None
    direccion: Optional[str] = None
    telefono: Optional[str] = None
    email: Optional[str] = None


@router.get("/instituciones")
def list_instituciones(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(RolUsuario.SUPERADMIN)),
):
    return db.exec(select(Institucion)).all()


@router.post("/instituciones", status_code=201)
def create_institucion(
    payload: InstitucionCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(RolUsuario.SUPERADMIN)),
):
    inst = Institucion(**payload.model_dump())
    db.add(inst)
    db.commit()
    db.refresh(inst)
    return inst


@router.get("/instituciones/{inst_id}")
def get_institucion(
    inst_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(RolUsuario.SUPERADMIN)),
):
    inst = obtener(db, Institucion, inst_id, current_user, que="institución")
    return inst


@router.put("/instituciones/{inst_id}")
def update_institucion(
    inst_id: int,
    payload: InstitucionCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(RolUsuario.SUPERADMIN)),
):
    inst = obtener(db, Institucion, inst_id, current_user, que="institución")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(inst, field, value)
    db.add(inst)
    db.commit()
    db.refresh(inst)
    return inst


@router.get("/licencias")
def list_licencias(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(RolUsuario.SUPERADMIN)),
):
    return db.exec(select(Licencia)).all()


@router.put("/licencias/{lic_id}/activar")
def activar_licencia(
    lic_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(RolUsuario.SUPERADMIN)),
):
    lic = obtener(db, Licencia, lic_id, current_user, que="licencia")
    lic.activa = True
    db.add(lic)
    db.commit()
    return {"message": "Licencia activada"}


@router.put("/licencias/{lic_id}/suspender")
def suspender_licencia(
    lic_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(RolUsuario.SUPERADMIN)),
):
    lic = obtener(db, Licencia, lic_id, current_user, que="licencia")
    lic.activa = False
    db.add(lic)
    db.commit()
    return {"message": "Licencia suspendida"}


@router.get("/auditoria")
def get_auditoria_superadmin(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(RolUsuario.SUPERADMIN)),
):
    return db.exec(select(AuditoriaSuperAdmin).order_by(AuditoriaSuperAdmin.fecha.desc()).limit(100)).all()
