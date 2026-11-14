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

## Este archivo lleva lo básico; el detalle está en `DETALLE.md`

Partido el 2026-08-10, cuando este archivo iba por **3 235 líneas y 192 KB** y se
cargaba entero en cada turno de cada sesión. Aquí queda lo que hace falta **antes
de tocar código**: qué es el producto, cómo se construye y se prueba, las reglas
que no se rompen y las convenciones que ya han costado tiempo.

**[`DETALLE.md`](DETALLE.md) lleva el resto, íntegro y sin resumir** — la bitácora
del port pantalla por pantalla, las decisiones con su medición, y lo histórico
(Angular, la API Python). No se carga solo: **se consulta con `grep`**, que es
como se usaba de verdad.

```bash
grep -n "freno\|bitácora\|FEV-RIPS" DETALLE.md   # buscar por tema
grep -n '^#\{2,3\} ' DETALLE.md                  # su índice, siempre al día
```

**Antes de escribir una sección larga aquí, mírala dos veces**: si es la crónica
de un cambio ya hecho —qué se midió, cuántos mutantes murieron, qué se decidió y
por qué— va a `DETALLE.md`. Aquí solo entra lo que cambia lo que alguien va a
teclear **hoy**.

## Qué es esto

Reimplementación de un sistema médico que hoy corre en **Visual FoxPro** en el
consultorio de un cliente real (`/mnt/data/aprograms`, fuente autoritativa;
`/mnt/data/repoNuevo`, el generador de RIPS y epidemiológicos). No es un producto
de cero: cada decisión se contrasta contra lo que el prestador ya usa.

**El producto es UN binario Rust** (`crates/`): axum + SQLx sobre SQLite, con las
29 pantallas tejidas en el servidor con `maud` + htmx (directiva 26). No hay
frontal externo, no hay Node en el build, y el recuento de dependencias de
frontal del producto entero es **UNA** (htmx, MIT, vendorizada).
Multi-institucional, con licencia por institución que decide qué módulos se ven.

