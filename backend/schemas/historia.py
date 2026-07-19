from typing import Optional, Dict, Any, List
from datetime import date, datetime
from pydantic import BaseModel
from models import FinalidadConsulta, CausaExterna, TipoDiagnostico


class PacienteInfo(BaseModel):
    id: int
    identificacion: str
    tipo_id: str
    nombre_completo: str
    fecha_nacimiento: date
    sexo: str
    edad: int
    celular: Optional[str] = None
    entidad_nombre: Optional[str] = None
    ant_familiares_struct: Optional[Dict[str, Any]] = None
    ant_personales_struct: Optional[Dict[str, Any]] = None
    ant_ginecologicos_struct: Optional[Dict[str, Any]] = None
    ant_obstetricos_struct: Optional[Dict[str, Any]] = None


class MedicoInfo(BaseModel):
    id: int
    nombre_completo: str
    especialidad: Optional[str] = None
    registro_medico: Optional[str] = None


class AntecedentesUpdate(BaseModel):
    ant_familiares_struct: Optional[Dict[str, Any]] = None
    ant_personales_struct: Optional[Dict[str, Any]] = None
    ant_ginecologicos_struct: Optional[Dict[str, Any]] = None
    ant_obstetricos_struct: Optional[Dict[str, Any]] = None


def _sv_fields_create():
    return dict(
        peso=None, talla=None, imc=None, peso_ideal=None, estado_nutricional=None,
        pam=None, pulso=None, frecuencia_cardiaca=None, frecuencia_respiratoria=None,
        temperatura=None, pabd=None, pc=None, tension_arterial=None,
    )

def _ef_fields():
    return dict(
        ef_cabeza=None, ef_ojos=None, ef_nariz=None, ef_oidos=None,
        ef_boca=None, ef_faringe=None, ef_cuello=None, ef_pulmones=None,
        ef_corazon=None, ef_abdomen=None, ef_gu=None, ef_neurologico=None,
        ef_extremidad=None, ef_osteomuscular=None, ef_piel=None, ef_hallazgos=None,
    )


class HistoriaCreate(BaseModel):
    paciente_id: int
    cita_id: Optional[int] = None
    motivo_consulta: str = "En proceso de atención"
    enfermedad_actual: str = "En proceso de atención"
    impresion_diagnostica: Optional[str] = None
    revision_sistemas: Optional[str] = None
    examen_fisico: Optional[str] = None
    # Signos vitales
    peso: Optional[float] = None
    talla: Optional[float] = None
    imc: Optional[float] = None
    peso_ideal: Optional[float] = None
    estado_nutricional: Optional[str] = None
    pam: Optional[str] = None
    pulso: Optional[int] = None
    frecuencia_cardiaca: Optional[int] = None
    frecuencia_respiratoria: Optional[int] = None
    temperatura: Optional[float] = None
    pabd: Optional[float] = None
    pc: Optional[float] = None
    tension_arterial: Optional[str] = None
    # Examen físico por sistemas
    ef_cabeza: Optional[str] = None
    ef_ojos: Optional[str] = None
    ef_nariz: Optional[str] = None
    ef_oidos: Optional[str] = None
    ef_boca: Optional[str] = None
    ef_faringe: Optional[str] = None
    ef_cuello: Optional[str] = None
    ef_pulmones: Optional[str] = None
    ef_corazon: Optional[str] = None
    ef_abdomen: Optional[str] = None
    ef_gu: Optional[str] = None
    ef_neurologico: Optional[str] = None
    ef_extremidad: Optional[str] = None
    ef_osteomuscular: Optional[str] = None
    ef_piel: Optional[str] = None
    ef_hallazgos: Optional[str] = None
    # Diagnóstico y plan
    diagnostico_principal: str = "Z00.0"
    diagnostico_relacionado1: Optional[str] = None
    cod_consulta_cups: Optional[str] = "890201"
    plan_tratamiento: str = "Por definir"
    finalidad: Optional[FinalidadConsulta] = None
    causa_externa: Optional[CausaExterna] = None
    tipo_diagnostico: Optional[TipoDiagnostico] = None
    datos_dinamicos: Optional[Dict[str, Any]] = None
    genera_incapacidad: bool = False
    dias_incapacidad: int = 0


class HistoriaUpdate(BaseModel):
    motivo_consulta: Optional[str] = None
    enfermedad_actual: Optional[str] = None
    impresion_diagnostica: Optional[str] = None
    revision_sistemas: Optional[str] = None
    examen_fisico: Optional[str] = None
    # Signos vitales
    peso: Optional[float] = None
    talla: Optional[float] = None
    imc: Optional[float] = None
    peso_ideal: Optional[float] = None
    estado_nutricional: Optional[str] = None
    pam: Optional[str] = None
    pulso: Optional[int] = None
    frecuencia_cardiaca: Optional[int] = None
    frecuencia_respiratoria: Optional[int] = None
    temperatura: Optional[float] = None
    pabd: Optional[float] = None
    pc: Optional[float] = None
    tension_arterial: Optional[str] = None
    # Examen físico por sistemas
    ef_cabeza: Optional[str] = None
    ef_ojos: Optional[str] = None
    ef_nariz: Optional[str] = None
    ef_oidos: Optional[str] = None
    ef_boca: Optional[str] = None
    ef_faringe: Optional[str] = None
    ef_cuello: Optional[str] = None
    ef_pulmones: Optional[str] = None
    ef_corazon: Optional[str] = None
    ef_abdomen: Optional[str] = None
    ef_gu: Optional[str] = None
    ef_neurologico: Optional[str] = None
    ef_extremidad: Optional[str] = None
    ef_osteomuscular: Optional[str] = None
    ef_piel: Optional[str] = None
    ef_hallazgos: Optional[str] = None
    # Diagnóstico y plan
    diagnostico_principal: Optional[str] = None
    diagnostico_relacionado1: Optional[str] = None
    plan_tratamiento: Optional[str] = None
    finalidad: Optional[FinalidadConsulta] = None
    causa_externa: Optional[CausaExterna] = None
    tipo_diagnostico: Optional[TipoDiagnostico] = None
    datos_dinamicos: Optional[Dict[str, Any]] = None
    genera_incapacidad: Optional[bool] = None
    dias_incapacidad: Optional[int] = None


