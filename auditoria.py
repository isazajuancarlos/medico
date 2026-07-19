"""
Sistema de Auditoría para MED-SYS Pro
Cumplimiento: Ley 1581/2012 (Habeas Data), Resolución 1995/1999

Registra TODOS los accesos y modificaciones a datos sensibles:
- Acceso a historias clínicas
- Modificación de datos de pacientes
- Creación/modificación de citas
- Acceso a resultados de laboratorio
"""

import hashlib
import json
from datetime import datetime
from typing import Optional, Dict, Any
from sqlmodel import Session
from database import engine
from models import AuditoriaAcceso, FirmaDigital, Historia, AuditoriaSuperAdmin


def registrar_auditoria(
    usuario_id: int,
    accion: str,
    tabla_afectada: str,
    registro_id: int,
    paciente_id: Optional[int] = None,
    datos_antes: Optional[Dict] = None,
    datos_despues: Optional[Dict] = None,
    ip_address: Optional[str] = None,
    user_agent: Optional[str] = None,
    exitoso: bool = True,
    mensaje_error: Optional[str] = None
):
    """
    Registra una acción en el log de auditoría.

    Args:
        usuario_id: ID del usuario que realiza la acción
        accion: Tipo de acción ("VER_HC", "MODIFICAR_HC", "CREAR_CITA", etc.)
        tabla_afectada: Nombre de la tabla afectada
        registro_id: ID del registro afectado
        paciente_id: ID del paciente (para filtros rápidos)
        datos_antes: Estado anterior del registro (JSON)
        datos_despues: Estado nuevo del registro (JSON)
        ip_address: IP del usuario
        user_agent: Navegador/dispositivo
        exitoso: Si la acción fue exitosa
        mensaje_error: Si hubo error, el mensaje
    """
    try:
        with Session(engine) as session:
            auditoria = AuditoriaAcceso(
                usuario_id=usuario_id,
                accion=accion,
                tabla_afectada=tabla_afectada,
                registro_id=registro_id,
                paciente_id=paciente_id,
                datos_antes=datos_antes,
                datos_despues=datos_despues,
                ip_address=ip_address,
                user_agent=user_agent,
                exitoso=exitoso,
                mensaje_error=mensaje_error
            )
            session.add(auditoria)
            session.commit()
    except Exception as e:
        # No fallar la operación principal si falla el log
        # Pero registrar el error para monitoreo
        print(f"ERROR AUDITORÍA: {e}")


def generar_hash_historia(historia_id: int) -> str:
    """
    Genera un hash SHA-256 del contenido completo de una historia clínica.
    Usado para verificar integridad y detectar alteraciones.

    Returns:
        Hash hexadecimal del documento
    """
    with Session(engine) as session:
        historia = session.get(Historia, historia_id)
        if not historia:
            return ""

        # Serializar todos los campos clínicos relevantes (Res. 1995/1999 - Integridad HC)
        contenido = {
            "id": historia.id,
            "codigo_historia": historia.codigo_historia,
            "fecha": historia.fecha.isoformat(),
            "paciente_id": historia.paciente_id,
            "medico_id": historia.medico_id,
            # Anamnesis
            "motivo_consulta": historia.motivo_consulta,
            "enfermedad_actual": historia.enfermedad_actual,
            # Signos vitales
            "tension_arterial": historia.tension_arterial,
            "frecuencia_cardiaca": historia.frecuencia_cardiaca,
            "frecuencia_respiratoria": historia.frecuencia_respiratoria,
            "temperatura": historia.temperatura,
            "peso": historia.peso,
            "talla": historia.talla,
            "imc": historia.imc,
            # Examen físico
            "ef_cabeza": historia.ef_cabeza,
            "ef_ojos": historia.ef_ojos,
            "ef_orl": historia.ef_orl,
            "ef_cuello": historia.ef_cuello,
            "ef_cardiopulmonar": historia.ef_cardiopulmonar,
            "ef_abdomen": historia.ef_abdomen,
            "ef_osteomuscular": historia.ef_osteomuscular,
            "ef_neurologico": historia.ef_neurologico,
            "ef_piel": historia.ef_piel,
            # Diagnósticos
            "diagnostico_principal": historia.diagnostico_principal,
            "diagnostico_relacionado1": historia.diagnostico_relacionado1,
            # Plan y tratamiento
            "plan_tratamiento": historia.plan_tratamiento,
            "genera_incapacidad": historia.genera_incapacidad,
            "dias_incapacidad": historia.dias_incapacidad,
            # Campos RIPS (Res. 2275/2023)
            "finalidad": historia.finalidad.value if historia.finalidad else None,
            "causa_externa": historia.causa_externa.value if historia.causa_externa else None,
            "tipo_diagnostico": historia.tipo_diagnostico.value if historia.tipo_diagnostico else None,
            "cod_consulta_cups": historia.cod_consulta_cups,
        }

        # Convertir a JSON ordenado y hashear
        contenido_json = json.dumps(contenido, sort_keys=True)
        hash_obj = hashlib.sha256(contenido_json.encode('utf-8'))
        return hash_obj.hexdigest()


