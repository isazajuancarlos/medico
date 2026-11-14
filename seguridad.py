"""
Módulo de utilidades de seguridad para MED-SYS Pro
Incluye:
- Encriptación AES-256 para datos sensibles
- Generación y validación de tokens 2FA
- Validación de políticas de contraseña
- Generación y validación de claves de licencia
- Gestión de sesiones seguras
"""

import secrets
import string
import hashlib
import re
from datetime import datetime, timedelta
from typing import Tuple, Optional, Dict
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.backends import default_backend
from cryptography.hazmat.primitives import padding
import base64
import os

# ==============================================================================
# ENCRIPTACIÓN AES-256
# ==============================================================================

class EncriptadorAES:
    """
    Encriptación AES-256-CBC para datos sensibles.
    Usa una clave maestra derivada de SECRET_KEY del entorno.
    """

    @staticmethod
    def _get_master_key() -> bytes:
        """Obtiene la clave maestra de 32 bytes desde env"""
        secret = os.getenv("ENCRYPTION_KEY", os.getenv("SECRET_KEY", "default_insecure_key_change_in_production"))
        # Derivar 32 bytes usando SHA-256
        return hashlib.sha256(secret.encode()).digest()

    @staticmethod
    def encriptar(texto_plano: str) -> Tuple[str, str]:
        """
        Encripta texto plano usando AES-256-CBC.

        Returns:
            Tuple[str, str]: (texto_encriptado_base64, iv_base64)
        """
        if not texto_plano:
            return "", ""

        # Generar IV aleatorio
        iv = os.urandom(16)

        # Obtener clave maestra
        key = EncriptadorAES._get_master_key()

        # Padding PKCS7
        padder = padding.PKCS7(128).padder()
        padded_data = padder.update(texto_plano.encode()) + padder.finalize()

        # Encriptar
        cipher = Cipher(algorithms.AES(key), modes.CBC(iv), backend=default_backend())
        encryptor = cipher.encryptor()
        encrypted = encryptor.update(padded_data) + encryptor.finalize()

        # Retornar en base64
        return (
            base64.b64encode(encrypted).decode('utf-8'),
            base64.b64encode(iv).decode('utf-8')
        )

    @staticmethod
    def desencriptar(texto_encriptado_b64: str, iv_b64: str) -> str:
        """
        Desencripta texto usando AES-256-CBC.

        Args:
            texto_encriptado_b64: Texto encriptado en base64
            iv_b64: IV en base64

        Returns:
            str: Texto plano desencriptado
        """
        if not texto_encriptado_b64 or not iv_b64:
            return ""

        try:
            # Decodificar base64
            encrypted = base64.b64decode(texto_encriptado_b64)
            iv = base64.b64decode(iv_b64)

            # Obtener clave maestra
            key = EncriptadorAES._get_master_key()

            # Desencriptar
            cipher = Cipher(algorithms.AES(key), modes.CBC(iv), backend=default_backend())
            decryptor = cipher.decryptor()
            decrypted_padded = decryptor.update(encrypted) + decryptor.finalize()

            # Remover padding
            unpadder = padding.PKCS7(128).unpadder()
            decrypted = unpadder.update(decrypted_padded) + unpadder.finalize()

            return decrypted.decode('utf-8')
        except Exception as e:
            print(f"Error al desencriptar: {e}")
            return ""

# ==============================================================================
# TOKENS 2FA
# ==============================================================================

class Generador2FA:
    """Generación y validación de tokens para autenticación de dos factores"""

    @staticmethod
    def generar_token_numerico(longitud: int = 6) -> str:
        """Genera token numérico de N dígitos"""
        return ''.join(secrets.choice(string.digits) for _ in range(longitud))

    @staticmethod
    def generar_token_alfanumerico(longitud: int = 8) -> str:
        """Genera token alfanumérico seguro"""
        alfabeto = string.ascii_uppercase + string.digits
        # Evitar caracteres confusos: 0, O, 1, I, l
        alfabeto = alfabeto.replace('0', '').replace('O', '').replace('1', '').replace('I', '')
        return ''.join(secrets.choice(alfabeto) for _ in range(longitud))

    @staticmethod
    def calcular_expiracion(minutos: int = 10) -> datetime:
        """Calcula fecha de expiración para el token"""
        return datetime.now() + timedelta(minutes=minutos)

    @staticmethod
    def token_valido(fecha_expiracion: datetime) -> bool:
        """Verifica si un token no ha expirado"""
        return datetime.now() < fecha_expiracion