class HistoriaRead(BaseModel):
    id: int
    paciente_id: int
    medico_id: int
    cita_id: Optional[int] = None
    codigo_historia: Optional[str] = None
    motivo_consulta: Optional[str] = None
    enfermedad_actual: Optional[str] = None
    impresion_diagnostica: Optional[str] = None
    revision_sistemas: Optional[str] = None
    examen_fisico: Optional[str] = None
    # Signos vitales
    peso: Optional[float] = None
    talla: Optional[float] = None
    imc: Optional[float] = None
    peso_ideal: Optional[float] = None
    estado_nutricional: Optional[str] = None
    pam: Optional[str] = None
    pulso: Optional[int] = None
    frecuencia_cardiaca: Optional[int] = None
    frecuencia_respiratoria: Optional[int] = None
    temperatura: Optional[float] = None
    pabd: Optional[float] = None
    pc: Optional[float] = None
    tension_arterial: Optional[str] = None
    # Examen físico por sistemas
    ef_cabeza: Optional[str] = None
    ef_ojos: Optional[str] = None
    ef_nariz: Optional[str] = None
    ef_oidos: Optional[str] = None
    ef_boca: Optional[str] = None
    ef_faringe: Optional[str] = None
    ef_cuello: Optional[str] = None
    ef_pulmones: Optional[str] = None
    ef_corazon: Optional[str] = None
    ef_abdomen: Optional[str] = None
    ef_gu: Optional[str] = None
    ef_neurologico: Optional[str] = None
    ef_extremidad: Optional[str] = None
    ef_osteomuscular: Optional[str] = None
    ef_piel: Optional[str] = None
    ef_hallazgos: Optional[str] = None
    # Diagnóstico y plan
    diagnostico_principal: Optional[str] = None
    diagnostico_relacionado1: Optional[str] = None
    plan_tratamiento: Optional[str] = None
    finalidad: Optional[str] = None
    causa_externa: Optional[str] = None
    tipo_diagnostico: Optional[str] = None
    datos_dinamicos: Optional[Dict[str, Any]] = None
    genera_incapacidad: Optional[bool] = None
    dias_incapacidad: Optional[int] = None
    fecha: Optional[datetime] = None
    firmada: bool = False
    paciente: Optional[PacienteInfo] = None
    medico: Optional[MedicoInfo] = None

    class Config:
        from_attributes = True


class PacienteContexto(BaseModel):
    """Contexto del paciente para crear una nueva historia."""
    paciente: PacienteInfo
    medico: Optional[MedicoInfo] = None


# ── Fórmula Médica ──────────────────────────────────────────────
class FormulaCreate(BaseModel):
    medicamento: str
    codigo_cum: Optional[str] = None
    dosis: str
    cantidad: int
    unidad: Optional[str] = "unidad"
    observaciones: Optional[str] = None


class FormulaUpdate(BaseModel):
    medicamento: Optional[str] = None
    codigo_cum: Optional[str] = None
    dosis: Optional[str] = None
    cantidad: Optional[int] = None
    unidad: Optional[str] = None
    observaciones: Optional[str] = None


class FormulaRead(BaseModel):
    id: int
    historia_id: int
    medicamento: str
    codigo_cum: Optional[str] = None
    dosis: str
    cantidad: int
    unidad: Optional[str] = None
    observaciones: Optional[str] = None
    fecha_formula: datetime
    dispensada: bool

    class Config:
        from_attributes = True


# ── Medicamento (catálogo INVIMA) ────────────────────────────────
class MedicamentoInfo(BaseModel):
    id: int
    codigo_cum: str
    nombre: str
    concentracion: str
    forma_farmaceutica: str
    titular: Optional[str] = None

    class Config:
        from_attributes = True


# ── Especialidad (dropdown en órdenes de remisión) ───────────────
class EspecialidadInfo(BaseModel):
    id: int
    nombre: str

    class Config:
        from_attributes = True


# ── Órdenes médicas (laboratorio, imagenología, remisiones) ──────
class OrdenCreate(BaseModel):
    tipo: str
    descripcion: str
    justificacion: str
    codigo_cups: Optional[str] = None
    especialidad_destino_id: Optional[int] = None
    institucion_destino_id: Optional[int] = None


class OrdenUpdate(BaseModel):
    estado: Optional[str] = None
    descripcion: Optional[str] = None
    justificacion: Optional[str] = None
    codigo_cups: Optional[str] = None


class OrdenRead(BaseModel):
    id: int
    historia_id: int
    paciente_id: Optional[int] = None
    medico_ordena_id: Optional[int] = None
    fecha_orden: datetime
    estado: str
    tipo: str
    codigo_cups: Optional[str] = None
    descripcion: str
    justificacion: str
    especialidad_destino_id: Optional[int] = None
    especialidad_nombre: Optional[str] = None

    class Config:
        from_attributes = True


# ── Imágenes diagnósticas ────────────────────────────────────────
class ImagenDiagnosticaRead(BaseModel):
    id: int
    historia_id: int
    ruta_archivo: str
    nombre_original: str
    descripcion: str
    fecha_subida: datetime

    class Config:
        from_attributes = True
