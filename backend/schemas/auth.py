from typing import Optional
from pydantic import BaseModel
from models import RolUsuario


class LoginRequest(BaseModel):
    username: str
    password: str


class TokenResponse(BaseModel):
    access_token: str
    refresh_token: str
    token_type: str = "bearer"
    user: "UserRead"


class UserRead(BaseModel):
    id: int
    username: str
    nombre_completo: str
    rol: RolUsuario
    institucion_id: Optional[int]
    email: Optional[str]

    class Config:
        from_attributes = True


class Token2FARequest(BaseModel):
    token: str
    temp_token: str


class ChangePasswordRequest(BaseModel):
    current_password: str
    new_password: str


class RefreshRequest(BaseModel):
    refresh_token: str


TokenResponse.model_rebuild()
