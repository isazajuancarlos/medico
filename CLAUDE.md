# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MED-SYS Pro is a comprehensive medical practice management system (v2.0). It provides EMR, appointment scheduling, patient management, and generates RIPS (Colombia's standardized health data format) reports with full regulatory compliance.

**Tech Stack:**
- **Backend:** FastAPI + Uvicorn, SQLModel (Pydantic + SQLAlchemy), PostgreSQL (pool_size=20) — in `backend/`
- **Frontend:** Angular 21 + PrimeNG 19 (Aura theme), standalone components — in `frontend/`
- **Auth:** JWT (httpOnly cookies + localStorage, access 15min + refresh 7d), bcrypt password hashing
- **Security:** AES-256 encryption, audit logging (Ley 1581/2012 compliance)
- **API:** All endpoints under `/api/*` prefix, documented at `http://localhost:8000/docs`

**User Roles (`RolUsuario` enum in `backend/models.py`):**
- `SUPERADMIN`: Multi-institutional system owner (`institucion_id=None`); impersonates institutions via header dropdown
- `ADMIN`: Institution administrator — user management, RIPS generation
- `MEDICO`: Clinical staff — EMR, prescriptions, appointments
- `ENFERMERIA`: Nursing staff — triage, medication administration
- `PACIENTE`: Patient portal
- Specialist roles: `ANESTESIOLOGO`, `LABORATORISTA`, `RADIOLOGO`, `BACTERIOLOGO`, `TERAPEUTA`, `NUTRICIONISTA`, `PSICOLOGO`, `TRABAJO_SOCIAL`, `REGENTE_FARMACIA`, `AUXILIAR_ENFERMERIA`

## Development Commands

```bash
# === BACKEND (FastAPI) ===
cd backend
pip install -r requirements.txt
python seed.py                              # Initialize DB + seed test data
uvicorn main:app --reload --port 8000       # Start API at http://localhost:8000
# Swagger UI: http://localhost:8000/docs

# === FRONTEND (Angular 21) ===
cd frontend
npm install --legacy-peer-deps             # --legacy-peer-deps required for PrimeNG peer conflicts
npm start                                  # Dev server at http://localhost:4200 (proxies /api → :8000)
npm run build                              # Production build to dist/frontend/browser/

# === DOCKER (full stack) ===
docker-compose up -d        # db (5432) + backend (8000) + frontend nginx (80)
docker-compose down

# === DB utilities ===
cd backend
python importer.py                         # Import CIE-10, CUPS, EPS, INVIMA catalogs
python crear_usuarios_multi_inst.py        # Bulk user creation
python reset_db.py && python seed.py       # DANGER: Full DB reset
```

**Required `.env` file** (at repo root, used by backend):
```
DATABASE_URL=postgresql://user:password@localhost/dbname
SECRET_KEY=your_secret_key_32chars_minimum
```

**Optional `.env` variables:** `ENCRYPTION_KEY`, `SMTP_HOST`, `SMTP_PORT`, `SMTP_USER`, `SMTP_PASSWORD`, `PORT` (default 8000), `HOST` (default 0.0.0.0)

**Test Credentials (from `backend/seed.py`):**
| Username | Password | Role |
|---|---|---|
| `superadmin` | `SuperAdmin2026!` | SUPERADMIN |
| `admin` | `admin123` | ADMIN |
| `medico1` | `medico123` | MEDICO |
| `paciente1` | `paciente123` | PACIENTE |
| `enfermera1` | `enfermera123` | ENFERMERIA |
| `anestesiologo1` | `aneste123` | ANESTESIOLOGO |
| `laboratorista1` | `lab123` | LABORATORISTA |
| `bacteriologo1` | `bact123` | BACTERIOLOGO |
| `radiologo1` | `radio123` | RADIOLOGO |

**Startup behavior:** `backend/main.py` lifespan calls `init_db()` + `auto_seed()` (inserts superadmin only if missing). Schema migrations in `backend/database.py:_run_migrations()` run on every startup via idempotent `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` statements — no manual migration commands needed.

**Docker startup:** `backend/Dockerfile` CMD runs `python seed.py && python main.py` — seed is idempotent.

No test suite exists. Manual testing through the UI or Swagger is required.

## Architecture

### Backend Structure (`backend/`)

