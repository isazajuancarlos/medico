import logging
from datetime import date, datetime, timedelta
from sqlmodel import Session, select
from database import engine, init_db
from models import (
    Usuario, RolUsuario, Especialidad, Entidad, Cie10, Medicamento, Cups,
    Institucion, Licencia, TipoLicencia, EstadoLicencia, ConfiguracionInstitucion,
    PoliticaSeguridad, Cama, ItemInventario, Paciente, ZonaResidencia, TipoUsuario,
    Historia, Cita, EstadoCita, TipoCita, Orden, TipoOrden, EstadoOrden,
    Quirofano as QuirofanoModel, TurnoTrabajo, PerfilLaboratorio
)
from seguridad import GestorLicencias
# Una sola implementación para todo el sistema: ver core/passwords.py.
from core.passwords import hash_password as get_password_hash

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

def seed_catalogos_profesionales(session: Session):
    """Crea especialidades y entidades base para IPS/Hospital completo"""
    especialidades = [
        "MEDICINA GENERAL", "MEDICINA INTERNA", "CARDIOLOGIA", "DERMATOLOGIA",
        "PEDIATRIA", "GINECOLOGIA Y OBSTETRICIA", "CIRUGIA GENERAL",
        "ORTOPEDIA Y TRAUMATOLOGIA", "ANESTESIOLOGIA", "UROLOGIA",
        "NEUROLOGIA", "NEUROCIRUGIA", "OTORRINOLARINGOLOGIA", "OFTALMOLOGIA",
        "PSIQUIATRIA", "GASTROENTEROLOGIA", "NEUMOLOGIA", "NEFROLOGIA",
        "ENDOCRINOLOGIA", "HEMATOLOGIA", "ONCOLOGIA", "REUMATOLOGIA",
        "CIRUGIA PLASTICA", "CIRUGIA CARDIOVASCULAR", "MEDICINA FISICA Y REHABILITACION",
        "PATOLOGIA", "RADIOLOGIA E IMAGENES", "MEDICINA NUCLEAR",
        "MEDICINA DE URGENCIAS", "CUIDADO INTENSIVO", "NEONATOLOGIA",
        "INFECTOLOGIA", "NUTRICION CLINICA", "PSICOLOGIA CLINICA",
        "TRABAJO SOCIAL", "TERAPIA FISICA", "TERAPIA OCUPACIONAL",
        "TERAPIA RESPIRATORIA", "FONOAUDIOLOGIA"
    ]
    count = 0
    for esp in especialidades:
        if not session.exec(select(Especialidad).where(Especialidad.nombre == esp)).first():
            session.add(Especialidad(nombre=esp))
            count += 1
    if count:
        logger.info(f"✓ {count} especialidades creadas")

    entidades = [
        {"nit": "800088702", "nombre": "EPS SURA", "codigo_eps": "EPS010"},
        {"nit": "830003564", "nombre": "NUEVA EPS", "codigo_eps": "EPS037"},
        {"nit": "800130907", "nombre": "SALUD TOTAL", "codigo_eps": "EPS002"},
        {"nit": "830009783", "nombre": "FAMISANAR", "codigo_eps": "EPS017"},
        {"nit": "800251440", "nombre": "SANITAS", "codigo_eps": "EPS005"},
        {"nit": "860066942", "nombre": "COOMEVA", "codigo_eps": "EPS016"},
        {"nit": "900156264", "nombre": "MEDIMAS", "codigo_eps": "EPS038"},
        {"nit": "800250119", "nombre": "COMPENSAR", "codigo_eps": "EPS008"},
        {"nit": "860045904", "nombre": "CAPITAL SALUD (SUBSIDIADO)", "codigo_eps": "EPSS34"},
        {"nit": "900298372", "nombre": "COOSALUD (SUBSIDIADO)", "codigo_eps": "EPSS40"},
        {"nit": "800112806", "nombre": "POSITIVA ARL", "codigo_eps": "ARL001"},
        {"nit": "860002183", "nombre": "SURA ARL", "codigo_eps": "ARL002"},
        {"nit": "800227072", "nombre": "SOAT - PREVISORA", "codigo_eps": "SOAT001"},
        {"nit": "000000000", "nombre": "PARTICULAR", "codigo_eps": "PART001"},
    ]
    count = 0
    for ent in entidades:
        if not session.exec(select(Entidad).where(Entidad.nit == ent["nit"])).first():
            session.add(Entidad(nit=ent["nit"], nombre=ent["nombre"], codigo_eps=ent["codigo_eps"]))
            count += 1
    if count:
        logger.info(f"✓ {count} entidades/EPS creadas")

def seed_institucion_demo(session: Session):
    """Crea una institución de demostración con licencia ENTERPRISE"""
    # Verificar si ya existe
    inst_demo = session.exec(
        select(Institucion).where(Institucion.nit == "900000000-1")
    ).first()

    if not inst_demo:
        # Crear institución demo
        inst_demo = Institucion(
            nit="900000000-1",
            razon_social="CLINICA DEMO SAS",
            nombre_comercial="Clínica Demo",
            codigo_habilitacion="0501000001",
            direccion="Calle 1 # 1-01",
            ciudad="Medellín",
            departamento="Antioquia",
            telefono="3001234567",
            email="info@clinicademo.com",
            representante_legal="Demo Representante",
            identificacion_representante="1234567890",
            codigo_prestador="900000000",
            tipo_prestador="IPS",
            activa=True
        )
        session.add(inst_demo)
        session.flush()
        logger.info(f"✓ Institución Demo creada: {inst_demo.nombre_comercial} (ID: {inst_demo.id})")

        # Crear configuración por defecto
        config = ConfiguracionInstitucion(institucion_id=inst_demo.id)
        session.add(config)
        logger.info(f"✓ Configuración creada para institución {inst_demo.id}")

        # Crear política de seguridad
        politica = PoliticaSeguridad(institucion_id=inst_demo.id)
        session.add(politica)
        logger.info(f"✓ Política de seguridad creada para institución {inst_demo.id}")

        # Crear licencia ENTERPRISE con 1 año de vigencia
        clave, hash_val = GestorLicencias.generar_clave_licencia(
            inst_demo.nit,
            "ENTERPRISE",
            (date.today() + timedelta(days=365)).isoformat()
        )

        licencia = Licencia(
            institucion_id=inst_demo.id,
            tipo_licencia=TipoLicencia.ENTERPRISE,
            estado=EstadoLicencia.ACTIVA,
            fecha_inicio=date.today(),
            fecha_vencimiento=date.today() + timedelta(days=365),
            max_usuarios=500,
            max_medicos=200,
            max_pacientes=100000,
            max_citas_mes=50000,
            modulos_habilitados=[
                # Core
                "CITAS", "PACIENTES", "HISTORIA", "URGENCIAS", "PORTAL_PACIENTE",
                # Hospitalario
                "HOSPITALIZACION", "EPICRISIS", "INTERCONSULTAS",
                "MAR", "KARDEX", "CENSO_DIARIO", "PLAN_CUIDADO", "CONTRARREFERENCIA",
                # Quirúrgico
                "QUIROFANO", "NOTA_OPERATORIA", "PREANESTESIA", "RECUPERACION",
                # Diagnóstico
                "LABORATORIO", "IMAGENES",
                # Soporte clínico
                "FARMACIA", "ENFERMERIA", "EVOLUCIONES", "CONSENTIMIENTO", "BANCO_SANGRE",
                # Especialidades
                "NUTRICION", "REHABILITACION", "TRABAJO_SOCIAL", "TELEMEDICINA",
                # Registros
                "CERTIFICADO_DEFUNCION", "REGISTRO_NACIMIENTO", "INCAPACIDADES", "REMISIONES",
                # Administrativo
                "FACTURACION", "RIPS", "AUDITORIA", "ESTADISTICAS",
                "TURNOS", "COMITES", "EQUIPOS_MEDICOS", "INDICADORES", "INFECCIONES"
            ],
            clave_licencia=clave,
            hash_validacion=hash_val,
            costo_mensual=1500000.0,
            renovacion_automatica=False
        )
        session.add(licencia)
        logger.info(f"✓ Licencia ENTERPRISE creada. Clave: {clave}")
    else:
        logger.info(f"→ Institución Demo ya existe (ID: {inst_demo.id})")

    return inst_demo

