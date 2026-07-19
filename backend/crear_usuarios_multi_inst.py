#!/usr/bin/env python3
"""
Script auxiliar para crear usuarios multi-institucionales.
Solo debe ejecutarse por el SUPERADMIN para pruebas.

USO:
    python crear_usuarios_multi_inst.py

IMPORTANTE: Este script es temporal mientras se desarrolla la UI
completa para que SUPERADMIN cree usuarios desde el panel web.
"""

import bcrypt
from sqlmodel import Session, select
from database import engine
from models import Usuario, Institucion, RolUsuario

def hash_password(password: str) -> str:
    """Genera hash bcrypt de una contraseña"""
    return bcrypt.hashpw(password.encode('utf-8'), bcrypt.gensalt()).decode('utf-8')

def listar_instituciones():
    """Muestra todas las instituciones disponibles"""
    with Session(engine) as session:
        instituciones = session.exec(select(Institucion)).all()

        print("\n" + "="*80)
        print("INSTITUCIONES REGISTRADAS")
        print("="*80)
        print(f"{'ID':<5} {'NIT':<15} {'Nombre Comercial':<40} {'Activa':<10}")
        print("-"*80)

        for inst in instituciones:
            activa = "SÍ" if inst.activa else "NO"
            print(f"{inst.id:<5} {inst.nit:<15} {inst.nombre_comercial:<40} {activa:<10}")

        print("="*80)
        return instituciones

def crear_usuario_interactivo():
    """Crea un usuario de forma interactiva"""

    # Listar instituciones
    instituciones = listar_instituciones()

    if not instituciones:
        print("\n❌ No hay instituciones registradas. Cree una primero desde el panel web.")
        return

    print("\n" + "="*80)
    print("CREAR NUEVO USUARIO")
    print("="*80)

    # Solicitar datos
    username = input("\nUsername (ej: admin_santamaria): ").strip()
    if not username:
        print("❌ Username es obligatorio")
        return

    # Verificar si ya existe
    with Session(engine) as session:
        existe = session.exec(select(Usuario).where(Usuario.username == username)).first()
        if existe:
            print(f"❌ El username '{username}' ya existe")
            return

    password = input("Contraseña (ej: Admin123!): ").strip()
    if not password:
        print("❌ Contraseña es obligatoria")
        return

    nombre_completo = input("Nombre Completo (ej: Juan Pérez Admin): ").strip()
    if not nombre_completo:
        print("❌ Nombre completo es obligatorio")
        return

    # Rol
    print("\nRoles disponibles:")
    print("  1. ADMIN")
    print("  2. MEDICO")
    print("  3. ENFERMERIA")
    print("  4. PACIENTE")

    rol_opcion = input("Seleccione rol (1-4): ").strip()

    roles_map = {
        '1': RolUsuario.ADMIN,
        '2': RolUsuario.MEDICO,
        '3': RolUsuario.ENFERMERIA,
        '4': RolUsuario.PACIENTE
    }

    if rol_opcion not in roles_map:
        print("❌ Opción de rol inválida")
        return

    rol = roles_map[rol_opcion]

    # Institución
    institucion_id_input = input("\nID de Institución (ver tabla arriba): ").strip()
    try:
        institucion_id = int(institucion_id_input)
    except ValueError:
        print("❌ ID de institución debe ser un número")
        return

    # Verificar que la institución existe
    with Session(engine) as session:
        institucion = session.get(Institucion, institucion_id)
        if not institucion:
            print(f"❌ No existe institución con ID {institucion_id}")
            return

    # Campos específicos para médicos
    especialidad = None
    registro_medico = None

    if rol == RolUsuario.MEDICO:
        especialidad = input("Especialidad (ej: Medicina General): ").strip()
        registro_medico = input("Registro Médico (ej: RM-12345): ").strip()

    # Confirmación
    print("\n" + "-"*80)
    print("RESUMEN:")
    print(f"  Username: {username}")
    print(f"  Nombre: {nombre_completo}")
    print(f"  Rol: {rol.value}")
    print(f"  Institución: {institucion.nombre_comercial} (ID: {institucion_id})")
    if especialidad:
        print(f"  Especialidad: {especialidad}")
    if registro_medico:
        print(f"  Registro Médico: {registro_medico}")
    print("-"*80)

    confirmar = input("\n¿Crear este usuario? (s/n): ").strip().lower()

    if confirmar != 's':
        print("❌ Operación cancelada")
        return

    # Crear usuario
    try:
        with Session(engine) as session:
            nuevo_usuario = Usuario(
                username=username,
                hashed_password=hash_password(password),
                nombre_completo=nombre_completo,
                rol=rol,
                activo=True,
                institucion_id=institucion_id,
                especialidad=especialidad,
                registro_medico=registro_medico
            )

            session.add(nuevo_usuario)
            session.commit()
            session.refresh(nuevo_usuario)

            print("\n✅ Usuario creado exitosamente!")
            print(f"   ID: {nuevo_usuario.id}")
            print(f"   Username: {nuevo_usuario.username}")
            print(f"   Rol: {nuevo_usuario.rol.value}")
            print(f"   Institución: {institucion.nombre_comercial}")
            print("\nPuede usar estas credenciales para hacer login:")
            print(f"   Usuario: {username}")
            print(f"   Contraseña: {password}")

    except Exception as e:
        print(f"\n❌ Error al crear usuario: {e}")