```
main.py           # FastAPI app, lifespan (init_db + auto_seed), router registration, CORS, background tasks
database.py       # SQLModel engine, pool config, get_session(), _run_migrations()
models.py         # All SQLModel table definitions (100+ models, 21 enums)
seed.py           # DB init: demo institution + all test users (idempotent)
auditoria.py      # Audit logging (Ley 1581/2012 compliance)
seguridad.py      # AES-256, 2FA, password validation, license management
middleware.py     # License enforcement, rate limiting, session helpers
rips_completo.py  # RIPS generator (AC, AP, AM, AU, AH, CT JSON files)

core/
  config.py       # Settings from .env
  security.py     # JWT create/decode (python-jose)
  deps.py         # FastAPI deps: get_db, get_current_active_user, filtrar_por_institucion

routers/          # 22 implemented + 26 stub routers (return empty lists)
  auth.py         # POST /login, /logout, /refresh, /change-password; GET /me
  pacientes.py    # CRUD + search
  citas.py        # Appointment management
  historias.py    # Medical records (EMR)
  usuarios.py     # User management
  urgencias.py    # Emergency dept (Manchester Triage)
  farmacia.py     # Pharmacy + inventory
  facturacion.py  # Billing
  hospitalizacion.py
  admin.py        # Institution admin functions
  superadmin.py   # Multi-institutional management
  rips.py         # RIPS report generation
  estadisticas.py # KPI dashboard
  agenda_medica.py
  laboratorio.py  # Lab orders + results
  imagenes_dx.py  # Diagnostic imaging (route prefix: /imagenes)
  evoluciones.py  # Clinical progress notes
  interconsultas.py
  epicrisis.py    # Discharge summaries
  nutricion.py
  rehabilitacion.py
  quirofano.py    # OR / surgical suite
  stubs.py        # 26 stub routers for unimplemented modules (via make_stub_router())

schemas/          # Pydantic request/response schemas (separate from SQLModel table models)
  auth.py, paciente.py, usuario.py, cita.py, historia.py, common.py
```

### Frontend Structure (`frontend/src/app/`)

```
app.ts                    # Root standalone component
app.routes.ts             # 40+ lazy-loaded routes with authGuard + roleGuard
app.config.ts             # HTTP interceptors, providers

core/
  auth.service.ts         # Login/logout, current_user signal, default route logic
  auth.guard.ts           # Redirects to /login if no session
  role.guard.ts           # Checks user.rol against route data.roles
  auth.interceptor.ts     # Injects Bearer token in Authorization header

layout/
  main-layout.component   # Sidebar + header + router-outlet + toast
  sidebar.component       # Navigation drawer (role-based)
  header.component        # Top bar with user menu

pages/                    # Implemented pages (lazy-loaded)
  login/, dashboard/, pacientes/, citas/, historias/, urgencias/,
  farmacia/, facturacion/, hospitalizacion/, admin/, superadmin/,
  rips/, estadisticas/, agenda-medica/, laboratorio/, imagenes/,
  evoluciones/, interconsultas/, epicrisis/, nutricion/,
  rehabilitacion/, quirofano/,
  placeholder/            # Reusable stub for unimplemented routes (reads title from route.data['title'])

services/
  paciente.service.ts, cita.service.ts, historia.service.ts
```

### Authentication Flow

**Backend:**
1. `POST /api/auth/login` — bcrypt verify → issue JWT access (15min) + refresh (7d) tokens as httpOnly cookies
2. `GET /api/auth/me` — returns current user from Bearer token
3. All protected endpoints use `Depends(get_current_active_user)` from `core/deps.py`
4. Role enforcement via `require_role(*roles)` factory: `Depends(require_role(RolUsuario.ADMIN, RolUsuario.SUPERADMIN))`

**Frontend:**
1. `AuthService.login()` → stores `access_token` + `current_user` in localStorage
2. `auth.interceptor` injects `Authorization: Bearer <token>` on every HTTP request
3. `auth.guard` reads localStorage; redirects to `/login` if missing
4. `role.guard` checks `current_user.rol` against `route.data.roles`

### Multi-Tenancy

Every `Paciente`, `Cita`, `Historia`, `Usuario` has `institucion_id` for data isolation.

