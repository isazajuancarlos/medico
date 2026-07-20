import asyncio
import logging
from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles

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
