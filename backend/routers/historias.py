from typing import Optional
from fastapi import APIRouter, Depends, HTTPException, UploadFile, File, Query, Form
from sqlmodel import Session, select
from models import (
    Historia, Usuario, FirmaDigital, Paciente, Cita, Formula, Medicamento,
    Orden, ImagenDiagnostica, TipoOrden, EstadoOrden, Especialidad,
)
from schemas.historia import (
    HistoriaCreate, HistoriaUpdate, HistoriaRead,
    PacienteInfo, MedicoInfo, PacienteContexto,
    FormulaCreate, FormulaUpdate, FormulaRead, MedicamentoInfo,
    OrdenCreate, OrdenUpdate, OrdenRead,
    ImagenDiagnosticaRead, EspecialidadInfo,
)
from models import ModuloSistema
from core.aislamiento import obtener
from core.deps import get_db, get_current_active_user, requiere_modulo
import os, uuid as _uuid
from core.config import UPLOAD_DIR

router = APIRouter(prefix="/historias", tags=["historias"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.HISTORIA))],
)


def _build_paciente_info(paciente: Paciente) -> PacienteInfo:
    entidad_nombre = None
    if paciente.entidad:
        entidad_nombre = paciente.entidad.nombre
    return PacienteInfo(
        id=paciente.id,
        identificacion=paciente.identificacion,
        tipo_id=paciente.tipo_id,
        nombre_completo=paciente.nombre_completo.strip(),
        fecha_nacimiento=paciente.fecha_nacimiento,
        sexo=paciente.sexo,
        edad=paciente.edad,
        celular=paciente.celular,
        entidad_nombre=entidad_nombre,
        ant_familiares_struct=paciente.ant_familiares_struct,
        ant_personales_struct=paciente.ant_personales_struct,
        ant_ginecologicos_struct=paciente.ant_ginecologicos_struct,
        ant_obstetricos_struct=paciente.ant_obstetricos_struct,
    )


def _build_medico_info(medico: Usuario) -> MedicoInfo:
    especialidad = None
    if medico.especialidad:
        especialidad = medico.especialidad.nombre
    return MedicoInfo(
        id=medico.id,
        nombre_completo=medico.nombre_completo,
        especialidad=especialidad,
        registro_medico=medico.registro_medico,
    )


_HISTORIA_RELATIONS = {'paciente', 'medico', 'cita', 'formulas', 'ordenes'}

def _enrich_historia(historia: Historia, db: Session) -> HistoriaRead:
    data = HistoriaRead.model_validate(
        historia.model_dump(exclude=_HISTORIA_RELATIONS)
    )

    paciente = db.get(Paciente, historia.paciente_id)  # aislamiento: ya autorizado por el padre
    if paciente:
        data.paciente = _build_paciente_info(paciente)

    # Use medico from the associated cita if available, else from historia.medico_id
    medico_id = historia.medico_id
    if historia.cita_id:
        cita = db.get(Cita, historia.cita_id)  # aislamiento: ya autorizado por el padre
        if cita:
            medico_id = cita.medico_id

    medico = db.get(Usuario, medico_id)  # aislamiento: ya autorizado por el padre
    if medico:
        data.medico = _build_medico_info(medico)

    firma = db.exec(select(FirmaDigital).where(FirmaDigital.historia_id == historia.id)).first()
    data.firmada = firma is not None

    return data


