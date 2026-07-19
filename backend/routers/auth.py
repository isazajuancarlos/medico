from datetime import timedelta
from typing import Optional
from fastapi import APIRouter, Depends, HTTPException, Response, Cookie, status
from sqlmodel import Session, select
from database import engine
from models import Usuario, RolUsuario
from schemas.auth import LoginRequest, TokenResponse, UserRead, ChangePasswordRequest
from core.security import create_access_token, create_refresh_token, decode_token
from core.deps import get_db, get_current_active_user
from core.passwords import hash_password, verificar_y_migrar, verify_password  # noqa: F401

router = APIRouter(prefix="/auth", tags=["auth"])


@router.post("/login", response_model=TokenResponse)
def login(payload: LoginRequest, response: Response, db: Session = Depends(get_db)):
    user = db.exec(select(Usuario).where(Usuario.username == payload.username)).first()
    # `verificar_y_migrar` sube el hash al esquema vigente cuando toca. Es el
    # único momento en que la contraseña está en claro, así que es aquí o nunca.
    if not user or not verificar_y_migrar(payload.password, user):
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Credenciales incorrectas")
    if not user.activo:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Cuenta inactiva")
    if user.cuenta_bloqueada:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Cuenta bloqueada")

    db.add(user)
    db.commit()
    db.refresh(user)

    token_data = {"sub": str(user.id), "rol": user.rol.value, "institucion_id": user.institucion_id}
    access_token = create_access_token(token_data)
    refresh_token = create_refresh_token(user.id)

    response.set_cookie("access_token", access_token, httponly=True, samesite="lax", max_age=900)
    response.set_cookie("refresh_token", refresh_token, httponly=True, samesite="lax", max_age=604800)

    return TokenResponse(
        access_token=access_token,
        refresh_token=refresh_token,
        user=UserRead.model_validate(user),
    )


@router.post("/logout")
def logout(response: Response, current_user: Usuario = Depends(get_current_active_user)):
    response.delete_cookie("access_token")
    response.delete_cookie("refresh_token")
    return {"message": "Sesión cerrada"}


@router.post("/refresh")
def refresh_token(response: Response, refresh_token: Optional[str] = Cookie(default=None), db: Session = Depends(get_db)):
    if not refresh_token:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="No refresh token")
    payload = decode_token(refresh_token)
    if payload.get("type") != "refresh":
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Token inválido")
    user_id = int(payload.get("sub", 0))
    user = db.get(Usuario, user_id)
    if not user or not user.activo:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Usuario no válido")
    token_data = {"sub": str(user.id), "rol": user.rol.value, "institucion_id": user.institucion_id}
    new_access = create_access_token(token_data)
    response.set_cookie("access_token", new_access, httponly=True, samesite="lax", max_age=900)
    return {"access_token": new_access, "token_type": "bearer"}


@router.get("/me", response_model=UserRead)
def get_me(current_user: Usuario = Depends(get_current_active_user)):
    return UserRead.model_validate(current_user)


@router.post("/change-password")
def change_password(
    payload: ChangePasswordRequest,
    current_user: Usuario = Depends(get_current_active_user),
    db: Session = Depends(get_db),
):
    if not verify_password(payload.current_password, current_user.hashed_password):
        raise HTTPException(status_code=400, detail="Contraseña actual incorrecta")
    current_user.hashed_password = hash_password(payload.new_password)
    db.add(current_user)
    db.commit()
    return {"message": "Contraseña actualizada"}
