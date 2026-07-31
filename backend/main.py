import asyncio
import logging
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from starlette.exceptions import HTTPException as StarletteHTTPException

from database import init_db

logger = logging.getLogger(__name__)

UPLOAD_DIR = str(Path(__file__).resolve().parent.parent / "uploads")


def _auto_seed():
    """Run seed.py if no superadmin exists."""
    try:
        from sqlmodel import Session, select
        from database import engine
        from models import Usuario, RolUsuario
        with Session(engine) as session:
            sa = session.exec(select(Usuario).where(Usuario.rol == RolUsuario.SUPERADMIN)).first()
            if not sa:
                logger.info("No superadmin found, running seed...")
                import seed
                seed.main()
    except Exception as e:
        logger.warning(f"Auto-seed skipped: {e}")


async def tarea_recordatorios():
    """Placeholder for appointment reminder task."""
    while True:
        await asyncio.sleep(3600)


@asynccontextmanager
async def lifespan(app: FastAPI):
    init_db()
    _auto_seed()
    asyncio.create_task(tarea_recordatorios())
    yield


app = FastAPI(
    title="MED-SYS Pro API",
    version="2.0.0",
    description="Sistema de Gestión de Práctica Médica - API REST",
    lifespan=lifespan,
    redirect_slashes=False,
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=[
        "http://localhost:4200",
        "http://localhost:80",
        "http://localhost",
    ],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Mount uploads directory
import os
if os.path.exists(UPLOAD_DIR):
    app.mount("/uploads", StaticFiles(directory=UPLOAD_DIR), name="uploads")

# Import and register routers
from routers.auth import router as auth_router
from routers.pacientes import router as pacientes_router
from routers.citas import router as citas_router
from routers.historias import router as historias_router
from routers.usuarios import router as usuarios_router
from routers.urgencias import router as urgencias_router
from routers.farmacia import router as farmacia_router
from routers.facturacion import router as facturacion_router
from routers.hospitalizacion import router as hospitalizacion_router
from routers.admin import router as admin_router
from routers.superadmin import router as superadmin_router
from routers.rips import router as rips_router
from routers.estadisticas import router as estadisticas_router
from routers.agenda_medica import router as agenda_router
from routers.laboratorio import router as laboratorio_router
from routers.imagenes_dx import router as imagenes_router
from routers.evoluciones import router as evoluciones_router
from routers.interconsultas import router as interconsultas_router
from routers.epicrisis import router as epicrisis_router
from routers.nutricion import router as nutricion_router
from routers.rehabilitacion import router as rehabilitacion_router
from routers.quirofano import router as quirofano_router
# Los módulos aún no implementados se montan desde UNA sola lista
# (routers/stubs.py). Antes se importaban 26 nombres a mano y ocho de ellos
# duplicaban routers reales: reordenar estos imports convertía módulos
# funcionales en cascarones sin que nada fallara.
from routers.stubs import ROUTERS_PENDIENTES
from routers.modulos import router as modulos_router
from routers.incapacidades import router as incapacidades_router
from routers.remisiones import router as remisiones_router
from routers.consentimiento import router as consentimiento_router
from routers.contrarreferencia import router as contrarreferencia_router
from routers.nota_operatoria import router as nota_operatoria_router

API_PREFIX = "/api"

core_routers = [
    auth_router, pacientes_router, citas_router, historias_router,
    usuarios_router, urgencias_router, farmacia_router, facturacion_router,
    hospitalizacion_router, admin_router, superadmin_router, rips_router,
    estadisticas_router, agenda_router, laboratorio_router, imagenes_router,
    evoluciones_router, interconsultas_router, epicrisis_router, nutricion_router,
    rehabilitacion_router, quirofano_router, modulos_router,
    incapacidades_router, remisiones_router, consentimiento_router,
    contrarreferencia_router, nota_operatoria_router,
]

stub_routers = ROUTERS_PENDIENTES

for router in core_routers + stub_routers:
    app.include_router(router, prefix=API_PREFIX)


@app.get("/health")
def health():
    try:
        from sqlmodel import Session, text
        from database import engine
        with Session(engine) as session:
            session.exec(text("SELECT 1"))
        return {"status": "ok", "db": "connected"}
    except Exception as e:
        return {"status": "error", "db": str(e)}


# =====================================================================
# EL FRONTEND COMPILADO, SERVIDO POR ESTE MISMO PROCESO
# =====================================================================
#
# Es la decisión de la tarea #73, que hasta ahora estaba escrita y no hecha:
# «el costo de Angular es de COMPILACIÓN, NO DE DESPLIEGUE — `ng build` produce
# estáticos que sirve el mismo FastAPI; una instalación standalone sigue siendo
# UN proceso, sin Node en ejecución».
#
# Lo que quita de en medio: hoy un consultorio necesita nginx además del API, y
# el `docker-compose` monta tres servicios para una máquina que atiende a un
# médico. Con esto el despliegue en sitio es un proceso y un directorio.
#
# En desarrollo NO se usa: `npm start` levanta el servidor de Angular en :4200
# con su proxy a :8000, que es lo que da recarga en caliente. Por eso la
# ausencia de `dist/` no es un error — pero sí se dice en el log cuál de los dos
# modos quedó activo, porque «no se ve la interfaz» tiene causas muy distintas
# en cada uno.

FRONTEND_DIR = Path(__file__).resolve().parent.parent / "frontend" / "dist" / "frontend" / "browser"

#: Prefijos que pertenecen al backend. El comodín del SPA NO los toca.
#:
#: Sin esta lista, `GET /api/pacintes` (con la errata) devolvería el index.html
#: con un 200: el cliente recibiría HTML donde espera JSON y el error saldría a
#: la luz mucho más tarde y en otro sitio. Una ruta de API que no existe tiene
#: que dar 404, que es lo que ya hace FastAPI cuando nadie se interpone.
PREFIJOS_DEL_BACKEND = ("/api", "/uploads", "/docs", "/redoc", "/openapi.json", "/health")


class AngularSPA(StaticFiles):
    """Estáticos de Angular, con `index.html` para las rutas del enrutador.

    Angular resuelve `/pacientes/12` en el navegador: no hay archivo con ese
    nombre y el servidor tiene que devolver el `index.html` para que el enrutador
    haga el resto. Devolver 404 rompe recargar la página o abrir un enlace
    directo — funciona navegando dentro de la aplicación y falla al compartir la
    dirección, que es la forma más incómoda de fallar.
    """

    async def get_response(self, path: str, scope):
        try:
            return await super().get_response(path, scope)
        except StarletteHTTPException as exc:
            if exc.status_code != 404:
                raise
            # Una ruta del backend que llega hasta aquí es una que NO EXISTE:
            # el montaje del SPA va después de los routers, así que todo lo que
            # el backend reconoce ya se resolvió antes. Devolverle el index.html
            # convertiría un 404 honesto en un 200 con HTML.
            if scope.get("path", "").startswith(PREFIJOS_DEL_BACKEND):
                raise
            return await super().get_response("index.html", scope)


if FRONTEND_DIR.is_dir():
    app.mount("/", AngularSPA(directory=str(FRONTEND_DIR), html=True), name="frontend")
    logger.info("Frontend servido desde %s (un solo proceso)", FRONTEND_DIR)
else:
    logger.info(
        "Sin frontend compilado en %s: solo API. En desarrollo es lo normal "
        "(`npm start` sirve la interfaz en :4200). Para un solo proceso: "
        "cd frontend && npm run build",
        FRONTEND_DIR,
    )
