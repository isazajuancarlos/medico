from typing import Optional
from datetime import datetime
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from models import Quirofano, ProgramacionQuirurgica, Paciente, Usuario, RolUsuario, ModuloSistema
from core.deps import get_db, get_current_active_user, requiere_modulo, filtrar_por_institucion
from core.aislamiento import obtener
from schemas.quirofano import (
    QuirofanoRead, ProgramacionRead, ProgramacionCreate,
    ChecklistUpdate, IniciarCirugia, CompletarCirugia, CancelarCirugia,
    MedicoBasic,
)

router = APIRouter(prefix="/quirofano", tags=["quirofano"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.QUIROFANO))],
)


def _sala_del_ambito(db: Session, quirofano_id: int, actual: Usuario) -> Quirofano:
    """La sala tiene que ser de esta institución **o de ninguna**.

    === POR QUÉ «O DE NINGUNA», Y NO `obtener()` A SECAS ===

    La primera versión de este PR usaba `obtener()`, que lee una sala con
    `institucion_id IS NULL` como `SIN_DUENIO` y la NIEGA. Suena bien y rompía el
    producto: `seed.py:706` crea las seis salas SIN institución, así que en toda
    instalación sembrada —la demo incluida— **ninguna** programación de cirugía
    se podía crear. Medido por la revisión de seguridad de esta rama:

        [salas/inst1]  200 -> ['SalaHUERFANA', 'SalaUNO']
        [crear, sala huérfana]  404 «Sala de quirófano no encontrado»

    Falla cerrado, así que no era una fuga — era el sesgo a cerrar de la
    directiva 21: una regla que se prueba contra el atacante y no contra la
    operación normal del cliente.

    Lo que se hace aquí es que las DOS capas digan lo mismo. `list_salas` (l. 60)
    ya enseña las huérfanas a todo el mundo desde antes de este PR: esa era la
    decisión del producto, y crear contra lo que se enseña tiene que funcionar.
    Una sala sin dueño **no es la sala de otro prestador**: es de nadie, y
    `Quirofano` no lleva dato clínico —nombre, tipo, estado—, así que aceptarla
    no abre nada.

    Lo que NO se hace: dejar pasar la sala de OTRA institución, que es lo que
    este PR vino a cerrar.

    La limpieza de fondo —darle dueño a las salas en `seed.py` y adoptar las
    existentes con una migración— es una decisión aparte y está anotada como
    tarea. Hasta entonces, esta función es la única verdad y `list_salas` su
    pareja.
    """
    sala = db.get(Quirofano, quirofano_id)  # aislamiento: la comprobación es la línea de abajo
    ajena = (
        sala is not None
        and sala.institucion_id is not None
        and actual.rol != RolUsuario.SUPERADMIN
        and sala.institucion_id != actual.institucion_id
    )
    if sala is None or ajena:
        # 404 y nunca 403: la sala ajena y la inexistente son indistinguibles.
        raise HTTPException(status_code=404, detail="Sala de quirófano no encontrada")
    return sala


def _exigir_del_ambito(db: Session, usuario_id: int, actual: Usuario, que: str) -> None:
    """El profesional que se nombra tiene que ser de la institución de quien captura.

    No usa `obtener()` porque `Usuario` no es un registro clínico y su mensaje
    debe hablar del ROL, no del modelo. Lo que sí comparte es la regla dura: 404
    y nunca 403, para que un id ajeno y uno inexistente sean indistinguibles.
    """
    u = db.get(Usuario, usuario_id)   # aislamiento: la comprobación es la línea de abajo
    if u is None or (
        actual.rol != RolUsuario.SUPERADMIN and u.institucion_id != actual.institucion_id
    ):
        raise HTTPException(status_code=404, detail=f"{que.capitalize()} no encontrado")


def _enrich(prog: ProgramacionQuirurgica, db: Session) -> ProgramacionRead:
    data = ProgramacionRead.model_validate(prog)
    p = db.get(Paciente, prog.paciente_id)  # aislamiento: ya autorizado por el padre
    if p:
        data.paciente_nombre = p.nombre_completo.strip()
        data.paciente_identificacion = p.identificacion
    q = db.get(Quirofano, prog.quirofano_id)  # aislamiento: ya autorizado por el padre
    if q:
        data.quirofano_nombre = q.nombre
    c = db.get(Usuario, prog.cirujano_id)  # aislamiento: ya autorizado por el padre
    if c:
        data.cirujano_nombre = c.nombre_completo
    if prog.anestesiologo_id:
        a = db.get(Usuario, prog.anestesiologo_id)  # aislamiento: ya autorizado por el padre
        if a:
            data.anestesiologo_nombre = a.nombre_completo
    return data


