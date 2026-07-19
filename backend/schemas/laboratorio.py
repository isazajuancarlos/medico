from typing import Optional, Dict, Any, List
from datetime import datetime
from pydantic import BaseModel


# ── Orden de laboratorio (enriquecida) ──────────────────────────────
class OrdenLabRead(BaseModel):
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
    # Campos enriquecidos
    paciente_nombre: Optional[str] = None
    paciente_identificacion: Optional[str] = None
    medico_nombre: Optional[str] = None
    tiene_resultado: bool = False

    class Config:
        from_attributes = True


class EstadoOrdenUpdate(BaseModel):
    estado: str


# ── Resultados de laboratorio ────────────────────────────────────────
class ResultadoCreate(BaseModel):
    resultados_json: Dict[str, Any]  # {"Hemoglobina": {"valor": "13.5", "unidad": "g/dL", "ref_min": "12", "ref_max": "16", "critico": false}}
    observaciones: Optional[str] = None
    valores_criticos: bool = False


class ResultadoUpdate(BaseModel):
    resultados_json: Optional[Dict[str, Any]] = None
    observaciones: Optional[str] = None
    valores_criticos: Optional[bool] = None


class ResultadoRead(BaseModel):
    id: int
    orden_id: int
    paciente_id: Optional[int] = None
    bioanalista_id: int
    bioanalista_nombre: Optional[str] = None
    fecha_resultado: datetime
    resultados_json: Dict[str, Any]
    observaciones: Optional[str] = None
    valores_criticos: bool
    notificado_medico: bool
    validado: bool
    fecha_validacion: Optional[datetime] = None

    class Config:
        from_attributes = True


# ── Perfil de laboratorio (plantillas de exámenes) ────────────────────
class PerfilCreate(BaseModel):
    nombre: str
    descripcion: Optional[str] = None
    examenes: List[Dict[str, Any]]  # [{"nombre": "Hemoglobina", "unidad": "g/dL", "ref_min": "12", "ref_max": "16"}]


class PerfilRead(BaseModel):
    id: int
    nombre: str
    descripcion: Optional[str] = None
    examenes: List[Dict[str, Any]]
    activo: bool

    class Config:
        from_attributes = True
