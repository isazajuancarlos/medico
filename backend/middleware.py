"""
Middleware para aislamiento de datos, control de licencias y seguridad.
"""

from sqlmodel import Session, select
from nicegui import app
from datetime import datetime, date
from typing import Optional, List, Tuple
from models import (
    Institucion, Licencia, PoliticaSeguridad, Usuario,
    SesionActiva, ModuloSistema, EstadoLicencia, IntentoLogin
)
from seguridad import GestorSesiones, ControlRateLimit
from database import engine

# ==============================================================================
# AISLAMIENTO DE DATOS POR INSTITUCIÓN
# ==============================================================================

class ContextoInstitucion:
    """
    Gestor de contexto de institución para aislar datos.
    Se usa en todas las consultas a BD para filtrar por institución.
    """

    @staticmethod
    def get_institucion_id() -> Optional[int]:
        """Obtiene el ID de institución del usuario actual"""
        return app.storage.user.get('institucion_id')

    @staticmethod
    def set_institucion_id(institucion_id: int) -> None:
        """Establece el ID de institución en el contexto"""
        app.storage.user['institucion_id'] = institucion_id

    @staticmethod
    def get_institucion() -> Optional[Institucion]:
        """Obtiene la institución completa del usuario actual"""
        institucion_id = ContextoInstitucion.get_institucion_id()
        if not institucion_id:
            return None

        with Session(engine) as session:
            return session.get(Institucion, institucion_id)

    @staticmethod
    def filtrar_query(query, modelo):
        """
        Agrega filtro de institución a una query automáticamente.

        Uso:
            query = select(Paciente)
            query = ContextoInstitucion.filtrar_query(query, Paciente)
        """
        institucion_id = ContextoInstitucion.get_institucion_id()

        if institucion_id and hasattr(modelo, 'institucion_id'):
            return query.where(modelo.institucion_id == institucion_id)

        return query

# ==============================================================================
# CONTROL DE LICENCIAS
# ==============================================================================

class ValidadorLicencia:
    """Valida que la institución tenga licencia activa y permisos"""

    @staticmethod
    def get_licencia_activa() -> Optional[Licencia]:
        """Obtiene la licencia activa de la institución actual"""
        institucion_id = ContextoInstitucion.get_institucion_id()
        if not institucion_id:
            return None

        with Session(engine) as session:
            licencia = session.exec(
                select(Licencia).where(
                    Licencia.institucion_id == institucion_id,
                    Licencia.estado == EstadoLicencia.ACTIVA
                )
            ).first()

            return licencia

    @staticmethod
    def licencia_valida() -> Tuple[bool, str]:
        """
        Verifica si la institución tiene licencia válida.

        Returns:
            Tuple[bool, str]: (valida, mensaje_error)
        """
        licencia = ValidadorLicencia.get_licencia_activa()

        if not licencia:
            return False, "No hay licencia activa para esta institución"

        # Verificar vencimiento
        if licencia.fecha_vencimiento < date.today():
            return False, f"Licencia vencida el {licencia.fecha_vencimiento}"

        return True, ""

    @staticmethod
    def modulo_habilitado(modulo: str) -> bool:
        """Verifica si un módulo está habilitado en la licencia"""
        licencia = ValidadorLicencia.get_licencia_activa()

        if not licencia:
            return False

        return modulo.upper() in [m.upper() for m in licencia.modulos_habilitados]

    @staticmethod
    def verificar_limite_usuarios() -> Tuple[bool, str]:
        """Verifica si se puede crear un nuevo usuario"""
        licencia = ValidadorLicencia.get_licencia_activa()

        if not licencia:
            return False, "No hay licencia activa"

        if licencia.uso_actual_usuarios >= licencia.max_usuarios:
            return False, f"Límite de usuarios alcanzado ({licencia.max_usuarios})"

        return True, ""

    @staticmethod
    def verificar_limite_medicos() -> Tuple[bool, str]:
        """Verifica si se puede crear un nuevo médico"""
        licencia = ValidadorLicencia.get_licencia_activa()

        if not licencia:
            return False, "No hay licencia activa"

        if licencia.uso_actual_medicos >= licencia.max_medicos:
            return False, f"Límite de médicos alcanzado ({licencia.max_medicos})"

        return True, ""

    @staticmethod
    def verificar_limite_pacientes() -> Tuple[bool, str]:
        """Verifica si se puede crear un nuevo paciente"""
        licencia = ValidadorLicencia.get_licencia_activa()

        if not licencia:
            return False, "No hay licencia activa"

        if licencia.uso_actual_pacientes >= licencia.max_pacientes:
            return False, f"Límite de pacientes alcanzado ({licencia.max_pacientes})"

        return True, ""

    @staticmethod
    def verificar_limite_citas_mes() -> Tuple[bool, str]:
        """Verifica si se puede crear una nueva cita este mes"""
        licencia = ValidadorLicencia.get_licencia_activa()

        if not licencia:
            return False, "No hay licencia activa"

        if licencia.uso_citas_mes_actual >= licencia.max_citas_mes:
            return False, f"Límite de citas mensuales alcanzado ({licencia.max_citas_mes})"

        return True, ""

    @staticmethod
    def actualizar_uso_usuarios(incremento: int = 1):
        """Incrementa el contador de usuarios activos"""
        institucion_id = ContextoInstitucion.get_institucion_id()
        if not institucion_id:
            return

        with Session(engine) as session:
            licencia = session.exec(
                select(Licencia).where(
                    Licencia.institucion_id == institucion_id,
                    Licencia.estado == EstadoLicencia.ACTIVA
                )
            ).first()

            if licencia:
                licencia.uso_actual_usuarios += incremento
                session.add(licencia)
                session.commit()

    @staticmethod
    def actualizar_uso_pacientes(incremento: int = 1):
        """Incrementa el contador de pacientes"""
        institucion_id = ContextoInstitucion.get_institucion_id()
        if not institucion_id:
            return

        with Session(engine) as session:
            licencia = session.exec(
                select(Licencia).where(
                    Licencia.institucion_id == institucion_id,
                    Licencia.estado == EstadoLicencia.ACTIVA
                )
            ).first()

            if licencia:
                licencia.uso_actual_pacientes += incremento
                session.add(licencia)
                session.commit()

