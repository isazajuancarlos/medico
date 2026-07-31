from typing import Generator, Optional, Callable
from fastapi import Depends, HTTPException, status, Cookie
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials
from sqlmodel import Session, select
from database import engine
from models import Usuario, RolUsuario
from core.security import decode_token

security = HTTPBearer(auto_error=False)


def get_db() -> Generator:
    with Session(engine) as session:
        yield session


def get_current_user(
    credentials: Optional[HTTPAuthorizationCredentials] = Depends(security),
    access_token: Optional[str] = Cookie(default=None),
    db: Session = Depends(get_db),
) -> Usuario:
    token = None
    if credentials:
        token = credentials.credentials
    elif access_token:
        token = access_token

    if not token:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="No autenticado",
            headers={"WWW-Authenticate": "Bearer"},
        )

    payload = decode_token(token)
    if payload.get("type") != "access":
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Token inválido")

    user_id = int(payload.get("sub", 0))
    user = db.get(Usuario, user_id)
    if not user:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Usuario no encontrado")
    return user


def get_current_active_user(user: Usuario = Depends(get_current_user)) -> Usuario:
    if not user.activo:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Cuenta inactiva")
    if user.cuenta_bloqueada:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Cuenta bloqueada")
    return user


def require_role(*roles: RolUsuario) -> Callable:
    def _checker(user: Usuario = Depends(get_current_active_user)) -> Usuario:
        if user.rol not in roles:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"Rol requerido: {', '.join(r.value for r in roles)}",
            )
        return user
    return _checker


def get_institucion_id(user: Usuario = Depends(get_current_active_user)) -> Optional[int]:
    return user.institucion_id


def filtrar_por_institucion(query, model, user: Usuario):
    """Filtra por institución si el usuario no es SUPERADMIN"""
    if user.rol != RolUsuario.SUPERADMIN and hasattr(model, "institucion_id"):
        query = query.where(model.institucion_id == user.institucion_id)
    return query


# =====================================================================
# MÓDULOS POR INSTITUCIÓN
# =====================================================================
# El mismo binario sirve a un consultorio, una IPS, una clínica y un hospital:
# lo que cambia es QUÉ MÓDULOS tiene licenciados cada institución
# (`Licencia.modulos_habilitados`, catálogo en `ModuloSistema`).
#
# Existía el modelo de datos pero no había forma de exigirlo: el único guardián
# era un decorador `requiere_modulo` heredado de la versión NiceGUI de la app,
# que importaba `nicegui`, llamaba a `ui.notify` y devolvía None. En un backend
# FastAPI no podía funcionar, y ningún router lo usaba.
#
# Distinguir estos tres estados importa, y hasta ahora los tres respondían
# `200 []`, indistinguibles para quien mira la pantalla:
#   403  el módulo existe pero la institución NO lo tiene licenciado
#   501  el módulo está licenciado pero AÚN NO está implementado
#   200 []  el módulo funciona y no hay registros
# En un hospital, que "Banco de Sangre" diga "vacío" en vez de "no habilitado"
# no es una molestia de interfaz: es un dato clínico falso.

def modulos_de(user: "Usuario", db: Session) -> set[str]:
    """Módulos habilitados para la institución del usuario, en MAYÚSCULAS.

    El SUPERADMIN no pertenece a ninguna institución y ve todo: es el dueño
    del sistema multi-institucional, no un cliente.
    """
    from models import EstadoLicencia, Licencia, ModuloSistema

    if user.rol == RolUsuario.SUPERADMIN:
        return {m.value for m in ModuloSistema}
    if not user.institucion_id:
        return set()

    licencia = db.exec(
        select(Licencia)
        .where(Licencia.institucion_id == user.institucion_id)
        .where(Licencia.estado == EstadoLicencia.ACTIVA)
    ).first()
    if not licencia:
        return set()
    return {str(m).upper() for m in (licencia.modulos_habilitados or [])}


def requiere_modulo(modulo) -> Callable:
    """Dependencia de FastAPI: 403 si la institución no licenció el módulo.

    Se usa a nivel de router, no de endpoint, para que no se olvide en uno:

        router = APIRouter(
            prefix="/quirofano",
            dependencies=[Depends(requiere_modulo(ModuloSistema.QUIROFANO))],
        )
    """
    nombre = getattr(modulo, "value", str(modulo)).upper()

    def _verificar(
        user: Usuario = Depends(get_current_active_user),
        db: Session = Depends(get_db),
    ) -> None:
        if nombre not in modulos_de(user, db):
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"El módulo {nombre} no está habilitado para su institución.",
            )

    return _verificar


