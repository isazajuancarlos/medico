from sqlmodel import text
from database import engine
from models import * # Importamos los modelos para recrearlos luego

def reset_database():
    print("☢️  INICIANDO REINICIO NUCLEAR DE BASE DE DATOS...")
    
    with engine.begin() as conn:
        # En PostgreSQL, la forma más rápida de limpiar todo (incluyendo tablas viejas)
        # es borrar el esquema 'public' completo y volverlo a crear.
        
        print("🗑️  Eliminando esquema public con CASCADE...")
        conn.execute(text("DROP SCHEMA public CASCADE;"))
        
        print("🏗️  Recreando esquema public...")
        conn.execute(text("CREATE SCHEMA public;"))
        conn.execute(text("GRANT ALL ON SCHEMA public TO public;"))
        # Si tu usuario no es dueño de la BD, podría requerir ajustes, 
        # pero en entornos locales estándar esto funciona.
    
    print("✨  Creando nuevas tablas desde models.py...")
    # Ahora sí, SQLModel crea la estructura limpia y nueva
    SQLModel.metadata.create_all(engine)
    
    print("✅  Base de datos limpia, sincronizada y lista.")

if __name__ == "__main__":
    reset_database()