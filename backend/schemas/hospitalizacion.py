from typing import Optional
from datetime import datetime
from pydantic import BaseModel


class CamaRead(BaseModel):
    id: int
    numero: str
    piso: str
    servicio: str
    estado: str
    tiene_oxigeno: bool = False
    tiene_monitor: bool = False
    es_aislamiento: bool = False

    class Config:
        from_attributes = True


class HospitalizacionRead(BaseModel):
    id: int
    paciente_id: int
    cama_id: int
    medico_tratante_id: int
    fecha_ingreso: datetime
    fecha_egreso: Optional[datetime] = None
    diagnostico_ingreso: str
    via_ingreso: str
    estado: str
    motivo_ingreso: Optional[str] = None
    observaciones_ingreso: Optional[str] = None
    diagnostico_egreso: Optional[str] = None
    resumen_clinico: Optional[str] = None
    # Enriched
    paciente_nombre: Optional[str] = None
    paciente_identificacion: Optional[str] = None
    cama_numero: Optional[str] = None
    cama_piso: Optional[str] = None
    cama_servicio: Optional[str] = None
    medico_nombre: Optional[str] = None

    class Config:
        from_attributes = True


class IngresoCreate(BaseModel):
    paciente_id: int
    cama_id: int
    medico_id: int
    diagnostico_ingreso: str = "Z00.0"
    via_ingreso: str = "PROGRAMADA"
    motivo_ingreso: Optional[str] = None
    observaciones_ingreso: Optional[str] = None


class EgresoUpdate(BaseModel):
    tipo_egreso: str = "ALTA_MEDICA"
    diagnostico_egreso: Optional[str] = None
    resumen_clinico: Optional[str] = None


class MedicoBasic(BaseModel):
    id: int
    nombre_completo: str

    class Config:
        from_attributes = True
