from typing import Optional
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from models import Usuario, RolUsuario
from schemas.usuario import UsuarioCreate, UsuarioUpdate, UsuarioRead
from schemas.common import PaginatedResponse
from core.deps import get_db, get_current_active_user, require_role
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
    usuario = db.get(Usuario, usuario_id)
    if not usuario:
        raise HTTPException(status_code=404, detail="Usuario no encontrado")
    return usuario


@router.put("/{usuario_id}", response_model=UsuarioRead)
def update_usuario(
    usuario_id: int,
    payload: UsuarioUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(RolUsuario.ADMIN, RolUsuario.SUPERADMIN)),
):
    usuario = db.get(Usuario, usuario_id)
    if not usuario:
        raise HTTPException(status_code=404, detail="Usuario no encontrado")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(usuario, field, value)
    db.add(usuario)
    db.commit()
    db.refresh(usuario)
    return usuario