# ==============================================================================
# CONTROL DE SESIONES
# ==============================================================================

class ControladorSesion:
    """Gestión de sesiones con timeout y validación"""

    @staticmethod
    def crear_sesion(usuario_id: int, institucion_id: int, ip: str, user_agent: str) -> str:
        """
        Crea una nueva sesión y retorna el token.

        Returns:
            str: Token de sesión
        """
        # Obtener política de seguridad
        with Session(engine) as session:
            politica = session.exec(
                select(PoliticaSeguridad).where(
                    PoliticaSeguridad.institucion_id == institucion_id
                )
            ).first()

            timeout_minutos = politica.timeout_sesion_minutos if politica else 30

            # Cerrar otras sesiones si está configurado
            if politica and politica.cerrar_otras_sesiones_al_login:
                session.exec(
                    select(SesionActiva).where(
                        SesionActiva.usuario_id == usuario_id,
                        SesionActiva.activa == True
                    )
                ).all()

                for s in session.exec(select(SesionActiva).where(SesionActiva.usuario_id == usuario_id)):
                    s.activa = False
                    session.add(s)

            # Generar token
            token = GestorSesiones.generar_session_token()

            # Crear sesión
            nueva_sesion = SesionActiva(
                usuario_id=usuario_id,
                institucion_id=institucion_id,
                session_token=token,
                ip_address=ip,
                user_agent=user_agent,
                fecha_expiracion=GestorSesiones.calcular_expiracion_sesion(timeout_minutos)
            )

            session.add(nueva_sesion)
            session.commit()

            return token

    @staticmethod
    def validar_sesion(token: str) -> Tuple[bool, Optional[SesionActiva]]:
        """
        Valida si una sesión es válida y activa.

        Returns:
            Tuple[bool, Optional[SesionActiva]]: (valida, sesion)
        """
        with Session(engine) as session:
            sesion = session.exec(
                select(SesionActiva).where(
                    SesionActiva.session_token == token,
                    SesionActiva.activa == True
                )
            ).first()

            if not sesion:
                return False, None

            # Obtener política
            politica = session.exec(
                select(PoliticaSeguridad).where(
                    PoliticaSeguridad.institucion_id == sesion.institucion_id
                )
            ).first()

            timeout = politica.timeout_sesion_minutos if politica else 30

            # Validar tiempos
            if not GestorSesiones.sesion_valida(
                sesion.fecha_ultima_actividad,
                sesion.fecha_expiracion,
                timeout
            ):
                # Marcar como inactiva
                sesion.activa = False
                session.add(sesion)
                session.commit()
                return False, None

            return True, sesion

    @staticmethod
    def actualizar_actividad(token: str):
        """Actualiza la última actividad de una sesión"""
        with Session(engine) as session:
            sesion = session.exec(
                select(SesionActiva).where(
                    SesionActiva.session_token == token
                )
            ).first()

            if sesion and sesion.activa:
                sesion.fecha_ultima_actividad = datetime.now()
                session.add(sesion)
                session.commit()

    @staticmethod
    def cerrar_sesion(token: str):
        """Cierra una sesión"""
        with Session(engine) as session:
            sesion = session.exec(
                select(SesionActiva).where(
                    SesionActiva.session_token == token
                )
            ).first()

            if sesion:
                sesion.activa = False
                sesion.cerrada_por_usuario = True
                session.add(sesion)
                session.commit()

    @staticmethod
    def cerrar_todas_sesiones_usuario(usuario_id: int):
        """Cierra todas las sesiones de un usuario"""
        with Session(engine) as session:
            sesiones = session.exec(
                select(SesionActiva).where(
                    SesionActiva.usuario_id == usuario_id,
                    SesionActiva.activa == True
                )
            ).all()

            for s in sesiones:
                s.activa = False
                session.add(s)

            session.commit()