# ==============================================================================
# VALIDACIÓN DE CONTRASEÑAS
# ==============================================================================

class ValidadorPassword:
    """Validación de contraseñas según políticas de seguridad"""

    @staticmethod
    def validar_politica(
        password: str,
        longitud_minima: int = 8,
        requiere_mayuscula: bool = True,
        requiere_minuscula: bool = True,
        requiere_numero: bool = True,
        requiere_especial: bool = True
    ) -> Tuple[bool, str]:
        """
        Valida si una contraseña cumple con la política de seguridad.

        Returns:
            Tuple[bool, str]: (es_valida, mensaje_error)
        """
        if len(password) < longitud_minima:
            return False, f"La contraseña debe tener al menos {longitud_minima} caracteres"

        if requiere_mayuscula and not re.search(r'[A-Z]', password):
            return False, "La contraseña debe contener al menos una letra mayúscula"

        if requiere_minuscula and not re.search(r'[a-z]', password):
            return False, "La contraseña debe contener al menos una letra minúscula"

        if requiere_numero and not re.search(r'\d', password):
            return False, "La contraseña debe contener al menos un número"

        if requiere_especial and not re.search(r'[!@#$%^&*(),.?":{}|<>_\-+=\[\]\\\/;~`]', password):
            return False, "La contraseña debe contener al menos un carácter especial (!@#$%^&*...)"

        # Verificar patrones comunes débiles
        if password.lower() in [
            'password', 'contraseña', '12345678', 'qwerty', 'abc123',
            'password123', 'admin123', 'welcome', 'letmein'
        ]:
            return False, "La contraseña es demasiado común"

        return True, ""

    @staticmethod
    def generar_password_seguro(longitud: int = 12) -> str:
        """Genera una contraseña segura aleatoria"""
        mayusculas = string.ascii_uppercase
        minusculas = string.ascii_lowercase
        numeros = string.digits
        especiales = "!@#$%^&*()-_=+[]{}|;:,.<>?"

        # Asegurar al menos un carácter de cada tipo
        password = [
            secrets.choice(mayusculas),
            secrets.choice(minusculas),
            secrets.choice(numeros),
            secrets.choice(especiales)
        ]

        # Completar longitud con caracteres aleatorios
        todos = mayusculas + minusculas + numeros + especiales
        password += [secrets.choice(todos) for _ in range(longitud - 4)]

        # Mezclar
        secrets.SystemRandom().shuffle(password)

        return ''.join(password)

# ==============================================================================
# LICENCIAS
# ==============================================================================

class GestorLicencias:
    """Generación y validación de claves de licencia"""

    @staticmethod
    def generar_clave_licencia(
        nit_institucion: str,
        tipo_licencia: str,
        fecha_vencimiento: str
    ) -> Tuple[str, str]:
        """
        Genera clave de licencia y hash de validación.

        Args:
            nit_institucion: NIT de la institución
            tipo_licencia: TRIAL, BASICA, PROFESIONAL, ENTERPRISE
            fecha_vencimiento: YYYY-MM-DD

        Returns:
            Tuple[str, str]: (clave_licencia, hash_validacion)
        """
        # Componentes de la clave
        timestamp = datetime.now().strftime('%Y%m%d%H%M%S')
        random_part = secrets.token_hex(8).upper()

        # Crear firma
        data = f"{nit_institucion}:{tipo_licencia}:{fecha_vencimiento}:{timestamp}"
        signature = hashlib.sha256(data.encode()).hexdigest()[:8].upper()

        # Formato: TIPO-NIT-RAND-SIG
        tipo_code = tipo_licencia[:3].upper()
        nit_code = nit_institucion[:6] if len(nit_institucion) >= 6 else nit_institucion.zfill(6)

        clave = f"{tipo_code}-{nit_code}-{random_part}-{signature}"

        # Hash de validación (SHA-256 completo)
        hash_validacion = hashlib.sha256(f"{clave}:{data}".encode()).hexdigest()

        return clave, hash_validacion

    @staticmethod
    def validar_hash_licencia(clave: str, hash_almacenado: str) -> bool:
        """
        Verifica que el hash de una licencia sea auténtico.
        Nota: Esta es una verificación básica. En producción se usaría firma digital RSA.
        """
        # En una implementación real, aquí se verificaría contra un servidor central
        # Por ahora, solo verificamos que el hash no esté vacío
        return bool(clave) and bool(hash_almacenado) and len(hash_almacenado) == 64

# ==============================================================================
# SESIONES
# ==============================================================================