def seed_usuarios_base(session: Session, institucion_demo: Institucion):
    """Crea usuarios base del sistema"""

    # =====================================================
    # 1. SUPERADMIN - Usuario global (dueños del sistema)
    # =====================================================
    if not session.exec(select(Usuario).where(Usuario.username == "superadmin")).first():
        superadmin = Usuario(
            username="superadmin",
            hashed_password=get_password_hash("SuperAdmin2026!"),  # Contraseña segura
            nombre_completo="Super Administrador",
            email="admin@medsys.com",
            rol=RolUsuario.SUPERADMIN,
            activo=True,
            institucion_id=None,  # No pertenece a ninguna institución
            requiere_cambio_password=False
        )
        session.add(superadmin)
        logger.info("=" * 60)
        logger.info("✓✓✓ USUARIO SUPERADMIN CREADO ✓✓✓")
        logger.info("=" * 60)
        logger.info(f"  Username: superadmin")
        logger.info(f"  Password: SuperAdmin2026!")
        logger.info(f"  Rol: SUPERADMIN (acceso total al sistema)")
        logger.info(f"  Institución: NINGUNA (gestión multi-institucional)")
        logger.info("=" * 60)
    else:
        logger.info("→ SUPERADMIN ya existe")

    # =====================================================
    # 2. ADMIN - Administrador de Clínica Demo
    # =====================================================
    if not session.exec(select(Usuario).where(Usuario.username == "admin")).first():
        admin = Usuario(
            username="admin",
            hashed_password=get_password_hash("admin123"),
            nombre_completo="Admin de Clínica Demo",
            email="admin@clinicademo.com",
            rol=RolUsuario.ADMIN,
            activo=True,
            institucion_id=institucion_demo.id,  # Asignado a Clínica Demo
            requiere_cambio_password=False
        )
        session.add(admin)
        logger.info(f"✓ Usuario ADMIN creado para institución {institucion_demo.nombre_comercial}")
    else:
        # Actualizar si ya existe para asignarle institución
        admin = session.exec(select(Usuario).where(Usuario.username == "admin")).first()
        if admin and not admin.institucion_id:
            admin.institucion_id = institucion_demo.id
            session.add(admin)
            logger.info(f"✓ Usuario ADMIN actualizado con institución {institucion_demo.nombre_comercial}")

    # =====================================================
    # 3. MEDICO - Médico de Clínica Demo
    # =====================================================
    if not session.exec(select(Usuario).where(Usuario.username == "medico1")).first():
        esp_gen = session.exec(select(Especialidad).where(Especialidad.nombre == "MEDICINA GENERAL")).first()
        medico = Usuario(
            username="medico1",
            hashed_password=get_password_hash("medico123"),
            nombre_completo="Dr. Gregory House",
            email="medico@clinicademo.com",
            rol=RolUsuario.MEDICO,
            activo=True,
            especialidad=esp_gen,
            registro_medico="RM-111",
            registro_rethus="RETHUS-111",
            institucion_id=institucion_demo.id,  # Asignado a Clínica Demo
            requiere_cambio_password=False
        )
        session.add(medico)
        logger.info(f"✓ Usuario MEDICO creado para institución {institucion_demo.nombre_comercial}")
    else:
        # Actualizar si ya existe
        medico = session.exec(select(Usuario).where(Usuario.username == "medico1")).first()
        if medico and not medico.institucion_id:
            medico.institucion_id = institucion_demo.id
            session.add(medico)
            logger.info(f"✓ Usuario MEDICO actualizado con institución {institucion_demo.nombre_comercial}")

    # =====================================================
    # 4. PACIENTE - Paciente de prueba
    # =====================================================
    if not session.exec(select(Usuario).where(Usuario.username == "paciente1")).first():
        paciente_user = Usuario(
            username="paciente1",
            hashed_password=get_password_hash("paciente123"),
            nombre_completo="Juan Perez",
            email="paciente@clinicademo.com",
            rol=RolUsuario.PACIENTE,
            activo=True,
            institucion_id=institucion_demo.id,  # Asignado a Clínica Demo
            requiere_cambio_password=False
        )
        session.add(paciente_user)
        logger.info(f"✓ Usuario PACIENTE creado para institución {institucion_demo.nombre_comercial}")
    else:
        paciente_user = session.exec(select(Usuario).where(Usuario.username == "paciente1")).first()
        if paciente_user and not paciente_user.institucion_id:
            paciente_user.institucion_id = institucion_demo.id
            session.add(paciente_user)
            logger.info(f"✓ Usuario PACIENTE actualizado con institución {institucion_demo.nombre_comercial}")