def crear_usuarios_demo():
    """Crea usuarios de demostración para pruebas rápidas"""

    print("\n" + "="*80)
    print("CREAR USUARIOS DE DEMOSTRACIÓN")
    print("="*80)

    # Verificar instituciones
    with Session(engine) as session:
        instituciones = session.exec(select(Institucion)).all()

        if len(instituciones) < 2:
            print("❌ Se necesitan al menos 2 instituciones para crear usuarios demo")
            print("   Cree instituciones desde el panel web primero")
            return

        inst1 = instituciones[0]
        inst2 = instituciones[1] if len(instituciones) > 1 else instituciones[0]

        print(f"\nCreando usuarios para:")
        print(f"  - Institución 1: {inst1.nombre_comercial} (ID: {inst1.id})")
        print(f"  - Institución 2: {inst2.nombre_comercial} (ID: {inst2.id})")

        usuarios_demo = [
            # Institución 1
            {
                'username': f'admin_{inst1.id}',
                'password': 'Admin123!',
                'nombre_completo': f'Admin {inst1.nombre_comercial}',
                'rol': RolUsuario.ADMIN,
                'institucion_id': inst1.id
            },
            {
                'username': f'medico_{inst1.id}_1',
                'password': 'Medico123!',
                'nombre_completo': f'Dr. Carlos Rodríguez ({inst1.nombre_comercial})',
                'rol': RolUsuario.MEDICO,
                'institucion_id': inst1.id,
                'especialidad': 'Medicina General',
                'registro_medico': f'RM-{inst1.id}001'
            },
            # Institución 2
            {
                'username': f'admin_{inst2.id}',
                'password': 'Admin123!',
                'nombre_completo': f'Admin {inst2.nombre_comercial}',
                'rol': RolUsuario.ADMIN,
                'institucion_id': inst2.id
            },
            {
                'username': f'medico_{inst2.id}_1',
                'password': 'Medico123!',
                'nombre_completo': f'Dra. María López ({inst2.nombre_comercial})',
                'rol': RolUsuario.MEDICO,
                'institucion_id': inst2.id,
                'especialidad': 'Pediatría',
                'registro_medico': f'RM-{inst2.id}001'
            }
        ]

        creados = 0
        for u_data in usuarios_demo:
            # Verificar si ya existe
            existe = session.exec(
                select(Usuario).where(Usuario.username == u_data['username'])
            ).first()

            if existe:
                print(f"⚠️  Usuario '{u_data['username']}' ya existe, saltando...")
                continue

            try:
                password_original = u_data.pop('password')
                nuevo = Usuario(
                    **u_data,
                    hashed_password=hash_password(password_original),
                    activo=True
                )
                session.add(nuevo)
                session.flush()

                print(f"✅ Creado: {u_data['username']} / {password_original} ({u_data['rol'].value})")
                creados += 1

            except Exception as e:
                print(f"❌ Error creando {u_data['username']}: {e}")

        session.commit()
        print(f"\n✅ Total usuarios demo creados: {creados}")

def menu_principal():
    """Menú principal del script"""

    print("\n" + "="*80)
    print("GESTOR DE USUARIOS MULTI-INSTITUCIONALES")
    print("MED-SYS Pro v2.0 - Herramienta para SUPERADMIN")
    print("="*80)

    while True:
        print("\nOpciones:")
        print("  1. Listar instituciones")
        print("  2. Crear usuario interactivo")
        print("  3. Crear usuarios de demostración (rápido)")
        print("  4. Salir")

        opcion = input("\nSeleccione opción (1-4): ").strip()

        if opcion == '1':
            listar_instituciones()
        elif opcion == '2':
            crear_usuario_interactivo()
        elif opcion == '3':
            crear_usuarios_demo()
        elif opcion == '4':
            print("\n👋 ¡Hasta luego!")
            break
        else:
            print("❌ Opción inválida")

if __name__ == '__main__':
    try:
        menu_principal()
    except KeyboardInterrupt:
        print("\n\n👋 Operación cancelada por el usuario")
    except Exception as e:
        print(f"\n❌ Error inesperado: {e}")
        import traceback
        traceback.print_exc()
