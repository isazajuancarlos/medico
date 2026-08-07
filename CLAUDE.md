# CLAUDE.md — medico

> **Configuración general: `~/.claude/CLAUDE.md`** — reglas que no se rompen,
> las directivas numeradas y las máquinas montadas. Se carga en toda sesión y
> **no se copia aquí**; este archivo lleva SOLO lo propio de medico.
>
> **Esta instancia se arranca desde esta carpeta**: `cd /mnt/data/medico && claude`.
> Desde el 2026-08-01 no se trabajan varios proyectos desde una sola sesión — y
> aquí importa doblemente: `/security-review` construye su diff con el `cwd`.

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> El resto del archivo va en español, como el código, los docstrings y los mensajes
> de commit de este repositorio.

## Qué es esto

Reimplementación de un sistema médico que hoy corre en **Visual FoxPro** en el
consultorio de un cliente real (`/mnt/data/aprograms`, fuente autoritativa;
`/mnt/data/repoNuevo`, el generador de RIPS y epidemiológicos). No es un producto
de cero: cada decisión se contrasta contra lo que el prestador ya usa.

Backend **FastAPI + SQLModel + PostgreSQL** en `backend/`, frontend **Angular 22 +
`@angular/material`** en `frontend/`. Multi-institucional, con licencia por
institución que decide qué módulos se ven.

