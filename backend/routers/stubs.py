"""Módulos declarados pero AÚN NO implementados.

Antes devolvían `200 []`. Eso es mentir con cara de estar funcionando: quien
abre "Vacunación" y ve una lista vacía no concluye "este módulo no existe",
concluye "este paciente no tiene vacunas registradas". En un sistema clínico esa
confusión es un dato falso, no una molestia de interfaz.

Ahora devuelven **501 Not Implemented**, que es lo que el docstring de este
archivo ya prometía y el código no cumplía.

Cuando un módulo se implemente de verdad: se crea su router en este mismo
paquete, se quita de la lista de abajo y se monta en `main.py` entre los reales.
NO deben coexistir un stub y un router real con el mismo nombre — llegó a haber
ocho (laboratorio, imagenes, quirofano, interconsultas, epicrisis, nutricion,
rehabilitacion, evoluciones), y bastaba reordenar los imports de `main.py` para
que los cascarones tumbaran módulos que sí funcionaban, sin que nada fallara.
"""
from fastapi import APIRouter, HTTPException, status

# Módulos del catálogo `ModuloSistema` que todavía no tienen implementación.
# Mantener ordenado y SIN duplicados con los routers reales de `main.py`.
PENDIENTES: dict[str, str] = {
    "consentimiento": "consentimiento",
    "remisiones": "remisiones",
    "banco-sangre": "banco_sangre",
    "nota-operatoria": "nota_operatoria",
    "preanestesia": "preanestesia",
    "recuperacion": "recuperacion",
    "contrarreferencia": "contrarreferencia",
    "trabajo-social": "trabajo_social",
    "telemedicina": "telemedicina",
    "vacunacion": "vacunacion",
    "prenatal": "prenatal",
    "crecimiento-desarrollo": "crecimiento_desarrollo",
    "salud-reproductiva": "salud_reproductiva",
    "certificado-defuncion": "certificado_defuncion",
    "registro-nacimiento": "registro_nacimiento",
    "turnos": "turnos",
    "comites": "comites",
    "equipos-medicos": "equipos_medicos",
    "indicadores": "indicadores",
    "infecciones": "infecciones",
    "mar": "mar",
    "kardex": "kardex",
    "censo": "censo_diario",
    "planes-cuidado": "plan_cuidado",
    "portal": "portal_paciente",
}


def make_stub_router(prefix: str, tag: str) -> APIRouter:
    """Router que responde 501 a cualquier método y cualquier subruta."""
    router = APIRouter(prefix=f"/{prefix}", tags=[tag])
    metodos = ["GET", "POST", "PUT", "PATCH", "DELETE"]

    def no_implementado():
        raise HTTPException(
            status_code=status.HTTP_501_NOT_IMPLEMENTED,
            detail=(
                f"El módulo '{tag}' está previsto pero aún no implementado. "
                "No confunda esta respuesta con ausencia de registros."
            ),
        )

    router.add_api_route("/", no_implementado, methods=metodos)
    router.add_api_route("/{ruta:path}", no_implementado, methods=metodos,
                         include_in_schema=False)
    return router


# Se exportan con el mismo nombre de siempre (`<tag>_router`) para no romper
# los imports existentes de main.py.
_generados = {
    f"{tag}_router": make_stub_router(prefix, tag)
    for prefix, tag in PENDIENTES.items()
}
globals().update(_generados)

ROUTERS_PENDIENTES = list(_generados.values())

__all__ = ["make_stub_router", "PENDIENTES", "ROUTERS_PENDIENTES", *_generados]