@router.get("/salas", response_model=list[QuirofanoRead])
def list_salas(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(Quirofano).order_by(Quirofano.nombre)
    if current_user.rol != RolUsuario.SUPERADMIN and current_user.institucion_id:
        from sqlalchemy import or_
        query = query.where(
            or_(
                Quirofano.institucion_id == current_user.institucion_id,
                Quirofano.institucion_id == None,
            )
        )
    return db.exec(query).all()


@router.get("/programaciones", response_model=list[ProgramacionRead])
def list_programaciones(
    estado: Optional[str] = Query(None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    # ESTE LISTADO NO ACOTABA NADA, y `_enrich` le añade el nombre completo y el
    # DOCUMENTO DE IDENTIDAD del paciente. Medido el 2026-08-06 con dos
    # instituciones: cada admin veía las cirugías de la otra con
    # `paciente='PacienteDOS Secreto' doc='doc2'`. Expone más que la fuga de
    # facturación —allí eran importes y nombres; aquí es qué cirugía se le va a
    # practicar a quién— y es categoría especial de la Ley 1581.
    #
    # `ProgramacionQuirurgica` no lleva `institucion_id` y no debe llevarla:
    # cuelga del paciente, igual que la historia. `filtrar_por_institucion` ya
    # sabe seguir esa cadena (`paciente_id` está en `ENLACES`), así que el
    # arreglo es usar la capa que existe y no escribir un filtro nuevo.
    query = filtrar_por_institucion(select(ProgramacionQuirurgica), ProgramacionQuirurgica, current_user)
    if estado:
        query = query.where(ProgramacionQuirurgica.estado == estado)
    query = query.order_by(ProgramacionQuirurgica.fecha_programada.desc())
    return [_enrich(p, db) for p in db.exec(query).all()]


@router.get("/programaciones/{prog_id}", response_model=ProgramacionRead)
def get_programacion(
    prog_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    prog = obtener(db, ProgramacionQuirurgica, prog_id, current_user, que="programación quirúrgica")
    return _enrich(prog, db)


@router.get("/medicos", response_model=list[MedicoBasic])
def list_medicos(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    roles = [RolUsuario.MEDICO, RolUsuario.ANESTESIOLOGO]
    query = select(Usuario).where(Usuario.rol.in_(roles)).where(Usuario.activo == True)
    if current_user.rol != RolUsuario.SUPERADMIN and current_user.institucion_id:
        query = query.where(Usuario.institucion_id == current_user.institucion_id)
    return db.exec(query).all()


@router.post("/programaciones", response_model=ProgramacionRead, status_code=201)
def crear_programacion(
    payload: ProgramacionCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    # LOS CUATRO ID DEL CUERPO SE COMPRUEBAN, y es el mismo agujero que se cazó
    # en `insertar_detalle` de facturación: sin esto se programa una cirugía
    # propia nombrando al PACIENTE del vecino, y `_enrich` devuelve su nombre y
    # su documento en la respuesta. La clave foránea no protege — es global.
    #
    # `obtener()` da 404 y nunca 403, que es la regla dura: un 403 confirmaría
    # que el id existe y con eso se enumera el padrón de al lado.
    obtener(db, Paciente, payload.paciente_id, current_user, que="paciente")
    _exigir_del_ambito(db, payload.cirujano_id, current_user, "cirujano")
    if payload.anestesiologo_id is not None:
        _exigir_del_ambito(db, payload.anestesiologo_id, current_user, "anestesiólogo")

    sala = _sala_del_ambito(db, payload.quirofano_id, current_user)
    if sala.estado in ("MANTENIMIENTO", "BLOQUEADO"):
        raise HTTPException(
            status_code=400,
            detail=f"La sala no está disponible (estado: {sala.estado})",
        )
    prog = ProgramacionQuirurgica(
        quirofano_id=payload.quirofano_id,
        paciente_id=payload.paciente_id,
        cirujano_id=payload.cirujano_id,
        anestesiologo_id=payload.anestesiologo_id,
        fecha_programada=payload.fecha_programada,
        duracion_estimada_min=payload.duracion_estimada_min,
        procedimiento=payload.procedimiento,
        codigo_cups=payload.codigo_cups,
        tipo_anestesia=payload.tipo_anestesia,
    )
    db.add(prog)
    db.commit()
    db.refresh(prog)
    return _enrich(prog, db)


@router.put("/programaciones/{prog_id}/checklist", response_model=ProgramacionRead)
def update_checklist(
    prog_id: int,
    payload: ChecklistUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    prog = obtener(db, ProgramacionQuirurgica, prog_id, current_user, que="programación quirúrgica")
    if payload.ayuno_verificado is not None:
        prog.ayuno_verificado = payload.ayuno_verificado
    if payload.labs_prequirurgicos is not None:
        prog.labs_prequirurgicos = payload.labs_prequirurgicos
    if payload.valoracion_preanestesica is not None:
        prog.valoracion_preanestesica = payload.valoracion_preanestesica
    if payload.hemoclasificacion is not None:
        prog.hemoclasificacion = payload.hemoclasificacion
    if payload.reserva_sangre is not None:
        prog.reserva_sangre = payload.reserva_sangre
    db.add(prog)
    db.commit()
    db.refresh(prog)
    return _enrich(prog, db)


@router.put("/programaciones/{prog_id}/iniciar", response_model=ProgramacionRead)
def iniciar_cirugia(
    prog_id: int,
    payload: IniciarCirugia,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    prog = obtener(db, ProgramacionQuirurgica, prog_id, current_user, que="programación quirúrgica")
    if prog.estado != "PROGRAMADA":
        raise HTTPException(
            status_code=400,
            detail=f"Solo se puede iniciar una cirugía en estado PROGRAMADA (actual: {prog.estado})",
        )
    prog.estado = "EN_CURSO"
    prog.hora_inicio_real = payload.hora_inicio or datetime.utcnow()
    sala = db.get(Quirofano, prog.quirofano_id)  # aislamiento: ya autorizado por el padre
    if sala:
        sala.estado = "EN_USO"
        db.add(sala)
    db.add(prog)
    db.commit()
    db.refresh(prog)
    return _enrich(prog, db)


@router.put("/programaciones/{prog_id}/completar", response_model=ProgramacionRead)
def completar_cirugia(
    prog_id: int,
    payload: CompletarCirugia,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    prog = obtener(db, ProgramacionQuirurgica, prog_id, current_user, que="programación quirúrgica")
    if prog.estado != "EN_CURSO":
        raise HTTPException(
            status_code=400,
            detail=f"Solo se puede completar una cirugía en estado EN_CURSO (actual: {prog.estado})",
        )
    prog.estado = "COMPLETADA"
    prog.hora_fin_real = payload.hora_fin or datetime.utcnow()
    if payload.nota_quirurgica is not None:
        prog.nota_quirurgica = payload.nota_quirurgica
    if payload.hallazgos_quirurgicos is not None:
        prog.hallazgos_quirurgicos = payload.hallazgos_quirurgicos
    if payload.sangrado_estimado_ml is not None:
        prog.sangrado_estimado_ml = payload.sangrado_estimado_ml
    if payload.especimenes_patologia is not None:
        prog.especimenes_patologia = payload.especimenes_patologia
    if payload.complicaciones is not None:
        prog.complicaciones = payload.complicaciones
    sala = db.get(Quirofano, prog.quirofano_id)  # aislamiento: ya autorizado por el padre
    if sala and sala.estado == "EN_USO":
        sala.estado = "DISPONIBLE"
        db.add(sala)
    db.add(prog)
    db.commit()
    db.refresh(prog)
    return _enrich(prog, db)


@router.put("/programaciones/{prog_id}/cancelar", response_model=ProgramacionRead)
def cancelar_cirugia(
    prog_id: int,
    payload: CancelarCirugia,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    prog = obtener(db, ProgramacionQuirurgica, prog_id, current_user, que="programación quirúrgica")
    if prog.estado in ("COMPLETADA", "CANCELADA"):
        raise HTTPException(
            status_code=400,
            detail=f"No se puede cancelar una cirugía en estado {prog.estado}",
        )
    prog.estado = "CANCELADA"
    if payload.motivo:
        prog.complicaciones = f"Cancelada: {payload.motivo}"
    if prog.hora_inicio_real:
        sala = db.get(Quirofano, prog.quirofano_id)  # aislamiento: ya autorizado por el padre
        if sala and sala.estado == "EN_USO":
            sala.estado = "DISPONIBLE"
            db.add(sala)
    db.add(prog)
    db.commit()
    db.refresh(prog)
    return _enrich(prog, db)