class GestorSesiones:
    """Gestión de sesiones seguras"""

    @staticmethod
    def generar_session_token() -> str:
        """Genera token de sesión seguro (256 bits)"""
        return secrets.token_urlsafe(32)

    @staticmethod
    def calcular_expiracion_sesion(timeout_minutos: int = 30) -> datetime:
        """Calcula fecha de expiración de sesión"""
        return datetime.now() + timedelta(minutes=timeout_minutos)

    @staticmethod
    def sesion_valida(
        fecha_ultima_actividad: datetime,
        fecha_expiracion: datetime,
        timeout_minutos: int = 30
    ) -> bool:
        """
        Verifica si una sesión es válida.

        Criteria:
        1. No ha expirado según fecha_expiracion
        2. No ha excedido el timeout desde última actividad
        """
        ahora = datetime.now()

        # Verificar expiración absoluta
        if ahora >= fecha_expiracion:
            return False

        # Verificar timeout por inactividad
        tiempo_inactivo = (ahora - fecha_ultima_actividad).total_seconds() / 60
        if tiempo_inactivo > timeout_minutos:
            return False

        return True

# ==============================================================================
# RATE LIMITING
# ==============================================================================

class ControlRateLimit:
    """Control de rate limiting simple basado en memoria"""

    # Almacenamiento en memoria (en producción usar Redis)
    _intentos: Dict[str, list] = {}

    @classmethod
    def registrar_intento(cls, identificador: str) -> None:
        """Registra un intento de login u operación"""
        ahora = datetime.now()

        if identificador not in cls._intentos:
            cls._intentos[identificador] = []

        cls._intentos[identificador].append(ahora)

    @classmethod
    def contar_intentos_recientes(
        cls,
        identificador: str,
        ventana_minutos: int = 15
    ) -> int:
        """
        Cuenta intentos en una ventana de tiempo.

        Args:
            identificador: username, IP, etc.
            ventana_minutos: Ventana de tiempo a considerar

        Returns:
            int: Número de intentos en la ventana
        """
        if identificador not in cls._intentos:
            return 0

        limite_tiempo = datetime.now() - timedelta(minutes=ventana_minutos)

        # Filtrar solo intentos recientes
        intentos_recientes = [
            t for t in cls._intentos[identificador]
            if t >= limite_tiempo
        ]

        # Actualizar lista (limpiar antiguos)
        cls._intentos[identificador] = intentos_recientes

        return len(intentos_recientes)

    @classmethod
    def bloqueado(
        cls,
        identificador: str,
        max_intentos: int = 5,
        ventana_minutos: int = 15
    ) -> Tuple[bool, int]:
        """
        Verifica si un identificador está bloqueado.

        Returns:
            Tuple[bool, int]: (bloqueado, intentos_restantes)
        """
        intentos = cls.contar_intentos_recientes(identificador, ventana_minutos)

        if intentos >= max_intentos:
            return True, 0

        return False, max_intentos - intentos

    @classmethod
    def limpiar_intentos(cls, identificador: str) -> None:
        """Limpia intentos de un identificador (tras login exitoso)"""
        if identificador in cls._intentos:
            del cls._intentos[identificador]

# ==============================================================================
# UTILIDADES DE HASHING
# ==============================================================================

def hash_determinista(texto: str, salt: str = "") -> str:
    """
    Crea hash SHA-256 determinista de un texto.
    Útil para comparaciones de unicidad.
    """
    data = f"{texto}:{salt}"
    return hashlib.sha256(data.encode()).hexdigest()

def verificar_integridad(contenido: str, hash_esperado: str) -> bool:
    """Verifica que el contenido no haya sido alterado"""
    hash_actual = hashlib.sha256(contenido.encode()).hexdigest()
    return hash_actual == hash_esperado

# ==============================================================================
# GENERADORES DE CÓDIGOS
# ==============================================================================

def generar_codigo_activacion(longitud: int = 16) -> str:
    """Genera código de activación legible en formato XXXX-XXXX-XXXX-XXXX"""
    alfabeto = string.ascii_uppercase + string.digits
    alfabeto = alfabeto.replace('0', '').replace('O', '').replace('1', '').replace('I', '')

    partes = []
    for _ in range(4):
        parte = ''.join(secrets.choice(alfabeto) for _ in range(4))
        partes.append(parte)

    return '-'.join(partes)

def generar_api_key() -> str:
    """Genera API key segura para integraciones"""
    return f"medsys_{secrets.token_urlsafe(32)}"
