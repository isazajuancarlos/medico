from typing import Optional, List
from datetime import datetime
from pydantic import BaseModel


class OrdenImagenRead(BaseModel):
    id: int
    historia_id: int
    paciente_id: Optional[int] = None
    medico_ordena_id: Optional[int] = None
    fecha_orden: datetime
    estado: str
    tipo: str
    codigo_cups: Optional[str] = None
    descripcion: str
    justificacion: str
    paciente_nombre: Optional[str] = None
    paciente_identificacion: Optional[str] = None
    medico_nombre: Optional[str] = None
    tiene_resultado: bool = False

    class Config:
        from_attributes = True


class EstadoOrdenUpdate(BaseModel):
    estado: str


class InformeCreate(BaseModel):
    tecnica: str
    hallazgos: str
    impresion_diagnostica: str
    recomendaciones: Optional[str] = None
    clasificacion: Optional[str] = None


class InformeUpdate(BaseModel):
    tecnica: Optional[str] = None
    hallazgos: Optional[str] = None
    impresion_diagnostica: Optional[str] = None
    recomendaciones: Optional[str] = None
    clasificacion: Optional[str] = None


class InformeRead(BaseModel):
    id: int
    orden_id: int
    paciente_id: Optional[int] = None
    radiologo_id: int
    radiologo_nombre: Optional[str] = None
    fecha_resultado: datetime
    tecnica: str
    hallazgos: str
    impresion_diagnostica: str
    recomendaciones: Optional[str] = None
    clasificacion: Optional[str] = None
    imagenes_paths: List[str] = []
    firmado: bool

    class Config:
        from_attributes = True
