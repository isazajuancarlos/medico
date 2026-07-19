from typing import Optional
from datetime import datetime
from pydantic import BaseModel


class QuirofanoRead(BaseModel):
    id: int
    nombre: str
    tipo: str
    estado: str
    equipamiento: dict
    institucion_id: Optional[int] = None

    class Config:
        from_attributes = True


class ProgramacionRead(BaseModel):
    id: int
    quirofano_id: int
    paciente_id: int
    cirujano_id: int
    anestesiologo_id: Optional[int] = None
    fecha_programada: datetime
    duracion_estimada_min: int
    procedimiento: str
    codigo_cups: Optional[str] = None
    tipo_anestesia: Optional[str] = None
    estado: str
    hora_inicio_real: Optional[datetime] = None
    hora_fin_real: Optional[datetime] = None
    complicaciones: Optional[str] = None
    nota_quirurgica: Optional[str] = None
    hallazgos_quirurgicos: Optional[str] = None
    sangrado_estimado_ml: Optional[int] = None
    especimenes_patologia: Optional[str] = None
    # Checklist prequirúrgico
    ayuno_verificado: bool
    labs_prequirurgicos: bool
    valoracion_preanestesica: bool
    hemoclasificacion: Optional[str] = None
    reserva_sangre: bool
    # Campos enriquecidos
    paciente_nombre: Optional[str] = None
    paciente_identificacion: Optional[str] = None
    quirofano_nombre: Optional[str] = None
    cirujano_nombre: Optional[str] = None
    anestesiologo_nombre: Optional[str] = None

    class Config:
        from_attributes = True


class ProgramacionCreate(BaseModel):
    quirofano_id: int
    paciente_id: int
    cirujano_id: int
    anestesiologo_id: Optional[int] = None
    fecha_programada: datetime
    duracion_estimada_min: int = 60
    procedimiento: str
    codigo_cups: Optional[str] = None
    tipo_anestesia: Optional[str] = None


class ChecklistUpdate(BaseModel):
    ayuno_verificado: Optional[bool] = None
    labs_prequirurgicos: Optional[bool] = None
    valoracion_preanestesica: Optional[bool] = None
    hemoclasificacion: Optional[str] = None
    reserva_sangre: Optional[bool] = None


class IniciarCirugia(BaseModel):
    hora_inicio: Optional[datetime] = None


class CompletarCirugia(BaseModel):
    nota_quirurgica: Optional[str] = None
    hallazgos_quirurgicos: Optional[str] = None
    sangrado_estimado_ml: Optional[int] = None
    especimenes_patologia: Optional[str] = None
    complicaciones: Optional[str] = None
    hora_fin: Optional[datetime] = None


class CancelarCirugia(BaseModel):
    motivo: Optional[str] = None


class MedicoBasic(BaseModel):
    id: int
    nombre_completo: str
    rol: Optional[str] = None

    class Config:
        from_attributes = True
