"""Consentimiento informado. Migrado de `modules/consentimiento.py` (NiceGUI).

Lo que cambia respecto al original, que guardaba el registro y lo daba por
bueno:

  - **Se juzga si el consentimiento vale.** Ver `consentimientos.py`. Firmar no
    es consentir: sin la constancia de que el paciente comprendió, hay una
    firma y no un consentimiento informado.
  - **La revocación nunca se bloquea.** El paciente puede retirarlo en cualquier
    momento; una comprobación que lo impidiera convertiría al sistema en el
    instrumento de una intervención no consentida.
  - **Filtra por institución**, y el detalle pasa por `obtener`.
  - **No se borra: se revoca.** Borrarlo dejaría a la institución sin la prueba
    de que en su momento sí lo hubo.
"""

from typing import Optional

from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select

import consentimientos as dominio
from auditoria import registrar_auditoria
from core.aislamiento import obtener
from core.deps import (
    filtrar_por_institucion,
    get_current_active_user,
    get_db,
    requiere_modulo,
)
from models import ConsentimientoInformado, ModuloSistema, Paciente, Usuario
from schemas.consentimiento import (
    ConsentimientoCreate,
    ConsentimientoRead,
    ConsentimientoUpdate,
    Revocacion,
)

router = APIRouter(
    prefix="/consentimiento",
    tags=["consentimiento"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.CONSENTIMIENTO))],
)


def _a_lectura(c: ConsentimientoInformado, db: Session) -> ConsentimientoRead:
    v = dominio.evaluar(c)
    paciente = db.get(Paciente, c.paciente_id)  # aislamiento: el consentimiento ya pasó por obtener() o por la lista filtrada
    medico = db.get(Usuario, c.medico_id)       # aislamiento: idem, solo para poner el nombre
    datos = c.model_dump()
    datos.pop("paciente", None)
    return ConsentimientoRead(
        **datos,
        paciente=paciente.nombre_completo if paciente else None,
        medico=medico.nombre_completo if medico else None,
        valido=v.valido,
        faltantes=list(v.faltantes),
        resumen=str(v),
    )


@router.get("/", response_model=list[ConsentimientoRead])
def listar(
    paciente_id: Optional[int] = None,
    solo_validos: bool = Query(False, description="Deja fuera los incompletos y revocados"),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    q = select(ConsentimientoInformado)
    if paciente_id:
        q = q.where(ConsentimientoInformado.paciente_id == paciente_id)
    sub = filtrar_por_institucion(select(Paciente.id), Paciente, current_user)
    q = q.where(ConsentimientoInformado.paciente_id.in_(sub))
    q = q.order_by(ConsentimientoInformado.fecha_firma.desc())

    salida = [_a_lectura(c, db) for c in db.exec(q).all()]
    return [c for c in salida if c.valido] if solo_validos else salida


@router.post("/", response_model=ConsentimientoRead, status_code=201)
def crear(
    payload: ConsentimientoCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    # El paciente tiene que ser de la institución de quien captura.
    obtener(db, Paciente, payload.paciente_id, current_user, que="paciente")

    c = ConsentimientoInformado(**payload.model_dump(), medico_id=current_user.id)
    db.add(c)
    db.commit()
    db.refresh(c)
    registrar_auditoria(
        usuario_id=current_user.id, accion="CREAR_CONSENTIMIENTO",
        tabla_afectada="consentimientoinformado", registro_id=c.id,
        paciente_id=c.paciente_id,
    )
    return _a_lectura(c, db)


@router.get("/{consentimiento_id}", response_model=ConsentimientoRead)
def detalle(
    consentimiento_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    c = obtener(db, ConsentimientoInformado, consentimiento_id, current_user,
                que="consentimiento")
    return _a_lectura(c, db)


@router.put("/{consentimiento_id}", response_model=ConsentimientoRead)
def actualizar(
    consentimiento_id: int,
    payload: ConsentimientoUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    c = obtener(db, ConsentimientoInformado, consentimiento_id, current_user,
                que="consentimiento")
    if c.revocado:
        raise HTTPException(
            status_code=409,
            detail=(
                "El paciente revocó este consentimiento y no se puede modificar. "
                "Si volvió a consentir, registre uno nuevo: así queda constancia "
                "de que hubo una revocación en medio."
            ),
        )
    for campo, valor in payload.model_dump(exclude_unset=True).items():
        setattr(c, campo, valor)
    db.add(c)
    db.commit()
    db.refresh(c)
    return _a_lectura(c, db)


@router.post("/{consentimiento_id}/revocar", response_model=ConsentimientoRead)
def revocar(
    consentimiento_id: int,
    payload: Revocacion,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    """Registra que el paciente retiró su consentimiento.

    No comprueba el estado previo a propósito: el derecho a revocar no depende
    de en qué punto esté el trámite.
    """
    c = obtener(db, ConsentimientoInformado, consentimiento_id, current_user,
                que="consentimiento")
    try:
        dominio.revocar(c, payload.motivo)
    except dominio.NoSePuedeRevocar as e:
        raise HTTPException(status_code=422, detail=str(e)) from None

    db.add(c)
    db.commit()
    db.refresh(c)
    registrar_auditoria(
        usuario_id=current_user.id, accion="REVOCAR_CONSENTIMIENTO",
        tabla_afectada="consentimientoinformado", registro_id=c.id,
        paciente_id=c.paciente_id,
        datos_despues={"motivo": c.motivo_revocacion},
    )
    return _a_lectura(c, db)
