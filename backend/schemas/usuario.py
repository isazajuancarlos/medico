from typing import Optional
from pydantic import BaseModel
from models import RolUsuario


class UsuarioCreate(BaseModel):
    username: str
    password: str
    nombre_completo: str
    email: Optional[str] = None
    rol: RolUsuario = RolUsuario.MEDICO
    especialidad_id: Optional[int] = None
    registro_medico: Optional[str] = None


class UsuarioUpdate(BaseModel):
    nombre_completo: Optional[str] = None
    email: Optional[str] = None
    rol: Optional[RolUsuario] = None
    activo: Optional[bool] = None
    especialidad_id: Optional[int] = None
    registro_medico: Optional[str] = None


class UsuarioRead(BaseModel):
    id: int
    username: str
    nombre_completo: str
    email: Optional[str]
    rol: RolUsuario
    activo: bool
    especialidad_id: Optional[int]
    institucion_id: Optional[int]
    cuenta_bloqueada: bool = False

    class Config:
        from_attributes = True
