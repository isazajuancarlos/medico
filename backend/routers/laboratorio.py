from datetime import datetime
from typing import Optional
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from models import (
    Orden, ResultadoLaboratorio, PerfilLaboratorio,
    Usuario, Paciente, TipoOrden, EstadoOrden, RolUsuario,
)
from schemas.laboratorio import (
    OrdenLabRead, EstadoOrdenUpdate,
    ResultadoCreate, ResultadoUpdate, ResultadoRead,
    PerfilCreate, PerfilRead,
)
from models import ModuloSistema
from core.deps import get_db, get_current_active_user, filtrar_por_institucion, requiere_modulo
from core.aislamiento import obtener

router = APIRouter(prefix="/laboratorio", tags=["laboratorio"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.LABORATORIO))],
)


def _enrich_orden(orden: Orden, db: Session) -> OrdenLabRead:
    data = OrdenLabRead.model_validate(orden)
    if orden.paciente_id:
        p = db.get(Paciente, orden.paciente_id)  # aislamiento: ya autorizado por el padre
        if p:
            data.paciente_nombre = p.nombre_completo.strip()
            data.paciente_identificacion = p.identificacion
    if orden.medico_ordena_id:
        m = db.get(Usuario, orden.medico_ordena_id)  # aislamiento: ya autorizado por el padre
        if m:
            data.medico_nombre = m.nombre_completo
    resultado = db.exec(
        select(ResultadoLaboratorio).where(ResultadoLaboratorio.orden_id == orden.id)
    ).first()
    data.tiene_resultado = resultado is not None
    return data


def _enrich_resultado(r: ResultadoLaboratorio, db: Session) -> ResultadoRead:
    data = ResultadoRead.model_validate(r)
    bio = db.get(Usuario, r.bioanalista_id)  # aislamiento: ya autorizado por el padre
    if bio:
        data.bioanalista_nombre = bio.nombre_completo
    return data


# ══════════════════════════════════════════════════════════════════
# ÓRDENES DE LABORATORIO
# ══════════════════════════════════════════════════════════════════

