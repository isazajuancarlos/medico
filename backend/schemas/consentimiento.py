from datetime import datetime
from typing import Optional

from pydantic import BaseModel


class ConsentimientoBase(BaseModel):
    historia_id: int
    paciente_id: int
    tipo_procedimiento: str
    codigo_cups: Optional[str] = None
    descripcion_procedimiento: str
    riesgos_explicados: str
    beneficios_esperados: str
    alternativas_explicadas: str
    consecuencias_no_realizarlo: Optional[str] = None
    paciente_comprende: bool = False
    acepta_procedimiento: bool = False
    firma_paciente_path: Optional[str] = None
    firma_medico_path: Optional[str] = None
    testigo_nombre: Optional[str] = None
    testigo_documento: Optional[str] = None


class ConsentimientoCreate(ConsentimientoBase):
    pass


class ConsentimientoUpdate(BaseModel):
    tipo_procedimiento: Optional[str] = None
    codigo_cups: Optional[str] = None
    descripcion_procedimiento: Optional[str] = None
    riesgos_explicados: Optional[str] = None
    beneficios_esperados: Optional[str] = None
    alternativas_explicadas: Optional[str] = None
    consecuencias_no_realizarlo: Optional[str] = None
    paciente_comprende: Optional[bool] = None
    acepta_procedimiento: Optional[bool] = None
    firma_paciente_path: Optional[str] = None
    firma_medico_path: Optional[str] = None
    testigo_nombre: Optional[str] = None
    testigo_documento: Optional[str] = None


class Revocacion(BaseModel):
    motivo: str


class ConsentimientoRead(ConsentimientoBase):
    id: int
    fecha_firma: datetime
    medico_id: int
    revocado: bool = False
    fecha_revocacion: Optional[datetime] = None
    motivo_revocacion: Optional[str] = None

    paciente: Optional[str] = None
    medico: Optional[str] = None

    #: El juicio, calculado. Que la pantalla no lo recalcule: la regla vive en
    #: `consentimientos.py` y en un solo sitio.
    valido: bool = False
    faltantes: list[str] = []
    resumen: str = ""
