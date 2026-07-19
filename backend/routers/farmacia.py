from typing import Optional
from datetime import date, timedelta
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from models import (
    ItemInventario, MovimientoInventario, Formula, Historia,
    DispensacionMedicamento, Paciente, Usuario, RolUsuario,
)
from core.deps import get_db, get_current_active_user, filtrar_por_institucion
from schemas.farmacia import (
    ItemInventarioRead, ItemInventarioCreate, ItemInventarioUpdate,
    EntradaStockCreate, FormulaPendienteRead, DispensarCreate,
    DispensacionRead, MovimientoRead, StatsInventario,
)

router = APIRouter(prefix="/farmacia", tags=["farmacia"])

ALERTA_VENCIMIENTO_DIAS = 30


def _enrich_item(item: ItemInventario) -> ItemInventarioRead:
    data = ItemInventarioRead.model_validate(item)
    data.alerta_stock = item.cantidad_actual <= item.cantidad_minima
    if item.fecha_vencimiento:
        hoy = date.today()
        data.alerta_vencimiento = item.fecha_vencimiento <= hoy + timedelta(days=ALERTA_VENCIMIENTO_DIAS)
    return data


# ── Estadísticas ──────────────────────────────────────────────────────────────

@router.get("/stats", response_model=StatsInventario)
def get_stats(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    items = db.exec(select(ItemInventario)).all()
    hoy = date.today()
    limite_venc = hoy + timedelta(days=ALERTA_VENCIMIENTO_DIAS)
    formulas_pend = db.exec(select(Formula).where(Formula.dispensada == False)).all()
    return StatsInventario(
        total_items=len(items),
        stock_bajo=sum(1 for i in items if i.cantidad_actual <= i.cantidad_minima),
        por_vencer=sum(1 for i in items if i.fecha_vencimiento and hoy < i.fecha_vencimiento <= limite_venc),
        vencidos=sum(1 for i in items if i.fecha_vencimiento and i.fecha_vencimiento < hoy),
        formulas_pendientes=len(formulas_pend),
        valor_total_inventario=sum(i.costo_unitario * i.cantidad_actual for i in items),
    )


# ── Inventario ────────────────────────────────────────────────────────────────

@router.get("/inventario", response_model=list[ItemInventarioRead])
def list_inventario(
    q: Optional[str] = None,
    tipo: Optional[str] = None,
    ubicacion: Optional[str] = None,
    solo_alertas: bool = False,
    page: int = Query(1, ge=1),
    size: int = Query(50, ge=1, le=200),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(ItemInventario)
    if q:
        query = query.where(ItemInventario.nombre.ilike(f"%{q}%"))
    if tipo:
        query = query.where(ItemInventario.tipo == tipo)
    if ubicacion:
        query = query.where(ItemInventario.ubicacion == ubicacion)
    if solo_alertas:
        hoy = date.today()
        limite = hoy + timedelta(days=ALERTA_VENCIMIENTO_DIAS)
        from sqlalchemy import or_
        query = query.where(
            or_(
                ItemInventario.cantidad_actual <= ItemInventario.cantidad_minima,
                ItemInventario.fecha_vencimiento <= limite,
            )
        )
    query = query.order_by(ItemInventario.nombre).offset((page - 1) * size).limit(size)
    return [_enrich_item(i) for i in db.exec(query).all()]


@router.get("/inventario/{item_id}", response_model=ItemInventarioRead)
def get_item(
    item_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    item = db.get(ItemInventario, item_id)
    if not item:
        raise HTTPException(status_code=404, detail="Item no encontrado")
    return _enrich_item(item)


@router.post("/inventario", response_model=ItemInventarioRead, status_code=201)
def crear_item(
    payload: ItemInventarioCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    item = ItemInventario(**payload.model_dump())
    item.valor_total = item.costo_unitario * item.cantidad_actual
    db.add(item)
    # Registrar movimiento inicial si hay stock
    if item.cantidad_actual > 0:
        db.flush()
        mov = MovimientoInventario(
            item_inventario_id=item.id,
            usuario_id=current_user.id,
            tipo_movimiento="ENTRADA",
            cantidad=item.cantidad_actual,
            motivo="Stock inicial al crear item",
        )
        db.add(mov)
    db.commit()
    db.refresh(item)
    return _enrich_item(item)


@router.put("/inventario/{item_id}", response_model=ItemInventarioRead)
def actualizar_item(
    item_id: int,
    payload: ItemInventarioUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    item = db.get(ItemInventario, item_id)
    if not item:
        raise HTTPException(status_code=404, detail="Item no encontrado")
    data = payload.model_dump(exclude_unset=True)
    for k, v in data.items():
        setattr(item, k, v)
    item.valor_total = item.costo_unitario * item.cantidad_actual
    db.add(item)
    db.commit()
    db.refresh(item)
    return _enrich_item(item)


@router.delete("/inventario/{item_id}", status_code=204)
def eliminar_item(
    item_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    item = db.get(ItemInventario, item_id)
    if not item:
        raise HTTPException(status_code=404, detail="Item no encontrado")
    if item.cantidad_actual > 0:
        raise HTTPException(
            status_code=400,
            detail="No se puede eliminar un item con stock. Ajuste el inventario primero.",
        )
    db.delete(item)
    db.commit()


@router.post("/inventario/{item_id}/entrada", response_model=ItemInventarioRead)
def entrada_stock(
    item_id: int,
    payload: EntradaStockCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    item = db.get(ItemInventario, item_id)
    if not item:
        raise HTTPException(status_code=404, detail="Item no encontrado")
    if payload.cantidad <= 0:
        raise HTTPException(status_code=400, detail="La cantidad debe ser mayor a 0")
    item.cantidad_actual += payload.cantidad
    if payload.lote:
        item.lote = payload.lote
    if payload.fecha_vencimiento:
        item.fecha_vencimiento = payload.fecha_vencimiento
    if payload.costo_unitario is not None:
        item.costo_unitario = payload.costo_unitario
    item.valor_total = item.costo_unitario * item.cantidad_actual
    mov = MovimientoInventario(
        item_inventario_id=item_id,
        usuario_id=current_user.id,
        tipo_movimiento="ENTRADA",
        cantidad=payload.cantidad,
        motivo=payload.motivo,
    )
    db.add(item)
    db.add(mov)
    db.commit()
    db.refresh(item)
    return _enrich_item(item)


# ── Fórmulas pendientes ───────────────────────────────────────────────────────

@router.get("/formulas-pendientes", response_model=list[FormulaPendienteRead])
def list_formulas_pendientes(
    q: Optional[str] = None,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(Formula).where(Formula.dispensada == False).order_by(Formula.fecha_formula.desc())
    formulas = db.exec(query).all()
    result = []
    for f in formulas:
        item = FormulaPendienteRead.model_validate(f)
        historia = db.get(Historia, f.historia_id)
        if historia:
            item.paciente_id = historia.paciente_id
            p = db.get(Paciente, historia.paciente_id)
            if p:
                item.paciente_nombre = p.nombre_completo.strip()
                if q and q.lower() not in item.paciente_nombre.lower() and q.lower() not in f.medicamento.lower():
                    continue
            m = db.get(Usuario, historia.medico_id)
            if m:
                item.medico_nombre = m.nombre_completo
        result.append(item)
    return result


# ── Dispensación ──────────────────────────────────────────────────────────────

@router.post("/dispensar", response_model=DispensacionRead, status_code=201)
def dispensar(
    payload: DispensarCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    formula = db.get(Formula, payload.formula_id)
    if not formula:
        raise HTTPException(status_code=404, detail="Fórmula no encontrada")
    if formula.dispensada:
        raise HTTPException(status_code=400, detail="Esta fórmula ya fue dispensada")

    item = db.get(ItemInventario, payload.item_inventario_id)
    if not item:
        raise HTTPException(status_code=404, detail="Item de inventario no encontrado")
    if item.cantidad_actual < payload.cantidad_dispensada:
        raise HTTPException(
            status_code=400,
            detail=f"Stock insuficiente. Disponible: {item.cantidad_actual} {item.unidad_medida}",
        )

    historia = db.get(Historia, formula.historia_id)
    if not historia:
        raise HTTPException(status_code=404, detail="Historia clínica no encontrada")

    lote = payload.lote_dispensado or item.lote or ""
    disp = DispensacionMedicamento(
        formula_id=payload.formula_id,
        item_inventario_id=payload.item_inventario_id,
        paciente_id=historia.paciente_id,
        regente_farmacia_id=current_user.id,
        cantidad_dispensada=payload.cantidad_dispensada,
        lote_dispensado=lote,
        observaciones=payload.observaciones,
    )

    item.cantidad_actual -= payload.cantidad_dispensada
    item.valor_total = item.costo_unitario * item.cantidad_actual
    formula.dispensada = True

    mov = MovimientoInventario(
        item_inventario_id=payload.item_inventario_id,
        usuario_id=current_user.id,
        tipo_movimiento="SALIDA",
        cantidad=payload.cantidad_dispensada,
        motivo=f"Dispensación — {formula.medicamento}",
        paciente_id=historia.paciente_id,
        formula_id=payload.formula_id,
    )

    db.add(disp)
    db.add(item)
    db.add(formula)
    db.add(mov)
    db.commit()
    db.refresh(disp)
    return _enrich_dispensacion(disp, db)


@router.get("/dispensaciones", response_model=list[DispensacionRead])
def list_dispensaciones(
    page: int = Query(1, ge=1),
    size: int = Query(30, ge=1, le=100),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = (
        select(DispensacionMedicamento)
        .order_by(DispensacionMedicamento.fecha_dispensacion.desc())
        .offset((page - 1) * size)
        .limit(size)
    )
    return [_enrich_dispensacion(d, db) for d in db.exec(query).all()]


def _enrich_dispensacion(d: DispensacionMedicamento, db: Session) -> DispensacionRead:
    data = DispensacionRead.model_validate(d)
    p = db.get(Paciente, d.paciente_id)
    if p:
        data.paciente_nombre = p.nombre_completo.strip()
    item = db.get(ItemInventario, d.item_inventario_id)
    if item:
        data.item_nombre = item.nombre
    f = db.get(Formula, d.formula_id)
    if f:
        data.medicamento_nombre = f.medicamento
    r = db.get(Usuario, d.regente_farmacia_id)
    if r:
        data.regente_nombre = r.nombre_completo
    return data


# ── Movimientos ───────────────────────────────────────────────────────────────

@router.get("/movimientos", response_model=list[MovimientoRead])
def list_movimientos(
    item_id: Optional[int] = None,
    tipo: Optional[str] = None,
    page: int = Query(1, ge=1),
    size: int = Query(50, ge=1, le=200),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(MovimientoInventario)
    if item_id:
        query = query.where(MovimientoInventario.item_inventario_id == item_id)
    if tipo:
        query = query.where(MovimientoInventario.tipo_movimiento == tipo)
    query = query.order_by(MovimientoInventario.fecha.desc()).offset((page - 1) * size).limit(size)
    movs = db.exec(query).all()
    result = []
    for m in movs:
        data = MovimientoRead.model_validate(m)
        u = db.get(Usuario, m.usuario_id)
        if u:
            data.usuario_nombre = u.nombre_completo
        i = db.get(ItemInventario, m.item_inventario_id)
        if i:
            data.item_nombre = i.nombre
        result.append(data)
    return result


# ── Alertas (compat) ──────────────────────────────────────────────────────────

@router.get("/alertas", response_model=list[ItemInventarioRead])
def alertas_inventario(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    hoy = date.today()
    limite = hoy + timedelta(days=ALERTA_VENCIMIENTO_DIAS)
    from sqlalchemy import or_
    query = select(ItemInventario).where(
        or_(
            ItemInventario.cantidad_actual <= ItemInventario.cantidad_minima,
            ItemInventario.fecha_vencimiento <= limite,
        )
    )
    return [_enrich_item(i) for i in db.exec(query).all()]
