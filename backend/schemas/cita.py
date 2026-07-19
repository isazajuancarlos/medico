from typing import Optional, Any
from datetime import date, datetime, time as time_t
from pydantic import BaseModel, model_validator
from models import EstadoCita, TipoCita


class CitaCreate(BaseModel):
    paciente_id: int
    medico_id: int
    fecha: date
    hora: str  # "HH:MM"
    tipo: TipoCita = TipoCita.PRIMERA_VEZ
    motivo: Optional[str] = None
    duracion_minutos: int = 30

    def get_fecha_hora(self) -> datetime:
        h, m = map(int, self.hora.split(':'))
        return datetime.combine(self.fecha, time_t(h, m))


class CitaUpdate(BaseModel):
    estado: Optional[EstadoCita] = None
    motivo: Optional[str] = None
    hora: Optional[str] = None
    fecha: Optional[date] = None
    duracion_minutos: Optional[int] = None


class CitaRead(BaseModel):
    id: int
    paciente_id: int
    medico_id: int
    fecha: date
    hora: str
    tipo: TipoCita
    estado: EstadoCita
    motivo: Optional[str] = None
    duracion_minutos: int = 30
    paciente_nombre: Optional[str] = None
    medico_nombre: Optional[str] = None

    model_config = {"from_attributes": True}

    @model_validator(mode='before')
    @classmethod
    def from_cita_orm(cls, v: Any) -> Any:
        if hasattr(v, 'fecha_hora') and v.fecha_hora is not None:
            return {
                'id': v.id,
                'paciente_id': v.paciente_id,
                'medico_id': v.medico_id,
                'fecha': v.fecha_hora.date(),
                'hora': v.fecha_hora.strftime('%H:%M'),
                'tipo': v.tipo,
                'estado': v.estado,
                'motivo': getattr(v, 'motivo', None),
                'duracion_minutos': getattr(v, 'duracion_minutos', None) or 30,
            }
        return v


class SlotDisponible(BaseModel):
    hora: str
    disponible: bool = True
    motivo_no_disponible: Optional[str] = None