def seed_inteligente(session: Session):
    # DIAGNÓSTICOS CIE-10 AMPLIADOS (frecuentes en IPS/Hospital)
    diagnosticos = [
        # Infecciosas
        ("A09", "DIARREA Y GASTROENTERITIS INFECCIOSA", "A", 0, 120),
        ("A90", "DENGUE", "A", 0, 120),
        ("B24", "ENFERMEDAD POR VIH SIN ESPECIFICAR", "A", 0, 120),
        ("B34.9", "INFECCION VIRAL NO ESPECIFICADA", "A", 0, 120),
        # Neoplasias
        ("C50", "TUMOR MALIGNO DE LA MAMA", "F", 18, 120),
        ("C61", "TUMOR MALIGNO DE LA PROSTATA", "M", 40, 120),
        ("D50", "ANEMIA POR DEFICIENCIA DE HIERRO", "A", 0, 120),
        # Endocrinas y metabólicas
        ("E10", "DIABETES MELLITUS TIPO 1", "A", 0, 120),
        ("E11", "DIABETES MELLITUS TIPO 2", "A", 18, 120),
        ("E66", "OBESIDAD", "A", 0, 120),
        ("E78", "TRASTORNOS DEL METABOLISMO DE LIPOPROTEINAS", "A", 18, 120),
        ("E03", "HIPOTIROIDISMO", "A", 0, 120),
        # Mentales
        ("F32", "EPISODIO DEPRESIVO", "A", 10, 120),
        ("F41", "TRASTORNO DE ANSIEDAD", "A", 10, 120),
        # Sistema nervioso
        ("G40", "EPILEPSIA", "A", 0, 120),
        ("G43", "MIGRANA", "A", 5, 120),
        # Ojos y oídos
        ("H10", "CONJUNTIVITIS", "A", 0, 120),
        ("H66", "OTITIS MEDIA", "A", 0, 120),
        # Circulatorio
        ("I10", "HIPERTENSION ARTERIAL", "A", 18, 120),
        ("I20", "ANGINA DE PECHO", "A", 30, 120),
        ("I21", "INFARTO AGUDO DE MIOCARDIO", "A", 25, 120),
        ("I50", "INSUFICIENCIA CARDIACA", "A", 20, 120),
        ("I63", "INFARTO CEREBRAL (ACV)", "A", 20, 120),
        ("I64", "ACCIDENTE CEREBROVASCULAR NO ESPECIFICADO", "A", 20, 120),
        # Respiratorio
        ("J00", "RINOFARINGITIS AGUDA", "A", 0, 120),
        ("J06", "INFECCION AGUDA VIAS RESPIRATORIAS SUPERIORES", "A", 0, 120),
        ("J18", "NEUMONIA ORGANISMO NO ESPECIFICADO", "A", 0, 120),
        ("J44", "EPOC", "A", 30, 120),
        ("J45", "ASMA", "A", 0, 120),
        ("J96", "INSUFICIENCIA RESPIRATORIA", "A", 0, 120),
        # Digestivo
        ("K21", "ENFERMEDAD POR REFLUJO GASTROESOFAGICO", "A", 0, 120),
        ("K29", "GASTRITIS Y DUODENITIS", "A", 5, 120),
        ("K35", "APENDICITIS AGUDA", "A", 0, 120),
        ("K40", "HERNIA INGUINAL", "A", 0, 120),
        ("K80", "COLELITIASIS", "A", 10, 120),
        # Piel
        ("L02", "ABSCESO CUTANEO FURUNCULO Y ANTRAX", "A", 0, 120),
        ("L50", "URTICARIA", "A", 0, 120),
        # Osteomuscular
        ("M17", "GONARTROSIS (ARTROSIS DE RODILLA)", "A", 30, 120),
        ("M54", "DORSALGIA / LUMBALGIA", "A", 10, 120),
        ("M79", "TRASTORNOS DE TEJIDOS BLANDOS", "A", 0, 120),
        # Genitourinario
        ("N39.0", "INFECCION DE VIAS URINARIAS", "A", 0, 120),
        ("N40", "HIPERPLASIA DE LA PROSTATA", "M", 40, 120),
        ("N76", "INFLAMACION DE VAGINA Y VULVA", "F", 5, 120),
        # Embarazo y parto
        ("O80", "PARTO UNICO ESPONTANEO", "F", 12, 55),
        ("O82", "PARTO UNICO POR CESAREA", "F", 12, 55),
        ("O20", "HEMORRAGIA PRECOZ DEL EMBARAZO", "F", 12, 55),
        ("O47", "FALSO TRABAJO DE PARTO", "F", 12, 55),
        ("O99", "OTRAS ENFERMEDADES MATERNAS", "F", 12, 55),
        # Perinatales
        ("P07", "TRASTORNOS RELACIONADOS CON PREMATURIDAD", "A", 0, 1),
        ("P22", "DIFICULTAD RESPIRATORIA DEL RECIEN NACIDO", "A", 0, 0),
        # Malformaciones congénitas
        ("Q21", "MALFORMACIONES CONGENITAS TABIQUES CARDIACOS", "A", 0, 120),
        # Traumatismos
        ("S52", "FRACTURA DE ANTEBRAZO", "A", 0, 120),
        ("S72", "FRACTURA DE FEMUR", "A", 0, 120),
        ("S82", "FRACTURA DE PIERNA", "A", 0, 120),
        ("S06", "TRAUMATISMO INTRACRANEAL", "A", 0, 120),
        ("T78.4", "ALERGIA NO ESPECIFICADA", "A", 0, 120),
        # Factores de salud
        ("Z001", "SUPERVISION DE SALUD DEL LACTANTE", "A", 0, 2),
        ("Z00.0", "EXAMEN MEDICO GENERAL", "A", 0, 120),
        ("Z34", "SUPERVISION DE EMBARAZO NORMAL", "F", 12, 55),
        ("Z30", "PLANIFICACION FAMILIAR", "A", 12, 55),
        ("Z76.0", "EXPEDICION DE CERTIFICADO MEDICO", "A", 0, 120),
    ]
    count = 0
    for cod, desc, sex, emin, emax in diagnosticos:
        if not session.exec(select(Cie10).where(Cie10.codigo == cod)).first():
            session.add(Cie10(codigo=cod, descripcion=desc, sexo_requerido=sex, edad_min=emin, edad_max=emax))
            count += 1
    logger.info(f"✓ {count} diagnósticos CIE-10 creados")

    # PROCEDIMIENTOS CUPS AMPLIADOS
    procedimientos = [
        # Consultas
        ("890201", "CONSULTA PRIMERA VEZ MEDICINA GENERAL", "CONSULTA", "A", 0, 120),
        ("890301", "CONSULTA CONTROL MEDICINA GENERAL", "CONSULTA", "A", 0, 120),
        ("890202", "CONSULTA PRIMERA VEZ MEDICINA ESPECIALIZADA", "CONSULTA", "A", 0, 120),
        ("890302", "CONSULTA CONTROL MEDICINA ESPECIALIZADA", "CONSULTA", "A", 0, 120),
        ("890203", "CONSULTA PRIMERA VEZ ODONTOLOGIA GENERAL", "CONSULTA", "A", 0, 120),
        ("890205", "CONSULTA PRIMERA VEZ PEDIATRIA", "CONSULTA", "A", 0, 18),
        ("890206", "CONSULTA PRIMERA VEZ GINECOLOGIA", "CONSULTA", "F", 10, 120),
        ("890207", "CONSULTA PRIMERA VEZ PSICOLOGIA", "CONSULTA", "A", 0, 120),
        ("890208", "CONSULTA PRIMERA VEZ NUTRICION", "CONSULTA", "A", 0, 120),
        # Urgencias
        ("890401", "ATENCION URGENCIAS MEDICINA GENERAL", "URGENCIAS", "A", 0, 120),
        ("890601", "ATENCION INICIAL URGENCIAS", "URGENCIAS", "A", 0, 120),
        # Laboratorio
        ("902210", "HEMOGRAMA COMPLETO", "LABORATORIO", "A", 0, 120),
        ("903841", "GLUCOSA EN SUERO", "LABORATORIO", "A", 0, 120),
        ("903856", "CREATININA EN SUERO", "LABORATORIO", "A", 0, 120),
        ("903868", "PERFIL LIPIDICO", "LABORATORIO", "A", 0, 120),
        ("907106", "UROANALISIS", "LABORATORIO", "A", 0, 120),
        ("906913", "TSH (HORMONA ESTIMULANTE TIROIDES)", "LABORATORIO", "A", 0, 120),
        ("906914", "T4 LIBRE", "LABORATORIO", "A", 0, 120),
        ("902049", "PRUEBAS CRUZADAS", "LABORATORIO", "A", 0, 120),
        ("902004", "HEMOCLASIFICACION ABO Y RH", "LABORATORIO", "A", 0, 120),
        ("904902", "HIV 1 Y 2 ANTICUERPOS", "LABORATORIO", "A", 0, 120),
        ("906249", "COPROLÓGICO", "LABORATORIO", "A", 0, 120),
        ("903895", "HBA1C (HEMOGLOBINA GLICOSILADA)", "LABORATORIO", "A", 0, 120),
        ("906023", "UROCULTIVO", "LABORATORIO", "A", 0, 120),
        ("907002", "PRUEBA DE EMBARAZO EN ORINA", "LABORATORIO", "F", 10, 55),
        # Imagenología
        ("871020", "RADIOGRAFIA DE TORAX", "IMAGENOLOGIA", "A", 0, 120),
        ("871040", "RADIOGRAFIA DE ABDOMEN", "IMAGENOLOGIA", "A", 0, 120),
        ("871100", "RADIOGRAFIA DE COLUMNA LUMBOSACRA", "IMAGENOLOGIA", "A", 0, 120),
        ("879100", "ECOGRAFIA ABDOMINAL TOTAL", "IMAGENOLOGIA", "A", 0, 120),
        ("879801", "ECOGRAFIA OBSTETRICA", "IMAGENOLOGIA", "F", 10, 55),
        ("883101", "RESONANCIA MAGNETICA CEREBRAL", "IMAGENOLOGIA", "A", 0, 120),
        ("879301", "ECOGRAFIA DE TEJIDOS BLANDOS", "IMAGENOLOGIA", "A", 0, 120),
        ("876001", "TOMOGRAFIA COMPUTARIZADA DE CRANEO", "IMAGENOLOGIA", "A", 0, 120),
        ("876011", "TAC DE TORAX", "IMAGENOLOGIA", "A", 0, 120),
        ("876021", "TAC DE ABDOMEN", "IMAGENOLOGIA", "A", 0, 120),
        ("881201", "MAMOGRAFIA BILATERAL", "IMAGENOLOGIA", "F", 35, 120),
        # Procedimientos quirúrgicos
        ("470100", "APENDICECTOMIA", "CIRUGIA", "A", 0, 120),
        ("513200", "COLECISTECTOMIA LAPAROSCOPICA", "CIRUGIA", "A", 10, 120),
        ("692001", "LEGRADO UTERINO", "CIRUGIA", "F", 12, 60),
        ("571001", "CISTOSCOPIA TRANSURETRAL", "CIRUGIA", "A", 0, 120),
        ("737100", "REDUCCION ABIERTA DE FRACTURA", "CIRUGIA", "A", 0, 120),
        ("741300", "ARTROPLASTIA DE RODILLA", "CIRUGIA", "A", 40, 120),
        ("661900", "CESAREA SEGMENTARIA", "CIRUGIA", "F", 12, 55),
        # Terapias
        ("931000", "TERAPIA FISICA INTEGRAL", "TERAPIA", "A", 0, 120),
        ("931001", "TERAPIA OCUPACIONAL INTEGRAL", "TERAPIA", "A", 0, 120),
        ("931002", "TERAPIA RESPIRATORIA", "TERAPIA", "A", 0, 120),
        ("931003", "TERAPIA DEL LENGUAJE", "TERAPIA", "A", 0, 120),
        # Otros procedimientos
        ("991601", "TRANSFUSION DE GLOBULOS ROJOS", "BANCO_SANGRE", "A", 0, 120),
        ("991602", "TRANSFUSION DE PLAQUETAS", "BANCO_SANGRE", "A", 0, 120),
        ("991603", "TRANSFUSION DE PLASMA", "BANCO_SANGRE", "A", 0, 120),
        ("990116", "NUTRICION PARENTERAL TOTAL", "NUTRICION", "A", 0, 120),
    ]
    count = 0
    for cod, desc, sec, sex, emin, emax in procedimientos:
        if not session.exec(select(Cups).where(Cups.codigo == cod)).first():
            session.add(Cups(codigo=cod, descripcion=desc, seccion=sec, sexo_requerido=sex, edad_min=emin, edad_max=emax))
            count += 1
    logger.info(f"✓ {count} procedimientos CUPS creados")