```python
# Backend: always filter by institution in routers
from core.deps import filtrar_por_institucion

def get_pacientes(db: Session = Depends(get_db), current_user = Depends(get_current_active_user)):
    query = select(Paciente)
    query = filtrar_por_institucion(query, Paciente, current_user)
    return db.exec(query).all()
```

SUPERADMIN has `institucion_id=None` and selects a target institution via the header dropdown (stored in the session/token context).

### Critical Data Model Relationships

**`Paciente`** → many `Historia`, many `Cita`, belongs to `Entidad` (EPS), belongs to `Institucion`

**`Historia`** — links `Paciente` + `Usuario` (medico) + optional `Cita`
- Children: `Formula` (prescriptions), `Orden` (orders), `ImagenDiagnostica`
- Dynamic fields: `datos_dinamicos` JSON (configurable via `ConfigCampo`)
- Required fields: `codigo_historia`, `motivo_consulta`, `enfermedad_actual`, `diagnostico_principal`, `plan_tratamiento`, `medico_id`, `paciente_id`
- RIPS fields: `finalidad`, `causa_externa`, `tipo_diagnostico`, `cod_consulta_cups`
- Uses `tension_arterial: str` (not separate systolic/diastolic fields)

**`Cita`** — status flow: `PENDIENTE → CONFIRMADA → ATENDIDA/CANCELADA/NO_ASISTIO`

**`Licencia`** — controls features/limits per institution (max users, patients, monthly appointments)

**Key field gotchas:**
- `AtencionUrgencias` (not `PacienteUrgencias`) — triage field: `motivo_consulta_triage`
- `Cama.estado` values: `"DISPONIBLE"` / `"OCUPADA"` (not a boolean)
- `Hospitalizacion.medico_tratante_id` (not `medico_responsable_id`)
- `MovimientoInventario` requires `tipo_movimiento`, `motivo`, `item_inventario_id`
- `FirmaDigital.timestamp` (not `fecha_firma`)

### Adding a New Router

1. Create `backend/routers/nuevo.py` with FastAPI `APIRouter`
2. Add schemas to `backend/schemas/` if needed
3. Register in `backend/main.py`: add `nuevo_router` to the `core_routers` list (routers are bulk-registered via loop with `prefix=API_PREFIX`)
4. Create `frontend/src/app/pages/nuevo/` with an Angular standalone component
5. Add route to `frontend/src/app/app.routes.ts` with appropriate `roleGuard` data

### RIPS Compliance (Resolución 2275/2023 — Updated 2026)

The system generates Colombian healthcare reports (FEV_RIPS) in JSON format:

**Normativa vigente:**
- Resolución 2275/2023: RIPS JSON structure
- Resolución 2706/2025: CUPS 2026 (effective 01/01/2026)
- Resolución 2765/2025: Plan de Beneficios en Salud 2026
- Resolución 1884/2024: Implementation groups

**Files generated:** AC (Consultas), AP (Procedimientos), AM (Medicamentos), AU (Urgencias), AH (Hospitalización), CT (Control)

```python
from rips_completo import exportar_rips_completo
rips = exportar_rips_completo(fecha_inicio=date(2026,1,1), fecha_fin=date(2026,1,31))
# Returns: {"AC": {...}, "AP": {...}, "AM": {...}, "AU": {...}, "AH": {...}, "CT": {...}}
```

### Security and Compliance

**Audit System (`backend/auditoria.py`):**
```python
from auditoria import registrar_auditoria, auditar_ver_historia, firmar_historia

registrar_auditoria(usuario_id=1, accion="VER_HC", tabla_afectada="historia",
    registro_id=5, paciente_id=10, ip_address="...",
    datos_antes={"dx": "old"}, datos_despues={"dx": "new"})

# Shortcuts:
auditar_ver_historia(usuario_id=1, historia_id=5, paciente_id=10, ip="...")
firmar_historia(medico_id=1, historia_id=5)  # Creates FirmaDigital (SHA-256)

# SUPERADMIN actions (separate AuditoriaSuperAdmin table):
from auditoria import auditar_superadmin, auditar_impersonacion
auditar_superadmin(superadmin_id=1, accion="CREAR_INSTITUCION", descripcion="...",
    institucion_afectada_id=10, ip_address="...")
```

**Encryption (`backend/seguridad.py`):**
```python
from seguridad import EncriptadorAES
encrypted, iv = EncriptadorAES.encriptar("Sensitive patient data")
plaintext = EncriptadorAES.desencriptar(encrypted, iv)
# Store in DatoSensibleEncriptado model
```