**Del Python (`backend/`) ya no queda API**: `main.py` y los `routers/` se
retiraron el 2026-08-09 (#598). Lo que sigue vivo es la **cadena RIPS/FEV-RIPS
como módulos** —oráculo de las pruebas de paridad de Rust y herramienta del MUV—.

El eje del trabajo actual es la cadena **RIPS → FEV-RIPS → MUV**: producir el JSON
de la Resolución 948 de 2026 y que el Mecanismo Único de Validación de MinSalud lo
acepte y devuelva CUV. Ver [La cadena RIPS](#la-cadena-rips--fev-rips--muv).

**Angular salió el 2026-08-08 y PrimeNG el 2026-08-02**; la crónica de las dos
retiradas, con lo que dejaron aprendido, está en `DETALLE.md`
(«Angular y PrimeNG»). `MIGRACION_MATERIAL.md` y `MIGRACION_FORMULARIOS.md`
quedan como **historia**, no como instrucciones.

## Comandos

```bash
# === BACKEND (Python) — ya NO es una API ===
# main.py y los routers/ se retiraron el 2026-08-09 (#598): no hay `uvicorn`, no
# hay Swagger, no hay servidor. Lo que queda del Python es la cadena RIPS/FEV-RIPS
# como MÓDULOS (oráculo de las pruebas de paridad de Rust y herramienta del MUV).
cd backend
.venv/bin/python -m pytest -q                    # 499 pruebas (~17 s) — es el job `cadena-rips-python`
.venv/bin/python -m pytest tests/test_cie11.py -q         # un módulo
.venv/bin/python -m pytest tests/test_cie11.py::test_x -q # una prueba
.venv/bin/python -m pytest -k "aislamiento" -q            # por nombre

# === LA INTERFAZ y la API ===
# Las dos viven en el binario Rust (`crates/medico-api`). Se ve arrancándolo —`/`
# lleva a `/panel`— y se prueba con `cargo test`, porque `maud` compila las
# plantillas con `rustc`. El FEV-RIPS lo genera Rust (`fev.rs`), probado byte a
# byte de extremo a extremo contra el Python (ver la sección de #598).

# === DOCKER ===
# Ya NO hay servicio `backend` (se fue con la API Python el 2026-08-09). El
# docker-compose queda solo con el `db` postgres de referencia, sin consumidor.

# === UTILIDADES DE BD (Python, para la cadena RIPS) ===
cd backend
python importer.py                               # CIE-10, CUPS, EPS, INVIMA
python reset_db.py && python seed.py             # PELIGRO: borra todo
```

**Nunca canalizar `pytest` a `tail`/`head` sin `set -o pipefail`**: el código de
salida se pierde y un fallo se lee como verde. Un hook global lo bloquea. Redirigir
a archivo y comprobar `$?` es la forma limpia.

## El CI — tres trabajos, y las dependencias NO salen de `requirements.txt`

`.github/workflows/ci.yml`. El producto es el binario Rust, y el FEV-RIPS ya se
genera y prueba en Rust (#598), así que el CI es **todo Rust**.

**El trabajo `frontend` se fue el 2026-08-08 con Angular** y el `backend`
(pytest) el 2026-08-09 con la API Python (#598). Eran cinco trabajos, quedaron
tres, y son **CUATRO desde el 2026-08-11**: volvió el pytest, acotado a lo único
que sobrevive del Python —la cadena RIPS/FEV-RIPS, oráculo de las pruebas de
paridad de Rust y herramienta del MUV—. Estaba en espera «hasta que el cupo
vuelva», y esa condición murió: no vuelve (ver el recuadro de abajo). El runner
es `verde_local.py`, así que el trabajo corre hoy.

| trabajo | comando exacto |
|---|---|
| `rust` | `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace --all-targets` + el clippy de `enviar-rips` con `--features cli`, que los dos anteriores **no compilan** (`--all-targets` salta los binarios con `required-features` apagada; medido con su control el 2026-08-11) |
| `auditoria-deps` | `cargo audit` (RustSec; el único *ignore* vive en `.cargo/audit.toml`) |
| `cadena-suministro` | `cargo deny check` (fuentes, bans, licencias; config en `deny.toml`) |
| `cadena-rips-python` | `.venv/bin/python -m pytest tests/ -q` desde `backend/` — **499 pruebas en 17 s** (medido el 2026-08-11; la cifra de 561 que decía este archivo era anterior a la poda del 2026-08-10) |

Para reproducir el verde en local se copian **esos** comandos, no unos parecidos.
Dos trampas medidas:

- **Las dependencias de prueba salen de `requirements-recuperado.txt`, en la RAÍZ**
  —el *freeze* completo del entorno que funciona—, no de `backend/requirements.txt`,
  que no trae `pytest`. Instalar el segundo y ver fallar la colección es el camino
  largo.
- `rust` instala `libsqlite3-dev`, y el motivo que aquí decía era **falso**
  (corregido el 2026-08-14 por las dos revisiones de seguridad del vendorizado):
  `libsqlite3-sys` 0.30 **sí** usa el *bundled*. `cargo tree -f '{p} {f}'` da
  `libsqlite3-sys v0.30.1 bundled,bundled_bindings,cc,…`, porque `sqlx/sqlite`
  arrastra `sqlx-sqlite/bundled`. O sea que SQLite se compila desde
  `vendor/libsqlite3-sys/sqlite3/sqlite3.c` —**257 673 líneas de C** dentro del
  binario que guarda historias clínicas— y ese `apt-get` no cambia nada.

Corre en `push` **solo a `main`** y en `pull_request` contra `main`; antes disparaba
los dos eventos sobre el mismo commit y los cinco checks corrían por duplicado, que
en un repositorio privado son minutos facturados. **Consecuencia declarada: una rama
empujada sin PR no tiene CI.**

> **GitHub NO ejecuta este `ci.yml`, y es permanente.** Este recuadro decía
> «apagado hasta el 2026-09-01, y ese día se levanta con `cupo.py levantar`».
> **Esa frase quedó falsa el 2026-08-10** y se corrige aquí porque un documento
> que se lee en cada turno es una máquina: a partir de septiembre habría estado
> ordenándole a cada sesión que reactivara GitHub Actions, que es justo lo
> contrario del protocolo, y sin dar un solo error.
>
> Lo que hay hoy: el protocolo **«solo local, librerías a pedido»** (declarado
> por Juan el 2026-08-10, PERMANENTE, no caduca). El origen del código es
> `/home/juan/espejos/medico.git` —`origin` apunta ahí—, los repositorios de
> GitHub están vaciados y Actions apagado en los 19. **No hay cupo que gastar y
> no hay fecha que esperar. `cupo.py levantar` no se ejecuta nunca.**
>
> **El verde es el local, y no como sucedáneo sino por construcción**:
> `python3 /mnt/data/conocimiento/cupo/verde_local.py medico` abre este mismo
> `ci.yml`, saca los `run:` en orden y los ejecuta con el mismo
> `bash -eo pipefail` que usaba GitHub. Dice siempre lo que NO reprodujo (los
> pasos `uses:`), que es lo que impide confundirlo con el verde del runner.
>
> Al afirmar que algo pasa: `gh run list` no va a mostrar nada, y eso no es una
> avería. **Mergear** ahora significa integrar en `main` del origen LOCAL, y
> sigue exigiendo la revisión de seguridad antes (directiva 25). **Desplegar al
> VPS** nunca dependió de Actions y sigue disponible, con la misma puerta:
> verde local + directiva 25 + anunciarlo.

## La demostración pública (medico.xiliux.com)

Es una **demostración**, no la instalación de un cliente: sus credenciales están
publicadas en la propia pantalla de acceso y cualquiera puede escribir en su base.

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

**El despliegue deja un PUNTO DE VUELTA, y hay guion para volver** (desde el
2026-08-16). Una carpeta por despliegue, `/opt/medico-demo/respaldos/<marca>/`,
con base + imágenes + binario juntos; se conservan las 6 últimas.

```bash
./despliegue/desplegar-demo.sh debian@192.99.42.108     # paso 3b/6: toma el punto
./despliegue/volver-atras.sh   debian@192.99.42.108     # lista los puntos
./despliegue/volver-atras.sh   debian@192.99.42.108 20260816-184048
```

**Lo toma `medico-api respaldar`, no un `cp`.** Salió de desplegar nueve
migraciones de una pasada (0060–0068) y descubrir que el guion respaldaba el
binario —o sea el código— y no los datos. La primera corrección fue un `cp` con
`sha256`, y era **peor que nada porque tranquilizaba**: la base va en modo WAL,
así que el archivo suelto no lleva las transacciones confirmadas que viven en
`medico.db-wal`, y el `sha256` sale verde igual —compara la copia con el
original, y los dos están igual de incompletos—. Medido con escritor vivo: de
300 filas confirmadas, el `cp` recuperó **0** con su ✔, y `respaldar`
(`VACUUM INTO`, más las imágenes, verificando al abrir) recuperó **300**. La
forma buena ya estaba en el repositorio desde #733; reimplementarla peor aquí
fue la directiva 1 al revés.

**Y volver atrás NO es restaurar el binario y reiniciar.** `sqlx::migrate!`
valida al arrancar que el binario conozca toda migración aplicada
(`ignore_missing` en `false`), así que **el binario anterior no arranca contra
una base migrada** —`VersionMissing`, servicio en `failed`— aunque las
migraciones sean aditivas. Y restaurar la base con `cp` dejando el `-wal` al
lado devuelve el estado que se quería abandonar, con `integrity_check` en `ok`:
`medico-api restaurar` los borra a propósito. `volver-atras.sh` hace las tres
piezas en orden, guarda el estado previo en `antes-de-volver-<marca>` y
comprueba contra HTTP.

Probado entero contra el servidor real, con su pareja: la vuelta completa deja
`medico.xiliux.com` en 200, y un punto de vuelta incompleto **aborta con 3 sin
parar el servicio**. La poda tiene su propia prueba: con 9 carpetas, borró las 5
más viejas nombrándolas y conservó las 6 más nuevas.

**Trampa del guion, que mordió DOS veces en una tarde**: dentro de
`ssh "$DESTINO" "…"` el shell LOCAL expande la cadena, así que un backtick
escrito en un **comentario** es sustitución de comandos y se ejecuta aquí. Por
eso el bloque `3b` pasó a viajar por un heredoc **citado** (`<<'REMOTO_SH'`) con
los datos como argumentos posicionales: el texto va literal y no hay escapes que
acertar. Los bloques antiguos del guion siguen con backticks escapados a mano.

**Los permisos también salieron de ahí**: los respaldos nacían `0644` y la base
viva estaba en `0644`, legibles por cualquier cuenta local — y en esa máquina hay
otra cuenta con shell (`quantum`). Hoy la base es `0600`, `respaldos/` es `0700`
y el punto de vuelta entero nace sin permisos para «otros». Un respaldo es
además **persistente**: conserva lo que la base viva ya no tiene, y el derecho de
supresión de la Ley 1581 no alcanza a copias que nadie sabe que existen.

## El workspace Rust

**Cinco** crates —`medico-dominio`, `medico-datos`, `medico-api`, `medico-ihce`
(transporte contra el mecanismo IHCE/RDA de MinSalud, #603) y `medico-muv`
(cliente del Mecanismo Único de Validación)— con axum y SQLx
sobre SQLite, y **la interfaz tejida dentro del propio binario** con `maud`. Es
el mismo modelo que informes: se entrega **un archivo**, y desde el 2026-08-08 es
literal — antes era un binario más un árbol de JavaScript que había que construir
primero.

```bash
./construir.sh                    # el binario. UN paso, sin Node.
cargo clippy --workspace --all-targets -- -D warnings   # lo que corre el CI
cargo test  --workspace --all-targets                   # 1 172 pruebas (~6,5 min)
```

**La única dependencia por `git` es `guaca`, y desde el 2026-08-11 sale del
ESPEJO LOCAL**: `file:///home/juan/espejos/guaca.git`, con el mismo `rev`
(`23395f3`) que fijan tunjo e informes — un solo árbol de guaca en todo el
universo. Apuntaba a `github.com/isazajuancarlos/guaca`, y ahí se había quedado
cuando el protocolo «solo local» del 2026-08-10 mudó el origen del código: cada
compilación limpia seguía bajando guaca de GitHub.

**No estaba roto, y por eso no se había notado**: medido ese día, GitHub sirve
todavía ese `rev` por SHA aunque el repositorio esté vaciado
(`git fetch <url> <sha>` → 0). O sea que el build dependía de que GitHub
conserve un objeto que ya no alcanza ninguna rama, en un repositorio que la
retirada dejó vacío y del que 20 hermanos ni siquiera existen. Un respaldo del
que nadie sabía que se dependía.

`deny.toml` se movió con él —`allow-git` nombra el espejo y **sustituye** a la
entrada de GitHub, no la acompaña—, y esa lista discrimina: devolviendo el
`Cargo.toml` a la URL de GitHub, `cargo deny check sources` sale **FAILED**.

**Ya no hay orden que respetar ni carpeta que llenar.** Hasta el 2026-08-08
`construir.sh` tenía tres pasos —`ng build`, copiar a `estaticos/`, compilar— y
dos máquinas cuidándolos: el guion evitaba el binario RANCIO (invertir el orden
lo dejaba con la interfaz de la vez anterior) y `build.rs` evitaba el binario
VACÍO (con `estaticos/` en `.gitignore`, un clon limpio producía un binario que
servía la API y devolvía 404 en la raíz sin avisar). Con `maud` las dos causas
desaparecen: `rustc` compila las plantillas, y htmx, `boi.js` y `panel.css` van
con `include_str!` desde `crates/medico-api/panel/`, **que sí está versionado**.

Medido al retirarlo, con el método que dejó informes: se extrajo el árbol de git
a otro directorio (`git archive` + `tar -x`) y se compiló allí. Es la
comprobación que hace falta porque en local todo sale verde aunque un archivo no
esté en git — a informes le pasó, con `.gitignore` tragándose dos activos del
panel.

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

**Lo que el port todavía NO cubre, cómo se cuenta, y la bitácora del port
pantalla por pantalla** —29 secciones con su medición y sus mutantes— están en
[`DETALLE.md`](DETALLE.md). Resumen a 2026-08-09: las **29 pantallas** están en
`maud`, los **cinco documentos RIPS** y el **FEV-RIPS** se generan en Rust y su
paridad con el Python está probada byte a byte y de extremo a extremo.

## Entorno

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

## El segundo factor (TOTP)

Desde el 2026-08-12 (#212). **La regla vive en `guaca::dosfactores`** —subió
desde informes el mismo día, porque medico es el segundo proyecto que la
necesita— y el QR de su enrolamiento en `guaca::qr`. Aquí quedan las columnas
(`medico_datos::segundofactor`, migración `0058`) y las pantallas.

| pieza | dónde |
|---|---|
| la regla: ventanas, reutilización, secreto ilegible | `guaca::dosfactores` (v0.6.0) |
| el QR en SVG, sin dependencias | `guaca::qr` |
| columnas y consultas | `medico_datos::segundofactor` |
| el acceso en dos pasos | `rutas::intentar_acceso` → `rutas::verificar_segundo_paso` |
| enrolamiento y retirada | `/panel/segundo-factor` |
| rescate del administrador | `DELETE /api/usuarios/{id}/2fa` |
| el ciclo dibujado por un navegador | `tests/pantalla.rs`, `el_segundo_factor_se_activa_y_el_acceso_pasa_a_dos_pasos` |

**El arnés de navegador es `mirilla`, no una copia local** (desde el
2026-08-13). `crates/medico-api/tests/comun/mod.rs` tenía 319 líneas que eran la
misma pieza que informes duplicaba: `mirilla` se extrajo de las dos el
2026-08-10 —de ~309 líneas la única diferencia era la ruta de la entrada, hoy su
único parámetro— y aquí la copia se quedó. Se consume por `rev` contra el espejo
local, en `dev-dependencies`, así que **no toca el binario que se instala en el
consultorio**; con ella se fue `libc`, que era su único uso en todo el crate.

Lo que la copia se llevaba y ahora no: el navegador no iba en su propio grupo de
procesos, así que **cada corrida del banco dejaba un perfil de Chromium huérfano
en `/var/tmp`**.

**Y esta línea decía «20 perfiles antes de correr y 20 después», que HOY ES
FALSO** (medido el 2026-08-16 con su control: 1 637 directorios
`org.chromium.Chromium.*` en `/var/tmp`, **1 646 al terminar la corrida**, o sea
**9 por corrida**, y 111 creados solo ese día). La cuenta sube todos los días:
663 el 08-15, 468 el 08-13, 336 el 08-12. Se corrige aquí porque una regla
escrita que describe una limpieza que no ocurre es peor que no tener la regla.

**Lo que la migración sí arregló y lo que no** hay que separarlo, porque no es lo
mismo: `mirilla` mata el PROCESO del navegador (grupo propio), y eso se ve — no
quedan Chromium vivos. Lo que no borra es el DIRECTORIO de perfil, que Chromium
crea en `/var/tmp` y solo limpia al salir por su cuenta. Pesan poco —21 MB entre
los 1 646— así que no es un problema de disco: es suciedad que crece sin techo en
un directorio que ya va por 4 399 entradas. Arreglarlo es aguas arriba, en
`mirilla`, no aquí.

Y la migración destapó una regresión de la propia extracción: `entrar` esperaba
`!location.pathname.startsWith(entrada)`, y con segundo factor el envío lleva a
`/panel/entrada/codigo` —que empieza por la entrada—, así que la espera no se
cumplía nunca. Arreglado aguas arriba (mirilla `c7de2e8`) y no aquí: parchear la
copia era justo lo que la extracción vino a impedir.

**El token del primer paso sirve UNA VEZ, y lo decide el servidor.** Lleva un
número que sale de `usuario.pendiente_version` y se consume en el mismo `UPDATE`
que lo comprueba (migración `0059`). Borrar la cookie con `Max-Age=0` **no
consume nada**: lo obedece un navegador y lo ignora cualquier cliente que hable
HTTP, que reenvía el mismo valor crudo con otro código hasta que caduque la
firma. El «quemado» vivía en el cliente, que es donde no puede vivir.

**Y va firmado en OTRO DOMINIO**, no con un campo más.
Con una bandera en la carga, un pendiente y una sesión compartirían firma válida
y la separación dependería de que todos los verificadores se acuerden de mirarla;
con el prefijo, un pendiente presentado como sesión **falla la firma**, que es el
camino que no se puede olvidar de mirar. Dura cinco minutos y va en su propia
cookie: con el mismo nombre pisaría una sesión abierta en otra pestaña.

**El secreto se guarda cifrado con `guaca::reposo` bajo `MEDICO_STORAGE_KEY`, y
sin esa frase el alta se niega a empezar** — no se guarda en claro «mientras
tanto», y la pantalla lo dice en vez de dejar que el botón falle.

**El contador antirreplay se quema en el MISMO `UPDATE` que lo comprueba.** Es el
hallazgo de la revisión de seguridad de informes: con la forma obvia —leer,
verificar, escribir— tres peticiones simultáneas con el mismo código abrían tres
sesiones. Lo sostiene una prueba de cien peticiones concurrentes.

**Retirarse el factor propio pide la CONTRASEÑA** (aquí medico se aparta de
informes): una sesión robada no puede dejar la cuenta con una sola puerta. Y el
rescate del administrador está **prohibido sobre uno mismo** — si valiera, una
sesión de administrador robada apagaría su propio segundo factor sin saber
ninguna contraseña.

**Lo que NO hace todavía**: `requiere_2fa` y `requiere_2fa_admin` siguen sin
aplicarse. El factor existe pero el enrolamiento es **voluntario**; imponerlo
pide decidir qué le pasa a quien entra sin haberlo activado, y eso es otra
pantalla. El inventario de `administracion.rs` lo dice con ese motivo, en vez de
darlas por aplicadas.

## La bitácora de acceso (Ley 1581) — tres formas de evento, y las fija el tipo

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

La regla completa —por qué `DeUnTitular` es una forma propia, qué guarda cada
una, la tabla de las rutas ya cableadas y qué queda fuera a propósito— está en
[`DETALLE.md`](DETALLE.md). **Lo que no se rompe**: un listado NO guarda ids; una
exportación SÍ, y solo cuando el archivo llegó a salir.

## Retención y supresión por defunción (#758, Ley 1581)

`medico_datos::retencion` (vuelca y purga) + `medico_api::retencion` (cifra, firma,
verifica, orquesta). Al cumplirse **20 años desde la fecha de DEFUNCIÓN** (config
`MEDICO_RETENCION_ANOS`; Res. 1995/1999, y la defunción es ≥ la última atención, así
que anclar ahí nunca borra antes del mínimo legal), una tarea de fondo archiva el
expediente completo en una copia **comprimida + cifrada + manifiesto firmado** bajo
custodia y **purga** la historia de la base viva, dejando un **certificado de
supresión** (`supresion`, migración `0076`): la prueba de que existió y se borró, sin
conservar el dato.

**INACTIVO por defecto** (D20): sin `MEDICO_RETENCION_KEY` + `MEDICO_RETENCION_DIR`
(+ `MEDICO_SIGNING_KEY` para firmar) no arranca y lo DICE en el log. El binario no
cambia de comportamiento observable hasta que el prestador lo configure.

**Reglas que no se rompen** (son destrucción de datos):
- **No se purga sin copia verificada.** El orden es archivar → re-descifrar y
  comparar hash → purgar. La purga (`retencion::purgar`) corre en una transacción que
  re-comprueba que la defunción sigue vigente y que los recuentos no cambiaron desde
  el volcado; si algo cambió, ABORTA sin borrar.
- **El alcance es UNA lista curada** (`EXPEDIENTE`), verificada contra el esquema real
  por `el_mapa_cubre_toda_fk_al_paciente`: toda tabla con FK a `paciente` (directa o
  vía `historia`) está en `EXPEDIENTE` o en `EXENTAS` con su razón, o la prueba falla
  (patrón de `respaldo::TABLAS`). Una tabla nueva ligada al paciente NO se cuela.
- **Lo que se conserva** (decisión de Juan, 2026-09-22): la fila `paciente` no se
  borra —la referencian `acceso` y los `recien_nacido` de hijos vivos— sino que se
  ANONIMIZA su PII; la cadena `acceso` entera (prueba de tratamiento lícito); el
  `recien_nacido` de una madre fallecida (es del hijo); y `envio_rda` queda fuera
  (sin `paciente_id`, ya transmitido, residuo documentado).
- **Claves separadas** (D32): el archivo se cifra con `MEDICO_RETENCION_KEY`, NO con
  `MEDICO_STORAGE_KEY`. El manifiesto se firma con `MEDICO_SIGNING_KEY`.

**Dos fail-safes que blindan la activación** (MERGEADOS a `main` y desplegados a la demo el
2026-09-27, `main` = `09e6f108`; 4º-6º concilio): (1) una atención POSTERIOR a la
defunción vuelve al paciente NO elegible —`sin_atencion_tras_defuncion()` en
`elegibles` y en el re-chequeo de la purga—, así una fecha de defunción mal tecleada
(un 2005 por 2025) no borra una historia viva. **La prueba de vida se DERIVA de
`EXPEDIENTE` (dos cubetas: `PRUEBA_DE_VIDA` bloquea, `ADMINISTRATIVAS_PARA_VIDA` no),
no de 4 tablas a mano**: el concilio del 2026-09-23 (fable/sonnet/opus, 9) midió que
las 4 originales dejaban colar una vacuna/medicación/transfusión post-mortem (un
paciente de P&P sin `historia` se purgaba); un guardián D33
(`toda_tabla_del_expediente_esta_clasificada_para_el_fail_safe`) obliga a clasificar
TODA tabla de `EXPEDIENTE` en una cubeta —sin patrón de fecha, para no tener punto
ciego: el concilio de cierre cazó que `recuperacion_postanestesica` fecha su acto en
`ingreso_recuperacion` (fuera de `fecha%`/`hora%`) y estaba mal clasificada—. La cubeta ADMINISTRATIVA (factura/resultado/nota de cierre) NO
bloquea, para no dejar a un fallecido real imposible de suprimir (Ley 1581, D23).
(2) Las imágenes viven en un
NAMESPACE por instancia `MEDICO_DIR_IMAGENES/{uuid}/{id}.dat` (uuid de `instancia`, migración
`0077`): cada base escribe, sirve, reconcilia y purga SOLO en su subcarpeta, así que un
`DATABASE_URL`+`MEDICO_DIR_IMAGENES` cruzado entre bases de uuid distinto no puede leer,
exfiltrar ni borrar la PHI de otra —imposible por construcción, no por un sello que verificar—.
Reemplazó al sello `.instancia` TOFU (4 parches, retirado; el 4º concilio midió que dejaba dos
vías de borrado/fuga). La guarda de `restaurar` (`respaldo::phi_de_otra_instancia`) rechaza
pisar la PHI de otra instancia, fail-closed ante symlink y contenido no-`.dat`. LÍMITE
documentado: un clon vivo con el mismo uuid sobre el mismo dir (viola «una base por instancia»,
D26). Detalle del rediseño y los 6 concilios: `[[medico-retencion-supresion-defuncion]]`.

## La vista previa de imágenes DICOM (el módulo de píxeles)

Una placa DICOM el navegador no la pinta: se sube, cifra, guarda y descarga como
cualquier adjunto (`tipos::tipo_real` la reconoce por la magia `DICM`), pero para
VERLA había que bajarla y abrirla en software externo. `pixeles::previa_png`
(`crates/medico-api/src/pixeles.rs`) decodifica el pixel data del primer fotograma a
un PNG —en escala de grises o RGB— que el navegador sí muestra, aplicando al gris el
windowing Modality+VOI LUT que el DICOM declara (`dicom-pixeldata`, Rust puro). Lo sirve
`GET /api/imagenes/{id}/previa` —misma puerta de ámbito, bitácora Ley 1581 y
descifrado en reposo que `/archivo`—, y la lista de adjuntos de la historia muestra
la miniatura.

Reglas que no se rompen:
- **DICOM-only y FALLA CERRADO**: `/previa` renderiza solo DICOM; una transfer syntax
  fuera del allowlist (ver abajo), un espacio de color que no sea gris ni RGB (YBR_*,
  paleta), un DICOM corrupto o uno demasiado grande devuelven **404** y el panel degrada
  al enlace de descarga — nunca una imagen inventada (D20). El archivo clínico sigue siendo
  el DICOM original, cifrado e intacto; la previa es solo para leerla.
- **Los bytes los sube un tercero, y `previa_png` es el ÚNICO sitio de medico que
  PARSEA un DICOM subido: el tope anti-bomba cubre los TRES drivers de asignación**
  (concilio 2026-11-03, sonnet/fable/haiku lo midieron). (1) ALLOWLIST de transfer syntax
  leído del **file-meta ANTES de `from_reader`** (`transfer_syntax_acotable`): solo lo
  cuya salida fija el encabezado —no comprimido + RLE—; se descartan la familia JPEG
  (SOF con dimensiones propias, `jpeg-decoder` sin cota) y el **Deflated** (infla el
  dataset entero al parsear — bomba de descompresión que `from_reader` dispararía antes
  de cualquier otro tope). (2) GEOMETRÍA (`Rows×Columns×Frames×SamplesPerPixel` ≤ 96 M).
  (3) `BitsAllocated ∈ {8,16}` y `1 ≤ BitsStored ≤ BitsAllocated` —acota la LUT de
  `1 << BitsStored` del camino de 16 bits (1×1 con `BitsStored=30` forzaba ~1 GiB, 0/>32
  un panic)—. Los tres van ANTES de decodificar. Re-habilitar JPEG con una cota del SOF
  es tarea aparte.
- **16 bits (TAC/RM) SÍ se renderiza**: `force_8bit()` es no-op en
  `to_vec_frame_with_options`, así que se piden las muestras gris como `u16` y se bajan a
  `u8` con auto-contraste min–max propio (una vía para 8 y 16 bits).
- **Gris (MONOCHROME1/2) y RGB verdadero** (radiografía/TAC/RM/eco en gris + eco
  Doppler/endoscopia/captura a color). **YBR_* se descarga a propósito**: sin la feature
  `image`, `to_vec_frame_with_options` NO convierte el espacio de color, así que pintar
  un YBR como RGB daría color FALSO — entre eso y descargar, se descarga (lo discrimina
  `una_placa_ybr_no_se_renderiza_y_se_descarga`, mismo `spp=3` que la aguja RGB). Ampliar
  a YBR→RGB (conversión a mano) y a JPEG 2000 (feature `openjp2` = `jpeg2k`, OpenJPEG
  transpilado por c2rust: `unsafe` sobre bytes no confiables) es trabajo aparte y DECIDIDO
  fuera por ahora — el color RGB no tocó deps ni `deny.toml`.
- **La feature `image` de dicom-pixeldata NO se activa, a propósito**: forzaba los
  códecs exr/webp/tiff que no usamos, y `exr → pulp → paste` metía `paste` (NO
  MANTENIDO) + licencias extra en el binario clínico. Se decodifica con
  `to_vec_frame_with_options` (que ya hace Modality+VOI LUT) y el PNG lo escribe la
  crate `png`. `encoding` (charset del parser DICOM) queda como unmaintained
  tolerado con su `ignore` exacto en `deny.toml`/`.cargo/audit.toml`.

## La cadena RIPS → FEV-RIPS → MUV

```
       genera                    funde                     sube
Rust:  medico_dominio::rips  →  rips::fev            →  medico-muv     →  CUV
Python: rips_completo.py     →  fev_rips.py          →  muv_client.py  →  CUV
        (6 documentos)          (1 documento)           (contenedor de MinSalud)
```

**Las tres etapas están en Rust desde el 2026-08-10, y ENCHUFADAS**:
`/panel/rips` sube el archivo al validador del prestador y enseña el CUV, o la
lista exacta de campos que hay que corregir. El Python queda como ORÁCULO de las
pruebas de paridad, no como el camino.

**Las credenciales del prestador viven en el ENTORNO del proceso**, y no es una
decisión nueva: es la que ya tomó `ihce.rs` para el mismo problema (directiva 1,
un solo sitio donde buscar el mismo tipo de secreto). NO van a la base —la edita
un administrador desde una pantalla y viaja en cada respaldo—, ni tienen defecto:
**hacen falta las CINCO o no hay envío** (`MEDICO_MUV_URL`,
`MEDICO_MUV_TIPO_DOCUMENTO`, `MEDICO_MUV_DOCUMENTO`, `MEDICO_MUV_CLAVE`,
`MEDICO_MUV_NIT`; ver `.env.example`). El arranque **dice** en qué quedó, esté
configurado o no: un servicio que calla cuando no lo está se ve igual que uno
sano, y eso se descubre el día del cierre de mes.

**Y el NIT de las credenciales tiene que ser el de la institución que envía**, o
el envío se niega con 409. Con una sola configuración por proceso, un despliegue
multi-institucional podría radicarle a la EPS con el NIT del vecino sin que nada
fallara — y eso, una vez radicado, no se deshace. **Esa es la limitación que hay
que ver de frente**: es UNA configuración por PROCESO. Hoy alcanza porque es una
base por instancia (directiva 26); un despliegue que sirva a dos prestadores que
facturan necesitaría credenciales por institución, y eso es trabajo con
decisiones propias.

**El CUV y los hallazgos QUEDAN GUARDADOS** (`envio_muv`, migración `0055`): se
anota cada envío que llegó a MinSalud —no solo el último, porque un periodo se
reenvía tras corregir—, y `/panel/rips/revisar` los enseña. Solo entra lo que
SALIÓ, y el invariante «hay CUV si y solo si fue aceptado» lo sostiene un `CHECK`
de la base. **Un consecutivo que ya obtuvo CUV no se repite** (409 sin gastar el
viaje); uno solo rechazado sí, que es el camino normal de corregir y reenviar.

**El backend TLS es `rustls`** (decisión de Juan el 2026-08-10, con la medición
delante: era el único camino que no exigía `libssl-dev` en toda máquina que
compile, y sus dos licencias —ISC y CDLA-Permissive-2.0— entraron en `deny.toml`
con su motivo). El certificado autofirmado del contenedor se resuelve declarando
`MEDICO_MUV_CA`; no verificar exige escribir `MEDICO_MUV_TLS_INSEGURO=true`
exactamente, y el arranque lo grita.

- **`rips_completo.py`** arma la forma anidada `usuarios[] → servicios{}` y emite los
  seis documentos históricos (AC, AP, AM, AU, AH, CT).
- **`fev_rips.py`** los funde en el **documento único** de FEV-RIPS: un paciente con
  consulta y medicamento aparece una vez con las dos cosas. El CT se descarta — es del
  formato plano de la 3374. `null` donde antes iba `""`, fechas sin segundos.
- **`medico-muv`** (Rust) y **`muv_client.py`**: `login → cargar → CUV`. Las
  credenciales son **del prestador** —MinSalud confirmó en el caso 2026403888 que no
  existe credencial de integrador—, así que viajan en un objeto cuyo `Debug`/`__repr__`
  está **censurado a mano**: derivarlo bastaría para que un `{:?}` en un log publicara
  la clave. Las pruebas de los dos corren **sin red y sin contenedor** (el Python
  inyecta el transporte; el Rust levanta un `axum` en un puerto efímero, directiva 28).
  **El Rust NO habla HTTPS todavía**: entra sin backend TLS, igual que `medico-ihce`,
  y el contenedor real es HTTPS — declarar la feature mete `ring` en el `Cargo.lock`
  aunque esté apagada (medido), y eso es decisión de cadena de suministro aparte.
- **`validar_muv.py`** corre el ciclo completo a mano. Credenciales por entorno, nunca
  por línea de órdenes.
- **`muv/`** levanta el validador de MinSalud en local. Leer `muv/README.md` antes de
  tocarlo. **Solo el ambiente de stage**: en producción una factura inventada obtiene
  un CUV verdadero, con efectos jurídicos y financieros reales.
  **`muv/compose.produccion.yml`** levanta el de PRODUCCIÓN, que es otro registro
  y otra imagen (`crmspsgovcoprd` / `production-fevrips-apilocal`). Los nombres
  llevan `-prod` **a propósito**: el archivo de MinSalud usa `fevrips-api` para
  los dos, así que levantar el equivocado ocupaba el nombre y la aplicación
  hablaba con el validador de pruebas sin decirlo. Los dos publican el 9443, de
  modo que el segundo falla al enlazar en vez de robar el puerto.
- **`enviar-rips`** (`cargo run -p medico-muv --features cli --bin enviar-rips --
  ARCHIVO.json`) sube un FEV-RIPS ya generado, para cuando hay que radicar a mano
  y no desde la pantalla. Credenciales por ENTORNO, nunca por argumento —en la
  línea de órdenes quedan en el historial y en la lista de procesos—. Lee el BOM
  sin ahogarse, resume lo que va a mandar, **compara el NIT del archivo con el de
  las credenciales** y no envía hasta que se teclea «enviar». Guarda siempre la
  respuesta cruda en `ARCHIVO.json.resultado.json` —aceptada o no, que es la
  constancia de lo que contestó el Ministerio— y agrupa los hallazgos por campo
  con su `PathFuente`. Va tras la bandera `cli` para que `tokio` no entre en el
  crate por el camino normal.

Un **HTTP 400 con resultado dentro es un RECHAZO del archivo, no una avería del
cliente**, y hay que tratarlo como tal.

Cómo redacta la queja el MUV importa al leer un rechazo:

```
«valores permitidos entre 1 y 999»            → rango: no comprueba nada más
«no es válido, verificar tabla de referencia» → sí contrasta contra el catálogo
```

Un campo del primer tipo no es opcional: es el sitio más fácil para que se cuele un
dato falso sin que nadie se entere.

### El banco de campos de la 948 — las DOS mitades

`backend/tests/test_campos_fev948.py` y `crates/medico-dominio/tests/campos_de_la_948.rs`
comparan lo que cada archivo emite contra `datos/normativa/campos_fev948.json`,
extraído del Documento Técnico 1. **La aguja sale del artefacto, no de una lista
copiada**, y los dos bancos llevan su positivo de control: si el JSON no se
cargara, las comparaciones medirían contra el conjunto vacío y saldrían verdes.

| mitad | qué pasa si falla | desde |
|---|---|---|
| lo que **sobra** | el MUV rechaza el archivo ENTERO: «Propiedades no permitidas en la estructura» | julio |
| lo que **falta** | el campo puede ser opcional… o puede que no | **2026-08-12** |

La segunda no existía, y esa mitad ciega costó **21 campos**: E2 y E3 del
diagnóstico relacionado de egreso en urgencias y hospitalización (12), y nueve
del AM —su par CIE-11 entre ellos—. Todo el banco en verde mientras tanto,
porque un archivo al que le falta un campo devuelve lo mismo con la regla buena
y con la regla ciega.

**Lo único que falta a propósito es `codigoVIDA`**, y está declarado con su
motivo en los dos bancos: es el id único de la atención en el **sistema IHCE**, y
la norma dice que «será obligatorio una vez entre en operación». Hoy es opcional;
emitirlo vacío sería inventar un identificador que nadie asignó. Sale de la lista
cuando el transporte IHCE/RDA esté enchufado (#603) — es el amarre entre esa
ficha y el RIPS.

**Concentración, unidad de medida y forma farmacéutica no se rellenan a ciegas.**
La 948 los declara CONDICIONADOS: para la preparación magistral (`tipoMedicamento`
03) van los datos reales, y «para los otros tipos de medicamentos debe informar
0, 0 y null». Ese 0 es la regla escrita en la norma, no un supuesto; mandarlo en
una fórmula magistral sí sería afirmar una concentración de cero, así que ahí el
Recolector lo anota como falta y el campo sale vacío.

### CIE-11 (Resolución 948 de 2026)

Son **22 campos** (11 `cod*CIE11` + 11 `nomCod*CIE11`), no 35. **Y desde el
2026-08-12 se emiten los 22**: hasta entonces salían 18, porque el AM era el
único de los cinco archivos sin su par CIE-11 y a urgencias y hospitalización les
faltaban los diagnósticos relacionados de egreso 2 y 3. Este archivo afirmaba los
22 y el código entregaba 18 — ver «El banco de campos de la 948», arriba.

`backend/cie11.py` y `medico-dominio/src/cie11.rs` los derivan del CIE-10 — **con la misma regla y desde
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

## El RDA se contrasta contra el PERFIL, no contra el ejemplo (#603)

**La Guía de Implementación está vendorizada** en
`crates/medico-dominio/datos/ihce/ig/` — 206 artefactos FHIR bajados del sitio
del Ministerio el 2026-08-16, con su `sha256` en `MANIFIESTO.txt`. Cayó así el
bloqueo que la ficha daba por inalcanzable.

**El `package.tgz` NO existe**, y hay que recordarlo antes de volver a buscarlo:
esa ruta —y `definitions.json.zip` y `full-ig.zip`— devuelve el `index.html`
**byte a byte**. Un 404 blando, que un `%{http_code}` lee como éxito. La otra
ruta canónica, `fhir.minsalud.gov.co`, no resuelve por DNS. Por eso se bajaron
uno a uno.

**Uno de los 207 NO entró**: `Bundle-Output-ConsultarInmunizacion.json` lleva los
nombres de dos profesionales de la salud sin anonimizar. Ver `ig/LEEME.md`, que
explica también las **dos reglas** que hay que respetar al usar esto —un
`CodeSystem` incompleto no juzga códigos; los canónicos que faltan están
declarados— y que las dos tienen prueba en `tests/conformidad_rda.rs`.

**Lo que el banco de conformidad cambia**: hasta ahora las pruebas del
ensamblador comparaban contra el **ejemplo** oficial, y eso deja fuera todo lo
que el ejemplo no ejercita. Contrastando contra el `StructureDefinition` salió a
la primera que `Composition.event` es obligatorio con **dos** códigos y un
`period` —medico emitía uno y sin periodo, **en los dos ensambladores**, cada uno
con su copia del bloque—.

**Y sus cifras se leen las cuatro, siempre.** Desde el 2026-08-22 el banco
**RESUELVE los discriminadores** de las rebanadas (`value`/`pattern`, en
`evaluar_rebanada`): el RDA de paciente pasó de `14 evaluadas · 34 SIN EVALUAR` a
**`48 evaluadas · 0 incumplidas · 0 sin evaluar`** — conforme a nivel de slice.
Los TRES RDA clínicos también se miden ya (`los_rda_clinicos_contra_su_perfil`).
El déficit era `30/37/35 incumplidas`, bajó a `3/4/2` (puerta del `nilknown` en
`clinico` + un falso positivo del banco) y **desde FASE 4 (2026-08-29) es
`0/0/0`**: los tres RDA clínicos son **estructuralmente conformes a nivel de la
`Composition`**. Fase 4 construyó los recursos que las secciones referencian
(`ihce::recursos` + `ihce::clinico`): `Composition.encounter`→**`Encounter`**,
`sectionProblems`→**`Condition`** (CIE-10 + par CIE-11 derivado),
`sectionPayers`→**`Organization` EAPB** (*no* `Coverage` —el perfil pide una
Organization—), `sectionAddendumDocuments`→**`DocumentReference`** de la epicrisis
(casi todo fijo, *no* «adjuntos»). **Cada recurso se contrasta contra SU perfil**
(`los_recursos_clinicos_contra_su_perfil`), no solo la referencia: emitir una
referencia a un `Encounter` no conforme es un verde falso (D28/D33). Para medirlos,
el banco (`evaluar_rebanada`) se amplió a rebanadas ANIDADAS
(`diagnosis:MainDiagnosis.extension:ExtensionDiagnosisType`) y a discriminadores
sobre CodeableConcept complejo (`participant.type`) y sobre extensión
(`url`→`type.profile`).

**Lo que fase 4 NO cierra en PRODUCCIÓN, y está medido (D20)**: el `Encounter`
exige el **entorno de atención**, que YA está cableado como config del prestador
(`MEDICO_ENTORNO_ATENCION`, default `05` Institucional) — no es hueco. La
**modalidad de atención** de urgencias/hospitalización YA no bloquea: el dominio la
**DERIVA** (internación/urgencias → intramural `01`) **solo cuando ese entorno es
`05` Institucional** — porque ahí la cama física / la urgencia presencial la fijan;
si el prestador se despliega extramural (Hogar, Comunitario…), NO se deriva y vuelve
a Falta, para no emitir «Hogar + Intramural», un dato falso al visor (lo cazó el
concilio, 2026-09-17). *(El entorno es config por PROCESO, no por institución —igual
que las credenciales MUV—: un binario multi-institución con sedes de distinto entorno
no está cubierto; límite del modelo «una base por instancia», no de este cambio.)*
Consulta SÍ la captura (`historia`) porque varía. El
**tipo de diagnóstico** de ingreso/egreso —que el PERFIL RDA exige
(`ExtensionDiagnosisType`, min=1) aunque la Resolución 948 no lo pida para AU/AH—
**YA se captura desde la migración 0075** (`hospitalizacion` y `atencion_urgencias`,
en `ingresar`/`egreso`/urgencias-`egreso`, **por API; aún sin pantalla `maud`**).
Lo que sigue faltando para emitir hosp/urgencias son los **avales del `nilknown`**
y, en urgencias, el **diagnóstico de egreso** (write-only). El ensamblador exige
los que falten como `Faltas` y no los inventa; las fixtures los traen del ejemplo,
así que la conformidad medida es ESTRUCTURAL. Ver PENDIENTES #603 (F4.5+). **Sin
evaluar sigue leyéndose distinto de conforme**: lo que no se pudo resolver se nombra.

## La puerta del `nilknown`: no se afirma «Nil Known» por defecto de máquina (#603)

Decidido por Juan el 2026-08-23, y es una regla que no se rompe. Una sección
vacía del RDA se emite con `emptyReason=nilknown` **SOLO si un clínico la
confirmó revisada**; si nadie la confirmó, el ensamblado FALLA (`Faltas`), en vez
de afirmar «sin hallazgos». `nilknown` es, por definición FHIR, una AFIRMACIÓN de
un clínico, no el defecto de una máquina; y el perfil de MinSalud **fija** la
razón a `nilknown`, así que la única salida honesta es el aval humano por sección
(o poblar la sección, o no emitir el RDA). El riesgo que cierra: afirmar «sin
alergias conocidas» de todos los pacientes solo porque medico aún no puebla las
secciones — un médico en otra institución podría recetar un fármaco al que el
paciente sí es alérgico.

Vive en `ihce::seccion` (compartida por los cuatro documentos, directiva 9) y se
alimenta de `Insumo.aval`. **Tres requisitos DUROS de la revisión de seguridad del
2026-08-23; los tres HECHOS** (F4.5 A+B, 2026-09-03):

1. **HECHO — la confirmación es aval humano imposible de falsificar por
   comodidad.** Ya no es un `&[&str]` de códigos: es `ihce::Aval`, un slice de
   `Confirmacion { seccion, usuario_id, sello }` con un solo constructor con
   contenido — «pasar los códigos» sin nombrar quién y cuándo **no compila**
   (como `procedencia.rs`; es de tipos + un solo sitio de construcción, no
   criptografía, y así lo dice su docstring sin sobrevender).
2. **HECHO — cada `nilknown` deja rastro de QUIÉN lo confirmó y CUÁNDO**: la
   bitácora `aval_seccion_rda` (migración 0072, `medico_datos::rda_aval`) guarda
   una fila atribuida por (historia, sección); el productor la LEE para construir
   el `Aval`, y la ruta de captura registra además el acto en la cadena `acceso`
   (no una segunda cadena — la que ya existe).
3. **HECHO — el RDA clínico porta la MISMA puerta** desde el 2026-08-29 (una sola
   `ihce::seccion` para los cuatro documentos).

**F4.5-C — HECHA Y DESPLEGADA** (main `3feb19ca`, 2026-09-04, verificada en el VPS:
raíz 303, `/panel/rda/consulta/{id}` 303 sin sesión = ruta viva, servicio active;
punto de vuelta `20260904-100639`). La PANTALLA del `nilknown` (`vista/rda.rs`,
`/panel/rda/consulta/{id}`) prepara el RDA de una consulta y AVALA sus secciones
vacías por casilla: GET dibuja (`estado_de_preparacion` → secciones avalables +
otras faltas), POST fija el aval (`rda_aval::fijar_avales`) con el usuario de la
SESIÓN (nunca del formulario) y los códigos filtrados server-side; registra el acto
en la cadena `acceso`. Enlace de entrada desde la historia. **Concilio de cierre: 4
modelos (opus/sonnet/fable/haiku).** Sonnet cazó (conf 9, verificado por ejecución)
que el upsert `DO UPDATE` reatribuía secciones que otro clínico avaló al reenviar el
`<form>`; corregido a `ON CONFLICT DO NOTHING` (la atribución la conserva quien
confirmó primero; re-atestar exige desavalar+avalar), con su pareja roja probada por
mutación. **La captura ya ESCRIBE en `aval_seccion_rda`**, así que un RDA sale
`Listo` cuando el clínico avala sus secciones vacías (antes devolvía siempre
`Faltan(emptyReason)`).

**F4.6 — EMITIR (el puente de encolado), HECHO, MERGEADO Y DESPLEGADO INERTE**
(main `ca21f84e`, 2026-09-07; concilio de cierre 4 modelos limpio; desplegado a la
demo con `cola IHCE: inactiva` confirmado en el journal). Avalar ≠ enviar: **emitir es un acto
aparte** con su ruta, `POST /panel/rda/consulta/{id}/emitir` (`vista::rda::emitir`).
Corre `rda_encuentro::preparar_y_encolar`, que si el RDA está `Listo` lo ENCOLA en
`envio_rda` (`cola_rda::encolar`) y si faltan datos NO encola nada (D20). No
transmite en línea: la tarea de fondo `ihce::correr_cola` drena la cola. **La
acción está cerrada tras `Estado.ihce_configurado`** (`MEDICO_IHCE_*` presentes): sin
credenciales de sandbox la pantalla DICE que no está configurado y no ofrece el
botón —encolar algo que nadie drenaría sería un falso «enviado»—, así que en la demo
y en cualquier instalación sin credenciales el binario **no cambia de comportamiento
observable**. La mitad de DRENAJE (`main.rs` lanza `correr_cola` solo con config)
ya estaba. Lo ÚNICO que sigue bloqueado es externo: las credenciales de sandbox del
prestador (no hay credencial de integrador). El código y el despliegue de F4.6 están
cerrados; el día que existan credenciales, ponerlas en `/etc/medico-demo/medico.env`
(MEDICO_IHCE_*) es lo que enciende la emisión — no hace falta redeploy de código. El nombre de
operación con que la cola persiste la fila y con que el transporte la decodifica lo
amarra un caso rojo en `medico-api` (`la_operacion_encolada_la_decodifica_el_transporte`).

**PIEZAS 2-4 — RDA de HOSPITALIZACIÓN y URGENCIAS + pantalla genérica — HECHAS Y
MERGEADAS a `main` (`73d6ff42`, 2026-09-17; concilio de cierre 4 modelos).** El RDA
dejó de ser solo consulta externa:
- **Aval del `nilknown` genérico por encuentro** (migración `0074`): `aval_seccion_rda`
  se llavea por `(tipo_encuentro, encuentro_id)` en vez de `historia_id`. El
  invariante no cambió; se extendió a los tres tipos. `rda_aval` es genérico y su
  aislamiento usa el MISMO `*::obtener` base-paciente que el productor (una sola vía).
- **Productores** `preparar_hospitalizacion` y `preparar_urgencias` (`rda_encuentro.rs`),
  llaveados por id del encuentro, con guarda de ámbito del encuentro y del profesional.
  Reúsan `muv::via_ingreso_de_hospitalizacion`/`condicion_destino_*` y
  `actores::fallecido_de_*`; el pagador sale de `paciente.entidad_id`.
- **Pantalla genérica** `/panel/rda/{tipo}/{id}` (`vista/rda.rs`), con `tipo ∈
  {consulta, hospitalizacion, urgencias}`; despacho por tipo en `estado_de_preparacion`/
  `preparar_y_encolar`. Enlaces «Preparar RDA» en las tablas de hospitalización y urgencias.
- **Son MEDIDORES honestos, como consulta lo fue antes de F4.5**: el RDA de hosp/urgencias
  NO emite todavía, pero la **modalidad de atención** ya no es el freno: el dominio la
  **DERIVA** (`modalidad_derivada`: internación/urgencias → intramural `01`) **solo con
  entorno `05` Institucional**; con un entorno extramural (Hogar…) NO deriva y es Falta,
  para no emitir «Hogar + Intramural» (hallazgo del concilio 2026-09-17). Consulta la
  captura porque varía (verificado por mutación en las tres direcciones: no-derivar,
  derivar-en-consulta, ignorar-entorno). El **tipo de
  diagnóstico** de ingreso/egreso —que el perfil RDA exige— **YA se captura** (migración
  0075: `tipo_diagnostico_ingreso`/`_egreso`; el productor lo pasa y la Falta desaparece al
  registrarlo, verificado por mutación). Lo que aún frena la emisión son los **avales del
  `nilknown`**. Para URGENCIAS queda además el diagnóstico de
  EGRESO, que es **write-only** (la tabla lo guarda desde la 0042 pero `Atencion`/`COLS` no
  lo leen y el productor lo fija en `None`): su `DischargeDiagnosis` (min=1 en el perfil AU)
  sigue faltando — hueco aparte. El **autor** del RDA de urgencias es la enfermera de triage
  (único clínico que la tabla captura) — decisión de producto a confirmar antes de que
  urgencias deje de ser inerte.

**Los `display` se DERIVAN del catálogo** (`ihce::terminologia`), no se reciben.
El servidor valida `system`+`code` y se traga el `display` que le manden, así que
ese campo es justo donde un dato falso sobrevive — la lección del `tableta = 1`.
Un código que el CodeSystem no tenga **no se emite**.

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
| `../crates/medico-datos/datos/unidad_dispensacion.json` | los 73 códigos M13, **copia verbatim** de `maestros_muv/upr.json` | se copia; no se transforma |
| `../crates/medico-dominio/datos/talla_ideal.json` | las 62 filas de `PESOTALLA1.DBF` del VFP (talla y peso ideal por edad y sexo) | `../herramientas/generar_talla_ideal.py` |
| `normativa/campos_fev948.json` | campos de la 948 | `../herramientas/extraer_campos_fev948.py` |

**Ningún código de catálogo se escribe a mano.** El 2026-07-26 se inventó un mapa de
presentaciones (`tableta = 1`, cuando el catálogo dice 66 y el 1 es AMPOLLA); el MUV lo
aceptó y devolvió CUV, porque ese campo lo valida por rango. Los maestros salen del
contenedor del validador, que es la fuente contra la que el MUV juzga.

**Ese caso tiene ya su prueba y su maestro** (2026-08-11). `unidad_dispensacion`
llevaba desde la migración 0010 creada y VACÍA —«un catálogo vacío da un selector
vacío, honesto, y no un valor inventado»—, y el selector de la fórmula llevaba
igual de vacío todo ese tiempo: honesto, sí, e inservible. Ahora se siembra en
cada arranque como los otros cuatro, desde `upr.json`, y
`la_tableta_no_es_la_ampolla` se pone roja si alguien vuelve a numerar por su
cuenta. La cuenta y el orden no bastan como prueba: una lista inventada de 73
filas ordenadas las pasa las dos: lo que discrimina es contrastar códigos
concretos contra el maestro y exigir que el archivo declare de qué tabla del
validador salió.

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

El sistema predeterminaba valores porque el clínico tiene doce minutos por
paciente: `cod_consulta_cups="890201"`, `estado_paciente_salida="ESTABLE"`,
`valor_servicio=0`. Datos que nadie afirmó, indistinguibles de los reales. Pero
quitarlos sin poner nada en su lugar solo hace la captura más lenta, y **lo lento
se rodea**: se cambia un dato falso por uno basura, con más fricción.

La salida es marcar el origen: `CONFIRMADO` · `DERIVADO` · `PROPUESTO` ·
`NO_APLICA`. **Está en Rust desde el port**, en `medico-dominio/src/procedencia.rs`
—que es el de `procedencia.py` + `auditoria_rips.py` juntos—, y se guarda CON el
valor en la columna `procedencias` (JSON `{campo: procedencia}`), no en una tabla
aparte.

**Las derivaciones YA existen** (`medico-dominio/src/derivaciones.rs`, port de
`derivaciones.py`, 2026-08-21): un dato que la máquina calcula de otros que ya
estaban se marca `DERIVADO`. La primera es el **`cod_municipio` desde el texto
libre de la ciudad** del paciente (`derivar_municipio` en `medico-datos`), que
los pacientes del VFP (#16/#65) traen sin código. `municipio_desde_texto` es puro
y devuelve `None` cuando el nombre es ambiguo (varios «San Juan») —no aproxima
(directiva 20)—; la búsqueda de candidatos pliega tildes en el DOMINIO, no en el
`LIKE` de la base (con él «Medellin» no encontraría «Medellín»). No pisa un
código que un humano tecleó: eso queda `CONFIRMADO`.

**Y el municipio DERIVADO por fin dispara `DuroSinAval`.** El bloque de
identificación del RIPS (`documentos::usuario_base`) auditaba los campos del
paciente contra `None` —«incremento futuro»—; ahora los audita contra
`paciente.procedencias` (columna nueva, migración `0069`). Como el municipio es
DURO, uno que puso la máquina y nadie miró sale marcado «duro sin aval» en el
informe de cierre en vez de pasar por cierto. La captura del paciente marca
`CONFIRMADO` lo que teclea una persona (el formulario **es** el toque), así que
solo el derivado queda sin aval.

**La fricción no se opina, se calcula, y va en la dirección segura**: `friccion_de`
enumera lo *provablemente administrativo* (cuatro campos) y manda todo lo demás
—lo que MinSalud cruza con sus reglas `RVC*`, identidad, dinero, autorización, y
lo aún no clasificado— a fricción DURA. Un campo nuevo nace protegido.

Y la auditoría **dejó de inferir**: `Recolector::en_registro` consulta el JSON al
generar el RIPS y anota dos cosas —el valor sin procedencia (la firma del dato por
defecto) y el campo duro respaldado solo por un `DERIVADO`—. No bloquea: sale en el
informe de cierre, para quien firma.

**Dónde está cableada hoy, medido**: la sección de **diagnóstico** de la consulta
(siete campos, casilla por campo) y la **fórmula** (cuatro campos, UNA casilla por
receta, migración `0060`). Las otras tres fuentes del RIPS —`orden`,
`atencion_urgencias`, `hospitalizacion`— llevan `procedencias: None` a propósito y
no por descuido: **ninguna pantalla captura sus campos RIPS todavía** (urgencias y
hospitalización son pantallas de solo lectura, cero formularios), y sin valor no hay
nada que respaldar. Cuando nazca su captura, nace con su procedencia. `tarifador.py` resuelve `valor_servicio` desde `Contrato →
TarifaContrato` y lo **congela al prestar el servicio**: si el informe de marzo se
generara con la tarifa de hoy, dos ejecuciones del mismo periodo darían cifras
distintas y ninguna sería la pactada.

**Regla dura**: nada que sugiera un modelo se escribe solo en la historia clínica.
Propone, un humano confirma, y queda registrado cuál fue cuál.

Los cuatro casos ya corregidos —finalidad, causa externa y tipo de diagnóstico
(2026-08-02) y el documento del profesional P11/P12 (2026-08-03)—, con el patrón
que los hacía invisibles, están en [`DETALLE.md`](DETALLE.md).

## Fidelidad al sistema original

`antropometria.py` y `epidemiologia.py` migran fórmulas de VFP **literalmente**,
aunque parezcan mejorables: si los números no coinciden con los que el prestador
viene reportando, el informe nuevo no sirve. Las mejoras van en una etapa aparte,
y así se anota en el código. La única desviación deliberada es el **cálculo de la
edad**: el original usaba años de 365 días y meses de 30, lo que envejece a la
gente, y en pediatría el tramo «0 años» va del recién nacido al de once meses.

**El PESO IDEAL ya está en Rust** (`medico_dominio::antropometria::peso_ideal`,
2026-08-11): `talla² × factor × 100`, con 0,23 para hombre y 0,22 para mujer, y
nada por debajo de los 12 años. Lo deriva el servidor al guardar el examen
físico, en la misma línea que el IMC y con la misma regla —sin talla no hay peso
ideal, y el anterior NO se queda al lado de una medición nueva—. Se contrasta
valor a valor contra `backend/antropometria.py`, que es el oráculo.

Dos rarezas que son FIDELIDAD y no descuidos, y por eso tienen prueba con su
nombre: un sexo que no empieza por «M» cae al lado femenino (el
`IF Lsexo='M' … ELSE` del original), y a los 12 años todavía no calcula —empieza
a los 13—. Lo que sí se desvía: devuelve «no se sabe» donde el original devolvía
**0**, que en un campo de peso se lee como una medición.

**La TALLA ideal ya está** (2026-08-12, `antropometria::talla_ideal`, migración
`0057`), y con ella se cierra la #104. No es una fórmula sino una BÚSQUEDA en
`PESOTALLA1.DBF` —62 filas, EDAD + TIPO + SEXO, transcritas por
`herramientas/generar_talla_ideal.py`—, y ahí estaba el hallazgo: en el original
**nunca funcionó**, porque el formulario abre `PESOTALLA`, que no tiene columna
EDAD ni ese índice, así que el `SEEK` no puede acertar y la talla ideal sale
siempre en 0.

**Es la única pieza del port sin número del prestador con el que cuadrar**, y por
eso la advertencia va en los cuatro sitios donde alguien se haría la pregunta: la
migración, el JSON, `antropometria.rs` y la prueba. No es migración fiel de un
comportamiento; es la tabla que su producto traía y no llegó a consultar.

**La edad entra en MESES**, y no es un detalle de firma: la tabla trae una fila
por CADA mes del primer año, así que resolverla en años devolvería la talla del
recién nacido para un niño de siete meses. `pacientes::edad_meses` vive al lado
de `edad` y no se deriva de ella. Y no se interpola: 42 meses caen en la fila de
3 años, no entre la de 3 y la de 4.

**Las dos referencias son COMPLEMENTARIAS, no gemelas**, y la pantalla del examen
enseña la que haya: la fórmula del peso ideal no aplica por debajo de los 13 años
y la tabla de tallas se acaba a los 19, de modo que casi nunca salen las dos. Un
renglón que dijera «talla ideal: —» se leería como si el sistema hubiera
calculado algo.

El contraste campo a campo de los antecedentes contra la pantalla del sistema
vivo (2026-08-03), con los doce campos que faltaban y los dos que NO se portaron a
propósito, está en [`DETALLE.md`](DETALLE.md).

## Frontend y despliegue

**No hay frontend**, y esa es la descripción exacta desde el 2026-08-08: las 29
pantallas se tejen en el servidor. El despliegue es **un archivo**: `./construir.sh`
es un solo `cargo build --release`, y htmx, `boi.js` y `panel.css` van con
`include_str!` desde `crates/medico-api/panel/`, que está versionado.

**No hay comodín que sirva el SPA**: `/` redirige a `/panel` y **lo que no case es
un 404**. Antes `GET /api/pacintes` con la errata devolvía HTML con 200 donde el
cliente espera JSON. Lo sujeta `la_raiz_lleva_al_panel_y_lo_que_no_existe_es_404`.

## Una historia FIRMADA no se edita por ninguna puerta

La regla vive en **`historia::admite_escritura`** (`medico-datos`), no en los
manejadores, y por ahí pasan las **doce** puertas —seis del panel y seis de la
API— que cuelgan o descuelgan algo de una historia: fórmulas, órdenes y
adjuntos. Devuelve `Firmada`, `NoExiste` o `Hecho`, y el orden importa: **lo
ajeno da `NoExiste` aunque esté firmado**, porque saber que una historia está
firmada sería un oráculo de que existe (404 y nunca otra cosa).

Antes estaba escrita en prosa y aplicada a medias: de las seis puertas del panel
la cumplían tres y de las seis de la API **ninguna** —la API dejaba **añadir** un
medicamento a una historia firmada, que es el ejemplo con el que esta misma regla
se enuncia—. Lo midió la revisión de seguridad del 2026-08-10.

Los dos enums que la expresan son hermanos y la distinción es el punto:
`Resultado { NoExiste, Firmada, Hecho }` para editar y borrar, `Creacion {
Creada(id), NoExiste, Firmada }` para las altas. **`Firmada` no es `NoExiste`**:
existe, es del ámbito, y está cerrado.

**Lo que NO se cierra, y son DOS**: `orden::cambiar_estado` —marcar una orden
como procesada es flujo posterior, no editar el documento— y la **placa que sube
imagenología**. Esa segunda salió de cerrar de más: al principio se bloqueó
también, y con eso el radiólogo no podía adjuntar la placa de una consulta ya
firmada, que es el caso normal. Se distinguen con `imagen::Origen` (migración
`0056`): el adjunto que el médico pone en la historia es contenido del documento;
el resultado de imagenología llega después y no lo es. Directiva 21 las dos.

**Y la firma CUBRE los anexos desde el 2026-08-10**, que es la otra mitad y se
hizo el mismo día. `firmable` sellaba solo la fila `historia`, así que borrar un
medicamento de una historia firmada dejaba `verificar-firma` en `Valida`. Ahora
entra el resumen de las tres tablas de contenido clínico —`formula`, `orden` e
`imagen_diagnostica`— bajo la clave `__anexos`, con `ORDER BY id` y columnas
fijas para que firmar y verificar produzcan los mismos bytes.

**Las dos capas son distintas y hacen falta las dos**: `admite_escritura` impide
la alteración **desde el producto**; la firma la **detecta** cuando se hace
contra la base o sobre un respaldo — que es la vía por la que llegaría una de
verdad, y la que responde ante un tercero (Ley 527/1999).

**Lo que NO entra, y es la pareja de la regla**: `documento_clinico` tiene su
propia firma; `consentimiento` y `factura` cuelgan de la historia por
conveniencia y no son el acto clínico que se firma. Una firma que se invalidara
porque después se emitió una factura sería ruido, y el ruido enseña a ignorar la
verificación.

**Se hizo el 2026-08-10 y no antes por una razón que caducaba**: había **cero
historias firmadas** en la demostración y ninguna instalación de cliente
—medido—, así que el cambio no invalidó nada. Con una sola firma real por medio,
habría costado invalidar documentos con efectos jurídicos.

## Una pantalla COMPARTIDA se abre con cualquiera de sus licencias y se escribe con UNA

Doce de los módulos de #249 no tienen pantalla propia: cuatro comparten
`/panel/enfermeria`, cuatro `/panel/pyp`, cuatro `/panel/habilitacion` y dos
`/panel/anestesia`. Eso es deliberado —son un solo momento de trabajo— y obliga a
**dos puertas, no una**:

| | quién decide | dónde |
|---|---|---|
| **abrir** la pantalla | cualquiera de sus licencias | `licencia::exige`, que devuelve **todos** los reclamantes |
| **escribir** | la licencia del módulo cuya TABLA se toca | `vista::exige_licencia(&e, &u, "MAR")` en el manejador |

**La capa no puede suplir la segunda: ve la RUTA, no la TABLA.** `POST
/panel/enfermeria` escribe en `administracion_medicamento`, que es `MAR` y solo
`MAR`, y la ruta es la misma que abre el kardex.

Lo cazó la revisión de seguridad del 2026-08-16 antes de integrar #249, y fallaba
**en las dos direcciones sin dar un error**: `exige()` resolvía la rama de
`PANTALLAS` con `.find()`, o sea el primero de la lista, así que quien compró
`MAR` y no `ENFERMERIA` recibía 403 en la pantalla que pagó, y quien compró
`ENFERMERIA` y no `MAR` escribía en la hoja de medicamentos que no compró. Sus
casos rojos: `una_pantalla_compartida_la_reclaman_todas_sus_licencias` y
`la_pantalla_compartida_se_abre_con_una_licencia_pero_no_se_escribe_con_otra`.

**Una pantalla nueva bajo una ruta compartida nace con su `exige_licencia`.** Y
`ninguna_pantalla_vive_dentro_de_otra` obliga a decidir el día que alguien anide
una: hoy `exige()` recoge todos los reclamantes con un `filter` simple, y con
anidamiento la licencia del padre abriría la hija.

## La FK que llega del cliente se comprueba: `pertenencia::es_del_paciente`

Un módulo clínico valida su `paciente_id` contra la institución y acto seguido
ata en crudo la clave foránea que le mandaron —la hospitalización, la atención de
urgencias, la inscripción de P&P—. Esa segunda mitad **faltaba en cinco sitios**,
y la misma revisión los encontró en bloque.

En cuatro el daño era una fila de la institución A apuntando a una de la B
(integridad referencial cruzada, sin fuga: ninguna consulta de lectura las une).
En el quinto —`defuncion::certificar`— el valor además se LEÍA, y la diferencia
entre `MurioAntesDeIngresar` y el resto convertía la omisión en un **oráculo**:
por bisección sobre `fecha_defuncion` se reconstruía la hora de ingreso de un
paciente de otro prestador, y la rama que porta la señal retorna antes del
`INSERT` — sonda gratis y repetible. Era **un defecto mirado por dos lados**, y el
lado callado no se queja nunca.

La regla vive en `medico_datos::pertenencia` y no en cinco copias (directiva 23:
el arreglo va en la regla). `Enlace` es un enum y no el nombre de la tabla en una
cadena, porque un nombre de tabla no puede ir por `bind`: se elige la consulta
entera con un `match` y no queda nada que inyectar. **No recibe el ámbito a
propósito** — con el paciente ya acotado, «es de este paciente» implica «es de
esta institución»; quien lo use sin acotar antes el paciente no está acotando
nada, y por eso el parámetro se llama `paciente_id`.

## Convenciones

- **`db.commit()` no es automático** en SQLModel.
- **Los dos sistemas migran distinto, y se confunden:**
  - **Python, sin Alembic**: añadir a la lista `sql_migrations` de
    `database.py:_run_migrations()` (hoy 52 entradas, todas `ALTER TABLE`), con
    `ADD COLUMN IF NOT EXISTS`. Corren en cada arranque y son idempotentes.
  - **Rust, con `sqlx::migrate!`**: un archivo nuevo y numerado en
    `crates/medico-datos/migrations/` (hoy 73, de `0001_fundacion.sql` a
    `0073_facturacion_de_la_dispensacion_otros_servicios.sql`). Las aplica `medico-datos/src/lib.rs:74` al abrir
    la base. Son **inmutables**: `sqlx` guarda el hash de cada una, así que editar
    una ya aplicada rompe el arranque contra una base existente — se añade otra.
- **Enums**: todos heredan `str, Enum`. Comparar por miembro (`estado ==
  EstadoCita.PENDIENTE`), no por cadena.
- ~~**Router nuevo**~~: los `routers/` se retiraron el 2026-08-09 (#598). Un
  endpoint nuevo se escribe en el binario Rust (`crates/medico-api`), no en Python.
- **Aislamiento**: toda consulta de paciente/historia/cita filtrada por institución, y
  todo acceso por id vía `obtener()`.
- **N+1**: `joinedload()` / `selectinload()`. El pool son 20 conexiones.
- Los `.py` de la raíz (`models.py`, `database.py`, `seed.py`, `seguridad.py`,
  `importer.py`, `reset_db.py`) son **copias antiguas de la etapa NiceGUI**,
  idénticas o casi a las de `backend/`. El código vivo es el de `backend/`.
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

- **Ley 1581/2012** — la bitácora VIVA es `medico_datos::acceso`, encadenada por
  hash (ver arriba). En el Python quedan los modelos `AuditoriaAcceso` y
  `AuditoriaSuperAdmin`; sus atajos (`auditoria.py`) se podaron el 2026-08-10 con
  la API que los llamaba.
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

### Residuo de la revisión del DESPLIEGUE (2026-08-16, tarde)

Los dos revisores del punto de vuelta dejaron esto por debajo del listón. Lo
arreglado ya está en `desplegar-demo.sh` y `volver-atras.sh`; esto es lo que
queda vivo.

| Dónde | Qué |
|---|---|
| El punto de vuelta entero | **Vive en el mismo disco, la misma máquina y con el mismo dueño que aquello que protege.** `respaldos/` es `medico:medico`, así que el propio servicio puede borrarlo todo. Protege de una migración mala —que es para lo que se hizo— y de nada más: ni de un compromiso de la aplicación, ni de la pérdida del VPS. **No hay ninguna copia de la base clínica fuera de esa máquina**: el respaldo cifrado de la casa cubre espejos y configuración, no el VPS. Hoy es la demo (datos de demostración); el día que haya una instalación de cliente, esto pasa a ser lo primero |
| `/opt/medico-demo/*.bak-*` | Quedan **6 `medico.db.bak-*` y 3 `medico-api.bak-*`** del esquema anterior, sueltos en el directorio. Los de base están **incompletos por construcción** (se tomaron con `cp` sobre una base en WAL), así que parecen una red y no lo son. Se les cerraron los permisos a 0600/0700 el 2026-08-16; **borrarlos es decisión aparte** y no la tomo yo: los creó el guion viejo, no esta sesión |
| `MEDICO_DIR_IMAGENES` | No se declara en `/etc/medico-demo/medico.env`, así que el binario cae al defecto `storage/imagenes` **relativo a su directorio de trabajo**. Hoy coincide con `/opt/medico-demo/storage/imagenes`, que existe y está vacío —comprobado, y la base tiene 0 filas con `ruta_archivo`, o sea que el «0 imágenes» del respaldo es cierto y no un falso negativo—. Pero depende de que el `WorkingDirectory` de la unidad no cambie nunca: declararlo explícitamente cuesta una línea |
| `restaurar` en el guion de vuelta | Se le pasa `--forzar` porque la base de destino tiene datos y sin la bandera se niega. Esa negativa es correcta y por eso no se le quita; lo que no hay es una confirmación interactiva antes de pisar la base de producción. Hoy la protección es que el operador teclea la marca a mano |

### Residuo de la revisión de seguridad del 2026-08-16 (bajo el listón)

Los cuatro revisores de #249 los dejaron por debajo del listón de «hallazgo» y
NO se arreglaron. Se escriben aquí porque esa sesión no tenía `TaskCreate` y, sin
esto, vivirían solo en una conversación cerrada. **Ninguno es una vulnerabilidad
conocida; todos son reales.**

| Dónde | Qué |
|---|---|
| `banco_sangre::tamizar` | **Resucita una unidad DESCARTADA**: no mira el estado actual, así que `tamizar(id, conforme=true)` sobre una bolsa descartada por tamizaje reactivo la devuelve a `DISPONIBLE`, con su `motivo_descarte` huérfano al lado. Contradice el invariante que el propio módulo declara —«que la que no pasó el tamizaje no se transfunda nunca»—. Y ni el veredicto ni su reversión registran quién ni cuándo: en el módulo cuya razón de ser es la trazabilidad de una bolsa, el acto que decide si entra en un paciente es el único sin firma |
| `banco_sangre::transfundible` + `0068:130` | **No componen como dice la documentación**: el código gatea por tamizaje, no-transfundida, no-descartada y no-vencida, pero **nunca exige `estado = 'DISPONIBLE'`**; y el `CHECK` cubre `DISPONIBLE/RESERVADA/TRANSFUNDIDA` y deja fuera `EN_CUARENTENA`. Una fila `EN_CUARENTENA` con `tamizaje_conforme = 1` es legal en la base **y** transfundible en el código. Hoy ningún camino del producto la produce; la cabecera del módulo promete que el `CHECK` protege «contra un importador que no pase por ahí», y contra ese importador la promesa es falsa |
| 4 transiciones sin autor | `banco_sangre::anotar_reaccion` (el dato que después se investiga), `habilitacion::notificar_iaas` («notificado al sistema nacional» sin decir quién lo afirma) y `anestesia::dar_salida` (reutiliza el `responsable_id` de la recepción, así que quien firma la salida queda registrado como quien recibió). Y cuatro tablas institucionales nacen sin columna de autor: `equipo_biomedico`, `comite`, `sesion_comite`, `turno_personal` |
| Fechas comparadas como CADENAS | `Unidad::vencida` hace `fecha_vencimiento < hoy` sobre texto sin validar formato, así que un `"2026-8-9"` sin cero de relleno compara mayor que `"2026-08-16"` y una bolsa vencida pasa por fresca **indefinidamente**. Igual `fecha_defuncion` y el `BETWEEN` de indicadores. **Es patrón de la casa, no regresión** —`inventario.rs:641` hace lo mismo—, así que merece una tarea transversal: un validador `AAAA-MM-DD` en `medico-dominio` aplicado en el borde |
| `recien_nacido` | Sin camino de corrección ni de anulación —no tiene columnas `anulada*` ni un solo `UPDATE`/`DELETE`—, al revés que `defuncion` y `administracion_medicamento`. Un registro erróneo que alimenta el registro civil por `codigo_vida` es hoy inmodificable. Falla cerrado, pero es una asimetría probablemente no querida |
| `creado_por` nulable | En `recien_nacido` (0062:92) e `inscripcion_pyp` (0066:57), mientras los autores equivalentes son `NOT NULL` en el resto del lote. Los dos llamantes pasan siempre `Some(u.id)`, así que **la única barrera es el código, no el esquema** |
| `transfusion` | `INSERT` + `UPDATE unidad_sangre` en dos pasos **sin transacción**: una caída entre los dos deja la transfusión registrada y la bolsa marcada como disponible. El `UNIQUE` sobre `transfusion.unidad_id` impide el daño clínico real |
| `censo` | El `LEFT JOIN hospitalizacion` no repite el filtro de institución (solo va sobre `c.institucion_id`). Hoy imposible que cruce, porque `ingresar` valida `cama_es_del_ambito`; defensa en profundidad barata |
| `vista::pyp::controlar` | Responde **403 nombrando el programa** si no está licenciado, y redirección con `inscripcion_inexistente` si es ajena o no existe — mientras `egresar`, su gemela de la API y `pyp::ver` responden lo mismo en los tres casos. No cruza el límite de institución |
| Superadmin y cuenta sin institución | Pierden **silenciosamente** las 15 escrituras de pantalla compartida, con **200 OK**: `exige_licencia` pasa por `contexto()`, que para ellos devuelve la página «esta cuenta no tiene panel clínico». Falla cerrado, pero un no-op con 200 es indistinguible de un éxito |
| `licencia::no_licenciado` | Devuelve **JSON** desde manejadores de pantalla, mientras sus otros finales son `Redirect` con `?aviso=`. Coherente con la capa; el usuario del panel verá JSON crudo |
| RIPS sin tope de longitud | `diagnostico_principal` (el MUV espera `varchar(6)`), `codigo_vida` (256) y `numero_documento` (20) entran sin acotar. Consecuencia: rechazo del paquete entero por el validador, que es calidad de dato, no inyección |
| `vista/nacimientos.rs:261` | `Box::leak` por render en un camino que el `CHECK` de 0062 hace hoy inalcanzable. `Cow<'static, str>` cuesta lo mismo y quita la clase |
| `Generacion::fev` | Un solo nacimiento mal enlazado aborta el FEV **completo** del periodo (`RecienNacidosSinAtencionDeLaMadre`). Es deliberado y está documentado; se anota porque el modo de fallo tiene consecuencia comercial fuerte el día del cierre de mes |

### #274 — CERRADA el 2026-08-10, por decisión de Juan

Decía «Migrar medico a Rust (axum + SQLx/SQLite) **con Angular embebido en el
binario**», y ese objetivo dejó de existir el 2026-08-08. Se cierra tras
verificar el estándar contra el ÁRBOL, no contra este archivo (SHA `126eb44`):

| | |
|---|---|
| Angular | cero rastro: **0 archivos `.ts` versionados**, sin `frontend/`, `package.json`, `build.rs`, `estaticos/` ni `rust-embed` |
| stack (directiva 26) | Rust + `axum` 0.8 + `SQLx` 0.8/SQLite + `maud` 0.27 + htmx vendorizado, un solo binario |
| htmx y `boi.js` | **byte a byte idénticos** a informes, chasqui y nexus (`md5sum`, no de memoria) |
| CI | tres trabajos, todos Rust, con grupo de concurrencia por `sha` y techo de 20 min |
| `opt-level` de dependencias | ausente **a propósito y medido**: aquí cuesta ~60 % más de minutos y no compra nada (directiva 16 sobre el estándar de nacimiento) |
| pruebas | 1 086, en verde (1 059 + las 27 de `medico-muv`) |

**Lo que la verificación destapó, y que la ficha no decía**: el binario generaba
el FEV-RIPS y **no lo subía**, y la mitad de `backend/` no la llamaba nadie. Las
dos cosas se hicieron el mismo día —`medico-muv` y la poda de 33 archivos— y lo
que queda de cada una tiene ficha propia: enchufar `medico-muv` (credenciales del
prestador, backend TLS, pantalla) y portar las reglas de VFP que siguen solo en
`notas_operatorias.py` y `contrarreferencias.py`.

| # | pendiente |
|---|---|
| **#16 / #65** | Migración VFP completa. **Los tres «sin portar» se resolvieron el 2026-08-16 y NINGUNO era lo que la ficha creía**: `ayudasdx` ya estaba (es `orden` + `resultados_dx`); `soportecobertura` no es un módulo sino un REPORTE, y entró como noveno panel de estadísticas (`/api/estadisticas/cobertura`) sin los dos defectos del original —el de 60 años exactos que no caía en ningún tramo, y la edad calculada restando años—; y **`terceros` ES la tabla `entidad`**, ya portada, a la que solo le faltaban siete campos de contacto (migración `0061`). Medido sobre las 51 filas VIVAS del `TERCEROS.DBF`: todas entidades responsables de pago, cero proveedores. Queda **UN módulo en 501** desde el 2026-08-16 — eran 22, y bajaron veintiuno.
El que queda es `PORTAL_PACIENTE`, y no le falta código: le falta una decisión de
producto sobre la identidad del paciente (ver su fila abajo). Los otros veinte
(`BANCO_SANGRE`, `TELEMEDICINA`, `TRABAJO_SOCIAL`, `TURNOS`,
`EQUIPOS_MEDICOS`, `COMITES`, `INFECCIONES`, `INDICADORES`,
`CENSO_DIARIO`, `PREANESTESIA`, `RECUPERACION`, los cuatro de P&P
(`VACUNACION`, `PRENATAL`, `CRECIMIENTO_DESARROLLO`, `SALUD_REPRODUCTIVA`),
`REGISTRO_NACIMIENTO`, el bloque de enfermería (`ENFERMERIA`, `KARDEX`, `MAR`,
`PLAN_CUIDADO`) y `CERTIFICADO_DEFUNCION`. El recuento no se lleva a mano: lo
cuenta `modulo.rs` y lo ata `el_catalogo_no_miente_sobre_lo_que_el_binario_sirve`. **Corregido el 2026-08-03**: aquí decía «tres sin existir», y es falso — los tres tienen formulario en el original (`soportecovertura.scx`, `terceros.scx`, `ayudasdx.scx`) y dos tienen tabla con datos (`TERCEROS.DBF`, `AYUDASDX.DBF`). «No existe» decía que no había nada que migrar; lo que hay es trabajo sin hacer. Orden fijado por Juan: **migración completa primero, mejoras después**. |
| **#249** | **Los 22 módulos pendientes. `REGISTRO_NACIMIENTO` HECHO el 2026-08-16**: tabla `recien_nacido` (migración `0062`), `medico_datos::recien_nacido`, arreglo `an.rs` del RIPS, pantalla `/panel/nacimientos` y rutas `/api/recien-nacidos`. Con él, medico emite **SEIS** de los siete arreglos de la 948. La forma salió del VALIDADOR —`Rips.RecienNacidos` del contenedor `fevrips-db`, 21 columnas con tipos y longitudes; catálogos `REFs_TipoDoc` y `REFs_CondicionDestino`—, no de la memoria. **Quedan 21**, y la determinación del 2026-08-15 sigue en pie: ninguno tiene formulario en el VFP vivo del prestador, así que construirlos es decisión de PRODUCTO, no deuda técnica. |
| **#249 · enfermería** | **HECHO el 2026-08-16.** Cuatro módulos y **una sola tabla**: `ENFERMERIA` y `PLAN_CUIDADO` viven en la espina `documento_clinico` con tipo propio; `KARDEX` es una VISTA derivada (fórmulas del paciente + sus administraciones — una tabla propia sería una tercera copia que se desincroniza de la orden médica); solo el `MAR` es un hecho nuevo (`0063`). **Lo que de verdad faltaba era una PUERTA, no una tabla**: `EscribirClinico` son `Superadmin\|Admin\|Medico`, así que una enfermera no podía escribir una nota de enfermería — y son parte integral de la historia clínica. Nace `EscribirEnfermeria` como capacidad propia, con la forma de `EscribirNutricion`. El tipo del documento **lo pone el servidor** en `/api/enfermeria/*`: no hay valor que el cliente pueda mandar para escribir una epicrisis, y `/api/documentos` rechaza los dos tipos de enfermería. El MAR registra la OMISIÓN con motivo obligatorio (un hueco no distingue «no se puso» de «se puso y no se anotó») y **no se borra: se anula**. |
| **#249 · defunción** | **HECHO el 2026-08-16** (`0064`). medico ya sabía que alguien murió —`hospitalizacion.estado`, `urgencias.destino_salida`— pero no de qué. La **causa básica no se deduce de la cadena**: elegirla es un acto médico, y deducirla produciría estadística vital falsa bien formada. Índice único **parcial** (`WHERE anulada = 0`) para que anular y reexpedir siga siendo posible. Se rechaza lo imposible (morir antes de ingresar) y **no** lo inusual: certificar sobre una hospitalización todavía abierta es el orden normal de trabajo y tiene prueba con su nombre. |
| **#249 · censo** | **HECHO el 2026-08-16, y SIN TABLA** — el tercero seguido (`soportecobertura`, `KARDEX`, éste). Se deriva de `hospitalizacion` y `cama`; guardarlo sería una copia que envejece y haría irreproducible el documento que el hospital archiva. Lo que lo hace censo y no lista es su identidad —`inicial + ingresos − egresos = final`—, con los cuatro números calculados **por separado** y la identidad COMPROBADA: derivar el último de los otros tres haría que cuadrara siempre, incluso con los datos rotos. **Cazó un `bind` mal atado que devolvía CERO en vez de error**: el `?` de la institución está al principio de la cadena y se ataba al final, así que el filtro recibía una fecha. Un censo que dice que el hospital estaba vacío se imprime igual de bien que uno correcto. |
| **#249 · anestesia** | **HECHO el 2026-08-16** (`0065`). `programacion_quirurgica` tenía desde la 0047 una CASILLA `valoracion_preanestesica`: dice que se hizo y no dice qué dijo — el mismo patrón que `dispensacion` frente al MAR, y comprobado que no condiciona nada en el dominio. La casilla se queda; lo que cambia es que ahora existe el documento y `hay_valoracion()` dice si ESTÁ. Dos tablas y no una con discriminador: son dos hechos en dos momentos, sin un campo en común más allá de la cirugía. **El umbral de Aldrete va con sus TRES casos** —≥9 sale, <9 sin razón se rechaza, <9 CON razón sale—: la regla es «lo inusual con su razón», no «lo inusual prohibido», y sin el tercer caso bloquearía al cliente justo en el traslado a UCI. |
| **#249 · P&P** | **HECHO el 2026-08-16** (`0066`). Los cuatro programas se anclan a su finalidad del MUV leída de `muv_finalidad.json`, que **ya estaba vendorizado** —11 valoración integral, 14 protección específica, 19 planificación, 23 prenatal— en vez de citar la norma de memoria; `las_cuatro_finalidades_existen_en_el_catalogo_del_muv` lo ata al artefacto con su positivo de control. **La inscripción y el control son dos cosas**: se puede estar inscrito sin haber ido nunca, y eso es lo que mide la adherencia. **El lote de la vacuna es obligatorio** — sin él, un retiro de mercado no encuentra a quién se le puso. Y el número de control lo pone el SERVIDOR: enviado por el cliente, dos «terceros controles» contarían como tres. **Cazado por `ninguna_ruta_de_modulo_se_queda_sin_licencia`**: dos rutas compartidas quedaban fuera de todo prefijo y se servían SIN puerta de licencia; al partirlas por programa se cerró además que una licencia de vacunación abría el control de un prenatal (`es_del_programa`). |
| **#249 · habilitación** | **HECHO el 2026-08-16** (`0067`). `item_inventario` NO servía para los equipos y hay que decir por qué: modela consumibles —cantidad, lote, mínimo— y un ecógrafo no es «cantidad_actual = 1». Lo VENCIDO no se guarda, se compara al leer —un booleano estaría mintiendo desde el día siguiente— y devuelve TRES cosas: al día, vencido y **sin cronograma**, que en una visita es un hallazgo distinto y peor. `INDICADORES` es el CUARTO derivado sin tabla, y trae `no_calculables` con **qué falta, nombrado**. Obligó a añadir `cita.fecha_solicitud`: la oportunidad de la consulta no se podía calcular, y rellenar el histórico con la fecha de la cita habría dado oportunidad CERO para todo lo anterior. |
| **#249 · los últimos** | **HECHO el 2026-08-16** (`0068`). `TRABAJO_SOCIAL` no lleva tabla —nota en la espina `documento_clinico` con capacidad propia `EscribirTrabajoSocial`—. `TELEMEDICINA` ya estaba a medias: `historia.modalidad_grupo_servicio` existe desde la 0005 y el CodeSystem `ColombianTechModality` está vendorizado con las cuatro modalidades (06-09); lo que faltaba era **quién está en el otro extremo**, y registrarla escribe la modalidad TAMBIÉN en `historia`, que es de donde la leen el RIPS y el RDA. `TURNOS` no es la agenda —esa es disponibilidad para citas— y su regla es que nadie está en dos sitios a la vez, con el RELEVO permitido. `BANCO_SANGRE`: la compatibilidad vive en `medico_dominio::sangre` con las 16 combinaciones ABO escritas a mano y **el plasma invertido** (donante universal AB, no O), verificado con mutante; el grupo del receptor **se registra en cada transfusión**, no se lee de `paciente` —que no lo tiene, y es correcto—. |
| **#249 · PORTAL_PACIENTE** | **EL ÚNICO QUE QUEDA, y a propósito.** No es un módulo más: los otros 21 los usa personal de la institución y el aislamiento es por institución; éste mete un SUJETO NUEVO —el paciente— que no encaja en `usuario` (no tiene rol), exige resolver cómo se prueba que quien dice ser un paciente lo es antes de enseñarle su historia, expone la superficie a internet y da la vuelta a la bitácora de la Ley 1581 (el que accede ES el titular). Era además el único de los 22 clasificado como **no obligatorio**: diferenciador comercial, sin norma que lo exija. Lo que falta no es código: es una decisión de producto de Juan sobre identidad del paciente. |
| **#67** | FEV-RIPS por archivo. **AU y AH contrastados a ojo contra el PDF el 2026-08-12**, que es lo que esta ficha exigía antes de tocarlos: el ejemplo de la propia norma emite E1, E2 y E3 del relacionado de egreso en los dos, y solo salía E1. Corregido, con el banco de las dos mitades detrás. **RN HECHO el 2026-08-16** (arreglo `recienNacidos`, tras desbloquearlo #249): sus 18 campos y su orden se atan al artefacto de la norma en `campos_de_la_948.rs`, y el orden de emisión se mide sobre `to_string` —`to_value` ordena alfabéticamente y haría pasar la prueba con cualquier orden—. **OS (`otrosServicios`) HECHO el 2026-09-08** (Opción 2 decidida por Juan, alcance solo insumos/dispositivos): la fuente NO es `traslado` sino la **dispensación de un insumo/dispositivo colgada de la ATENCIÓN** (migración `0073`: `dispensacion.formula_id` nullable + `historia_id` + `CHECK` exactamente-uno + quinteto de facturación + `num_autorizacion`/`id_mipres`; `item_inventario.ium`). Dominio `aos.rs` (`documento_aos`, `tipoOS="01"`, `codTecnologiaSalud`=IUM o `registro_invima`), lectura `otros_servicios_del_periodo` (excluye MEDICAMENTO), emisor fundido en el AT, escritura `inventario::suministrar_insumo` (rechaza medicamento y historia firmada, transacción, D8), y pantalla `farmacia`→«Suministros (otros servicios)» con E2E HTTP. Los `vr*` se CAPTURAN (no se inventan): sin dato, el Recolector anota falta (D20). Ver `docs/PROPUESTA_67_OTROS_SERVICIOS.md`. `codigoVIDA` sigue fuera (amarre #603). Cierre pendiente: D25/concilio antes de mergear/desplegar (toca dinero facturado a EPS). |
| **#77** | Cargar el CUM a **base de datos** (66 mil registros que caducan, no JSON empaquetado). `Medicamento` se queda corto: faltan estado_cum, registro sanitario, vencimiento anulable, ATC, principio activo, vía, unidad e IUM. Pendiente aparte: CUPS (`cups.xlsx`, cabecera corrida una columna). |
| **#88** | Barrer el backend buscando el patrón «filtro opcional» fuera de los routers, y decidir si `test_guardia_aislamiento.py` puede cubrir esa capa. |
| **#84** | Captura asistida. **La fricción graduada está HECHA y cableada** (diagnóstico y fórmula; ver «Procedencia»), y la decisión que la ficha daba por pendiente —qué campos tienen efecto jurídico o económico— está resuelta por construcción: se enumera lo administrativo y el resto es duro. **Y el 2026-08-16 se cerró lo que la captura dejaba abierto por debajo**: el aval ya no se hereda sobre un valor nuevo, la firma sella las trece columnas de dinero de la 0041, hay guardián de la lista firmada, los diagnósticos relacionados por fin se auditan, y la cantidad se valida en la frontera de `serde` en vez de en cada puerta (ver `DETALLE.md`, «La firma de los anexos»). **Las derivaciones HECHAS el 2026-08-21** (`derivaciones.rs` + `paciente::derivar_municipio` + columna `paciente.procedencias`, migración `0069`): el `cod_municipio` se deriva del texto libre de la ciudad y se marca `DERIVADO`, y `usuario_base` lo audita contra la procedencia del paciente en vez de contra `None`, así que la falta «duro sin aval» **ya se dispara en producción** —el municipio es un campo DURO puesto por la máquina sin aval humano—. No pisa el código tecleado (queda `CONFIRMADO`) ni aproxima ante un homónimo (varios «San Juan» → no deriva). **La demografía del `PatientRDA` (#603) se CAPTURA desde el 2026-08-22** (migración `0070`): nacionalidad, pertenencia étnica (+ comunidad, texto libre), discapacidad e identidad de género, por la API (`PacienteNuevo`), con su procedencia `confirmado` y los tres catálogos (etnia/discapacidad/género) servidos por `ihce::terminologia` desde el CodeSystem vendorizado. El ensamblador del RDA (`ihce::actores`) ya tenía los huecos; esto los alimenta. **Queda**: (a) la PANTALLA de captura del paciente —hoy no hay formulario `maud`, la ficha es solo lectura y se captura por API—; (b) cablear `ihce::actores`→`cola_rda` a producción, que es paso de **#603** (revisión de seguridad: encolar manda datos clínicos a la plataforma nacional); (c) más derivaciones cuando aparezca más texto legado —BLOQUEADO hoy: `zona` no deriva del municipio y no hay importador VFP—; (d) medir propuestas aceptadas sin mirar —BLOQUEADO hoy: ninguna pantalla genera `propuesto`—. |
| ~~#104~~ | **CERRADA el 2026-08-12.** Las dos mitades hechas: el PESO ideal el 11 y la TALLA ideal hoy, con su tabla transcrita de `PESOTALLA1`, su columna (migración `0057`) y su sitio en la pantalla del examen. Ver «Fidelidad al sistema original». |
| ~~#186~~ | **CERRADA el 2026-08-11.** Era la bitácora de acceso a dato personal (Ley 1581); lo único que quedaba era cablear listado y exportación, en espera de que los endpoints existieran. Ya existen y ya anotan: los listados dejan su colección **sin ids** y la exportación de RIPS anota **con** ellos, en `rips::exportar` —la generación y la anotación en la MISMA función, para que la pantalla del panel no pueda exportar sin dejar la línea—. |
| ~~#187~~ | **HECHA y VALIDADA el 2026-08-11; falta desplegarla.** Era cabeceras + rate-limit de login, y llevaba desde julio en espera de que medico estuviera en el VPS. Está: `despliegue/medico.xiliux.com.nginx` estrecha las dos rutas de entrada a **30r/m con ráfaga de 10** (zona propia por `map` sobre `$uri`), frente a los 600 intentos por minuto que admitía la zona general. Validado con el `nginx` DEL VPS —aquí no hay: `nginx -t -c` sobre una carpeta de usar y tirar, sin instalar ni recargar— y con su control: quitándole la declaración a la zona, el test falla nombrándola, que es lo que prueba que el `include` llegó a leerse. **No está desplegado** — eso se anuncia aparte y pasa por la directiva 25. |
| ~~#236~~ | **CERRADA el 2026-08-11 por medición.** Decía que seis módulos tenían pantalla y licencia sin backend en Rust —`admin`, `facturacion`, `quirofano`, `estadisticas`, `consentimiento`, `rips`—. Los seis sirven hoy: están en `CATALOGO` con prefijo y el router los atiende. El recuento vivo es **24 de 46 módulos con backend** (eran 15 el 2026-08-01), y no se lleva a mano: lo cuenta `modulo.rs` y lo ata `el_catalogo_no_miente_sobre_lo_que_el_binario_sirve`. |
| ~~#254~~ | **CADUCA el 2026-08-11.** Era «alinear medico 21 con informes 22» de Angular. No hay Angular en ninguno de los dos: **cero `.ts` versionados** en este repositorio. |
| ~~#310~~ | **CADUCA el 2026-08-11.** Era «el CI está caído por facturación». Lo sustituye el protocolo solo-local: no hay CI de GitHub que reparar y no lo habrá (ver el recuadro de «El CI»). |
| ~~#610~~ | **CERRADA el 2026-08-11.** Era reintroducir el `pytest` de la cadena RIPS/FEV «al volver el cupo». El cupo no vuelve, pero el runner sí existe —`verde_local.py`—, así que el trabajo se hizo hoy: job `cadena-rips-python`, verificado con sus DOS casos rojos (mutar `_sin_segundos` lo pone rojo; esconder el `.venv` también, en vez de recoger cero pruebas y salir verde). |
| ~~#80~~ | **CERRADA por retirada el 2026-08-08.** Era «Angular 21 → 22 y decidir el salto»; se hizo el 2026-08-02 y el 2026-08-08 Angular salió entero. No queda nada que actualizar. |
| **#65.7** | La verificación que de verdad cierra el RIPS: comparar el JSON propio contra el del conversor web de MinSalud para el mismo caso. |
| ~~#65.6~~ | **CERRADA el 2026-08-08.** `modulos_habilitados` se edita en `/panel/superadmin/{id}`: la ruta `PUT /api/instituciones/{id}/modulos` existía y ningún frontal la llamaba, así que la licencia solo se podía cambiar tocando la base. Ya no bloquea dar de alta un cliente. |

Preguntas abiertas para el prestador: qué hace `mousemovex` (108 usos); si alguna vez
registró un CVE distinto del diagnóstico (los informes darían números diferentes,
porque ahora el evento se deriva del diagnóstico); si el archivo que produce hoy pasa
el validador 1.4.8.0; y **si los tres campos que hoy exige la firma de la historia
—motivo, diagnóstico y plan— son los que él considera indispensables para cerrarla**
(elegidos por Juan el 2026-08-08; la medición que los eligió está en `DETALLE.md`,
«La historia clínica tampoco se firmaba en blanco». El VFP vivo no exige
ninguno, así que confirmarlo es contrastar, no desbloquear).

Cerradas y ya no pendientes: #79 (las pruebas compartían una BD por accidente — lo
resuelve `conftest.py`), DIVIPOLA y CIE-10 cargados, ejecutor de pruebas del frontend
montado.