def firmar_historia(medico_id: int, historia_id: int) -> bool:
    """
    Registra la firma digital de una historia clínica.

    En producción, aquí se integraría con un proveedor de firma digital
    como Certicámara, GSE, etc.

    Returns:
        True si se firmó exitosamente
    """
    try:
        hash_documento = generar_hash_historia(historia_id)

        with Session(engine) as session:
            firma = FirmaDigital(
                medico_id=medico_id,
                historia_id=historia_id,
                hash_documento=hash_documento,
                firmado_exitosamente=True,
                algoritmo="SHA256withRSA"
            )
            session.add(firma)
            session.commit()

        # Registrar en auditoría
        registrar_auditoria(
            usuario_id=medico_id,
            accion="FIRMAR_HC",
            tabla_afectada="historia",
            registro_id=historia_id,
            exitoso=True
        )

        return True
    except Exception as e:
        print(f"ERROR FIRMA DIGITAL: {e}")
        return False


def verificar_integridad_historia(historia_id: int) -> bool:
    """
    Verifica que una historia clínica no haya sido alterada después de firmarse.

    Returns:
        True si el hash actual coincide con el hash firmado
    """
    with Session(engine) as session:
        # Obtener la última firma
        from sqlmodel import select
        firma = session.exec(
            select(FirmaDigital)
            .where(FirmaDigital.historia_id == historia_id)
            .order_by(FirmaDigital.timestamp.desc())
        ).first()

        if not firma:
            return False  # No está firmada

        # Calcular hash actual
        hash_actual = generar_hash_historia(historia_id)

        # Comparar
        return hash_actual == firma.hash_documento


def obtener_log_paciente(paciente_id: int, limit: int = 50):
    """
    Obtiene el log de auditoría de un paciente específico.
    Útil para responder derechos de Habeas Data.

    Returns:
        Lista de accesos al expediente del paciente
    """
    with Session(engine) as session:
        from sqlmodel import select
        from sqlalchemy.orm import joinedload

        logs = session.exec(
            select(AuditoriaAcceso)
            .options(joinedload(AuditoriaAcceso.usuario))  # Necesita relación en modelo
            .where(AuditoriaAcceso.paciente_id == paciente_id)
            .order_by(AuditoriaAcceso.fecha_hora.desc())
            .limit(limit)
        ).all()

        return [
            {
                "fecha": log.fecha_hora.strftime("%Y-%m-%d %H:%M:%S"),
                "usuario": log.usuario_id,  # Reemplazar con nombre cuando exista relación
                "accion": log.accion,
                "tabla": log.tabla_afectada,
                "ip": log.ip_address,
                "exitoso": log.exitoso
            }
            for log in logs
        ]


# ==============================================================================
# FUNCIONES DE CONVENIENCIA (Para usar en los módulos)
# ==============================================================================

def auditar_ver_historia(usuario_id: int, historia_id: int, paciente_id: int, ip: str = None):
    """Shortcut para registrar visualización de HC"""
    registrar_auditoria(
        usuario_id=usuario_id,
        accion="VER_HC",
        tabla_afectada="historia",
        registro_id=historia_id,
        paciente_id=paciente_id,
        ip_address=ip
    )


def auditar_modificar_paciente(usuario_id: int, paciente_id: int, datos_antes: dict, datos_despues: dict):
    """Shortcut para registrar modificación de datos de paciente"""
    registrar_auditoria(
        usuario_id=usuario_id,
        accion="MODIFICAR_PACIENTE",
        tabla_afectada="paciente",
        registro_id=paciente_id,
        paciente_id=paciente_id,
        datos_antes=datos_antes,
        datos_despues=datos_despues
    )


def auditar_crear_cita(usuario_id: int, cita_id: int, paciente_id: int):
    """Shortcut para registrar creación de cita"""
    registrar_auditoria(
        usuario_id=usuario_id,
        accion="CREAR_CITA",
        tabla_afectada="cita",
        registro_id=cita_id,
        paciente_id=paciente_id
    )

# ==============================================================================
# AUDITORÍA DE SUPER-ADMINISTRADOR
# ==============================================================================

