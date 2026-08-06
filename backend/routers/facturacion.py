from typing import Optional
from datetime import date
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from models import Factura, DetalleFactura, Glosa, Contrato, Entidad, Historia, Paciente, Usuario, ModuloSistema
from core.deps import get_db, get_current_active_user, filtrar_por_institucion, requiere_modulo
from models import RolUsuario
from core.aislamiento import obtener
from schemas.facturacion import (
    FacturaRead, FacturaCreate, FacturaUpdate, DetalleRead, DetalleCreate,
    GlosaRead, GlosaCreate, GlosaResponder, ContratoRead, ContratoCreate,
    ContratoUpdate, EntidadRead, RadicarFactura, StatsFacturacion,
)

router = APIRouter(prefix="/facturacion", tags=["facturacion"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.FACTURACION))],
)

# Transiciones de estado permitidas
_TRANSICIONES = {
    "BORRADOR": {"RADICADA"},
    "RADICADA": {"ACEPTADA", "GLOSADA", "ANULADA"},
    "ACEPTADA": {"PAGADA", "ANULADA"},
    "GLOSADA": {"RADICADA", "ANULADA"},
    "PAGADA": set(),
    "ANULADA": set(),
}


def _exigir_propios(db: Session, user: Usuario, det) -> None:
    """El paciente y la historia que nombra una LÍNEA tienen que ser de esta institución.

    Sin esto, un facturador de A creaba una factura PROPIA con una línea que
    nombraba al `paciente_id` de un paciente de B — la clave foránea es global y
    la acepta— y `_enrich_factura` devolvía su NOMBRE COMPLETO. Enumerando
    identificadores se sacaba el censo de pacientes del vecino, y «ser paciente
    de X» es dato de salud: es el mismo argumento con el que `core/aislamiento.py`
    justifica responder 404 y nunca 403.

    El comentario que había —«# aislamiento: ya autorizado por el padre»— era
    falso justo aquí: el padre autorizado es la FACTURA, no el paciente que el
    cliente eligió en el cuerpo de la petición.

    Se usa `obtener`, que ya es el camino seguro del proyecto y responde 404 sin
    distinguir «no existe» de «es de otro». La comprobación va en UNA función que
    comparten el alta y el añadido de líneas: dos copias es lo que ya dejó dos
    pantallas muertas con `orden.estado`.
    """
    if getattr(det, "paciente_id", None) is not None:
        obtener(db, Paciente, det.paciente_id, user, que="paciente")
    if getattr(det, "historia_id", None) is not None:
        obtener(db, Historia, det.historia_id, user, que="historia")


def _exigir_contrato_propio(db: Session, user: Usuario, contrato_id) -> None:
    """El contrato que nombra una factura tiene que ser de esta institución.

    Esta fuga la CREÓ el cambio del 2026-08-05: hasta entonces `Contrato` estaba
    declarado en `COMPARTIDOS`, así que leer su nombre era legítimo por
    declaración escrita. Al darle dueño —porque un contrato es el acuerdo entre
    UN prestador y UNA entidad con SUS tarifas— se cerró `/contratos` y
    `obtener`, y quedó abierto este camino: `_enrich_factura` hacía
    `db.get(Contrato, f.contrato_id)` y devolvía `contrato_nombre`, con el id
    puesto por el cliente. Además dejaba una factura de A apuntando a un contrato
    de B, que es una referencia cruzada entre inquilinos que persiste.
    """
    if contrato_id is not None:
        obtener(db, Contrato, contrato_id, user, que="contrato")


def _de_mi_institucion(user: Usuario):
    """La condición del JOIN que acota una glosa por la institución de su factura.

    Una glosa no lleva institución propia y no debe llevarla: cuelga de una
    factura con `factura_id` obligatorio, así que su dueño es demostrable en un
    salto. Darle columna crearía la posibilidad de que dijera una institución y
    su factura otra — dos verdades para un hecho.
    """
    if user.rol == RolUsuario.SUPERADMIN:
        return True
    return Factura.institucion_id == user.institucion_id


