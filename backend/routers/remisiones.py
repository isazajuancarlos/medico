"""Remisiones a especialista. Migrado de `modules/remisiones.py` (NiceGUI).

Una remisión es una `Orden` de tipo ESPECIALISTA; no hay modelo aparte, y se
conserva así.

Lo que cambia respecto al original:

  - **El estado ya no se puede poner en cualquier cosa.** Ver `remisiones.py`.
  - **Filtra por institución.** El original hacía `session.get(Orden, id)` sin
    filtro y consultaba las órdenes de todas.
  - **Una remisión inexistente da 404**, no un `if orden:` que no hace nada y
    deja al usuario creyendo que guardó.
"""

from datetime import date
from typing import Optional

from fastapi import APIRouter, Depends, HTTPException, Query
from sqlalchemy.orm import joinedload
from sqlmodel import Session, select

import remisiones as dominio
from auditoria import registrar_auditoria
from core.aislamiento import obtener
from core.deps import (
    filtrar_por_institucion,
    get_current_active_user,
    get_db,
    requiere_modulo,
)
from models import EstadoOrden, ModuloSistema, Orden, Paciente, TipoOrden, Usuario
from schemas.remision import CambioEstado, RemisionRead, ResumenRemisiones

router = APIRouter(
    prefix="/remisiones",
    tags=["remisiones"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.REMISIONES))],
)


def _consulta(current_user: Usuario, *, estado, desde, hasta, buscar):
    q = (
        select(Orden)
        .options(
            joinedload(Orden.paciente),
            joinedload(Orden.medico_ordena),
            joinedload(Orden.especialidad_destino),
            joinedload(Orden.institucion_destino),
        )
        .where(Orden.tipo == TipoOrden.ESPECIALISTA)
    )
    if estado:
        q = q.where(Orden.estado == estado)
    if desde:
        q = q.where(Orden.fecha_orden >= desde)
    if hasta:
        q = q.where(Orden.fecha_orden <= hasta)
    if buscar:
        aguja = f"%{buscar}%"
        q = q.join(Paciente, Orden.paciente_id == Paciente.id).where(
            Paciente.nombre1.ilike(aguja)
            | Paciente.apellido1.ilike(aguja)
            | Paciente.identificacion.ilike(aguja)
        )
    # `Orden` no lleva institucion_id propio —el que tiene es el de DESTINO—,
    # así que el aislamiento va por el paciente.
    sub = filtrar_por_institucion(select(Paciente.id), Paciente, current_user)
    q = q.where(Orden.paciente_id.in_(sub))
    return q.order_by(Orden.fecha_orden.desc())


def _a_lectura(o: Orden) -> RemisionRead:
    return RemisionRead(
        id=o.id,
        fecha_orden=o.fecha_orden,
        estado=o.estado,
        paciente=o.paciente.nombre_completo if o.paciente else "—",
        identificacion=o.paciente.identificacion if o.paciente else "—",
        descripcion=o.descripcion,
        justificacion=o.justificacion,
        especialidad_destino=o.especialidad_destino.nombre if o.especialidad_destino else None,
        institucion_destino=o.institucion_destino.razon_social if o.institucion_destino else None,
        medico_ordena=o.medico_ordena.nombre_completo if o.medico_ordena else None,
        codigo_cups=o.codigo_cups,
        estados_posibles=dominio.destinos(o.estado),
        abierta=dominio.esta_abierta(o.estado),
    )


@router.get("/", response_model=list[RemisionRead])
def listar(
    estado: Optional[EstadoOrden] = None,
    desde: Optional[date] = None,
    hasta: Optional[date] = None,
    buscar: Optional[str] = Query(None, description="Nombre, apellido o documento"),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    q = _consulta(current_user, estado=estado, desde=desde, hasta=hasta, buscar=buscar)
    return [_a_lectura(o) for o in db.exec(q).unique().all()]


@router.get("/resumen", response_model=ResumenRemisiones)
def resumen(
    desde: Optional[date] = None,
    hasta: Optional[date] = None,
    buscar: Optional[str] = None,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    """Cuenta por estado, sobre el MISMO conjunto que devuelve `listar`."""
    q = _consulta(current_user, estado=None, desde=desde, hasta=hasta, buscar=buscar)
    ordenes = db.exec(q).unique().all()
    por_estado: dict[str, int] = {e.value: 0 for e in EstadoOrden}
    for o in ordenes:
        por_estado[o.estado.value] = por_estado.get(o.estado.value, 0) + 1
    return ResumenRemisiones(
        total=len(ordenes),
        abiertas=sum(1 for o in ordenes if dominio.esta_abierta(o.estado)),
        por_estado=por_estado,
    )


@router.get("/{remision_id}", response_model=RemisionRead)
def detalle(
    remision_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    return _a_lectura(_remision(db, remision_id, current_user))


@router.put("/{remision_id}/estado", response_model=RemisionRead)
def cambiar_estado(
    remision_id: int,
    payload: CambioEstado,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    o = _remision(db, remision_id, current_user)
    anterior = o.estado
    try:
        o.estado = dominio.validar_cambio(anterior, payload.estado)
    except dominio.TransicionNoValida as e:
        raise HTTPException(status_code=409, detail=str(e)) from None

    db.add(o)
    db.commit()
    db.refresh(o)

    registrar_auditoria(
        usuario_id=current_user.id,
        accion="ACTUALIZAR_ESTADO_REMISION",
        tabla_afectada="orden",
        registro_id=o.id,
        paciente_id=o.paciente_id,
        datos_antes={"estado": anterior.value},
        datos_despues={"estado": o.estado.value},
    )
    return _a_lectura(o)


def _remision(db: Session, remision_id: int, current_user: Usuario) -> Orden:
    o = obtener(db, Orden, remision_id, current_user, que="remisión")
    if o.tipo != TipoOrden.ESPECIALISTA:
        # Es una orden, pero no una remisión. Se responde igual que si no
        # existiera: por esta ruta no se ven órdenes de otro tipo.
        raise HTTPException(status_code=404, detail="Remisión no encontrada")
    return o
