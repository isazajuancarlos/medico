from typing import Optional
from datetime import datetime
from pydantic import BaseModel


class InterconsultaCreate(BaseModel):
    paciente_id: int
    especialidad_solicitada_id: int
    prioridad: str = "NORMAL"
    motivo: str
    historia_clinica_resumen: Optional[str] = None
    hospitalizacion_id: Optional[int] = None


class InterconsultaResponder(BaseModel):
    respuesta: str
    recomendaciones: Optional[str] = None


class InterconsultaRead(BaseModel):
    id: int
    paciente_id: int
    hospitalizacion_id: Optional[int] = None
    medico_solicita_id: int
    especialidad_solicitada_id: int
    medico_responde_id: Optional[int] = None
    prioridad: str
    estado: str
    motivo: str
    historia_clinica_resumen: Optional[str] = None
    respuesta: Optional[str] = None
    recomendaciones: Optional[str] = None
    fecha_solicitud: datetime
    fecha_respuesta: Optional[datetime] = None
    # Enriched
    paciente_nombre: Optional[str] = None
    paciente_identificacion: Optional[str] = None
    medico_solicita_nombre: Optional[str] = None
    medico_responde_nombre: Optional[str] = None
    especialidad_nombre: Optional[str] = None

    class Config:
        from_attributes = True
