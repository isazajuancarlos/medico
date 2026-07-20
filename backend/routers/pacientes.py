from typing import Optional
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from database import engine
from models import Paciente, Historia, RolUsuario, ModuloSistema
from schemas.paciente import PacienteCreate, PacienteUpdate, PacienteRead
from schemas.common import PaginatedResponse
from schemas.historia import AntecedentesUpdate
from core.deps import get_db, get_current_active_user, filtrar_por_institucion, requiere_modulo
from core.aislamiento import obtener
from models import Usuario

router = APIRouter(prefix="/pacientes", tags=["pacientes"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.PACIENTES))],
)


@router.get("/", response_model=PaginatedResponse[PacienteRead])
def list_pacientes(
    q: Optional[str] = None,
    page: int = Query(1, ge=1),
    size: int = Query(20, ge=1, le=100),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(Paciente)
    query = filtrar_por_institucion(query, Paciente, current_user)
    if q:
        query = query.where(
            Paciente.nombre1.ilike(f"%{q}%") |
            Paciente.apellido1.ilike(f"%{q}%") |
            Paciente.identificacion.ilike(f"%{q}%")
        )
    total = len(db.exec(query).all())
    items = db.exec(query.offset((page - 1) * size).limit(size)).all()
    return PaginatedResponse(total=total, items=items, page=page, size=size)


@router.post("/", response_model=PacienteRead, status_code=201)
def create_paciente(
    payload: PacienteCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    existing = db.exec(select(Paciente).where(Paciente.identificacion == payload.identificacion)).first()
    if existing:
        raise HTTPException(status_code=400, detail="Ya existe un paciente con esta identificación")
    paciente = Paciente(**payload.model_dump(), institucion_id=current_user.institucion_id)
    db.add(paciente)
    db.commit()
    db.refresh(paciente)
    return paciente


@router.get("/buscar", response_model=list[PacienteRead])
def buscar_pacientes(
    q: str,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(Paciente).where(
        Paciente.nombre1.ilike(f"%{q}%") |
        Paciente.apellido1.ilike(f"%{q}%") |
        Paciente.identificacion.ilike(f"%{q}%")
    ).limit(20)
    query = filtrar_por_institucion(query, Paciente, current_user)
    return db.exec(query).all()


@router.get("/{paciente_id}", response_model=PacienteRead)
def get_paciente(
    paciente_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    paciente = obtener(db, Paciente, paciente_id, current_user, que="paciente")
    return paciente


@router.put("/{paciente_id}", response_model=PacienteRead)
def update_paciente(
    paciente_id: int,
    payload: PacienteUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    paciente = obtener(db, Paciente, paciente_id, current_user, que="paciente")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(paciente, field, value)
    db.add(paciente)
    db.commit()
    db.refresh(paciente)
    return paciente


@router.put("/{paciente_id}/antecedentes", response_model=PacienteRead)
def update_antecedentes(
    paciente_id: int,
    payload: AntecedentesUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    paciente = obtener(db, Paciente, paciente_id, current_user, que="paciente")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(paciente, field, value)
    db.add(paciente)
    db.commit()
    db.refresh(paciente)
    return paciente


@router.get("/{paciente_id}/historias")
def get_historias_paciente(
    paciente_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    from models import Historia
    # Primero el paciente: si es de otra institución, ni se consultan sus
    # historias. Filtrar después no sirve — la lista saldría igual de completa.
    obtener(db, Paciente, paciente_id, current_user, que="paciente")
    historias = db.exec(select(Historia).where(Historia.paciente_id == paciente_id)).all()
    return historias
