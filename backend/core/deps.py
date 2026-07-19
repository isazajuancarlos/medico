from typing import Generator, Optional, Callable
from fastapi import Depends, HTTPException, status, Cookie
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials
from sqlmodel import Session, select
from database import engine
from models import Usuario, RolUsuario
from core.security import decode_token

security = HTTPBearer(auto_error=False)


def get_db() -> Generator:
    with Session(engine) as session:
        yield session


def get_current_user(
    credentials: Optional[HTTPAuthorizationCredentials] = Depends(security),
    access_token: Optional[str] = Cookie(default=None),
    db: Session = Depends(get_db),
) -> Usuario:
    token = None
    if credentials:
        token = credentials.credentials
    elif access_token:
        token = access_token

    if not token:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="No autenticado",
            headers={"WWW-Authenticate": "Bearer"},
        )

    payload = decode_token(token)
    if payload.get("type") != "access":
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Token inválido")

    user_id = int(payload.get("sub", 0))
    user = db.get(Usuario, user_id)
    if not user:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Usuario no encontrado")
    return user


def get_current_active_user(user: Usuario = Depends(get_current_user)) -> Usuario:
    if not user.activo:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Cuenta inactiva")
    if user.cuenta_bloqueada:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Cuenta bloqueada")
    return user


def require_role(*roles: RolUsuario) -> Callable:
    def _checker(user: Usuario = Depends(get_current_active_user)) -> Usuario:
        if user.rol not in roles:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"Rol requerido: {', '.join(r.value for r in roles)}",
            )
        return user
    return _checker


def get_institucion_id(user: Usuario = Depends(get_current_active_user)) -> Optional[int]:
    return user.institucion_id


def filtrar_por_institucion(query, model, user: Usuario):
    """Filtra por institución si el usuario no es SUPERADMIN"""
    if user.rol != RolUsuario.SUPERADMIN and hasattr(model, "institucion_id"):
        query = query.where(model.institucion_id == user.institucion_id)
    return query
