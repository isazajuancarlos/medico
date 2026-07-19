"""Qué módulos ve esta institución.

Es la pieza que permite vender el MISMO binario a un consultorio, una IPS, una
clínica y un hospital: lo que cambia no es el código, es la licencia.

El frontend arma su menú desde aquí en vez de traer una lista fija. Sin esto,
un consultorio de consulta general veía cuarenta opciones de las que treinta y
cinco no le aplican, y un hospital no tenía forma de saber cuáles de las suyas
están realmente implementadas.

Se distinguen tres estados, que antes eran indistinguibles porque todo
respondía `200 []`:
  habilitado   licenciado y funcionando
  pendiente    licenciado pero aún sin implementar (el endpoint responde 501)
  no_incluido  existe en el producto pero no está en el plan de la institución
"""
from fastapi import APIRouter, Depends
from sqlmodel import Session

from core.deps import get_current_active_user, get_db, modulos_de
from models import ModuloSistema, Usuario
from routers.stubs import PENDIENTES

router = APIRouter(prefix="/modulos", tags=["modulos"])

# Nombre del módulo en el catálogo -> tag del stub, para saber cuáles están
# declarados pero todavía sin implementar.
_TAGS_PENDIENTES = set(PENDIENTES.values())

# El catálogo agrupa por área asistencial; el menú se construye con esto.
GRUPOS: dict[str, tuple[str, ...]] = {
    "Core": ("CITAS", "PACIENTES", "HISTORIA", "URGENCIAS", "PORTAL_PACIENTE"),
    "Hospitalario": ("HOSPITALIZACION", "EPICRISIS", "INTERCONSULTAS", "MAR",
                     "KARDEX", "CENSO_DIARIO", "PLAN_CUIDADO", "CONTRARREFERENCIA"),
    "Quirúrgico": ("QUIROFANO", "NOTA_OPERATORIA", "PREANESTESIA", "RECUPERACION"),
    "Diagnóstico": ("LABORATORIO", "IMAGENES"),
    "Soporte clínico": ("FARMACIA", "ENFERMERIA", "EVOLUCIONES", "CONSENTIMIENTO",
                        "BANCO_SANGRE"),
    "Especialidades": ("NUTRICION", "REHABILITACION", "TRABAJO_SOCIAL", "TELEMEDICINA"),
    "Promoción y prevención": ("VACUNACION", "PRENATAL", "CRECIMIENTO_DESARROLLO",
                              "SALUD_REPRODUCTIVA"),
    "Registros civiles": ("CERTIFICADO_DEFUNCION", "REGISTRO_NACIMIENTO",
                          "INCAPACIDADES", "REMISIONES"),
    "Administrativo": ("FACTURACION", "RIPS", "AUDITORIA", "ESTADISTICAS", "TURNOS",
                       "COMITES", "EQUIPOS_MEDICOS", "INDICADORES", "INFECCIONES"),
}


def _estado(nombre: str, habilitados: set[str]) -> str:
    if nombre not in habilitados:
        return "no_incluido"
    if nombre.lower() in _TAGS_PENDIENTES:
        return "pendiente"
    return "habilitado"


@router.get("/")
def listar_modulos(
    user: Usuario = Depends(get_current_active_user),
    db: Session = Depends(get_db),
):
    """Catálogo completo con el estado de cada módulo para esta institución."""
    habilitados = modulos_de(user, db)
    conocidos = {m.value for m in ModuloSistema}

    grupos = []
    for grupo, nombres in GRUPOS.items():
        items = [
            {"modulo": n, "estado": _estado(n, habilitados)}
            for n in nombres if n in conocidos
        ]
        grupos.append({"grupo": grupo, "modulos": items})

    return {
        "institucion_id": user.institucion_id,
        "habilitados": sorted(habilitados),
        "grupos": grupos,
    }
