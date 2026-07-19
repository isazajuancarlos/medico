from typing import Optional, List
from datetime import date, datetime
from pydantic import BaseModel


class EntidadRead(BaseModel):
    id: int
    nit: str
    nombre: str
    codigo_eps: str

    class Config:
        from_attributes = True


class ContratoRead(BaseModel):
    id: int
    entidad_id: int
    nombre: str
    numero_contrato: str
    fecha_inicio: date
    fecha_fin: date
    activo: bool
    tipo_tarifa: str
    porcentaje_incremento: float
    observaciones: Optional[str] = None
    entidad_nombre: Optional[str] = None

    class Config:
        from_attributes = True


class ContratoCreate(BaseModel):
    entidad_id: int
    nombre: str
    numero_contrato: str
    fecha_inicio: date
    fecha_fin: date
    tipo_tarifa: str = "CUPS"
    porcentaje_incremento: float = 0
    observaciones: Optional[str] = None


class ContratoUpdate(BaseModel):
    nombre: Optional[str] = None
    fecha_inicio: Optional[date] = None
    fecha_fin: Optional[date] = None
    tipo_tarifa: Optional[str] = None
    porcentaje_incremento: Optional[float] = None
    observaciones: Optional[str] = None
    activo: Optional[bool] = None


class DetalleRead(BaseModel):
    id: int
    factura_id: int
    servicio: str
    codigo_cups: str
    cantidad: int
    valor_unitario: float
    valor_total: float
    paciente_id: Optional[int] = None
    historia_id: Optional[int] = None
    paciente_nombre: Optional[str] = None

    class Config:
        from_attributes = True


class DetalleCreate(BaseModel):
    servicio: str
    codigo_cups: str = "890201"
    cantidad: int = 1
    valor_unitario: float
    paciente_id: Optional[int] = None
    historia_id: Optional[int] = None


class GlosaRead(BaseModel):
    id: int
    factura_id: int
    item_glosado: str
    motivo: str
    valor_glosado: float
    fecha_glosa: date
    respuesta: Optional[str] = None
    fecha_respuesta: Optional[date] = None
    estado: str
    numero_factura: Optional[str] = None

    class Config:
        from_attributes = True


class GlosaCreate(BaseModel):
    item_glosado: str
    motivo: str
    valor_glosado: float


class GlosaResponder(BaseModel):
    respuesta: str
    aceptar: bool  # True = acepta la glosa, False = rechaza (contesta)


class FacturaRead(BaseModel):
    id: int
    numero_factura: str
    entidad_id: int
    contrato_id: Optional[int] = None
    fecha_factura: date
    fecha_radicado: Optional[date] = None
    num_radicado: Optional[str] = None
    periodo_inicio: date
    periodo_fin: date
    valor_bruto: float
    descuentos: float
    valor_neto: float
    estado: str
    glosada: bool
    observaciones: Optional[str] = None
    detalles: List[DetalleRead] = []
    glosas: List[GlosaRead] = []
    # Enriched
    entidad_nombre: Optional[str] = None
    contrato_nombre: Optional[str] = None
    total_detalles: int = 0
    glosas_pendientes: int = 0

    class Config:
        from_attributes = True


class FacturaCreate(BaseModel):
    entidad_id: int
    contrato_id: Optional[int] = None
    periodo_inicio: date
    periodo_fin: date
    descuentos: float = 0
    observaciones: Optional[str] = None
    detalles: List[DetalleCreate] = []


class FacturaUpdate(BaseModel):
    contrato_id: Optional[int] = None
    periodo_inicio: Optional[date] = None
    periodo_fin: Optional[date] = None
    descuentos: Optional[float] = None
    observaciones: Optional[str] = None


class RadicarFactura(BaseModel):
    num_radicado: str
    fecha_radicado: Optional[date] = None


class CambiarEstado(BaseModel):
    estado: str
    observacion: Optional[str] = None


class StatsFacturacion(BaseModel):
    total_facturas: int
    borradores: int
    radicadas: int
    aceptadas: int
    pagadas: int
    glosadas: int
    valor_total_radicado: float
    valor_total_pagado: float
    glosas_pendientes: int
    valor_glosado_pendiente: float