# =====================================================================
# LICENCIA: vigencia y cupos
# =====================================================================
#
# Estos tres controles existían en la versión NiceGUI (`middleware.py`,
# `ValidadorLicencia`) y se perdieron al pasar a FastAPI. Nadie los aplicaba:
# `Licencia.fecha_vencimiento` no se leía en ninguna parte del backend, y
# `max_usuarios`/`max_medicos` solo aparecían en el modelo y en la semilla.
# En la práctica, una licencia vencía y el sistema seguía funcionando para
# siempre — que es lo mismo que no tener licencia.
#
# DOS DECISIONES DE DISEÑO QUE NO SON DETALLES:
#
# 1. LOS CUPOS SE CUENTAN, NO SE LEEN DE UN CONTADOR. El modelo trae
#    `uso_actual_usuarios`, `uso_actual_medicos` y `uso_actual_pacientes`, y
#    **ningún sitio del sistema los actualiza**: valen 0 desde que se sembró la
#    licencia. El control heredado comparaba contra ellos, así que nunca podía
#    saltar. Aquí se cuentan las filas reales; un valor derivado que nadie
#    deriva no es un dato, es un adorno que aparenta control.
#
# 2. UNA LICENCIA VENCIDA NO CIEGA LA HISTORIA CLÍNICA. Bloquear la lectura
#    dejaría a un hospital sin sus registros por un asunto comercial, y con
#    ello incumpliendo ante un tercero —el paciente, la EPS, la Supersalud—.
#    Lo que se bloquea es la EXPANSIÓN: crear usuarios, dar de alta médicos.
#    Vencida se lee y se sigue operando; no se crece. Ver la tarea #71.

def licencia_de(user: "Usuario", db: Session):
    """La licencia de la institución del usuario, en el estado que esté.

    A diferencia de `modulos_de`, NO filtra por estado: quien pregunta por la
    vigencia necesita ver la licencia vencida para poder decirlo. Filtrarla
    aquí devolvería «no hay licencia», que es un diagnóstico distinto.
    """
    from models import Licencia

    if not user.institucion_id:
        return None
    return db.exec(
        select(Licencia).where(Licencia.institucion_id == user.institucion_id)
    ).first()


def licencia_vigente(user: "Usuario", db: Session) -> tuple[bool, str]:
    """(vigente, motivo). El motivo va vacío si lo está.

    El SUPERADMIN no pertenece a ninguna institución: es el dueño del sistema,
    no un cliente, y no se le puede vencer una licencia que no tiene.
    """
    from datetime import date

    from models import EstadoLicencia

    if user.rol == RolUsuario.SUPERADMIN:
        return True, ""

    licencia = licencia_de(user, db)
    if licencia is None:
        return False, "La institución no tiene licencia registrada."
    if licencia.estado != EstadoLicencia.ACTIVA:
        return False, f"La licencia está en estado {licencia.estado.value}."
    hoy = date.today()
    if licencia.fecha_vencimiento < hoy:
        return False, f"La licencia venció el {licencia.fecha_vencimiento}."
    if licencia.fecha_inicio > hoy:
        return False, f"La licencia empieza a regir el {licencia.fecha_inicio}."
    return True, ""


def _en_uso(db: Session, institucion_id: int, solo_medicos: bool) -> int:
    """Cuenta usuarios ACTIVOS de la institución. Ver decisión (1) arriba.

    Los inactivos no consumen: una clínica de diez puestos que en tres años ha
    tenido quince empleados, desactivando a cada uno que se va, chocaba contra
    el tope con solo diez trabajando. Se venden asientos ocupados, no personas
    que pasaron por allí.

    De aquí sale la obligación de comprobar la REACTIVACIÓN: si desactivar
    libera un asiento, volver a activar lo ocupa, y sin ese control bastaría
    desactivar, crear y reactivar para pasarse del tope.
    """
    from models import Usuario as U

    consulta = select(U).where(U.institucion_id == institucion_id,
                               U.activo == True)  # noqa: E712 — SQL, no Python
    if solo_medicos:
        consulta = consulta.where(U.rol == RolUsuario.MEDICO)
    return len(db.exec(consulta).all())


def hay_cupo(user: "Usuario", db: Session, *, para_medico: bool) -> tuple[bool, str]:
    """¿Cabe un usuario más en la licencia? (cabe, motivo).

    Sin licencia no se afirma que hay cupo: se dice que no se puede comprobar,
    y quien llama decide. Devolver `True` ante la duda convertiría un control
    en un adorno.
    """
    if user.rol == RolUsuario.SUPERADMIN:
        return True, ""

    licencia = licencia_de(user, db)
    if licencia is None:
        return False, "La institución no tiene licencia registrada."

    tope = licencia.max_medicos if para_medico else licencia.max_usuarios
    usados = _en_uso(db, user.institucion_id, solo_medicos=para_medico)
    if usados >= tope:
        que = "médicos" if para_medico else "usuarios"
        return False, (
            f"La licencia permite {tope} {que} y ya hay {usados}. "
            "Amplíe la licencia para agregar más."
        )
    return True, ""


