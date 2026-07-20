"""Incapacidades médicas. Migrado de `modules/incapacidad.py` (NiceGUI).

Lo que cambia respecto al original, aparte de dejar de ser una pantalla:

  - **Filtra por institución.** El módulo NiceGUI consultaba `Historia` sin
    ningún filtro: en un despliegue multi-institucional listaba las
    incapacidades de todas. Aquí pasa por `filtrar_por_institucion` y el
    detalle por `obtener`.
  - **La fecha de fin ya no lleva un día de más.** Ver `incapacidades.py`.
  - El resumen del mes se calcula sobre lo mismo que se lista, no con una
    segunda consulta que podía discrepar del filtro aplicado.
"""

from datetime import date
from typing import Optional

from fastapi import APIRouter, Depends, Query
from sqlalchemy.orm import joinedload
from sqlmodel import Session, select

import incapacidades as dominio
from core.aislamiento import obtener
from core.deps import (
    filtrar_por_institucion,
    get_current_active_user,
    get_db,
    requiere_modulo,
)
from models import Historia, ModuloSistema, Paciente, Usuario
from schemas.incapacidad import IncapacidadDetalle, IncapacidadRead, ResumenIncapacidades

router = APIRouter(
    prefix="/incapacidades",
    tags=["incapacidades"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.INCAPACIDADES))],
)


def _consulta(db: Session, current_user: Usuario, *, desde, hasta, medico_id, buscar):
    q = (
        select(Historia)
        .options(joinedload(Historia.paciente), joinedload(Historia.medico))
        .where(Historia.genera_incapacidad == True)  # noqa: E712
    )
    if desde:
        q = q.where(Historia.fecha >= desde)
    if hasta:
        q = q.where(Historia.fecha <= hasta)
    if medico_id:
        q = q.where(Historia.medico_id == medico_id)
    if buscar:
        aguja = f"%{buscar}%"
        q = q.join(Paciente, Historia.paciente_id == Paciente.id).where(
            Paciente.nombre1.ilike(aguja)
            | Paciente.apellido1.ilike(aguja)
            | Paciente.identificacion.ilike(aguja)
        )
    # `Historia` no lleva institucion_id: se filtra por el paciente, que sí.
    sub = filtrar_por_institucion(select(Paciente.id), Paciente, current_user)
    q = q.where(Historia.paciente_id.in_(sub))
    return q.order_by(Historia.fecha.desc())


def _a_lectura(h: Historia) -> IncapacidadRead:
    p = dominio.desde_historia(h)
    return IncapacidadRead(
        historia_id=h.id,
        codigo_historia=h.codigo_historia,
        fecha_consulta=h.fecha,
        paciente=h.paciente.nombre_completo if h.paciente else "—",
        identificacion=h.paciente.identificacion if h.paciente else "—",
        dias=p.dias if p else 0,
        fecha_inicio=p.inicio if p else None,
        fecha_fin=p.fin if p else None,
        diagnostico=h.diagnostico_principal,
        medico=h.medico.nombre_completo if h.medico else "—",
    )


@router.get("/", response_model=list[IncapacidadRead])
def listar(
    desde: Optional[date] = None,
    hasta: Optional[date] = None,
    medico_id: Optional[int] = None,
    buscar: Optional[str] = Query(None, description="Nombre, apellido o documento"),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    q = _consulta(db, current_user, desde=desde, hasta=hasta,
                  medico_id=medico_id, buscar=buscar)
    return [_a_lectura(h) for h in db.exec(q).unique().all()]


@router.get("/resumen", response_model=ResumenIncapacidades)
def resumen(
    desde: Optional[date] = None,
    hasta: Optional[date] = None,
    medico_id: Optional[int] = None,
    buscar: Optional[str] = None,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    """Cuenta y días del MISMO conjunto que devuelve `listar`.

    En el original el resumen salía de una consulta aparte, sin los filtros de
    la pantalla: el usuario veía «12 incapacidades» arriba y tres en la lista.
    """
    q = _consulta(db, current_user, desde=desde, hasta=hasta,
                  medico_id=medico_id, buscar=buscar)
    r = dominio.resumir(db.exec(q).unique().all())
    return ResumenIncapacidades(
        cantidad=r.cantidad,
        dias_totales=r.dias_totales,
        promedio_dias=r.promedio_dias,
    )


@router.get("/{historia_id}", response_model=IncapacidadDetalle)
def detalle(
    historia_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    """El detalle para imprimir la certificación."""
    h = obtener(db, Historia, historia_id, current_user, que="incapacidad")
    base = _a_lectura(h)
    return IncapacidadDetalle(
        **base.model_dump(),
        motivo_consulta=h.motivo_consulta,
        diagnostico_relacionado=h.diagnostico_relacionado1,
        observaciones=h.plan_tratamiento,
    )
