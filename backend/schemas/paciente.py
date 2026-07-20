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
    """Lo que se puede corregir de un paciente ya creado.

    Incluye `identificacion`, `tipo_id`, `fecha_nacimiento` y `sexo`, que antes
    quedaban fuera. No era una salvaguarda: eran justo los campos de los que
    depende el RIPS —la edad y el sexo determinan la clasificación, y el
    documento es la llave del usuario— y un error de digitación en ellos era
    permanente. Quien lo capturó mal a las siete de la mañana tiene que poder
    arreglarlo; lo que hace falta es que quede registro, no que sea imposible.
    """

    identificacion: Optional[str] = None
    tipo_id: Optional[str] = None
    nombre1: Optional[str] = None
    nombre2: Optional[str] = None
    apellido1: Optional[str] = None
    apellido2: Optional[str] = None
    fecha_nacimiento: Optional[date] = None
    sexo: Optional[str] = None
    celular: Optional[str] = None
    email: Optional[str] = None
    direccion: Optional[str] = None
    tipo_usuario: Optional[TipoUsuario] = None
    cod_municipio: Optional[str] = None
    cod_pais: Optional[str] = None
    zona: Optional[ZonaResidencia] = None
    entidad_id: Optional[int] = None


class PacienteRead(PacienteBase):
    id: int
    edad: Optional[int] = None
    institucion_id: Optional[int] = None

    class Config:
        from_attributes = True