def _gen_numero_factura(db: Session, institucion_id: Optional[int]) -> str:
    """El siguiente `FAC-{año}-{n}` DE ESTA INSTITUCIÓN.

    Hasta el 2026-08-05 la secuencia era global —el `WHERE` no llevaba
    institución— y eso hacía tres cosas malas a la vez con dos prestadores en la
    misma base: los números saltaban sin explicación (el 00007 después del 00004
    porque los tres de en medio son del vecino), **el propio número revelaba
    cuántas facturas había emitido el otro**, y dos prestadores no podían
    numerar en paralelo sin pisarse.
    """
    anio = date.today().year
    prefix = f"FAC-{anio}-"
    last = db.exec(
        select(Factura)
        .where(Factura.numero_factura.like(f"{prefix}%"))
        .where(Factura.institucion_id == institucion_id)
        .order_by(Factura.numero_factura.desc())
    ).first()
    if last:
        try:
            seq = int(last.numero_factura.split("-")[-1]) + 1
        except ValueError:
            seq = 1
    else:
        seq = 1
    return f"{prefix}{seq:05d}"


def _enrich_factura(f: Factura, db: Session, include_detalles: bool = True) -> FacturaRead:
    data = FacturaRead.model_validate(f)
    e = db.get(Entidad, f.entidad_id)  # aislamiento: ya autorizado por el padre
    if e:
        data.entidad_nombre = e.nombre
    if f.contrato_id:
        c = db.get(Contrato, f.contrato_id)  # aislamiento: el contrato se valida al ESCRIBIR (_exigir_contrato_propio), así que el id guardado ya es de esta institución
        if c:
            data.contrato_nombre = c.nombre
    if include_detalles:
        detalles = db.exec(select(DetalleFactura).where(DetalleFactura.factura_id == f.id)).all()
        enriched_det = []
        for d in detalles:
            dr = DetalleRead.model_validate(d)
            if d.paciente_id:
                p = db.get(Paciente, d.paciente_id)  # aislamiento: el paciente se valida al ESCRIBIR la línea (_exigir_propios), no basta con que la factura esté autorizada
                if p:
                    dr.paciente_nombre = p.nombre_completo.strip()
            enriched_det.append(dr)
        data.detalles = enriched_det
        data.total_detalles = len(detalles)
        glosas = db.exec(select(Glosa).where(Glosa.factura_id == f.id)).all()
        data.glosas = [GlosaRead.model_validate(g) for g in glosas]
        data.glosas_pendientes = sum(1 for g in glosas if g.estado == "PENDIENTE")
    return data


def _recalcular_totales(factura_id: int, descuentos: float, db: Session) -> None:
    detalles = db.exec(select(DetalleFactura).where(DetalleFactura.factura_id == factura_id)).all()
    bruto = sum(d.valor_total for d in detalles)
    factura = db.get(Factura, factura_id)  # aislamiento: helper interno, la factura ya pasó por obtener() en el endpoint
    if factura:
        factura.valor_bruto = bruto
        factura.valor_neto = max(0, bruto - descuentos)
        db.add(factura)


# ── Stats ─────────────────────────────────────────────────────────────────────