@router.post("/", response_model=HistoriaRead, status_code=201)
def create_historia(
    payload: HistoriaCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    historia = Historia(
        **payload.model_dump(),
        medico_id=current_user.id,
        codigo_historia=f"HC-{_uuid.uuid4().hex[:8].upper()}",
    )
    db.add(historia)
    db.commit()
    db.refresh(historia)
    return _enrich_historia(historia, db)


@router.get("/unidades-dispensacion")
def unidades_dispensacion(
    current_user: Usuario = Depends(get_current_active_user),
):
    """Las presentaciones del catálogo del MUV (campo M13).

    Se sirven desde aquí y no se copian al frontend: una lista repetida en dos
    sitios se desincroniza, y de este código dependen el valor unitario y la
    cantidad que el MUV valida al lado.

    Son las 73 habilitadas de `REFs_UPR`, extraídas del propio validador. Antes
    eran cinco escritas a mano y con los códigos equivocados.
    """
    import maestros_muv

    return maestros_muv.nombres_presentacion()


@router.get("/paciente/{paciente_id}", response_model=list[HistoriaRead])
def list_historias_paciente(
    paciente_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    """Lista todas las historias clínicas de un paciente, ordenadas de más reciente a más antigua."""
    historias = db.exec(
        select(Historia).where(Historia.paciente_id == paciente_id).order_by(Historia.fecha.desc())
    ).all()
    return [_enrich_historia(h, db) for h in historias]


@router.get("/paciente/{paciente_id}/contexto", response_model=PacienteContexto)
def get_paciente_contexto(
    paciente_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    """Carga datos del paciente + antecedentes para iniciar una nueva historia."""
    paciente = obtener(db, Paciente, paciente_id, current_user, que="paciente")
    return PacienteContexto(
        paciente=_build_paciente_info(paciente),
        medico=_build_medico_info(current_user),
    )


# ══════════════════════════════════════════════════════════════════
# ESPECIALIDADES (dropdown para órdenes de remisión)
# ══════════════════════════════════════════════════════════════════

@router.get("/especialidades", response_model=list[EspecialidadInfo])
def list_especialidades(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    return db.exec(select(Especialidad).order_by(Especialidad.nombre)).all()


@router.get("/{historia_id}", response_model=HistoriaRead)
def get_historia(
    historia_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    historia = obtener(db, Historia, historia_id, current_user, que="historia")
    return _enrich_historia(historia, db)


@router.put("/{historia_id}", response_model=HistoriaRead)
def update_historia(
    historia_id: int,
    payload: HistoriaUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    historia = obtener(db, Historia, historia_id, current_user, que="historia")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(historia, field, value)
    db.add(historia)
    db.commit()
    db.refresh(historia)
    return _enrich_historia(historia, db)


@router.post("/{historia_id}/firmar")
def firmar_historia(
    historia_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    historia = obtener(db, Historia, historia_id, current_user, que="historia")
    import hashlib, json
    data = {"id": historia.id, "paciente_id": historia.paciente_id, "medico_id": historia.medico_id}
    hash_val = hashlib.sha256(json.dumps(data, sort_keys=True).encode()).hexdigest()
    firma = FirmaDigital(
        historia_id=historia_id,
        medico_id=current_user.id,
        hash_documento=hash_val,
    )
    db.add(firma)
    db.commit()
    return {"message": "Historia firmada", "hash": hash_val}


@router.delete("/{historia_id}", status_code=204)
def delete_historia(
    historia_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    historia = obtener(db, Historia, historia_id, current_user, que="historia")
    firma = db.exec(select(FirmaDigital).where(FirmaDigital.historia_id == historia_id)).first()
    if firma:
        raise HTTPException(status_code=400, detail="No se puede eliminar una historia firmada digitalmente")
    db.delete(historia)
    db.commit()


@router.get("/{historia_id}/imagenes-diagnosticas", response_model=list[ImagenDiagnosticaRead])
def list_imagenes(
    historia_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    historia = obtener(db, Historia, historia_id, current_user, que="historia")
    return db.exec(
        select(ImagenDiagnostica)
        .where(ImagenDiagnostica.historia_id == historia_id)
        .order_by(ImagenDiagnostica.fecha_subida)
    ).all()


@router.post("/{historia_id}/imagenes", response_model=ImagenDiagnosticaRead, status_code=201)
async def upload_imagen(
    historia_id: int,
    file: UploadFile = File(...),
    descripcion: str = Form("Sin descripción"),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    historia = obtener(db, Historia, historia_id, current_user, que="historia")
    ext = os.path.splitext(file.filename or "")[1]
    filename = f"{_uuid.uuid4()}{ext}"
    filepath = os.path.join(UPLOAD_DIR, "imagenes", filename)
    os.makedirs(os.path.dirname(filepath), exist_ok=True)
    content = await file.read()
    with open(filepath, "wb") as f:
        f.write(content)
    imagen = ImagenDiagnostica(
        historia_id=historia_id,
        ruta_archivo=f"/uploads/imagenes/{filename}",
        nombre_original=file.filename or filename,
        descripcion=descripcion,
    )
    db.add(imagen)
    db.commit()
    db.refresh(imagen)
    return imagen


@router.delete("/{historia_id}/imagenes-diagnosticas/{imagen_id}", status_code=204)
def delete_imagen(
    historia_id: int,
    imagen_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    imagen = db.exec(
        select(ImagenDiagnostica).where(
            ImagenDiagnostica.id == imagen_id,
            ImagenDiagnostica.historia_id == historia_id,
        )
    ).first()
    if not imagen:
        raise HTTPException(status_code=404, detail="Imagen no encontrada")
    filepath = os.path.join(UPLOAD_DIR, "imagenes", os.path.basename(imagen.ruta_archivo))
    if os.path.exists(filepath):
        os.remove(filepath)
    db.delete(imagen)
    db.commit()


# ══════════════════════════════════════════════════════════════════
# CATÁLOGO DE MEDICAMENTOS (INVIMA)
# ══════════════════════════════════════════════════════════════════

@router.get("/medicamentos/buscar", response_model=list[MedicamentoInfo])
def buscar_medicamentos(
    q: str = Query(..., min_length=2),
    limit: int = Query(15, le=50),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    """Busca medicamentos del catálogo INVIMA por nombre, código CUM o forma farmacéutica."""
    query = select(Medicamento).where(
        Medicamento.nombre.ilike(f"%{q}%") |
        Medicamento.codigo_cum.ilike(f"%{q}%") |
        Medicamento.forma_farmaceutica.ilike(f"%{q}%")
    ).limit(limit)
    return db.exec(query).all()


# ══════════════════════════════════════════════════════════════════
# FÓRMULAS MÉDICAS
# ══════════════════════════════════════════════════════════════════

@router.get("/{historia_id}/formulas", response_model=list[FormulaRead])
def list_formulas(
    historia_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    historia = obtener(db, Historia, historia_id, current_user, que="historia")
    return db.exec(select(Formula).where(Formula.historia_id == historia_id).order_by(Formula.fecha_formula)).all()


@router.post("/{historia_id}/formulas", response_model=FormulaRead, status_code=201)
def create_formula(
    historia_id: int,
    payload: FormulaCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    historia = obtener(db, Historia, historia_id, current_user, que="historia")
    firma = db.exec(select(FirmaDigital).where(FirmaDigital.historia_id == historia_id)).first()
    if firma:
        raise HTTPException(status_code=400, detail="No se puede modificar una historia firmada")
    formula = Formula(historia_id=historia_id, **payload.model_dump())
    db.add(formula)
    db.commit()
    db.refresh(formula)
    return formula


@router.put("/{historia_id}/formulas/{formula_id}", response_model=FormulaRead)
def update_formula(
    historia_id: int,
    formula_id: int,
    payload: FormulaUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    formula = db.exec(select(Formula).where(Formula.id == formula_id, Formula.historia_id == historia_id)).first()
    if not formula:
        raise HTTPException(status_code=404, detail="Fórmula no encontrada")
    if formula.dispensada:
        raise HTTPException(status_code=400, detail="No se puede editar una fórmula ya dispensada")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(formula, field, value)
    db.add(formula)
    db.commit()
    db.refresh(formula)
    return formula


@router.delete("/{historia_id}/formulas/{formula_id}", status_code=204)
def delete_formula(
    historia_id: int,
    formula_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    formula = db.exec(select(Formula).where(Formula.id == formula_id, Formula.historia_id == historia_id)).first()
    if not formula:
        raise HTTPException(status_code=404, detail="Fórmula no encontrada")
    if formula.dispensada:
        raise HTTPException(status_code=400, detail="No se puede eliminar una fórmula ya dispensada")
    db.delete(formula)
    db.commit()


# ══════════════════════════════════════════════════════════════════
# ÓRDENES MÉDICAS (Laboratorio, Imagenología, Remisiones)
# ══════════════════════════════════════════════════════════════════

def _enrich_orden(orden: Orden, db: Session) -> OrdenRead:
    r = OrdenRead.model_validate(orden)
    if orden.especialidad_destino_id:
        esp = db.get(Especialidad, orden.especialidad_destino_id)  # aislamiento: ya autorizado por el padre
        if esp:
            r.especialidad_nombre = esp.nombre
    return r


@router.get("/{historia_id}/ordenes", response_model=list[OrdenRead])
def list_ordenes(
    historia_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    historia = obtener(db, Historia, historia_id, current_user, que="historia")
    ordenes = db.exec(
        select(Orden).where(Orden.historia_id == historia_id).order_by(Orden.fecha_orden)
    ).all()
    return [_enrich_orden(o, db) for o in ordenes]


@router.post("/{historia_id}/ordenes", response_model=OrdenRead, status_code=201)
def create_orden(
    historia_id: int,
    payload: OrdenCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    historia = obtener(db, Historia, historia_id, current_user, que="historia")
    firma = db.exec(select(FirmaDigital).where(FirmaDigital.historia_id == historia_id)).first()
    if firma:
        raise HTTPException(status_code=400, detail="No se puede modificar una historia firmada")
    orden = Orden(
        historia_id=historia_id,
        paciente_id=historia.paciente_id,
        medico_ordena_id=current_user.id,
        **payload.model_dump(),
    )
    db.add(orden)
    db.commit()
    db.refresh(orden)
    return _enrich_orden(orden, db)


@router.put("/{historia_id}/ordenes/{orden_id}", response_model=OrdenRead)
def update_orden(
    historia_id: int,
    orden_id: int,
    payload: OrdenUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    orden = db.exec(
        select(Orden).where(Orden.id == orden_id, Orden.historia_id == historia_id)
    ).first()
    if not orden:
        raise HTTPException(status_code=404, detail="Orden no encontrada")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(orden, field, value)
    db.add(orden)
    db.commit()
    db.refresh(orden)
    return _enrich_orden(orden, db)


@router.delete("/{historia_id}/ordenes/{orden_id}", status_code=204)
def delete_orden(
    historia_id: int,
    orden_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    orden = db.exec(
        select(Orden).where(Orden.id == orden_id, Orden.historia_id == historia_id)
    ).first()
    if not orden:
        raise HTTPException(status_code=404, detail="Orden no encontrada")
    firma = db.exec(select(FirmaDigital).where(FirmaDigital.historia_id == historia_id)).first()
    if firma:
        raise HTTPException(status_code=400, detail="No se puede modificar una historia firmada")
    db.delete(orden)
    db.commit()
