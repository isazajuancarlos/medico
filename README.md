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

## El producto, y lo que queda del sistema anterior

El port terminó: **el producto es el binario Rust**. Del Python ya no queda API
—`main.py` y los `routers/` se retiraron el 2026-08-09— sino la cadena
RIPS/FEV-RIPS como módulos, que es el ORÁCULO contra el que Rust prueba su
paridad byte a byte.

| | el producto | lo que queda del anterior |
|---|---|---|
| **Servidor** | axum + SQLx + SQLite, cinco crates en `crates/` | la cadena RIPS/FEV-RIPS en `backend/`, sin servidor |
| **Interfaz** | **`maud` + htmx, tejida en el servidor** | — |
| **Entrega** | **un archivo** | — |

> ## Angular salió el 2026-08-08
>
> Las **29 pantallas** viven en el binario Rust, tejidas en el servidor con
> `maud` + htmx y una micro-librería vanilla propia (directiva 26). Con ellas se
> retiró el SPA entero: `frontend/` —482 MB, 98 archivos TypeScript—, el
> empotrado con `rust-embed`, el `build.rs` que rompía el *release* sin
> `index.html`, la bandera de escape `MEDICO_SIN_INTERFAZ`, el trabajo
> `frontend` del CI y los dos primeros pasos de `construir.sh`.
>
> **El build dejó de necesitar Node.** `maud` son macros: el HTML lo compila
> `rustc`, con los tipos del dominio dentro, así que un error de plantilla es un
> error de compilación y no una pantalla en blanco a las tres semanas. htmx
> (MIT), `boi.js` y la hoja de estilos van con `include_str!` desde
> `crates/medico-api/panel/`, que está versionado. **El recuento de dependencias
> de frontal del producto entero es UNA.**
>
> **`/` redirige a `/panel`**, y lo que no case es un **404 de verdad**. Antes
> el comodín del SPA devolvía el `index.html` con 200 a cualquier ruta
> desconocida: un `<script src>` con una errata recibía HTML, el navegador lo
> descartaba por el MIME y la pantalla se veía bien sin que nada fallara.
>
> Lo que se pierde y no se disimula: el buscador incremental de medicamentos y
> de CUPS (hoy son campos de texto), y los gráficos de anillo y tarta de
> estadísticas (hoy barras y columnas en SVG, con el número al lado). La receta
> de aquella migración se conserva en
> [`MIGRACION_MATERIAL.md`](MIGRACION_MATERIAL.md) y
> [`MIGRACION_FORMULARIOS.md`](MIGRACION_FORMULARIOS.md) como historia del
> repositorio, no como instrucciones vigentes.

**La cadena RIPS ya está entera en Rust**, y este README decía lo contrario
hasta el 2026-08-10. `/api/rips` emite **los cinco** documentos desde el
2026-08-04; el **FEV-RIPS** —el documento único que los funde y es por lo que le
pagan al prestador— se generó en Rust el 2026-08-08 y su paridad con el Python
está probada **byte a byte** y **de extremo a extremo**, cada uno leyendo la base
con su propia capa de datos. El **cliente del MUV** que lo sube y trae el CUV se
portó el 2026-08-10 (`crates/medico-muv`).

**Cuánto falta.** La cuenta que se llevaba —cuántos de los endpoints que pedía
Angular servía ya el binario— **dejó de tener sentido el 2026-08-08**, cuando
Angular salió: la última medición, del 2026-08-06, dio 197 de 197, y hoy no hay
un frontal externo cuyas llamadas contar. Las pantallas no piden endpoints: las
teje el mismo proceso que tiene los datos.

Lo que falta, dicho sin adornos: **`medico-muv` todavía no lo llama nadie**.
Enchufarlo exige decidir dónde viven las credenciales SISPRO del prestador —son
suyas, no nuestras, y no caben en una columna en claro— y elegir el backend TLS,
porque el contenedor del validador habla HTTPS. Hasta entonces el envío al MUV se
hace con el guion Python, a mano y contra el ambiente de stage.

Lo que el port **sí** cubre: pacientes, historia clínica con firma, urgencias,
hospitalización, nutrición, rehabilitación, remisiones y contrarreferencia,
incapacidades, citas y **agenda médica** (franjas, bloqueos, turnos libres e
indicadores), catálogo de módulos por licencia, farmacia e inventario,
proveedores, laboratorio, imágenes diagnósticas, ambulancias, facturación,
consentimiento informado, los cuatro documentos clínicos bajo `/api/documentos`,
los ocho paneles de estadística, **quirófano** (salas y programación quirúrgica,
con su máquina de estados) y **administración** (ficha de la institución,
configuración de operación, política de seguridad, campos dinámicos de la
historia, maestros y gestión de personal). Los cuatro maestros —CIE-10, CUPS, CUM y
DIVIPOLA— van empotrados en el binario.

## Arrancar

```bash
# --- El producto ---
./construir.sh                         # el binario. UN paso, sin Node.
cargo test  --workspace --all-targets  # 1 059 pruebas
cargo clippy --workspace --all-targets -- -D warnings

# --- La cadena RIPS/FEV del Python (oráculo de la paridad, y el MUV a mano) ---
cd backend
.venv/bin/python -m pytest -q          # 499 pruebas
```

**Ya no hay `uvicorn` que arrancar**: `main.py` y los `routers/` se retiraron el
2026-08-09, así que no hay servidor Python ni Swagger. Lo que queda de ese árbol
se usa como módulos.

**No hace falta Node.** `./construir.sh` es hoy `cargo build --release` con su
comentario: las plantillas las compila `rustc` y lo vendorizado va con
`include_str!`. El guion se conserva porque es el comando que nombran el
despliegue y la documentación, y para que el día que vuelva a haber un paso
previo vuelva ahí y no a la memoria de nadie.

La interfaz se ve arrancando el binario: `/` lleva a `/panel`, y sin sesión el
panel rebota a `/panel/entrada`.

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

Software **propietario**. Copyright © 2026 Juan Carlos Isaza. Todos los
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

`DETALLE.md` lleva lo demás, para consultar con `grep`: la bitácora del port
pantalla por pantalla —cada cambio con lo que se midió y por qué se decidió así—,
la regla completa de la bitácora de acceso de la Ley 1581, y lo histórico (el SPA
de Angular y la API Python, las dos retiradas). No hace falta leerlo entero.
