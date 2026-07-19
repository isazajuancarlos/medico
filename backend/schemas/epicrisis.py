from typing import Optional
from datetime import datetime
from pydantic import BaseModel


class EpicrisisCreate(BaseModel):
    paciente_id: int
    fecha_ingreso: datetime
    fecha_egreso: datetime
    dias_estancia: int
    diagnostico_ingreso: str
    diagnostico_egreso: str
    complicaciones: Optional[str] = None
    resumen_evolucion: str
    procedimientos_realizados: str
    examenes_relevantes: Optional[str] = None
    recomendaciones_egreso: str
    medicamentos_egreso: Optional[str] = None
    controles_programados: Optional[str] = None
    pronostico: str = "BUENO"
    incapacidad_dias: int = 0


class EpicrisisUpdate(BaseModel):
    fecha_ingreso: Optional[datetime] = None
    fecha_egreso: Optional[datetime] = None
    dias_estancia: Optional[int] = None
    diagnostico_ingreso: Optional[str] = None
    diagnostico_egreso: Optional[str] = None
    complicaciones: Optional[str] = None
    resumen_evolucion: Optional[str] = None
    procedimientos_realizados: Optional[str] = None
    examenes_relevantes: Optional[str] = None
    recomendaciones_egreso: Optional[str] = None
    medicamentos_egreso: Optional[str] = None
    controles_programados: Optional[str] = None
    pronostico: Optional[str] = None
    incapacidad_dias: Optional[int] = None


class EpicrisisRead(BaseModel):
    id: int
    paciente_id: int
    medico_tratante_id: int
    fecha_ingreso: datetime
    fecha_egreso: datetime
    dias_estancia: int
    diagnostico_ingreso: str
    diagnostico_egreso: str
    complicaciones: Optional[str] = None
    resumen_evolucion: str
    procedimientos_realizados: str
    examenes_relevantes: Optional[str] = None
    recomendaciones_egreso: str
    medicamentos_egreso: Optional[str] = None
    controles_programados: Optional[str] = None
    pronostico: str
    incapacidad_dias: int = 0
    firma_medico_path: Optional[str] = None
    # Enriched
    paciente_nombre: Optional[str] = None
    paciente_identificacion: Optional[str] = None
    medico_nombre: Optional[str] = None

    class Config:
        from_attributes = True