def seed_camas(session: Session):
    """Crea camas hospitalarias de prueba"""
    camas = [
        {"numero": "101-A", "piso": "1", "servicio": "HOSPITALIZACION GENERAL", "tiene_oxigeno": True},
        {"numero": "101-B", "piso": "1", "servicio": "HOSPITALIZACION GENERAL", "tiene_oxigeno": True},
        {"numero": "102-A", "piso": "1", "servicio": "HOSPITALIZACION GENERAL"},
        {"numero": "UCI-01", "piso": "2", "servicio": "UCI - Unidad de Cuidados Intensivos", "tiene_oxigeno": True, "tiene_monitor": True},
        {"numero": "UCI-02", "piso": "2", "servicio": "UCI - Unidad de Cuidados Intensivos", "tiene_oxigeno": True, "tiene_monitor": True},
        {"numero": "PED-01", "piso": "3", "servicio": "PEDIATRIA", "tiene_oxigeno": True},
        {"numero": "PED-02", "piso": "3", "servicio": "PEDIATRIA"},
        {"numero": "GIN-01", "piso": "4", "servicio": "GINECOBSTETRICIA"},
        {"numero": "ISO-01", "piso": "2", "servicio": "HOSPITALIZACION GENERAL", "es_aislamiento": True},
    ]
    for c in camas:
        if not session.exec(select(Cama).where(Cama.numero == c["numero"])).first():
            session.add(Cama(**c))
    logger.info(f"✓ {len(camas)} camas hospitalarias creadas")


def seed_farmacia(session: Session):
    """Crea inventario base de farmacia"""
    items = [
        {"nombre": "ACETAMINOFEN 500MG TABLETA", "tipo": "MEDICAMENTO", "categoria": "Analgésico",
         "presentacion": "Tableta", "lote": "L2026-001", "fecha_vencimiento": date(2027, 6, 30),
         "cantidad_actual": 500, "cantidad_minima": 50, "costo_unitario": 150},
        {"nombre": "IBUPROFENO 400MG TABLETA", "tipo": "MEDICAMENTO", "categoria": "Antiinflamatorio",
         "presentacion": "Tableta", "lote": "L2026-002", "fecha_vencimiento": date(2027, 3, 31),
         "cantidad_actual": 300, "cantidad_minima": 30, "costo_unitario": 200},
        {"nombre": "AMOXICILINA 500MG CAPSULAS", "tipo": "MEDICAMENTO", "categoria": "Antibiótico",
         "presentacion": "Cápsula", "lote": "L2026-003", "fecha_vencimiento": date(2026, 12, 31),
         "cantidad_actual": 200, "cantidad_minima": 20, "costo_unitario": 350},
        {"nombre": "LOSARTAN 50MG TABLETA", "tipo": "MEDICAMENTO", "categoria": "Antihipertensivo",
         "presentacion": "Tableta", "lote": "L2026-004", "fecha_vencimiento": date(2027, 9, 30),
         "cantidad_actual": 400, "cantidad_minima": 40, "costo_unitario": 280},
        {"nombre": "METFORMINA 850MG TABLETA", "tipo": "MEDICAMENTO", "categoria": "Antidiabético",
         "presentacion": "Tableta", "lote": "L2026-005", "fecha_vencimiento": date(2027, 6, 30),
         "cantidad_actual": 250, "cantidad_minima": 25, "costo_unitario": 180},
        {"nombre": "SUERO FISIOLOGICO 500ML", "tipo": "MEDICAMENTO", "categoria": "Solución Parenteral",
         "presentacion": "Frasco", "lote": "L2026-010", "fecha_vencimiento": date(2026, 8, 31),
         "cantidad_actual": 80, "cantidad_minima": 20, "costo_unitario": 8500},
        {"nombre": "GUANTES LATEX TALLA M (100u)", "tipo": "INSUMO", "categoria": "Insumo Médico",
         "presentacion": "Frasco", "lote": "L2026-020", "fecha_vencimiento": None,
         "cantidad_actual": 50, "cantidad_minima": 10, "costo_unitario": 18000},
        {"nombre": "JERINGA 10ML C/AGUJA (100u)", "tipo": "INSUMO", "categoria": "Insumo Médico",
         "presentacion": "Frasco", "lote": "L2026-021", "fecha_vencimiento": None,
         "cantidad_actual": 40, "cantidad_minima": 10, "costo_unitario": 22000},
        {"nombre": "DIAZEPAM 10MG AMPOLLA", "tipo": "MEDICAMENTO", "categoria": "Otro",
         "presentacion": "Ampolla", "lote": "L2026-030", "fecha_vencimiento": date(2027, 3, 31),
         "cantidad_actual": 30, "cantidad_minima": 10, "costo_unitario": 1200, "controlado": True},
    ]
    count = 0
    for item_data in items:
        if not session.exec(select(ItemInventario).where(ItemInventario.nombre == item_data["nombre"])).first():
            controlado = item_data.pop("controlado", False)
            cantidad = item_data.pop("cantidad_actual")
            item = ItemInventario(**item_data, controlado=controlado)
            item.cantidad_actual = cantidad
            item.valor_total = cantidad * item.costo_unitario
            session.add(item)
            count += 1
    logger.info(f"✓ {count} ítems de farmacia creados")