> **PrimeNG SALIÓ el 2026-08-02** (#255, cerrada). El stack de todos los
> desarrollos es Rust (axum + SQLx/SQLite) con Angular empotrado en el binario y
> **`@angular/material`**; medico era el único que no cumplía y ya cumple.
>
> Medido: **287 → 0** importaciones `from 'primeng/…'` (35 → 0 archivos).
> `primeng` y `@primeuix/themes` salieron del `package.json` y de
> `node_modules`; `providePrimeNG` salió de `app.config.ts`. Las pruebas del
> frontend fueron de **82 → 366** y `ng build` quedó en verde en cada lote.
> **`primeicons` se queda**: es MIT, es solo una fuente de iconos y no tiene el
> problema de licencia de `primeng` — por eso los componentes propios usan
> `<i class="pi pi-…">` y no `<mat-icon>`, que obligaría a empaquetar otra
> fuente.
>
> **Nunca se subió `primeng` a la 22**, y esa es la razón de todo esto: la 22
> dejó de ser MIT —es «PrimeUI», comercial, con clave obligatoria que sin ella
> estampa *«Invalid PrimeUI License»* sobre la aplicación—. Un binario que va
> offline a casa de un cliente no puede llevar dentro una clave que caduca; se
> probó entero en SIGER y se descartó por eso.
>
> **#254 se hizo a continuación, el mismo día**: Angular 22.1.0, Material
> 22.1.0, CDK 22.1.0 y TypeScript 6.0.3, que es lo que corre `informes`. El
> orden fue el decidido: primero Material con Angular 21, después el salto.
> Medido de paso: Angular 22 con PrimeNG 21 *sí* compilaba, así que había atajo
> — no se tomó, porque dejaba una librería corriendo fuera de su rango declarado
> en un producto que se instala offline. Ver [La subida a Angular 22](#la-subida-a-angular-22).
>
> **`--legacy-peer-deps` se fue con PrimeNG**: era su bandera. `npm ci` limpio
> funciona, y así están el CI y `construir.sh`.
>
> **La receta, las equivalencias y las trampas ya pagadas están en
> [`MIGRACION_MATERIAL.md`](MIGRACION_MATERIAL.md)** — leerlo antes de tocar una
> pantalla.
>
> Dos redes, y hacen falta las dos: el **banco de humo**
> (`pantallas-montan.spec.ts`) monta las 30 pantallas, y el **guardián de las
> directivas mudas** (`directivas-de-material-cableadas.spec.ts`) comprueba que
> cada directiva de Material escrita en una plantilla esté en los `imports` de
> **su** componente. Lo segundo el primero no lo ve: `matTooltip` sin
> `MatTooltipModule` es, para Angular, un atributo estático corriente, así que el
> tooltip no existe y **nada falla**. Pasó en cinco pantallas.
>
> Y el guardián **tampoco discriminaba hasta el 2026-08-02**: miraba si el nombre
> del módulo aparecía en el ARCHIVO, no en la lista de `imports`, así que la
> línea `import` de arriba lo daba por cableado — y con los diálogos hechos
> componentes propios, un archivo lleva de dos a cinco y el `imports` de la
> pantalla tapaba el del diálogo. Se comprobó rompiéndolo a propósito (295
> verdes con la lista rota). Hoy corta por `@Component(` y comprueba cada
> componente contra su lista: 156 → 227 comprobaciones, y con la lista rota
> nombra al componente culpable y solo a él.

El eje del trabajo actual es la **cadena RIPS → FEV-RIPS → MUV**: producir el JSON
de la Resolución 948 de 2026 y que el Mecanismo Único de Validación de MinSalud lo
acepte y devuelva CUV. Ver [La cadena RIPS](#la-cadena-rips--fev-rips--muv).

**Y hay un port a Rust en curso, que este archivo no describía.** Ver
[El workspace Rust](#el-workspace-rust).

## Comandos

```bash
# === BACKEND ===
cd backend
.venv/bin/python -m pytest -q                    # 702 pruebas (~2 min 20 s)
.venv/bin/python -m pytest tests/test_cie11.py -q         # un módulo
.venv/bin/python -m pytest tests/test_cie11.py::test_x -q # una prueba
.venv/bin/python -m pytest -k "aislamiento" -q            # por nombre

pip install -r requirements.txt -r requirements-dev.txt   # `pytest` va en el segundo
python seed.py                                   # BD + datos de prueba (idempotente)
uvicorn main:app --reload --port 8000            # Swagger en /docs

# === FRONTEND ===
cd frontend
npm ci                                           # sin banderas: --legacy-peer-deps era por PrimeNG
npm start                                        # :4200, proxy /api → :8000
npm test                                         # vitest vía `ng test`: 12 specs, 367 pruebas
npm run build

# === DOCKER (pila completa) ===
# Los tres secretos se EXIGEN del entorno: sin ellos compose aborta, a propósito.
export POSTGRES_PASSWORD="$(openssl rand -hex 24)"
export SECRET_KEY="$(openssl rand -hex 32)"
export JWT_SECRET_KEY="$(openssl rand -hex 32)"
docker-compose up -d                             # db 5432 + backend 8000 + nginx 80

# === UTILIDADES DE BD ===
cd backend
python importer.py                               # CIE-10, CUPS, EPS, INVIMA
python reset_db.py && python seed.py             # PELIGRO: borra todo
```

**Nunca canalizar `pytest` a `tail`/`head` sin `set -o pipefail`**: el código de
salida se pierde y un fallo se lee como verde. Un hook global lo bloquea. Redirigir
a archivo y comprobar `$?` es la forma limpia.

## El CI — cinco trabajos, y las dependencias NO salen de `requirements.txt`

`.github/workflows/ci.yml`. Cubre **los dos sistemas** más el frontend: probar solo
el Rust dejaría sin red al producto vivo.

| trabajo | comando exacto |
|---|---|
| `rust` | `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace --all-targets` |
| `backend` | `pytest -q` en `backend/`, con `pip install -r requirements-recuperado.txt` |
| `frontend` | `npm ci` + `npx ng build` + `npx ng test --watch=false` |
| `auditoria-deps` | `cargo audit` (RustSec; el único *ignore* vive en `.cargo/audit.toml`) |
| `cadena-suministro` | `cargo deny check` (fuentes, bans, licencias; config en `deny.toml`) |

Para reproducir el verde en local se copian **esos** comandos, no unos parecidos.
Dos trampas medidas:

- **Las dependencias de prueba salen de `requirements-recuperado.txt`, en la RAÍZ**
  —el *freeze* completo del entorno que funciona—, no de `backend/requirements.txt`,
  que no trae `pytest`. Instalar el segundo y ver fallar la colección es el camino
  largo.
- `rust` instala `libsqlite3-dev`: `libsqlite3-sys` 0.30 enlaza el SQLite del
  sistema, no el *bundled*. Sin cabeceras no compila.

Corre en `push` **solo a `main`** y en `pull_request` contra `main`; antes disparaba
los dos eventos sobre el mismo commit y los cinco checks corrían por duplicado, que
en un repositorio privado son minutos facturados. **Consecuencia declarada: una rama
empujada sin PR no tiene CI.**

> **Desde el 2026-07-31 21:35 el CI de GitHub no arranca** —repositorio privado,
> facturación—. Mientras dure, el único verde es el **local con los siete comandos
> de la tabla**, y hay que decirlo al afirmar que algo pasa: `gh run list` no va a
> mostrar nada.

## La subida a Angular 22

Hecha el 2026-08-02, justo después de que saliera PrimeNG (#254). Angular
22.1.0, Material 22.1.0, CDK 22.1.0, TypeScript 6.0.3 — lo mismo que corre
`informes`, que era el objetivo: **una sola versión de Angular en todos los
desarrollos**.

Se hizo con `ng update @angular/core@22 @angular/cli@22` y después
`ng update @angular/material@22` (exige el árbol limpio entre una y otra).
Material no tocó **una sola plantilla**: el vocabulario de la 21 y el de la 22
son el mismo, que era la apuesta de hacer #255 primero.

**Las migraciones de la 22 escriben tres opt-out, y los tres hay que leerlos**,
porque los tres dicen «quédate como estabas»:

| lo que cambió la 22 | lo que escribió la migración | qué se hizo |
|---|---|---|
| **OnPush pasa a ser el defecto** (`Default` queda deprecado como alias de `Eager`) | `changeDetection: ChangeDetectionStrategy.Eager` en 71 componentes | **se deja**, ver abajo |
| El cliente HTTP pasa a `fetch` | `provideHttpClient(withXhr(), …)` | **se deja hasta comprobarlo contra el servidor real** |
| Dos diagnósticos nuevos de plantilla | `extendedDiagnostics: suppress` en los dos tsconfig | **se quitó**, y se arregló la causa |

**El silenciador se quitó porque tapaba UN aviso, y era verdadero.** Sin él, el
proyecto entero produce un solo aviso: `config()!.dias_atencion?.includes(…)` en
`admin`, diciendo que el `?.` sobra porque el tipo no admite nulo. La respuesta
no era quitar el `?.`: **el tipo mentía**. La columna del servidor es un JSON sin
`NOT NULL`, así que una fila anterior a que el campo existiera lo trae vacío — y
el código ya se defendía dos veces a mano. Hoy `dias_atencion: string[] | null`
dice la verdad, las dos defensas quedan justificadas y el aviso desaparece solo.
Silenciarlo habría dejado el tipo mintiendo y las defensas pareciendo
redundantes: el siguiente que pasara las habría borrado.

**`Eager` se quitó el 2026-08-02 (#256), y primero se construyó la red.** En la
22 OnPush es el defecto; adoptarlo exige que ningún campo que la plantilla lee
se escriba desde un callback asíncrono sin señal de por medio, y el fallo de esa
auditoría es una pantalla que no se refresca **sin que nada falle**. Ver
[La tercera red](#la-tercera-red-el-estado-que-la-plantilla-lee-es-señal).

**El `--legacy-peer-deps` se fue con PrimeNG.** Era su bandera: `npm ci` limpio
funciona desde que la librería no está. Quitado del CI, de `construir.sh`, del
README y de aquí.

### La tercera red: el estado que la plantilla lee es señal

`src/app/estado-que-la-plantilla-lee-es-señal.spec.ts`. La regla, en una frase:
**si la plantilla lo lee y algo asíncrono lo escribe, tiene que ser señal.**

Existe porque las otras dos redes no llegan. Un componente OnPush solo se
repinta cuando algo lo marca sucio: una señal que cambió, un evento de su propia
vista, o un `markForCheck()`. Un campo llano escrito desde un `subscribe` no
marca nada — y el fallo no se parece a un fallo: la petición responde, el campo
cambia, la memoria tiene el dato bueno y **la pantalla sigue mostrando lo
anterior**. `ng build` pasa; el banco de humo monta la pantalla igual, porque
monta con la pantalla VACÍA, que es antes de que llegue la respuesta.

Dos decisiones de diseño, y las dos vienen de un error propio:

- **Sigue las llamadas, no solo el cuerpo del callback.** La primera versión
  miraba únicamente lo que hay dentro del `subscribe` y se dejó fuera
  `historias`, donde el callback llama a `_loadAnt(...)` y es ESE método el que
  escribe los cuatro objetos de antecedentes. Hoy recorre transitivamente todo
  método alcanzable desde una vía asíncrona. Un guardián que mira un solo nivel
  da un verde que no significa nada.
- **Lee las EXPRESIONES de la plantilla, no su texto.** Buscar el nombre del
  campo en el HTML da falsos positivos absurdos: un campo llamado `form`
  aparece dentro de `mat-form-field` en toda pantalla con un formulario
  (directiva 23).

**Discrimina, comprobado**: se volvió a poner `errorMessage` como campo llano en
`login` y el guardián nombró a `LoginComponent` y solo a él.

Lo que encontró al escribirlo: **23 campos en 7 archivos** con el primer
recorrido, y **35 en 6** con el transitivo. Entre ellos, el mensaje de error del
acceso (un login fallido no habría mostrado nada), las observaciones de un
resultado de laboratorio (habrían quedado las del examen ANTERIOR) y los cuatro
objetos de antecedentes de la historia clínica (los del paciente anterior).

**Y el cambio se notó en las pruebas**, que es la señal de que no era teórico:
dos que estaban en verde se pusieron rojas al quitar `Eager`, y las dos estaban
mal escritas — asignaban un `@Input` a pelo (`barra.modo = …`) en vez de usar
`fixture.componentRef.setInput`, que es lo que hace un padre y lo único que
marca sucio un OnPush. Pasaban midiendo el estado anterior.

### La fuente que solo se vio abriendo la aplicación

Con las 366 pruebas en verde y el binario construido, se abrió el producto en un
navegador — y los campos, los botones y los diálogos de Material salían **con
serifas**, mientras el resto de la página iba en Inter.

La causa: el CSS precompilado `prebuilt-themes/azure-blue.css` trae **30 tokens
`--mat-sys-*` con `Roboto` dentro**, y Roboto no viaja en el binario. La única
fuente que empaquetamos es Inter (`@fontsource`). En un equipo con internet no
se nota; en el portátil offline de un consultorio, el navegador cae a su fuente
por defecto.

El arreglo es generar el tema con la API Sass en vez de usar el precompilado
—misma paleta `azure`, así que los colores no cambian— y fijar la familia una
sola vez:

```scss
:root {
  color-scheme: light;
  @include mat.theme((color: mat.$azure-palette,
                      typography: (plain-family: (Inter, …), brand-family: (Inter, …)),
                      density: 0));
}
```

`color-scheme: light` va porque `mat.theme` emite los colores con `light-dark()`
y por tanto seguiría el modo del SISTEMA, cosa que el precompilado no hacía: un
equipo en modo oscuro habría pintado la página clara con los controles oscuros.
Se conserva lo de antes; el modo oscuro, si algún día se hace, se decide.

Medido después: **cero** apariciones de `Roboto` en el CSS producido, y
`getComputedStyle` sobre un `mat-label` y un `input` del binario real devuelve
Inter donde antes devolvía Roboto.

**La lección, que es la de la directiva 28**: 366 pruebas en verde, `ng build`
en verde, el guardián de las directivas en verde — y el producto se veía mal. Lo
que ninguna de las redes mira es cómo QUEDA la pantalla.

## La demostración pública (medico.xiliux.com)

Desplegada el 2026-08-02. Es una **demostración**, no la instalación de un
cliente: sus credenciales están publicadas en la propia pantalla de acceso y
cualquiera puede escribir en su base.

```
https://medico.xiliux.com        demo@xiliux.com / demo-medico-2026
```

| pieza | dónde |
|---|---|
| binario y base | `/opt/medico-demo/` (usuario `medico`) |
| entorno | `/etc/medico-demo/medico.env` (root:medico 0640) |
| servicio | `medico-demo.service`, escucha en `127.0.0.1:8010` |
| sitio | `/etc/nginx/sites-available/medico.xiliux.com` |
| guiones | `despliegue/desplegar-demo.sh` y `despliegue/publicar-demo.sh` |

**El modo demostración se enciende con DOS variables y ninguna otra cosa**:
`MEDICO_DEMO_CORREO` y `MEDICO_DEMO_CLAVE`. Hacen dos trabajos a la vez, y ese
es el diseño: siembran el usuario (`medico-api sembrar-demo`) y son lo que
`GET /api/demo` publica para que la pantalla lo enseñe. Así **no puede
anunciarse una credencial distinta de la que existe**. Media configuración —una
sola de las dos— **no arranca** (directiva 20): publicar credenciales es una
decisión, y a medias no se sabe cuál se tomó.

Sin esas variables, `/api/demo` responde **204** y la pantalla no pinta nada. Es
lo que le pasa a la instalación de un cliente, y por eso **no hay nada que
acordarse de apagar**. El arranque además grita en el log cuando el modo está
encendido.

La licencia de la demo se fija con **todo el catálogo** (`modulo::CATALOGO`, 46
módulos), no con una lista escrita a mano que se desincronizaría al primer
módulo nuevo: cada uno aparece como `habilitado` o `pendiente` según lo que el
binario sirva de verdad, que es justo lo que la demostración debe enseñar.

**Dos cosas medidas al desplegar, para que no sorprendan:**

- `/api/acceso` responde **503 si se llama directo al puerto 8010**, y es
  correcto: el entorno declara `MEDICO_TRAS_PROXY=true` y ahí no llega
  `X-Real-IP`. El freno al ensayo de contraseñas prefiere negarse antes que
  meter a todos en el mismo cubo. Por nginx llega y responde 200.
- El **panel de inicio salía vacío** porque `/api/estadisticas/kpis` daba 404.
  **Portado el 2026-08-06**, con los otros siete paneles: ya responde. Lo que
  sigue en 404 desde la demo son `admin` y `quirofano`, y esos sí se ven.

## El workspace Rust

Tres crates —`medico-dominio`, `medico-datos`, `medico-api`— con axum y SQLx sobre
SQLite, y la interfaz Angular **empotrada en el binario** con `rust-embed`. Es el
mismo modelo que informes: se entrega **un archivo**.

```bash
./construir.sh                    # interfaz → empotrado → binario. EL comando.
cargo clippy --workspace --all-targets -- -D warnings   # lo que corre el CI
cargo test  --workspace --all-targets                   # 750 pruebas (~2 min 30 s)
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

**La zona horaria es la del PRESTADOR, nunca la del reloj de la máquina**
(`medico_dominio::zona`, desde el 2026-08-03). Cuatro sitios preguntaban la
fecha con `chrono::Local` —la hora que el RIPS reporta, los límites del cierre
del mes, la edad del paciente y el «hoy» que juzga si una licencia venció—, y
`Local` es la zona del sistema operativo. En el portátil de un consultorio de
Medellín daba lo correcto por casualidad; el VPS de la demostración corre en
`Etc/UTC`, así que allí **toda atención se reportaba cinco horas más tarde de la
que ocurrió**, con el archivo aceptándose igual. Lo destapó una prueba que
fallaba solo en el CI.

Se fija una vez al arrancar (`MEDICO_ZONA`, defecto `America/Bogota`) y el log
la dice **siempre**, esté puesta o no. Un nombre que no exista en la base IANA
**impide arrancar**: no se cae a la del sistema ni al defecto. El defecto no es
relleno —esto emite RIPS a MinSalud y Colombia tiene una sola zona desde 1993—,
y no hay columna por institución a propósito: tres de los cuatro sitios no
tienen institución a la que preguntarle, y una columna sin pantalla que la edite
(`admin` no está portada) sería una característica falsa. `config.rs` la fija;
`zona::hoy()`, `desde_utc()` y `a_utc()` la usan.

**Freno al ensayo de contraseñas** (`medico_datos::freno`, siempre activo): cinco
fallos seguidos del mismo origen contra el mismo correo se responden normal; a
partir de ahí el intento siguiente recibe **429 con `Retry-After`**, empezando en
15 s, doblando y topando en 5 minutos. Es un freno, **no un bloqueo**: se agota
solo y nadie desbloquea a nadie — un bloqueo convierte cinco peticiones de un
desconocido en una historia clínica inaccesible de madrugada.

La clave es **(correo, origen)** y no solo el correo, para que quien insiste se
frene a sí mismo en vez de dejar fuera al dueño de la cuenta. Un correo que no
existe se frena igual: si solo se frenaran las cuentas reales, recibir el 429
diría qué correos existen.

El origen es **`X-Real-IP`** y solo con `MEDICO_TRAS_PROXY=true`; sin proxy, la
dirección del zócalo. **No `X-Forwarded-For`**: nuestro nginx la escribe con
`$proxy_add_x_forwarded_for`, que AÑADE, así que su primer salto es el valor que
puso el cliente — leerlo dejaba elegir cubo en cada petición y el freno no
frenaba nada en la única postura para la que se escribió. `X-Real-IP` sí la pisa
nginx con `$remote_addr`. Si se declara el proxy y esa cabecera no llega, el
acceso responde **503**: rellenar el origen pone a todos en el mismo cubo, que es
el bloqueo por cuenta que este freno existe para no tener.

La REGLA (umbral, techo, qué borra la cuenta, marca ilegible) vive en
`guaca::freno`, compartida con informes; `medico-datos/src/freno.rs` solo pone
las dos consultas.

Y la bandera arrastra **dónde escucha el binario** (`MEDICO_ESCUCHA`): sin valor,
`127.0.0.1` si se declaró el proxy y `0.0.0.0` si no. Declarar el proxy y seguir
abierto a todas las interfaces deja una puerta por detrás de él — quien alcance el
puerto elige su propio `X-Real-IP` y el freno no engancha. Es un defecto, no una
regla: con nginx en otra máquina o en otro contenedor se pone a mano.

Y `autenticar` verifica contra un **hash de descarte** cuando el usuario no
existe o está inactivo: sin eso respondía al instante con un correo falso y tras
~100 ms con uno real, y esa diferencia se mide con un bucle de `curl`.

Para arrancarlo hace falta `SECRET_KEY` (≥ 32 bytes, sin valores conocidos); el
binario **no arranca sin secreto**, a propósito. `DATABASE_URL` sí tiene defecto
—`sqlite://medico.db`—, que es lo contrario de un secreto: no hay dato falso, hay
un archivo local. Y abre la base
relativa a su directorio de trabajo: arrancarlo desde otro sitio crea una base
vacía y el acceso responde «credenciales incorrectas», que es indistinguible de
«esta base no tiene a nadie». Ya costó una tarde.

**Lo que el port todavía NO cubre**, para no confundir avance con paridad.

**Cómo se cuenta, corregido el 2026-08-06.** Hasta esa fecha la lista salía de
comparar los PREFIJOS de `rutas.rs` contra los de `backend/routers/*.py`, y esa
cuenta miente en las dos direcciones: suma como trabajo lo que solo cambió de
nombre (`auth` → `acceso`/`salir`/`yo`, `superadmin` → `/api/instituciones`, los
cuatro documentos clínicos → `/api/documentos`) y no ve un verbo suelto sin
servir dentro de un módulo que sí existe. Se cuenta contra lo que la INTERFAZ
pide, que es el único criterio que es una prueba: se resuelven las llamadas
`http.<verbo>` de `frontend/src/**/*.ts` contra las constantes `API` de cada
servicio y se cruzan con las rutas del `Router::new()`.

Los dos extractores llevan control positivo, y hace falta: la primera versión
leía el `.route(` solo hasta el final de la línea —así que era ciega a
`get(x).put(y).delete(z)` partido en varias— y cortaba el genérico de TypeScript
en el primer `;`, con lo que se le escapó `GET /api/admin/auditoria`. Un fallo
contaba de más y el otro de menos.

```
medido el 2026-08-06:  229/229 llamadas del frontal resueltas (0 sin resolver)
                       145 llamadas .route() leídas
                       el frontal pide 197 endpoints distintos
                       el binario Rust sirve  193
                       FALTA UN SOLO MÓDULO:

  admin      26 endpoints — usuarios, institución, configuración, política de
                            seguridad, entidades, especialidades, campos
                            dinámicos, capacidades y auditoría

  PORTADOS y ya fuera de la lista:
    facturacion   (17)  2026-08-05
    consentimiento (5)  2026-08-05
    estadisticas   (8)  2026-08-06 — los ocho paneles, con el aislamiento que
                        en el Python vivo fallaba ABIERTO: `/financiero` sumaba
                        el dinero de TODOS los prestadores
    quirofano      (9)  2026-08-06 — con la máquina de estados en el dominio y
                        el aislamiento que el Python no tenía: su listado
                        cruzaba instituciones con el DOCUMENTO del paciente
    admin         (26)  2026-08-06 — el de más privilegio: crea gente, le cambia
                        el rol y le restablece la contraseña. Con las tres
                        reglas que el Python tuvo abiertas por la mitad —solo un
                        superadmin fabrica otro superadmin, el cupo se consume
                        también al REACTIVAR, y el alta pasa por la licencia—,
                        cada una con su prueba HTTP por los DOS caminos.
                        Y con las TRES que le faltaban, ver abajo

a medias (1):
  rips — LOS CINCO desde el 2026-08-04. Falta el FEV-RIPS, que es
         la fusión de los cinco en un documento único, no un sexto
```

### Las tres que le faltaban a `/api/admin` (2026-08-06)

Las encontró la revisión de seguridad del PR #49 sobre `39375c3`, con las 760
pruebas en verde y sin que ninguna las viera. Las tres son de la misma familia:
**el mecanismo existía y no estaba enchufado.**

1. **El mínimo de contraseña no existía, y la rama que lo decía estaba MUERTA.**
   `claves::hashear` devuelve `None` solo si Argon2 falla, y Argon2 hashea `""`
   sin queja — así que los dos `else { 400, "la contraseña no cumple el mínimo
   exigido" }` no se emitían jamás. Medido: `{"nueva_password":""}` → **204**, y
   después se entraba con `""`. El admin de la IPS pulsaba «restablecer» con el
   campo en blanco y dejaba abierta una cuenta que **firma** historia clínica
   (Ley 527/1999). Era **regresión**: el Python vivo sí lo comprueba
   (`backend/routers/admin.py:400`, `len < 6`).

   La regla vive ahora en `administracion::password_admisible`, y **contra la
   política de la institución**, no como un `if` en cada ruta. Eso arregla de
   paso la otra mitad: las 21 columnas de `politica_seguridad` eran de solo
   escritura —el admin configuraba una postura que el binario no aplicaba en
   ninguna parte—, y ya se aplican `longitud_minima_password` y los cuatro
   `requiere_*` de composición. El suelo es **8**, y no es un defecto inventado
   (directiva 20): es el mismo número del `CHECK (… BETWEEN 8 AND 128)` del
   esquema. Las demás columnas siguen sin lector; están enumeradas en la tarea.

   **El orden es parte de la regla**: `politica_para_administrar` cruza la puerta
   del ámbito ANTES de mirar la contraseña. Al revés, el mensaje «la política de
   la institución exige una mayúscula» se respondería sobre un id que debía dar
   404, y con eso se enumera la plantilla del vecino probando números — la misma
   razón por la que aquí es 404 y nunca 403.

2. **Las siete rutas de maestros no tenían puerta de ámbito.** Un `admin` con
   `institucion_id = NULL` —que un superadmin crea por `POST /api/usuarios` sin
   dar institución— tiene `Ambito::Ninguno`, no ve una sola fila de nadie, y
   alcanzaba `especialidades` y `entidades`: **200** al listar y **201** al
   escribir, mientras `/api/admin/institucion` y `/api/capacidades` le
   respondían 403. `PUT /api/admin/entidades/{id}` reescribe el `codigo_eps`,
   que viaja al RIPS de TODAS las instituciones.

   Lo cierra `puede_administrar_maestros`, que es **función propia y no una
   cuarta copia** de `institucion_objetivo`: un maestro es global, así que no
   hay institución que elegir y lo único exigible es pertenecer a alguna. Es la
   tercera vez que aparece esta forma (`institucion_objetivo`,
   `capacidades::institucion_de`, `modulos::institucion_de`).

3. **Ninguna de las 24 rutas `/api/admin` dejaba línea en la bitácora.** Cero,
   medido, mientras `PUT /api/capacidades` sí escribía en esa MISMA tabla y
   `adm_auditoria` la sirve como la pantalla de auditoría de la institución. O
   sea que esa pantalla no podía enseñar jamás quién le cambió los permisos a
   quién, ni quién restableció la contraseña de un médico.

   Lo pone `rutas::anotar_acto_admin`, **antes de aplicar y obligatorio** —el
   mismo criterio, y el mismo texto, que `capacidades::registrar_cambio` ya
   había tomado: aquí no hay paciente esperando, y si no se puede registrar, no
   se aplica—. Va como `Coleccion`, porque no hay UN paciente. **Los listados no
   anotan**, a propósito: los pide cada pintado de pantalla y ahogarían la señal.

   Dos actos llevan su detalle porque **borran evidencia y su nombre no lo
   dice**: `restablecer_password` llama a `quitar_freno` por dentro, y
   `quitar_freno` borra `intento_acceso` de todos los orígenes de ese correo,
   que es el único registro de un ensayo de contraseñas.

Las sujetan tres pruebas HTTP y siete unitarias, **todas con su pareja**, y las
tres pasaron el banco de mutación.

### Y lo que la misma revisión dejó por debajo del listón (2026-08-06)

Cuatro observaciones y una regresión comercial, hechas en una segunda pasada.

**El cupo de MÉDICOS de la licencia, que no existía.** La tabla `licencia` tenía
una sola cota, `max_usuarios`, y el Python vivo tiene dos (`core/deps.py:337`,
`hay_cupo(..., para_medico=True)`): una IPS con plan de 3 médicos podía dar de
alta 20. Migración `0049`, columna **anulable y sin `DEFAULT`** —`NULL` es «este
plan no pactó cupo de médicos», nunca cero, porque las licencias ya firmadas no
lo pactaron y ponerles un número sería inventar una cláusula—.

**La pieza que casi sale mal es `licencia::Asiento { de_usuario, de_medico }`.**
La primera versión pasaba un solo booleano, y con eso un **ascenso** exigía
además asiento de usuario: una IPS con el cupo justo no habría podido ascender a
nadie que ya estuviera contado. Son dos magnitudes (directiva 32):

| operación | asiento de usuario | asiento de médico |
|---|---|---|
| alta de una recepcionista | sí | no |
| alta de un médico | sí | **sí** |
| reactivar a quien se fue | sí | según su rol |
| **ascender a médico a quien ya está activo** | **no** | **sí** |

Y el ascenso se cobra, que es la otra mitad que el Python sí tenía: sin ella el
tope se rodea creando auxiliares y ascendiéndolos después, porque el `PUT` no
toca `activo` y `reactiva` no lo veía. `Puerta::CupoDeMedicosLleno` es variante
propia —motivo `cupo_de_medicos_lleno`— y no un matiz de `cupo_lleno`: a quien lo
recibe todavía le caben auxiliares, y un solo motivo le haría ampliar el plan
equivocado.

**`GestionarUsuarios` era la RAÍZ y la matriz no lo decía.** Un laboratorista con
esa sola casilla se concedía `firmar_clinico` y firmaba una historia. Ahora
`capacidades::actualizar` rechaza con 403 conceder lo que el rol de quien concede
no trae **por defecto nacional** — y esa elección es la regla: contra la matriz
EFECTIVA, un admin que se hubiera revocado una capacidad no podría devolvérsela
nunca. El bloque va **después** de `validar`, para no adelantarse al 422 más
específico de `configurable_por_institucion`; adelantarlo puso en rojo a
`no_se_concede_la_gestion_de_instituciones_ni_se_toca_al_superadmin`, y la prueba
tenía razón.

**El filtro del ámbito, DENTRO del `UPDATE`.** Las tres escrituras de personal
comprobaban con `del_ambito` y después escribían con `WHERE id = ?` a secas,
contra la regla que este repositorio ya tiene escrita en
`usuario::fijar_documento`. No era explotable —ninguna ruta muda una fila de
institución— así que `cola_del_ambito` es segunda vuelta de llave, con la
exclusión del superadmin **por rol** y no por institución.

**Y nace `POST /api/mi-clave`, porque nadie podía cambiar su propia contraseña.**
Ese hueco tenía dos mitades que se tapaban entre sí: la falta de ruta, y
`requiere_cambio_password` —que el restablecimiento del admin SÍ escribe— siendo
una bandera imposible de satisfacer. Cuatro reglas: exige la contraseña actual
(sin ella, una sesión robada se queda con la cuenta), pasa la nueva por la misma
`password_admisible`, sube `token_version` en la misma sentencia y **reemite la
cookie** — las dos últimas juntas son la diferencia entre «cerré todas las
sesiones» y «cerré todas menos la mía». Está en `ABIERTAS_A_PROPOSITO` del
guardián de escrituras con su motivo: escribe la fila propia, y la contraseña
actual es una prueba más fuerte que cualquier capacidad.

**`politica_seguridad` sigue a medias, y ahora lo dice.** De sus 21 columnas se
aplican cinco. El doc-comment de `Politica` lleva la tabla entera con cuáles no
se aplican y por qué — y **tres no se aplicarán nunca**: `max_intentos_login` y
`tiempo_bloqueo_minutos`, porque este binario decidió freno-que-se-agota en vez
de bloqueo, y `retener_logs_dias`, porque la bitácora está encadenada por hash y
podarla la rompe. Lo que queda pendiente es que la pantalla deje de presentarlas
como si mandaran.

**`/api/admin/capacidades` no existía en NINGUNA de las dos pilas** y el frontal
la llamaba: la matriz de capacidades vive en `/api/capacidades` desde #235, con
su banco de pruebas. Se corrigió el cliente, no se duplicó la ruta.

**Lo que NO se portó, y es una decisión escrita**: `cuenta_bloqueada` y su
endpoint de desbloqueo. Este binario decidió que el freno al ensayo de
contraseñas fuera una ESPERA que se dobla y no un bloqueo —«un bloqueo que exige
a alguien desbloquear convierte cinco peticiones de un desconocido en una
historia clínica inaccesible a las tres de la mañana»—, así que la columna sería
un segundo mecanismo para lo mismo. `cuenta_bloqueada` se **deriva** del freno y
«desbloquear» borra los intentos de ese correo, que es lo que hace un acierto.

**Ya no hay «routers de Python sin cliente» que apuntar como huecos**:
`superadmin`, `epicrisis`, `evoluciones`, `interconsultas` y `nota-operatoria`
tienen su pantalla funcionando contra el binario por otra ruta —las cuatro
clínicas hablan con `/api/documentos` y superadmin con `/api/instituciones`— y
desde el 2026-08-06 el catálogo de módulos lo DICE: los cinco declaran su
`prefijo`, así que dejan de anunciarse como pendientes a un cliente que los tiene
comprados. Lo vigila `ningun_modulo_construido_se_anuncia_como_pendiente`.

**`/api/rips` ya no se niega, y esa es la novedad.** Emite **los cinco**
documentos —AC, AP, AM, AU y AH—, así que `sin_portar` va vacío y **el 409 se
quitó**: existía para no entregar un archivo al que le faltaban documentos, y ya
no falta ninguno. Dejarlo habría sido pedir una bandera que no significa nada, y
una bandera que no significa nada se pone sin leerla — que es como deja de
protegerse la siguiente que sí importa (`permitir_supuestos`, que sigue en pie).
El parámetro `parcial` se acepta y se ignora, para no romperle la llamada a quien
la tuviera escrita. Un archivo sin las hospitalizaciones
**tiene la forma de un RIPS completo**: se sube, el
validador lo acepta, y lo que falta no aparece por ningún lado — es sub-reporte,
o sea dinero que no se cobra e información incompleta ante la Supersalud. Por eso
responde **409** salvo que se pida `parcial=true`, igual que
`permitir_supuestos`: el camino permisivo existe, lo pide el llamante, y la
respuesta lleva siempre `sin_portar` con lo que falta. El RIPS de verdad se sigue
generando con el Python.



### AP y AM (2026-08-04)

`medico-dominio/src/rips/ap.rs` y `am.rs`, con sus consultas en
`medico-datos/src/rips.rs` y la migración `0041`. Lo que hay que saber:

- **El periodo lo fija la fecha de la HISTORIA**, no la de la orden ni la de la
  fórmula. Una orden escrita al día siguiente pertenece a la atención en que se
  ordenó; filtrar por su propia fecha partiría la misma atención en dos facturas.
- **La `0041` añade trece columnas que hoy no escribe ninguna pantalla**
  (`via_ingreso_servicio`, `grupo_servicios`, `cod_servicio` y el bloque
  económico en `orden`; eso más `tipo_medicamento` y `valor_unitario` en
  `formula`). Todas NULL y **ninguna con `DEFAULT`**, que es la lección de la
  `0038`: un defecto haría que el recolector —que solo ve AUSENCIAS— no tuviera
  nada que anotar. Son las mismas de `CamposRips` en `backend/models.py`.
  Consecuencia hoy: **el AP y el AM salen con faltas** hasta que alguien las
  capture o las llene el tarifador, y esa es la conducta correcta.
- **El CUM se valida contra el catálogo del INVIMA**, no solo «que no esté
  vacío». Van los 65 997 códigos empotrados en
  `medico-dominio/datos/cum_codigos.txt` (710 KB), que genera
  `herramientas/generar_cum_para_rust.py` **del mismo `cum.json.gz` que lee el
  Python**; `backend/tests/test_cum_paridad_con_rust.py` se pone rojo si las dos
  copias se separan (comprobado quitándole un código).
- **`unidadMinDispensa` (M13) sale de `REFs_UPR`** (`muv_upr.json`, 73
  presentaciones). Se tolera el plural y las mayúsculas; lo que **no** se hace es
  aproximar a la más parecida. Es el campo donde ya se coló un dato falso —
  `tableta = 1`, cuando el 1 del catálogo es AMPOLLA— porque el MUV lo valida por
  rango.
- **La cantidad no se trunca**: `2.5` sale tal cual y se anota. Truncarla a `2`
  perdería media dosis en silencio.
- **Y se corrigió un fallo del Python al portarlo**: los seis campos CIE-11 del
  AP salían **siempre vacíos**, porque se derivaban de
  `getattr(orden, 'diagnostico_principal', None)` y `Orden` no tiene ese campo —
  los diagnósticos viven en la historia. El archivo decía el diagnóstico en un
  campo (el CIE-10, que sí sale de la historia) y lo callaba en el de al lado.
  Arreglado en los dos sistemas en la misma pasada, para que sigan pudiendo
  compararse byte a byte.
- **La bitácora de la Ley 1581 saca los pacientes de TODOS los documentos.**
  Antes salían solo del AC, y alguien que en el periodo únicamente tuvo una
  fórmula, un laboratorio o una atención de urgencias iba en el archivo sin
  dejar rastro.

### AH (2026-08-04) — dos mapas más, y estos SÍ emitían

`medico-dominio/src/rips/ah.rs`, `hospitalizaciones_del_periodo` y la migración
`0043`. Cierra los cinco documentos.

**El AU tuvo suerte: cuatro de sus seis claves no casaban y caían como falta.
Aquí las claves SÍ casan** —`via_ingreso` y `estado` son texto en mayúsculas—,
así que los códigos equivocados llegaban al archivo:

| campo | emitía | que en el catálogo es |
|---|---|---|
| vía `URGENCIAS` | `02` | DERIVADO DE CONSULTA EXTERNA (urgencias es el **03**) |
| vía `PROGRAMADA` | `03` | DERIVADO DE URGENCIAS (programada es el **02**) |
| vía `REMISION` | `04` | DERIVADO DE HOSPITALIZACION — **`AplicaAH = NO`** |
| egreso `REMISION` | `03` | DERIVADO A OTRO SERVICIO — **`AplicaAH = NO`** |
| egreso `DEFUNCION` | `04` | REFERIDO A OTRA INSTITUCION (muerto es el **02**) |

Las dos vías estaban **intercambiadas**: quien entró por urgencias se reportaba
como ingreso programado y al revés. Dos códigos no aplican al AH y hacen que el
MUV rechace el archivo entero. Y la defunción, otra vez, reportada como traslado.

El maestro de vías (`REFs_ViaIngresoServicioSalud`) **ya estaba extraído desde
julio** en `backend/datos/maestros_muv/via_ingreso.json` y nadie lo había cruzado
con el mapa. Hoy `muv_via_ingreso.json` va también en el binario.

- **`PROGRAMADA` se deriva al 02** porque un ingreso programado se origina en la
  consulta externa; el catálogo no tiene «programada». Es derivación, no
  parecido: las otras trece filas nombran orígenes que este sistema no distingue.
- **Las hospitalizaciones abiertas no entran**, y se exigen **las dos** señales
  —`fecha_egreso IS NOT NULL` y `estado <> 'ACTIVA'`—, porque en una base real se
  desincronizan. El código viejo sabía que no debían reportarse (su mapa tenía un
  `ACTIVA: "00"` con el comentario puesto) y las reportaba igual, con el egreso
  en el instante de la generación.
- **Ni profesional ni bloque económico**, igual que el AU. Y **no lleva
  `diasEstancia`**: la 948 no tiene ese campo, la estancia se deduce de las dos
  fechas.
- **Los dos campos nuevos ya se capturan**: `causa_motivo` en
  `POST /api/hospitalizacion/ingresar` —iba quemada en «13», y con eso ninguna
  hospitalización por accidente de tránsito se cobraba al SOAT— y
  `diagnostico_causa_muerte` en el egreso, con `COALESCE`.

Lo ata `test_condicion_destino_au.py`, que ahora mide los **cuatro** mapas contra
los maestros y contra Rust (con los mapas viejos: 5 en rojo).

### AU (2026-08-04) — y el código que estaba cruzado

`medico-dominio/src/rips/au.rs`, con `urgencias_del_periodo` y la migración
`0042`. Dos cosas propias del AU, y una que **estaba mal en el sistema vivo**:

- **No lleva documento del profesional ni bloque económico.** El MUV lo dice
  literal: «Propiedades no permitidas en la estructura de urgencias:
  tipoDocumentoIdentificacion, numDocumentoIdentificacion». Tampoco
  `vrServicio`, `conceptoRecaudo`, `valorPagoModerador` ni `numFEVPagoModerador`.
  **Un campo que SOBRA hace rechazar el archivo entero**, igual que uno que
  falta, y por eso `FuenteUrgencia` ni siquiera trae profesional: pedirlo en la
  consulta habría sido la manera de que acabara en el JSON.
- **Las atenciones sin egreso no entran.** No han terminado y no se pueden
  facturar; antes se les ponía la hora de la generación, o sea que un paciente
  todavía en urgencias se reportaba como dado de alta en el instante en que
  alguien pidió el archivo.
- **`condicionDestinoUsuarioEgreso` salía con el código equivocado.** La 948 no
  trae esa tabla —su ficha remite a `web.sispro.gov.co`—, así que el Python
  llevaba un mapa **escrito a mano**. Se sacó del contenedor del validador
  (`Rips.REFs_CondicionDestino`, ocho filas, ahora en `condicion_destino.json` y
  `muv_condicion_destino.json`) y el mapa estaba mal de dos maneras a la vez:

  | destino que guarda el sistema | emitía | que en el catálogo es |
  |---|---|---|
  | `Domicilio`, `Hospitalización`, `Remisión`, `Observación` | *nada* (falta → con supuestos, `01`) | — |
  | `Fuga` | `05` | CONTRAREFERIDO A OTRA INSTITUCION |
  | **`Fallecido`** | **`04`** | **REFERIDO A OTRA INSTITUCION** |

  Sus claves (`ALTA`, `HOSPITALIZADO`, `REMITIDO`, `OBSERVACION`) no son las de
  `DestinoSalidaUrgencias`, así que cuatro de seis no casaban con nada; y los dos
  que casaban emitían otro código. **Una muerte en urgencias se reportaba como un
  traslado**, y con eso se caen las dos validaciones que la norma cuelga de ese
  campo (que no haya servicios con fecha posterior a la muerte, y que se informe
  la causa básica). Corregido en los dos sistemas, con
  `backend/tests/test_condicion_destino_au.py` midiendo el mapa contra el ENUM y
  contra el MAESTRO — las dos mitades por las que se rompió (comprobado: con el
  mapa viejo, 7 de 10 en rojo).

- **La fuga no se traduce, a propósito.** El catálogo no tiene código para «el
  paciente se fue sin que nadie le diera salida»; el más parecido por su forma
  es el `01`, y el `01` afirma que salió con destino a su domicilio, o sea que
  hubo un alta. Se anota como falta con su motivo. **Pregunta abierta para el
  prestador**: qué reporta hoy en ese caso.

- **Y los cuatro campos del AU ya se pueden capturar** — que es la otra mitad, y
  la que faltó en P11/P12 durante meses. `causa_motivo` entra por
  `POST /api/urgencias/triage` (se sabe al llegar: accidente de trabajo, de
  tránsito o agresión son causas distintas con pagadores distintos) y los tres
  diagnósticos por `PUT /api/urgencias/{id}/egreso`, porque los pone el médico y
  este módulo no tiene paso de «atender» — el egreso es el único momento en que
  un médico toca la fila. **El egreso usa `COALESCE`**: lo que el cuerpo omite no
  borra lo que ya hubiera. Sin esto, `codDiagnosticoPrincipal` —obligatorio— y la
  causa básica de muerte no se podían llenar desde el producto, y el AU habría
  salido con cuatro faltas por atención para siempre.

  Lo cierra `lo_que_se_captura_en_urgencias_llega_entero_al_au`, que hace triage
  y egreso **por la API** y luego pide el RIPS. No es adorno: encontró un `bind`
  que faltaba en el `INSERT` del triage, o sea la columna escrita en el SQL y el
  valor nunca enviado.

**Son dos medidas distintas y hay que decir cuál se usa.** La ficha #236 mide lo
primero —cruce de los `/api/…` que piden los `.ts` contra los `.route(` de
`rutas.rs`— porque lo que le duele a un cliente es una página muerta. La lista
completa mide lo segundo, que es lo que decide cuándo se puede apagar Python.
Dar seis por «lo que falta del port» es quedarse corto; dar once por «pantallas
rotas», pasarse.

**FEV-RIPS es el que decide**: sin él no hay facturación electrónica, que es por
lo que le pagan al prestador, y el ADR no autoriza apagar Python hasta que la
generación del RIPS sea byte a byte idéntica. En Rust están el recolector de datos
faltantes y los CINCO documentos (`medico-dominio/src/rips/`); el FEV-RIPS
—el documento ÚNICO que funde los cinco— sigue siendo solo del Python.

`/api/modulos` y `/api/agenda/*` **sí existen ya en Rust** (`rutas.rs:236` y
`rutas.rs:260`…), así que la pantalla de Agenda Médica carga contra el binario —
lo contrario estuvo escrito aquí hasta el 2026-08-01 y era falso.

**El workspace Rust está en `main`**, no en una rama aparte: `crates/`, `Cargo.toml`,
`construir.sh` y `deny.toml` van versionados ahí junto al backend Python. Los dos
sistemas conviven en el mismo árbol; el producto vivo sigue siendo el Python.

**Sí cubre, desde el 2026-07-31**: farmacia e inventario con dispensación contra
fórmula, proveedores, laboratorio, imágenes diagnósticas y ambulancias. Ver
[Los módulos del port](#los-modulos-del-port).

### Los módulos del port

| módulo | tabla(s) | capacidad que exige escribir |
|---|---|---|
| Farmacia | `item_inventario`, `movimiento_inventario`, `dispensacion` | `GestionarFarmacia` (regente, admin) — **no** el médico |
| Proveedores | `proveedor` (+ `proveedor_id` en el movimiento) | `GestionarFarmacia` |
| Laboratorio | `resultado_laboratorio`, `perfil_laboratorio` | `EscribirLaboratorio` |
| Imágenes | `informe_imagen` | `EscribirImagenologia` |
| Ambulancias | `ambulancia`, `traslado` | `GestionarAmbulancias` |

**Leer no exige capacidad en ninguno**: enfermería tiene que poder saber si queda
adrenalina, y el médico que pidió el examen, ver su resultado.

**Un vocabulario, no tres.** `orden.estado` y `orden.tipo` llevan CHECK con los
valores del producto —«Pendiente»/«En Proceso»/«Completada»/«Cancelada» y
«Laboratorio»/«Imagenología»/«Remisión Especialista»/…— que son los que escribe
`historias.component.ts` y los que comparan las pantallas. Antes convivían tres
escrituras y dos pantallas estaban muertas sin que nada fallara: la de laboratorio
no mostraba el botón de procesar (comparaba «Pendiente» contra «PENDIENTE») y la
bandeja de remisiones no veía las remisiones creadas desde la historia (buscaba
«ESPECIALISTA» contra «Remisión Especialista»). Ver `0027_vocabulario_de_orden.sql`.

**Cerrado es cerrado.** Un resultado de laboratorio validado y un informe de
imágenes firmado no se editan, igual que la historia y el documento clínico: sobre
ellos ya se decidió una conducta. Se corrige emitiendo otro.

**Los bytes entran por un solo sitio.** `rutas::guardar_imagen_de` lo comparten el
adjunto de la historia y la imagen de un informe radiológico — con su tipo
deducido de la firma de los bytes y su cifrado en reposo. Un segundo almacén sería
la puerta por donde entrase el SVG que `tipos.rs` existe para no admitir.

### La bitácora de acceso (Ley 1581) — tres formas de evento, y las fija el tipo

`medico_datos::acceso` encadena por hash y firma cada acceso a dato de paciente
(sobre `guaca::auditoria`): borrar o reordenar un eslabón rompe la cadena, no solo
lo altera. Se registra **best-effort** —si falla, se grita en el log y NO se
bloquea la atención—, y `GET /api/acceso/verificar` (solo superadmin) da el
veredicto de la cadena entera.

`acceso::Objeto` es un **enum de tres variantes**, y eso es el diseño, no un
detalle: hasta el 2026-08-02 eran tres `Option` con un `debug_assert` encima
—«uno u otro, nunca ambos ni ninguno»—, que solo grita en depuración y solo si
alguien ejecuta esa línea. La afirmación que sostiene todo el rastro dependía de
que una prueba pasara por ahí; hoy un evento mal formado **no compila**.

| variante | cuándo | lleva | ¿entra en el rastro del titular? |
|---|---|---|---|
| `Recurso` | ver/editar su historia, un documento, una imagen, su ficha | `paciente_id` + `recurso_id` | sí |
| `DeUnTitular` | sus historias, sus documentos, sus resultados | `paciente_id` + `alcance` | sí |
| `Coleccion` | el directorio de pacientes, una exportación, un cambio de configuración | `alcance` | solo si es exportación y su id está en `alcance.pacientes` |

Helpers: `rutas::registrar_acceso`, `registrar_coleccion_de_un_titular`,
`registrar_coleccion`.

Ninguna forma rellena la otra (directiva 20), y `Recurso` **no inserta la clave
`alcance`**: su contenido canónico es byte a byte el de antes de existir las
colecciones y el de antes del enum, así que las cadenas ya selladas siguen
verificando. Lo fija `el_contenido_canonico_del_recurso_unico_esta_congelado`
comparando contra la **cadena literal** — comparar contra algo construido con el
mismo código no comprobaría nada.

**Por qué `DeUnTitular` es una forma propia y no una colección con el id dentro.**
Como `Coleccion` desaparecía del rastro de su titular (un listado no guarda ids, y
con razón), así que quien abría el expediente entero de alguien por la vía de los
listados no dejaba huella. Y meterle el id en `alcance.pacientes` habría dicho que
el dato **salió** del sistema, que es lo que significa esa lista. En el rastro se
lee `pacientes_n: 0`, y esa es justo la diferencia que el titular necesita ver
entre alguien que consultó su expediente y alguien que se lo llevó.

**Un listado NO guarda ids; una exportación SÍ.** Ver un nombre en una lista
filtrada no es abrir el expediente, y un eslabón por fila ahogaría la cadena; una
exportación, en cambio, saca el dato del sistema, y el titular tiene derecho a
saber que el suyo iba dentro. Por eso `por_paciente` casa las dos vías:
`$.paciente_id` **o** pertenencia a `$.alcance.pacientes`.

**Y de vuelta, esa lista no sale.** `acceso::proyectar` sustituye
`alcance.pacientes` por `alcance.pacientes_n` antes de devolver el rastro: quien
consulta el de un paciente no puede llevarse los ids de los demás que iban en el
mismo archivo — que en una exportación de superadmin son de otras instituciones.
Lo almacenado conserva los ids (son los que entraron en el hash); lo proyectado
**no es el contenido sellado**, y la verificación va contra la columna.

Cableado (2026-08-02):

| ruta | forma |
|---|---|
| `GET /api/pacientes/{id}`, `GET /api/historias/paciente/{id}/contexto` | `Recurso` |
| `GET /api/pacientes/{id}/historias` | `DeUnTitular` |
| `GET /api/documentos?paciente_id=…` | `DeUnTitular` |
| `GET /api/laboratorio/paciente/{id}/resultados` | `DeUnTitular` |
| `GET /api/imagenes/paciente/{id}/informes` | `DeUnTitular` |
| `GET /api/pacientes`, `GET /api/documentos` (sin filtro) | `Coleccion` |
| `GET /api/rips` | `Coleccion` con `pacientes` |

`/api/documentos` sale dos veces a propósito: **es la regla la que decide, no el
caso** (directiva 23). Con `paciente_id` es el expediente de un titular; sin él
atraviesa titulares. La regla, en una frase: *se anota como `DeUnTitular` toda
lectura que devuelve un conjunto de registros clínicos de UN paciente
identificado en la petición.*

La exportación se anota **solo cuando el archivo llegó a salir** —un 409 por
`parcial`, un 422 por datos faltantes o un 403 por rol no dejan rastro, porque una
bitácora que registra lo que no ocurrió es peor que una que no registra—; y una
ficha de otra institución tampoco, porque el 404 es anterior al acceso.

**Lo que falta, dicho para que no pase por cubierto**: los listados clínicos que
atraviesan pacientes (urgencias, hospitalización, remisiones, incapacidades,
agenda) todavía no anotan. El helper es de una línea; el freno es el volumen de
eslabones por refresco de pantalla, que hay que medir antes. Y las sublistas de
una historia (`/api/historias/{id}/formulas`, `/ordenes`) quedan fuera por la
regla: el paciente no va en la petición, se deriva.

El contenido ilegible no puede acortar el rastro, y el guardián sale gratis: el
índice de expresión `json_extract(contenido,'$.paciente_id')` de la migración
`0015` hay que evaluarlo para escribir la fila, así que SQLite rechaza un
`contenido` que no sea JSON.

### Entorno

`.env` en la **raíz del repositorio** (no en `backend/`); `backend/core/config.py` lo
carga con `load_dotenv(_BASE_DIR.parent / ".env")`.

```
DATABASE_URL=postgresql://usuario:clave@localhost/base
SECRET_KEY=<mínimo 32 caracteres>
```

Opcionales: `ENCRYPTION_KEY`, `SMTP_*`, `PORT`, `HOST`.

**`.env.example` NO es el ejemplo de este `.env`**: documenta el binario **Rust**,
que no carga ningún archivo —lee el entorno del proceso— y comparte los nombres
`SECRET_KEY` y `DATABASE_URL` con el Python **con otro contenido**
(`sqlite://medico.db` frente a `postgresql://…`). Exportar el `DATABASE_URL` del
Python y arrancar el binario es pedirle a SQLx que abra Postgres.

Y una trampa ya pagada: ese ejemplo traía
`SECRET_KEY=CAMBIE_ESTO_por_un_valor_aleatorio_de_32_o_mas_caracteres`, que
**pasaba** la validación —54 caracteres, y no figuraba en la lista de valores
prohibidos—, así que copiar el ejemplo dejaba el servidor firmando sesiones con
una clave publicada en el repositorio. Hoy va **vacío a propósito** y el arranque
falla; lo ata la prueba `el_ejemplo_no_entrega_un_secreto_que_sirva`
(`crates/medico-api/src/config.rs:211`). Esa línea no se «completa por comodidad».

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
models.py          83 tablas SQLModel, 25 enums (2 399 líneas, ~99 kB)
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
   la URL para leer, y con `PUT` modificar, la historia de otra institución. En vez
   de parchear cada llamada existe `obtener()`, y `tests/test_guardia_aislamiento.py`
   falla si un router vuelve a usar `db.get` directo sobre un modelo con dueño.

   La guardia **no lleva lista de excepciones por línea** —se desactualiza en el
   primer refactor y acaba silenciando lo que vigila—. Cada `db.get` legítimo lleva
   su razón escrita al lado: `# aislamiento: <por qué>`, con **mínimo 15 caracteres
   de razón**, para que la marca no se convierta en un silenciador. Medido el
   2026-08-01: 87 `db.get` en los routers, 84 de ellos con marca; el resto son
   modelos de `SIN_DUENNO` (catálogos, `Institucion`, `Cama`…), alineado con
   `core.aislamiento.COMPARTIDOS`. La lista `PENDIENTES_DE_MIGRAR` está **vacía**:
   llegó a tener diecinueve routers.

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

Son **22 campos** (11 `cod*CIE11` + 11 `nomCod*CIE11`), no 35. `backend/cie11.py` y
`medico-dominio/src/cie11.rs` los derivan del CIE-10 — **con la misma regla y desde
el mismo archivo**, escrito para los dos por una sola corrida de
`herramientas/generar_mapeo_cie11.py`. `tests/cie11_paridad_con_python.rs` compara
los 12 406 códigos uno a uno y falla si divergen, que es la única diferencia que
importa: dos CIE-11 distintos para el mismo diagnóstico entran los dos como válidos
y no rompen nada.

| caso | qué hace |
|---|---|
| equivalencia exacta y única | **deriva** sin preguntar |
| equivalencia por categoría | **propone**, menos específica |
| varios destinos | los propone **todos**, no elige |
| sin equivalencia | lo dice, y el campo queda vacío |

Medido el 2026-08-01 sobre los 12 406 códigos mapeados: **8 070 se derivan solos**
(65 %) y **1 402 (11,3 %) tienen más de un destino, hasta 35** —la cifra que decía
«hasta 42» era de una versión anterior de la tabla—. Poner el primero de una lista de
35 es exactamente lo mismo que `cod_consulta_cups="890201"`: un dato con la forma
correcta que nadie afirmó. La 948 admite estos campos vacíos.

## Catálogos: empaquetados, no consultados por red

`backend/datos/` va versionado con el código. Un consultorio en zona rural no puede
depender de que `web.sispro.gov.co` esté arriba el día del cierre de mes. Regenerar es
un paso deliberado.

| archivo | qué es | se regenera con |
|---|---|---|
| `divipola.json` | municipios y departamentos | publicaciones oficiales |
| `cum.json.gz` | 65 997 CUM del INVIMA (2,6 MB) | `herramientas/generar_cum.py` |
| `../crates/medico-dominio/datos/cum_codigos.txt` | los mismos 65 997 códigos, para el binario Rust | `../herramientas/generar_cum_para_rust.py` |
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

### Los tres que se corrigieron el 2026-08-02

`finalidad`, `causa_externa` y `tipo_diagnostico` eran el caso peor del párrafo
anterior, y llevaban meses sin verse porque **dos fallos se tapaban entre sí**:

1. tenían defecto (`'10'`, `'13'`, `'01'`) en el modelo Python, en el DDL de
   SQLite y en dos `unwrap_or` de `historia.rs`, así que nunca llegaban vacíos y
   el recolector —que solo ve AUSENCIAS— no tenía nada que anotar;
2. **ninguna pantalla los ofrecía**, así que nadie eligió uno nunca.

Y medido contra los maestros del validador: `FinalidadConsulta` (02–10) y
`CausaExterna` (01–15) eran las tablas **anteriores** a la 948, que usa 11–44 y
21–49. Ninguno de los 22 valores existía. O sea: dos campos del RIPS salían con un
código rechazable en toda atención, puestos por el sistema en nombre del médico.

Hoy: los enums se **generan** del maestro (`maestros_muv.finalidades()` /
`causas_motivo()`), los defectos se quitaron en los tres sitios, la migración
`0038` convierte lo escrito en hueco, el AC valida contra el catálogo y
`GET /api/finalidades` y `GET /api/causas-motivo` alimentan los dos selectores
nuevos del paso «Diagnóstico».

**Lo que NO se hizo, y es la parte que importa**: traducir los datos viejos. `13`
(enfermedad general) es exactamente el `38` de la tabla nueva, y aun así no se
reescribe — un valor que nadie eligió, traducido, sigue sin haberlo dicho nadie, y
encima pasa a ser válido, o sea que ya no lo detecta nada. Los códigos de la tabla
anterior salen en el informe de cierre diciendo que hay que convertirlos.

### El cuarto, encontrado el 2026-08-03: el documento del profesional (P11/P12)

Mismo patrón que los tres de arriba, y **más difícil de ver porque no había
defecto que lo tapara**: `usuario.tipo_documento` y `numero_documento` existían
desde `0037`, el RIPS los exige en los cinco archivos, y **ninguna ruta los
escribía** —las únicas escrituras del repositorio eran fixtures de prueba, y
`CrearUsuario` llevaba `nombre, email, clave, rol, institucion_id` y nada más—.
El recolector los anotaba como ausentes en toda atención y no había forma de
arreglarlo desde el producto.

Hoy entran por `POST /api/usuarios`, y con tres reglas:

- **o los dos o ninguno**: medio documento no identifica a nadie, y aceptar solo
  el número dejaría una fila que *parece* traer el dato;
- **el tipo se valida contra `muv::admite_como_profesional`**, que se genera del
  maestro del validador — `RC` y `TI` están en el catálogo (un paciente puede
  tenerlos) pero un menor de edad no firma un acto médico. Es el campo que ya se
  equivocó una vez mandando el documento del PACIENTE;
- **no se repite**: `ux_usuario_documento` (migración `0039`), único en todo el
  sistema y no por institución, porque una persona es la misma en dos IPS.

Sigue siendo opcional, y eso no es una concesión: los usuarios que ya existen no
lo traen, y exigirlo en el alta no arregla a los que están. Para esos hay
**`PUT /api/usuarios/{id}/documento`**, con `GestionarUsuarios`, aislado por
institución (**404 y nunca 403**: un 403 confirma que el id existe y con eso se
enumera la plantilla del vecino) y con la misma validación —la regla vive en
`documento_del_profesional`, UNA función que comparten el alta y la corrección,
porque dos copias es lo que ya dejó dos pantallas muertas con `orden.estado`.

Enviar los dos campos vacíos **borra** el documento, a propósito: corregir un
dedazo dejando la fila sin documento es mejor que dejarla con uno falso, que es
justo lo que P11/P12 enseñó a no hacer.

**No es un `PATCH /api/usuarios/{id}` general**, y esa es la decisión: un
actualizador general sería la puerta por la que se cambia el rol o la clave de
otro, y necesitaría repetir la regla de que solo un superadmin fabrica otro
superadmin más varias decisiones aparte. Esta ruta no puede hacer nada más que
lo que dice su nombre.

**La pantalla es `/usuarios`** (`pages/usuarios/`), y existe aparte de
`/admin` a propósito. Aquella tiene su pestaña de usuarios pero habla
ÚNICAMENTE con `/api/admin`, que es Python y da 404 contra el binario; su
formulario pide además `username`, `especialidad_id`, `registro_medico`,
`registro_rethus` y `cuenta_bloqueada`, y ninguno existe en la tabla `usuario`
de Rust —la especialidad y el registro llegan con el maestro de talento humano,
y el bloqueo no existe porque en su lugar hay un freno que se agota solo—.
Esperar al port completo de `admin` dejaba P11/P12 vacío para todos los
profesionales; cuando se porte, esto es su pestaña de usuarios y no trabajo
tirado.

La pantalla dice **cuántos van sin documento** y lo marca en la tabla, que es la
pregunta que hay que poder responder de un vistazo — por eso `ResumenUsuario`
lleva los dos campos: sin ellos habría que abrir uno por uno. Los ocho tipos los
sirve `GET /api/tipos-documento-profesional`: la lista **no** se escribe en el
navegador, porque una copia allí y otra en `muv.rs` divergen al primer cambio
del maestro y la del navegador es la que ve el médico.

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

### Los antecedentes, contrastados contra la pantalla (2026-08-03)

Se comparó campo a campo el formulario `Examen físico` del sistema vivo —capturas
en `~/Imágenes/virtualbox`, pestañas GRAL · AF · AP · AGO · API · MVTO— contra
nuestros cuatro bloques. **AF y AP salieron completos**; los otros dos no.

| bloque | resultado |
|---|---|
| **AGO** | faltaban **IRS**, **FPP**, **FUP** y **preclampsia**. Añadidos. |
| **API** | faltaban **ocho**: embarazo deseado, hospitalización por IRA / EDA / otra, meses de lactancia, inmunización completa, desarrollo psicomotor y hermanos muertos menores de 5 años. Añadidos. |

**Y el cuarto bloque tenía tres nombres.** La columna era `ant_obstetricos`
—antecedentes de la MADRE— y guarda los del NIÑO; la pantalla lo llamaba
`antPrenatales` y el original, API. Hoy es `ant_perinatales` en los tres sitios
(migración `0040`), por la misma razón que se unificó el vocabulario de `orden`.

**Por qué esto no era cosmético**: IRA, EDA, esquema de vacunación completo,
desarrollo psicomotor y muertes de menores de cinco años son los indicadores de
AIEPI y de crecimiento y desarrollo, o sea la materia prima de
`epidemiologia.py`. Sin campo donde ponerlos, el informe nuevo **no puede dar los
números** que el prestador viene entregando, que es la condición de fidelidad de
todo el port — y eso no se descubre hasta el cierre del mes.

**Dos campos NO se portaron, a propósito**: `EM` y `M` de la pestaña AGO. No
están en la fórmula obstétrica G-P-A-C-V y no sabemos qué significan («embarazo
múltiple» y «muertos»? ¿«ectópico/molar»?). Inventar el sentido de un campo es
lo mismo que inventar su valor. Preguntado al prestador, junto con la
incoherencia que se ve en la propia captura: un paciente de **1 año** con
menarquia a los 11 y FUM de 2023 en la pestaña AGO — o son registros distintos,
o el formulario admite antecedentes ginecológicos sobre un lactante sin
quejarse.

## Frontend

Angular 22.1 + `@angular/material` 22.1, standalone, 30 páginas y 50 entradas en
`app.routes.ts` (47 con `loadComponent`, el resto redirecciones), con `authGuard`,
`roleGuard` y `modulo.guard`. Medido el 2026-08-01.

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
- **Los dos sistemas migran distinto, y se confunden:**
  - **Python, sin Alembic**: añadir a la lista `sql_migrations` de
    `database.py:_run_migrations()` (hoy 52 entradas, todas `ALTER TABLE`), con
    `ADD COLUMN IF NOT EXISTS`. Corren en cada arranque y son idempotentes.
  - **Rust, con `sqlx::migrate!`**: un archivo nuevo y numerado en
    `crates/medico-datos/migrations/` (hoy 46, de `0001_fundacion.sql` a
    `0046_consentimiento_informado.sql`). Las aplica `medico-datos/src/lib.rs:74` al abrir
    la base. Son **inmutables**: `sqlx` guarda el hash de cada una, así que editar
    una ya aplicada rompe el arranque contra una base existente — se añade otra.
- **Enums**: todos heredan `str, Enum`. Comparar por miembro (`estado ==
  EstadoCita.PENDIENTE`), no por cadena.
- **Router nuevo**: crearlo en `backend/routers/`, importarlo en `main.py`, añadirlo a
  `core_routers`, y **quitarlo de `PENDIENTES` en `stubs.py`**.
- **Aislamiento**: toda consulta de paciente/historia/cita filtrada por institución, y
  todo acceso por id vía `obtener()`.
- **N+1**: `joinedload()` / `selectinload()`. El pool son 20 conexiones.
- **Frontend**: `npm ci` a secas. El `--legacy-peer-deps` era de PrimeNG y se fue con él (2026-08-02).
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
| **#16 / #65** | Migración VFP completa. Quedan **21 módulos en 501** y tres sin portar: `soportecobertura`, `terceros` y `ayudasdx`. **Corregido el 2026-08-03**: aquí decía «tres sin existir», y es falso — los tres tienen formulario en el original (`soportecovertura.scx`, `terceros.scx`, `ayudasdx.scx`) y dos tienen tabla con datos (`TERCEROS.DBF`, `AYUDASDX.DBF`). «No existe» decía que no había nada que migrar; lo que hay es trabajo sin hacer. Orden fijado por Juan: **migración completa primero, mejoras después**. |
| **#67** | FEV-RIPS por archivo. AC y AM están verificados por dos fuentes; **AP/AU/AH/RN/OS no se implementan sin contrastarlos a ojo contra el PDF** — las tablas de urgencias, recién nacidos y otros servicios no se dejaron extraer por prefijo. |
| **#77** | Cargar el CUM a **base de datos** (66 mil registros que caducan, no JSON empaquetado). `Medicamento` se queda corto: faltan estado_cum, registro sanitario, vencimiento anulable, ATC, principio activo, vía, unidad e IUM. Pendiente aparte: CUPS (`cups.xlsx`, cabecera corrida una columna). |
| **#88** | Barrer el backend buscando el patrón «filtro opcional» fuera de los routers, y decidir si `test_guardia_aislamiento.py` puede cubrir esa capa. |
| **#84** | Captura asistida: fricción graduada — bloquear solo donde hay efecto jurídico o económico. |
| **#104** | Antropometría: migrar `PESOTALLA1` (62 filas, con índice EDAD), **no** `PESOTALLA`. En el original el `SEEK` abría la tabla equivocada y la talla ideal salía siempre en 0. |
| **#80** | Angular 21.2.0 → 21.2.18 y decidir el salto a la 22. Los paquetes se peer-dependen con versión exacta: `ng update @angular/core@21 @angular/cli@21`, nunca `npm install` sobre el árbol existente. **Ya no lo frena nadie**: PrimeNG salió el 2026-08-02 y era su único peer atado a la 21. |
| **#65.7** | La verificación que de verdad cierra el RIPS: comparar el JSON propio contra el del conversor web de MinSalud para el mismo caso. |
| **#65.6** | Panel para editar `modulos_habilitados` sin tocar la base. Bloquea dar de alta un cliente nuevo. |

Preguntas abiertas para el prestador: qué hace `mousemovex` (108 usos); si alguna vez
registró un CVE distinto del diagnóstico (los informes darían números diferentes,
porque ahora el evento se deriva del diagnóstico); si el archivo que produce hoy pasa
el validador 1.4.8.0.

Cerradas y ya no pendientes: #79 (las pruebas compartían una BD por accidente — lo
resuelve `conftest.py`), DIVIPOLA y CIE-10 cargados, ejecutor de pruebas del frontend
montado.
