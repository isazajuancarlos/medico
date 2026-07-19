from typing import Optional
from datetime import datetime, date
from pydantic import BaseModel


class PlanCreate(BaseModel):
    paciente_id: int
    diagnostico_funcional: str
    objetivos: str
    sesiones_autorizadas: int = 10
    tipo_terapia: str  # FISICA, OCUPACIONAL, LENGUAJE, RESPIRATORIA
    frecuencia: str
    duracion_sesion_min: int = 45


class PlanUpdate(BaseModel):
    diagnostico_funcional: Optional[str] = None
    objetivos: Optional[str] = None
    sesiones_autorizadas: Optional[int] = None
    tipo_terapia: Optional[str] = None
    frecuencia: Optional[str] = None
    duracion_sesion_min: Optional[int] = None
    activo: Optional[bool] = None
    fecha_cierre: Optional[date] = None
    resultado_final: Optional[str] = None


class PlanRead(BaseModel):
    id: int
    paciente_id: int
    terapeuta_id: int
    fecha_creacion: datetime
    diagnostico_funcional: str
    objetivos: str
    sesiones_autorizadas: int
    sesiones_realizadas: int
    tipo_terapia: str
    frecuencia: str
    duracion_sesion_min: int
    activo: bool
    fecha_cierre: Optional[date] = None
    resultado_final: Optional[str] = None
    paciente_nombre: Optional[str] = None
    paciente_identificacion: Optional[str] = None
    terapeuta_nombre: Optional[str] = None

    class Config:
        from_attributes = True


class SesionCreate(BaseModel):
    evaluacion_funcional: str
    dolor_eva: Optional[int] = None
    rango_movimiento: Optional[str] = None
    tecnicas_aplicadas: str
    ejercicios_domicilio: Optional[str] = None
    progreso: Optional[str] = None  # MEJORIA, ESTABLE, DETERIORO
    observaciones: Optional[str] = None


class SesionUpdate(BaseModel):
    evaluacion_funcional: Optional[str] = None
    dolor_eva: Optional[int] = None
    rango_movimiento: Optional[str] = None
    tecnicas_aplicadas: Optional[str] = None
    ejercicios_domicilio: Optional[str] = None
    progreso: Optional[str] = None
    observaciones: Optional[str] = None


class SesionRead(BaseModel):
    id: int
    plan_id: int
    paciente_id: int
    terapeuta_id: int
    fecha_sesion: datetime
    numero_sesion: int
    evaluacion_funcional: str
    dolor_eva: Optional[int] = None
    rango_movimiento: Optional[str] = None
    tecnicas_aplicadas: str
    ejercicios_domicilio: Optional[str] = None
    progreso: Optional[str] = None
    observaciones: Optional[str] = None
    terapeuta_nombre: Optional[str] = None

    class Config:
        from_attributes = True