def seed_pacientes_demo(session: Session, institucion: Institucion, entidad_id: int):
    """Crea pacientes de prueba"""
    pacientes_data = [
        {"identificacion": "1234567890", "nombre1": "Carlos", "apellido1": "García",
         "apellido2": "López", "fecha_nacimiento": date(1975, 5, 15), "sexo": "M",
         "celular": "3001111111", "email": "carlos@ejemplo.com"},
        {"identificacion": "9876543210", "nombre1": "María", "nombre2": "Isabel",
         "apellido1": "Martínez", "fecha_nacimiento": date(1988, 8, 20), "sexo": "F",
         "celular": "3002222222"},
        {"identificacion": "1122334455", "nombre1": "Luis", "apellido1": "Rodríguez",
         "fecha_nacimiento": date(1960, 3, 10), "sexo": "M", "celular": "3003333333"},
        {"identificacion": "5566778899", "nombre1": "Ana", "nombre2": "María",
         "apellido1": "Sánchez", "apellido2": "Vargas", "fecha_nacimiento": date(1995, 12, 1),
         "sexo": "F", "celular": "3004444444"},
        {"identificacion": "6677889900", "nombre1": "Tomás", "apellido1": "Hernández",
         "fecha_nacimiento": date(2018, 6, 25), "sexo": "M", "celular": "3005555555"},
    ]
    pacientes_creados = []
    for p in pacientes_data:
        if not session.exec(select(Paciente).where(Paciente.identificacion == p["identificacion"])).first():
            paciente = Paciente(
                **p,
                tipo_id="CC",
                tipo_usuario=TipoUsuario.CONTRIBUTIVO,
                zona=ZonaResidencia.URBANA,
                institucion_id=institucion.id,
                entidad_id=entidad_id
            )
            session.add(paciente)
            session.flush()
            pacientes_creados.append(paciente)
        else:
            paciente = session.exec(select(Paciente).where(Paciente.identificacion == p["identificacion"])).first()
            pacientes_creados.append(paciente)
    logger.info(f"✓ {len(pacientes_creados)} pacientes de prueba creados/encontrados")
    return pacientes_creados


def seed_historias_y_ordenes(session: Session, pacientes: list, medico: Usuario):
    """Crea historias clínicas de muestra con órdenes para lab e imágenes"""
    from datetime import datetime as dt
    from models import FinalidadConsulta, CausaExterna, TipoDiagnostico

    datos_historias = [
        {"paciente": pacientes[0], "dx": "I10", "dx_desc": "Hipertensión arterial",
         "motivo": "Control de presión arterial", "plan": "Continuar tratamiento antihipertensivo",
         "lab": "Perfil lipídico", "img": None, "incapacidad": False},
        {"paciente": pacientes[1], "dx": "J00", "dx_desc": "Rinofaringitis aguda",
         "motivo": "Tos y fiebre hace 3 días", "plan": "Acetaminofen 1g c/8h por 3 días",
         "lab": "Hemograma Completo", "img": "Rayos X Tórax",
         "incapacidad": True, "dias_incap": 3},
        {"paciente": pacientes[2], "dx": "E11", "dx_desc": "Diabetes mellitus tipo 2",
         "motivo": "Control glucémico trimestral", "plan": "Metformina 850mg c/12h",
         "lab": "Química Sanguínea", "img": None, "incapacidad": False},
        {"paciente": pacientes[3], "dx": "N39.0", "dx_desc": "Infección de vías urinarias",
         "motivo": "Ardor al orinar, fiebre", "plan": "Amoxicilina 500mg c/8h por 7 días",
         "lab": "Uroanálisis", "img": None, "incapacidad": True, "dias_incap": 2},
    ]

    count_hist = 0
    count_ord = 0
    for i, d in enumerate(datos_historias):
        pac = d["paciente"]
        dias_atras = timedelta(days=i * 3)

        # Verificar si ya hay historia para este paciente hoy
        exists = session.exec(
            select(Historia).where(
                Historia.paciente_id == pac.id,
                Historia.medico_id == medico.id
            )
        ).first()
        if exists:
            continue

        historia = Historia(
            codigo_historia=f"HC-2026-{1000 + i:04d}",
            paciente_id=pac.id,
            medico_id=medico.id,
            fecha=dt.now() - dias_atras,
            motivo_consulta=d["motivo"],
            enfermedad_actual=d["motivo"],
            diagnostico_principal=d["dx"],
            plan_tratamiento=d["plan"],
            # Por CÓDIGO y no por nombre de miembro: los miembros los genera
            # el maestro del validador y cambiarían si la norma renombra una
            # fila. 16 = TRATAMIENTO, 38 = ENFERMEDAD GENERAL, los dos de la
            # tabla de la 948. Antes iban NO_APLICA («10») y ENFERMEDAD_GENERAL
            # («13»), que no existen en ella: la semilla producía demos cuyo
            # RIPS el MUV habría rechazado.
            finalidad=FinalidadConsulta("16"),
            causa_externa=CausaExterna("38"),
            tipo_diagnostico=TipoDiagnostico.CONFIRMADO_NUEVO,
            genera_incapacidad=d.get("incapacidad", False),
            dias_incapacidad=d.get("dias_incap", 0)
        )
        session.add(historia)
        session.flush()
        count_hist += 1

        # Orden de laboratorio
        if d.get("lab"):
            orden_lab = Orden(
                historia_id=historia.id,
                paciente_id=pac.id,
                medico_ordena_id=medico.id,
                tipo=TipoOrden.LABORATORIO,
                descripcion=d["lab"],
                justificacion="Evaluación de rutina",
                estado=EstadoOrden.PENDIENTE,
                fecha_orden=dt.now() - dias_atras
            )
            session.add(orden_lab)
            count_ord += 1

        # Orden de imagen
        if d.get("img"):
            orden_img = Orden(
                historia_id=historia.id,
                paciente_id=pac.id,
                medico_ordena_id=medico.id,
                tipo=TipoOrden.IMAGENOLOGIA,
                descripcion=d["img"],
                justificacion="Descarte patología pulmonar",
                estado=EstadoOrden.PENDIENTE,
                fecha_orden=dt.now() - dias_atras
            )
            session.add(orden_img)
            count_ord += 1

    logger.info(f"✓ {count_hist} historias y {count_ord} órdenes de prueba creadas")


def seed_usuario_enfermeria(session: Session, institucion: Institucion):
    """Crea usuario de enfermería de prueba"""
    if not session.exec(select(Usuario).where(Usuario.username == "enfermera1")).first():
        session.add(Usuario(
            username="enfermera1",
            hashed_password=get_password_hash("enfermera123"),
            nombre_completo="Enf. Laura Ramírez",
            email="enfermera@clinicademo.com",
            rol=RolUsuario.ENFERMERIA,
            activo=True,
            institucion_id=institucion.id,
            requiere_cambio_password=False
        ))
        logger.info("✓ Usuario ENFERMERIA creado (enfermera1 / enfermera123)")


