from typing import Optional
from datetime import date
from pydantic import BaseModel, field_validator

from divipola import MunicipioNoValido, validar as validar_municipio
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
    # Sin defecto. Tenía "05001", que reportaba a TODO paciente del país como
    # residente de Medellín: el archivo pasaba la validación de estructura y la
    # estadística de residencia salía falsa.
    cod_municipio: str
    cod_pais: str = "170"
    zona: ZonaResidencia = ZonaResidencia.URBANA
    entidad_id: Optional[int] = None


    @field_validator("cod_municipio")
    @classmethod
    def _municipio_existe(cls, v: str) -> str:
        """Normaliza a cinco dígitos y comprueba contra DIVIPOLA.

        Lo de normalizar no es cosmética: Antioquia es el departamento 05, así
        que un `5001` llegado de una hoja de cálculo tiene que volverse
        `05001` o el RIPS sale mal para el 13 % de los municipios del país.
        """
        try:
            return validar_municipio(v)
        except MunicipioNoValido as e:
            raise ValueError(str(e)) from None


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

    @field_validator("cod_municipio")
    @classmethod
    def _municipio_existe(cls, v):
        # Al corregir sí puede venir ausente —significa "no lo toques"—, pero
        # si viene, tiene que ser uno real.
        return validar_municipio(v) if v is not None else v


class PacienteRead(PacienteBase):
    """Lo que sale hacia el cliente.

    NO revalida el municipio a propósito. Los pacientes creados antes de esta
    comprobación pueden tener un código vacío o heredado del defecto viejo, y
    si leerlos fallara no habría forma de ABRIRLOS PARA CORREGIRLOS: el dato
    malo quedaría encerrado. La exigencia va en la escritura; en la lectura,
    `auditoria_rips` es quien los señala antes de generar el archivo.
    """

    # Optional a propósito: en la base ya hay pacientes con el municipio en
    # NULL, y `str` haría que LEERLOS reventara con un 500. El dato malo
    # quedaría encerrado, sin forma de abrirlo para corregirlo.
    cod_municipio: Optional[str] = None

    @field_validator("cod_municipio")
    @classmethod
    def _municipio_existe(cls, v):
        return v

    id: int
    edad: Optional[int] = None
    institucion_id: Optional[int] = None

    class Config:
        from_attributes = True
