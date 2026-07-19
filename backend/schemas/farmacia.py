from typing import Optional
from datetime import date, datetime
from pydantic import BaseModel


class ItemInventarioRead(BaseModel):
    id: int
    nombre: str
    tipo: str
    categoria: Optional[str] = None
    presentacion: Optional[str] = None
    lote: str
    fecha_vencimiento: Optional[date] = None
    registro_invima: Optional[str] = None
    cantidad_actual: int
    cantidad_minima: int
    unidad_medida: str
    ubicacion: str
    costo_unitario: float
    valor_total: float
    requiere_refrigeracion: bool
    controlado: bool
    # Computed
    alerta_stock: bool = False
    alerta_vencimiento: bool = False

    class Config:
        from_attributes = True


class ItemInventarioCreate(BaseModel):
    nombre: str
    tipo: str = "MEDICAMENTO"
    categoria: Optional[str] = None
    presentacion: Optional[str] = None
    lote: str = ""
    fecha_vencimiento: Optional[date] = None
    registro_invima: Optional[str] = None
    cantidad_actual: int = 0
    cantidad_minima: int = 10
    unidad_medida: str = "UNIDAD"
    ubicacion: str = "FARMACIA"
    costo_unitario: float = 0
    requiere_refrigeracion: bool = False
    controlado: bool = False


class ItemInventarioUpdate(BaseModel):
    nombre: Optional[str] = None
    tipo: Optional[str] = None
    categoria: Optional[str] = None
    presentacion: Optional[str] = None
    lote: Optional[str] = None
    fecha_vencimiento: Optional[date] = None
    registro_invima: Optional[str] = None
    cantidad_minima: Optional[int] = None
    unidad_medida: Optional[str] = None
    ubicacion: Optional[str] = None
    costo_unitario: Optional[float] = None
    requiere_refrigeracion: Optional[bool] = None
    controlado: Optional[bool] = None


class EntradaStockCreate(BaseModel):
    cantidad: int
    motivo: str = "Compra"
    lote: Optional[str] = None
    fecha_vencimiento: Optional[date] = None
    costo_unitario: Optional[float] = None


class FormulaPendienteRead(BaseModel):
    id: int
    historia_id: int
    medicamento: str
    codigo_cum: Optional[str] = None
    dosis: str
    cantidad: int
    unidad: Optional[str] = None
    observaciones: Optional[str] = None
    fecha_formula: datetime
    dispensada: bool
    paciente_nombre: Optional[str] = None
    paciente_id: Optional[int] = None
    medico_nombre: Optional[str] = None

    class Config:
        from_attributes = True


class DispensarCreate(BaseModel):
    formula_id: int
    item_inventario_id: int
    cantidad_dispensada: int
    lote_dispensado: str = ""
    observaciones: Optional[str] = None


class DispensacionRead(BaseModel):
    id: int
    formula_id: int
    item_inventario_id: int
    paciente_id: int
    regente_farmacia_id: int
    fecha_dispensacion: datetime
    cantidad_dispensada: int
    lote_dispensado: str
    observaciones: Optional[str] = None
    paciente_nombre: Optional[str] = None
    medicamento_nombre: Optional[str] = None
    item_nombre: Optional[str] = None
    regente_nombre: Optional[str] = None

    class Config:
        from_attributes = True


class MovimientoRead(BaseModel):
    id: int
    item_inventario_id: int
    usuario_id: int
    fecha: datetime
    tipo_movimiento: str
    cantidad: int
    motivo: str
    paciente_id: Optional[int] = None
    formula_id: Optional[int] = None
    usuario_nombre: Optional[str] = None
    item_nombre: Optional[str] = None

    class Config:
        from_attributes = True


class StatsInventario(BaseModel):
    total_items: int
    stock_bajo: int
    por_vencer: int
    vencidos: int
    formulas_pendientes: int
    valor_total_inventario: float
