from typing import Optional
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from models import Usuario, RolUsuario
from schemas.usuario import UsuarioCreate, UsuarioUpdate, UsuarioRead
from schemas.common import PaginatedResponse
from core.deps import (get_db, get_current_active_user, hay_cupo,
                       exigir_cupo_al_modificar,
                       licencia_vigente, require_role)
from core.aislamiento import obtener
import bcrypt

router = APIRouter(prefix="/usuarios", tags=["usuarios"])


@router.get("/", response_model=PaginatedResponse[UsuarioRead])
def list_usuarios(
    page: int = Query(1, ge=1),
    size: int = Query(20, ge=1, le=100),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(RolUsuario.ADMIN, RolUsuario.SUPERADMIN)),
):
    query = select(Usuario)
    if current_user.rol != RolUsuario.SUPERADMIN:
        query = query.where(Usuario.institucion_id == current_user.institucion_id)
    total = len(db.exec(query).all())
    items = db.exec(query.offset((page - 1) * size).limit(size)).all()
    return PaginatedResponse(total=total, items=items, page=page, size=size)


@router.post("/", response_model=UsuarioRead, status_code=201)
def create_usuario(
    payload: UsuarioCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(RolUsuario.ADMIN, RolUsuario.SUPERADMIN)),
):
    # La licencia se comprueba ANTES de tocar nada. Dar de alta un usuario es
    # crecer, y crecer con la licencia vencida es justo lo que no debe poder
    # hacerse — a diferencia de leer la historia clínica, que sigue abierta
    # aunque haya vencido: un asunto comercial no puede dejar a un hospital sin
    # sus registros.
    vigente, motivo = licencia_vigente(current_user, db)
    if not vigente:
        raise HTTPException(status_code=403, detail=motivo)

    # Y el cupo se cuenta contra las filas reales, no contra
    # `uso_actual_usuarios`: ese contador no lo actualiza nadie y vale 0 desde
    # que se sembró la licencia, así que el control heredado nunca saltaba.
    cabe, motivo = hay_cupo(
        current_user, db, para_medico=payload.rol == RolUsuario.MEDICO)
    if not cabe:
        raise HTTPException(status_code=403, detail=motivo)

    existing = db.exec(select(Usuario).where(Usuario.username == payload.username)).first()
    if existing:
        raise HTTPException(status_code=400, detail="Username ya existe")
    hashed = bcrypt.hashpw(payload.password.encode(), bcrypt.gensalt()).decode()
    user_data = payload.model_dump(exclude={"password"})
    usuario = Usuario(**user_data, hashed_password=hashed, institucion_id=current_user.institucion_id)
    db.add(usuario)
    db.commit()
    db.refresh(usuario)
    return usuario


@router.get("/{usuario_id}", response_model=UsuarioRead)
def get_usuario(usuario_id: int, db: Session = Depends(get_db), current_user: Usuario = Depends(get_current_active_user)):
    usuario = obtener(db, Usuario, usuario_id, current_user, que="usuario")
    return usuario


@router.put("/{usuario_id}", response_model=UsuarioRead)
def update_usuario(
    usuario_id: int,
    payload: UsuarioUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(RolUsuario.ADMIN, RolUsuario.SUPERADMIN)),
):
    usuario = obtener(db, Usuario, usuario_id, current_user, que="usuario")
    # Reactivar o ascender a médico consumen cupo igual que un alta nueva.
    exigir_cupo_al_modificar(usuario, payload.model_dump(exclude_none=True),
                             current_user, db)
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(usuario, field, value)
    db.add(usuario)
    db.commit()
    db.refresh(usuario)
    return usuario
