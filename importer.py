import requests
import logging
from sqlmodel import Session, select
from database import engine, init_db
from models import Medicamento, Cie10, Cups, Entidad

# Configuración
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger("MED-SYS IMPORTER")

# --- URLs de Datos Abiertos Colombia (Socrata API) ---
# Endpoint oficial INVIMA (Medicamentos) - Ejemplo con límite para no saturar
API_MEDICAMENTOS = "https://www.datos.gov.co/resource/i7cb-raxc.json?$limit=1000"

def importar_medicamentos():
    logger.info("⬇️  Descargando medicamentos desde datos.gov.co...")
    try:
        response = requests.get(API_MEDICAMENTOS, timeout=10)
        data = response.json()
        
        with Session(engine) as session:
            count = 0
            for item in data:
                # Mapeo de campos API -> Modelo
                cum = item.get('expediente', '') # Simplificación, el CUM real es complejo
                nombre = f"{item.get('producto', '')} - {item.get('principio_activo', '')}"
                
                # Evitar duplicados
                if not session.exec(select(Medicamento).where(Medicamento.codigo_cum == cum)).first():
                    med = Medicamento(
                        codigo_cum=cum,
                        nombre=nombre[:100], # Cortar si es muy largo
                        concentracion=item.get('concentracion', 'N/A'),
                        forma_farmaceutica=item.get('forma_farmaceutica', 'N/A'),
                        titular=item.get('titular', 'N/A')
                    )
                    session.add(med)
                    count += 1
            
            session.commit()
            logger.info(f"✅  Importados {count} medicamentos nuevos.")
            
    except Exception as e:
        logger.error(f"❌  Error importando medicamentos: {e}")

def importar_cie10_base():
    """Carga una base sólida de diagnósticos comunes para RIPS"""
    logger.info("📄  Cargando diagnósticos CIE-10 base...")
    # Lista resumida de los 20 más usados en consulta externa
    diagnosticos = [
        ("A09", "DIARREA Y GASTROENTERITIS DE PRESUNTO ORIGEN INFECCIOSO"),
        ("I10", "HIPERTENSION ESENCIAL (PRIMARIA)"),
        ("E11", "DIABETES MELLITUS NO INSULINODEPENDIENTE"),
        ("J00", "RINOFARINGITIS AGUDA [RESFRIADO COMUN]"),
        ("J03", "AMIGDALITIS AGUDA"),
        ("N390", "INFECCION DE VIAS URINARIAS, SITIO NO ESPECIFICADO"),
        ("M545", "LUMBAGO NO ESPECIFICADO"),
        ("R51", "CEFALEA"),
        ("K297", "GASTRITIS, NO ESPECIFICADA"),
        ("Z000", "EXAMEN MEDICO GENERAL"),
        ("Z010", "EXAMEN DE OJOS Y DE LA VISION"),
        ("Z012", "EXAMEN ODONTOLOGICO"),
        ("G43", "MIGRAÑA"),
        ("H10", "CONJUNTIVITIS"),
        ("L20", "DERMATITIS ATOPICA")
    ]
    
    with Session(engine) as session:
        count = 0
        for code, desc in diagnosticos:
            if not session.exec(select(Cie10).where(Cie10.codigo == code)).first():
                session.add(Cie10(codigo=code, descripcion=desc))
                count += 1
        session.commit()
        logger.info(f"✅  Cargados {count} diagnósticos CIE-10.")

def importar_cups_base():
    """Carga los procedimientos CUPS más comunes en Colombia (2024-2025)"""
    logger.info("📄  Cargando procedimientos CUPS base...")
    # Estos códigos son reales y se usan en facturación
    procedimientos = [
        ("890201", "CONSULTA DE PRIMERA VEZ POR MEDICINA GENERAL", "CONSULTA"),
        ("890301", "CONSULTA DE CONTROL O DE SEGUIMIENTO POR MEDICINA GENERAL", "CONSULTA"),
        ("890203", "CONSULTA DE PRIMERA VEZ POR ESPECIALISTA EN PEDIATRIA", "CONSULTA"),
        ("902207", "HEMOGRAMA IV (HEMOGLOBINA HEMATOCRITO RECUENTO DE ERITROCITOS INDICES ERITROCITARIOS LEUCOGRAMA RECUENTO DE PLAQUETAS E INDICES PLAQUETARIOS Y MORFOLOGIA ELECTRONICA)", "LABORATORIO"),
        ("903841", "CREATININA EN SUERO U OTROS FLUIDOS", "LABORATORIO"),
        ("903825", "GLUCOSA EN SUERO U OTRO FLUIDO DIFERENTE A ORINA [GLICEMIA]", "LABORATORIO"),
        ("902213", "VELOCIDAD DE SEDIMENTACION GLOBULAR [VSG]", "LABORATORIO"),
        ("906913", "PROTEINA C REACTIVA [PCR]", "LABORATORIO"),
        ("871020", "RADIOGRAFIA DE TORAX (PA O AP Y LATERAL)", "IMAGENOLOGIA"),
        ("881301", "ECOGRAFIA OBSTETRICA", "IMAGENOLOGIA"),
        ("895100", "ELECTROCARDIOGRAMA DE RITMO O DE SUPERFICIE", "CARDIOLOGIA")
    ]
    
    with Session(engine) as session:
        count = 0
        for code, desc, sec in procedimientos:
            if not session.exec(select(Cups).where(Cups.codigo == code)).first():
                session.add(Cups(codigo=code, descripcion=desc, seccion=sec))
                count += 1
        session.commit()
        logger.info(f"✅  Cargados {count} procedimientos CUPS.")

def importar_eps():
    logger.info("🏥  Cargando EPS oficiales...")
    eps_list = [
        {"nit": "800130907", "nombre": "SALUD TOTAL S.A. E.P.S.", "cod": "EPS002"},
        {"nit": "800251440", "nombre": "EPS SANITAS", "cod": "EPS005"},
        {"nit": "800088702", "nombre": "EPS SURA", "cod": "EPS010"},
        {"nit": "900156264", "nombre": "NUEVA EPS S.A.", "cod": "EPS037"},
        {"nit": "890904996", "nombre": "SAVIA SALUD EPS", "cod": "EPS040"},
    ]
    
    with Session(engine) as session:
        for e in eps_list:
            if not session.exec(select(Entidad).where(Entidad.nit == e['nit'])).first():
                session.add(Entidad(nit=e['nit'], nombre=e['nombre'], codigo_eps=e['cod']))
        session.commit()

if __name__ == "__main__":
    init_db() # Asegura que las tablas existan con la estructura nueva
    print("--- INICIANDO IMPORTACIÓN DE MAESTROS RIPS ---")
    importar_eps()
    importar_cie10_base()
    importar_cups_base()
    importar_medicamentos() # ESTE CONECTA A INTERNET
    print("--- PROCESO FINALIZADO ---")