def auditar_superadmin(
    superadmin_id: int,
    accion: str,
    descripcion: str,
    institucion_afectada_id: Optional[int] = None,
    tabla_afectada: Optional[str] = None,
    registros_afectados: int = 0,
    detalles: Optional[Dict] = None,
    ip_address: Optional[str] = None,
    user_agent: Optional[str] = None,
    exitoso: bool = True,
    mensaje_error: Optional[str] = None
):
    """
    Registra una acción realizada por el SUPERADMIN.

    Args:
        superadmin_id: ID del usuario SUPERADMIN
        accion: Tipo de acción ("IMPERSONAR_INSTITUCION", "CREAR_USUARIO", "BACKUP_BD", etc.)
        descripcion: Descripción legible de la acción
        institucion_afectada_id: ID de la institución afectada (si aplica)
        tabla_afectada: Nombre de la tabla afectada (si aplica)
        registros_afectados: Número de registros modificados/eliminados
        detalles: Información adicional en formato dict
        ip_address: IP del SUPERADMIN
        user_agent: Navegador/dispositivo
        exitoso: Si la acción fue exitosa
        mensaje_error: Si hubo error, el mensaje
    """
    try:
        with Session(engine) as session:
            auditoria = AuditoriaSuperAdmin(
                superadmin_id=superadmin_id,
                accion=accion,
                descripcion=descripcion,
                institucion_afectada_id=institucion_afectada_id,
                tabla_afectada=tabla_afectada,
                registros_afectados=registros_afectados,
                detalles=detalles or {},
                ip_address=ip_address,
                user_agent=user_agent,
                exitoso=exitoso,
                mensaje_error=mensaje_error
            )
            session.add(auditoria)
            session.commit()
    except Exception as e:
        print(f"ERROR AUDITORÍA SUPERADMIN: {e}")


def auditar_impersonacion(superadmin_id: int, institucion_id: int, ip: str = None):
    """Registra cuando un SUPERADMIN impersona una institución"""
    auditar_superadmin(
        superadmin_id=superadmin_id,
        accion="IMPERSONAR_INSTITUCION",
        descripcion=f"Accedió al contexto de la institución ID: {institucion_id}",
        institucion_afectada_id=institucion_id,
        ip_address=ip
    )


def auditar_backup_bd(superadmin_id: int, tipo: str, tamano_mb: float, ip: str = None):
    """Registra creación de backup de base de datos"""
    auditar_superadmin(
        superadmin_id=superadmin_id,
        accion="BACKUP_BD",
        descripcion=f"Creó backup de BD ({tipo})",
        detalles={"tipo": tipo, "tamano_mb": tamano_mb},
        ip_address=ip
    )


def auditar_restauracion_bd(superadmin_id: int, archivo: str, exitoso: bool, error: str = None, ip: str = None):
    """Registra restauración de base de datos"""
    auditar_superadmin(
        superadmin_id=superadmin_id,
        accion="RESTAURAR_BD",
        descripcion=f"Restauró BD desde archivo: {archivo}",
        detalles={"archivo": archivo},
        exitoso=exitoso,
        mensaje_error=error,
        ip_address=ip
    )


def auditar_carga_masiva(superadmin_id: int, tabla: str, registros: int, institucion_id: int = None, ip: str = None):
    """Registra carga masiva de datos"""
    auditar_superadmin(
        superadmin_id=superadmin_id,
        accion="CARGA_MASIVA",
        descripcion=f"Carga masiva en tabla {tabla}: {registros} registros",
        institucion_afectada_id=institucion_id,
        tabla_afectada=tabla,
        registros_afectados=registros,
        ip_address=ip
    )


def obtener_log_superadmin(superadmin_id: Optional[int] = None, limit: int = 100):
    """
    Obtiene el log de auditoría de acciones del SUPERADMIN.

    Args:
        superadmin_id: Filtrar por un SUPERADMIN específico (None = todos)
        limit: Número máximo de registros a retornar

    Returns:
        Lista de acciones del SUPERADMIN
    """
    with Session(engine) as session:
        from sqlmodel import select

        query = select(AuditoriaSuperAdmin).order_by(AuditoriaSuperAdmin.fecha_hora.desc()).limit(limit)

        if superadmin_id:
            query = query.where(AuditoriaSuperAdmin.superadmin_id == superadmin_id)

        logs = session.exec(query).all()

        return [
            {
                "id": log.id,
                "fecha": log.fecha_hora.strftime("%Y-%m-%d %H:%M:%S"),
                "superadmin_id": log.superadmin_id,
                "accion": log.accion,
                "descripcion": log.descripcion,
                "institucion_id": log.institucion_afectada_id,
                "tabla": log.tabla_afectada,
                "registros": log.registros_afectados,
                "ip": log.ip_address,
                "exitoso": log.exitoso,
                "error": log.mensaje_error,
                "detalles": log.detalles
            }
            for log in logs
        ]
