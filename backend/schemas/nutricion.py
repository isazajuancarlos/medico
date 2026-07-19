from typing import Optional, Dict, Any
from datetime import datetime
from pydantic import BaseModel


class ValoracionCreate(BaseModel):
    paciente_id: int
    peso: float
    talla: float
    imc: float
    circunferencia_brazo: Optional[float] = None
    pliegue_tricipital: Optional[float] = None
    screening_nutricional: str = "NRS-2002"
    puntaje_screening: Optional[int] = None
    riesgo_nutricional: str = "BAJO"
    diagnostico_nutricional: str
    requerimiento_calorico: Optional[float] = None
    requerimiento_proteico: Optional[float] = None
    albumina: Optional[float] = None
    prealbumina: Optional[float] = None
    observaciones: Optional[str] = None
    hospitalizacion_id: Optional[int] = None


class ValoracionUpdate(BaseModel):
    peso: Optional[float] = None
    talla: Optional[float] = None
    imc: Optional[float] = None
    circunferencia_brazo: Optional[float] = None
    pliegue_tricipital: Optional[float] = None
    screening_nutricional: Optional[str] = None
    puntaje_screening: Optional[int] = None
    riesgo_nutricional: Optional[str] = None
    diagnostico_nutricional: Optional[str] = None
    requerimiento_calorico: Optional[float] = None
    requerimiento_proteico: Optional[float] = None
    albumina: Optional[float] = None
    prealbumina: Optional[float] = None
    observaciones: Optional[str] = None


class ValoracionRead(BaseModel):
    id: int
    paciente_id: int
    nutricionista_id: int
    fecha_valoracion: datetime
    peso: float
    talla: float
    imc: float
    circunferencia_brazo: Optional[float] = None
    pliegue_tricipital: Optional[float] = None
    screening_nutricional: str
    puntaje_screening: Optional[int] = None
    riesgo_nutricional: str
    diagnostico_nutricional: str
    requerimiento_calorico: Optional[float] = None
    requerimiento_proteico: Optional[float] = None
    albumina: Optional[float] = None
    prealbumina: Optional[float] = None
    observaciones: Optional[str] = None
    hospitalizacion_id: Optional[int] = None
    paciente_nombre: Optional[str] = None
    paciente_identificacion: Optional[str] = None
    nutricionista_nombre: Optional[str] = None

    class Config:
        from_attributes = True


class PlanCreate(BaseModel):
    tipo_dieta: str
    calorias_totales: float
    distribucion_calorica: Dict[str, Any] = {}
    menu_diario: Dict[str, Any] = {}
    restricciones: Optional[str] = None
    suplementos: Optional[str] = None
    via_alimentacion: str = "ORAL"
    observaciones: Optional[str] = None


class PlanUpdate(BaseModel):
    tipo_dieta: Optional[str] = None
    calorias_totales: Optional[float] = None
    distribucion_calorica: Optional[Dict[str, Any]] = None
    menu_diario: Optional[Dict[str, Any]] = None
    restricciones: Optional[str] = None
    suplementos: Optional[str] = None
    via_alimentacion: Optional[str] = None
    observaciones: Optional[str] = None
    activo: Optional[bool] = None


class PlanRead(BaseModel):
    id: int
    valoracion_id: int
    paciente_id: int
    nutricionista_id: int
    fecha_creacion: datetime
    tipo_dieta: str
    calorias_totales: float
    distribucion_calorica: Dict[str, Any] = {}
    menu_diario: Dict[str, Any] = {}
    restricciones: Optional[str] = None
    suplementos: Optional[str] = None
    via_alimentacion: str
    activo: bool
    observaciones: Optional[str] = None
    nutricionista_nombre: Optional[str] = None

    class Config:
        from_attributes = True
