"""Nota operatoria. Migrado de `modules/nota_operatoria.py` (NiceGUI).

Lo que cambia:

  - **El tiempo quirúrgico se calcula de las horas.** El modelo guarda los tres
    campos y escritos por separado acaban discrepando.
  - **`estado_paciente_salida` es obligatorio.** Traía «ESTABLE» de defecto: una
    afirmación clínica que nadie firmó.
  - **Avisa sin bloquear.** Un sangrado de 2 litros o una cirugía de 14 horas
    son infrecuentes, no imposibles. Impedir registrarlas obligaría a mentir.
"""

from fastapi import APIRouter, Depends, HTTPException
from sqlmodel import Session, select

import notas_operatorias as dominio
from auditoria import registrar_auditoria
from core.aislamiento import obtener
from core.deps import (
    filtrar_por_institucion,
    get_current_active_user,
    get_db,
    requiere_modulo,
)
from models import ModuloSistema, Paciente, ProtocoloQuirurgico, Usuario
from schemas.nota_operatoria import NotaOperatoriaCreate, NotaOperatoriaRead

router = APIRouter(
    prefix="/nota-operatoria",
    tags=["nota-operatoria"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.NOTA_OPERATORIA))],
)


def _a_lectura(n: ProtocoloQuirurgico, db: Session) -> NotaOperatoriaRead:
    t = dominio.duracion(n.hora_inicio, n.hora_fin, n.tiempo_quirurgico_min)
    p = db.get(Paciente, n.paciente_id)   # aislamiento: ya pasó por obtener() o por la lista filtrada
    c = db.get(Usuario, n.cirujano_id)    # aislamiento: idem, solo para el nombre
    datos = {k: v for k, v in n.model_dump().items()
             if k in NotaOperatoriaRead.model_fields}
    datos["tiempo_quirurgico_min"] = t.minutos
    return NotaOperatoriaRead(
        **datos,
        paciente=p.nombre_completo if p else None,
        cirujano=c.nombre_completo if c else None,
        avisos=list(dominio.revisar(n).mensajes),
    )


@router.get("/", response_model=list[NotaOperatoriaRead])
def listar(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    q = (
        select(ProtocoloQuirurgico)
        .where(ProtocoloQuirurgico.paciente_id.in_(
            filtrar_por_institucion(select(Paciente.id), Paciente, current_user)))
        .order_by(ProtocoloQuirurgico.fecha_creacion.desc())
    )
    return [_a_lectura(n, db) for n in db.exec(q).all()]


@router.post("/", response_model=NotaOperatoriaRead, status_code=201)
def crear(
    payload: NotaOperatoriaCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    obtener(db, Paciente, payload.paciente_id, current_user, que="paciente")

    try:
        t = dominio.duracion(payload.hora_inicio, payload.hora_fin)
        estado = dominio.validar_estado_salida(payload.estado_paciente_salida)
    except (dominio.TiemposNoValidos, dominio.EstadoNoValido) as e:
        raise HTTPException(status_code=422, detail=str(e)) from None

    datos = payload.model_dump()
    datos["estado_paciente_salida"] = estado
    n = ProtocoloQuirurgico(
        **datos,
        cirujano_id=current_user.id,
        tiempo_quirurgico_min=t.minutos,
        institucion_id=current_user.institucion_id,
    )
    db.add(n)
    db.commit()
    db.refresh(n)
    registrar_auditoria(
        usuario_id=current_user.id, accion="CREAR_NOTA_OPERATORIA",
        tabla_afectada="protocoloquirurgico", registro_id=n.id,
        paciente_id=n.paciente_id,
    )
    return _a_lectura(n, db)


@router.get("/{nota_id}", response_model=NotaOperatoriaRead)
def detalle(
    nota_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    n = obtener(db, ProtocoloQuirurgico, nota_id, current_user, que="nota operatoria")
    return _a_lectura(n, db)
