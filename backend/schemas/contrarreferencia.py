from datetime import date, datetime
from typing import Optional

from pydantic import BaseModel


class ContrarreferenciaCreate(BaseModel):
    paciente_id: int
    #: Contra qué remisión. Opcional porque un paciente puede volver de una
    #: institución sin que hubiera remisión formal.
    orden_id: Optional[int] = None
    diagnostico_final: str
    procedimientos_realizados: Optional[str] = None
    tratamiento_instaurado: Optional[str] = None
    recomendaciones: Optional[str] = None
    requiere_seguimiento: bool = False
    fecha_control_sugerida: Optional[date] = None


class ContrarreferenciaRead(BaseModel):
    id: int
    paciente_id: int
    orden_id: Optional[int] = None
    fecha_creacion: datetime
    diagnostico_final: str
    procedimientos_realizados: Optional[str] = None
    tratamiento_instaurado: Optional[str] = None
    recomendaciones: Optional[str] = None
    requiere_seguimiento: bool = False
    fecha_control_sugerida: Optional[date] = None
    recibida: bool = False
    fecha_recepcion: Optional[datetime] = None

    paciente: Optional[str] = None
    #: Qué pasó con la remisión al registrarla. Se devuelve para que la
    #: pantalla pueda decirlo, en vez de que el usuario lo descubra después.
    cierre: Optional[str] = None


class RemisionAbierta(BaseModel):
    """Una remisión contra la que se puede contrarreferir."""

    id: int
    fecha_orden: datetime
    paciente: str
    descripcion: str
    estado: str