def seed_usuarios_roles_nuevos(session: Session, institucion: Institucion):
    """Crea usuarios de prueba para todos los roles especializados"""
    usuarios_nuevos = [
        {
            "username": "anestesiologo1", "password": "aneste123",
            "nombre_completo": "Dr. Ricardo Vélez", "email": "anestesiologo@clinicademo.com",
            "rol": RolUsuario.ANESTESIOLOGO, "registro_medico": "RM-201", "registro_rethus": "RETHUS-201"
        },
        {
            "username": "laboratorista1", "password": "lab123",
            "nombre_completo": "Bact. Sandra Ospina", "email": "lab@clinicademo.com",
            "rol": RolUsuario.LABORATORISTA
        },
        {
            "username": "bacteriologo1", "password": "bact123",
            "nombre_completo": "Bact. Andrés Cárdenas", "email": "bacteriologo@clinicademo.com",
            "rol": RolUsuario.BACTERIOLOGO
        },
        {
            "username": "radiologo1", "password": "radio123",
            "nombre_completo": "Dr. Felipe Moreno", "email": "radiologo@clinicademo.com",
            "rol": RolUsuario.RADIOLOGO, "registro_medico": "RM-301", "registro_rethus": "RETHUS-301"
        },
        {
            "username": "terapeuta1", "password": "terap123",
            "nombre_completo": "Ft. Carolina Duque", "email": "terapeuta@clinicademo.com",
            "rol": RolUsuario.TERAPEUTA
        },
        {
            "username": "nutricionista1", "password": "nutri123",
            "nombre_completo": "Nut. Valentina Ríos", "email": "nutricionista@clinicademo.com",
            "rol": RolUsuario.NUTRICIONISTA
        },
        {
            "username": "psicologo1", "password": "psico123",
            "nombre_completo": "Ps. Daniela Herrera", "email": "psicologo@clinicademo.com",
            "rol": RolUsuario.PSICOLOGO
        },
        {
            "username": "tsocial1", "password": "tsocial123",
            "nombre_completo": "TS. Camila Restrepo", "email": "tsocial@clinicademo.com",
            "rol": RolUsuario.TRABAJO_SOCIAL
        },
        {
            "username": "regente1", "password": "regente123",
            "nombre_completo": "RF. Jorge Castillo", "email": "regente@clinicademo.com",
            "rol": RolUsuario.REGENTE_FARMACIA
        },
        {
            "username": "auxiliar1", "password": "auxiliar123",
            "nombre_completo": "Aux. Paola Gómez", "email": "auxiliar@clinicademo.com",
            "rol": RolUsuario.AUXILIAR_ENFERMERIA
        },
    ]

    count = 0
    for u in usuarios_nuevos:
        if not session.exec(select(Usuario).where(Usuario.username == u["username"])).first():
            session.add(Usuario(
                username=u["username"],
                hashed_password=get_password_hash(u["password"]),
                nombre_completo=u["nombre_completo"],
                email=u["email"],
                rol=u["rol"],
                activo=True,
                institucion_id=institucion.id,
                requiere_cambio_password=False,
                registro_medico=u.get("registro_medico"),
                registro_rethus=u.get("registro_rethus"),
            ))
            count += 1
            logger.info(f"✓ Usuario {u['rol'].value} creado ({u['username']} / {u['password']})")
    if count:
        logger.info(f"✓ {count} usuarios de roles especializados creados")


def seed_quirofanos(session: Session):
    """Crea quirófanos de prueba"""
    quirofanos = [
        {"nombre": "Quirófano 1 - General", "tipo": "GENERAL"},
        {"nombre": "Quirófano 2 - General", "tipo": "GENERAL"},
        {"nombre": "Quirófano 3 - Ambulatorio", "tipo": "AMBULATORIA"},
        {"nombre": "Sala de Partos 1", "tipo": "PARTOS"},
        {"nombre": "Sala de Partos 2", "tipo": "PARTOS"},
        {"nombre": "Quirófano 4 - Especializado", "tipo": "ESPECIALIZADA"},
    ]
    count = 0
    for q in quirofanos:
        if not session.exec(select(QuirofanoModel).where(QuirofanoModel.nombre == q["nombre"])).first():
            session.add(QuirofanoModel(**q))
            count += 1
    if count:
        logger.info(f"✓ {count} quirófanos creados")


def seed_turnos(session: Session):
    """Crea turnos de trabajo base"""
    turnos = [
        {"nombre": "Mañana", "hora_inicio": "06:00", "hora_fin": "14:00", "tipo": "ASISTENCIAL", "color": "#4CAF50"},
        {"nombre": "Tarde", "hora_inicio": "14:00", "hora_fin": "22:00", "tipo": "ASISTENCIAL", "color": "#2196F3"},
        {"nombre": "Noche", "hora_inicio": "22:00", "hora_fin": "06:00", "tipo": "ASISTENCIAL", "color": "#9C27B0"},
        {"nombre": "Diurno 12h", "hora_inicio": "07:00", "hora_fin": "19:00", "tipo": "ASISTENCIAL", "color": "#FF9800"},
        {"nombre": "Nocturno 12h", "hora_inicio": "19:00", "hora_fin": "07:00", "tipo": "ASISTENCIAL", "color": "#607D8B"},
        {"nombre": "Administrativo", "hora_inicio": "08:00", "hora_fin": "17:00", "tipo": "ADMINISTRATIVO", "color": "#795548"},
    ]
    count = 0
    for t in turnos:
        if not session.exec(select(TurnoTrabajo).where(TurnoTrabajo.nombre == t["nombre"])).first():
            session.add(TurnoTrabajo(**t))
            count += 1
    if count:
        logger.info(f"✓ {count} turnos de trabajo creados")


