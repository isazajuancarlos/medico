"""Aislamiento entre instituciones para el acceso POR IDENTIFICADOR.

El sistema es multi-institucional. `filtrar_por_institucion` protege las
consultas por lista añadiendo `WHERE institucion_id = …` a un `select()`, y
funciona. Pero **todo lo que se busca por identificador usa `db.get(Modelo, id)`,
que no pasa por ahí**: bastaba incrementar un número en la URL para leer —y con
`PUT`, modificar— la historia clínica de un paciente de otra institución.
Verificado por prueba, no por lectura: `tests/test_aislamiento_institucion.py`.

=== POR QUÉ NO SE PARCHEA RUTA POR RUTA ===

Hay 169 llamadas a `db.get` en los routers. Añadir una comprobación en cada una
garantiza que la número 170 se escriba sin ella. En vez de eso hay UNA función,
`obtener()`, y una prueba que falla si un router vuelve a usar `db.get` directo
sobre un modelo con dueño.

=== POR QUÉ 404 Y NO 403 ===

Un 403 confirma que el registro existe en otra institución. Con eso se enumera:
se prueban identificaciones hasta que una deja de dar 404, y ya se sabe que esa
persona es paciente de la clínica de al lado. Eso también es un dato de salud.
Desde fuera, un registro ajeno tiene que ser indistinguible de uno inexistente.

=== CÓMO SE AVERIGUA EL DUEÑO ===

Cuarenta y un modelos llevan `institucion_id`. El resto de la historia clínica
—`Historia`, `Epicrisis`, `EvolucionMedica`, `NotaEnfermeria`…— NO lo lleva:
cuelga del paciente, y el paciente sí lo tiene. Así que se sigue la cadena de
enlaces hasta encontrar a alguien que lo declare.

Lo que NO se hace es suponer. Un modelo sin `institucion_id`, sin enlace conocido
y que no esté declarado como catálogo compartido levanta `RuntimeError`. Es
deliberado: si mañana alguien añade un modelo clínico nuevo y olvida decir de
quién es, tiene que romperse aquí —en una prueba— y no en producción sirviendo
datos ajenos a quien los pida.
"""
from __future__ import annotations

from typing import Any, Optional

from fastapi import HTTPException, status

from models import RolUsuario, Usuario

#: Campo que lleva al siguiente eslabón, en orden de preferencia.
#: `Historia` → `paciente_id` → `Paciente.institucion_id`.
ENLACES: tuple[tuple[str, str], ...] = (
    ("paciente_id", "Paciente"),
    ("historia_id", "Historia"),
    ("hospitalizacion_id", "Hospitalizacion"),
    ("factura_id", "Factura"),
    ("cita_id", "Cita"),
    ("contrato_id", "Contrato"),
)

#: Catálogos y tablas de sistema que NO pertenecen a ninguna institución y son
#: iguales para todas. Van por nombre y a mano: la lista es la declaración
#: explícita de "esto sí puede verlo cualquiera", y obliga a pensarlo una vez
#: por modelo en vez de dejar que el silencio decida.
COMPARTIDOS: frozenset[str] = frozenset({
    # Catálogos de códigos
    "Cie10", "Cups", "Medicamento", "Especialidad", "Entidad",
    # Configuración e inventario que hoy no está segmentado
    # `Contrato` SALIÓ de aquí el 2026-08-05: ya tiene `institucion_id`. Estaba
    # junto a los catálogos de códigos como si fuera nacional, y no lo es —es el
    # acuerdo entre UN prestador y UNA entidad, con SUS tarifas—. Además era la
    # causa de que `institucion_de(Factura)` respondiera «de todos»: la cadena
    # de enlaces de la factura terminaba justo aquí.
    #
    # `Entidad` se QUEDA, y las dos decisiones son opuestas a propósito: una EPS
    # es la misma persona jurídica para todos los prestadores del país.
    #
    # `TarifaContrato` sigue aquí pero ya no por el mismo motivo: cuelga de
    # `contrato_id`, y `Contrato` ya sabe de quién es. Queda como tarea sacarla
    # también; se deja ahora para no mezclar dos cambios de alcance en uno.
    "ConfigCampo", "BloqueHorario", "Cama", "ItemInventario",
    "TarifaContrato",
    # Tablas de sistema: su propio control de acceso es otro
    "Institucion", "AuditoriaSuperAdmin", "HistorialPassword", "Token2FA",
})

#: Tope de saltos al seguir la cadena. Protege de un ciclo entre modelos.
MAX_SALTOS = 4


