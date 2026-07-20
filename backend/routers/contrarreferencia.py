"""Contrarreferencia. Migrado de `modules/contrarreferencia.py` (NiceGUI).

Lo que cambia:

  - **Cierra la remisión.** El original guardaba los dos documentos sin
    relacionarlos operativamente: la remisión seguía PENDIENTE para siempre.
  - **Solo ofrece remisiones abiertas.** El original listaba las últimas
    treinta sin mirar el estado, así que se podía contrarreferir contra una
    cancelada.
  - **Filtra por institución** y el acuse de recibo exige responsable.
"""

from typing import Optional

from fastapi import APIRouter, Depends, HTTPException
from sqlmodel import Session, select

import contrarreferencias as dominio
import remisiones as maquina
from auditoria import registrar_auditoria
from core.aislamiento import obtener
from core.deps import (
    filtrar_por_institucion,
    get_current_active_user,
    get_db,
    requiere_modulo,
)
from models import (
    Contrarreferencia,
    ModuloSistema,
    Orden,
    Paciente,
    TipoOrden,
    Usuario,
)
from schemas.contrarreferencia import (
    ContrarreferenciaCreate,
    ContrarreferenciaRead,
    RemisionAbierta,
)

router = APIRouter(
    prefix="/contrarreferencia",
    tags=["contrarreferencia"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.CONTRARREFERENCIA))],
)


def _a_lectura(cr: Contrarreferencia, db: Session,
               cierre: Optional[str] = None) -> ContrarreferenciaRead:
    p = db.get(Paciente, cr.paciente_id)  # aislamiento: el registro ya pasó por la lista filtrada o por obtener()
    datos = {k: v for k, v in cr.model_dump().items()
             if k in ContrarreferenciaRead.model_fields}
    return ContrarreferenciaRead(
        **datos,
        paciente=p.nombre_completo if p else None,
        cierre=cierre,
    )


def _mias(current_user: Usuario):
    return filtrar_por_institucion(select(Paciente.id), Paciente, current_user)


@router.get("/", response_model=list[ContrarreferenciaRead])
def listar(
    solo_pendientes: bool = False,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    q = select(Contrarreferencia).where(
        Contrarreferencia.paciente_id.in_(_mias(current_user))
    )
    if solo_pendientes:
        q = q.where(Contrarreferencia.recibida == False)  # noqa: E712
    q = q.order_by(Contrarreferencia.fecha_creacion.desc())
    return [_a_lectura(cr, db) for cr in db.exec(q).all()]


@router.get("/remisiones-abiertas", response_model=list[RemisionAbierta])
def remisiones_abiertas(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    """Contra qué se puede contrarreferir.

    Solo las abiertas: contrarreferir contra una cancelada no significa nada.
    """
    q = (
        select(Orden)
        .where(Orden.tipo == TipoOrden.ESPECIALISTA)
        .where(Orden.estado.in_(list(maquina.ABIERTAS)))
        .where(Orden.paciente_id.in_(_mias(current_user)))
        .order_by(Orden.fecha_orden.desc())
    )
    salida = []
    for o in db.exec(q).all():
        p = db.get(Paciente, o.paciente_id)  # aislamiento: la consulta ya filtró por institución
        salida.append(RemisionAbierta(
            id=o.id, fecha_orden=o.fecha_orden,
            paciente=p.nombre_completo if p else "—",
            descripcion=o.descripcion, estado=o.estado.value,
        ))
    return salida


@router.post("/", response_model=ContrarreferenciaRead, status_code=201)
def crear(
    payload: ContrarreferenciaCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    obtener(db, Paciente, payload.paciente_id, current_user, que="paciente")

    orden = None
    if payload.orden_id is not None:
        orden = obtener(db, Orden, payload.orden_id, current_user, que="remisión")
        if orden.tipo != TipoOrden.ESPECIALISTA:
            raise HTTPException(status_code=404, detail="Remisión no encontrada")
        if orden.paciente_id != payload.paciente_id:
            raise HTTPException(
                status_code=422,
                detail="Esa remisión es de otro paciente.",
            )

    cr = Contrarreferencia(
        **payload.model_dump(),
        medico_contrarrefiere_id=current_user.id,
        institucion_origen_id=current_user.institucion_id,
    )
    cierre = dominio.cerrar_remision(orden)
    if cierre.cerrada:
        db.add(orden)

    db.add(cr)
    db.commit()
    db.refresh(cr)

    registrar_auditoria(
        usuario_id=current_user.id, accion="CREAR_CONTRARREFERENCIA",
        tabla_afectada="contrarreferencia", registro_id=cr.id,
        paciente_id=cr.paciente_id,
        datos_despues={"orden_id": cr.orden_id, "cierre": str(cierre)},
    )
    return _a_lectura(cr, db, cierre=str(cierre))


@router.get("/{cr_id}", response_model=ContrarreferenciaRead)
def detalle(
    cr_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    cr = obtener(db, Contrarreferencia, cr_id, current_user, que="contrarreferencia")
    return _a_lectura(cr, db)


@router.post("/{cr_id}/recibir", response_model=ContrarreferenciaRead)
def recibir(
    cr_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    """Acusa recibo. Es un acto distinto de crear el documento."""
    cr = obtener(db, Contrarreferencia, cr_id, current_user, que="contrarreferencia")
    try:
        dominio.marcar_recibida(cr, current_user.id)
    except dominio.YaRecibida as e:
        raise HTTPException(status_code=409, detail=str(e)) from None
    db.add(cr)
    db.commit()
    db.refresh(cr)
    registrar_auditoria(
        usuario_id=current_user.id, accion="RECIBIR_CONTRARREFERENCIA",
        tabla_afectada="contrarreferencia", registro_id=cr.id,
        paciente_id=cr.paciente_id,
    )
    return _a_lectura(cr, db)