**Rate Limiting:** In-memory via `ControlRateLimit`. Default: 5 attempts per 15 minutes. Account lockout via `Usuario.cuenta_bloqueada`.

**Password Policies:** Per-institution via `PoliticaSeguridad` model. History via `HistorialPassword`.

## Important Conventions

### Database Sessions (Backend)
```python
# In routers — use dependency injection:
def my_endpoint(db: Session = Depends(get_db)):
    patient = db.get(Paciente, patient_id)
    patient.nombre1 = "Updated"
    db.commit()  # REQUIRED — SQLModel does not auto-commit

# In utility scripts — use context manager:
with Session(engine) as session:
    session.commit()
```

### Schema Migrations
Do **not** use Alembic. Add new columns to the `sql_migrations` list in `backend/database.py:_run_migrations()`:
```python
"ALTER TABLE paciente ADD COLUMN IF NOT EXISTS nuevo_campo VARCHAR",
```
These run automatically on startup and are idempotent.

### Enum Usage (Backend)
```python
# All enums in models.py are str-based
class EstadoCita(str, Enum):
    PENDIENTE = "Pendiente"

estado == EstadoCita.PENDIENTE  # correct (compare by enum member)
estado == "Pendiente"           # incorrect
```

### File Uploads
Static files served from `uploads/` at `/uploads` URL. Subdirectories:
- `uploads/firmas/` — doctor signatures
- `uploads/imagenes/` — diagnostic images
- `uploads/documentos/` — general documents
- `uploads/consentimientos/` — signed consents
- `logs/` — application logs (created by Docker; create manually in dev)

### Background Tasks
`backend/main.py` registers `tarea_recordatorios()` at startup via `asyncio.create_task()`. **Currently a placeholder** — the function is an infinite `asyncio.sleep(3600)` loop with no actual email logic yet. Hot reload (`--reload`) in dev can double-register this task.

### Module-Specific Notes

**`backend/routers/historias.py` (EMR core):**
- Workflow: Anamnesis → Vital signs → Physical exam → Diagnosis → Plan
- Dynamic fields: Reads `ConfigCampo` for institution-specific fields
- Validation: CIE-10 checks patient age/sex constraints; IMC auto-calculated

**`backend/routers/urgencias.py`:**
- Manchester Triage: 5-level (Rojo/Naranja/Amarillo/Verde/Azul), queue sorted by priority + wait time

**`backend/routers/facturacion.py`:**
- Contract-based tariffs: `Contrato` → `TarifaContrato` (per CUPS); types: ISS, SOAT, CUPS, custom
- Electronic invoicing: `FacturaElectronica` model for DIAN compliance

**`backend/routers/farmacia.py`:**
- Lot control: `ItemInventario` with `lote` + `fecha_vencimiento`
- Dispensation: validates `Formula` exists, reduces stock, logs to `MovimientoInventario`

## Common Gotchas

- **Commit required**: `db.commit()` is NOT automatic in SQLModel
- **N+1 queries**: Use `joinedload()` or `selectinload()` for relationships
- **Postgres pool**: Max 20 connections; always release sessions via context manager or dependency injection
- **Multi-tenancy**: All patient/history/appointment queries MUST be filtered by `institucion_id`
- **Enum comparison**: Compare enum members, not raw strings
- **Frontend install**: Always use `npm install --legacy-peer-deps`
- **RIPS dates**: Use `fecha.isoformat()` → `"2026-01-06"`
- **SUPERADMIN**: Has `institucion_id=None` — handle this case explicitly before filtering by institution
- **Stub routers**: 26 modules in `backend/routers/stubs.py` return empty lists — implement by creating a real router file, importing it in `main.py`, adding it to `core_routers`, and removing it from the `stubs` import

## Regulatory Compliance (Colombian Healthcare)

- **Ley 1581/2012**: Full audit trail in `AuditoriaAcceso` table
- **Ley 527/1999**: Digital signatures via `FirmaDigital` model (SHA-256)
- **Resolución 2275/2023**: RIPS file generation (AC, AP, AM, AU, AH, CT)
- **Production TODO**: Integrate certified digital signature provider (Certicámara, GSE); replace in-memory rate limiting with Redis
