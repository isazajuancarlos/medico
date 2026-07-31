"""Generación del RIPS.

Dos reglas que este endpoint aprendió a la mala:

1. **La institución sale del usuario autenticado**, nunca de un parámetro. Este
   llamado iba sin institución, y el generador entonces no filtraba nada: el
   archivo salía con las atenciones de TODAS las instituciones del sistema.

2. **Un RIPS incompleto no se genera solo.** Se puede forzar, pero hay que
   pedirlo, y forzarlo queda en auditoría con la lista de lo que se supuso.
   La salida existe porque lo que se migre del sistema viejo no va a traer los
   campos de facturación, y un cierre de mes bloqueado sin alternativa se
   resuelve editando la base a mano, que es peor que el supuesto declarado.
"""
import os
import sys
from collections import Counter
from datetime import date

from fastapi import APIRouter, Depends, HTTPException, Query, Request
from fastapi.responses import JSONResponse

from core.deps import require_role, requiere_modulo
from models import ModuloSistema, RolUsuario, Usuario

sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

router = APIRouter(prefix="/rips", tags=["rips"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.RIPS))],
)


@router.get("/")
def generar_rips(
    request: Request,
    inicio: date = Query(...),
    fin: date = Query(...),
    permitir_supuestos: bool = Query(
        False,
        description="Genera el archivo aunque falten datos obligatorios, "
                    "reportando el valor supuesto. Queda registrado en "
                    "auditoría con la lista de lo que se supuso.",
    ),
    current_user: Usuario = Depends(require_role(RolUsuario.ADMIN, RolUsuario.SUPERADMIN)),
):
    from auditoria import registrar_auditoria
    from rips_completo import (
        DatosIncompletos, InstitucionRequerida, Recolector,
        exportar_rips_completo,
    )

    if not current_user.institucion_id:
        raise HTTPException(
            status_code=400,
            detail="El usuario no está asociado a ninguna institución; no se "
                   "puede determinar de quién sería el reporte.",
        )

    # El recolector se crea AQUÍ para poder mirarlo después: sin eso, forzar la
    # generación sería exactamente lo de antes —datos supuestos que salen sin
    # que nadie se entere—, solo que con un parámetro de por medio.
    recolector = Recolector(permitir_supuestos)
    try:
        resultado = exportar_rips_completo(
            fecha_inicio=inicio, fecha_fin=fin,
            institucion_id=current_user.institucion_id,
            recolector=recolector,
        )
    except InstitucionRequerida as e:
        raise HTTPException(status_code=400, detail=str(e))
    except DatosIncompletos as e:
        # 422 y no 500: la petición es válida y el servidor está bien; lo que
        # falta son datos en las historias. Y se devuelve la lista, que es lo
        # único que permite corregirlos.
        raise HTTPException(status_code=422, detail={
            "mensaje": e.resumen(),
            "faltantes": [
                {"archivo": f.archivo, "registro": f.registro_id,
                 "campo": f.campo, "etiqueta": f.etiqueta}
                for f in e.faltantes
            ],
        })
    except Exception as e:
        # El `except Exception` que había se tragaba también la fuga: todo salía
        # como 500 con el texto pegado, sin distinguir un dato que falta de un
        # error del servidor.
        raise HTTPException(status_code=500, detail=f"Error generando RIPS: {e}")

    supuestos = _resumen_supuestos(recolector)
    if supuestos:
        registrar_auditoria(
            usuario_id=current_user.id,
            accion="GENERAR_RIPS_CON_SUPUESTOS",
            tabla_afectada="rips",
            registro_id=current_user.institucion_id,
            datos_despues={
                "periodo": f"{inicio}/{fin}",
                "total_supuestos": sum(supuestos.values()),
                "por_campo": supuestos,
            },
            ip_address=request.client.host if request.client else None,
            user_agent=request.headers.get("user-agent"),
        )

    return JSONResponse(content={
        "archivos": resultado,
        # Viaja con el archivo, no en un log aparte: quien lo firma tiene que
        # ver qué lleva supuesto ANTES de firmarlo.
        "supuestos": supuestos,
    })


def _resumen_supuestos(recolector) -> dict:
    """Cuántos valores se supusieron, por campo. Vacío si no se supuso ninguno."""
    return dict(Counter(f.etiqueta for f in recolector.faltantes))
