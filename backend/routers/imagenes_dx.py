import os
import uuid as _uuid
from datetime import datetime
from typing import Optional
from fastapi import APIRouter, Depends, HTTPException, Query, UploadFile, File
from sqlmodel import Session, select
from models import (
    Orden, ResultadoImagen, Usuario, Paciente,
    TipoOrden, EstadoOrden, RolUsuario,
)
from schemas.imagenes import (
    OrdenImagenRead, EstadoOrdenUpdate,
    InformeCreate, InformeUpdate, InformeRead,
)
from models import ModuloSistema
from core.deps import get_db, get_current_active_user, requiere_modulo
from core.config import UPLOAD_DIR

router = APIRouter(prefix="/imagenes", tags=["imagenes"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.IMAGENES))],
)


def _enrich_orden(orden: Orden, db: Session) -> OrdenImagenRead:
    data = OrdenImagenRead.model_validate(orden)
    if orden.paciente_id:
        p = db.get(Paciente, orden.paciente_id)  # aislamiento: ya autorizado por el padre
        if p:
            data.paciente_nombre = p.nombre_completo.strip()
            data.paciente_identificacion = p.identificacion
    if orden.medico_ordena_id:
        m = db.get(Usuario, orden.medico_ordena_id)  # aislamiento: ya autorizado por el padre
        if m:
            data.medico_nombre = m.nombre_completo
    informe = db.exec(
        select(ResultadoImagen).where(ResultadoImagen.orden_id == orden.id)
    ).first()
    data.tiene_resultado = informe is not None
    return data


def _enrich_informe(r: ResultadoImagen, db: Session) -> InformeRead:
    data = InformeRead.model_validate(r)
    rad = db.get(Usuario, r.radiologo_id)  # aislamiento: ya autorizado por el padre
    if rad:
        data.radiologo_nombre = rad.nombre_completo
    return data


# ══════════════════════════════════════════════════════════════════
# ÓRDENES DE IMAGENOLOGÍA
# ══════════════════════════════════════════════════════════════════

