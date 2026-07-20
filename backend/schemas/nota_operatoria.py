from datetime import datetime
from typing import Optional

from pydantic import BaseModel


class NotaOperatoriaCreate(BaseModel):
    programacion_id: int
    paciente_id: int
    ayudante_id: Optional[int] = None
    anestesiologo_id: Optional[int] = None
    instrumentadora: Optional[str] = None
    circulante: Optional[str] = None
    diagnostico_preoperatorio: str
    diagnostico_postoperatorio: str
    procedimiento_realizado: str
    codigo_cups: Optional[str] = None
    tipo_anestesia: Optional[str] = None
    hora_inicio: datetime
    hora_fin: datetime
    descripcion_tecnica: str
    hallazgos: Optional[str] = None
    #: Sin defecto: «no se midió» y «no sangró» son cosas distintas.
    sangrado_estimado_ml: Optional[int] = None
    transfusiones: Optional[str] = None
    drenajes: Optional[str] = None
    especimenes_patologia: Optional[str] = None
    complicaciones: Optional[str] = None
    plan_postoperatorio: str
    #: Obligatorio y sin defecto: es una afirmación clínica que alguien firma.
    estado_paciente_salida: str


class NotaOperatoriaRead(NotaOperatoriaCreate):
    id: int
    cirujano_id: int
    fecha_creacion: datetime
    firma_cirujano_path: Optional[str] = None

    paciente: Optional[str] = None
    cirujano: Optional[str] = None
    #: Derivado de las horas, no escrito a mano.
    tiempo_quirurgico_min: int = 0
    #: Lo que conviene mirar. No bloquea.
    avisos: list[str] = []
