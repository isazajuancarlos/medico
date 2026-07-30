# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> El resto del archivo va en español, como el código, los docstrings y los mensajes
> de commit de este repositorio.

## Qué es esto

Reimplementación de un sistema médico que hoy corre en **Visual FoxPro** en el
consultorio de un cliente real (`/mnt/data/aprograms`, fuente autoritativa;
`/mnt/data/repoNuevo`, el generador de RIPS y epidemiológicos). No es un producto
de cero: cada decisión se contrasta contra lo que el prestador ya usa.

Backend **FastAPI + SQLModel + PostgreSQL** en `backend/`, frontend **Angular 21 +
PrimeNG 21** en `frontend/`. Multi-institucional, con licencia por institución que
decide qué módulos se ven.

El eje del trabajo actual es la **cadena RIPS → FEV-RIPS → MUV**: producir el JSON
de la Resolución 948 de 2026 y que el Mecanismo Único de Validación de MinSalud lo
acepte y devuelva CUV. Ver [La cadena RIPS](#la-cadena-rips--fev-rips--muv).

**Y hay un port a Rust en curso, que este archivo no describía.** Ver
[El workspace Rust](#el-workspace-rust).

## Comandos

```bash
# === BACKEND ===
cd backend
.venv/bin/python -m pytest -q                    # 645 pruebas (~62 s)
.venv/bin/python -m pytest tests/test_cie11.py -q         # un módulo
.venv/bin/python -m pytest tests/test_cie11.py::test_x -q # una prueba
.venv/bin/python -m pytest -k "aislamiento" -q            # por nombre

pip install -r requirements.txt
python seed.py                                   # BD + datos de prueba (idempotente)
uvicorn main:app --reload --port 8000            # Swagger en /docs

# === FRONTEND ===
cd frontend
npm install --legacy-peer-deps                   # obligatorio: conflictos de peers de PrimeNG
npm start                                        # :4200, proxy /api → :8000
npm test                                         # vitest vía `ng test`: 5 specs, 57 pruebas
npm run build

# === DOCKER (pila completa) ===
docker-compose up -d                             # db 5432 + backend 8000 + nginx 80

# === UTILIDADES DE BD ===
cd backend
python importer.py                               # CIE-10, CUPS, EPS, INVIMA
python reset_db.py && python seed.py             # PELIGRO: borra todo
```

**Nunca canalizar `pytest` a `tail`/`head` sin `set -o pipefail`**: el código de
salida se pierde y un fallo se lee como verde. Un hook global lo bloquea. Redirigir
a archivo y comprobar `$?` es la forma limpia.

## El workspace Rust

Tres crates —`medico-dominio`, `medico-datos`, `medico-api`— con axum y SQLx sobre
SQLite, y la interfaz Angular **empotrada en el binario** con `rust-embed`. Es el
mismo modelo que informes: se entrega **un archivo**.

```bash
./construir.sh                    # interfaz → empotrado → binario. EL comando.
cargo clippy --workspace --all-targets -- -D warnings   # lo que corre el CI
cargo test  --workspace --all-targets                   # 144 pruebas
```

**`cargo build --release` a secas NO construye la interfaz.** `rust-embed`
empotra lo que haya en `crates/medico-api/estaticos/`, cuyo contenido está en
`.gitignore`: en un clon limpio la carpeta está vacía, y el binario salía
sirviendo la API con un 404 en la raíz **sin avisar**. Desde entonces
`crates/medico-api/build.rs` rompe el release si falta `estaticos/index.html`; si
de verdad se quiere un binario sin interfaz hay que pedirlo con
`MEDICO_SIN_INTERFAZ=1`. `construir.sh` cubre el otro fallo, el del binario
rancio: fija el orden Angular → copia → cargo.

El CI corre clippy y test en perfil **debug**, así que el guardián no dispara ahí.

Para arrancarlo hace falta `SECRET_KEY` (≥ 32 bytes, sin valores conocidos) y
`DATABASE_URL`; el binario **no arranca sin secreto**, a propósito. Y abre la base
relativa a su directorio de trabajo: arrancarlo desde otro sitio crea una base
vacía y el acceso responde «credenciales incorrectas», que es indistinguible de
«esta base no tiene a nadie». Ya costó una tarde.

**Lo que el port todavía NO cubre**, para no confundir avance con paridad:
FEV-RIPS no existe en Rust, y el ADR no autoriza apagar Python hasta que sea byte
a byte idéntico al de Python. Las ramas Rust viven en `feat/cie11-mapeo-oms`;
`main` y `master` siguen siendo el sistema Python, que es el producto vivo.

### Entorno

`.env` en la **raíz del repositorio** (no en `backend/`); `backend/core/config.py` lo
carga con `load_dotenv(_BASE_DIR.parent / ".env")`.

```
DATABASE_URL=postgresql://usuario:clave@localhost/base
SECRET_KEY=<mínimo 32 caracteres>
```

Opcionales: `ENCRYPTION_KEY`, `SMTP_*`, `PORT`, `HOST`.

**No hay valores por defecto para los secretos y no debe haberlos.** `core/config.py`
levanta `ConfiguracionInsegura` y el proceso no arranca si falta `SECRET_KEY` o si es
una de las conocidas (`changeme`, …). Un token firmado con una clave pública se
falsifica, y con eso todo `core/aislamiento.py` deja de proteger nada. Si alguna vez
parece cómodo poner un defecto «solo para desarrollo», ese es exactamente el camino
por el que `changeme` llega a producción.

Las pruebas no tocan el `.env`: `backend/conftest.py` fija `DATABASE_URL` a una
SQLite temporal **con `=`, no con `setdefault`**, antes de que se importe nada, y la
borra al terminar.

## Arquitectura del backend

```
main.py            lifespan (init_db + auto_seed), CORS, registro de routers
database.py        engine, pool=20, _run_migrations() (52 ALTER idempotentes)
models.py          83 tablas SQLModel, 25 enums  (~89 000 líneas de definiciones)
core/
  config.py        secretos: falla al arrancar, no se degrada
  security.py      JWT (python-jose)
  passwords.py     Argon2id con migración oportunista desde bcrypt
  deps.py          get_db, get_current_active_user, filtrar_por_institucion,
                   requiere_modulo, licencia_vigente, hay_cupo
  aislamiento.py   obtener(): el `db.get` seguro entre instituciones
routers/           28 reales + los pendientes desde stubs.py
schemas/           Pydantic de petición/respuesta, separados de las tablas
```

### Aislamiento entre instituciones — las dos capas

1. **Consultas por lista**: `filtrar_por_institucion(query, Modelo, user)` añade el
   `WHERE institucion_id = …`.
2. **Acceso por identificador**: `core/aislamiento.obtener(db, Modelo, id, user)`.
   `db.get(Modelo, id)` **no** pasa por la capa 1 — bastaba incrementar un número en
   la URL para leer, y con `PUT` modificar, la historia de otra institución. Hay 169
   llamadas a `db.get` en los routers; en vez de parchear cada una, existe `obtener()`
   y `tests/test_guardia_aislamiento.py` falla si un router vuelve a usar `db.get`
   directo sobre un modelo con dueño.

Reglas que sostienen esa capa:

- **404, nunca 403**, para un registro de otra institución. Un 403 confirma que
  existe, y con eso se enumera: se prueban identificaciones hasta que una deja de dar
  404 y ya se sabe que esa persona es paciente de la clínica de al lado. Eso también
  es un dato de salud.
- Los modelos clínicos (`Historia`, `Epicrisis`, `EvolucionMedica`…) **no** llevan
  `institucion_id`: cuelgan del paciente. `aislamiento.ENLACES` sigue la cadena.
- Un modelo sin `institucion_id`, sin enlace conocido y no declarado en
  `COMPARTIDOS` levanta `RuntimeError`. Es deliberado: que se rompa en una prueba y
  no en producción sirviendo datos ajenos.
- **El patrón que ya causó una fuga**: `institucion_id: Optional[int] = None` que al
  omitirse deja de filtrar. Pasó en `rips_completo.py`, que `routers/rips.py` llamaba
  sin pasarlo. Está corregido ahí, pero **el barrido del resto del backend sigue
  pendiente** (tarea #88): `fev_rips`, `epidemiologia`, `tarifador`, `auditoria_rips`,
  `muv_client`, `importer`, `datos_sinteticos`.

`SUPERADMIN` tiene `institucion_id=None` y ve todo: es el dueño del sistema, no un
cliente. Hay que tratar ese caso explícitamente antes de filtrar.

### Licencia y módulos — tres estados, no dos

Se vende **el mismo binario** a un consultorio, una IPS y un hospital; lo que cambia
es `Licencia.modulos_habilitados`. `routers/modulos.py` alimenta el menú del frontend
y distingue:

| estado | significado | respuesta |
|---|---|---|
| `habilitado` | licenciado y funcionando | la del router real |
| `pendiente` | licenciado pero sin implementar | **501** desde `routers/stubs.py` |
| `no_incluido` | existe en el producto, no en su plan | 403 vía `requiere_modulo` |

**Los stubs devuelven 501, no `200 []`.** Quien abre «Vacunación» y ve una lista
vacía no concluye «este módulo no existe», concluye «este paciente no tiene vacunas».
En un sistema clínico esa confusión es un dato falso.

`requiere_modulo(ModuloSistema.X)` se pone **a nivel de router**, en
`dependencies=[...]`, no endpoint por endpoint: así no se olvida en uno.

Nunca deben coexistir un stub y un router real con el mismo nombre. Llegó a haber
ocho, y bastaba reordenar los imports de `main.py` para que los cascarones tumbaran
módulos que sí funcionaban, sin que nada fallara.

Vigencia y cupos (`core/deps.py`):

- **Los cupos se cuentan, no se leen.** `Licencia.uso_actual_*` existe en el modelo y
  nadie lo actualiza: vale 0 desde la semilla. El control heredado comparaba contra
  esos campos, así que nunca podía saltar. Se cuentan las filas reales. Un usuario
  inactivo no consume cupo.
- **Una licencia vencida no ciega la historia clínica.** Se bloquea la EXPANSIÓN
  (crear usuarios, dar de alta médicos), no la lectura. Dejar a un hospital sin sus
  registros por un asunto comercial lo pone a incumplir ante el paciente, la EPS y la
  Supersalud.

## La cadena RIPS → FEV-RIPS → MUV

```
rips_completo.py  →  fev_rips.py  →  muv_client.py  →  CUV
   (6 documentos)     (1 documento)   (contenedor de MinSalud)
```

- **`rips_completo.py`** arma la forma anidada `usuarios[] → servicios{}` y emite los
  seis documentos históricos (AC, AP, AM, AU, AH, CT).
- **`fev_rips.py`** los funde en el **documento único** de FEV-RIPS: un paciente con
  consulta y medicamento aparece una vez con las dos cosas. El CT se descarta — es del
  formato plano de la 3374. `null` donde antes iba `""`, fechas sin segundos.
- **`muv_client.py`**: `login → cargar → CUV`. El transporte va **inyectado**, así que
  las pruebas corren sin red y sin contenedor. Las credenciales son **del prestador**
  (MinSalud confirmó en el caso 2026403888 que no existe credencial de integrador):
  se reciben en un objeto con `__repr__` censurado y no aparecen en código ni en logs.
- **`validar_muv.py`** corre el ciclo completo a mano. Credenciales por entorno, nunca
  por línea de órdenes.
- **`muv/`** levanta el validador de MinSalud en local. Leer `muv/README.md` antes de
  tocarlo. **Solo el ambiente de stage**: en producción una factura inventada obtiene
  un CUV verdadero, con efectos jurídicos y financieros reales.

Un **HTTP 400 con resultado dentro es un RECHAZO del archivo, no una avería del
cliente**, y hay que tratarlo como tal.

Cómo redacta la queja el MUV importa al leer un rechazo:

```
«valores permitidos entre 1 y 999»            → rango: no comprueba nada más
«no es válido, verificar tabla de referencia» → sí contrasta contra el catálogo
```

Un campo del primer tipo no es opcional: es el sitio más fácil para que se cuele un
dato falso sin que nadie se entere.

### CIE-11 (Resolución 948 de 2026)

Son **22 campos** (11 `cod*CIE11` + 11 `nomCod*CIE11`), no 35. `cie11.py` los deriva
del CIE-10:

| caso | qué hace |
|---|---|
| equivalencia exacta y única | **deriva** sin preguntar |
| equivalencia por categoría | **propone**, menos específica |
| varios destinos | los propone **todos**, no elige |
| sin equivalencia | lo dice, y el campo queda vacío |

**1 402 códigos (11,3 %) tienen más de un destino, hasta 42.** Poner el primero de una
lista de 42 es exactamente lo mismo que `cod_consulta_cups="890201"`: un dato con la
forma correcta que nadie afirmó. La 948 admite estos campos vacíos.

## Catálogos: empaquetados, no consultados por red

`backend/datos/` va versionado con el código. Un consultorio en zona rural no puede
depender de que `web.sispro.gov.co` esté arriba el día del cierre de mes. Regenerar es
un paso deliberado.

| archivo | qué es | se regenera con |
|---|---|---|
| `divipola.json` | municipios y departamentos | publicaciones oficiales |
| `cum.json.gz` | 65 997 CUM del INVIMA (2,6 MB) | `herramientas/generar_cum.py` |
| `mapeo_cie10_cie11.json` | tablas de la OMS | `../herramientas/generar_mapeo_cie11.py` |
| `cie10_rips.json` | 12 422 códigos | `../herramientas/extraer_cie10_rips.py` |
| `maestros_muv/*` | 8 catálogos del validador | `herramientas/extraer_maestros_muv.py` |
| `normativa/campos_fev948.json` | campos de la 948 | `../herramientas/extraer_campos_fev948.py` |

**Ningún código de catálogo se escribe a mano.** El 2026-07-26 se inventó un mapa de
presentaciones (`tableta = 1`, cuando el catálogo dice 66 y el 1 es AMPOLLA); el MUV lo
aceptó y devolvió CUV, porque ese campo lo valida por rango. Los maestros salen del
contenedor del validador, que es la fuente contra la que el MUV juzga.

Trampas ya medidas y respetadas al cargar:

- **El CUM no es una columna**: es `expedientecum` + `"-"` + `consecutivocum`.
- Las filas del CSV son el **producto cartesiano** de principios activos × roles;
  cargar fila a fila multiplica por 2,4.
- **Fechas centinela**: 28 822 filas traen vencimiento en el año 3000 (más 9999, 3001,
  3200, 1900). No son fechas: son «sin vencimiento» escrito como fecha. Van a NULL.
- **El cero a la izquierda de DIVIPOLA no es cosmético**: `codMunicipioResidencia` va
  de cinco dígitos y 148 municipios (13 % del país) empiezan por cero, Medellín entre
  ellos. Los datos de MinSalud los publican sin él, porque pasaron por una hoja de
  cálculo.

## Procedencia: de dónde salió cada dato

`procedencia.py` + `derivaciones.py` son la respuesta al problema de los defectos.

El sistema predeterminaba valores porque el clínico tiene doce minutos por paciente:
`cod_consulta_cups="890201"`, `estado_paciente_salida="ESTABLE"`, `valor_servicio=0`.
Datos que nadie afirmó, indistinguibles de los reales. Pero quitarlos sin poner nada
en su lugar solo hace la captura más lenta, y **lo lento se rodea**: se cambia un dato
falso por uno basura, con más fricción.

La salida es marcar el origen: `CONFIRMADO` · `DERIVADO` · `PROPUESTO_ACEPTADO` ·
`AUSENTE`. Un valor derivado de otros que ya estaban cuesta cero segundos y es
reproducible. `derivaciones.py` devuelve `None` cuando no puede derivar — no aproxima.

`auditoria_rips.py` mira la historia **antes** de generar y dice qué atenciones van
completas y cuáles con un supuesto: el usuario corrige doce consultas hoy en vez de
descubrir en un mes que reportó mil.

`tarifador.py` resuelve `valor_servicio` desde `Contrato → TarifaContrato`, y lo
**congela al prestar el servicio**. Las tarifas cambian; si el informe de marzo se
generara con la tarifa de hoy, dos ejecuciones del mismo periodo darían cifras
distintas y ninguna sería la pactada. Congelarlo es lo que permite responder una glosa.

**Regla dura**: nada que sugiera un modelo se escribe solo en la historia clínica.
Propone, un humano confirma, y queda registrado cuál fue cuál.

## Fidelidad al sistema original

`antropometria.py` y `epidemiologia.py` migran fórmulas de VFP **literalmente**, aunque
parezcan mejorables: si los números no coinciden con los que el prestador viene
reportando, el informe nuevo no sirve. Las mejoras van en una etapa aparte, y así se
anota en el código (p. ej. el peso ideal no se calcula en menores de 12 años, que es
justo donde más importa).

La única desviación deliberada es el **cálculo de la edad**: el original usaba años de
365 días y meses de 30, lo que envejece a la gente. Se corrigió porque en pediatría el
tramo «0 años» va del recién nacido al de once meses.

## Frontend

Angular 21.2 + PrimeNG 21.1, standalone, 29 páginas, 49 rutas con `authGuard`,
`roleGuard` y `modulo.guard`.

`src/app/shared/formulario/` es el hallazgo que cambió el método de migración: **los
37 formularios de VFP no son 37 diseños**, todos instancian el mismo marco (zoom con
clic derecho 522 usos, navegación primero/anterior/siguiente/último, barra CRUD
wznew/wzedit/wzdelete/wzsave/wzundo en 18 formularios). El marco se construye una vez
—`estado-formulario.ts`, `barra-crud.component.ts`, `zoom.directive.ts`,
`teclas.directive.ts`— y cada pantalla pasa a ser configuración. La receta está en
`MIGRACION_FORMULARIOS.md`.

Aplicado ya a seis pantallas: pacientes, incapacidades, remisiones, contrarreferencia,
consentimiento y nota-operatoria.

### Un solo proceso en despliegue

`npm run build` y el mismo FastAPI sirve el Angular compilado desde
`frontend/dist/frontend/browser` (`AngularSPA` al final de `main.py`). En sitio no
hace falta nginx: un proceso y un directorio. Es lo que decidió #73 — «el costo de
Angular es de compilación, no de despliegue».

En desarrollo no se usa: `npm start` da recarga en caliente en :4200 con proxy a
:8000, y la ausencia de `dist/` es normal. El log dice cuál de los dos modos quedó
activo.

**El comodín del SPA no puede tapar el backend.** Va montado después de los routers,
así que solo ve lo que nadie reconoció — y ahí devolvería `index.html` con 200 a
`GET /api/pacintes`, que es una errata. `PREFIJOS_DEL_BACKEND` la deja pasar como
404. Si se añade un prefijo nuevo del backend, va en esa lista;
`tests/test_frontend_embebido.py` lo comprueba contra el esquema OpenAPI y falla si
falta.

## Convenciones

- **`db.commit()` no es automático** en SQLModel.
- **Migraciones sin Alembic**: añadir a la lista `sql_migrations` de
  `database.py:_run_migrations()`, con `ADD COLUMN IF NOT EXISTS`. Corren en cada
  arranque y son idempotentes.
- **Enums**: todos heredan `str, Enum`. Comparar por miembro (`estado ==
  EstadoCita.PENDIENTE`), no por cadena.
- **Router nuevo**: crearlo en `backend/routers/`, importarlo en `main.py`, añadirlo a
  `core_routers`, y **quitarlo de `PENDIENTES` en `stubs.py`**.
- **Aislamiento**: toda consulta de paciente/historia/cita filtrada por institución, y
  todo acceso por id vía `obtener()`.
- **N+1**: `joinedload()` / `selectinload()`. El pool son 20 conexiones.
- **Frontend**: siempre `npm install --legacy-peer-deps`.
- Los `.py` de la raíz (`models.py`, `database.py`, `seed.py`, `auditoria.py`,
  `seguridad.py`, `importer.py`, `reset_db.py`) son **copias antiguas de la etapa
  NiceGUI**, idénticas o casi a las de `backend/`. El código vivo es el de `backend/`.
  No editar los de la raíz.

### Detalles del modelo que ya han costado tiempo

- `AtencionUrgencias` (no `PacienteUrgencias`); campo de triage
  `motivo_consulta_triage`.
- `Cama.estado` es `"DISPONIBLE"` / `"OCUPADA"`, no un booleano.
- `Hospitalizacion.medico_tratante_id` (no `medico_responsable_id`).
- `MovimientoInventario` exige `tipo_movimiento`, `motivo`, `item_inventario_id`.
- `FirmaDigital.timestamp` (no `fecha_firma`).
- `Historia` usa `tension_arterial: str`, no sistólica/diastólica separadas.
- Fechas RIPS: `fecha.isoformat()` → `"2026-01-06"`.

## Cumplimiento

- **Ley 1581/2012** — rastro completo en `AuditoriaAcceso`; `AuditoriaSuperAdmin` para
  las acciones del superadmin. Atajos en `auditoria.py`.
- **Ley 527/1999** — `FirmaDigital` (SHA-256). En producción falta proveedor
  certificado (Certicámara, GSE).
- **Resolución 948/2026** — FEV-RIPS. `tests/test_campos_fev948.py` convierte la norma
  en prueba, en vez de dejarla como prosa.
- **Resolución 2275/2023** — estructura RIPS. **2706/2025** CUPS 2026. **2765/2025**
  PBS 2026.
- Cifrado AES-256 en `seguridad.py` (`EncriptadorAES` → `DatoSensibleEncriptado`).
- Rate limiting en memoria (`ControlRateLimit`, 5 intentos / 15 min). En producción
  debe ser Redis.

## Reglas que no se rompen

1. **No abrir los `.dbf` de datos de `/mnt/data/aprograms`** (98 ficheros): son
   historias clínicas, categoría especial de la Ley 1581. Estructura, código y
   catálogos de códigos sí; contenido de pacientes nunca.
2. **`/mnt/data/aprograms` es la fuente autoritativa**, no `/mnt/data/medfuente`
   (copia aplanada que da resultados engañosos).
3. **Contra el MUV, solo el ambiente de stage y solo con datos sintéticos**
   (`datos_sinteticos.py`). El ambiente de pruebas de MinSalud tiene controles más
   débiles y una finalidad distinta de aquella para la que se recogieron las historias.
   Los códigos sí son reales, para que un rechazo no venga por el motivo equivocado.
4. **Las credenciales del MUV son del prestador.** Nunca en código, log ni excepción.
5. **Ningún secreto con valor por defecto.** Fallar al arrancar, no degradarse.

## Tareas pendientes del proyecto

Inventario en `/mnt/data/conocimiento/PENDIENTES.md` (se regenera solo en cada turno).
**No recrear estas tareas con `TaskCreate`**: la deduplicación es por texto y una tarea
reescrita «mejor» se suma a la original en vez de sustituirla.

| # | pendiente |
|---|---|
| **#16 / #65** | Migración VFP completa. Quedan **21 módulos en 501** y tres sin existir (`soportecobertura`, `terceros`, `ayudasdx`). Orden fijado por Juan: **migración completa primero, mejoras después**. |
| **#67** | FEV-RIPS por archivo. AC y AM están verificados por dos fuentes; **AP/AU/AH/RN/OS no se implementan sin contrastarlos a ojo contra el PDF** — las tablas de urgencias, recién nacidos y otros servicios no se dejaron extraer por prefijo. |
| **#77** | Cargar el CUM a **base de datos** (66 mil registros que caducan, no JSON empaquetado). `Medicamento` se queda corto: faltan estado_cum, registro sanitario, vencimiento anulable, ATC, principio activo, vía, unidad e IUM. Pendiente aparte: CUPS (`cups.xlsx`, cabecera corrida una columna). |
| **#88** | Barrer el backend buscando el patrón «filtro opcional» fuera de los routers, y decidir si `test_guardia_aislamiento.py` puede cubrir esa capa. |
| **#84** | Captura asistida: fricción graduada — bloquear solo donde hay efecto jurídico o económico. |
| **#104** | Antropometría: migrar `PESOTALLA1` (62 filas, con índice EDAD), **no** `PESOTALLA`. En el original el `SEEK` abría la tabla equivocada y la talla ideal salía siempre en 0. |
| **#80** | Angular 21.2.0 → 21.2.18 y decidir el salto a la 22. Los paquetes se peer-dependen con versión exacta: `ng update @angular/core@21 @angular/cli@21`, nunca `npm install` sobre el árbol existente. |
| **#65.7** | La verificación que de verdad cierra el RIPS: comparar el JSON propio contra el del conversor web de MinSalud para el mismo caso. |
| **#65.6** | Panel para editar `modulos_habilitados` sin tocar la base. Bloquea dar de alta un cliente nuevo. |

Preguntas abiertas para el prestador: qué hace `mousemovex` (108 usos); si alguna vez
registró un CVE distinto del diagnóstico (los informes darían números diferentes,
porque ahora el evento se deriva del diagnóstico); si el archivo que produce hoy pasa
el validador 1.4.8.0.

Cerradas y ya no pendientes: #79 (las pruebas compartían una BD por accidente — lo
resuelve `conftest.py`), DIVIPOLA y CIE-10 cargados, ejecutor de pruebas del frontend
montado.
