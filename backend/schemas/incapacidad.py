from datetime import date, datetime
from typing import Optional

from pydantic import BaseModel


class IncapacidadRead(BaseModel):
    """Una fila de la lista."""

    historia_id: int
    codigo_historia: str
    fecha_consulta: datetime
    paciente: str
    identificacion: str
    dias: int
    #: `None` si la consulta está marcada como incapacitante pero sin días.
    #: Se prefiere el nulo a una fecha inventada: de ella depende cuándo se
    #: reincorpora el trabajador.
    fecha_inicio: Optional[date] = None
    fecha_fin: Optional[date] = None
    diagnostico: str
    medico: str


class IncapacidadDetalle(IncapacidadRead):
    """Lo que hace falta para imprimir la certificación."""

    motivo_consulta: Optional[str] = None
    diagnostico_relacionado: Optional[str] = None
    observaciones: Optional[str] = None


class ResumenIncapacidades(BaseModel):
    cantidad: int
    dias_totales: int
    promedio_dias: float
