<!-- SPDX-License-Identifier: LicenseRef-Propietario -->
# medico

Sistema de información para prestadores de servicios de salud en Colombia:
historia clínica, atención, facturación y la cadena **RIPS → FEV-RIPS → MUV** de
la Resolución 948 de 2026.

No es un producto de cero. Reimplementa un sistema que corre hoy en **Visual
FoxPro** en el consultorio de un cliente real, y cada decisión se contrasta
contra lo que el prestador ya usa: las fórmulas de antropometría y los informes
epidemiológicos se migran literalmente aunque parezcan mejorables, porque si los
números no coinciden con los que viene reportando, el informe nuevo no sirve.

**Software propietario.** Ver [Titularidad y licencia](#titularidad-y-licencia).

## Los dos sistemas que conviven

| | producto vivo | port en curso |
|---|---|---|
| **Backend** | FastAPI + SQLModel + PostgreSQL (`backend/`) | axum + SQLx + SQLite (`crates/`) |
| **Interfaz** | Angular 22 + `@angular/material` (`frontend/`) | la misma, **empotrada en el binario** |
| **Entrega** | proceso + base de datos | **un archivo** |
| **Ramas** | `main`, `master` | `feat/*` |

> **PrimeNG salió el 2026-08-02.** El estándar de todos los desarrollos es
> Angular con `@angular/material`; medico era el único que no cumplía. La
> migración terminó: **287 → 0** importaciones de `primeng`, y la librería y
> `@primeuix/themes` salieron del `package.json`, con la compilación y las
> pruebas en verde en cada lote (82 → **366**). La receta y las trampas, en
> [`MIGRACION_MATERIAL.md`](MIGRACION_MATERIAL.md). Nunca se subió `primeng` a
> la 22: dejó de ser MIT (es «PrimeUI», comercial, con clave obligatoria) y un
> binario que vive offline en casa de un cliente no puede llevar dentro una clave
> que caduca. De la familia solo queda `primeicons`, que es MIT y es una fuente
> de iconos.

**Y el salto se hizo el mismo día**: Angular 22.1.0, Material 22.1.0, CDK 22.1.0
y TypeScript 6.0.3 (#254). Con eso los dos desarrollos —medico e informes— corren
la misma versión de Angular, que era el objetivo. Dos migraciones de la 22 hay
que leerlas y no solo aceptarlas: en la 22 **OnPush pasa a ser el defecto**
(`ChangeDetectionStrategy.Eager` es el opt-out que dejó la herramienta en 71
componentes, y ahí sigue a propósito), y el cliente HTTP pasa a `fetch`
(`withXhr()` es el opt-out).

El `--legacy-peer-deps` desapareció con PrimeNG: `npm ci` limpio funciona, y así
está el CI y `construir.sh`.

El port a Rust **no reemplaza** al Python todavía, y este README no va a decir lo
contrario para que parezca más avanzado: **FEV-RIPS no existe en Rust** —solo el
recolector de datos faltantes—, y sin él no hay facturación electrónica, que es
por lo que le pagan al prestador. Comparados los prefijos de `rutas.rs` contra los
de `backend/routers/` el 2026-08-01, siguen sin backend en Rust diez prefijos y
uno va a medias. Cinco los llama la interfaz, así que hoy romperían una pantalla:
`/api/facturacion`, `/api/estadisticas`, `/api/admin`, `/api/quirofano` y
`/api/consentimiento`. `/api/rips` ya existe pero **solo emite el AC**, y se
niega a hacerlo sin un `parcial=true` explícito: un archivo sin los
procedimientos ni los medicamentos parece completo y no lo está. Los otros cinco
no los llama nadie desde Angular, pero son
Python que sigue vivo: `/api/superadmin`, `/api/epicrisis`, `/api/evoluciones`,
`/api/interconsultas` y `/api/nota-operatoria`. El ADR
no autoriza apagar Python hasta que la generación del RIPS sea byte a byte
idéntica.

Lo que el port **sí** cubre: pacientes, historia clínica con firma, urgencias,
hospitalización, nutrición, rehabilitación, remisiones y contrarreferencia,
incapacidades, citas y **agenda médica** (franjas, bloqueos, turnos libres e
indicadores), catálogo de módulos por licencia, farmacia e inventario,
proveedores, laboratorio, imágenes diagnósticas y ambulancias. Los cuatro
maestros —CIE-10, CUPS, CUM y DIVIPOLA— van empotrados en el binario.

## Arrancar

```bash
# --- Python (el producto vivo) ---
cd backend
.venv/bin/python -m pytest -q          # 676 pruebas
python seed.py                         # base + datos de prueba (idempotente)
uvicorn main:app --reload --port 8000  # Swagger en /docs

# --- Rust (el port) ---
./construir.sh                         # interfaz → empotrado → binario. EL comando.
cargo test  --workspace --all-targets  # 488 pruebas
cargo clippy --workspace --all-targets -- -D warnings

# --- Interfaz ---
cd frontend
npm ci                                 # sin --legacy-peer-deps: era por PrimeNG, y PrimeNG ya no está
npm start                              # :4200 con proxy a :8000
```

`cargo build --release` a secas **no** construye la interfaz: `rust-embed`
empotra lo que haya en `crates/medico-api/estaticos/`, y en un clon limpio esa
carpeta está vacía. El `build.rs` rompe el release si falta `index.html`; para un
binario sin interfaz hay que pedirlo con `MEDICO_SIN_INTERFAZ=1`.

### Configuración

`.env` en la raíz del repositorio. **Ningún secreto tiene valor por defecto y no
debe tenerlo**: el proceso no arranca sin `SECRET_KEY` (≥ 32 bytes) ni con una
conocida (`changeme`, `dev`, …). Un token firmado con una clave pública se
falsifica, y con eso el aislamiento entre instituciones deja de proteger nada.

| variable | qué es |
|---|---|
| `DATABASE_URL` | la base. Obligatoria. |
| `SECRET_KEY` | firma las sesiones. Obligatoria, ≥ 32 bytes. |
| `MEDICO_TRAS_PROXY` | declara que hay nginx delante. Cambia de dónde sale la IP de origen y dónde escucha el binario. |
| `MEDICO_ESCUCHA` | interfaz de escucha. Por defecto `127.0.0.1` tras proxy y `0.0.0.0` sin él. |
| `MEDICO_SIGNING_KEY` / `MEDICO_VERIFYING_KEY` | firma criptográfica de la historia clínica (`medico-api claves-firma` las genera). |
| `MEDICO_STORAGE_KEY` | cifra en reposo los archivos subidos. Sin ella se guardan en claro. |

## Cómo está construido

### Multi-institucional: una regla, y no se puede omitir

El mismo binario sirve a un consultorio, a una IPS y a un hospital. Lo que separa
sus datos es `Ambito`, y es un **parámetro obligatorio** de toda consulta: no hay
forma de leer una fila sin decir desde dónde se mira. En Python la misma regla
eran 169 llamadas a `db.get` que había que acordarse de envolver.

**404, nunca 403, para lo ajeno.** Un 403 confirma que el registro existe, y con
eso se enumera: se prueban identificaciones hasta que una deja de dar 404 y ya se
sabe que esa persona es paciente de la clínica de al lado. Eso también es un dato
de salud. Por eso `Visibilidad` no tiene una variante `Prohibido`: al no existir
el valor, ningún camino del código puede filtrarlo por descuido.

### Autorización por capacidad, escrita en la firma

Una ruta declara lo que necesita en su propio tipo — `Autorizado<Firmar>` en vez
de `Autenticado`—, así que se lee en el listado del router sin abrir el cuerpo.
Nació de que, de 62 rutas, seis comprobaban el rol y ninguna sobre un recurso
clínico: cualquier usuario de la institución podía **recetar medicamentos**.

Las listas por rol son **defectos**, no la última palabra: la planta de una IPS
rural de tres personas y la de un hospital de tercer nivel no se parecen.

### Nada que sugiera un modelo se escribe solo en la historia

El sistema predeterminaba valores porque el clínico tiene doce minutos por
paciente: `cod_consulta_cups="890201"`, `estado_paciente_salida="ESTABLE"`,
`valor_servicio=0`. Datos que nadie afirmó, indistinguibles de los reales. Pero
quitarlos sin poner nada en su lugar solo hace la captura más lenta, y **lo lento
se rodea**. La salida es marcar el origen de cada dato: `CONFIRMADO` ·
`DERIVADO` · `PROPUESTO_ACEPTADO` · `AUSENTE`.

Es la misma regla en todo el sistema: **ante un dato ausente se falla o se dice
que falta; nunca se rellena**. Un SOAT sin registrar no es un SOAT vigente, un
módulo sin implementar responde 501 y no `200 []` —quien abre «Vacunación» y ve
una lista vacía concluye que el paciente no tiene vacunas—, y un lote de
medicamento sin fecha no está «al día».

### Lo cerrado no se reabre

Historia firmada, documento clínico firmado, resultado de laboratorio validado e
informe de imágenes firmado son **inmutables**. Sobre un resultado de laboratorio
un médico ya decidió una conducta: reescribirlo deja la decisión apuntando a un
dato que ya no existe. Se corrige emitiendo otro.

### Catálogos empaquetados, no consultados por red

`backend/datos/` va versionado con el código: CIE-10, CUPS, DIVIPOLA, los 65 997
CUM del INVIMA y los maestros del validador de MinSalud. Un consultorio en zona
rural no puede depender de que `web.sispro.gov.co` esté arriba el día del cierre
de mes.

**Ningún código de catálogo se escribe a mano.** El 2026-07-26 se inventó un mapa
de presentaciones (`tableta = 1`, cuando el catálogo dice 66 y el 1 es AMPOLLA);
el MUV lo aceptó y devolvió CUV, porque ese campo lo valida por rango.

## Cumplimiento

| norma | qué cubre |
|---|---|
| **Resolución 948/2026** | FEV-RIPS y los 22 campos CIE-11. `tests/test_campos_fev948.py` convierte la norma en prueba. |
| **Resolución 2275/2023** | estructura RIPS. **2706/2025** CUPS 2026. **2765/2025** PBS 2026. |
| **Ley 1581/2012** | rastro de acceso a datos personales, con bitácora encadenada y firmada. Tres formas de evento: un **recurso** suyo, un **conjunto de registros suyos** (sus historias, sus resultados) y una **colección** que atraviesa pacientes. Las dos primeras entran en el rastro del titular; la tercera, solo si es una exportación y su dato salió. Al consultar el rastro de uno, la lista de exportados no se devuelve: sale cuántos eran, nunca quiénes. |
| **Ley 527/1999** | firma electrónica de la historia clínica (Ed25519 + ML-DSA-87 vía Quipu). |
| **Resolución 3100/2019** | habilitación; el módulo de ambulancias usa sus modalidades TAB y TAM. |

Contra el Mecanismo Único de Validación de MinSalud **solo se prueba en el
ambiente de stage y solo con datos sintéticos**: en producción una factura
inventada obtiene un CUV verdadero, con efectos jurídicos y financieros reales.

## Titularidad y licencia

Software **propietario**. Copyright © 2026 Juan Carlos Isaza Arenas. Todos los
derechos reservados. Ver [`LICENSE`](LICENSE) y [`NOTICE`](NOTICE).

No se publica en crates.io ni en PyPI: el workspace declara `publish = false` a
propósito, para que un `cargo publish` accidental falle.

Las dependencias propias —[Quipu](https://github.com/isazajuancarlos/quipu) y
[guaca](https://github.com/isazajuancarlos/guaca)— sí son libres, bajo sus
propias licencias.

## Dónde está el resto

`CLAUDE.md` documenta las trampas ya medidas: qué campos del modelo han costado
tiempo, por qué el cero a la izquierda de DIVIPOLA no es cosmético, cómo redacta
sus rechazos el MUV y en qué se diferencia un rango de una validación contra
catálogo. Se lee antes de tocar la cadena RIPS.