@router.get("/ordenes", response_model=list[OrdenLabRead])
def list_ordenes(
    estado: Optional[str] = Query(None, description="Filtrar por estado"),
    paciente_id: Optional[int] = Query(None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    """Lista todas las órdenes de tipo LABORATORIO de la institución."""
    query = select(Orden).where(Orden.tipo == TipoOrden.LABORATORIO)
    if estado:
        query = query.where(Orden.estado == estado)
    if paciente_id:
        query = query.where(Orden.paciente_id == paciente_id)
    # Filtro multi-tenancy por paciente's institution
    if current_user.rol != RolUsuario.SUPERADMIN and current_user.institucion_id:
        # Join implícito via paciente
        query = query.join(Paciente, Orden.paciente_id == Paciente.id, isouter=True).where(
            (Paciente.institucion_id == current_user.institucion_id) | (Orden.paciente_id == None)
        )
    query = query.order_by(Orden.fecha_orden.desc())
    ordenes = db.exec(query).all()
    return [_enrich_orden(o, db) for o in ordenes]


@router.get("/ordenes/{orden_id}", response_model=OrdenLabRead)
def get_orden(
    orden_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    orden = obtener(db, Orden, orden_id, current_user, que="orden de laboratorio")
    if orden.tipo != TipoOrden.LABORATORIO:
        raise HTTPException(status_code=404, detail="Orden de laboratorio no encontrada")
    return _enrich_orden(orden, db)


@router.put("/ordenes/{orden_id}/estado", response_model=OrdenLabRead)
def update_estado(
    orden_id: int,
    payload: EstadoOrdenUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    orden = obtener(db, Orden, orden_id, current_user, que="orden de laboratorio")
    if orden.tipo != TipoOrden.LABORATORIO:
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
# RESULTADOS
# ══════════════════════════════════════════════════════════════════

@router.get("/ordenes/{orden_id}/resultado", response_model=ResultadoRead)
def get_resultado(
    orden_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    resultado = db.exec(
        select(ResultadoLaboratorio).where(ResultadoLaboratorio.orden_id == orden_id)
    ).first()
    if not resultado:
        raise HTTPException(status_code=404, detail="Resultado no encontrado")
    return _enrich_resultado(resultado, db)


@router.post("/ordenes/{orden_id}/resultado", response_model=ResultadoRead, status_code=201)
def create_resultado(
    orden_id: int,
    payload: ResultadoCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    orden = obtener(db, Orden, orden_id, current_user, que="orden de laboratorio")
    if orden.tipo != TipoOrden.LABORATORIO:
        raise HTTPException(status_code=404, detail="Orden no encontrada")
    existente = db.exec(
        select(ResultadoLaboratorio).where(ResultadoLaboratorio.orden_id == orden_id)
    ).first()
    if existente:
        raise HTTPException(status_code=400, detail="Ya existe un resultado para esta orden. Use PUT para actualizarlo.")
    resultado = ResultadoLaboratorio(
        orden_id=orden_id,
        paciente_id=orden.paciente_id,
        bioanalista_id=current_user.id,
        **payload.model_dump(),
    )
    db.add(resultado)
    # Update order status to EN_PROCESO if still PENDIENTE
    if orden.estado == EstadoOrden.PENDIENTE:
        orden.estado = EstadoOrden.EN_PROCESO
        db.add(orden)
    db.commit()
    db.refresh(resultado)
    return _enrich_resultado(resultado, db)


@router.put("/ordenes/{orden_id}/resultado", response_model=ResultadoRead)
def update_resultado(
    orden_id: int,
    payload: ResultadoUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    resultado = db.exec(
        select(ResultadoLaboratorio).where(ResultadoLaboratorio.orden_id == orden_id)
    ).first()
    if not resultado:
        raise HTTPException(status_code=404, detail="Resultado no encontrado")
    if resultado.validado:
        raise HTTPException(status_code=400, detail="No se puede modificar un resultado validado")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(resultado, field, value)
    db.add(resultado)
    db.commit()
    db.refresh(resultado)
    return _enrich_resultado(resultado, db)


@router.post("/ordenes/{orden_id}/resultado/validar", response_model=ResultadoRead)
def validar_resultado(
    orden_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    resultado = db.exec(
        select(ResultadoLaboratorio).where(ResultadoLaboratorio.orden_id == orden_id)
    ).first()
    if not resultado:
        raise HTTPException(status_code=404, detail="Resultado no encontrado")
    resultado.validado = True
    resultado.fecha_validacion = datetime.now()
    resultado.validado_por_id = current_user.id
    # Mark order as COMPLETADA
    orden = obtener(db, Orden, orden_id, current_user, que="orden de laboratorio")
    if orden:
        orden.estado = EstadoOrden.COMPLETADA
        db.add(orden)
    db.add(resultado)
    db.commit()
    db.refresh(resultado)
    return _enrich_resultado(resultado, db)


# ══════════════════════════════════════════════════════════════════
# HISTORIAL DE PACIENTE
# ══════════════════════════════════════════════════════════════════

@router.get("/paciente/{paciente_id}/resultados", response_model=list[ResultadoRead])
def get_resultados_paciente(
    paciente_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    """Devuelve todos los resultados de laboratorio de un paciente."""
    resultados = db.exec(
        select(ResultadoLaboratorio)
        .where(ResultadoLaboratorio.paciente_id == paciente_id)
        .order_by(ResultadoLaboratorio.fecha_resultado.desc())
    ).all()
    return [_enrich_resultado(r, db) for r in resultados]


# ══════════════════════════════════════════════════════════════════
# PERFILES DE LABORATORIO (plantillas de exámenes)
# ══════════════════════════════════════════════════════════════════

@router.get("/perfiles", response_model=list[PerfilRead])
def list_perfiles(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(PerfilLaboratorio).where(PerfilLaboratorio.activo == True)
    if current_user.rol != RolUsuario.SUPERADMIN and current_user.institucion_id:
        query = query.where(
            (PerfilLaboratorio.institucion_id == current_user.institucion_id)
            | (PerfilLaboratorio.institucion_id == None)
        )
    return db.exec(query.order_by(PerfilLaboratorio.nombre)).all()


@router.post("/perfiles", response_model=PerfilRead, status_code=201)
def create_perfil(
    payload: PerfilCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    if current_user.rol not in (RolUsuario.ADMIN, RolUsuario.SUPERADMIN,
                                 RolUsuario.LABORATORISTA, RolUsuario.BACTERIOLOGO):
        raise HTTPException(status_code=403, detail="Sin permiso para crear perfiles")
    perfil = PerfilLaboratorio(
        **payload.model_dump(),
        institucion_id=current_user.institucion_id,
    )
    db.add(perfil)
    db.commit()
    db.refresh(perfil)
    return perfil


@router.delete("/perfiles/{perfil_id}", status_code=204)
def delete_perfil(
    perfil_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    perfil = obtener(db, PerfilLaboratorio, perfil_id, current_user, que="perfil")
    perfil.activo = False
    db.add(perfil)
    db.commit()
