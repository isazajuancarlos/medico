from typing import Optional, List, Dict
from datetime import date, datetime
from sqlmodel import SQLModel, Field, Relationship
from sqlalchemy import Column, JSON
from enum import Enum

# --- ENUMS RIPS ---
class TipoUsuario(str, Enum):
    CONTRIBUTIVO = "01"
    SUBSIDIADO = "02"
    VINCULADO = "03"
    PARTICULAR = "04"
    OTRO = "05"

class FinalidadConsulta(str, Enum):
    NO_APLICA = "10"
    ATENCION_PARTO = "06"
    ATENCION_REC_NACIDO = "07"
    PLANIFICACION_FAMILIAR = "08"
    DETECCION_ALT_CRECIMIENTO = "09"
    DETECCION_ALT_JOVEN = "02"
    DETECCION_ALT_ADULTO = "03"
    DETECCION_ALT_AGUDEZA_VISUAL = "04"
    ENFERMEDAD_PROFESIONAL = "05"

class CausaExterna(str, Enum):
    ENFERMEDAD_GENERAL = "13"
    ACCIDENTE_TRABAJO = "01"
    ACCIDENTE_TRANSITO = "02"
    ACCIDENTE_RABICO = "03"
    ACCIDENTE_OFIDICO = "04"
    OTRO_ACCIDENTE = "05"
    EVENTO_CATASTROFICO = "06"
    LESION_AGRESION = "07"
    AUTOINFLIGIDA = "08"
    SOSPECHA_MALTRATO = "09"
    SOSPECHA_ABUSO = "10"
    SOSPECHA_VIOLENCIA_SEXUAL = "11"
    OTRA = "15"

class TipoDiagnostico(str, Enum):
    IMPRESION_DIAGNOSTICA = "01"
    CONFIRMADO_NUEVO = "02"
    CONFIRMADO_REPETIDO = "03"

class ZonaResidencia(str, Enum):
    URBANA = "01"
    RURAL = "02"

# --- OTROS ENUMS ---
class RolUsuario(str, Enum):
    SUPERADMIN = "superadmin"  # Gestión multi-institucional
    ADMIN = "admin"
    MEDICO = "medico"
    PACIENTE = "paciente"
    ENFERMERIA = "enfermeria"
    AUXILIAR_ENFERMERIA = "auxiliar_enfermeria"
    REGENTE_FARMACIA = "regente_farmacia"
    LABORATORISTA = "laboratorista"
    RADIOLOGO = "radiologo"
    ANESTESIOLOGO = "anestesiologo"
    TERAPEUTA = "terapeuta"
    NUTRICIONISTA = "nutricionista"
    PSICOLOGO = "psicologo"
    TRABAJO_SOCIAL = "trabajo_social"
    BACTERIOLOGO = "bacteriologo"

class TipoCita(str, Enum):
    PRIMERA_VEZ = "Primera Vez"
    CONTROL = "Control / Revisión"
    URGENCIA = "Urgencia"

class EstadoCita(str, Enum):
    PENDIENTE = "Pendiente"
    CONFIRMADA = "Confirmada"
    ATENDIDA = "Atendida"
    CANCELADA = "Cancelada"
    NO_ASISTIO = "No Asistió"

class TipoOrden(str, Enum):
    LABORATORIO = "Laboratorio"
    IMAGENOLOGIA = "Imagenología"
    ESPECIALISTA = "Remisión Especialista"
    CIRUGIA = "Cirugía"
    TERAPIA = "Terapia"
    OTRO = "Otro"

class EstadoOrden(str, Enum):
    PENDIENTE = "Pendiente"
    EN_PROCESO = "En Proceso"
    COMPLETADA = "Completada"
    CANCELADA = "Cancelada"

class TipoCampo(str, Enum):
    TEXTO = "Texto"
    NUMERO = "Número"
    BOOLEANO = "Si/No"
    SELECCION = "Lista Opciones"

class TipoTriage(str, Enum):
    """Sistema Manchester de Triage (usado en Colombia)"""
    TRIAGE_1 = "Rojo - Resucitación"
    TRIAGE_2 = "Naranja - Emergencia"
    TRIAGE_3 = "Amarillo - Urgente"
    TRIAGE_4 = "Verde - Menos Urgente"
    TRIAGE_5 = "Azul - No Urgente"

class DestinoSalidaUrgencias(str, Enum):
    DOMICILIO = "Domicilio"
    HOSPITALIZACION = "Hospitalización"
    REMISION = "Remisión"
    OBSERVACION = "Observación"
    FUGA = "Fuga"
    FALLECIDO = "Fallecido"

class PrioridadInterconsulta(str, Enum):
    URGENTE = "Urgente"
    NORMAL = "Normal"
    DIFERIBLE = "Diferible"

class EstadoInterconsulta(str, Enum):
    SOLICITADA = "Solicitada"
    ACEPTADA = "Aceptada"
    RESPONDIDA = "Respondida"
    CANCELADA = "Cancelada"

class ClasificacionASA(str, Enum):
    ASA_I = "ASA I - Sano"
    ASA_II = "ASA II - Enfermedad sistémica leve"
    ASA_III = "ASA III - Enfermedad sistémica grave"
    ASA_IV = "ASA IV - Amenaza constante para la vida"
    ASA_V = "ASA V - Moribundo"
    ASA_VI = "ASA VI - Muerte cerebral"

class TipoMantenimiento(str, Enum):
    PREVENTIVO = "Preventivo"
    CORRECTIVO = "Correctivo"
    CALIBRACION = "Calibración"

class TipoComite(str, Enum):
    MORTALIDAD = "Mortalidad"
    MORBILIDAD = "Morbilidad"
    INFECCIONES = "Infecciones"
    FARMACIA = "Farmacia"
    ETICA = "Ética"
    HISTORIAS = "Historias Clínicas"

class TipoMuerte(str, Enum):
    NATURAL = "Natural"
    VIOLENTA = "Violenta"
    EN_ESTUDIO = "En Estudio"

class GrupoSanguineo(str, Enum):
    O_POSITIVO = "O+"
    O_NEGATIVO = "O-"
    A_POSITIVO = "A+"
    A_NEGATIVO = "A-"
    B_POSITIVO = "B+"
    B_NEGATIVO = "B-"
    AB_POSITIVO = "AB+"
    AB_NEGATIVO = "AB-"

# --- MODELOS ---