@router.get("/stats", response_model=StatsFacturacion)
def get_stats(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(Factura)
    query = filtrar_por_institucion(query, Factura, current_user)
    facturas = db.exec(query).all()
    # Las glosas se acotan POR SU FACTURA. Esta consulta no llevaba filtro
    # NINGUNO —ni siquiera pasaba por `filtrar_por_institucion`—, así que el
    # panel de un prestador contaba y SUMABA EN DINERO las glosas de todos los
    # demás. Es la fuga que menos se veía, porque un contador no parece un dato.
    glosas = db.exec(
        select(Glosa)
        .join(Factura, Glosa.factura_id == Factura.id)
        .where(Glosa.estado == "PENDIENTE")
        .where(_de_mi_institucion(current_user))
    ).all()
    return StatsFacturacion(
        total_facturas=len(facturas),
        borradores=sum(1 for f in facturas if f.estado == "BORRADOR"),
        radicadas=sum(1 for f in facturas if f.estado == "RADICADA"),
        aceptadas=sum(1 for f in facturas if f.estado == "ACEPTADA"),
        pagadas=sum(1 for f in facturas if f.estado == "PAGADA"),
        glosadas=sum(1 for f in facturas if f.estado == "GLOSADA"),
        valor_total_radicado=sum(
            f.valor_neto for f in facturas if f.estado in ("RADICADA", "ACEPTADA", "GLOSADA")
        ),
        valor_total_pagado=sum(f.valor_neto for f in facturas if f.estado == "PAGADA"),
        glosas_pendientes=len(glosas),
        valor_glosado_pendiente=sum(g.valor_glosado for g in glosas),
    )


# ── Facturas ──────────────────────────────────────────────────────────────────

@router.get("/facturas", response_model=list[FacturaRead])
def list_facturas(
    estado: Optional[str] = None,
    entidad_id: Optional[int] = None,
    q: Optional[str] = None,
    page: int = Query(1, ge=1),
    size: int = Query(30, ge=1, le=100),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(Factura)
    query = filtrar_por_institucion(query, Factura, current_user)
    if estado:
        query = query.where(Factura.estado == estado)
    if entidad_id:
        query = query.where(Factura.entidad_id == entidad_id)
    if q:
        query = query.where(Factura.numero_factura.ilike(f"%{q}%"))
    query = query.order_by(Factura.fecha_factura.desc()).offset((page - 1) * size).limit(size)
    return [_enrich_factura(f, db, include_detalles=False) for f in db.exec(query).all()]


@router.get("/facturas/{factura_id}", response_model=FacturaRead)
def get_factura(
    factura_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    f = obtener(db, Factura, factura_id, current_user, que="factura")
    return _enrich_factura(f, db)


@router.post("/facturas", response_model=FacturaRead, status_code=201)
def crear_factura(
    payload: FacturaCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    if not db.get(Entidad, payload.entidad_id):  # aislamiento: Entidad es catálogo nacional, declarado en COMPARTIDOS
        raise HTTPException(status_code=404, detail="Entidad no encontrada")

    _exigir_contrato_propio(db, current_user, payload.contrato_id)
    for det in payload.detalles:
        _exigir_propios(db, current_user, det)

    numero = _gen_numero_factura(db, current_user.institucion_id)
    factura = Factura(
        # La factura NACE con dueño. Sin esto la fila queda en NULL y no la ve
        # nadie —ni quien la acaba de crear—, que es fallar cerrado pero inútil.
        institucion_id=current_user.institucion_id,
        numero_factura=numero,
        entidad_id=payload.entidad_id,
        contrato_id=payload.contrato_id,
        periodo_inicio=payload.periodo_inicio,
        periodo_fin=payload.periodo_fin,
        descuentos=payload.descuentos,
        observaciones=payload.observaciones,
        valor_bruto=0,
        valor_neto=0,
    )
    db.add(factura)
    db.flush()

    for det in payload.detalles:
        d = DetalleFactura(
            factura_id=factura.id,
            servicio=det.servicio,
            codigo_cups=det.codigo_cups,
            cantidad=det.cantidad,
            valor_unitario=det.valor_unitario,
            valor_total=det.valor_unitario * det.cantidad,
            paciente_id=det.paciente_id,
            historia_id=det.historia_id,
        )
        db.add(d)

    db.flush()
    _recalcular_totales(factura.id, payload.descuentos, db)
    db.commit()
    db.refresh(factura)
    return _enrich_factura(factura, db)


@router.put("/facturas/{factura_id}", response_model=FacturaRead)
def actualizar_factura(
    factura_id: int,
    payload: FacturaUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    f = obtener(db, Factura, factura_id, current_user, que="factura")
    if f.estado != "BORRADOR":
        raise HTTPException(status_code=400, detail="Solo se pueden editar facturas en estado BORRADOR")
    data = payload.model_dump(exclude_unset=True)
    if "contrato_id" in data:
        _exigir_contrato_propio(db, current_user, data["contrato_id"])
    for k, v in data.items():
        setattr(f, k, v)
    _recalcular_totales(f.id, f.descuentos, db)
    db.add(f)
    db.commit()
    db.refresh(f)
    return _enrich_factura(f, db)


@router.delete("/facturas/{factura_id}", status_code=204)
def eliminar_factura(
    factura_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    f = obtener(db, Factura, factura_id, current_user, que="factura")
    if f.estado != "BORRADOR":
        raise HTTPException(status_code=400, detail="Solo se pueden eliminar facturas en estado BORRADOR")
    for det in db.exec(select(DetalleFactura).where(DetalleFactura.factura_id == factura_id)).all():
        db.delete(det)
    db.delete(f)
    db.commit()


@router.put("/facturas/{factura_id}/radicar", response_model=FacturaRead)
def radicar_factura(
    factura_id: int,
    payload: RadicarFactura,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    f = obtener(db, Factura, factura_id, current_user, que="factura")
    if f.estado != "BORRADOR":
        raise HTTPException(status_code=400, detail="Solo se pueden radicar facturas en estado BORRADOR")
    if f.valor_neto <= 0:
        raise HTTPException(status_code=400, detail="No se puede radicar una factura sin valor")
    f.estado = "RADICADA"
    f.num_radicado = payload.num_radicado
    f.fecha_radicado = payload.fecha_radicado or date.today()
    db.add(f)
    db.commit()
    db.refresh(f)
    return _enrich_factura(f, db)


@router.put("/facturas/{factura_id}/estado", response_model=FacturaRead)
def cambiar_estado(
    factura_id: int,
    nuevo_estado: str = Query(...),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    f = obtener(db, Factura, factura_id, current_user, que="factura")
    permitidos = _TRANSICIONES.get(f.estado, set())
    if nuevo_estado not in permitidos:
        raise HTTPException(
            status_code=400,
            detail=f"Transición no permitida: {f.estado} → {nuevo_estado}. Permitidos: {', '.join(permitidos) or 'ninguno'}",
        )
    f.estado = nuevo_estado
    if nuevo_estado == "GLOSADA":
        f.glosada = True
    db.add(f)
    db.commit()
    db.refresh(f)
    return _enrich_factura(f, db)


# ── Detalles ──────────────────────────────────────────────────────────────────

@router.post("/facturas/{factura_id}/detalles", response_model=FacturaRead, status_code=201)
def agregar_detalle(
    factura_id: int,
    payload: DetalleCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    f = obtener(db, Factura, factura_id, current_user, que="factura")
    if f.estado != "BORRADOR":
        raise HTTPException(status_code=400, detail="Solo se pueden agregar detalles a facturas en BORRADOR")
    _exigir_propios(db, current_user, payload)
    det = DetalleFactura(
        factura_id=factura_id,
        servicio=payload.servicio,
        codigo_cups=payload.codigo_cups,
        cantidad=payload.cantidad,
        valor_unitario=payload.valor_unitario,
        valor_total=payload.valor_unitario * payload.cantidad,
        paciente_id=payload.paciente_id,
        historia_id=payload.historia_id,
    )
    db.add(det)
    db.flush()
    _recalcular_totales(factura_id, f.descuentos, db)
    db.commit()
    db.refresh(f)
    return _enrich_factura(f, db)


@router.delete("/facturas/{factura_id}/detalles/{detalle_id}", response_model=FacturaRead)
def eliminar_detalle(
    factura_id: int,
    detalle_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    f = obtener(db, Factura, factura_id, current_user, que="factura")
    if f.estado != "BORRADOR":
        raise HTTPException(status_code=400, detail="Solo se pueden modificar facturas en BORRADOR")
    det = db.get(DetalleFactura, detalle_id)  # aislamiento: se exige abajo que pertenezca a la factura ya autorizada
    if not det or det.factura_id != factura_id:
        raise HTTPException(status_code=404, detail="Detalle no encontrado")
    db.delete(det)
    db.flush()
    _recalcular_totales(factura_id, f.descuentos, db)
    db.commit()
    db.refresh(f)
    return _enrich_factura(f, db)


# ── Glosas ────────────────────────────────────────────────────────────────────

@router.get("/glosas", response_model=list[GlosaRead])
def list_glosas(
    estado: Optional[str] = None,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    # `select(Glosa)` a secas devolvía las glosas de TODAS las instituciones, y
    # encima el bucle de abajo las enriquecía con el `numero_factura` ajeno.
    query = (
        select(Glosa)
        .join(Factura, Glosa.factura_id == Factura.id)
        .where(_de_mi_institucion(current_user))
    )
    if estado:
        query = query.where(Glosa.estado == estado)
    query = query.order_by(Glosa.fecha_glosa.desc())
    glosas = db.exec(query).all()
    result = []
    for g in glosas:
        gr = GlosaRead.model_validate(g)
        f = db.get(Factura, g.factura_id)  # aislamiento: ya autorizado por el padre
        if f:
            gr.numero_factura = f.numero_factura
        result.append(gr)
    return result


@router.post("/facturas/{factura_id}/glosas", response_model=GlosaRead, status_code=201)
def agregar_glosa(
    factura_id: int,
    payload: GlosaCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    f = obtener(db, Factura, factura_id, current_user, que="factura")
    g = Glosa(
        factura_id=factura_id,
        item_glosado=payload.item_glosado,
        motivo=payload.motivo,
        valor_glosado=payload.valor_glosado,
    )
    f.glosada = True
    if f.estado == "RADICADA":
        f.estado = "GLOSADA"
    db.add(g)
    db.add(f)
    db.commit()
    db.refresh(g)
    gr = GlosaRead.model_validate(g)
    gr.numero_factura = f.numero_factura
    return gr


@router.put("/glosas/{glosa_id}/responder", response_model=GlosaRead)
def responder_glosa(
    glosa_id: int,
    payload: GlosaResponder,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    g = obtener(db, Glosa, glosa_id, current_user, que="glosa")
    if g.estado != "PENDIENTE":
        raise HTTPException(status_code=400, detail="Esta glosa ya fue respondida")
    g.respuesta = payload.respuesta
    g.fecha_respuesta = date.today()
    g.estado = "ACEPTADA" if payload.aceptar else "RECHAZADA"
    db.add(g)
    db.commit()
    db.refresh(g)
    gr = GlosaRead.model_validate(g)
    f = db.get(Factura, g.factura_id)  # aislamiento: ya autorizado por el padre
    if f:
        gr.numero_factura = f.numero_factura
    return gr


# ── Contratos ─────────────────────────────────────────────────────────────────

@router.get("/contratos", response_model=list[ContratoRead])
def list_contratos(
    activo: Optional[bool] = None,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = filtrar_por_institucion(select(Contrato), Contrato, current_user)
    if activo is not None:
        query = query.where(Contrato.activo == activo)
    query = query.order_by(Contrato.nombre)
    contratos = db.exec(query).all()
    result = []
    for c in contratos:
        cr = ContratoRead.model_validate(c)
        e = db.get(Entidad, c.entidad_id)  # aislamiento: ya autorizado por el padre
        if e:
            cr.entidad_nombre = e.nombre
        result.append(cr)
    return result


@router.post("/contratos", response_model=ContratoRead, status_code=201)
def crear_contrato(
    payload: ContratoCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    # La unicidad se mira DENTRO de la institución. Era global, así que el
    # segundo prestador que numerara sus contratos «001» recibía un 400 — y con
    # él, la confirmación de que el vecino ya tiene ese número.
    existing = db.exec(
        select(Contrato)
        .where(Contrato.numero_contrato == payload.numero_contrato)
        .where(Contrato.institucion_id == current_user.institucion_id)
    ).first()
    if existing:
        raise HTTPException(status_code=400, detail="Ya existe un contrato con ese número")
    c = Contrato(**payload.model_dump(), institucion_id=current_user.institucion_id)
    db.add(c)
    db.commit()
    db.refresh(c)
    cr = ContratoRead.model_validate(c)
    e = db.get(Entidad, c.entidad_id)  # aislamiento: ya autorizado por el padre
    if e:
        cr.entidad_nombre = e.nombre
    return cr


@router.put("/contratos/{contrato_id}", response_model=ContratoRead)
def actualizar_contrato(
    contrato_id: int,
    payload: ContratoUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    c = obtener(db, Contrato, contrato_id, current_user, que="contrato")
    for k, v in payload.model_dump(exclude_unset=True).items():
        setattr(c, k, v)
    db.add(c)
    db.commit()
    db.refresh(c)
    cr = ContratoRead.model_validate(c)
    e = db.get(Entidad, c.entidad_id)  # aislamiento: ya autorizado por el padre
    if e:
        cr.entidad_nombre = e.nombre
    return cr


# ── Entidades ─────────────────────────────────────────────────────────────────

@router.get("/entidades", response_model=list[EntidadRead])
def list_entidades(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    return db.exec(select(Entidad).order_by(Entidad.nombre)).all()
