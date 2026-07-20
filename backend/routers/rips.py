from datetime import date
from fastapi import APIRouter, Depends, HTTPException, Query
from fastapi.responses import JSONResponse
from models import Usuario, ModuloSistema
from core.deps import get_current_active_user, require_role, requiere_modulo
from models import RolUsuario

router = APIRouter(prefix="/rips", tags=["rips"],
    dependencies=[Depends(requiere_modulo(ModuloSistema.RIPS))],
)


@router.get("/")
def generar_rips(
    inicio: date = Query(...),
    fin: date = Query(...),
    current_user: Usuario = Depends(require_role(RolUsuario.ADMIN, RolUsuario.SUPERADMIN)),
):
    import os
    import sys
    sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))
    from rips_completo import InstitucionRequerida, exportar_rips_completo

    # La institución sale del usuario autenticado, NO de un parámetro: si
    # viniera por la petición, un administrador podría pedir el RIPS de otra.
    # Este llamado iba sin institución, y entonces el generador no filtraba
    # nada: el archivo salía con las atenciones de todas. Ver `rips_completo`.
    if not current_user.institucion_id:
        raise HTTPException(
            status_code=400,
            detail="El usuario no está asociado a ninguna institución; no se "
                   "puede determinar de quién sería el reporte.",
        )
    try:
        resultado = exportar_rips_completo(
            fecha_inicio=inicio, fecha_fin=fin,
            institucion_id=current_user.institucion_id,
        )
    except InstitucionRequerida as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        # El `except Exception` de antes se tragaba también la fuga: cualquier
        # fallo salía como 500 con el texto pegado, sin distinguir un dato que
        # falta de un error del servidor.
        raise HTTPException(status_code=500, detail=f"Error generando RIPS: {e}")
    return JSONResponse(content=resultado)