class ConfigCampo(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    etiqueta: str 
    tipo: TipoCampo
    opciones: Optional[str] = None 
    activo: bool = Field(default=True)
    orden: int = Field(default=0)

class Especialidad(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    nombre: str
    medicos: List["Usuario"] = Relationship(back_populates="especialidad")
    ordenes: List["Orden"] = Relationship(back_populates="especialidad_destino")

class Entidad(SQLModel, table=True): 
    id: Optional[int] = Field(default=None, primary_key=True)
    nit: str = Field(unique=True)
    nombre: str
    codigo_eps: str 
    pacientes: List["Paciente"] = Relationship(back_populates="entidad")

class Cie10(SQLModel, table=True): 
    id: Optional[int] = Field(default=None, primary_key=True)
    codigo: str = Field(unique=True, index=True)
    descripcion: str
    sexo_requerido: str = Field(default="A") 
    edad_min: int = Field(default=0)
    edad_max: int = Field(default=120)

class Cups(SQLModel, table=True): 
    id: Optional[int] = Field(default=None, primary_key=True)
    codigo: str = Field(unique=True, index=True) 
    descripcion: str
    seccion: Optional[str] = None
    sexo_requerido: str = Field(default="A")
    edad_min: int = Field(default=0)
    edad_max: int = Field(default=120)

class Medicamento(SQLModel, table=True): 
    id: Optional[int] = Field(default=None, primary_key=True)
    codigo_cum: str = Field(unique=True, index=True)
    nombre: str 
    concentracion: str 
    forma_farmaceutica: str 
    titular: Optional[str] = None

class Usuario(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)

    # Multi-institucionalidad
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id", index=True)

    username: str = Field(unique=True, index=True)
    hashed_password: str
    nombre_completo: str
    email: Optional[str] = None
    rol: RolUsuario = Field(default=RolUsuario.MEDICO)
    activo: bool = Field(default=True)
    especialidad_id: Optional[int] = Field(default=None, foreign_key="especialidad.id")
    especialidad: Optional[Especialidad] = Relationship(back_populates="medicos")

    # Campos profesionales (Resolución 1995/1999)
    registro_medico: Optional[str] = None  # Número de tarjeta profesional
    registro_rethus: Optional[str] = None  # Registro RETHUS obligatorio
    tarjeta_profesional: Optional[str] = None
    firma_digital_path: Optional[str] = None  # Path a certificado digital

    # Seguridad
    cuenta_bloqueada: bool = Field(default=False)
    fecha_bloqueo: Optional[datetime] = None
    intentos_fallidos: int = Field(default=0)
    ultimo_cambio_password: Optional[datetime] = None
    requiere_cambio_password: bool = Field(default=False)

    paciente_id: Optional[int] = Field(default=None, foreign_key="paciente.id")

TIPOS_DOCUMENTO = {
    "CC": "Cédula de Ciudadanía",
    "TI": "Tarjeta de Identidad",
    "RC": "Registro Civil",
    "CE": "Cédula de Extranjería",
    "PA": "Pasaporte",
    "AS": "Adulto sin identificación",
    "MS": "Menor sin identificación",
    "NU": "NUIP",
    "PE": "Permiso Especial de Permanencia",
    "PT": "Permiso Temporal de Permanencia",
}

class Paciente(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)

    # Multi-institucionalidad
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id", index=True)

    identificacion: str = Field(unique=True, index=True)
    tipo_id: str = "CC"
    nombre1: str
    nombre2: Optional[str] = None
    apellido1: str
    apellido2: Optional[str] = None
    fecha_nacimiento: date
    sexo: str
    celular: str
    email: Optional[str] = None
    direccion: Optional[str] = None

    # RIPS
    tipo_usuario: TipoUsuario = Field(default=TipoUsuario.CONTRIBUTIVO)
    cod_pais: str = Field(default="170")
    cod_municipio: str = Field(default="05001")
    zona: ZonaResidencia = Field(default=ZonaResidencia.URBANA)
    cod_pais_origen: str = Field(default="170")

    # Código interno
    codigo: Optional[str] = Field(default=None, index=True)

    # Contacto adicional
    telefono2: Optional[str] = None
    acompanante: Optional[str] = None
    telefono_acompanante: Optional[str] = None

    # Datos demográficos
    estado_civil: Optional[str] = None
    oficio: Optional[str] = None

    # Residencia (texto libre; cod_pais + cod_municipio se mantienen para RIPS)
    pais_residencia: Optional[str] = Field(default="Colombia")
    departamento_residencia: Optional[str] = None
    ciudad_residencia: Optional[str] = None

    # Lugar de nacimiento (texto libre; cod_pais_origen se mantiene para RIPS)
    pais_nacimiento: Optional[str] = Field(default="Colombia")
    departamento_nacimiento: Optional[str] = None
    ciudad_nacimiento: Optional[str] = None

    # Datos de padres
    nombre_padre: Optional[str] = None
    nombre_madre: Optional[str] = None
    direccion_padres: Optional[str] = None

    # Antecedentes
    ant_patologicos: Optional[str] = None
    ant_quirurgicos: Optional[str] = None
    ant_alergicos: Optional[str] = None
    ant_familiares: Optional[str] = None
    ant_farmacologicos: Optional[str] = None
    ant_toxicos: Optional[str] = None

    # Antecedentes estructurados (checkboxes historia clínica)
    ant_familiares_struct: Optional[dict] = Field(default=None, sa_column=Column(JSON))
    ant_personales_struct: Optional[dict] = Field(default=None, sa_column=Column(JSON))
    ant_ginecologicos_struct: Optional[dict] = Field(default=None, sa_column=Column(JSON))
    ant_obstetricos_struct: Optional[dict] = Field(default=None, sa_column=Column(JSON))

    entidad_id: Optional[int] = Field(default=None, foreign_key="entidad.id")
    entidad: Optional[Entidad] = Relationship(back_populates="pacientes")

    historias: List["Historia"] = Relationship(back_populates="paciente")
    citas: List["Cita"] = Relationship(back_populates="paciente")
    autorizaciones: List["AutorizacionAcceso"] = Relationship(back_populates="paciente")

    @property
    def nombre_completo(self):
        return f"{self.nombre1} {self.nombre2 or ''} {self.apellido1} {self.apellido2 or ''}"

    @property
    def edad(self):
        today = date.today()
        return today.year - self.fecha_nacimiento.year - ((today.month, today.day) < (self.fecha_nacimiento.month, self.fecha_nacimiento.day))

    @property
    def edad_detallada(self):
        from calendar import monthrange
        today = date.today()
        years = today.year - self.fecha_nacimiento.year
        months = today.month - self.fecha_nacimiento.month
        days = today.day - self.fecha_nacimiento.day
        if days < 0:
            months -= 1
            pm = today.month - 1 if today.month > 1 else 12
            py = today.year if today.month > 1 else today.year - 1
            days += monthrange(py, pm)[1]
        if months < 0:
            years -= 1
            months += 12
        return {'anios': years, 'meses': months, 'dias': days}

class Cita(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    fecha_hora: datetime
    motivo: str
    tipo: TipoCita = Field(default=TipoCita.PRIMERA_VEZ)
    estado: EstadoCita = Field(default=EstadoCita.PENDIENTE)
    num_autorizacion: Optional[str] = None 
    paciente_id: int = Field(foreign_key="paciente.id")
    paciente: Paciente = Relationship(back_populates="citas")
    medico_id: int = Field(foreign_key="usuario.id")
    medico: Optional[Usuario] = Relationship(sa_relationship_kwargs={"foreign_keys": "Cita.medico_id"})
    historia: Optional["Historia"] = Relationship(back_populates="cita")

class AutorizacionAcceso(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    medico_id: int = Field(foreign_key="usuario.id")
    paciente_id: int = Field(foreign_key="paciente.id")
    fecha_expiracion: datetime
    activo: bool = True
    paciente: Paciente = Relationship(back_populates="autorizaciones")

class Historia(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    fecha: datetime = Field(default_factory=datetime.now)
    codigo_historia: str
    
    paciente_id: int = Field(foreign_key="paciente.id")
    paciente: Paciente = Relationship(back_populates="historias")
    medico_id: int = Field(foreign_key="usuario.id")
    medico: Optional[Usuario] = Relationship(sa_relationship_kwargs={"foreign_keys": "Historia.medico_id"})
    cita_id: Optional[int] = Field(default=None, foreign_key="cita.id")
    cita: Optional[Cita] = Relationship(back_populates="historia")
    
    # DATOS ACOMPAÑANTE
    acomp_nombre: Optional[str] = None
    acomp_parentesco: Optional[str] = None
    acomp_telefono: Optional[str] = None
    acomp_documento: Optional[str] = None

    # ANAMNESIS
    motivo_consulta: str
    enfermedad_actual: str
    
    # SIGNOS VITALES (Numéricos para cálculos)
    tension_arterial: Optional[str] = None
    frecuencia_cardiaca: Optional[int] = None
    frecuencia_respiratoria: Optional[int] = None
    temperatura: Optional[float] = None
    peso: Optional[float] = None
    talla: Optional[float] = None # En cm
    imc: Optional[float] = None # Calculado

    # EXAMEN FÍSICO DETALLADO
    ef_cabeza: Optional[str] = None
    ef_ojos: Optional[str] = None
    ef_orl: Optional[str] = None # Nariz, Oidos, Boca, Faringe
    ef_cuello: Optional[str] = None
    ef_cardiopulmonar: Optional[str] = None
    ef_abdomen: Optional[str] = None
    ef_osteomuscular: Optional[str] = None
    ef_neurologico: Optional[str] = None
    ef_piel: Optional[str] = None
    
    # RIPS
    finalidad: FinalidadConsulta = Field(default=FinalidadConsulta.NO_APLICA)
    causa_externa: CausaExterna = Field(default=CausaExterna.ENFERMEDAD_GENERAL)
    tipo_diagnostico: TipoDiagnostico = Field(default=TipoDiagnostico.IMPRESION_DIAGNOSTICA)
    cod_consulta_cups: Optional[str] = Field(default="890201") 

    datos_dinamicos: Dict = Field(default={}, sa_column=Column(JSON))
    
    # DIAGNÓSTICOS (Separados para RIPS)
    diagnostico_principal: str 
    diagnostico_relacionado1: Optional[str] = None
    
    # INCAPACIDAD
    genera_incapacidad: bool = Field(default=False)
    dias_incapacidad: int = Field(default=0)

    plan_tratamiento: str
    
    formulas: List["Formula"] = Relationship(back_populates="historia")
    ordenes: List["Orden"] = Relationship(back_populates="historia")
    imagenes: List["ImagenDiagnostica"] = Relationship(back_populates="historia")

class ImagenDiagnostica(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    historia_id: int = Field(foreign_key="historia.id")
    historia: Historia = Relationship(back_populates="imagenes")
    ruta_archivo: str 
    nombre_original: str
    descripcion: str
    fecha_subida: datetime = Field(default_factory=datetime.now)

class Formula(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    historia_id: int = Field(foreign_key="historia.id")
    historia: Optional["Historia"] = Relationship(back_populates="formulas")
    medicamento: str
    codigo_cum: Optional[str] = None
    dosis: str
    cantidad: int
    unidad: Optional[str] = Field(default="unidad")
    observaciones: Optional[str] = None
    fecha_formula: datetime = Field(default_factory=datetime.now)
    dispensada: bool = Field(default=False)

class Orden(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    historia_id: int = Field(foreign_key="historia.id")
    historia: Historia = Relationship(back_populates="ordenes")

    # Campos adicionales para facilitar consultas
    paciente_id: Optional[int] = Field(default=None, foreign_key="paciente.id")
    medico_ordena_id: Optional[int] = Field(default=None, foreign_key="usuario.id")
    fecha_orden: datetime = Field(default_factory=datetime.now)
    estado: EstadoOrden = Field(default=EstadoOrden.PENDIENTE)

    tipo: TipoOrden
    codigo_cups: Optional[str] = None
    descripcion: str
    especialidad_destino_id: Optional[int] = Field(default=None, foreign_key="especialidad.id")
    especialidad_destino: Optional[Especialidad] = Relationship(back_populates="ordenes")
    institucion_destino_id: Optional[int] = Field(default=None, foreign_key="institucion.id")
    institucion_destino: Optional["Institucion"] = Relationship(back_populates="ordenes")
    justificacion: str

    # Relaciones
    paciente: Optional["Paciente"] = Relationship()
    medico_ordena: Optional["Usuario"] = Relationship()

# ==============================================================================
# MODELOS DE SEGURIDAD Y AUDITORÍA (LEY 1581/2012 - HABEAS DATA)
# ==============================================================================

class AuditoriaAcceso(SQLModel, table=True):
    """
    Registro de auditoría obligatorio para cumplimiento de Habeas Data.
    Registra TODOS los accesos y modificaciones a datos sensibles (historias clínicas).
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    usuario_id: int = Field(foreign_key="usuario.id")
    fecha_hora: datetime = Field(default_factory=datetime.now, index=True)

    # Qué hizo
    accion: str  # "VER_HC", "MODIFICAR_HC", "CREAR_HC", "VER_PACIENTE", "CREAR_CITA", etc.
    tabla_afectada: str
    registro_id: int

    # Sobre quién (para filtros por paciente)
    paciente_id: Optional[int] = Field(default=None, foreign_key="paciente.id", index=True)

    # Información de sesión
    ip_address: Optional[str] = None
    user_agent: Optional[str] = None

    # Snapshot de datos (para trazabilidad completa)
    datos_antes: Optional[Dict] = Field(default=None, sa_column=Column(JSON))
    datos_despues: Optional[Dict] = Field(default=None, sa_column=Column(JSON))

    # Resultado
    exitoso: bool = Field(default=True)
    mensaje_error: Optional[str] = None

class FirmaDigital(SQLModel, table=True):
    """
    Registro de firmas digitales para historias clínicas (Ley 527/1999).
    Garantiza integridad y no repudio de documentos médicos.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    medico_id: int = Field(foreign_key="usuario.id")
    historia_id: int = Field(foreign_key="historia.id")

    # Hash del documento para verificar integridad
    hash_documento: str  # SHA-256 del contenido completo de la HC

    # Firma digital (si se integra con proveedor como Certicámara)
    firma_base64: Optional[str] = None
    certificado_serial: Optional[str] = None

    # Timestamp confiable
    timestamp: datetime = Field(default_factory=datetime.now)
    timestamp_authority: Optional[str] = None  # URL del servicio de timestamping

    # Metadata
    firmado_exitosamente: bool = Field(default=True)
    algoritmo: str = Field(default="SHA256withRSA")

# ==============================================================================
# MODELOS DE HISTORIA CLÍNICA COMPLETA (RESOLUCIÓN 1995/1999)
# ==============================================================================

class ConsentimientoInformado(SQLModel, table=True):
    """
    Consentimiento informado obligatorio para procedimientos invasivos,
    cirugías, y tratamientos de alto riesgo.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    historia_id: int = Field(foreign_key="historia.id")
    paciente_id: int = Field(foreign_key="paciente.id")
    medico_id: int = Field(foreign_key="usuario.id")

    fecha_firma: datetime = Field(default_factory=datetime.now)

    # Información del procedimiento
    tipo_procedimiento: str
    codigo_cups: Optional[str] = None

    # Explicación documentada
    descripcion_procedimiento: str
    riesgos_explicados: str
    beneficios_esperados: str
    alternativas_explicadas: str
    consecuencias_no_realizarlo: Optional[str] = None

    # Consentimiento
    paciente_comprende: bool = Field(default=False)
    acepta_procedimiento: bool = Field(default=False)

    # Firmas (paths a imágenes escaneadas o firma digital)
    firma_paciente_path: Optional[str] = None
    firma_medico_path: Optional[str] = None

    # Testigo (requerido en algunos casos)
    testigo_nombre: Optional[str] = None
    testigo_documento: Optional[str] = None
    testigo_firma_path: Optional[str] = None

    # Revocación (el paciente puede revocar el consentimiento)
    revocado: bool = Field(default=False)
    fecha_revocacion: Optional[datetime] = None
    motivo_revocacion: Optional[str] = None

class EvolucionMedica(SQLModel, table=True):
    """
    Notas de evolución para consultas de control y seguimiento.
    Formato SOAP (Subjetivo, Objetivo, Análisis, Plan).
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id")
    medico_id: int = Field(foreign_key="usuario.id")
    fecha: datetime = Field(default_factory=datetime.now)

    # SOAP
    subjetivo: str  # Lo que dice el paciente
    objetivo: str   # Hallazgos del examen físico
    analisis: str   # Interpretación médica
    plan: str       # Conducta a seguir

    # Relación con HC original (si aplica)
    historia_relacionada_id: Optional[int] = Field(default=None, foreign_key="historia.id")

    # Signos vitales de control
    tension_arterial: Optional[str] = None
    frecuencia_cardiaca: Optional[int] = None
    temperatura: Optional[float] = None
    peso: Optional[float] = None

class NotaEnfermeria(SQLModel, table=True):
    """
    Registro de actividades de enfermería (administración de medicamentos,
    curaciones, toma de signos vitales, etc.).
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id")
    enfermera_id: int = Field(foreign_key="usuario.id")
    fecha: datetime = Field(default_factory=datetime.now)

    # Signos vitales
    signos_vitales: Dict = Field(default={}, sa_column=Column(JSON))
    # Ejemplo: {"ta": "120/80", "fc": 72, "fr": 18, "temp": 36.5, "spo2": 98}

    # Actividades realizadas
    procedimientos_realizados: str
    medicamentos_administrados: Optional[str] = None

    # Observaciones
    observaciones: str
    nivel_conciencia: Optional[str] = None  # "ALERTA", "SOMNOLIENTO", "ESTUPOROSO", etc.
    nivel_dolor: Optional[int] = None  # Escala 0-10

class Epicrisis(SQLModel, table=True):
    """
    Resumen de egreso hospitalario (obligatorio en hospitalización).
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id")
    medico_tratante_id: int = Field(foreign_key="usuario.id")

    # Fechas
    fecha_ingreso: datetime
    fecha_egreso: datetime
    dias_estancia: int

    # Diagnósticos
    diagnostico_ingreso: str  # Código CIE-10
    diagnostico_egreso: str   # Código CIE-10 (puede ser diferente)
    complicaciones: Optional[str] = None

    # Resumen clínico
    resumen_evolucion: str
    procedimientos_realizados: str
    examenes_relevantes: Optional[str] = None

    # Plan de egreso
    recomendaciones_egreso: str
    medicamentos_egreso: Optional[str] = None
    controles_programados: Optional[str] = None

    # Pronóstico
    pronostico: str  # "BUENO", "RESERVADO", "MALO"
    incapacidad_dias: int = Field(default=0)

    # Firma
    firma_medico_path: Optional[str] = None

# ==============================================================================
# MÓDULO DE URGENCIAS (Obligatorio para IPS con servicio de urgencias)
# ==============================================================================

class AtencionUrgencias(SQLModel, table=True):
    """
    Registro de atención en servicio de urgencias.
    Incluye triage, atención médica y destino de salida.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id")
    paciente: Paciente = Relationship()

    # Ingreso
    fecha_ingreso: datetime = Field(default_factory=datetime.now)
    fecha_egreso: Optional[datetime] = None
    tiempo_espera_minutos: Optional[int] = None  # Indicador de calidad

    # TRIAGE (Clasificación de urgencia)
    triage: TipoTriage
    enfermera_triage_id: int = Field(foreign_key="usuario.id")
    motivo_consulta_triage: str
    signos_vitales_triage: Dict = Field(default={}, sa_column=Column(JSON))

    # Atención médica
    medico_id: Optional[int] = Field(default=None, foreign_key="usuario.id")
    hora_atencion_medica: Optional[datetime] = None

    # Diagnósticos
    diagnostico_ingreso: str  # Código CIE-10
    diagnostico_egreso: Optional[str] = None  # Puede cambiar después de atención

    # Historia clínica de urgencias
    historia_urgencias_id: Optional[int] = Field(default=None, foreign_key="historia.id")

    # Salida
    destino_salida: Optional[DestinoSalidaUrgencias] = None
    observaciones_egreso: Optional[str] = None

    # Indicadores
    en_atencion: bool = Field(default=True)
    priorizado: bool = Field(default=False)  # Si fue movido por gravedad

# ==============================================================================
# RESULTADOS DE LABORATORIO E IMÁGENES DIAGNÓSTICAS
# ==============================================================================

class ResultadoLaboratorio(SQLModel, table=True):
    """
    Resultados de exámenes de laboratorio.
    Permite registro digital y consulta de valores por el médico.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    orden_id: int = Field(foreign_key="orden.id")
    orden: Optional["Orden"] = Relationship()
    paciente_id: Optional[int] = Field(default=None, foreign_key="paciente.id")  # Denormalizado para búsquedas rápidas

    # Profesional que procesa
    bioanalista_id: int = Field(foreign_key="usuario.id")
    fecha_resultado: datetime = Field(default_factory=datetime.now)

    # Resultados estructurados (JSON para flexibilidad)
    # Ejemplo: {"hemoglobina": {"valor": 13.5, "unidad": "g/dL", "ref_min": 12, "ref_max": 16}}
    resultados_json: Dict = Field(default={}, sa_column=Column(JSON))

    # Observaciones del profesional
    observaciones: Optional[str] = None

    # Alertas
    valores_criticos: bool = Field(default=False)  # Si hay valores que requieren atención inmediata
    notificado_medico: bool = Field(default=False)

    # Firma y validación
    validado: bool = Field(default=False)
    fecha_validacion: Optional[datetime] = None
    validado_por_id: Optional[int] = Field(default=None, foreign_key="usuario.id")

class ResultadoImagen(SQLModel, table=True):
    """
    Resultados de estudios de imágenes diagnósticas.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    orden_id: int = Field(foreign_key="orden.id")
    orden: Optional["Orden"] = Relationship()
    paciente_id: Optional[int] = Field(default=None, foreign_key="paciente.id")

    # Profesional que interpreta
    radiologo_id: int = Field(foreign_key="usuario.id")
    fecha_resultado: datetime = Field(default_factory=datetime.now)

    # Informe radiológico
    tecnica: str  # "Proyecciones PA y lateral de tórax"
    hallazgos: str  # Descripción de lo encontrado
    impresion_diagnostica: str  # Conclusión del radiólogo
    recomendaciones: Optional[str] = None  # "Se sugiere TAC contrastado"

    # Imágenes DICOM o archivos (paths)
    imagenes_paths: List[str] = Field(default=[], sa_column=Column(JSON))

    # Clasificación BIRADS (para mamografías) u otro sistema
    clasificacion: Optional[str] = None

    # Firma digital
    firmado: bool = Field(default=False)
    firma_path: Optional[str] = None

# ==============================================================================
# SISTEMA DE FACTURACIÓN Y TARIFARIOS
# ==============================================================================

class Contrato(SQLModel, table=True):
    """
    Contratos con entidades (EPS, ARL, seguros) para prestación de servicios.
    Define las tarifas aplicables.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    entidad_id: int = Field(foreign_key="entidad.id")
    entidad: Entidad = Relationship()

    nombre: str  # "Contrato SURA 2025"
    numero_contrato: str = Field(unique=True)

    # Vigencia
    fecha_inicio: date
    fecha_fin: date
    activo: bool = Field(default=True)

    # Tipo de tarifa base
    tipo_tarifa: str  # "ISS", "SOAT", "CUPS", "PERSONALIZADA"
    porcentaje_incremento: float = Field(default=0)  # % sobre tarifa base (ej: +30% sobre ISS)

    # Metadata
    observaciones: Optional[str] = None

    # Relación con tarifas específicas
    tarifas: List["TarifaContrato"] = Relationship(back_populates="contrato")

class TarifaContrato(SQLModel, table=True):
    """
    Tarifa específica de un servicio dentro de un contrato.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    contrato_id: int = Field(foreign_key="contrato.id")
    contrato: Contrato = Relationship(back_populates="tarifas")

    codigo_servicio: str  # Código del procedimiento (CUPS)
    valor_tarifa: float  # Valor en pesos colombianos

    # Opcional: restricciones
    requiere_autorizacion: bool = Field(default=False)
    copago_porcentaje: float = Field(default=0)

class Factura(SQLModel, table=True):
    """
    Factura para radicación a entidades.
    Agrupa servicios prestados en un período.
    Decreto 4747/2007 y Resolución 3047/2008 - mecanismos de pago.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    numero_factura: str = Field(unique=True, index=True)
    entidad_id: int = Field(foreign_key="entidad.id")
    entidad: Entidad = Relationship()

    # Contrato asociado (opcional — facturación por contrato)
    contrato_id: Optional[int] = Field(default=None, foreign_key="contrato.id")

    # Fechas
    fecha_factura: date = Field(default_factory=date.today)
    # Alias para compatibilidad con módulo facturacion.py
    fecha_radicado: Optional[date] = Field(default=None, alias="fecha_radicacion")
    num_radicado: Optional[str] = None

    # Período facturado — con aliases para compatibilidad
    periodo_inicio: date = Field(alias="fecha_inicio_periodo")
    periodo_fin: date = Field(alias="fecha_fin_periodo")

    # Valores
    valor_bruto: float = Field(default=0)
    descuentos: float = Field(default=0)
    valor_neto: float = Field(default=0)

    # Estado
    estado: str = Field(default="BORRADOR")  # BORRADOR, RADICADA, ACEPTADA, GLOSADA, PAGADA

    # Observaciones y control
    observaciones: Optional[str] = None
    glosada: bool = Field(default=False)

    # Relaciones
    detalles: List["DetalleFactura"] = Relationship(back_populates="factura")
    glosas: List["Glosa"] = Relationship(back_populates="factura")


class DetalleFactura(SQLModel, table=True):
    """
    Línea de servicio dentro de una factura.
    Cada línea representa un procedimiento/consulta cobrado.
    Resolución 3047/2008 - estructura de cobro de servicios de salud.
    """
    __tablename__ = "detallefactura"

    id: Optional[int] = Field(default=None, primary_key=True)
    factura_id: int = Field(foreign_key="factura.id", index=True)

    # Descripción del servicio
    servicio: str  # "Consulta - HC 2026-001"
    codigo_cups: str = Field(default="890201")  # Código CUPS del procedimiento
    cantidad: int = Field(default=1)

    # Valores
    valor_unitario: float = Field(default=0)
    valor_total: float = Field(default=0)

    # Paciente asociado (opcional, para trazabilidad)
    paciente_id: Optional[int] = Field(default=None, foreign_key="paciente.id")
    historia_id: Optional[int] = Field(default=None, foreign_key="historia.id")

    factura: Optional[Factura] = Relationship(back_populates="detalles")

class Glosa(SQLModel, table=True):
    """
    Control de glosas (rechazos/objeciones) de facturación.
    Resolución 3047/2008 - proceso de glosas en el SGSSS.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    factura_id: int = Field(foreign_key="factura.id", index=True)

    # Descripción de la glosa
    item_glosado: str  # "Consulta del 2026-01-05"
    motivo: str  # "Falta autorización" — alias motivo_glosa para compat.
    valor_glosado: float
    fecha_glosa: date = Field(default_factory=date.today)

    # Respuesta
    respuesta: Optional[str] = None
    fecha_respuesta: Optional[date] = None
    estado: str = Field(default="PENDIENTE")  # PENDIENTE, ACEPTADA, RECHAZADA

    # Documentos soporte
    documentos_adjuntos: List[str] = Field(default=[], sa_column=Column(JSON))

    # Relación con factura (requerida para joinedload en módulo facturacion)
    factura: Optional[Factura] = Relationship(back_populates="glosas")

    @property
    def motivo_glosa(self) -> str:
        """Alias para compatibilidad con módulo facturacion.py."""
        return self.motivo

# ==============================================================================
# MÓDULO DE HOSPITALIZACIÓN
# ==============================================================================

class Cama(SQLModel, table=True):
    """
    Gestión de camas hospitalarias.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    numero: str = Field(unique=True)  # "101-A"
    piso: str
    servicio: str  # "UCI", "GENERAL", "PEDIATRIA", "MATERNIDAD"

    estado: str = Field(default="DISPONIBLE")  # DISPONIBLE, OCUPADA, EN_ASEO, BLOQUEADA, MANTENIMIENTO

    # Características
    tiene_oxigeno: bool = Field(default=False)
    tiene_monitor: bool = Field(default=False)
    es_aislamiento: bool = Field(default=False)

class Hospitalizacion(SQLModel, table=True):
    """
    Registro de hospitalización de pacientes.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id")
    paciente: Paciente = Relationship()

    cama_id: int = Field(foreign_key="cama.id")
    cama: Cama = Relationship()
    medico_tratante_id: int = Field(foreign_key="usuario.id")
    medico_tratante: "Usuario" = Relationship()

    # Fechas
    fecha_ingreso: datetime = Field(default_factory=datetime.now)
    fecha_egreso: Optional[datetime] = None

    # Diagnósticos
    diagnostico_ingreso: str  # CIE-10
    via_ingreso: str = Field(default="PROGRAMADA")  # "URGENCIAS", "PROGRAMADA", "REMISION"

    # Estado
    activo: bool = Field(default=True)
    estado: str = Field(default="ACTIVA")  # ACTIVA, ALTA_MEDICA, ALTA_VOLUNTARIA, REMISION, DEFUNCION

    # Observaciones
    observaciones_ingreso: Optional[str] = None
    motivo_ingreso: Optional[str] = None
    diagnostico_egreso: Optional[str] = None
    resumen_clinico: Optional[str] = None

class OrdenMedicaIntrahospitalaria(SQLModel, table=True):
    """
    Órdenes médicas para pacientes hospitalizados.
    Diferente de las órdenes ambulatorias.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    hospitalizacion_id: int = Field(foreign_key="hospitalizacion.id")
    medico_id: int = Field(foreign_key="usuario.id")

    fecha_orden: datetime = Field(default_factory=datetime.now)
    fecha_suspension: Optional[datetime] = None

    tipo_orden: str  # "MEDICAMENTO", "PROCEDIMIENTO", "DIETA", "LABORATORIO", "IMAGEN"
    descripcion: str
    frecuencia: Optional[str] = None  # "Cada 8 horas", "PRN"

    # Estado
    estado: str = Field(default="PENDIENTE")  # PENDIENTE, EN_CURSO, COMPLETADA, SUSPENDIDA
    activa: bool = Field(default=True)
    ejecutada: bool = Field(default=False)

    # Ejecución (enfermería)
    aplicaciones: List[Dict] = Field(default=[], sa_column=Column(JSON))
    # Ejemplo: [{"fecha": "2025-01-06 08:00", "enfermera_id": 5, "observacion": "Sin novedad"}]

    # Relación con médico que ordena
    medico: Optional["Usuario"] = Relationship(sa_relationship_kwargs={"foreign_keys": "OrdenMedicaIntrahospitalaria.medico_id"})

# ==============================================================================
# INVENTARIO DE MEDICAMENTOS E INSUMOS
# ==============================================================================

class ItemInventario(SQLModel, table=True):
    """
    Control de stock de medicamentos e insumos médicos.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    medicamento_id: Optional[int] = Field(default=None, foreign_key="medicamento.id")

    # Identificación
    nombre: str
    tipo: str = Field(default="MEDICAMENTO")  # "MEDICAMENTO", "INSUMO", "DISPOSITIVO", "REACTIVO"
    categoria: Optional[str] = None  # "Analgésico", "Antibiótico", etc.
    presentacion: Optional[str] = None  # "Tableta", "Cápsula", "Jarabe", etc.

    # Control de lote
    lote: str = Field(default="")
    fecha_vencimiento: Optional[date] = None
    registro_invima: Optional[str] = None

    # Stock
    cantidad_actual: int = Field(default=0)
    cantidad_minima: int = Field(default=10)  # Para alertas
    unidad_medida: str = Field(default="UNIDAD")  # UNIDAD, CAJA, ML, MG, etc.

    # Ubicación
    ubicacion: str = Field(default="FARMACIA")  # FARMACIA, UCI, URGENCIAS, etc.

    # Costos
    costo_unitario: float = Field(default=0)
    valor_total: float = Field(default=0)  # cantidad * costo

    # Control
    requiere_refrigeracion: bool = Field(default=False)
    controlado: bool = Field(default=False)  # Medicamentos controlados

class MovimientoInventario(SQLModel, table=True):
    """
    Registro de movimientos de inventario (entradas/salidas).
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    item_inventario_id: int = Field(foreign_key="iteminventario.id")
    usuario_id: int = Field(foreign_key="usuario.id")

    fecha: datetime = Field(default_factory=datetime.now)
    tipo_movimiento: str  # "ENTRADA", "SALIDA", "AJUSTE", "VENCIMIENTO"

    cantidad: int
    motivo: str

    # Si es salida, puede estar ligado a una dispensación
    paciente_id: Optional[int] = Field(default=None, foreign_key="paciente.id")
    formula_id: Optional[int] = Field(default=None, foreign_key="formula.id")

class DispensacionMedicamento(SQLModel, table=True):
    """
    Registro de dispensación de medicamentos en farmacia.
    Valida que exista fórmula médica y controla stock.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    formula_id: int = Field(foreign_key="formula.id")
    item_inventario_id: int = Field(foreign_key="iteminventario.id")
    paciente_id: int = Field(foreign_key="paciente.id")

    # Quien dispensa
    regente_farmacia_id: int = Field(foreign_key="usuario.id")
    fecha_dispensacion: datetime = Field(default_factory=datetime.now)

    cantidad_dispensada: int
    lote_dispensado: str

    # Trazabilidad
    paciente_firma: Optional[str] = None  # Path a firma
    observaciones: Optional[str] = None

# ==============================================================================
# MULTI-INSTITUCIONALIDAD
# ==============================================================================

class Institucion(SQLModel, table=True):
    """
    Representa una IPS/clínica/consultorio que usa el sistema.
    Permite aislar datos entre diferentes instituciones.
    """
    id: Optional[int] = Field(default=None, primary_key=True)

    # Identificación
    nit: str = Field(unique=True, index=True)
    razon_social: str
    nombre_comercial: str
    codigo_habilitacion: Optional[str] = None  # Código REPS

    # Ubicación
    direccion: str
    ciudad: str
    departamento: str
    telefono: str
    email: str

    # Representante Legal
    representante_legal: str
    identificacion_representante: str

    # Estado
    activa: bool = Field(default=True)
    fecha_creacion: datetime = Field(default_factory=datetime.now)
    fecha_vencimiento: Optional[date] = None  # Para control de licencia

    # Logo y branding
    logo_path: Optional[str] = None
    color_primario: str = Field(default="#008080")
    color_secundario: str = Field(default="#2c3e50")

    # Configuración RIPS
    codigo_prestador: str  # Código para RIPS
    tipo_prestador: str = Field(default="IPS")  # IPS, CLINICA, HOSPITAL, etc.

    # Relaciones (necesarias para back_populates)
    ordenes: List["Orden"] = Relationship(back_populates="institucion_destino")

class ConfiguracionInstitucion(SQLModel, table=True):
    """
    Configuraciones específicas por institución.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    institucion_id: int = Field(foreign_key="institucion.id", unique=True)

    # Horarios de atención
    horario_apertura: str = Field(default="08:00")
    horario_cierre: str = Field(default="18:00")
    dias_atencion: List[str] = Field(default=["L", "M", "X", "J", "V"], sa_column=Column(JSON))

    # Duraciones por defecto
    duracion_cita_default: int = Field(default=30)  # minutos
    tiempo_buffer: int = Field(default=5)  # minutos entre citas

    # Notificaciones
    enviar_recordatorios: bool = Field(default=True)
    horas_anticipacion: int = Field(default=24)
    smtp_host: Optional[str] = None
    smtp_port: Optional[int] = None
    smtp_usuario: Optional[str] = None
    smtp_password_encrypted: Optional[str] = None  # Encriptada

    # Facturación
    prefijo_factura: str = Field(default="FAC")
    resolucion_facturacion: Optional[str] = None
    rango_desde: Optional[int] = None
    rango_hasta: Optional[int] = None

    # Opciones de sistema
    permitir_citas_online: bool = Field(default=True)
    requiere_autorizacion: bool = Field(default=False)
    dias_maximos_agendamiento: int = Field(default=90)

    # Datos adicionales en JSON
    configuracion_adicional: Dict = Field(default={}, sa_column=Column(JSON))

# ==============================================================================
# SISTEMA DE LICENCIAMIENTO
# ==============================================================================

class TipoLicencia(str, Enum):
    TRIAL = "TRIAL"
    BASICA = "BASICA"
    PROFESIONAL = "PROFESIONAL"
    ENTERPRISE = "ENTERPRISE"
    PERSONALIZADA = "PERSONALIZADA"

class EstadoLicencia(str, Enum):
    ACTIVA = "ACTIVA"
    SUSPENDIDA = "SUSPENDIDA"
    VENCIDA = "VENCIDA"
    CANCELADA = "CANCELADA"

class Licencia(SQLModel, table=True):
    """
    Licencia de uso del sistema por institución.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    institucion_id: int = Field(foreign_key="institucion.id")

    # Tipo de licencia
    tipo_licencia: TipoLicencia
    estado: EstadoLicencia = Field(default=EstadoLicencia.ACTIVA)

    # Vigencia
    fecha_inicio: date
    fecha_vencimiento: date
    fecha_activacion: datetime = Field(default_factory=datetime.now)

    # Límites
    max_usuarios: int = Field(default=5)
    max_medicos: int = Field(default=2)
    max_pacientes: int = Field(default=500)
    max_citas_mes: int = Field(default=1000)

    # Módulos habilitados (lista de strings)
    modulos_habilitados: List[str] = Field(
        default=["CITAS", "PACIENTES", "HISTORIA"],
        sa_column=Column(JSON)
    )

    # Control de uso
    uso_actual_usuarios: int = Field(default=0)
    uso_actual_medicos: int = Field(default=0)
    uso_actual_pacientes: int = Field(default=0)
    uso_citas_mes_actual: int = Field(default=0)

    # Clave de activación
    clave_licencia: str = Field(unique=True)
    hash_validacion: str  # Hash para validar autenticidad

    # Información comercial
    costo_mensual: float = Field(default=0)
    descuento_aplicado: float = Field(default=0)

    # Observaciones
    notas: Optional[str] = None

    # Renovación automática
    renovacion_automatica: bool = Field(default=False)

class ModuloSistema(str, Enum):
    """Módulos disponibles en el sistema"""
    # Core
    CITAS = "CITAS"
    PACIENTES = "PACIENTES"
    HISTORIA = "HISTORIA"
    URGENCIAS = "URGENCIAS"
    PORTAL_PACIENTE = "PORTAL_PACIENTE"
    # Hospitalario
    HOSPITALIZACION = "HOSPITALIZACION"
    EPICRISIS = "EPICRISIS"
    INTERCONSULTAS = "INTERCONSULTAS"
    MAR = "MAR"
    KARDEX = "KARDEX"
    CENSO_DIARIO = "CENSO_DIARIO"
    PLAN_CUIDADO = "PLAN_CUIDADO"
    CONTRARREFERENCIA = "CONTRARREFERENCIA"
    # Quirúrgico
    QUIROFANO = "QUIROFANO"
    NOTA_OPERATORIA = "NOTA_OPERATORIA"
    PREANESTESIA = "PREANESTESIA"
    RECUPERACION = "RECUPERACION"
    # Diagnóstico
    LABORATORIO = "LABORATORIO"
    IMAGENES = "IMAGENES"
    # Soporte clínico
    FARMACIA = "FARMACIA"
    ENFERMERIA = "ENFERMERIA"
    EVOLUCIONES = "EVOLUCIONES"
    CONSENTIMIENTO = "CONSENTIMIENTO"
    BANCO_SANGRE = "BANCO_SANGRE"
    # Especialidades
    NUTRICION = "NUTRICION"
    REHABILITACION = "REHABILITACION"
    TRABAJO_SOCIAL = "TRABAJO_SOCIAL"
    TELEMEDICINA = "TELEMEDICINA"
    # Registros civiles
    CERTIFICADO_DEFUNCION = "CERTIFICADO_DEFUNCION"
    REGISTRO_NACIMIENTO = "REGISTRO_NACIMIENTO"
    INCAPACIDADES = "INCAPACIDADES"
    REMISIONES = "REMISIONES"
    # Administrativo
    FACTURACION = "FACTURACION"
    RIPS = "RIPS"
    AUDITORIA = "AUDITORIA"
    ESTADISTICAS = "ESTADISTICAS"
    TURNOS = "TURNOS"
    COMITES = "COMITES"
    EQUIPOS_MEDICOS = "EQUIPOS_MEDICOS"
    INDICADORES = "INDICADORES"
    INFECCIONES = "INFECCIONES"

class RegistroUso(SQLModel, table=True):
    """
    Registro de uso del sistema para métricas y control de licencia.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    institucion_id: int = Field(foreign_key="institucion.id")
    licencia_id: int = Field(foreign_key="licencia.id")

    # Fecha
    fecha: date = Field(default_factory=date.today)
    mes: int
    anio: int

    # Métricas diarias
    usuarios_activos: int = Field(default=0)
    citas_creadas: int = Field(default=0)
    historias_creadas: int = Field(default=0)
    pacientes_nuevos: int = Field(default=0)

    # Por módulo
    uso_por_modulo: Dict = Field(default={}, sa_column=Column(JSON))
    # Ejemplo: {"CITAS": 45, "HISTORIA": 30, "LABORATORIO": 12}

    # Almacenamiento
    espacio_usado_mb: float = Field(default=0)

# ==============================================================================
# SEGURIDAD AVANZADA
# ==============================================================================

class IntentoLogin(SQLModel, table=True):
    """
    Registro de intentos de login para detectar ataques de fuerza bruta.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

    username: str = Field(index=True)
    ip_address: str
    user_agent: Optional[str] = None

    fecha_intento: datetime = Field(default_factory=datetime.now, index=True)
    exitoso: bool

    # Razón de fallo
    razon_fallo: Optional[str] = None  # "CREDENCIALES_INVALIDAS", "CUENTA_BLOQUEADA", "LICENCIA_VENCIDA"

    # Ubicación (opcional)
    pais: Optional[str] = None
    ciudad: Optional[str] = None

class SesionActiva(SQLModel, table=True):
    """
    Control de sesiones activas con timeout automático.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    usuario_id: int = Field(foreign_key="usuario.id", index=True)
    institucion_id: int = Field(foreign_key="institucion.id")

    # Identificación de sesión
    session_token: str = Field(unique=True, index=True)

    # Metadata
    ip_address: str
    user_agent: str
    dispositivo: Optional[str] = None  # "WEB", "MOBILE", "TABLET"

    # Tiempos
    fecha_inicio: datetime = Field(default_factory=datetime.now)
    fecha_ultima_actividad: datetime = Field(default_factory=datetime.now)
    fecha_expiracion: datetime

    # Estado
    activa: bool = Field(default=True)
    cerrada_por_usuario: bool = Field(default=False)

    # Ubicación
    pais: Optional[str] = None
    ciudad: Optional[str] = None

class PoliticaSeguridad(SQLModel, table=True):
    """
    Políticas de seguridad configurables por institución.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    institucion_id: int = Field(foreign_key="institucion.id", unique=True)

    # Contraseñas
    longitud_minima_password: int = Field(default=8)
    requiere_mayuscula: bool = Field(default=True)
    requiere_minuscula: bool = Field(default=True)
    requiere_numero: bool = Field(default=True)
    requiere_caracter_especial: bool = Field(default=True)
    dias_expiracion_password: int = Field(default=90)  # 0 = nunca
    historial_passwords: int = Field(default=5)  # No reutilizar últimas N

    # Sesiones
    timeout_sesion_minutos: int = Field(default=30)
    sesiones_simultaneas_permitidas: int = Field(default=3)
    cerrar_otras_sesiones_al_login: bool = Field(default=False)

    # Intentos de login
    max_intentos_login: int = Field(default=5)
    tiempo_bloqueo_minutos: int = Field(default=15)

    # 2FA
    requiere_2fa: bool = Field(default=False)
    requiere_2fa_admin: bool = Field(default=True)
    metodo_2fa: str = Field(default="EMAIL")  # EMAIL, SMS, TOTP

    # Auditoría
    registrar_todos_los_accesos: bool = Field(default=True)
    retener_logs_dias: int = Field(default=365)

    # Restricciones de IP
    permitir_solo_ips_autorizadas: bool = Field(default=False)
    ips_autorizadas: List[str] = Field(default=[], sa_column=Column(JSON))

    # Notificaciones de seguridad
    notificar_login_nuevo_dispositivo: bool = Field(default=True)
    notificar_cambio_password: bool = Field(default=True)
    notificar_intentos_fallidos: bool = Field(default=True)

class Token2FA(SQLModel, table=True):
    """
    Tokens de autenticación de dos factores.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    usuario_id: int = Field(foreign_key="usuario.id", index=True)

    # Token
    token: str = Field(index=True)
    tipo: str  # "EMAIL", "SMS", "TOTP"

    # Validez
    fecha_generacion: datetime = Field(default_factory=datetime.now)
    fecha_expiracion: datetime
    usado: bool = Field(default=False)

    # Metadata
    ip_generacion: str
    propósito: str = Field(default="LOGIN")  # LOGIN, CAMBIO_PASSWORD, ACCION_CRITICA

class HistorialPassword(SQLModel, table=True):
    """
    Historial de contraseñas para evitar reutilización.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    usuario_id: int = Field(foreign_key="usuario.id", index=True)

    password_hash: str
    fecha_cambio: datetime = Field(default_factory=datetime.now)

# ==============================================================================
# ENCRIPTACIÓN DE DATOS SENSIBLES
# ==============================================================================

class DatoSensibleEncriptado(SQLModel, table=True):
    """
    Almacenamiento de datos sensibles encriptados (documentos, resultados críticos).
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    institucion_id: int = Field(foreign_key="institucion.id")

    # Referencia
    tipo_entidad: str  # "PACIENTE", "HISTORIA", "RESULTADO_LAB", etc.
    entidad_id: int
    campo_encriptado: str  # Nombre del campo

    # Datos encriptados
    valor_encriptado: str  # Valor cifrado con AES-256
    iv: str  # Initialization vector

    # Metadata
    fecha_encriptacion: datetime = Field(default_factory=datetime.now)
    algoritmo: str = Field(default="AES-256-CBC")
    usuario_encripto_id: int = Field(foreign_key="usuario.id")

    # Auditoría de accesos
    accesos: List[Dict] = Field(default=[], sa_column=Column(JSON))
    # Ejemplo: [{"usuario_id": 5, "fecha": "2026-01-06 10:30", "proposito": "VER_HISTORIA"}]

# ==============================================================================
# AUDITORÍA DE SUPER-ADMINISTRADOR
# ==============================================================================

class AuditoriaSuperAdmin(SQLModel, table=True):
    """
    Registro de auditoría para todas las acciones del SUPERADMIN.
    Rastrea impersonación de instituciones, modificaciones masivas, backups, etc.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    superadmin_id: int = Field(foreign_key="usuario.id", index=True)
    fecha_hora: datetime = Field(default_factory=datetime.now, index=True)

    # Acción realizada
    accion: str  # "IMPERSONAR_INSTITUCION", "CREAR_USUARIO", "BACKUP_BD", "ELIMINAR_REGISTROS", etc.
    descripcion: str  # Descripción detallada de la acción

    # Contexto
    institucion_afectada_id: Optional[int] = Field(default=None, foreign_key="institucion.id")
    tabla_afectada: Optional[str] = None
    registros_afectados: int = Field(default=0)  # Número de registros modificados/eliminados

    # Detalles adicionales en JSON
    detalles: Dict = Field(default={}, sa_column=Column(JSON))
    # Ejemplo: {"usuario_id": 5, "cambios": {"activo": false}, "ip": "192.168.1.1"}

    # Resultado
    exitoso: bool = Field(default=True)
    mensaje_error: Optional[str] = None

    # Información de sesión
    ip_address: Optional[str] = None
    user_agent: Optional[str] = None

# ==============================================================================
# MODELOS DE WORKFLOWS HOSPITALARIOS (FASE 3-4)
# ==============================================================================

class AdministracionMedicamento(SQLModel, table=True):
    """
    Hoja de Administración de Medicamentos (MAR).
    Registro de cada administración de medicamento a paciente hospitalizado.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    hospitalizacion_id: int = Field(foreign_key="hospitalizacion.id", index=True)
    orden_id: Optional[int] = Field(default=None, foreign_key="ordenmedicaintrahospitalaria.id")
    enfermera_id: int = Field(foreign_key="usuario.id")

    medicamento_nombre: str
    dosis: str
    via_administracion: str  # ORAL, IV, IM, SC, TOPICA, INHALATORIA, RECTAL, SUBLINGUAL
    hora_programada: datetime
    hora_administrada: Optional[datetime] = None
    administrado: bool = Field(default=False)
    omitido: bool = Field(default=False)
    motivo_omision: Optional[str] = None
    observaciones: Optional[str] = None
    signos_vitales_post: Dict = Field(default={}, sa_column=Column(JSON))

class KardexEnfermeria(SQLModel, table=True):
    """
    Tarjeta resumen (Kardex) de paciente hospitalizado.
    Vista consolidada del estado actual del paciente.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    hospitalizacion_id: int = Field(foreign_key="hospitalizacion.id", index=True)

    diagnostico_actual: str
    dieta: Optional[str] = None
    actividad: Optional[str] = None  # REPOSO_ABSOLUTO, REPOSO_RELATIVO, DEAMBULACION
    via_venosa: Optional[str] = None
    oxigeno: Optional[str] = None
    sonda_vesical: bool = Field(default=False)
    sonda_nasogastrica: bool = Field(default=False)
    aislamiento: Optional[str] = None
    alergias: Optional[str] = None
    precauciones_especiales: Optional[str] = None
    ultima_actualizacion: datetime = Field(default_factory=datetime.now)
    actualizado_por_id: int = Field(foreign_key="usuario.id")

class PlanCuidadoEnfermeria(SQLModel, table=True):
    """
    Plan de cuidado de enfermería basado en NANDA-NIC-NOC.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    hospitalizacion_id: int = Field(foreign_key="hospitalizacion.id", index=True)
    enfermera_id: int = Field(foreign_key="usuario.id")
    fecha_creacion: datetime = Field(default_factory=datetime.now)

    diagnostico_enfermeria: str  # Diagnóstico NANDA
    resultado_esperado: str  # Resultado NOC
    intervenciones: List[Dict] = Field(default=[], sa_column=Column(JSON))
    # Ejemplo: [{"intervencion": "Monitoreo signos vitales", "frecuencia": "c/4h", "completada": false}]

    activo: bool = Field(default=True)
    fecha_evaluacion: Optional[datetime] = None
    evaluacion_resultado: Optional[str] = None  # LOGRADO, PARCIAL, NO_LOGRADO

class EventoInfeccion(SQLModel, table=True):
    """
    Vigilancia epidemiológica y control de infecciones asociadas
    a la atención en salud (IAAS).
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    hospitalizacion_id: Optional[int] = Field(default=None, foreign_key="hospitalizacion.id")
    reportado_por_id: int = Field(foreign_key="usuario.id")

    fecha_deteccion: datetime = Field(default_factory=datetime.now)
    tipo_infeccion: str  # IVU, NEUMONIA, BACTERIEMIA, SITIO_QUIRURGICO, PIEL, OTRA
    microorganismo: Optional[str] = None
    antibiograma: Optional[str] = None
    asociada_dispositivo: bool = Field(default=False)
    dispositivo: Optional[str] = None  # CVC, SONDA_VESICAL, VENTILADOR, OTRO
    servicio: Optional[str] = None
    medidas_tomadas: Optional[str] = None
    aislamiento_requerido: bool = Field(default=False)
    notificada_sivigila: bool = Field(default=False)
    estado: str = Field(default="ACTIVA")  # ACTIVA, RESUELTA, DEFUNCION

class Contrarreferencia(SQLModel, table=True):
    """
    Seguimiento de contrarreferencia (retorno del especialista/institución de referencia).
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    orden_id: Optional[int] = Field(default=None, foreign_key="orden.id")
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    institucion_origen_id: Optional[int] = Field(default=None, foreign_key="institucion.id")
    medico_contrarrefiere_id: Optional[int] = Field(default=None, foreign_key="usuario.id")

    diagnostico_final: str
    procedimientos_realizados: Optional[str] = None
    tratamiento_instaurado: Optional[str] = None
    recomendaciones: Optional[str] = None
    requiere_seguimiento: bool = Field(default=False)
    fecha_control_sugerida: Optional[date] = None
    recibida: bool = Field(default=False)
    fecha_recepcion: Optional[datetime] = None
    recibida_por_id: Optional[int] = Field(default=None, foreign_key="usuario.id")
    fecha_creacion: datetime = Field(default_factory=datetime.now)

class Quirofano(SQLModel, table=True):
    """
    Salas de cirugía disponibles en la institución.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    nombre: str
    tipo: str = Field(default="GENERAL")  # GENERAL, AMBULATORIA, PARTOS, ESPECIALIZADA
    estado: str = Field(default="DISPONIBLE")  # DISPONIBLE, EN_USO, MANTENIMIENTO, BLOQUEADO
    equipamiento: Dict = Field(default={}, sa_column=Column(JSON))
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class ProgramacionQuirurgica(SQLModel, table=True):
    """
    Programación de procedimientos quirúrgicos.
    """
    id: Optional[int] = Field(default=None, primary_key=True)
    quirofano_id: int = Field(foreign_key="quirofano.id", index=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    cirujano_id: int = Field(foreign_key="usuario.id")
    anestesiologo_id: Optional[int] = Field(default=None, foreign_key="usuario.id")

    fecha_programada: datetime
    duracion_estimada_min: int = Field(default=60)
    procedimiento: str
    codigo_cups: Optional[str] = None
    tipo_anestesia: Optional[str] = None  # GENERAL, REGIONAL, LOCAL, SEDACION
    consentimiento_id: Optional[int] = Field(default=None, foreign_key="consentimientoinformado.id")

    # Checklist prequirúrgico
    ayuno_verificado: bool = Field(default=False)
    labs_prequirurgicos: bool = Field(default=False)
    valoracion_preanestesica: bool = Field(default=False)
    hemoclasificacion: Optional[str] = None
    reserva_sangre: bool = Field(default=False)

    # Ejecución
    estado: str = Field(default="PROGRAMADA")  # PROGRAMADA, EN_CURSO, COMPLETADA, CANCELADA, SUSPENDIDA
    hora_inicio_real: Optional[datetime] = None
    hora_fin_real: Optional[datetime] = None
    complicaciones: Optional[str] = None
    nota_quirurgica: Optional[str] = None

    # Hallazgos y descripción operatoria
    hallazgos_quirurgicos: Optional[str] = None
    sangrado_estimado_ml: Optional[int] = None
    especimenes_patologia: Optional[str] = None

# ==============================================================================
# FASE 2: INTERCONSULTAS Y NOTA OPERATORIA
# ==============================================================================

class Interconsulta(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    hospitalizacion_id: Optional[int] = Field(default=None, foreign_key="hospitalizacion.id")
    medico_solicita_id: int = Field(foreign_key="usuario.id")
    especialidad_solicitada_id: int = Field(foreign_key="especialidad.id")
    medico_responde_id: Optional[int] = Field(default=None, foreign_key="usuario.id")
    prioridad: PrioridadInterconsulta = Field(default=PrioridadInterconsulta.NORMAL)
    estado: EstadoInterconsulta = Field(default=EstadoInterconsulta.SOLICITADA)
    motivo: str
    historia_clinica_resumen: Optional[str] = None
    respuesta: Optional[str] = None
    recomendaciones: Optional[str] = None
    fecha_solicitud: datetime = Field(default_factory=datetime.now)
    fecha_respuesta: Optional[datetime] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class ProtocoloQuirurgico(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    programacion_id: int = Field(foreign_key="programacionquirurgica.id", index=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    cirujano_id: int = Field(foreign_key="usuario.id")
    ayudante_id: Optional[int] = Field(default=None, foreign_key="usuario.id")
    anestesiologo_id: Optional[int] = Field(default=None, foreign_key="usuario.id")
    instrumentadora: Optional[str] = None
    circulante: Optional[str] = None
    diagnostico_preoperatorio: str
    diagnostico_postoperatorio: str
    procedimiento_realizado: str
    codigo_cups: Optional[str] = None
    tipo_anestesia: Optional[str] = None
    hora_inicio: datetime
    hora_fin: datetime
    tiempo_quirurgico_min: int
    descripcion_tecnica: str
    hallazgos: Optional[str] = None
    sangrado_estimado_ml: int = Field(default=0)
    transfusiones: Optional[str] = None
    drenajes: Optional[str] = None
    especimenes_patologia: Optional[str] = None
    complicaciones: Optional[str] = None
    plan_postoperatorio: str
    estado_paciente_salida: str = Field(default="ESTABLE")
    firma_cirujano_path: Optional[str] = None
    fecha_creacion: datetime = Field(default_factory=datetime.now)
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class PerfilLaboratorio(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    nombre: str = Field(unique=True)
    descripcion: Optional[str] = None
    examenes: List[Dict] = Field(default=[], sa_column=Column(JSON))
    activo: bool = Field(default=True)
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

# ==============================================================================
# FASE 3: MÓDULOS HOSPITALARIOS
# ==============================================================================

class ComponenteSanguineo(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    tipo_componente: str  # SANGRE_TOTAL, GRE, PLASMA, PLAQUETAS, CRIOPRECIPITADO
    grupo_sanguineo: GrupoSanguineo
    numero_unidad: str = Field(unique=True)
    volumen_ml: int
    fecha_recoleccion: date
    fecha_vencimiento: date
    estado: str = Field(default="DISPONIBLE")  # DISPONIBLE, RESERVADA, TRANSFUNDIDA, VENCIDA, DESCARTADA
    donante_id: Optional[str] = None
    pruebas_infecciosas: Dict = Field(default={}, sa_column=Column(JSON))
    temperatura_almacenamiento: Optional[float] = None
    observaciones: Optional[str] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class TransfusionSanguinea(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    componente_id: int = Field(foreign_key="componentesanguineo.id")
    medico_ordena_id: int = Field(foreign_key="usuario.id")
    enfermera_administra_id: Optional[int] = Field(default=None, foreign_key="usuario.id")
    fecha_orden: datetime = Field(default_factory=datetime.now)
    fecha_inicio: Optional[datetime] = None
    fecha_fin: Optional[datetime] = None
    indicacion: str
    prueba_cruzada_compatible: bool = Field(default=False)
    signos_vitales_pre: Dict = Field(default={}, sa_column=Column(JSON))
    signos_vitales_post: Dict = Field(default={}, sa_column=Column(JSON))
    reaccion_adversa: bool = Field(default=False)
    tipo_reaccion: Optional[str] = None
    descripcion_reaccion: Optional[str] = None
    estado: str = Field(default="ORDENADA")  # ORDENADA, EN_CURSO, COMPLETADA, SUSPENDIDA
    observaciones: Optional[str] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class ValoracionPreanestesica(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    programacion_id: int = Field(foreign_key="programacionquirurgica.id", index=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    anestesiologo_id: int = Field(foreign_key="usuario.id")
    fecha_valoracion: datetime = Field(default_factory=datetime.now)
    antecedentes_anestesicos: Optional[str] = None
    reacciones_previas: Optional[str] = None
    mallampati: str = Field(default="I")  # I, II, III, IV
    clasificacion_asa: ClasificacionASA = Field(default=ClasificacionASA.ASA_I)
    riesgo_cardiovascular: Optional[str] = None
    riesgo_via_aerea: Optional[str] = None
    examenes_prequirurgicos: Dict = Field(default={}, sa_column=Column(JSON))
    plan_anestesico: str
    tipo_anestesia_propuesta: str  # GENERAL, REGIONAL, LOCAL, SEDACION
    medicacion_preanestesica: Optional[str] = None
    ayuno_indicado: Optional[str] = None
    consentimiento_firmado: bool = Field(default=False)
    observaciones: Optional[str] = None
    apto_cirugia: bool = Field(default=True)
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class RecuperacionPostanestesica(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    programacion_id: int = Field(foreign_key="programacionquirurgica.id", index=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    anestesiologo_id: Optional[int] = Field(default=None, foreign_key="usuario.id")
    enfermera_id: int = Field(foreign_key="usuario.id")
    hora_ingreso: datetime = Field(default_factory=datetime.now)
    hora_egreso: Optional[datetime] = None
    aldrete_ingreso: int = Field(default=0)  # 0-10
    aldrete_30min: Optional[int] = None
    aldrete_60min: Optional[int] = None
    aldrete_egreso: Optional[int] = None
    monitoreo: List[Dict] = Field(default=[], sa_column=Column(JSON))
    complicaciones: Optional[str] = None
    dolor_eva: Optional[int] = None  # 0-10
    nauseas_vomito: bool = Field(default=False)
    sangrado: Optional[str] = None
    destino_egreso: str = Field(default="HOSPITALIZACION")  # HOSPITALIZACION, AMBULATORIO, UCI
    observaciones: Optional[str] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class ValoracionNutricional(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    hospitalizacion_id: Optional[int] = Field(default=None, foreign_key="hospitalizacion.id")
    nutricionista_id: int = Field(foreign_key="usuario.id")
    fecha_valoracion: datetime = Field(default_factory=datetime.now)
    peso: float
    talla: float
    imc: float
    circunferencia_brazo: Optional[float] = None
    pliegue_tricipital: Optional[float] = None
    screening_nutricional: str = Field(default="NRS-2002")  # MST, NRS-2002, MUST
    puntaje_screening: Optional[int] = None
    riesgo_nutricional: str = Field(default="BAJO")  # BAJO, MODERADO, ALTO
    diagnostico_nutricional: str
    requerimiento_calorico: Optional[float] = None
    requerimiento_proteico: Optional[float] = None
    albumina: Optional[float] = None
    prealbumina: Optional[float] = None
    observaciones: Optional[str] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class PlanAlimentacion(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    valoracion_id: int = Field(foreign_key="valoracionnutricional.id")
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    nutricionista_id: int = Field(foreign_key="usuario.id")
    fecha_creacion: datetime = Field(default_factory=datetime.now)
    tipo_dieta: str  # NORMAL, BLANDA, LIQUIDA, HIPOCALORICA, HIPOPROTEICA, etc.
    calorias_totales: float
    distribucion_calorica: Dict = Field(default={}, sa_column=Column(JSON))
    menu_diario: Dict = Field(default={}, sa_column=Column(JSON))
    restricciones: Optional[str] = None
    suplementos: Optional[str] = None
    via_alimentacion: str = Field(default="ORAL")  # ORAL, ENTERAL, PARENTERAL, MIXTA
    activo: bool = Field(default=True)
    observaciones: Optional[str] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class PlanRehabilitacion(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    terapeuta_id: int = Field(foreign_key="usuario.id")
    fecha_creacion: datetime = Field(default_factory=datetime.now)
    diagnostico_funcional: str
    objetivos: str
    sesiones_autorizadas: int = Field(default=10)
    sesiones_realizadas: int = Field(default=0)
    tipo_terapia: str  # FISICA, OCUPACIONAL, LENGUAJE, RESPIRATORIA
    frecuencia: str  # 3 veces/semana
    duracion_sesion_min: int = Field(default=45)
    activo: bool = Field(default=True)
    fecha_cierre: Optional[date] = None
    resultado_final: Optional[str] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class SesionTerapia(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    plan_id: int = Field(foreign_key="planrehabilitacion.id", index=True)
    paciente_id: int = Field(foreign_key="paciente.id")
    terapeuta_id: int = Field(foreign_key="usuario.id")
    fecha_sesion: datetime = Field(default_factory=datetime.now)
    numero_sesion: int
    evaluacion_funcional: str
    dolor_eva: Optional[int] = None  # 0-10
    rango_movimiento: Optional[str] = None
    tecnicas_aplicadas: str
    ejercicios_domicilio: Optional[str] = None
    progreso: Optional[str] = None  # MEJORIA, ESTABLE, DETERIORO
    observaciones: Optional[str] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class ValoracionTrabajoSocial(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    hospitalizacion_id: Optional[int] = Field(default=None, foreign_key="hospitalizacion.id")
    trabajador_social_id: int = Field(foreign_key="usuario.id")
    fecha_valoracion: datetime = Field(default_factory=datetime.now)
    composicion_familiar: str
    situacion_socioeconomica: str
    estrato: Optional[int] = None  # 1-6
    tipo_vivienda: Optional[str] = None  # PROPIA, ARRENDADA, FAMILIAR
    servicios_publicos: Optional[str] = None
    red_apoyo: Optional[str] = None
    riesgos_sociales: Optional[str] = None
    situacion_laboral: Optional[str] = None
    seguridad_alimentaria: Optional[str] = None
    plan_intervencion: str
    seguimiento: Optional[str] = None
    remision_entidades: Optional[str] = None
    observaciones: Optional[str] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class CertificadoDefuncion(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    medico_certifica_id: int = Field(foreign_key="usuario.id")
    fecha_defuncion: datetime
    hora_defuncion: Optional[str] = None
    lugar_defuncion: str  # HOSPITAL, DOMICILIO, VIA_PUBLICA
    numero_certificado: str = Field(unique=True)
    causa_directa: str  # CIE-10
    causa_antecedente_1: Optional[str] = None
    causa_antecedente_2: Optional[str] = None
    causa_basica: Optional[str] = None
    otros_estados_patologicos: Optional[str] = None
    tipo_muerte: TipoMuerte = Field(default=TipoMuerte.NATURAL)
    muerte_materna: bool = Field(default=False)
    muerte_perinatal: bool = Field(default=False)
    autopsia_solicitada: bool = Field(default=False)
    circunstancia_violenta: Optional[str] = None
    firma_medico_path: Optional[str] = None
    fecha_expedicion: datetime = Field(default_factory=datetime.now)
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class RegistroNacimiento(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    madre_id: int = Field(foreign_key="paciente.id", index=True)
    medico_id: int = Field(foreign_key="usuario.id")
    fecha_nacimiento: datetime
    hora_nacimiento: str
    sexo: str  # M, F, INDETERMINADO
    peso_gramos: int
    talla_cm: float
    perimetro_cefalico_cm: Optional[float] = None
    apgar_1min: int
    apgar_5min: int
    apgar_10min: Optional[int] = None
    edad_gestacional_semanas: int
    tipo_parto: str  # VAGINAL, CESAREA, INSTRUMENTADO
    presentacion: Optional[str] = None  # CEFALICA, PODALICA, TRANSVERSA
    reanimacion_requerida: bool = Field(default=False)
    tipo_reanimacion: Optional[str] = None
    malformaciones_congenitas: Optional[str] = None
    grupo_sanguineo_rn: Optional[str] = None
    tamizaje_neonatal: Dict = Field(default={}, sa_column=Column(JSON))
    vacunas_aplicadas: Dict = Field(default={}, sa_column=Column(JSON))
    estado_rn: str = Field(default="VIVO")  # VIVO, MORTINATO
    destino: str = Field(default="ALOJAMIENTO_CONJUNTO")  # ALOJAMIENTO_CONJUNTO, UCIN, INCUBADORA
    observaciones: Optional[str] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

# ==============================================================================
# FASE 4: MÓDULOS ADMINISTRATIVOS Y CALIDAD
# ==============================================================================

class TurnoTrabajo(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    nombre: str  # "Mañana", "Tarde", "Noche"
    hora_inicio: str  # "06:00"
    hora_fin: str  # "14:00"
    tipo: str = Field(default="ASISTENCIAL")  # ASISTENCIAL, ADMINISTRATIVO
    color: str = Field(default="#4CAF50")
    activo: bool = Field(default=True)
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class AsignacionTurno(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    usuario_id: int = Field(foreign_key="usuario.id", index=True)
    turno_id: int = Field(foreign_key="turnotrabajo.id")
    fecha: date
    servicio: Optional[str] = None
    confirmado: bool = Field(default=False)
    observaciones: Optional[str] = None
    creado_por_id: int = Field(foreign_key="usuario.id")
    fecha_creacion: datetime = Field(default_factory=datetime.now)
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

# ==============================================================================
# AGENDA MÉDICA MENSUAL
# ==============================================================================

class TipoBloqueo(str, Enum):
    VACACIONES = "Vacaciones"
    CAPACITACION = "Capacitación"
    INCAPACIDAD = "Incapacidad"
    REUNION = "Reunión"
    DIA_LIBRE = "Día Libre"
    OTRO = "Otro"

class AgendaMedico(SQLModel, table=True):
    """Agenda mensual de un médico: define los días y horarios que trabaja en un mes."""
    __tablename__ = "agenda_medico"
    id: Optional[int] = Field(default=None, primary_key=True)
    medico_id: int = Field(foreign_key="usuario.id", index=True)
    año: int
    mes: int  # 1-12
    activa: bool = Field(default=True)
    observaciones: Optional[str] = None
    creada_por_id: Optional[int] = Field(default=None, foreign_key="usuario.id")
    fecha_creacion: datetime = Field(default_factory=datetime.now)
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

    bloques: List["BloqueHorario"] = Relationship(
        back_populates="agenda",
        sa_relationship_kwargs={"cascade": "all, delete-orphan"}
    )

class BloqueHorario(SQLModel, table=True):
    """Bloque de atención dentro de una agenda: un rango horario por día de semana."""
    __tablename__ = "bloque_horario"
    id: Optional[int] = Field(default=None, primary_key=True)
    agenda_id: int = Field(foreign_key="agenda_medico.id", index=True)
    dia_semana: int  # 0=Lunes, 1=Martes, 2=Miércoles, 3=Jueves, 4=Viernes, 5=Sábado, 6=Domingo
    hora_inicio: str  # "08:00"
    hora_fin: str     # "12:00"
    activo: bool = Field(default=True)

    agenda: Optional[AgendaMedico] = Relationship(back_populates="bloques")

class BloqueoAgenda(SQLModel, table=True):
    """Bloqueo puntual de agenda: vacaciones, incapacidades, reuniones, etc."""
    __tablename__ = "bloqueo_agenda"
    id: Optional[int] = Field(default=None, primary_key=True)
    medico_id: int = Field(foreign_key="usuario.id", index=True)
    fecha_inicio: datetime
    fecha_fin: datetime
    motivo: str
    tipo: TipoBloqueo = Field(default=TipoBloqueo.OTRO)
    creado_por_id: Optional[int] = Field(default=None, foreign_key="usuario.id")
    fecha_creacion: datetime = Field(default_factory=datetime.now)
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

# ==============================================================================
# SALUD PREVENTIVA: VACUNACIÓN PAI, C&D, PRENATAL, SALUD REPRODUCTIVA
# ==============================================================================

class RegistroVacuna(SQLModel, table=True):
    __tablename__ = "registro_vacuna"
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    vacuna: str              # "BCG", "Pentavalente", "Triple Viral", etc.
    dosis: str               # "1ra dosis", "Refuerzo", etc.
    fecha_aplicacion: date
    lote: Optional[str] = None
    sitio_aplicacion: Optional[str] = None  # "Deltoides derecho"
    aplicada_por_id: Optional[int] = Field(default=None, foreign_key="usuario.id")
    aplicada_por: Optional["Usuario"] = Relationship()
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")
    observaciones: Optional[str] = None
    fecha_registro: datetime = Field(default_factory=datetime.now)

class ControlCyD(SQLModel, table=True):
    __tablename__ = "control_cyd"
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    medico_id: int = Field(foreign_key="usuario.id")
    fecha: date = Field(default_factory=date.today)
    edad_meses: int
    peso_kg: Optional[float] = None
    talla_cm: Optional[float] = None
    perimetro_cefalico_cm: Optional[float] = None
    imc: Optional[float] = None
    clasificacion_peso: Optional[str] = None
    clasificacion_talla: Optional[str] = None
    hitos_alcanzados: List[str] = Field(default=[], sa_column=Column(JSON))
    alertas_desarrollo: List[str] = Field(default=[], sa_column=Column(JSON))
    clasificacion_desarrollo: Optional[str] = None
    observaciones: Optional[str] = None
    proximo_control: Optional[date] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class ControlPrenatal(SQLModel, table=True):
    __tablename__ = "control_prenatal"
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    medico_id: int = Field(foreign_key="usuario.id")
    fecha: date = Field(default_factory=date.today)
    numero_control: int = Field(default=1)
    semanas_gestacion: int
    fecha_ultima_regla: Optional[date] = None
    fecha_probable_parto: Optional[date] = None
    es_primigestante: bool = Field(default=False)
    gestas: int = Field(default=0)
    partos: int = Field(default=0)
    abortos: int = Field(default=0)
    cesareas: int = Field(default=0)
    vivos: int = Field(default=0)
    peso_kg: Optional[float] = None
    talla_cm: Optional[float] = None
    tension_arterial: Optional[str] = None
    altura_uterina_cm: Optional[float] = None
    frecuencia_cardiaca_fetal: Optional[int] = None
    presentacion_fetal: Optional[str] = None
    riesgo_obstetrico: str = Field(default="BAJO")
    laboratorios: Dict = Field(default={}, sa_column=Column(JSON))
    vacunas_embarazo: List[str] = Field(default=[], sa_column=Column(JSON))
    ecografias: List[Dict] = Field(default=[], sa_column=Column(JSON))
    observaciones: Optional[str] = None
    proximo_control: Optional[date] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class TipoTamizaje(str, Enum):
    CITOLOGIA = "Citología Vaginal"
    MAMOGRAFIA = "Mamografía"
    PSA = "PSA (Próstata)"
    VPH = "Prueba VPH"
    ECOGRAFIA_MAMA = "Ecografía Mamaria"
    TESTICULAR = "Autoexamen Testicular"
    COLPOSCOPIA = "Colposcopía"
    OTRO = "Otro"

class TamizajeSaludReproductiva(SQLModel, table=True):
    __tablename__ = "tamizaje_salud_reproductiva"
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    medico_id: Optional[int] = Field(default=None, foreign_key="usuario.id")
    tipo: TipoTamizaje
    fecha_realizacion: date
    resultado: Optional[str] = None
    clasificacion: Optional[str] = None
    requiere_seguimiento: bool = Field(default=False)
    fecha_proximo: Optional[date] = None
    observaciones: Optional[str] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")
    fecha_registro: datetime = Field(default_factory=datetime.now)

class Comite(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    tipo: TipoComite
    nombre: str
    coordinador_id: int = Field(foreign_key="usuario.id")
    miembros: List[int] = Field(default=[], sa_column=Column(JSON))
    descripcion: Optional[str] = None
    frecuencia_reunion: str = Field(default="MENSUAL")  # SEMANAL, QUINCENAL, MENSUAL, TRIMESTRAL
    activo: bool = Field(default=True)
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class SesionComite(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    comite_id: int = Field(foreign_key="comite.id", index=True)
    fecha_sesion: datetime
    numero_acta: str
    asistentes: List[int] = Field(default=[], sa_column=Column(JSON))
    orden_del_dia: str
    casos_analizados: Optional[str] = None
    conclusiones: str
    compromisos: List[Dict] = Field(default=[], sa_column=Column(JSON))
    plan_mejora: Optional[str] = None
    acta_firmada: bool = Field(default=False)
    archivo_acta_path: Optional[str] = None
    creado_por_id: int = Field(foreign_key="usuario.id")
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class EquipoMedico(SQLModel, table=True):
    __tablename__ = "equipomedico"
    id: Optional[int] = Field(default=None, primary_key=True)
    nombre: str
    marca: Optional[str] = None
    modelo: Optional[str] = None
    numero_serie: Optional[str] = None
    numero_inventario: str = Field(unique=True)
    clasificacion_riesgo: str = Field(default="I")  # I, IIA, IIB, III
    ubicacion: str
    servicio: str
    fecha_adquisicion: Optional[date] = None
    fecha_instalacion: Optional[date] = None
    vida_util_anios: Optional[int] = None
    registro_invima: Optional[str] = None
    proveedor: Optional[str] = None
    estado: str = Field(default="OPERATIVO")  # OPERATIVO, EN_MANTENIMIENTO, FUERA_SERVICIO, DADO_BAJA
    activo: bool = Field(default=True)
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class MantenimientoEquipo(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    equipo_id: int = Field(foreign_key="equipomedico.id", index=True)
    tipo: TipoMantenimiento
    fecha_programada: date
    fecha_realizada: Optional[date] = None
    tecnico: str
    empresa_servicio: Optional[str] = None
    descripcion_trabajo: str
    hallazgos: Optional[str] = None
    repuestos_usados: Optional[str] = None
    costo: float = Field(default=0)
    resultado: str = Field(default="PENDIENTE")  # PENDIENTE, CONFORME, NO_CONFORME, REQUIERE_SEGUIMIENTO
    proxima_fecha: Optional[date] = None
    certificado_path: Optional[str] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class IncidenteEquipo(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    equipo_id: int = Field(foreign_key="equipomedico.id", index=True)
    reportado_por_id: int = Field(foreign_key="usuario.id")
    fecha_incidente: datetime = Field(default_factory=datetime.now)
    descripcion: str
    paciente_afectado_id: Optional[int] = Field(default=None, foreign_key="paciente.id")
    gravedad: str = Field(default="LEVE")  # LEVE, MODERADO, GRAVE
    acciones_tomadas: Optional[str] = None
    reportado_invima: bool = Field(default=False)
    estado: str = Field(default="REPORTADO")  # REPORTADO, EN_INVESTIGACION, CERRADO
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

# ==============================================================================
# FASE 5: TELEMEDICINA Y FACTURA ELECTRÓNICA
# ==============================================================================

class ConsultaTelemedicina(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    paciente_id: int = Field(foreign_key="paciente.id", index=True)
    medico_id: int = Field(foreign_key="usuario.id")
    cita_id: Optional[int] = Field(default=None, foreign_key="cita.id")
    fecha_consulta: datetime = Field(default_factory=datetime.now)
    plataforma: str = Field(default="JITSI")
    sala_id: Optional[str] = None
    url_sala: Optional[str] = None
    consentimiento_digital: bool = Field(default=False)
    calidad_conexion: Optional[str] = None  # BUENA, REGULAR, MALA
    duracion_minutos: Optional[int] = None
    historia_id: Optional[int] = Field(default=None, foreign_key="historia.id")
    grabacion_path: Optional[str] = None
    estado: str = Field(default="PROGRAMADA")  # PROGRAMADA, EN_CURSO, COMPLETADA, FALLIDA
    motivo_fallo: Optional[str] = None
    observaciones: Optional[str] = None
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")

class FacturaElectronica(SQLModel, table=True):
    id: Optional[int] = Field(default=None, primary_key=True)
    factura_id: int = Field(foreign_key="factura.id", index=True)
    cufe: str = Field(unique=True)  # Código Único de Factura Electrónica
    numero_resolucion_dian: Optional[str] = None
    prefijo: Optional[str] = None
    xml_ubl: Optional[str] = None  # Path al XML UBL 2.1
    pdf_path: Optional[str] = None
    qr_code: Optional[str] = None
    firma_electronica: Optional[str] = None
    fecha_emision: datetime = Field(default_factory=datetime.now)
    fecha_validacion_dian: Optional[datetime] = None
    estado_dian: str = Field(default="PENDIENTE")  # PENDIENTE, ACEPTADA, RECHAZADA
    respuesta_dian: Optional[str] = None
    ambiente: str = Field(default="PRUEBAS")  # PRUEBAS, PRODUCCION
    institucion_id: Optional[int] = Field(default=None, foreign_key="institucion.id")