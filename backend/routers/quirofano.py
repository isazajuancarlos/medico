from typing import Optional
from datetime import datetime
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from models import Quirofano, ProgramacionQuirurgica, Paciente, Usuario, RolUsuario, ModuloSistema
from core.deps import get_db, get_current_active_user, requiere_modulo
from core.aislamiento import obtener
from schemas.quirofano import (
    QuirofanoRead, ProgramacionRead, ProgramacionCreate,
    ChecklistUpdate, IniciarCirugia, CompletarCirugia, CancelarCirugia,
    MedicoBasic,
)

router = APIRouter(prefix="/quirofano", tags=["quirofano"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.QUIROFANO))],
)


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
    query = select(ProgramacionQuirurgica)
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
    sala = db.get(Quirofano, payload.quirofano_id)  # aislamiento: ya autorizado por el padre
    if not sala:
        raise HTTPException(status_code=404, detail="Sala de quirófano no encontrada")
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