class _SinDuenio:
    """«No se puede saber de quién es», que NO es «es de todos».

    Existe porque las dos cosas viajaban como `None` y `puede_ver` leía ese
    `None` como «catálogo compartido → lo ve cualquiera». Con eso, **todo
    registro con `institucion_id` en NULL quedaba visible desde cualquier
    institución** — y el `SUPERADMIN` tiene `institucion_id = NULL` por diseño,
    así que un ADMIN podía leerlo, reescribirle el correo, desbloquearlo y
    **cambiarle la contraseña** (`POST /api/admin/usuarios/{id}/reset-password`).
    De ahí a entrar como dueño del sistema hay un login.

    El alcance era mayor que ese caso: `models.py` declara `institucion_id`
    opcional en 34 modelos, `Paciente` incluido. Una historia importada sin
    institución quedaba legible desde cualquier otra.

    La cabecera de este módulo ya decía la regla —«lo que NO se hace es
    suponer»— y `COMPARTIDOS` ya era la declaración explícita de lo público. Lo
    que faltaba era que el huérfano no pudiera confundirse con ella, y eso lo
    arregla un tipo, no un `if` en cada llamador.

    Lo cazó una revisión de seguridad independiente el 2026-07-31.
    """

    __slots__ = ()

    def __repr__(self) -> str:  # pragma: no cover - solo para depurar
        return "SIN_DUENIO"


#: Marcador de dueño indeterminable. Ante él se NIEGA.
SIN_DUENIO = _SinDuenio()


def institucion_de(db, objeto: Any, _saltos: int = 0) -> Optional[int] | _SinDuenio:
    """A qué institución pertenece un registro, siguiendo la cadena de enlaces.

    Tres respuestas, no dos:

    - un `int` — el dueño.
    - `None` — **de todos**, y solo si el modelo está declarado en
      `COMPARTIDOS`.
    - [`SIN_DUENIO`] — no se puede saber: la columna está en NULL, o el enlace
      apunta a un padre que ya no existe. Ante esto se niega.

    Levanta `RuntimeError` si el modelo ni siquiera declara cómo averiguarlo: es
    preferible una caída ruidosa a entregar el dato por defecto.
    """
    nombre = type(objeto).__name__

    directa = getattr(objeto, "institucion_id", None)
    if directa is not None:
        return directa

    # COMPARTIDOS se consulta ANTES que el NULL de la columna, y el orden no es
    # un detalle: `ItemInventario`, `Cama`, `Contrato` y `TarifaContrato` están
    # declarados compartidos Y llevan la columna. Con el orden inverso, sus
    # filas sin institución —que es como están hoy, porque el módulo no está
    # segmentado— caerían en la rama del huérfano y desaparecerían para todo el
    # mundo. La declaración explícita manda sobre el valor ausente.
    if nombre in COMPARTIDOS:
        return None

    if "institucion_id" in getattr(type(objeto), "model_fields", {}):
        # Lo declara pero está vacío: es un registro huérfano, no uno público.
        return SIN_DUENIO

    if _saltos >= MAX_SALTOS:
        raise RuntimeError(
            f"{nombre}: la cadena de enlaces no llega a una institución en "
            f"{MAX_SALTOS} saltos; ¿hay un ciclo?"
        )

    import models as _m

    for campo, destino in ENLACES:
        valor = getattr(objeto, campo, None)
        if valor is None:
            continue
        padre = db.get(getattr(_m, destino), valor)
        if padre is None:
            # El enlace apunta a algo que ya no existe: sin dueño demostrable.
            # NO es «de todos» — antes lo era, y con eso bastaba borrar al padre
            # para volver público al hijo.
            return SIN_DUENIO
        return institucion_de(db, padre, _saltos + 1)

    raise RuntimeError(
        f"{nombre} no tiene `institucion_id`, no enlaza con ningún modelo que lo "
        f"tenga, y no está declarado en COMPARTIDOS. Decida a quién pertenece "
        f"antes de exponerlo por la API: añádale `institucion_id`, enlácelo con "
        f"su paciente, o inclúyalo en COMPARTIDOS si de verdad es de todos."
    )


def puede_ver(db, objeto: Any, user: Usuario) -> bool:
    """Si este usuario tiene derecho a ver este registro.

    El SUPERADMIN es el dueño del sistema, no un cliente: ve todo.
    """
    if user.rol == RolUsuario.SUPERADMIN:
        return True
    duenno = institucion_de(db, objeto)
    if duenno is SIN_DUENIO:
        # No se sabe de quién es. Se NIEGA, que es lo que la cabecera de este
        # módulo promete y lo que antes no ocurría: este caso caía en la rama de
        # abajo y se concedía a todo el mundo.
        return False
    if duenno is None:
        return True  # catálogo compartido, DECLARADO en COMPARTIDOS
    return duenno == user.institucion_id


def obtener(db, modelo: type, id_: Any, user: Usuario, *, que: str = "registro") -> Any:
    """`db.get` con aislamiento. Es la única forma de traer algo por id.

    Un registro de otra institución responde 404, igual que uno inexistente.
    """
    objeto = db.get(modelo, id_)
    if objeto is None or not puede_ver(db, objeto, user):
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"{que.capitalize()} no encontrado",
        )
    return objeto