def exigir_cupo_al_modificar(usuario, cambios: dict, user: "Usuario",
                             db: Session) -> None:
    """El cupo se consume también SIN crear a nadie.

    `_en_uso` cuenta usuarios ACTIVOS y los médicos POR ROL, así que hay dos
    formas de ocupar un asiento sin pasar por el alta:

        activo: False -> True     reactivar a quien se había ido
        rol:    otro  -> MEDICO   ascender a quien ya está

    `hay_cupo` solo se comprobaba al crear, de modo que una institución con el
    tope lleno podía crecer indefinidamente por cualquiera de las dos.

    Se comprueba al ENTRAR al conjunto contado, nunca al salir: desactivar o
    dejar de ser médico liberan asiento y estorbarlo sería cerrar por cerrar.
    Guardar sin tocar ninguno de los dos campos tampoco es crecer.
    """
    from models import RolUsuario as R

    sera_activo = cambios.get("activo", usuario.activo)
    sera_medico = cambios.get("rol", usuario.rol) == R.MEDICO
    era_activo = usuario.activo
    era_medico = usuario.rol == R.MEDICO

    def negar(motivo):
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail=motivo)

    if sera_activo and not era_activo:
        cabe, motivo = hay_cupo(user, db, para_medico=False)
        if not cabe:
            negar(motivo)
    if (sera_activo and sera_medico) and not (era_activo and era_medico):
        cabe, motivo = hay_cupo(user, db, para_medico=True)
        if not cabe:
            negar(motivo)


def exigir_ascenso_permitido(cambios: dict, user: "Usuario") -> None:
    """**Solo un superadmin puede fabricar otro superadmin.**

    Va en LOS CUATRO sitios que escriben `Usuario.rol` por HTTP, y la lista se
    mantiene aquí porque el fallo de la primera versión fue justamente cubrir la
    mitad:

        PUT  /api/usuarios/{id}        `usuarios.py`  setattr
        PUT  /api/admin/usuarios/{id}  `admin.py`     setattr
        POST /api/usuarios/            `usuarios.py`  Usuario(**payload)
        POST /api/admin/usuarios       `admin.py`     Usuario(**payload)

    Los dos primeros se cerraron y los dos ALTAS quedaron abiertas — que son el
    camino más fácil, porque quien crea la cuenta elige además su contraseña y no
    necesita suplantar a nadie. Las pruebas salieron verdes porque recorrían solo
    los `PUT`: su alcance era el de la mitigación y no el de la amenaza. Lo cazó
    una revisión de seguridad independiente el 2026-07-31. Si mañana aparece un
    quinto sitio que escriba `rol`, va en esta lista y con su prueba.

    Sin esta comprobación, un ADMIN pone `rol=SUPERADMIN` en una cuenta de su
    institución (la suya incluida) y con eso sale del aislamiento: `puede_ver()`
    no acota al superadmin, así que el ascenso es una escalada de privilegios que
    CRUZA instituciones. El límite de institución de `admin.py` no lo frena,
    porque el ascenso ocurre dentro de la institución y el efecto ocurre fuera.

    Se comprueba el DESTINO y no el origen: degradar a un superadmin no crea
    autoridad nueva, y prohibirlo dejaría al dueño del sistema sin poder retirar
    la suya. Y no se prohíbe cambiar de rol en general —un admin sube a alguien
    de auxiliar a enfermería a diario—: eso sería cerrar lo inusual en vez de lo
    imposible.

    Es la misma regla que `crear_usuario` ya aplica en el binario Rust
    (`crates/medico-api/src/rutas.rs`), dicha en el otro lado del port para que
    las dos implementaciones no difieran justo aquí.
    """
    from models import RolUsuario as R

    if cambios.get("rol") == R.SUPERADMIN and user.rol != R.SUPERADMIN:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Solo un superadministrador puede conceder el rol de superadministrador.",
        )


def requiere_licencia_vigente(
    user: "Usuario" = Depends(get_current_active_user),
    db: Session = Depends(get_db),
) -> None:
    """Dependencia para las acciones que NO son de lectura clínica.

    Se pone donde el sistema CRECE —altas de usuarios, de médicos, de sedes—,
    nunca sobre consultar o registrar atención: ver la decisión (2) de arriba.
    """
    vigente, motivo = licencia_vigente(user, db)
    if not vigente:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail=motivo)
