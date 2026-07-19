from typing import Optional
from datetime import datetime
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from models import AtencionUrgencias, TipoTriage, DestinoSalidaUrgencias, Usuario
from core.deps import get_db, get_current_active_user
from schemas.common import PaginatedResponse
from pydantic import BaseModel

router = APIRouter(prefix="/urgencias", tags=["urgencias"])


class TriageCreate(BaseModel):
    paciente_id: int
    triage: TipoTriage
    motivo_consulta: str
    signos_vitales: Optional[dict] = None


class TriageRead(BaseModel):
    id: int
    paciente_id: int
    triage: TipoTriage
    motivo_consulta: str
    fecha_ingreso: Optional[datetime] = None
    estado: Optional[str] = None

    class Config:
        from_attributes = True


@router.get("/cola", response_model=list[TriageRead])
def cola_triage(db: Session = Depends(get_db), current_user: Usuario = Depends(get_current_active_user)):
    try:
        pacientes = db.exec(
            select(AtencionUrgencias)
            .where(AtencionUrgencias.destino_salida == None)
            .order_by(AtencionUrgencias.triage, AtencionUrgencias.fecha_ingreso)
        ).all()
        result = []
        for p in pacientes:
            result.append(TriageRead(
                id=p.id,
                paciente_id=p.paciente_id,
                triage=p.triage,
                motivo_consulta=p.motivo_consulta_triage,
                fecha_ingreso=p.fecha_ingreso,
                estado="EN_ESPERA" if not p.destino_salida else "EGRESADO",
            ))
        return result
    except Exception:
        return []


@router.post("/triage", status_code=201)
def registrar_triage(
    payload: TriageCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    atencion = AtencionUrgencias(
        paciente_id=payload.paciente_id,
        triage=payload.triage,
        motivo_consulta_triage=payload.motivo_consulta,
        signos_vitales_triage=payload.signos_vitales or {},
        enfermera_triage_id=current_user.id,
        diagnostico_ingreso="Z00.0",  # placeholder
    )
    db.add(atencion)
    db.commit()
    db.refresh(atencion)
    return atencion


@router.put("/{urgencia_id}/egreso")
def egreso_urgencias(
    urgencia_id: int,
    destino: DestinoSalidaUrgencias,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    urg = db.get(AtencionUrgencias, urgencia_id)
    if not urg:
        raise HTTPException(status_code=404, detail="No encontrado")
    urg.destino_salida = destino
    urg.fecha_egreso = datetime.utcnow()
    db.add(urg)
    db.commit()
    return {"message": "Egreso registrado"}