def seed_perfiles_laboratorio(session: Session):
    """Crea perfiles de laboratorio predefinidos"""
    perfiles = [
        {
            "nombre": "Hemograma Completo",
            "descripcion": "Cuadro hemático completo con diferencial",
            "examenes": [
                {"nombre": "Hemoglobina", "unidad": "g/dL", "ref_min": 12.0, "ref_max": 17.0},
                {"nombre": "Hematocrito", "unidad": "%", "ref_min": 36.0, "ref_max": 50.0},
                {"nombre": "Leucocitos", "unidad": "/mm³", "ref_min": 4500, "ref_max": 11000},
                {"nombre": "Plaquetas", "unidad": "/mm³", "ref_min": 150000, "ref_max": 400000},
                {"nombre": "Neutrófilos", "unidad": "%", "ref_min": 40, "ref_max": 70},
                {"nombre": "Linfocitos", "unidad": "%", "ref_min": 20, "ref_max": 45},
                {"nombre": "VCM", "unidad": "fL", "ref_min": 80, "ref_max": 100},
                {"nombre": "HCM", "unidad": "pg", "ref_min": 27, "ref_max": 33},
                {"nombre": "CHCM", "unidad": "g/dL", "ref_min": 32, "ref_max": 36},
            ]
        },
        {
            "nombre": "Perfil Lipídico",
            "descripcion": "Panel completo de lípidos en sangre",
            "examenes": [
                {"nombre": "Colesterol Total", "unidad": "mg/dL", "ref_min": 0, "ref_max": 200},
                {"nombre": "HDL", "unidad": "mg/dL", "ref_min": 40, "ref_max": 60},
                {"nombre": "LDL", "unidad": "mg/dL", "ref_min": 0, "ref_max": 130},
                {"nombre": "Triglicéridos", "unidad": "mg/dL", "ref_min": 0, "ref_max": 150},
                {"nombre": "VLDL", "unidad": "mg/dL", "ref_min": 0, "ref_max": 30},
            ]
        },
        {
            "nombre": "Química Sanguínea Básica",
            "descripcion": "Panel metabólico básico",
            "examenes": [
                {"nombre": "Glucosa", "unidad": "mg/dL", "ref_min": 70, "ref_max": 110},
                {"nombre": "BUN", "unidad": "mg/dL", "ref_min": 7, "ref_max": 20},
                {"nombre": "Creatinina", "unidad": "mg/dL", "ref_min": 0.6, "ref_max": 1.2},
                {"nombre": "Ácido Úrico", "unidad": "mg/dL", "ref_min": 3.0, "ref_max": 7.0},
            ]
        },
        {
            "nombre": "Función Hepática",
            "descripcion": "Enzimas y proteínas hepáticas",
            "examenes": [
                {"nombre": "TGO/AST", "unidad": "U/L", "ref_min": 0, "ref_max": 40},
                {"nombre": "TGP/ALT", "unidad": "U/L", "ref_min": 0, "ref_max": 40},
                {"nombre": "Fosfatasa Alcalina", "unidad": "U/L", "ref_min": 44, "ref_max": 147},
                {"nombre": "GGT", "unidad": "U/L", "ref_min": 0, "ref_max": 50},
                {"nombre": "Bilirrubina Total", "unidad": "mg/dL", "ref_min": 0, "ref_max": 1.2},
                {"nombre": "Bilirrubina Directa", "unidad": "mg/dL", "ref_min": 0, "ref_max": 0.3},
                {"nombre": "Albúmina", "unidad": "g/dL", "ref_min": 3.5, "ref_max": 5.5},
                {"nombre": "Proteínas Totales", "unidad": "g/dL", "ref_min": 6.0, "ref_max": 8.0},
            ]
        },
        {
            "nombre": "Función Tiroidea",
            "descripcion": "Hormonas tiroideas",
            "examenes": [
                {"nombre": "TSH", "unidad": "mUI/L", "ref_min": 0.4, "ref_max": 4.0},
                {"nombre": "T4 Libre", "unidad": "ng/dL", "ref_min": 0.8, "ref_max": 1.8},
                {"nombre": "T3 Total", "unidad": "ng/dL", "ref_min": 80, "ref_max": 200},
            ]
        },
        {
            "nombre": "Tiempos de Coagulación",
            "descripcion": "Panel de coagulación prequirúrgico",
            "examenes": [
                {"nombre": "PT (Tiempo Protrombina)", "unidad": "seg", "ref_min": 11, "ref_max": 14},
                {"nombre": "PTT", "unidad": "seg", "ref_min": 25, "ref_max": 35},
                {"nombre": "INR", "unidad": "", "ref_min": 0.8, "ref_max": 1.2},
                {"nombre": "Fibrinógeno", "unidad": "mg/dL", "ref_min": 200, "ref_max": 400},
            ]
        },
        {
            "nombre": "Uroanálisis Completo",
            "descripcion": "Examen de orina completo",
            "examenes": [
                {"nombre": "pH", "unidad": "", "ref_min": 5.0, "ref_max": 8.0},
                {"nombre": "Densidad", "unidad": "", "ref_min": 1.005, "ref_max": 1.030},
                {"nombre": "Proteínas", "unidad": "", "ref_min": 0, "ref_max": 0, "cualitativo": True},
                {"nombre": "Glucosa", "unidad": "", "ref_min": 0, "ref_max": 0, "cualitativo": True},
                {"nombre": "Leucocitos", "unidad": "/campo", "ref_min": 0, "ref_max": 5},
                {"nombre": "Eritrocitos", "unidad": "/campo", "ref_min": 0, "ref_max": 3},
                {"nombre": "Bacterias", "unidad": "", "ref_min": 0, "ref_max": 0, "cualitativo": True},
            ]
        },
        {
            "nombre": "Gases Arteriales",
            "descripcion": "Análisis de gases en sangre arterial",
            "examenes": [
                {"nombre": "pH", "unidad": "", "ref_min": 7.35, "ref_max": 7.45},
                {"nombre": "pCO2", "unidad": "mmHg", "ref_min": 35, "ref_max": 45},
                {"nombre": "pO2", "unidad": "mmHg", "ref_min": 80, "ref_max": 100},
                {"nombre": "HCO3", "unidad": "mEq/L", "ref_min": 22, "ref_max": 26},
                {"nombre": "SaO2", "unidad": "%", "ref_min": 95, "ref_max": 100},
                {"nombre": "Lactato", "unidad": "mmol/L", "ref_min": 0, "ref_max": 2},
            ]
        },
        {
            "nombre": "Electrolitos",
            "descripcion": "Panel de electrolitos séricos",
            "examenes": [
                {"nombre": "Sodio (Na)", "unidad": "mEq/L", "ref_min": 135, "ref_max": 145},
                {"nombre": "Potasio (K)", "unidad": "mEq/L", "ref_min": 3.5, "ref_max": 5.0},
                {"nombre": "Cloro (Cl)", "unidad": "mEq/L", "ref_min": 98, "ref_max": 106},
                {"nombre": "Calcio (Ca)", "unidad": "mg/dL", "ref_min": 8.5, "ref_max": 10.5},
                {"nombre": "Magnesio (Mg)", "unidad": "mg/dL", "ref_min": 1.7, "ref_max": 2.2},
            ]
        },
    ]
    count = 0
    for p in perfiles:
        if not session.exec(select(PerfilLaboratorio).where(PerfilLaboratorio.nombre == p["nombre"])).first():
            session.add(PerfilLaboratorio(
                nombre=p["nombre"],
                descripcion=p["descripcion"],
                examenes=p["examenes"]
            ))
            count += 1
    if count:
        logger.info(f"✓ {count} perfiles de laboratorio creados")


def seed_medicamentos_base(session: Session):
    """Crea medicamentos del catálogo base (CUM simplificado)"""
    medicamentos = [
        ("CUM001", "ACETAMINOFEN", "500 mg", "Tableta"),
        ("CUM002", "IBUPROFENO", "400 mg", "Tableta"),
        ("CUM003", "AMOXICILINA", "500 mg", "Cápsula"),
        ("CUM004", "LOSARTAN", "50 mg", "Tableta"),
        ("CUM005", "METFORMINA", "850 mg", "Tableta"),
        ("CUM006", "ENALAPRIL", "20 mg", "Tableta"),
        ("CUM007", "AMLODIPINO", "5 mg", "Tableta"),
        ("CUM008", "OMEPRAZOL", "20 mg", "Cápsula"),
        ("CUM009", "ATORVASTATINA", "40 mg", "Tableta"),
        ("CUM010", "METOPROLOL", "50 mg", "Tableta"),
        ("CUM011", "CIPROFLOXACINA", "500 mg", "Tableta"),
        ("CUM012", "CLINDAMICINA", "300 mg", "Cápsula"),
        ("CUM013", "DICLOFENACO", "50 mg", "Tableta"),
        ("CUM014", "TRAMADOL", "50 mg", "Cápsula"),
        ("CUM015", "LORATADINA", "10 mg", "Tableta"),
        ("CUM016", "CEFALEXINA", "500 mg", "Cápsula"),
        ("CUM017", "RANITIDINA", "150 mg", "Tableta"),
        ("CUM018", "FUROSEMIDA", "40 mg", "Tableta"),
        ("CUM019", "PREDNISONA", "5 mg", "Tableta"),
        ("CUM020", "HIDROCLOROTIAZIDA", "25 mg", "Tableta"),
        ("CUM021", "WARFARINA", "5 mg", "Tableta"),
        ("CUM022", "INSULINA GLARGINA", "100 UI/mL", "Solución inyectable"),
        ("CUM023", "ENOXAPARINA", "40 mg/0.4mL", "Jeringa prellenada"),
        ("CUM024", "CEFTRIAXONA", "1 g", "Polvo para inyección"),
        ("CUM025", "VANCOMICINA", "500 mg", "Polvo para inyección"),
        ("CUM026", "MIDAZOLAM", "5 mg/mL", "Solución inyectable"),
        ("CUM027", "FENTANILO", "0.5 mg/10mL", "Solución inyectable"),
        ("CUM028", "PROPOFOL", "10 mg/mL", "Emulsión inyectable"),
        ("CUM029", "MORFINA", "10 mg/mL", "Solución inyectable"),
        ("CUM030", "OXITOCINA", "10 UI/mL", "Solución inyectable"),
        ("CUM031", "SULFATO DE MAGNESIO", "20%", "Solución inyectable"),
        ("CUM032", "SALBUTAMOL", "100 mcg/dosis", "Inhalador"),
        ("CUM033", "BECLOMETASONA", "250 mcg/dosis", "Inhalador"),
        ("CUM034", "METOCLOPRAMIDA", "10 mg", "Tableta"),
        ("CUM035", "ONDANSETRON", "8 mg", "Tableta"),
    ]
    count = 0
    for cum, nombre, conc, forma in medicamentos:
        if not session.exec(select(Medicamento).where(Medicamento.codigo_cum == cum)).first():
            session.add(Medicamento(codigo_cum=cum, nombre=nombre, concentracion=conc, forma_farmaceutica=forma))
            count += 1
    if count:
        logger.info(f"✓ {count} medicamentos del catálogo base creados")


