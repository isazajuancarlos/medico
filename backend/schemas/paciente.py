from typing import Optional
from datetime import date
from pydantic import BaseModel
from models import TipoUsuario, ZonaResidencia


class PacienteBase(BaseModel):
    identificacion: str
    tipo_id: str = "CC"
    nombre1: str
    nombre2: Optional[str] = None
    apellido1: str
    apellido2: Optional[str] = None
    fecha_nacimiento: date
    sexo: str
    celular: str
    email: Optional[str] = None
    direccion: Optional[str] = None
    tipo_usuario: TipoUsuario = TipoUsuario.CONTRIBUTIVO
    cod_municipio: str = "05001"
    cod_pais: str = "170"
    zona: ZonaResidencia = ZonaResidencia.URBANA
    entidad_id: Optional[int] = None


class PacienteCreate(PacienteBase):
    pass


class PacienteUpdate(BaseModel):
    nombre1: Optional[str] = None
    nombre2: Optional[str] = None
    apellido1: Optional[str] = None
    apellido2: Optional[str] = None
    celular: Optional[str] = None
    email: Optional[str] = None
    direccion: Optional[str] = None
    tipo_usuario: Optional[TipoUsuario] = None
    zona: Optional[ZonaResidencia] = None
    entidad_id: Optional[int] = None


class PacienteRead(PacienteBase):
    id: int
    edad: Optional[int] = None
    institucion_id: Optional[int] = None

    class Config:
        from_attributes = True