# ==============================================================================
# CONTROL DE ACCESO
# ==============================================================================

class ControlAcceso:
    """Verificación de permisos y rate limiting"""

    @staticmethod
    def verificar_modulo_habilitado(modulo: ModuloSistema) -> Tuple[bool, str]:
        """
        Verifica si el usuario puede acceder a un módulo.

        Returns:
            Tuple[bool, str]: (permitido, mensaje_error)
        """
        # Verificar licencia válida
        valida, msg = ValidadorLicencia.licencia_valida()
        if not valida:
            return False, msg

        # Verificar módulo habilitado
        if not ValidadorLicencia.modulo_habilitado(modulo.value):
            return False, f"Módulo {modulo.value} no habilitado en su licencia"

        return True, ""

    @staticmethod
    def verificar_rate_limit_login(username: str, ip: str) -> Tuple[bool, int]:
        """
        Verifica rate limiting para intentos de login.

        Returns:
            Tuple[bool, int]: (bloqueado, intentos_restantes)
        """
        institucion_id = ContextoInstitucion.get_institucion_id()

        # Obtener política
        with Session(engine) as session:
            if institucion_id:
                politica = session.exec(
                    select(PoliticaSeguridad).where(
                        PoliticaSeguridad.institucion_id == institucion_id
                    )
                ).first()
            else:
                politica = None

            max_intentos = politica.max_intentos_login if politica else 5
            ventana_minutos = politica.tiempo_bloqueo_minutos if politica else 15

        # Verificar por username
        bloqueado_user, rest_user = ControlRateLimit.bloqueado(
            f"user:{username}",
            max_intentos,
            ventana_minutos
        )

        # Verificar por IP
        bloqueado_ip, rest_ip = ControlRateLimit.bloqueado(
            f"ip:{ip}",
            max_intentos * 2,  # Permitir más intentos por IP (diferentes usuarios)
            ventana_minutos
        )

        if bloqueado_user or bloqueado_ip:
            return True, 0

        return False, min(rest_user, rest_ip)

    @staticmethod
    def registrar_intento_login(
        username: str,
        ip: str,
        exitoso: bool,
        razon_fallo: Optional[str] = None
    ):
        """Registra un intento de login en BD y rate limiter"""
        institucion_id = ContextoInstitucion.get_institucion_id()

        # Registrar en rate limiter
        if not exitoso:
            ControlRateLimit.registrar_intento(f"user:{username}")
            ControlRateLimit.registrar_intento(f"ip:{ip}")
        else:
            # Limpiar intentos tras login exitoso
            ControlRateLimit.limpiar_intentos(f"user:{username}")
            ControlRateLimit.limpiar_intentos(f"ip:{ip}")

        # Registrar en BD
        with Session(engine) as session:
            intento = IntentoLogin(
                institucion_id=institucion_id,
                username=username,
                ip_address=ip,
                exitoso=exitoso,
                razon_fallo=razon_fallo
            )
            session.add(intento)
            session.commit()

    @staticmethod
    def verificar_cuenta_bloqueada(usuario_id: int) -> Tuple[bool, str]:
        """
        Verifica si la cuenta de usuario está bloqueada.

        Returns:
            Tuple[bool, str]: (bloqueada, razon)
        """
        with Session(engine) as session:
            usuario = session.get(Usuario, usuario_id)

            if not usuario:
                return True, "Usuario no encontrado"

            if not usuario.activo:
                return True, "Cuenta desactivada"

            if usuario.cuenta_bloqueada:
                return True, "Cuenta bloqueada por seguridad. Contacte al administrador."

            return False, ""

# ==============================================================================
# DECORADORES DE VERIFICACIÓN
# ==============================================================================

def requiere_licencia_activa(func):
    """Decorador para verificar licencia activa antes de ejecutar función"""
    def wrapper(*args, **kwargs):
        valida, msg = ValidadorLicencia.licencia_valida()
        if not valida:
            from nicegui import ui
            ui.notify(f"Licencia no válida: {msg}", type='negative', icon='error')
            return None
        return func(*args, **kwargs)
    return wrapper

def requiere_modulo(modulo: ModuloSistema):
    """Decorador para verificar que módulo esté habilitado"""
    def decorator(func):
        def wrapper(*args, **kwargs):
            permitido, msg = ControlAcceso.verificar_modulo_habilitado(modulo)
            if not permitido:
                from nicegui import ui
                ui.notify(msg, type='warning', icon='lock')
                return None
            return func(*args, **kwargs)
        return wrapper
    return decorator