def main():
    logger.info("=" * 70)
    logger.info("  MED-SYS PRO v4.0 - Inicialización de Base de Datos")
    logger.info("  Sistema Completo para IPS / Hospitales / Clínicas")
    logger.info("=" * 70)

    init_db()

    with Session(engine) as session:
        # 1. Catálogos base (especialidades, entidades/EPS)
        logger.info("\n[01/11] Creando catálogos profesionales (especialidades + EPS)...")
        seed_catalogos_profesionales(session)
        session.commit()

        # 2. Institución de demostración con licencia ENTERPRISE completa
        logger.info("\n[02/11] Creando institución con licencia ENTERPRISE (todos los módulos)...")
        inst_demo = seed_institucion_demo(session)
        session.commit()

        # 3. Usuarios del sistema (todos los roles)
        logger.info("\n[03/11] Creando usuarios del sistema (16 roles)...")
        seed_usuarios_base(session, inst_demo)
        seed_usuario_enfermeria(session, inst_demo)
        seed_usuarios_roles_nuevos(session, inst_demo)
        session.commit()

        # 4. Catálogos médicos ampliados (CIE-10, CUPS)
        logger.info("\n[04/11] Creando catálogos médicos (CIE-10 + CUPS ampliados)...")
        seed_inteligente(session)
        session.commit()

        # 5. Medicamentos base (CUM)
        logger.info("\n[05/11] Creando catálogo de medicamentos...")
        seed_medicamentos_base(session)
        session.commit()

        # 6. Camas hospitalarias
        logger.info("\n[06/11] Creando camas hospitalarias...")
        seed_camas(session)
        session.commit()

        # 7. Quirófanos
        logger.info("\n[07/11] Creando quirófanos...")
        seed_quirofanos(session)
        session.commit()

        # 8. Turnos de trabajo
        logger.info("\n[08/11] Creando turnos de trabajo...")
        seed_turnos(session)
        session.commit()

        # 9. Perfiles de laboratorio
        logger.info("\n[09/11] Creando perfiles de laboratorio...")
        seed_perfiles_laboratorio(session)
        session.commit()

        # 10. Farmacia - inventario inicial
        logger.info("\n[10/11] Creando inventario de farmacia...")
        seed_farmacia(session)
        session.commit()

        # 11. Pacientes + Historias + Órdenes de prueba
        logger.info("\n[11/11] Creando datos clínicos de prueba...")
        entidad = session.exec(select(Entidad).where(Entidad.nit == "800088702")).first()
        medico = session.exec(select(Usuario).where(Usuario.username == "medico1")).first()
        if entidad and medico:
            pacientes = seed_pacientes_demo(session, inst_demo, entidad.id)
            session.commit()
            seed_historias_y_ordenes(session, pacientes, medico)
            session.commit()

    logger.info("\n" + "=" * 70)
    logger.info("  ✓✓✓ INICIALIZACIÓN COMPLETADA EXITOSAMENTE ✓✓✓")
    logger.info("  MED-SYS PRO v4.0 - Sistema Completo para IPS/Hospital")
    logger.info("=" * 70)
    logger.info("\n" + "=" * 70)
    logger.info("  DATOS SEMBRADOS:")
    logger.info("=" * 70)
    logger.info("  - 39 Especialidades médicas")
    logger.info("  - 14 Entidades (EPS contributivo/subsidiado, ARL, SOAT, Particular)")
    logger.info("  - 60+ Diagnósticos CIE-10 con filtros edad/sexo")
    logger.info("  - 50+ Procedimientos CUPS por sección")
    logger.info("  - 35 Medicamentos base (CUM)")
    logger.info("  - 9 Camas hospitalarias (General, UCI, Pediatría, Gineco, Aislamiento)")
    logger.info("  - 6 Quirófanos (General, Ambulatorio, Partos, Especializado)")
    logger.info("  - 6 Turnos de trabajo (Mañana, Tarde, Noche, 12h, Administrativo)")
    logger.info("  - 9 Perfiles de laboratorio con valores de referencia")
    logger.info("  - 9 Ítems de farmacia (medicamentos + insumos)")
    logger.info("  - 5 Pacientes de prueba con historias clínicas")
    logger.info("  - Licencia ENTERPRISE con 39 módulos habilitados")
    logger.info("")
    logger.info("  CREDENCIALES DE ACCESO:")
    logger.info("-" * 70)
    logger.info("  SUPERADMIN (Gestión Global Multi-Institucional):")
    logger.info("    Username: superadmin   Password: SuperAdmin2026!")
    logger.info("    URL: http://localhost:8004/superadmin")
    logger.info("")
    logger.info("  ADMIN (Clínica Demo):")
    logger.info("    Username: admin        Password: admin123")
    logger.info("")
    logger.info("  MEDICO (Clínica Demo):")
    logger.info("    Username: medico1      Password: medico123")
    logger.info("")
    logger.info("  ENFERMERIA (Clínica Demo):")
    logger.info("    Username: enfermera1   Password: enfermera123")
    logger.info("")
    logger.info("  ROLES ESPECIALIZADOS (Clínica Demo):")
    logger.info("    anestesiologo1 / aneste123    | laboratorista1 / lab123")
    logger.info("    bacteriologo1  / bact123      | radiologo1     / radio123")
    logger.info("    terapeuta1     / terap123      | nutricionista1 / nutri123")
    logger.info("    psicologo1     / psico123      | tsocial1       / tsocial123")
    logger.info("    regente1       / regente123    | auxiliar1      / auxiliar123")
    logger.info("-" * 70)
    logger.info("")
    logger.info("  MÓDULOS HABILITADOS (39 total):")
    logger.info("  Core: Citas, Pacientes, HC, Urgencias, Portal")
    logger.info("  Hospitalario: Hospitalización, Epicrisis, Interconsultas, MAR, Kardex")
    logger.info("  Quirúrgico: Quirófano, Nota Operatoria, Preanestesia, Recuperación")
    logger.info("  Diagnóstico: Laboratorio, Imágenes")
    logger.info("  Soporte: Farmacia, Enfermería, Evoluciones, Consentimiento, Banco Sangre")
    logger.info("  Especialidades: Nutrición, Rehabilitación, Trabajo Social, Telemedicina")
    logger.info("  Registros: Cert. Defunción, Reg. Nacimiento, Incapacidades, Remisiones")
    logger.info("  Admin: Facturación, RIPS, Auditoría, Estadísticas, Turnos, Comités")
    logger.info("         Equipos Biomédicos, Indicadores, Infecciones")
    logger.info("=" * 70)

if __name__ == "__main__":
    main()