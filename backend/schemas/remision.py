from datetime import datetime
from typing import Optional

from pydantic import BaseModel

from models import EstadoOrden


class RemisionRead(BaseModel):
    id: int
    fecha_orden: datetime
    estado: EstadoOrden
    paciente: str
    identificacion: str
    descripcion: str
    justificacion: str
    especialidad_destino: Optional[str] = None
    institucion_destino: Optional[str] = None
    medico_ordena: Optional[str] = None
    codigo_cups: Optional[str] = None
    #: Adónde puede pasar desde aquí. La pantalla no debe ofrecer más que esto.
    estados_posibles: list[EstadoOrden] = []
    abierta: bool = True


class CambioEstado(BaseModel):
    estado: EstadoOrden


class ResumenRemisiones(BaseModel):
    total: int
    abiertas: int
    por_estado: dict[str, int]
