from typing import Optional
from datetime import datetime
from pydantic import BaseModel


class EvolucionCreate(BaseModel):
    paciente_id: int
    subjetivo: str
    objetivo: str
    analisis: str
    plan: str
    historia_relacionada_id: Optional[int] = None
    tension_arterial: Optional[str] = None
    frecuencia_cardiaca: Optional[int] = None
    temperatura: Optional[float] = None
    peso: Optional[float] = None


class EvolucionUpdate(BaseModel):
    subjetivo: Optional[str] = None
    objetivo: Optional[str] = None
    analisis: Optional[str] = None
    plan: Optional[str] = None
    tension_arterial: Optional[str] = None
    frecuencia_cardiaca: Optional[int] = None
    temperatura: Optional[float] = None
    peso: Optional[float] = None


class EvolucionRead(BaseModel):
    id: int
    paciente_id: int
    medico_id: int
    fecha: datetime
    subjetivo: str
    objetivo: str
    analisis: str
    plan: str
    historia_relacionada_id: Optional[int] = None
    tension_arterial: Optional[str] = None
    frecuencia_cardiaca: Optional[int] = None
    temperatura: Optional[float] = None
    peso: Optional[float] = None
    paciente_nombre: Optional[str] = None
    paciente_identificacion: Optional[str] = None
    medico_nombre: Optional[str] = None

    class Config:
        from_attributes = True
