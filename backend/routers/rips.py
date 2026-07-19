from datetime import date
from fastapi import APIRouter, Depends, HTTPException, Query
from fastapi.responses import JSONResponse
from models import Usuario
from core.deps import get_current_active_user, require_role
from models import RolUsuario

router = APIRouter(prefix="/rips", tags=["rips"])


@router.get("/")
def generar_rips(
    inicio: date = Query(...),
    fin: date = Query(...),
    current_user: Usuario = Depends(require_role(RolUsuario.ADMIN, RolUsuario.SUPERADMIN)),
):
    try:
        import sys, os
        sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))
        from rips_completo import exportar_rips_completo
        resultado = exportar_rips_completo(fecha_inicio=inicio, fecha_fin=fin)
        return JSONResponse(content=resultado)
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Error generando RIPS: {str(e)}")
