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


# =====================================================================
# MÓDULOS POR INSTITUCIÓN
# =====================================================================
# El mismo binario sirve a un consultorio, una IPS, una clínica y un hospital:
# lo que cambia es QUÉ MÓDULOS tiene licenciados cada institución
# (`Licencia.modulos_habilitados`, catálogo en `ModuloSistema`).
#
# Existía el modelo de datos pero no había forma de exigirlo: el único guardián
# era un decorador `requiere_modulo` heredado de la versión NiceGUI de la app,
# que importaba `nicegui`, llamaba a `ui.notify` y devolvía None. En un backend
# FastAPI no podía funcionar, y ningún router lo usaba.
#
# Distinguir estos tres estados importa, y hasta ahora los tres respondían
# `200 []`, indistinguibles para quien mira la pantalla:
#   403  el módulo existe pero la institución NO lo tiene licenciado
#   501  el módulo está licenciado pero AÚN NO está implementado
#   200 []  el módulo funciona y no hay registros
# En un hospital, que "Banco de Sangre" diga "vacío" en vez de "no habilitado"
# no es una molestia de interfaz: es un dato clínico falso.

def modulos_de(user: "Usuario", db: Session) -> set[str]:
    """Módulos habilitados para la institución del usuario, en MAYÚSCULAS.

    El SUPERADMIN no pertenece a ninguna institución y ve todo: es el dueño
    del sistema multi-institucional, no un cliente.
    """
    from models import EstadoLicencia, Licencia, ModuloSistema

    if user.rol == RolUsuario.SUPERADMIN:
        return {m.value for m in ModuloSistema}
    if not user.institucion_id:
        return set()

    licencia = db.exec(
        select(Licencia)
        .where(Licencia.institucion_id == user.institucion_id)
        .where(Licencia.estado == EstadoLicencia.ACTIVA)
    ).first()
    if not licencia:
        return set()
    return {str(m).upper() for m in (licencia.modulos_habilitados or [])}


def requiere_modulo(modulo) -> Callable:
    """Dependencia de FastAPI: 403 si la institución no licenció el módulo.

    Se usa a nivel de router, no de endpoint, para que no se olvide en uno:

        router = APIRouter(
            prefix="/quirofano",
            dependencies=[Depends(requiere_modulo(ModuloSistema.QUIROFANO))],
        )
    """
    nombre = getattr(modulo, "value", str(modulo)).upper()

    def _verificar(
        user: Usuario = Depends(get_current_active_user),
        db: Session = Depends(get_db),
    ) -> None:
        if nombre not in modulos_de(user, db):
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"El módulo {nombre} no está habilitado para su institución.",
            )

    return _verificar