@router.get("/ordenes", response_model=list[OrdenImagenRead])
def list_ordenes(
    estado: Optional[str] = Query(None),
    paciente_id: Optional[int] = Query(None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(Orden).where(Orden.tipo == TipoOrden.IMAGENOLOGIA)
    if estado:
        query = query.where(Orden.estado == estado)
    if paciente_id:
        query = query.where(Orden.paciente_id == paciente_id)
    if current_user.rol != RolUsuario.SUPERADMIN and current_user.institucion_id:
        query = query.join(Paciente, Orden.paciente_id == Paciente.id, isouter=True).where(
            (Paciente.institucion_id == current_user.institucion_id) | (Orden.paciente_id == None)
        )
    query = query.order_by(Orden.fecha_orden.desc())
    return [_enrich_orden(o, db) for o in db.exec(query).all()]


@router.get("/ordenes/{orden_id}", response_model=OrdenImagenRead)
def get_orden(
    orden_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    orden = obtener(db, Orden, orden_id, current_user, que="orden de imagen")
    if orden.tipo != TipoOrden.IMAGENOLOGIA:
        raise HTTPException(status_code=404, detail="Orden de imagen no encontrada")
    return _enrich_orden(orden, db)


@router.put("/ordenes/{orden_id}/estado", response_model=OrdenImagenRead)
def update_estado(
    orden_id: int,
    payload: EstadoOrdenUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    orden = obtener(db, Orden, orden_id, current_user, que="orden de imagen")
    if orden.tipo != TipoOrden.IMAGENOLOGIA:
        raise HTTPException(status_code=404, detail="Orden no encontrada")
    try:
        orden.estado = EstadoOrden(payload.estado)
    except ValueError:
        raise HTTPException(status_code=400, detail=f"Estado inválido: {payload.estado}")
    db.add(orden)
    db.commit()
    db.refresh(orden)
    return _enrich_orden(orden, db)


# ══════════════════════════════════════════════════════════════════
# INFORMES RADIOLÓGICOS
# ══════════════════════════════════════════════════════════════════

@router.get("/ordenes/{orden_id}/informe", response_model=InformeRead)
def get_informe(
    orden_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    informe = db.exec(
        select(ResultadoImagen).where(ResultadoImagen.orden_id == orden_id)
    ).first()
    if not informe:
        raise HTTPException(status_code=404, detail="Informe no encontrado")
    return _enrich_informe(informe, db)


@router.post("/ordenes/{orden_id}/informe", response_model=InformeRead, status_code=201)
def create_informe(
    orden_id: int,
    payload: InformeCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    orden = obtener(db, Orden, orden_id, current_user, que="orden de imagen")
    if orden.tipo != TipoOrden.IMAGENOLOGIA:
        raise HTTPException(status_code=404, detail="Orden no encontrada")
    if db.exec(select(ResultadoImagen).where(ResultadoImagen.orden_id == orden_id)).first():
        raise HTTPException(status_code=400, detail="Ya existe un informe. Use PUT para actualizarlo.")
    informe = ResultadoImagen(
        orden_id=orden_id,
        paciente_id=orden.paciente_id,
        radiologo_id=current_user.id,
        **payload.model_dump(),
    )
    db.add(informe)
    if orden.estado == EstadoOrden.PENDIENTE:
        orden.estado = EstadoOrden.EN_PROCESO
        db.add(orden)
    db.commit()
    db.refresh(informe)
    return _enrich_informe(informe, db)


@router.put("/ordenes/{orden_id}/informe", response_model=InformeRead)
def update_informe(
    orden_id: int,
    payload: InformeUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    informe = db.exec(
        select(ResultadoImagen).where(ResultadoImagen.orden_id == orden_id)
    ).first()
    if not informe:
        raise HTTPException(status_code=404, detail="Informe no encontrado")
    if informe.firmado:
        raise HTTPException(status_code=400, detail="No se puede modificar un informe firmado")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(informe, field, value)
    db.add(informe)
    db.commit()
    db.refresh(informe)
    return _enrich_informe(informe, db)


@router.post("/ordenes/{orden_id}/informe/firmar", response_model=InformeRead)
def firmar_informe(
    orden_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    informe = db.exec(
        select(ResultadoImagen).where(ResultadoImagen.orden_id == orden_id)
    ).first()
    if not informe:
        raise HTTPException(status_code=404, detail="Informe no encontrado")
    informe.firmado = True
    orden = obtener(db, Orden, orden_id, current_user, que="orden de imagen")
    if orden:
        orden.estado = EstadoOrden.COMPLETADA
        db.add(orden)
    db.add(informe)
    db.commit()
    db.refresh(informe)
    return _enrich_informe(informe, db)


@router.post("/ordenes/{orden_id}/informe/imagenes", response_model=InformeRead)
async def upload_imagen_informe(
    orden_id: int,
    file: UploadFile = File(...),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    informe = db.exec(
        select(ResultadoImagen).where(ResultadoImagen.orden_id == orden_id)
    ).first()
    if not informe:
        raise HTTPException(status_code=404, detail="Informe no encontrado")
    if informe.firmado:
        raise HTTPException(status_code=400, detail="No se puede modificar un informe firmado")
    ext = os.path.splitext(file.filename or "")[1]
    filename = f"{_uuid.uuid4()}{ext}"
    save_dir = os.path.join(UPLOAD_DIR, "imagenes_dx")
    os.makedirs(save_dir, exist_ok=True)
    filepath = os.path.join(save_dir, filename)
    content = await file.read()
    with open(filepath, "wb") as f:
        f.write(content)
    paths = list(informe.imagenes_paths or [])
    paths.append(f"/uploads/imagenes_dx/{filename}")
    informe.imagenes_paths = paths
    db.add(informe)
    db.commit()
    db.refresh(informe)
    return _enrich_informe(informe, db)


# ══════════════════════════════════════════════════════════════════
# HISTORIAL POR PACIENTE
# ══════════════════════════════════════════════════════════════════

@router.get("/paciente/{paciente_id}/informes", response_model=list[InformeRead])
def get_informes_paciente(
    paciente_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    informes = db.exec(
        select(ResultadoImagen)
        .where(ResultadoImagen.paciente_id == paciente_id)
        .order_by(ResultadoImagen.fecha_resultado.desc())
    ).all()
    return [_enrich_informe(r, db) for r in informes]
