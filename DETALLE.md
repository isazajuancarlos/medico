# medico — el detalle

> **Esto NO se carga solo.** Lo básico —qué es el producto, cómo se construye y
> se prueba, las reglas que no se rompen y las convenciones— vive en
> [`CLAUDE.md`](CLAUDE.md), que sí entra en cada sesión. Aquí está el resto,
> íntegro y sin resumir, para **consultarlo con `grep`** cuando haga falta:
>
> ```bash
> grep -n "freno\|FEV-RIPS\|hx-target" DETALLE.md
> grep -n '^#\{2,3\} ' DETALLE.md      # el índice, siempre al día
> ```
>
> Partido de `CLAUDE.md` el 2026-08-10, cuando aquel archivo iba por 3 235 líneas
> y 192 KB y se cargaba entero en cada turno de cada sesión. **No se ha borrado ni
> una línea**: lo que sale de allí entra aquí tal cual.

**Lo que se escribe aquí y no allí**: la crónica de un cambio ya hecho —qué se
midió, qué mutantes murieron, qué se decidió y por qué—, y lo histórico. Lo que
cambia lo que alguien va a teclear hoy va en `CLAUDE.md`.

## Índice

- **Angular y PrimeNG, y lo que dejaron escrito** — por qué salieron, las tres
  lecciones que sobreviven, y lo medido al desplegar la demostración
- **Lo que el port todavía no cubre** — cómo se cuenta el avance, y por qué la
  cuenta vieja mentía en las dos direcciones
- **Bitácora del port, pantalla por pantalla** — 29 secciones, de `/api/admin` al
  FEV-RIPS de extremo a extremo, cada una con su medición y sus mutantes
- **La bitácora de acceso (Ley 1581) — las reglas completas** — las tres formas de
  evento, la tabla de rutas cableadas y qué queda fuera a propósito
- **Arquitectura del backend (histórico)** — la API Python retirada el 2026-08-09,
  su capa de aislamiento y sus tres estados de licencia
- **Procedencia: los cuatro casos corregidos** — los defectos que nadie eligió y
  el patrón que los hacía invisibles
- **Fidelidad: los antecedentes contrastados contra la pantalla** (2026-08-03)
- **Frontend (histórico)** — el SPA de Angular 22 + Material
- **Revisión de tareas tras el cambio de estándar** (2026-08-08)

---

## Angular y PrimeNG, y lo que dejaron escrito

> **ANGULAR SALIÓ EL 2026-08-08.** Las 29 pantallas están en el panel de `maud`,
> `/` redirige a `/panel`, y se retiró el SPA entero: `frontend/` —482 MB, 98
> archivos TypeScript—, `rust-embed`, el `build.rs` que rompía el *release* sin
> `index.html`, la bandera `MEDICO_SIN_INTERFAZ`, el trabajo `frontend` del CI,
> los dos primeros pasos de `construir.sh` y el montaje `AngularSPA` del Python.
> **El build dejó de necesitar Node**, y el recuento de dependencias de frontal
> del producto entero es **UNA** (htmx, MIT, vendorizada).
>
> **PrimeNG había salido antes, el 2026-08-02** (#255), y esa lección se queda
> aunque la librería que la sustituyó también se haya ido: PrimeNG 22 dejó de ser
> MIT —es «PrimeUI», comercial, con clave obligatoria que sin ella estampa
> *«Invalid PrimeUI License»* sobre la aplicación—, y **un producto que corre
> offline en casa de un cliente no puede llevar dentro una clave que caduca**.
> Ninguna dependencia de pantalla entra si puede exigir clave, telemetría o red.
>
> **`MIGRACION_MATERIAL.md` y `MIGRACION_FORMULARIOS.md` quedan como historia**,
> no como instrucciones: describen migraciones dentro de un frontal que ya no
> existe.

### Lo que Angular dejó escrito, y que no vuelve

Retirado el 2026-08-08. Aquí vivían tres apartados —la subida a Angular 22, la
red del «estado que la plantilla lee es señal» y la fuente que solo se vio
abriendo la aplicación— y se borran con el SPA, porque describían cómo mantener
algo que ya no existe (directiva 7). **Tres lecciones sí se quedan**, porque no
eran de Angular:

1. **Un opt-out que escribe una herramienta hay que LEERLO.** Las migraciones de
   Angular 22 dejaron tres, y los tres decían «quédate como estabas»; uno
   silenciaba un diagnóstico, y el aviso que tapaba era verdadero — el tipo
   mentía, no el compilador. Silenciarlo habría dejado dos defensas pareciendo
   redundantes, y el siguiente que pasara las habría borrado.
2. **Un fallo que no se parece a un fallo es el caro.** Con OnPush, un campo
   escrito desde un `subscribe` no marcaba nada sucio: la petición respondía, el
   dato llegaba, y la pantalla seguía mostrando lo anterior. Sin error. El
   guardián que lo encontró miraba las llamadas transitivamente, no solo el
   cuerpo del callback, y con un nivel habría dado un verde que no significa
   nada.
3. **Ninguna red mira cómo QUEDA la pantalla.** 366 pruebas en verde, `ng build`
   en verde, y el producto se veía con serifas porque el tema precompilado traía
   `Roboto` en 30 tokens y Roboto no viajaba en el binario. Es la directiva 28, y
   es la razón de `tests/pantalla.rs` — que hoy sigue sin alcanzar las pantallas
   con sesión (tarea #2).

## La demostración pública — cómo se montó y qué se midió

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

## Lo que el port todavía no cubre

**Lo que el port todavía NO cubre**, para no confundir avance con paridad.

**Cómo se cuenta, corregido el 2026-08-06.** Hasta esa fecha la lista salía de
comparar los PREFIJOS de `rutas.rs` contra los de `backend/routers/*.py`, y esa
cuenta miente en las dos direcciones: suma como trabajo lo que solo cambió de
nombre (`auth` → `acceso`/`salir`/`yo`, `superadmin` → `/api/instituciones`, los
cuatro documentos clínicos → `/api/documentos`) y no ve un verbo suelto sin
servir dentro de un módulo que sí existe. Se cuenta contra lo que la INTERFAZ
pide, que es el único criterio que es una prueba: se resuelven las llamadas
`http.<verbo>` de los `.ts` del frontal contra las constantes `API` de cada
servicio y se cruzaban con las rutas del `Router::new()`. **Esa cuenta murió con
Angular el 2026-08-08**: no hay frontal externo cuyas llamadas contar, porque las
pantallas las teje el mismo proceso que tiene los datos. Se conserva escrito
porque el MÉTODO —contar contra lo que se PIDE, no contra prefijos que se
parecen— es el que valdrá la próxima vez que haya dos lados que comparar.

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

## Bitácora del port, pantalla por pantalla

> Cada sección es un cambio terminado: qué se decidió, contra qué se midió y qué
> mutantes murieron. Se lee cuando se va a tocar esa pieza, no antes.

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

   Dos actos llevan su detalle porque tocan la evidencia y su nombre no lo dice:
   `restablecer_password` llama a `quitar_freno` por dentro, y `quitar_freno`
   afecta al freno de todos los orígenes de ese correo. **Desde #539 (2026-08-09)
   ya NO borra `intento_acceso`** —el único registro de un ensayo de
   contraseñas—: anota una **reposición** que `guaca::freno::evaluar` trata como
   un acierto sintético (limpia la espera, conserva las filas de fallos). El
   detalle se sigue anotando porque limpiar la espera de una cuenta ajena es un
   acto que no debe quedar sin rastro propio.

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

**Y desde el 2026-08-09 (#538) la bandera OBLIGA.** Nacía imposible en la otra
dirección: `mi-clave` la dejaba satisfacer, pero **nadie la leía en el login**,
así que la contraseña temporal del admin valía indefinidamente sobre una cuenta
cuyos actos van firmados a esa persona —quien la restableció podía firmar como
ella—. Ahora hay **dos gates**, uno por superficie, y `requiere_cambio_password`
viaja en `usuario::Usuario` (que la sesión ya relee en cada petición, sin consulta
extra). El gate vive en los EXTRACTORES de sesión, no en `contexto`:
- **El panel**: `EnSesion` y `EnSesionCon` rebotan a `/panel/cambiar-clave`
  mientras la bandera esté puesta. Va en los extractores y no en `contexto`
  porque **no toda pantalla de datos construye `Contexto`** —el fragmento del
  buscador de paciente no, y ahí se colaba dato de paciente—. La pantalla de
  cambio y `salir`/`salir-todo` cargan la sesión A MANO (`usuario_de`), no por
  esos extractores, así que no quedan encerrados. El núcleo del cambio es el
  MISMO `rutas::cambiar_clave_propia` que `mi-clave` (una sola copia de las cuatro
  reglas).
- **La API**: los extractores `Autenticado` y `Autorizado` responden 403 a un
  usuario con la bandera, así que no puede firmar por `/api/*` lo que el panel le
  impide. El check NO va en `usuario_de` a propósito: lo comparte el panel, y ahí
  gatearía la propia pantalla de cambio. `EnSesionCon` delega en `Autorizado`,
  así que comprueba la bandera ANTES de delegar —si no, el panel daría el 403 de
  la API en vez de rebotar—. Cambiar la contraseña (por el panel) sube
  `token_version`, limpia la bandera y reemite la cookie: se sale del gate por los
  dos lados a la vez.

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
«desbloquear» **repone** el freno de ese correo —anota una reposición que la
regla trata como un acierto (limpia la espera)—; desde #539 ya no borra los
intentos, que son la única evidencia del ensayo.

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

### El panel en `maud` — la migración al estándar nuevo (2026-08-07, EN CURSO)

`crates/medico-api/src/vista/`. Directiva 26, cambiada el 2026-08-06: la pantalla
se teje EN EL SERVIDOR con `maud` + htmx + `boi`, sin Node en el build.

**Medido antes de empezar, para no prometer de más: 29 pantallas vivas y
~19 000 líneas de Angular** (48 rutas en `app.routes.ts`, de las cuales 20 son
cascarones de módulos sin construir y NO se migran — con la capa de licencia dan
404 y el menú ya lo dice). El panel de informes, que es la referencia, son 2 465
líneas de `maud` en un archivo. Esto es trabajo de varias sesiones.

**Los dos frontales CONVIVIERON, y fue la decisión de fondo.** El panel se montó
bajo `/panel` mientras Angular seguía sirviendo `/` por el `fallback`, así que
cada pantalla que llegaba se podía abrir, probar y desplegar sola sobre un
producto que ya está en manos de un cliente. **El cambio de dueño de `/` fue el
ÚLTIMO paso, el 2026-08-08** — informes lo hizo igual: su «Angular fuera» fue el
último commit, no el primero. Hoy `/` redirige a `/panel` y no queda `fallback`
de SPA: lo que no case es un 404.

| pieza | estado |
|---|---|
| armazón (`pagina`, menú, avisos, CSS, `piezas`, `Contexto`) | hecho |
| entrada / salir / cerrar todas las sesiones | hecho |
| pacientes, citas, incapacidades, remisiones, contrarreferencia | hecho |
| consentimiento, urgencias, personal, los cuatro documentos clínicos | hecho |
| tablero, hospitalización, rehabilitación | hecho |
| **RIPS** (revisar el periodo + descargar) | hecho |
| **Imágenes diagnósticas** (bandeja + informe, la primera que ESCRIBE) | hecho |
| **Ambulancias** (parque, traslados, alta, ficha y despacho) | hecho |
| **Estadísticas** (los siete paneles, con los gráficos en SVG) | hecho |
| **Laboratorio** (bandeja, resultado por parámetros y perfiles) | hecho |
| **Agenda médica** (horarios, bloqueos e indicadores) | hecho |
| **Nutrición** (valoraciones y plan de alimentación) | hecho |
| **Quirófano** (agenda, verificación, salas y las transiciones) | hecho |
| **Farmacia** (inventario, fórmulas, dispensaciones, movimientos, proveedores) | hecho |
| **Facturación** (facturas, líneas, glosas y contratos) | hecho |
| **Administración** (las ocho pestañas) | hecho |
| **Historia clínica** (las ocho secciones) | hecho |
| **Superadministración** (instituciones, plan, módulos, primer admin) | hecho — **29 de 29** |
| cambiar el dueño de `/` | pendiente, y es el ÚLTIMO paso |

**Lo vendorizado va en `crates/medico-api/panel/`, y esa elección salvó el
retiro de Angular**: `estaticos/` era la salida del build del SPA y estaba entera
en `.gitignore`, así que un archivo nuestro ahí habría desaparecido en el primer
clon limpio — que es exactamente el defecto que informes se comió al retirar el
suyo. Comprobado el 2026-08-08 extrayendo el árbol de git a otro directorio y
compilándolo allí. `estaticos/` y su regla del `.gitignore` ya no existen. htmx
(`19a5737…`), `boi.js` (`c27e7b0…`) y `confirmar-doble.js` (`e83cdce…`) son byte
a byte los de nexus, chasqui e informes — comprobado con `md5sum`, no de memoria.

**El acceso NO se reimplementa.** `rutas::intentar_acceso` se extrajo del
manejador de `/api/acceso` y ahora la comparten los dos: el freno, el hash de
descarte y los cuatro finales viven en un sitio. El panel solo traduce esos
finales a cuatro pantallas — y el 503 se dibuja distinto del 401 a propósito,
porque decirle «credenciales incorrectas» a quien sufrió una avería lo manda a
cambiar una contraseña que estaba bien.

**La bitácora de la Ley 1581 se anota con los MISMOS argumentos.** Abrir una
ficha en el panel deja `("ver", "paciente", id, id)`, igual que
`GET /api/pacientes/{id}`. El contenido del eslabón está congelado byte a byte,
así que un verbo distinto habría partido el rastro de un mismo hecho en dos según
qué pantalla lo abriera — y migrar una pantalla sin anotar habría BORRADO rastro
que hoy existe, sin que ninguna prueba de pantalla lo viera.

**Una tabla, dos usos.** `licencia::PANTALLAS` dice de qué módulo es cada
pantalla del panel, y con ella se construyen el menú **y** la puerta. Con dos
listas volvería a pasar lo de #534: el menú diciendo una cosa y el servidor
haciendo otra. El guardián de fuente lee los DOS routers, así que migrar una
pantalla a `maud` ya no la saca de la licencia en silencio.

**Y desde el 2026-08-08 esa tabla lleva una CUARTA columna: qué capacidad exige
la pantalla** (`Pantalla { modulo, ruta, etiqueta, exige }`). La estrenó
`/panel/rips`, que es la primera que pide algo más que estar dentro —`GenerarRips`,
la misma que `GET /api/rips`—. Es un dato y no un `if` en el manejador porque el
MENÚ también lo necesita: sin él se ofrece una entrada que lleva a «esta sección
no le corresponde», y el sidebar de Angular que se está migrando sí la escondía
(`roles: ['admin','superadmin']`). Se escribe la CAPACIDAD y no el rol: qué roles
la tienen lo decide la matriz de cada institución.

**Eso obligó a `vista::Contexto`**, que es lo que toda pantalla necesita antes de
pintar: institución, módulos del plan y **qué puede quien mira**. Sustituye a las
mismas ocho líneas repetidas en catorce pantallas, y la tercera pieza es nueva —
se pregunta a `estado.capacidades.ajustes(...)`, la matriz de la institución con
su caché, que es la MISMA respuesta que da `Autorizado`. Preguntársela al defecto
nacional escondería una pantalla que el servidor sí abre para un rol al que esa
IPS se la concedió: la segunda lista de #534 otra vez, por la puerta de atrás.
Si la matriz no se puede leer es **avería**, no un menú recortado (directiva 20).

**Y el banco de navegador, que este repositorio no tenía** (`tests/pantalla.rs`
y `tests/comun/`). Arranca el binario REAL en un puerto efímero y lo pinta en
Chromium. Había 390
pruebas de frontend en `vitest` —que montaban componentes en jsdom— y **cero**
que dibujaran nada; ya costó una vez, con 366 en verde y el producto viéndose con
serifas. Prueba que la cadena de módulos CORRE bajo la CSP real, mirando el
`data-boi="listo"` que `boi` escribe justo para esto. Mutante: con
`script-src 'none'` se pone roja esa prueba y solo ésa.

**Desde el 2026-08-08 alcanza las pantallas CON sesión**, que era el hueco. Ver
[El banco de navegador](#el-banco-de-navegador-habla-cdp-y-entra-por-el-formulario-2026-08-08).

**Dos defectos reales cazados por parejas de prueba, no por lectura:**

- **`PORT=0` hacía mentir al log de arranque**: anunciaba el puerto configurado y
  no el que ligó. Ahora dice `listener.local_addr()`.
- **Una ruta errónea bajo `/panel` devolvía el `index.html` de Angular con
  200**, porque el comodín del SPA la recogía. Es el mismo fallo que este
  repositorio ya había cerrado para `api/`. Peor que el 404 que falta: un
  `<script src>` con una errata recibía HTML con 200 y el navegador lo descartaba
  por el MIME **sin que nada fallara**, y quien tecleara mal aterrizaba en la
  aplicación vieja sin enterarse.

**El recuento se corrigió el 2026-08-08, contando los directorios de
`frontend/src/app/pages` antes de borrarlos**: son **29** pantallas vivas (30 menos
`modulo-no-disponible` y `placeholder`, que no lo son), no 28. La cifra vieja
salía de mapear las cuatro páginas de documentos clínicos a la única pantalla del
panel que las sirve, y por eso el avance se venía anunciando una unidad por
delante.

**HECHO, 29 de 29, y Angular retirado el mismo día.** Ver
[Angular fuera](#angular-fuera-el-panel-es-el-producto-2026-08-08). Y el banco de
navegador ya alcanza las pantallas con sesión. Sigue faltando EXTRAER el arnés del
banco de navegador —copiado de informes— que la directiva 9 pide extraer ahora
que va por su tercer proyecto.

### La pantalla del RIPS: revisar antes de descargar (2026-08-08)

`crates/medico-api/src/vista/rips.rs`. Es la pantalla 18 de 28 y la primera que
no es un listado: dos fechas, un botón que REVISA el periodo y otro que descarga
el archivo.

**El paso de revisión es la única diferencia de fondo con la de Angular, y es
donde estaba el desperdicio.** Aquel componente pedía `/api/rips` y, si faltaba
un dato obligatorio, enseñaba `err.error?.mensaje` — una línea: «faltan 37 datos
obligatorios»—. El servidor ya devolvía en ese mismo 422 **la lista exacta**: qué
archivo, qué registro, qué campo y por qué. Se tiraba entera. Enseñarla es lo que
convierte la pantalla en lo que `auditoria_rips.py` hace en el Python: corregir
doce consultas hoy en vez de descubrir en un mes que se reportaron mil.

**No se reimplementa nada del RIPS.** `rips.rs` se repartió en dos funciones y
`GET /api/rips` pasó a ser una de ellas: `generar_documentos` (las cinco
consultas, el recolector compartido, el resumen antes de cerrar) y `exportar`,
que llama a la anterior **y anota la exportación en la misma función**. La
pantalla usa la primera para revisar y la segunda para descargar, así que la
línea de la Ley 1581 no se puede olvidar: no hay forma de exportar sin anotar.
El `Fallo` es un enum y no una `Response` ya armada, porque los dos consumidores
lo dibujan distinto —JSON uno, pantalla el otro— y los códigos y cuerpos de
`/api/rips` quedaron byte a byte los de antes.

**Revisar NO deja eslabón; descargar SÍ**, y las dos mitades tienen prueba. Es la
regla que ya estaba escrita —la exportación se anota cuando el archivo llegó a
salir; un 422 por datos faltantes no deja rastro— y la revisión es exactamente
eso: una generación que no entrega nada y que no enseña un solo dato
identificable. Hay una segunda razón, práctica: se revisa una y otra vez mientras
se corrige, y un eslabón por pulsación ahogaría la cadena.

**La descarga va por POST**, no por un enlace: exporta dato personal y deja
eslabón, así que no puede dispararla nada que precargue enlaces — el mismo
razonamiento que `/panel/salir`. Y con `permitir_supuestos` sigue en pie la
puerta de siempre: un periodo incompleto no se entrega salvo que se pida, y el
botón que lo pide lleva `data-confirmar` (dos pulsaciones) porque firmar un RIPS
con datos supuestos tiene efectos ante la EPS y la Supersalud.

**Es el primer `hx-swap` del panel, y eso trae una trampa nueva.** Con
`hx-target="#resultados"` —una letra de más— el servidor responde el mismo 200
con el mismo fragmento, htmx no encuentra dónde meterlo y **no pasa nada**: ni
error, ni pantalla en blanco, el botón no hace nada. Lo cierra
`el_destino_del_intercambio_existe_en_la_propia_pantalla`, que LEE el destino del
HTML servido en vez de escribirlo en la prueba, y su pareja, que exige que lo
que vuelve sea un fragmento y no una página entera dentro de un `<div>`. Muere
con el mutante de la letra de más. Y **desde el 2026-08-08 también está medido que el navegador HACE el
intercambio**: el banco entra por el formulario, pulsa y comprueba que el destino
—vacío antes— queda con la revisión del periodo pedido. El mutante de la letra de
más muere ahí.

Ocho pruebas nuevas, todas con pareja, y cuatro mutantes muertos: el menú sin
filtro de capacidad, la pantalla sin `EnSesionCon<GenerarRips>`, la descarga sin
bitácora y el destino del intercambio con una errata.

### Imágenes diagnósticas: la primera pantalla del panel que ESCRIBE (2026-08-08)

`crates/medico-api/src/vista/imagenes.rs`. Bandeja del servicio + informe
radiológico: guardar, firmar, iniciar el estudio y adjuntar un archivo. Las
dieciocho anteriores solo leían.

**Dos URL donde Angular tenía un `signal<'lista'|'informe'>`.** Ese estado
imitaba lo que una dirección hace gratis: con `/panel/imagenes` y
`/panel/imagenes/{id}` el botón «atrás» funciona, un informe se puede enlazar y
recargar no pierde el sitio.

**POST normal y redirección 303, no htmx.** Escribir y repintar la pantalla
entera es lo correcto cuando el resultado cambia media pantalla —al firmar, el
formulario se convierte en un documento de solo lectura— y además funciona sin
JavaScript. El 303 convierte la recarga en un GET, así que no repite la
escritura. Lo que salió bien viaja en `?aviso=`, **traducido por una lista
cerrada**: el valor llega del cliente, y aceptar texto libre sería dejar que
cualquiera eligiera lo que la pantalla afirma.

**La protección contra CSRF ya existía y no se reinventa**: la cookie de sesión
es `SameSite=Strict`, así que un formulario de otro sitio no la lleva y el POST
llega sin sesión, o sea a la entrada. No hay token que añadir.

**Las acciones se dibujan solo para quien puede hacerlas.** Leer no exige
capacidad —el médico que pidió el examen tiene que poder ver su resultado, y
`GET /api/imagenes/ordenes` es `Autenticado`—; las cuatro escrituras exigen
`EscribirImagenologia`, la misma que sus rutas JSON. Por eso `PANTALLAS` la
lleva con `exige: None` **a propósito**: poner ahí la capacidad de escritura
habría cerrado la pantalla a quien la API deja entrar.

**Y eso ENSANCHA lo que hacía el sidebar de Angular**, que la ocultaba salvo a
radiólogo, admin, médico y superadmin. Es deliberado y hay que decirlo: aquella
lista de roles vivía en el navegador y no se correspondía con lo que el servidor
permite. Un menú que esconde una puerta abierta es la mentira de #534 mirada por
el otro lado. Si esas órdenes no deben verlas todos, la regla va en la API.

**Dos huecos que la migración destapó y que NO se tapan aquí** (tareas #3 y #4):

- **Los tres campos obligatorios del informe solo se exigían en el navegador.**
  **CERRADO el 2026-08-08**, ver
  [La obligación del informe](#la-obligación-del-informe-radiológico-está-en-la-firma-2026-08-08).
- **Ni la bandeja ni el informe dejaban eslabón.** **CERRADO el 2026-08-08** en
  los dos sitios a la vez, ver
  [La bitácora de imagenología](#la-bitácora-alcanza-imagenología-y-el-freno-que-la-frenaba-no-existe-2026-08-08).

Siete pruebas, todas con pareja, y **cinco mutantes muertos**: la escritura sin
capacidad, el formulario pintado sin mirarla, el aviso de la URL reflejado tal
cual, el filtro de estado sin comprobar y `adjuntar` fabricando el informe que
falta.

### Ambulancias: la regla del despacho deja de estar escrita dos veces (2026-08-08)

`crates/medico-api/src/vista/ambulancias.rs`. Parque, traslados, alta, ficha y
despacho — cinco URL donde Angular tenía dos pestañas y dos `MatDialog`.

**Lo que de verdad cambia es `ambulancia::motivo_para_no_despachar`.** El
componente calculaba las ambulancias despachables con un `computed` que repetía
la regla del servidor, y su propio comentario lo admitía: *«Es la misma regla que
aplica `ambulancia::despachar`, escrita aquí para no ofrecer lo que va a
fallar»*. Ofrecer solo lo que puede salir es correcto —hacer que el usuario
descubra la regla chocándose con un 409 es enseñársela dos veces—; el problema es
la copia, que se queda atrás en el primer arreglo y entonces la pantalla ofrece
lo que el servidor niega. La regla se extrajo, `despachar` la usa, y la pantalla
la CONSULTA (directiva 1).

Con ella se hace además algo que Angular no hacía: **decir por qué** no sale cada
una —«está en el traslado #4», «el SOAT venció el 2020-01-01», «está Dada de
baja»— en vez de esconderla. Una ambulancia que desaparece de la lista sin
explicación se lee como un fallo del sistema.

**Y el orden de la regla no estaba medido.** El banco de `despachar` cubría los
tres motivos por separado, pero no cuál se contesta cuando concurren: se
comprobó con un mutante que invierte el orden y **solo la prueba nueva se puso
roja**. Una ambulancia ocupada Y con el SOAT vencido tiene que decir lo primero,
que es lo que se puede resolver cerrando un traslado.

**El resumen es del parque ENTERO, no de lo filtrado**, y ahí había un defecto
real en la pantalla anterior: las cifras se calculaban sobre la lista ya
filtrada, así que «SOAT vencido: 0» aparecía en cuanto alguien tecleaba en el
buscador — un cero que significaba «no en estos resultados» y se leía como
«ninguna». Aquí son dos consultas y cada una dice lo suyo.

**Los errores con dato dentro NO se redirigen.** El aviso de la lista cerrada
—`?aviso=creada`— solo sirve para los finales sin datos. «Ya hay una ambulancia
con placa AAA111» o «está en el traslado #4» vuelven a dibujar el formulario con
el motivo **y con lo que la persona había escrito**: teclear ocho campos otra vez
es lo que hace que la gente rodee el formulario.

**Dos decisiones pequeñas que evitan un fallo cada una**: la baja **alterna** en
vez de recibir `activa` del cliente —un formulario reenviado desde el «atrás»
reactivaría lo que alguien acaba de dar de baja— y la edición **no manda
`estado`**, que lo pone el despacho y lo quita el cierre; si se pudiera fijar a
mano dejaría de decir dónde está el vehículo.

Mirar el parque solo pide sesión, igual que `/api/ambulancias`; las seis
escrituras exigen `GestionarAmbulancias` y sus botones solo se dibujan para quien
puede pulsarlos.

Diez pruebas nuevas —nueve de pantalla y una del dominio—, todas con pareja, y
**ocho mutantes muertos**, entre ellos el que reproduce literalmente el defecto
de Angular: calcular el resumen sobre lo filtrado.

### Estadísticas: los gráficos se dibujan en el servidor (2026-08-08)

`crates/medico-api/src/vista/estadisticas.rs`. Siete paneles —resumen, citas,
pacientes, diagnósticos, producción, financiero y urgencias— donde Angular tenía
siete pestañas y **catorce gráficos de `chart.js`**.

**`chart.js` no viene, y no hacía falta.** Es una dependencia de npm con su
`node_modules` detrás, que es justo lo que el cambio de stack quitó (directiva
26: el frontal de este producto es htmx y código nuestro). Lo que esos paneles
enseñan son distribuciones y series, y las dos se dibujan con rectángulos:
`piezas::barras` y `piezas::columnas`, con los números al lado. **Lo que se
pierde son los anillos y las tartas**, y conviene decirlo: un anillo de cinco
porciones se lee peor que cinco barras con su porcentaje escrito, pero es un
cambio de aspecto que quien usaba la pantalla va a notar.

**Y se dibujan en SVG, no con `style="width:X%"`.** El `'unsafe-inline'` de
`style-src` estaba en la CSP **por Angular** —su propio comentario lo decía— y el
día que Angular saliera había que poder cerrarlo. **Se cerró el 2026-08-08**, y
esta decisión es la que lo permitió. Una barra con estilo en línea sería
una razón nueva para dejarlo abierto, escrita por el frontal que vino a
sustituirlo. En SVG el ancho es un atributo de geometría, no CSS. Lo vigilan dos
pruebas: una sobre la pieza y otra sobre los siete paneles enteros.

**El periodo lo resuelve la MISMA función que `/api/estadisticas`**
(`RangoEstadistica::resolver`, que pasó a `pub(crate)`), con sus dos defectos
—mes en curso para actividad, año en curso para acumulados— y su rechazo de una
fecha ilegible con 400. Copiarla habría hecho que los mismos filtros dieran
periodos distintos según el frontal, que es la clase de diferencia que nadie ve
hasta que dos informes no cuadran. **El periodo efectivo se escribe en la
pantalla**: sin eso, una cifra del mes y una del año se ven igual.

**Tres cosas que la pantalla dice y la anterior callaba**: que el resumen y el
padrón no dependen del filtro de fechas (ofrecerlo y que no hiciera nada es
peor que no ofrecerlo), que los diagnósticos son los DIEZ primeros y no todos, y
que el indicador de urgencias es la ESTANCIA y no el tiempo de espera — que es
un dato que este sistema no registra y por eso no se afirma.

Y el banco de humo del panel no bastaba: solo mira la fila de `PANTALLAS`, o sea
el resumen. Los otros seis paneles los recorre ahora
`los_siete_paneles_responden_y_ninguno_revienta`, que es la misma prueba que la
API ya tenía — el lado Python tuvo dos paneles devolviendo 500 durante meses
porque ninguna prueba los invocaba.

Ocho pruebas nuevas y **cinco mutantes muertos**: el aislamiento perdido
(consultar como superadmin), la fecha ilegible dejada pasar, el financiero
arrancando en el mes en vez de en enero, la barra volviendo al estilo en línea y
el porcentaje dividiendo por cero.

**`valor_facturado` = FACTURADO, no «todo» (2026-08-09, #535).** Había tres
totales con tres definiciones y el usuario veía dos sin saber por qué:
`estadisticas::financiero` sumaba los SEIS estados (borrador y anulada incluidos),
`resumen::valor_pendiente` suma BORRADOR+RADICADA y `facturacion::stats` suma
RADICADA+ACEPTADA+GLOSADA. Decisión de Juan: «facturado» = **emitido y no
anulado** = Radicada, Aceptada, Glosada, Pagada — la definición vive en
`EstadoFactura::es_facturado` (una sola fuente), y `financiero` renombró
`total_valor` a `valor_facturado` y filtra por ella (el desglose `by_estado`
sigue trayendo los seis). Los tres paneles quedan etiquetados por lo que enseñan:
«facturado (emitido, sin borradores ni anuladas)», «valor pendiente de cobro» y
«radicado». Contar el borrador inflaba la cifra que el gerente reporta.

### Laboratorio: la forma del resultado sale del navegador (2026-08-08)

`crates/medico-api/src/vista/laboratorio.rs`. Bandeja, resultado y perfiles de
examen — la hermana de imagenología, con la que comparte todo lo estructural.
Lo propio de aquí es que **el resultado no es un texto: es una tabla de
parámetros** con valor, unidad, referencias y marca de crítico.

**`resultados_json` sigue siendo un `Value` libre, pero su forma ya no la
escribe el navegador.** La escribía `_filasToPayload` y la leía
`_resultadoToFilas`, las dos dentro del componente de Angular; con un segundo
frontal esa forma pasaba a tener dos autores, y dos autores de un formato JSON
es un formato que se separa. Subió a `medico_datos::diagnostico::Parametro`, con
`parametros_de`, `json_de_parametros` y `parametros_de_perfil`, y con su ida y
su vuelta probadas. **No es una validación**: `crear_resultado` sigue aceptando
el `Value` que le llegue.

Y la lectura es **tolerante a propósito**: un `resultados_json` que no sea un
objeto devuelve la lista vacía y un parámetro con forma rara conserva su nombre.
Fallar dejaría sin abrir un resultado ya guardado, y eso es una historia clínica.

**Las filas dinámicas, sin JavaScript**: el formulario dibuja las que hay más
tres en blanco, y al guardar se descartan las que no tienen nombre —lo mismo que
hacía `_filasToPayload`—. Se piden tres parámetros por pasada; para un hemograma
entero está el perfil.

**Y los campos llevan el índice en el NOMBRE (`critico_1`), no como listas.** Es
la decisión que evita el fallo peor de esta pantalla: una casilla sin marcar no
se envía, así que con `critico[]` la columna se desalinea y el crítico del
potasio aterriza en la hemoglobina — un valor normal marcado como crítico y uno
crítico pasando por normal, los dos a la vez. Hay una prueba con el crítico en la
SEGUNDA fila y un mutante que reproduce la desalineación.

**Cargar un perfil no guarda nada** (`?perfil=N`) y **no pisa lo ya medido**:
quien elige un perfil por error no puede perder los valores escritos. Un perfil
propone qué se mide, no qué salió.

**Un resultado sin ningún parámetro con nombre no se guarda, y se dice por qué.**
En Angular el botón estaba deshabilitado; aquí lo decide el servidor y además lo
explica.

Ocho pruebas de pantalla, cuatro del dominio y **siete mutantes muertos**. Dos
sobrevivieron en la primera pasada y los dos fueron culpa del banco, no del
código: uno era un mutante inerte —el `valor` de un perfil se ignora dos veces— y
el otro se corrió **contra el crate equivocado**, así que la prueba no existía
allí y el cero pasó por verde (directiva 30). El primero se cerró reforzando la
prueba del dominio para que un perfil con un `valor` escrito a mano por `/api` no
pueda proponer el resultado de otro paciente.

### Agenda médica, y el menú deja de tener huérfanas (2026-08-08)

`crates/medico-api/src/vista/agenda.rs`. Horarios mensuales, bloqueos e
indicadores — las tres pestañas y los dos diálogos de la pantalla de Angular,
convertidos en tres URL.

**No es un módulo del catálogo, y hereda esa decisión de `/api/agenda`**: añadir
uno cambia lo que se vende, y eso es decisión comercial. Sigue en pie la pregunta
anotada: `TURNOS` figura como pendiente mientras `/api/agenda/turnos` responde
gratis.

**Y ahí estaba el problema de fondo: exenta de licencia se había vuelto exenta de
MENÚ.** `/panel/usuarios` llevaba desde el lote 3 sirviéndose sin que nada
enlazara a ella —solo se llegaba tecleando la URL— y la agenda iba a ser la
segunda. Dos huérfanas son suficiente evidencia. Nace `panel::SIN_MODULO`, con su
grupo «Administración» en el menú y su capacidad por fila, y **no es la segunda
lista de #534**: cada ruta está en `PANTALLAS` o en `SIN_MODULO`, nunca en las
dos, y lo vigila `las_pantallas_sin_modulo_estan_exentas_de_licencia`, que exige
que cada fila esté declarada exenta con su motivo y que ninguna sea de un módulo.
De paso, el banco de humo del panel pasa a recorrerlas: hasta hoy tampoco las
miraba nadie.

**Los bloques no se dibujan por día de la semana**, que sería más simple y estaría
mal: perdería la jornada partida —«lunes de 8 a 12» y «lunes de 14 a 18»—, que en
una IPS es la norma. Van como filas libres con su día, y hay una prueba que crea
las dos y un mutante que reduce la lectura a una sola.

**Una agenda SIN bloques se dice**, porque no ofrece un solo turno y eso no es lo
mismo que «el profesional no atiende ese mes» — aunque la pantalla de citas los
vea igual.

**Los tres silencios de los indicadores se enseñan tal cual**: «sin datos» no es
cero (cero sería el mejor resultado posible y el más engañoso), las citas sin
fecha de solicitud se cuentan aparte, y las que traen una solicitud posterior a
la cita no se promedian — darían días negativos que MEJORAN el indicador.

Ocho pruebas de pantalla, un guardián nuevo, y **siete mutantes muertos**. Uno de
ellos destapó una aguja mal elegida: «no ofrece turnos» aparece también en el
texto que explica la casilla de agenda activa, así que como aguja casaba siempre
y su pareja no medía nada. Lo dijo la pareja al ponerse roja.

### Nutrición: el IMC deja de calcularlo el navegador (2026-08-08)

`crates/medico-api/src/vista/nutricion.rs`, y sobre todo
`medico-dominio/src/antropometria.rs`.

**El índice de masa corporal lo derivaba el NAVEGADOR y el servidor lo guardaba
tal cual.** `calcularIMC()` en el componente de Angular lo metía en el cuerpo del
POST y `crear_valoracion` hacía `.bind(v.imc)`, así que
`{"peso":70,"talla":1.70,"imc":18}` entraba en la historia clínica **con un
índice que contradice su propio peso y su propia talla**, y nada lo decía. No era
explotable ni una fuga entre instituciones: era un valor DERIVADO guardado como
si fuera medido — la directiva 32 en su forma más cara, con el derivado firmado
por el cliente.

Ahora lo deriva el servidor. La fórmula sube al dominio y es **la del sistema
vivo**, no una nueva: `round(peso / talla², 1)`, igual que `antropometria.py:78`
y que el `Math.round(x*10)/10` del componente. Si aquí se hubiera elegido otro
redondeo, el informe nuevo daría números distintos de los que el prestador viene
reportando, que es la condición de fidelidad de todo el port.

**Y editar el peso mueve el índice**, que era la otra mitad: el
`imc=COALESCE(?,imc)` de la edición lo dejaba como estaba. Se recalcula con el
peso y la talla que queden DESPUÉS de la edición, leyendo la fila antes de
escribir.

`imc` sigue en el cuerpo de `/api` y **se acepta y se ignora**, que es el mismo
criterio que `parcial` en `/api/rips`: no se le rompe la llamada a quien la tenga
escrita, y un campo que se ignora no puede mentir. Sin peso o sin talla no hay
valoración —el esquema tiene `imc REAL NOT NULL`— y eso es una variante propia
(`Alta::SinAntropometria`), no un negativo más: «este paciente no es tuyo» y
«este peso no puede ser» se corrigen distinto, y colapsarlas mandaría a pedir
permisos a quien lo que tiene es un cero en la báscula.

**Segundo defecto, en la misma pantalla: el filtro por riesgo no filtraba.**
`nutricion.service.ts` mandaba `?riesgo=ALTO` desde el selector y `FiltroNutri`
solo leía `paciente_id`, así que elegir «Alto» devolvía la lista entera. Sin
error y sin aviso — y quien busca a los pacientes en riesgo alto se los llevaba a
todos, que es peor que un fallo. Se añadió al filtro, y la pantalla **valida el
valor antes de consultar**: un riesgo que no existe no se pasa a la consulta,
porque devolvería la lista vacía y «no hay ninguno en riesgo alto» es justo lo
que no se puede decir por error aquí.

Seis pruebas de pantalla, seis del dominio y de datos, y **ocho mutantes
muertos**. Uno sobrevivió en la primera pasada por ser inerte: revertir la
edición a `COALESCE` sobre un valor que nunca es nulo no cambia nada. Replantado
con el código exacto de antes —`COALESCE(?,imc)` con el `imc` del cliente— murió,
y mató también la prueba del dominio.

### Quirófano: los botones los decide la máquina de estados (2026-08-08)

`crates/medico-api/src/vista/quirofano.rs`. Agenda quirúrgica, lista de
verificación, salas y las cuatro transiciones.

**La pantalla PREGUNTA qué transiciones caben; no las deduce.** El componente de
Angular decidía con su propia lectura del estado qué botones pintar, y el
servidor volvía a decidirlo al recibir: la regla enseñada dos veces, y la copia
libre de quedarse atrás. Aquí se llama a `medico_dominio::quirofano::transicion`,
así que un botón imposible no llega a dibujarse. Hay un mutante que los dibuja
todos y la prueba lo caza sobre los tres estados.

**SUSPENDIDA dejó de ser inalcanzable (2026-08-09, #537).** El dominio ya tenía
el estado y sus dos salidas (reanudar y cancelar) —la corrección sobre el Python,
donde una suspendida se quedaba sin salida—, pero no había verbo que lo alcanzara:
faltaba la ruta. Ahora `PUT /api/quirofano/programaciones/{id}/suspender` (y
`POST /panel/quirofano/{id}/suspender`), con `AtenderUrgencias`, la misma que
iniciar. **Y reanudar NO pisa la hora del primer inicio**: reanudar es el verbo
`Iniciar` sobre una suspendida, y `mover` lo distingue del primer inicio por el
estado de partida —desde PROGRAMADA fija `hora_inicio_real`, desde SUSPENDIDA fija
la columna nueva `hora_reinicio` (migración 0052) y no toca la primera—. Son dos
datos, no uno (directiva 32): sin la separación, reanudar perdía la marca del
primer inicio en silencio. El panel llama «Reanudar» al botón cuando el estado es
SUSPENDIDA, para no llamar «iniciar» a lo que continúa algo ya empezado.

**El vocabulario del estado también sale del dominio.** `ESTADO_LABEL` vivía en
el frontal; con dos pantallas serían dos diccionarios y el mismo estado se leería
distinto en cada una. `EstadoCirugia::etiqueta` lo dice una vez, y `TODOS` da el
orden del filtro para que no se quede corto cuando entre uno nuevo.

**Dos capacidades distintas dentro de la misma pantalla, y ninguna en la tabla.**
PROGRAMAR exige `EscribirClinico` —es un acto de la historia— y CONDUCIR la
cirugía exige `AtenderUrgencias`, la de quien está en la sala. Poner cualquiera
de las dos en la columna `exige` de `PANTALLAS` habría cerrado la pantalla a
quien la API deja mirar, así que va `None` y cada manejador declara la suya. La
prueba lo mide con enfermería, que conduce y no programa.

**Desmarcar una casilla la desmarca**, y vaciar la hemoclasificación la BORRA:
el formulario manda las cuatro casillas con su valor resultante, así que un
`false` explícito llega como `false` y no como «no lo toques». En una lista de
verificación prequirúrgica «no lo sé» y «O+ equivocado» no son lo mismo — es la
decisión que el dominio ya había tomado y que la pantalla tenía que respetar.

**Lo que falta de la verificación se dice por su nombre y NO impide iniciar**:
quién puede iniciar lo decide la máquina de estados, y una urgencia se opera con
la lista a medias. Lo que no puede pasar es que se inicie sin que nadie se entere
de qué falta.

**Y el listado deja el MISMO eslabón que su ruta JSON**, que aquí no es un
detalle: cruza titulares y devuelve nombre, documento y qué cirugía se le
practica a cada uno.

Siete pruebas y **ocho mutantes muertos**. Dos agujas mías salieron mal y las
dijeron las propias pruebas: el estado de una sala ocupada es `EN_USO` y no
`OCUPADO`, y `<option value="2"` casaba en tres selectores a la vez —hay una
usuaria con id 2— así que la comprobación se hace ahora dentro del selector de
salas y no sobre la página entera.

### Farmacia: la pantalla más grande, y las dos puertas que no abre (2026-08-08)

`crates/medico-api/src/vista/farmacia.rs`. Inventario, fórmulas pendientes,
dispensaciones, movimientos y proveedores — 1 342 líneas de Angular y cinco
pestañas, convertidas en cinco URL.

**Lo que había que no romper eran dos puertas que este módulo cierra a
propósito:**

1. **Las existencias no se editan a mano.** Solo suben o bajan con una entrada,
   una salida o un ajuste, y las tres escriben su movimiento en la misma
   transacción. La ficha no ofrece el campo, y mandarlo igual en el cuerpo no
   mueve nada. El mutante que lo abriría **no se pudo plantar**: `ItemEdicion` no
   tiene `cantidad_actual`, así que la garantía es del TIPO y no del
   comportamiento — se dice porque un mutante que no compila no es un mutante que
   sobrevive, y la diferencia importa.
2. **Un lote distinto no se mezcla con el que ya hay.** Sumar dos remesas en una
   fila las vuelve indistinguibles: el sistema diría que hay 25 unidades del lote
   nuevo cuando 5 son del viejo, con otra fecha de vencimiento. El día de un
   retiro sanitario del INVIMA, eso es no saber a quién se le entregó qué. La
   pantalla lo dice con el dato que hace falta —«quedan 10 del lote L-100»— en
   vez de redirigir con un aviso genérico.

**Y una tercera que se conserva por omisión**: el proveedor de una entrada es
OPCIONAL, porque hay entradas que no son una compra —una donación, un traslado
entre sedes, un ajuste de conteo— y obligar a inventar uno produciría el dato
falso que este módulo evita en todo lo demás. Lo que no se acepta es uno ajeno o
inactivo: eso no se registra y se dice.

El resumen es del inventario ENTERO, no de lo filtrado — la misma lección que
dejó la pantalla de ambulancias.

Seis pruebas y **seis mutantes muertos** de los siete intentados; el séptimo no
existe como mutante, ver arriba.

### Facturación: los destinos los decide la máquina de estados (2026-08-08)

`crates/medico-api/src/vista/facturacion.rs`. Facturas, sus líneas, las glosas y
los contratos — 1 531 líneas de Angular, la pantalla más grande del producto.

**Los destinos de estado salen de `EstadoFactura::siguientes`, no de una lectura
propia.** El componente pintaba «Anular» para cualquier factura que no estuviera
pagada ni anulada, y de un BORRADOR **no se anula**: solo se radica. Ese mismo
`Anulada` que la pantalla ofrecía ni siquiera existía en el `CHECK` de la
migración 0044 hasta que la 0045 lo añadió — o sea que el botón llevaba a un
rechazo, y antes de la 0045, a un error de esquema.

**Lo aprendí escribiendo la prueba al revés.** La escribí esperando que un
borrador se pudiera anular —porque eso es lo que hacía la pantalla de Angular— y
se puso roja: el dominio devuelve `[Radicada]` y tenía razón. Es la segunda vez
en dos turnos que una suposición mía sobre «lo que hacía antes» resulta ser el
defecto que el port ya había corregido.

**Y una regla que estaba escondida sube al dominio**: `EstadoFactura::admite_glosa`.
Estaba dentro de `agregar_glosa` como un `matches!`, y la pantalla la necesitaba
para no ofrecer un formulario que el servidor iba a rechazar — la EPS glosa lo
que RECIBIÓ, y un borrador no ha salido de aquí. Con la regla en dos sitios una
copia se queda atrás; preguntándola, no.

**Lo que no se puede editar no se ofrece**: solo un BORRADOR admite líneas
nuevas y radicación, y una pagada o una anulada son terminales. Y los importes
se pintan, no se recalculan: `valor_bruto` y `valor_neto` los calcula el dominio
de datos en la misma transacción que los escribe.

Seis pruebas de pantalla, una del dominio, y **ocho mutantes muertos** — entre
ellos el que ofrece los seis estados como hacía Angular y el que deja glosar un
borrador.

### Administración: la pantalla de más privilegio (2026-08-08)

`crates/medico-api/src/vista/admin.rs`. Ocho pestañas —institución,
configuración, personal, seguridad, campos propios, catálogos, capacidades y
auditoría— sobre las 26 rutas de `/api/admin`. Es la que crea gente, le cambia el
rol y le restablece la contraseña.

**Las tres escrituras de personal se extrajeron a `rutas.rs` y los manejadores
JSON pasaron a ser traductores.** `dar_de_alta_personal`,
`restablecer_password_de` y `editar_personal` llevan dentro las cinco puertas
—ascenso, ámbito, licencia, política de contraseña y bitácora obligatoria— y
devuelven `EscrituraPersonal`, que cada consumidor dibuja a su manera. Escribir
la pantalla contra las rutas JSON habría duplicado el orden de esas puertas, que
es justo donde la revisión del PR #49 encontró tres huecos.

**Y de ahí salió una distinción que faltaba: la decisión es UNA, la forma de
contarla son DOS** (directiva 32). La licencia viaja como
`EscrituraPersonal::SinCupo(licencia::Puerta)`, y `/api` la convierte en su clave
—`sin_licencia`, `cupo_lleno`, `cupo_de_medicos_lleno`, que es lo que un cliente
distingue sin leer castellano— mientras el panel la convierte en una frase con
`frase_de_licencia`. La primera versión enseñaba la clave: el administrador de
una IPS recibía **`sin_licencia`** en pantalla, o sea el nombre interno de un
problema que además no es suyo, porque es comercial y no un permiso que le falte.
Lo mismo pasaba con `{rechazo:?}` en la matriz de capacidades —`Rechazo` ya tenía
`Display` con la frase escrita— y con `{motivo:?}` en la configuración, que tenía
`Invalido::mensaje()`. Tres sitios, el mismo descuido.

**La pestaña de seguridad solo deja editar lo que este binario LEE**, y ésa es la
corrección de fondo. `politica_seguridad` tiene 21 columnas y se aplican cinco;
la primera versión de la pantalla ofrecía once campos y `guardar_seguridad`
escribía siete, así que cuatro se tecleaban, se enviaban, se contestaban con
«política guardada» y volvían al valor de antes al recargar — el defecto que esta
migración lleva encontrando en cada pantalla, esta vez escrito por mí. Hoy:

- **editable = las cinco que se aplican**, ni una más, y el `POST` escribe
  exactamente esas cinco: mandar `max_intentos_login=99` en un formulario
  reconstruido a mano no cambia nada;
- **las otras dieciséis se enseñan con su valor guardado y su motivo, sin poder
  tocarlas** — quien las configuró tiene derecho a ver qué quedó escrito, y
  cambiarlas no cambiaría nada;
- la lista se **deriva de `adm::POLITICA_APLICADA`**, la misma tabla que el
  guardián de `medico-datos` compara contra el tipo, así que una columna nueva
  aparece en pantalla sola y una que pase a aplicarse pone roja la prueba.

`POLITICA_APLICADA` nació en este cambio (`medico-datos/src/administracion.rs`),
convirtiendo en dato la tabla que vivía en un doc-comment — que ninguna pantalla
puede leer. Al escribir su guardián apareció una columna que la prosa se había
dejado: **`registrar_todos_los_accesos`**, que tampoco manda nada, porque la
bitácora de la Ley 1581 registra siempre y un interruptor que la apagara sería
incumplir por configuración.

**`/panel/admin/auditoria` es la única pestaña que se vende por módulo.**
`AUDITORIA` está en el catálogo, así que va en `PANTALLAS` con su fila y una
institución que no lo contrató recibe el 403 de la capa; las otras siete van en
`PANEL_FUERA_DEL_LICENCIAMIENTO` con su motivo, y `/panel/admin` en
`SIN_MODULO`, con el grupo «Administración» del menú.

**Reactivar consume asiento, y el panel lo dice.** Es la regla que el Python tenía
abierta por la mitad —el `PUT` no toca `activo` y `reactiva` no lo veía—, así que
el tope se rodeaba desactivando y volviendo a activar. Desactivar no se estorba
nunca: libera asiento.

Once pruebas de pantalla y **diez mutantes muertos**, entre ellos el que vuelve a
ofrecer las columnas muertas, el que las escribe aunque no se ofrezcan, el que
ofrece los roles que esta cuenta no puede conceder y el que deja reactivar sin
mirar el cupo.

**Y el primer intento del mutante de la licencia SOBREVIVIÓ, por el arnés y no
por el código**: lo planté en el arm de la reactivación y lo medí con la prueba
del ALTA, que no pasa por ahí. Los arms son TRES —alta, reactivación y
restablecimiento— y cada uno necesita el suyo; hoy hay mutante para los dos
primeros, cada uno apuntado a la prueba que sí recorre ese camino. Es la
directiva 33 en su forma más barata de equivocarse: el mutante compiló, la
prueba corrió y salió verde, y las tres cosas eran ciertas.

**Dos cosas quedan dichas y sin cerrar**, para no confundirlas con lo hecho:

- **La configuración de la institución no restaura lo tecleado al rechazar.** Se
  vuelve a leer de la base, así que un envío con una hora imposible pierde lo
  demás. Se acepta porque el único campo rechazable es la hora y el
  `<input type="time">` la impide en el navegador: el rechazo solo llega por un
  formulario reconstruido a mano. Si aparece una segunda validación, esto cambia.
- **De las 18 columnas de `configuracion_institucion` no se ha medido cuáles
  lee este binario.** Un barrido rápido no encontró lectores fuera de la propia
  pantalla —ni siquiera de `duracion_cita_default`, que la agenda debería usar—,
  y eso es la misma familia que `politica_seguridad`. Queda como tarea: afirmarlo
  sin medirlo sería exactamente el error contrario.

### La historia clínica: el paso deja de ser un número en memoria (2026-08-08)

`crates/medico-api/src/vista/historias.rs`. 2 237 líneas de Angular, tres vistas
en un `signal` y un `mat-stepper` de siete pasos, convertidas en **quince rutas**
bajo una sola fila de `PANTALLAS` —`exige` casa por prefijo—.

**Cada paso es una URL y cada paso GUARDA lo suyo.** `activeStep()` iba de 1 a 7
y no salía del navegador: recargar volvía al principio, «atrás» se salía de la
historia entera y no había forma de enlazarle a un colega el examen físico de una
atención. Y el componente mantenía la historia completa en memoria hasta el botón
final — una consulta se interrumpe, entra una urgencia, se cae la red, y lo
tecleado se perdía sin dejar rastro.

**Ese diseño trae UN riesgo y es el que había que cerrar antes de escribirlo.**
`PUT /api/historias/{id}` **reemplaza las 61 columnas editables de una vez**, así
que una sección que reconstruyera el resto a mano borraría lo que se dejara
fuera: no da error, no rompe nada, y se nota cuando alguien vuelve a abrir la
historia y el examen físico está en blanco. En una historia clínica eso no se
reconstruye. Lo cierra **`historia::como_nueva`**, en `medico-datos` y no en la
capa de pantallas, con su guardián:
`como_nueva_lleva_todas_las_columnas_editables` compara las dos estructuras
SERIALIZADAS y juzga la diferencia contra `SOLO_DEL_SERVIDOR`, donde cada
ausencia legítima está escrita con su motivo. Su mutante —quitarle una sola
columna— mata la prueba de la pantalla.

**Y la extracción de fondo: la forma de los antecedentes deja de vivir en el
navegador.** Las cuatro columnas `ant_*` son JSON sin esquema, y los **65 campos**
que llevan dentro estaban declarados ÚNICAMENTE en cuatro `interface` de
TypeScript. Con un frontal funcionaba; con dos es un formato con dos autores, y
basta que uno escriba el bloque sin un campo para que ese campo **desaparezca del
expediente** al primer guardado desde ahí. No es hipotético: IRA, EDA, esquema de
vacunación, desarrollo psicomotor y muertes de hermanos menores de cinco años son
los indicadores de AIEPI y de crecimiento y desarrollo, o sea la materia prima de
`epidemiologia.py`, y un campo que se cae en silencio no se nota hasta el informe
del mes.

Suben a **`medico-dominio::antecedentes`** como tabla —clave, etiqueta y TIPO—, y
la pantalla se dibuja y se relee de ahí. La tabla **no valida**: `/api` sigue
aceptando el `Value` que le llegue, porque rechazar un bloque con un campo de más
rompería a un cliente que hoy funciona sobre historia clínica ya escrita. Y lo
que haya de más en la columna **se conserva** al guardar: no se sabe qué es, y
borrarlo sería tirar historia clínica por no reconocerla.

**La paridad con Angular se MIDE mientras los dos convivan**
(`antecedentes_paridad_con_angular.rs`): se leen las claves de las cuatro
`default*()` del componente y se comparan con la tabla. El extractor lleva
control positivo —si sacara menos de nueve claves, lo dice en vez de dar verde—,
su caso rojo va contra **fuente sintética** con un campo de más, y se comprobó
además contra el ARTEFACTO quitándole `hosp_eda` a la tabla: rojo con el nombre
del campo. El día que Angular salga del repositorio esa prueba se borra en la
misma pasada, y que falle es el recordatorio.

**Tres cosas que se conservan porque ya estaban bien**: una historia firmada no
se edita **por ninguna puerta** —tampoco añadiéndole un medicamento o una orden,
que sería modificarla por al lado—; el CUPS se valida con la misma función que la
ruta JSON **antes** de escribir; y abrir una sección deja el MISMO eslabón que
`GET /api/historias/{id}`, con el contenido congelado byte a byte.

**Y una que se aparta de Angular a propósito**: los **antecedentes siguen
editándose con la historia firmada**. Son del PACIENTE y no de la atención — el
antecedente sigue siendo el mismo mañana, y cerrarlos con la historia habría sido
el sesgo a cerrar (directiva 21). Tiene su prueba, que es la pareja de la
anterior.

**Dos defectos propios corregidos de paso:**

- **`POST /panel/imagenes/{id}/adjuntar` no tenía el tope de subida.** Le regía
  el defecto de axum —2 MB— mientras su ruta JSON hermana admite 25: la misma
  radiografía entraba por la API y el panel la rechazaba con un 413 que no dice
  nada. Se vio al montar el adjunto de la historia, que necesitaba el mismo
  `DefaultBodyLimit`.
- **Los tipos de orden que escribí de memoria no eran los del esquema.**
  «Procedimiento» no está en el `CHECK` de la migración `0027`: el selector
  ofrecía un botón que lleva a un error de base. Lo dice
  `los_tipos_de_orden_son_los_del_esquema`, que crea una orden de CADA tipo leído
  del propio selector.

**`TODAVIA_EN_ANGULAR` queda VACÍA**, y eso puso en rojo el control positivo de su
propio guardián —que exigía que las dos listas tuvieran contenido—. La respuesta
no era borrar el control: una comprobación que solo recorre artefactos reales,
con uno vacío, sale verde con la regla buena **y con una regla ciega** (directiva
33). La regla se extrajo a `colisiones(migrados, en_angular)`, se corre sobre lo
real —donde hoy no hay nada que encontrar— y sobre **dos listas fabricadas**,
donde tiene que encontrar exactamente una.

Once pruebas de pantalla, dos guardianes nuevos en `medico-datos` y tres en el
dominio, y **doce mutantes muertos**.

**Lo que NO se portó, y se dice**: el buscador incremental de medicamentos y el
de CUPS del paso de diagnóstico. En el panel los dos campos son de texto —el
código se teclea— porque el buscador de Angular es una petición por tecla y aquí
sería un `hx-get` con su destino, o sea el primer intercambio de esta pantalla; y
la única pieza de este panel que hace intercambio sigue **sin medirse en un
navegador de verdad** (tarea #2). Se hace cuando el banco de navegador alcance a
las pantallas con sesión.

### Superadministración: el onboarding de un cliente (2026-08-08)

`crates/medico-api/src/vista/superadmin.rs`. La última pantalla del port, y la
única que **no es de un prestador**: dar de alta una IPS, fijarle el plan,
decidir qué módulos compró y crearle su primer administrador.

**El superadmin no tenía panel, literalmente.** `vista::contexto` resuelve la
institución de quien mira, y el superadmin no pertenece a ninguna: para todas las
pantallas devolvía `SinPanel::Superadmin`, «esta cuenta no tiene panel». El
tablero le hacía una excepción y le dibujaba un listado de instituciones de solo
lectura **sin menú y sin un solo enlace** — o sea que entraba, veía los nombres de
sus clientes y ahí se acababa el producto para él.

Esta pantalla **no usa `Contexto` ni `panel::menu`**: lleva su propio menú y su
puerta es `EnSesionCon<GestionarInstituciones>`, que no depende de ninguna
institución —esa capacidad no se configura desde la matriz de una: un
administrador que se la concediera se estaría regalando el producto entero—. Así
el superadmin gana su pantalla **sin que las clínicas ganen un espectador**: si
`contexto()` empezara a funcionar para él, `/panel/pacientes` le enseñaría el
padrón de todos los clientes.

Y las dos salidas se arreglaron: el tablero **redirige** a la consola (el listado
duplicado se borró, directiva 1) y la página de «esta cuenta no tiene panel
clínico» lleva enlace.

**Dos cosas que ninguna pantalla del producto podía hacer, y ahora sí:**

1. **Los módulos contratados — cierra la tarea #65.6.**
   `PUT /api/instituciones/{id}/modulos` existía en el binario y **ningún frontal
   lo llamaba**: medido, `modulos_habilitados` solo se podía cambiar tocando la
   base. Un alta de cliente que no puede decidir qué se vendió no es un alta. La
   lista que llega **es** la licencia entera —quitar un módulo es una operación
   real—, y un nombre fuera del catálogo se **rechaza**, no se filtra en
   silencio: guardar `TELETRANSPORTE` dentro deja la licencia inerte y nadie sabe
   por qué ese cliente no ve lo que cree haber comprado.
2. **El cupo de MÉDICOS.** `superadmin.service.ts` mandaba `{max_usuarios,
   vence}` y nada más, así que `max_medicos` —la segunda cota, que nació el
   2026-08-06 porque una IPS con plan de 3 médicos podía dar de alta 20— no tenía
   por dónde fijarse. La columna existía, la puerta la miraba y **nadie podía
   escribirla**.

**Y un defecto que salió al dibujar el formulario: `licencia::Estado` leía la
fecha de vencimiento y la tiraba.** Con eso el campo salía en blanco, y guardar un
cupo nuevo convertía la licencia en **perpetua** sin que nadie lo pidiera y sin
que nada fallara. Peor: una perpetua y una que vence hoy daban el MISMO `Estado`,
así que ni siquiera se podían distinguir. Hoy `vence` viaja en `Vigente` y
`Vencida` —que además dice CUÁNDO venció— y su prueba es que los dos casos ya no
son iguales.

**Tres decisiones que se conservan o se acotan:**

- El **NIT es único**: dos IPS con el mismo NIT facturan a la EPS con la misma
  identidad.
- El **primer administrador se crea sin pasar por el cupo**, que es lo que
  `crear_usuario` ya hacía: si lo consumiera, una licencia recién fijada podría
  bloquear al administrador que va a usarla.
- Esta pantalla crea **administradores y nada más**: el rol no viaja en el
  formulario, y mandarlo a mano tampoco sirve. Ofrecer el selector entero aquí
  sería fabricar superadministradores desde el alta de un cliente; al resto del
  personal lo da de alta ese administrador, que es quien conoce a su gente.

**El menú NO enlaza `/panel/usuarios`, y estuvo a punto.** La pantalla de Angular
repetía debajo la tabla de todos los usuarios del sistema y lo obvio era
enlazarla — pero `vista/usuarios.rs` llama a `contexto()`, así que al superadmin
le contestaría «esta cuenta no tiene panel». Habría sido una entrada de menú que
lleva a una página que dice que no hay nada: el mismo callejón que dejó
`/panel/usuarios` huérfano durante tres lotes, visto por el otro lado. El personal
se enseña **por institución**, dentro de su ficha, que es además lo que hace falta
para dar de alta a un cliente. Lo vigila
`ninguna_entrada_del_menu_lleva_a_una_pagina_que_dice_que_no_hay_nada`, que **lee
las rutas del HTML servido** y las visita una por una.

Once pruebas y **diez mutantes muertos**.

### La bitácora alcanza imagenología, y el freno que la frenaba no existe (2026-08-08)

`GET /api/imagenes/ordenes` y `/ordenes/{id}` no llamaban a `registrar_*`, así que
**se podía mirar quién tiene qué placas sin que constara**. `img_historial` sí
anotaba, y ése era el positivo de control de que el hueco era de las otras dos.
El panel se había migrado **respetando el hueco a propósito** —si anotara y la
API no, el rastro de un mismo hecho quedaría partido según qué frontal lo
abriera—, así que se cerró en los dos a la vez.

**Primero se midió el freno, que es lo que la nota pedía**, y resultó que no
existe. Sobre el binario de *release*, con la demostración sembrada:

| | |
|---|---|
| 200 peticiones a una ruta que SÍ anota (`/api/pacientes`) | **1,168 s** |
| 200 peticiones a una que NO anota (`/api/imagenes/ordenes`) | **1,157 s** |
| coste por eslabón | **≈ 0,055 ms** — no se distingue del ruido |
| 100 peticiones concurrentes (20 a la vez) | **100 × HTTP 200**, 0,27 s, ninguna falló |
| tamaño de un eslabón | **232 bytes** → 16 MB/año a 200 aperturas diarias |

El positivo de control de la medición: la ruta que anota escribió **exactamente
200** filas en `acceso`, así que el reloj estaba midiendo lo que se creía. Y el
`BEGIN IMMEDIATE` con reintentos aguantó la concurrencia sin un solo fallo.

**Eso desbloquea también el resto de la familia** —urgencias, hospitalización,
remisiones, incapacidades, agenda—: lo que quedaba ahí no era una decisión, era
trabajo.

**Las tres formas, cada una donde toca** (la regla ya estaba escrita; aquí solo
se aplica):

| ruta | forma | por qué |
|---|---|---|
| bandeja sin filtro | `Coleccion`, **sin ids** | atraviesa pacientes y enseña nombre y documento: es `/api/pacientes` otra vez. Ver un nombre en una lista no es abrir el expediente |
| bandeja con `paciente_id` | `DeUnTitular` | es el expediente de imagenología de UNA persona identificada en la petición — la misma regla que separa `/api/documentos` con filtro del que no lo lleva |
| `/ordenes/{id}` y `/ordenes/{id}/informe` | `Recurso` | una apertura, con los dos ids, y entra en el rastro que el titular puede pedir |

**El `recurso_id` del informe es el de la ORDEN**, que es por donde se pide y por
donde se enlaza: un id distinto del que aparece en la URL haría que el rastro no
se pudiera casar con la petición que lo produjo.

**Y un 404 no deja rastro**, que es la regla que este repositorio ya tenía escrita
para la exportación del RIPS: una bitácora que registra lo que no ocurrió es peor
que una que no registra — quien pruebe números aparecería habiendo «visto»
expedientes que no existen.

Cuatro pruebas y **seis mutantes muertos**, entre ellos el que hace que la
pantalla anote con OTRO verbo que la API —el rastro partido en dos— y el que
guarda los ids en el listado, que diría que el dato SALIÓ del sistema.

### FEV-RIPS en Rust: lo último que quedaba fuera del estándar (2026-08-08)

`medico-dominio/src/rips/fev.rs`. Era **la condición del ADR para apagar el
Python**: en Rust ya estaban el recolector y los cinco documentos de la 948; el
documento ÚNICO que los funde —el que se sube al Mecanismo Único de Validación—
solo existía en `backend/fev_rips.py`. Sin él no hay facturación electrónica, que
es por lo que le pagan al prestador.

**Qué añade sobre los cinco.** Aquéllos producen seis documentos separados, cada
uno con su lista de usuarios: un paciente con consulta y medicamento aparece dos
veces. FEV-RIPS es UNO, con cada usuario una sola vez y todos sus servicios
juntos. Más: el sobre dice qué tipo de documento es, y `numAutorizacion` y
`numFEVPagoModerador` van **`null` y no `""`** —enviar cadena vacía donde el
esquema declara nulable es causal de rechazo, y el JSON sigue siendo válido, así
que no hay error que avise—.

**Los dos sobres, y el CUERPO no cambia entre ellos:**

| | `numFactura` | `tipoNota` | `xmlFevFile` | quién |
|---|---|---|---|---|
| RIPS sin factura | `null` | `"RS"` | vacío | profesional independiente (< 3 500 UVT) |
| FEV en salud | el número | `null` | AttachedDocument | IPS, clínica, hospital |

**Los dos campos nulables obligaron a un cambio de tipo, y es la parte fina.**
`num_autorizacion` y `num_fev_pago_moderador` pasaron de `String` a
`Option<String>`, con los generadores poniendo `Some(...)`: así los CINCO
documentos siguen emitiendo `""` byte a byte como hasta ahora, y el FEV-RIPS
convierte a `null` en la fusión. Un solo juego de structs para las dos formas, en
vez de dos copias de 47 campos.

**PROBADO BYTE A BYTE CONTRA EL PYTHON**, que es lo que el ADR pide:
`fev_rips_paridad_con_python.rs` construye los documentos con los generadores de
Rust, se los pasa al `fev_rips.py` del backend por `stdin`, y **compara las
cadenas**. Los dos sobres coinciden. Si falta `python3` o el árbol `backend/`, la
prueba FALLA en vez de saltarse.

**Lo que esa paridad prueba, y lo que NO** — importa decirlo porque de aquí
depende apagar el Python: prueba **la fusión y el sobre**, que es lo portado. NO
prueba que `medico-datos` y SQLModel traigan las mismas filas de la base; eso es
una propiedad aparte, con sus propias pruebas, y mezclarlas aquí dejaría un rojo
que no dice cuál de las dos falló. **La condición del ADR está cumplida para la
transformación, no para el extremo a extremo.**

**Seis mutantes muertos y un control.** Dos sobrevivieron en la primera vuelta y
los dos eran huecos de la fixture, no del código: los consecutivos del ejemplo ya
coincidían con los globales —se añadió un paciente que aparece **solo** en el AM,
que dentro de su documento es el 2 y en el fundido el 3—, y la regla del «SI de
incapacidad gana» no se recorría nunca, porque con los generadores reales el
«SI» no llega por un documento posterior. Esa regla se probó donde sí se puede
controlar: documentos escritos a mano, en `rips::fev::pruebas`. El control que
NO debe morir: cambiar el mapa interno por un `BTreeMap` no altera la salida,
porque el orden lo lleva el `Vec`.

**Enchufado por las DOS puertas**, y las dos anotan: `GET /api/rips/fev` y
`POST /panel/rips/fev`, las dos por `rips::exportar`, que es la que escribe el
eslabón de la Ley 1581 con los pacientes que cubrió. Dos descargas del mismo dato
personal y una sin rastro sería el rastro partido que este repositorio ya cerró
en imagenología.

El panel pide el **número de nota** en su propio formulario y no escondido en el
otro: es obligatorio, consecutivo y **no se puede repetir entre envíos**
—repetirlo genera rechazo—, así que meterlo oculto sería regalar un rechazo del
validador a quien pulsara sin leer. La puerta va ANTES de exportar, y por eso un
envío sin nota tampoco deja eslabón.

La pantalla ofrece **solo la ruta sin factura**: la FEV en salud exige el XML de
la factura ya aprobado por la DIAN, y ese archivo todavía no vive en este
sistema. Ofrecer un campo para pegarlo sería prometer lo que no se puede cumplir.

El archivo sale con **BOM y sangría de un espacio**, que es lo que escribe el
conversor oficial de MinSalud y lo que trae un archivo real del prestador; y sin
el envoltorio de la API, porque **hay dos formas y confundirlas cuesta un ciclo
de validación**: el archivo es el objeto pelado, el cuerpo de la API lo envuelve
en `{"rips": …, "xmlFevFile": …}`.

### FEV-RIPS de extremo a extremo: la última medición del ADR, en verde (2026-08-09)

`crates/medico-datos/tests/fev_extremo_a_extremo.rs` +
`crates/medico-datos/tests/apagar_python/generar_fev_desde_base.py`. Es la
**condición que faltaba para apagar el Python**: la paridad de la FUSIÓN ya
estaba probada byte a byte (`fev_rips_paridad_con_python.rs`), pero le pasaba a
Python documentos hechos por Rust, así que **no cruzaba la frontera base →
documentos** —que `medico-datos` con SQLx traiga las mismas filas que
`rips_completo` con SQLModel—. Aquí se cruza, con los cinco documentos y un
paciente que aparece en los CINCO.

**No es «una base que ambos leen».** Los dos esquemas divergen (medido el
2026-08-09): el documento del paciente es `tipo_documento`/`numero_documento` en
Rust y `tipo_id`/`identificacion` en Python; el destino de urgencias es
«DOMICILIO» en Rust y «Domicilio» en Python; el AU y el AH no llevan bloque
económico y esas columnas **no existen** en el esquema Rust. Así que el enfoque
es **sembrar datos EQUIVALENTES en cada esquema desde UN `spec` neutral** —la
única fuente de la verdad— y comparar las dos cadenas. Si las dos siembras no
son equivalentes, la culpa es de una traducción y se ve en el diff, no de la
generación.

**Tres decisiones que hubo que tomar antes, y por qué:**

- **La zona del prestador se fija en UTC.** Rust convierte la fecha guardada
  (UTC) a la zona del prestador antes de emitirla; Python emite el naive tal
  cual. Con la zona en UTC la conversión de Rust es la identidad y los dos parten
  del mismo instante de pared. Es correcto para una prueba de EQUIVALENCIA: la
  conversión de zona es comportamiento SOLO de Rust, y su corrección ya tiene sus
  pruebas en `zona.rs`. Los dos, además, ya truncan a minutos en el origen
  (`fecha_dt` / `_fecha_dt`).
- **El orden.** Las cinco consultas de `rips_completo` **no llevan `order_by`**,
  así que el orden de usuarios —y la numeración de `consecutivo`— sale del orden
  de retorno de la base. Rust ordena por `(fecha, id)`. El `spec` siembra las
  historias en ese orden y SQLite las devuelve por rowid = inserción, así que
  casan. Si se rompe, la prueba se pone roja.
- **La incapacidad del escenario va inequívoca** (las dos consultas del paciente
  7 con el mismo valor): Python fija la incapacidad del usuario con la PRIMERA
  historia que ve en el AC, y un valor distinto mediría esa regla de orden en vez
  de la equivalencia base→JSON.

**Discrimina** (directivas 30 y 33): el recolector de Rust va en modo ESTRICTO y
la prueba EXIGE cero faltas —si el escenario dejara un supuesto, se compararían
dos documentos con relleno—; el control positivo comprueba que el documento es no
trivial (dos usuarios, el 7 con sus cinco clases de servicio); y el control rojo
—`quitar_una_fila_a_un_lado_pone_los_dos_fev_distintos`— le quita la
hospitalización a un solo lado y exige que los dos FEV ya no coincidan, que es la
divergencia por la vía real —una fila que un sistema trae y el otro no—.

**Si falta `python3` o el árbol `backend/`, FALLA; no se salta.** El intérprete
preferido es `backend/.venv/bin/python`, igual que la prueba de fusión.

**Verde el 2026-08-09** sobre `HEAD` de `fix/licencia-en-el-servidor`. Con esto
la condición del ADR está cumplida **de extremo a extremo**, no solo para la
transformación. Lo que queda es la RETIRADA del Python (decisión ya dada por Juan
el 2026-08-08): `main.py` + los 30 `routers/` + los tests que pasan por
`TestClient`/`main` + el servicio `backend` de `docker-compose` + el job
`backend` del CI. **Se conserva la cadena RIPS/FEV-RIPS** —`rips_completo.py`,
`fev_rips.py`, `models.py`, `database.py`, `cie11.py` y sus catálogos—, que es
autónoma (no importa nada de `main`/routers) y es el ORÁCULO de esta prueba y de
la de fusión: si se borrara, las dos dejarían de medir.

### El cliente del MUV en Rust: la cadena termina dentro del binario (2026-08-10)

`crates/medico-muv`. Salió de VERIFICAR #274 contra el árbol en vez de contra la
prosa: el binario **generaba** el FEV-RIPS y no lo **subía**. `reqwest` vivía en
un único crate —`medico-ihce`, y para IHCE/RDA—, así que `login → cargar → CUV`
seguía siendo `backend/muv_client.py`. O sea que la cadena por la que le pagan al
prestador terminaba fuera del producto, y ninguna ficha lo decía con esas
palabras.

**Portado con su banco: las 22 pruebas del Python, una a una, más cinco propias.**
Las cinco nuevas salieron de cosas que en Python no podían fallar y en Rust sí
—el token vacío, el recorte multibyte— o que el Python no tenía —el `Destino`
como enum—.

**Tres decisiones que se conservan del Python porque son la razón de que este
cliente exista, y no una llamada HTTP pelada:**

1. **Un rechazo NO es una avería, y el código HTTP no lo decide.** Un archivo
   rechazado viaja con **400 y el resultado completo dentro** (observado contra
   stage el 2026-07-26). Lo que separa un resultado de una avería es si el cuerpo
   **es** un resultado, así que se exige su FORMA: un 400 con el HTML de un proxy
   sigue siendo avería. Las dos mitades tienen prueba, y son pareja.
2. **El 401 se mira ANTES que la forma, y ese orden es la regla.** Al revés, un
   401 cuyo cuerpo se pareciera a un resultado se leería como archivo malo y
   mandaría a corregir un RIPS que está bien.
3. **Lo que no es notificación bloquea, por EXCLUSIÓN.** Si mañana aparece una
   clase nueva conviene equivocarse hacia el lado seguro, antes que darla por
   inocua y creer que un archivo pasó cuando no. Y un CUV **con** bloqueantes no
   se da por aceptado: un resultado contradictorio no entrega código.

**El `Debug` de `Credenciales` se escribe A MANO.** Derivarlo bastaría para que
un `tracing::error!("{cfg:?}")` publicara la clave del prestador —que no es
nuestra, caso 2026403888—, y eso no se deshace. Su prueba lleva positivo de
control: además de que no aparezcan la clave, el documento ni el NIT, se exige
que el `Debug` **diga algo**, porque una implementación vacía también pasaría la
mitad negativa.

**El intento de feature TLS, que se retiró, y por qué se cuenta.** La primera
versión traía `tls-local = ["reqwest/rustls-tls"]`, apagada por defecto, para
poder hablar con el contenedor real —que es HTTPS—. Medido: **`ring` pasó de 0 a
2 entradas en el `Cargo.lock`**, porque Cargo resuelve también lo opcional. El
`Cargo.toml` de `medico-ihce` avisa de esto con todas las letras («no se mete de
tapadillo»), así que la feature se quitó y el hueco queda declarado: hoy este
cliente sirve contra un mock y no contra el contenedor real, y elegir el backend
TLS de un binario clínico es decisión de cadena de suministro con la directiva 25
delante. **Sin TLS una `base_url` `https` falla al conectar**, que es el modo
ruidoso correcto y no un silencio.

**Once mutantes, y el undécimo SOBREVIVIÓ en la primera vuelta** — señalando mi
prueba, no el código. El caso del recorte multibyte usaba `"ñ".repeat(400)`, y
como «ñ» son 2 bytes, el corte en 300 cae en **frontera exacta de carácter**: el
mutante que recorta por bytes no reventaba. Con un ASCII delante
(`format!("x{}", …)`) el corte cae DENTRO de una «ñ» y el caso rojo falla por la
vía real. Es la directiva 33 en su forma más barata de equivocarse: la prueba
existía, estaba bien escrita, pasaba, y no medía nada.

**El banco de mutación lleva TRES veredictos**, no dos: muerto, sobrevivió y **no
concluyente** —cuando el mutante no compiló o cuando la prueba no llegó a correr—.
Es la lección que ya costó una vez en este repositorio: un nombre de prueba mal
escrito da `exit 0` sobre cero pruebas y pasa por verde (directiva 30). Su
positivo de control es que las once pruebas objetivo pasen ANTES de mutar nada.

### Y enchufado el mismo día: `/panel/rips` envía al MUV (2026-08-10)

El crate quedó unas horas sin llamante —la misma forma que
`PUT /api/instituciones/{id}/modulos`, que existía y ningún frontal invocaba—, y
se cerró con las tres decisiones que faltaban.

**Dónde viven las credenciales: en el ENTORNO del proceso.** No es una decisión
nueva; es la que `ihce.rs` ya había tomado dos días antes para el mismo problema,
y tener dos sitios donde buscar el mismo tipo de secreto es el mecanismo de más
que la directiva 1 manda quitar. **No van a la base**: `institucion` y
`configuracion_institucion` se editan desde una pantalla de administración, y un
secreto que un administrador puede leer de vuelta deja de ser secreto — además de
que la base viaja en cada respaldo. Hacen falta las **cinco** variables o no hay
envío, y el arranque dice en qué quedó: un servicio que calla cuando no está
configurado se ve igual que uno sano (directiva 30).

**La limitación, dicha antes de que sorprenda**: es UNA configuración por
PROCESO. Hoy alcanza —una base por instancia—, pero un despliegue con dos
prestadores que facturan necesitaría credenciales por institución. Lo que sí hace
el envío es **negarse con 409 si el NIT de las credenciales no es el de la
institución que envía**: radicarle a la EPS con el NIT del vecino no se deshace.

**El backend TLS: `rustls`, y la elección se MIDIÓ** en vez de heredarse. Los
tres candidatos contra `cargo deny`:

| | resultado |
|---|---|
| `rustls-tls` | pedía **ISC** (`ring`, `rustls-webpki`, `untrusted`) y **CDLA-Permissive-2.0** (`webpki-roots`) |
| `rustls-tls-native-roots` | pedía ISC |
| `native-tls` | pasaba la política **sin tocarla**… y no compila sin `libssl-dev` en cada máquina |

Decisión de Juan: rustls, y las dos licencias entran en `deny.toml` con su motivo
escrito —son permisivas, ISC es equivalente al BSD-2-Clause que ya estaba, y
CDLA-Permissive-2.0 cubre DATOS (el juego de CAs raíz), no código—. A favor:
Rust puro en un binario que maneja historia clínica, y el producto sigue
entregándose como **un archivo**, sin depender de la OpenSSL del cliente.
**Efecto cruzado declarado**: las features de Cargo son unión, así que
`medico-ihce` ganó TLS sin pedirlo; su comentario, que decía lo contrario, se
corrigió el mismo día en vez de quedarse mintiendo.

**Tres decisiones de producto en la pantalla:**

1. **El envío va en su PROPIO formulario**, no como un segundo botón de la
   descarga. Descargar es reversible —se borra el archivo y ya—; enviar no: un
   archivo aceptado obtiene un CUV con efectos ante la EPS y la Supersalud. La
   separación visual es parte de la salvaguarda, y el botón pide dos pulsaciones.
2. **La configuración se mira ANTES de generar nada.** Si no hay a dónde enviar,
   no tiene sentido leer un mes entero de historia clínica para tirarlo — y su
   prueba comprueba justamente que no quedó ni un eslabón.
3. **El eslabón se anota DESPUÉS del envío y solo si el archivo salió**, con
   acción propia `enviar_muv`. Decir «exportar» sería cierto a medias: el dato no
   se descargó, se entregó a MinSalud. Por eso se llama a `generar_documentos`
   —que no anota— y no a `exportar`.

**Y `Estado` ganó la configuración, que era un defecto de diseño mío**: la
primera versión leía el entorno dentro del manejador, y con eso la ruta no se
podía probar contra un validador de mentira sin tocar variables globales del
proceso de pruebas. Resuelta al arrancar: se lee una vez, el log la anuncia, y el
banco puede montar un MUV falso en un puerto efímero.

**Diez mutantes, y uno SOBREVIVIÓ señalando un hueco real del banco.** El mutante
que anota el eslabón antes de `cargar` no moría, porque la única prueba que medía
un fallo lo provocaba en el LOGIN — o sea, antes de llegar a la línea mutada. El
caso que faltaba es real: un contenedor que acepta la sesión y se cae al subir el
paquete (timeout, reinicio a media carga). Con
`si_la_carga_falla_despues_del_login_tampoco_queda_eslabon` el mutante muere. Es
la directiva 33: el caso rojo tiene que fallar **por la vía real**.

**Y la ruta FEV en salud sigue sin ofrecerse**: exige el XML de la factura
aprobado por la DIAN, que este sistema todavía no guarda.

### Los cinco hallazgos de la revisión de seguridad, cerrados (2026-08-10)

La revisión sobre `6f96941` —tres agentes de identificación, cuatro
verificadores adversariales— dejó cinco cosas. Dos eran vulnerabilidades
reportables; las otras tres no lo eran y se arreglaron igual, porque el criterio
de «vulnerabilidad» y el de «defecto» no son el mismo.

**Las tres comparten UNA causa, y por eso se arreglaron juntas: la regla vivía en
el manejador y no al lado del dato**, así que cada puerta nueva la heredaba solo
si alguien se acordaba. Las tres se cierran subiéndola, que es lo que este
repositorio ya hizo bien con `faltan_para_firmar` y `motivo_para_no_despachar`.

**1. Escalada por la matriz de capacidades** (`vista/admin.rs`, Media). El panel
validaba solo con `Ajustes::validar`, y la regla «nadie concede lo que su rol no
trae» vivía **solo** en el manejador de `/api/capacidades`. Con Angular retirado,
el panel es la única interfaz: un admin delega `gestionar_usuarios` en el
laboratorista —operación legítima que la matriz ofrece— y ese laboratorista se
concede `firmar_clinico` y **firma historia clínica**.

Lo peor no es el hueco: es que **el comentario del manejador afirmaba que
`validar` aplicaba la regla, y el aviso que la pantalla enseña al usuario se la
prometía citando el caso del laboratorista**. Un comentario que afirma una
garantía es una afirmación como otra cualquiera y se comprueba (directiva 2).
Ahora la regla es el **guardarraíl 3** de `Ajustes::validar`, la copia de la API
se borró y solo quedó allí la traducción a HTTP —que conserva su 403—.

Va al FINAL de `validar` a propósito: así los rechazos más específicos siguen
ganando, que es el orden exacto que tenía cuando la comprobación estaba fuera.
**Una prueba existente se puso roja** y ése fue el positivo de control de que la
regla muerde: `el_relevo_vale_en_la_misma_peticion` usaba un `Medico` como actor
incidental para medir el ORDEN de los cambios, y un médico ya no puede conceder.
Se le cambió el actor, no la regla. Y se escribió la consecuencia como prueba:
**`GestionarUsuarios` no se propaga** — un delegado no reparte la administración.

**2. Los maestros del panel sin puerta de ámbito** (Media). `crear_especialidad`
y `crear_entidad` descartaban el usuario (`_u`) y no llamaban a
`puede_administrar_maestros`, que las siete rutas de `/api` sí usan. Una cuenta
`admin` con `institucion_id = NULL` —que solo un superadmin fabrica— no ve una
sola fila de datos de nadie y escribía en catálogos **globales**; `entidad.codigo_eps`
viaja al RIPS de todos los prestadores. Era la **cuarta** aparición de la forma
que `rutas.rs` ya llamaba «la tercera vez». Se comparte la función, no se copia.

**3. La bitácora del panel** (13 manejadores). El verificador la **refutó como
vulnerabilidad** —cae en la exclusión de «falta de registros de auditoría», y
quien ve esas listas ya tenía derecho a verlas— y aun así se cableó: es
incumplimiento normativo y contradice la regla escrita de este repositorio («el
panel anota igual que su ruta JSON»). Ocho actos administrativos y cinco
listados, con los **mismos argumentos** que su gemelo —incluida la acción en
mayúsculas de consentimiento, porque un verbo distinto partiría en dos el rastro
del mismo hecho—.

Lo sostiene un **guardián de fuente** (`todo_post_de_administracion_deja_rastro`),
y en su primera corrida cazó tres cosas, dos mías: mi llamada multilínea en
`crear_campo` no casaba el patrón, y **los tres nombres de mi lista de
excepciones eran inventados** —`crear_personal`, `editar_personal`,
`restablecer_clave`; los reales son otros—. Ahora la lista declara las
FUNCIONES en las que se delega, que es lo comprobable (directiva 20).

**4. El plan de nutrición del paciente equivocado** (`vista/nutricion.rs`). El
`{id}` de la ruta es la VALORACIÓN, y la rama de actualizar se lo pasaba a
`actualizar_plan`, cuyo `WHERE id = ?` es la PK del **plan**. Las dos series
divergen en cuanto una valoración no tiene plan, así que editar un plan
**sobrescribía el de otra valoración —otro paciente—** con dieta, vía de
alimentación y calorías, conservando su titular y su nutricionista: el cambio
quedaba **sin autor**. Y la pantalla decía «plan guardado».

El verificador lo clasificó como **bug funcional, no vulnerabilidad** —no cruza
instituciones ni amplía lo escribible— y tiene razón; se arregló igual, porque es
una escritura silenciosa en una historia clínica. La prueba que había no lo veía:
usaba UNA valoración y UN plan, el caso degenerado en que los dos ids coinciden.
La nueva **desalinea las series a propósito** y lleva su control positivo.

**5. La historia firmada, mutable por al lado.** Confirmado técnicamente pero
**preexistente**, así que no era hallazgo del PR — y el verificador encontró algo
peor: **la API dejaba AÑADIR** a una historia firmada, que es literalmente el
ejemplo con el que el CLAUDE.md enuncia el invariante. De las seis puertas del
panel lo cumplían tres; de las seis de la API, ninguna.

La regla sube a `historia::admite_escritura` y las doce puertas pasan por ella.
`Creacion { Creada, NoExiste, Firmada }` nace al lado del `Resultado` que ya
existía, por lo mismo: `Firmada` **no es** `NoExiste`. Y el orden importa —lo
ajeno sigue dando `NoExiste` aunque esté firmado, porque saber que una historia
está firmada sería un oráculo de que existe—.

**Lo que NO se cierra, y es deliberado**: `orden::cambiar_estado`. El laboratorio
procesa sobre historias firmadas, que es lo normal; bloquearlo lo dejaría sin
poder cerrar su trabajo (directiva 21). Tiene prueba propia.

**Y la decisión que quedaba abierta se tomó el mismo día: la firma pasa a cubrir
los anexos.** `firmable` sellaba solo la fila `historia`, así que acreditaba
sesenta campos y **no decía nada de la prescripción**.

Lo que inclinó la decisión fue una medición, no un argumento: **cero historias
firmadas y cero firmas** en la base de la demostración, y ninguna instalación de
cliente —el prestador sigue en VFP—. El motivo por el que esto se había aplazado
—«invalidaría las firmas ya selladas»— era cierto en abstracto y **falso hoy**, y
la ventana se cerraba sola con la primera firma real.

Entra el resumen de `formula`, `orden` e `imagen_diagnostica` bajo la clave
`__anexos` —un nombre que ninguna columna usa, para que no pueda sustituir a un
campo en silencio—, con `ORDER BY id` y columnas fijas: firmar y verificar tienen
que producir los mismos bytes, porque una verificación inestable es peor que
ninguna. De los adjuntos entra nombre, tipo y tamaño, no los bytes: sellar el
contenido obligaría a leer y descifrar cada archivo en cada verificación, y lo
que hay que detectar es que el anexo cambió o desapareció.

**El caso rojo va por SQL directo a propósito**: por la aplicación ya no se puede
—`admite_escritura` lo cierra—, así que probarlo por ahí mediría la puerta y no
la firma. Y la pareja que hace discriminar al banco es
`lo_que_no_es_contenido_de_la_historia_no_rompe_la_firma`: sin ella, un resumen
que metiera medio esquema pasaría igual y convertiría la verificación en ruido.
Medido con su mutante —quitar los anexos de los bytes firmados—: las dos pruebas
de alteración se ponen rojas y la pareja sigue verde.

### El CUV se guarda, y con él la historia del periodo (2026-08-10)

Migración `0055`, `medico_datos::envio_muv`. El CUV se enseñaba y se perdía al
recargar; es el comprobante de que un periodo se radicó —lo que el prestador cita
al responder una glosa—, así que vivir solo en una pantalla no le servía.

**Se guarda CADA envío, no el último.** Un periodo se reenvía: se manda, el
validador señala doce campos, se corrigen, se vuelve a mandar. Guardar solo el
último borraría la historia de esas correcciones, que es justo lo que hay que
poder enseñar si alguien pregunta por qué se radicó tarde.

**Solo entra lo que SALIÓ**, la misma regla del eslabón de la Ley 1581: si el
contenedor no responde, no hay fila — una tabla que registra lo que no ocurrió
diría que se reportó un periodo que no se reportó.

**El invariante lo sostiene la BASE**: `CHECK ((aceptado = 1 AND cuv IS NOT NULL)
OR (aceptado = 0 AND cuv IS NULL))`. Un CUV en una fila rechazada sería el
comprobante de algo que no pasó, y un aceptado sin CUV no sirve de nada; las dos
formas se leerían como datos válidos. Va en el esquema y no en el manejador
porque esta tabla la escribirá algún día algo que no es el código de hoy
(directiva 6). Y `registrar` **descarta el CUV** si el envío no fue aceptado, en
vez de dejar que el `CHECK` reviente: eso sería un error de base a mitad de un
envío ya hecho, y el envío no se puede deshacer para reintentarlo.

**La puerta del consecutivo cierra SOLO sobre los aceptados, y esa elección es la
regla.** Un número que ya obtuvo un CUV no se puede reutilizar jamás: es
inequívoco, y se sabe aquí sin gastar el viaje al validador. Un número que se usó
en un envío RECHAZADO es otra cosa —el archivo no se radicó, y corregir y
reenviar con el mismo número es el camino normal—; cerrar también ese caso sería
rechazar lo inusual en vez de lo imposible (directiva 21) y obligaría a quemar un
consecutivo por cada errata.

**Guardar es *best-effort*, y la pantalla lo DICE.** El envío ya ocurrió y no se
deshace, así que un fallo al escribir no puede tragarse el CUV que el usuario
tiene delante: se enseña igual, con el aviso de que no quedó guardado y de que lo
copie. Fingir que se guardó sería la directiva 20 al revés.

**Y `Hallazgo` ganó `Serialize`, que trae una trampa nueva**: desde que los
hallazgos se guardan, esa forma tiene DOS autores —el validador y nuestra base— y
releerlos tiene que dar el mismo tipo. Un `rename` que se le olvide a `Serialize`
produce una fila que se relee con todos los campos en blanco **sin un solo
error**. Lo fija `un_hallazgo_sobrevive_a_la_ida_y_la_vuelta`, que además exige
los nombres del validador (`PathFuente`, `Clase`) y no los de Rust.

**Ocho mutantes, y uno SOBREVIVIÓ por la MISMA causa que en el cambio
anterior**: el que guarda la fila antes de `cargar` no moría, porque la prueba de
fallo que tenía a mano muere en el LOGIN y nunca llega a esa línea. Es la segunda
vez en el mismo día, así que conviene escribirlo: **cuando una función tiene dos
puntos de fallo en secuencia, un caso rojo que dispara el primero no mide nada de
lo que hay después del segundo.** Se cerró añadiendo el assert que faltaba a
`si_la_carga_falla_despues_del_login_tampoco_queda_eslabon` — la prueba ya
existía y solo miraba los eslabones.

### Las diecisiete columnas de configuración que no leía nadie (2026-08-08)

`medico_datos::administracion::CONFIGURACION_APLICADA`, la gemela de
`POLITICA_APLICADA`.

**Medido columna por columna con `grep` sobre `crates/`, no de memoria: de las
17 de `configuracion_institucion`, CERO tenían lector** fuera de esa capa y de
su propia pantalla. Diecisiete interruptores que el administrador movía y que no
cambiaban nada — la directiva 20 al revés: un sistema que *parece* funcionar. Y
con Angular retirado ya no queda ni el «lo lee el frontal para pintarlo»: el
consumidor único era esa pantalla y ya no existe.

**Dos se enchufaron en la misma pasada**, porque eran descartes puros: el binario
tenía el valor escrito a mano justo al lado de la columna que lo dice.

| columna | dónde manda ahora | qué había antes |
|---|---|---|
| `duracion_cita_default` | `agenda::crear` y `anadir_bloque`, cuando el bloque no trae la suya | `COALESCE(?, 30)` en los dos `INSERT` |
| `prefijo_factura` | `facturacion::siguiente_numero` | `format!("FAC-{anio}-")` |

**El prefijo obligó a cambiar su propio defecto, y eso es la parte interesante**
(migración `0050`). La `0048` lo creó con `DEFAULT 'FE'` mientras el código
numeraba `FAC-`; enchufarlo tal cual habría **partido la serie** de quien ya
factura —todas sus facturas dicen `FAC-2026-000NN`, el contador busca el último
con ese prefijo, y al pasar a `FE-` empezaría de nuevo en 1—. Una serie de
numeración no es estética: es lo que la resolución de la DIAN autoriza. Así que
el defecto pasa a `FAC`, que es lo ÚNICO que este producto ha escrito nunca en
una factura, y las filas que seguían en `FE` —valor que nadie pudo elegir con
efecto— se pasan con él. Quien quiera `FE` lo escribe y ahora sí numera con él.

**Las tres de la resolución de facturación se APLICARON el 2026-08-09 (#597).**
`facturacion::siguiente_numero` ya las lee: la serie arranca en `rango_desde`, un
número que superaría `rango_hasta` se **rechaza** (`Resultado::FueraDelRango`, 409)
en vez de emitir una factura inválida ante la DIAN, y un rango con tope pero sin
`resolucion_facturacion` no factura (`ResolucionIncompleta`) — un número que no se
puede citar es tan inválido como uno fuera de rango. La restricción **solo existe
si hay tope** (`rango_hasta`): una IPS que aún no cargó su resolución sigue
facturando (directiva 21). Sus pruebas: cinco en `medico-datos` (con el mutante
del `>`/`>=`) y una HTTP de 409 en `rutas.rs`. **Lo que queda pendiente**: avisar
al ~90 % del rango antes de agotarlo (necesita canal de UI y umbral decidido), y
que el formulario permita BORRAR un rango puesto —hoy el `COALESCE` de
`actualizar_configuracion` conserva el valor cuando el campo llega vacío, así que
se cambia pero no se limpia—.

**Las otras doce llevan su motivo, y el motivo distingue dos cosas que no son la
misma**:

- **falta la MÁQUINA** — `smtp_host`, `smtp_port`, `smtp_usuario`,
  `enviar_recordatorios`, `horas_anticipacion` (no hay salida de correo),
  `permitir_citas_online` (no hay portal del paciente);
- **falta una DECISIÓN** — `dias_maximos_agendamiento` (aplicarlo rechazaría un
  control a un año, que es una cita legítima: directiva 21), `horario_apertura`,
  `horario_cierre`, `dias_atencion` (cerrarían los turnos de noche y las
  urgencias), `requiere_autorizacion`, `tiempo_buffer` (sería un segundo
  mecanismo para lo que ya hace `duracion_slot_min`: directiva 1).

**La pantalla DERIVA de la tabla**, igual que `/panel/admin/seguridad`: arriba el
formulario con las CINCO que mandan (duración, prefijo y las tres de la
resolución), abajo las doce con su valor y su motivo, sin poder editarse. El día
que una se enchufe, cambiar su entrada a `None` la mueve sola al formulario — no
hay una segunda lista que actualizar. Lo vigila
`la_configuracion_ensena_lo_que_no_manda_con_su_motivo`, que EXIGE un input por
cada columna aplicada; fue ese guardián el que obligó a añadir los tres campos al
formulario cuando la tabla las declaró aplicadas.

**Y el POST escribe EXACTAMENTE las que se aplican.** Quitar los campos del
formulario y seguir aceptándolos en el manejador dejaría el aviso de la pantalla
convertido en una mentira que nadie ve: bastaría reconstruir el formulario a
mano. Lo fija `la_configuracion_solo_escribe_las_columnas_que_se_aplican`, cuya
mitad discriminante es que las columnas que NO se aplican quedan **intactas**.

`toda_columna_de_la_configuracion_dice_si_se_aplica` compara contra el TIPO
SERIALIZADO —no contra una lista a mano, que se queda atrás—, así que una columna
nueva no puede entrar sin que alguien diga si sirve para algo.

Cinco pruebas y **siete mutantes muertos**. Una prueba se retiró y hay que
decirlo: `una_hora_imposible_no_se_guarda_y_se_explica` medía la validación del
horario **por esta pantalla**, y el horario ya no entra por aquí. La regla no se
perdió —vive en `actualizar_configuracion`, con su
`una_hora_mal_escrita_se_rechaza` en `medico-datos`—; lo que se perdió es una
puerta que ya no existe.

### Elegir un paciente sin cargar el padrón (2026-08-08)

`crates/medico-api/src/vista/buscador_paciente.rs` y
`medico_datos::paciente::buscar`.

Cuatro pantallas —despacho de ambulancias, programación de quirófano y las dos
del alta de nutrición— pintaban un `<select>` con **todas** las fichas de la
institución, y `/panel/pacientes` y `/panel/historias` filtraban **en memoria**
sobre el padrón entero. La causa era una sola: `paciente::listar` no aceptaba
criterio. La de Angular tenía la otra mitad del problema y era peor: pedía las
primeras 500 y a partir de ahí un paciente **no se podía elegir y nada lo
decía**.

**Medido con 20 000 pacientes, binario de *release*:**

| | bytes | segundos |
|---|---|---|
| el padrón entero (`GET /api/pacientes`, que es lo que la pantalla embebía) | **8 357 789** | **0,279** |
| `/panel/ambulancias/despachar` ahora | **5 648** | **0,0013** |
| `/panel/nutricion` ahora | 6 729 | 0,0026 |
| el fragmento del buscador, aguja selectiva | 817 | 0,145 |
| el fragmento, aguja ancha (20 de 20 000) | 1 339 | 0,078 |

O sea **~1 500× menos peso y ~220× menos tiempo** en la pantalla que era el
problema. `/api/pacientes` se deja como estaba a propósito: es la superficie de
integración y cambiarla en silencio sería otro defecto.

**Lo que la medición destapó, y que no estaba en la ficha**: la búsqueda costaba
**~145 ms** con 20 000 fichas, y el culpable era el plegado. Medido a solas: un
`LIKE` plano son 5,6 ms; con solo cuatro `REPLACE` sube a 12,5. La cadena entera
—42 sustituciones por columna— multiplica por cincuenta y **no puede usar
índice**. Se bajó a la mitad quitando una consulta: **el `COUNT` solo se pide
cuando puede haber más**; si se pidieron veinte y vinieron menos, ésas son todas
—aritmética, no aproximación—.

**RESUELTO el 2026-08-09 (#596): el plegado se MATERIALIZA** en la columna
`paciente.plegado`, que un trigger de la migración 0051 mantiene con la misma
regla (insert y update de nombre/documento). La búsqueda compara contra la
columna con un `LIKE` plano en vez de plegar cada fila. Medido antes/después en
la MISMA máquina sobre 20 000 fichas, la misma consulta y con el mismo resultado
(`count 11111` en las dos, control de que no es rápido por hacer menos):
**120,9 ms → 3,1 ms, ~39×** (release; el número de la ficha era de otra máquina).
El invariante lo sostiene la BASE (el trigger), así que vale para el escritor
Rust, para el SQL de las pruebas y para cualquier otra escritura, sin tocar
`crear`/`actualizar`.

**Lo que la búsqueda GANA**, y no lo tenía ni el filtro en memoria ni el
`nombre1 LIKE ? OR apellido1 LIKE ?` de incapacidades y remisiones:

- **pliega caja y tildes** — «munoz» encuentra a «Muñoz», y «PÁEZ» a «Páez», que
  con `LOWER` de SQLite a secas no pasa: solo baja el ASCII;
- **cruza columnas** — «maria paez» encuentra a «María Fernanda Páez Gómez», que
  comparando trozo a trozo no sale nunca;
- **las palabras van con AND**, así que añadir una acota en vez de ampliar.

`plegar` (Rust) y el trigger de la 0051 (SQL) son **dos copias de una regla**,
que es el riesgo que trae esto: si divergen, la pantalla encuentra lo que la
consulta no y no hay error en ninguna parte. Lo fija
`la_columna_plegado_dice_lo_mismo_que_plegar`, que lee la columna que el trigger
escribe y la compara con `plegar`. **Ese banco cazó una divergencia REAL al
materializar** (2026-08-09): `plegar` usaba `to_lowercase` —que baja el Unicode
entero— mientras `LOWER` de SQLite solo baja el ASCII, así que «Ý» quedaba «ý» en
Rust y «Ý» en SQL y la aguja no casaba la columna. El banco viejo no lo veía
porque solo probaba letras del mapa de tildes. `plegar` pasó a bajar **solo
ASCII**, como el trigger; el mapa de tildes cubre los acentos españoles en las
dos cajas, así que la búsqueda no pierde nada.

**`Hallazgo` lleva `filas` y `total`, y son dos campos porque son dos cosas**
(directiva 32): lo que cabe bajo el techo y cuántos hay. Sin el segundo no se
puede escribir «coinciden 25, se enseñan 20», y eso es exactamente lo que
convierte truncar en **decirlo**.

**La ruta del fragmento es `/panel/buscar/paciente` y no `/panel/pacientes/…`**,
por dos razones que valen las dos: bajo `pacientes` chocaría con
`/panel/pacientes/{id}`, y quedaría dentro del módulo PACIENTES de la licencia —
así que un cliente que compró AMBULANCIAS y no PACIENTES se quedaría sin poder
despachar. Está en `PANEL_FUERA_DEL_LICENCIAMIENTO` con ese motivo escrito. Exige
sesión, va acotada al ámbito y **deja eslabón** como cualquier listado que
atraviesa titulares.

Once pruebas y **nueve mutantes muertos**, dos de ellos solo cazables en el
navegador: el `hx-target` que apunta a un hueco que no existe y el `hx-trigger`
que no se dispara al teclear. Los dos devuelven el MISMO HTML que la versión
buena — es la directiva 28 con instrumento.

### La bitácora alcanza los cinco listados clínicos que faltaban (2026-08-08)

Urgencias, hospitalización, incapacidades, remisiones y las citas: los cinco
atraviesan titulares y devuelven **nombre, documento y dato clínico** de cada uno
—el motivo de consulta en la cola de triage, el diagnóstico de ingreso en el
censo, la justificación de la remisión—, y ninguno llamaba a `registrar_*`.
Estaba declarado en este archivo desde que nació la bitácora, con un freno
—«el volumen de eslabones por refresco de pantalla»— que se midió al cerrar
imagenología y **no existe** (≈0,055 ms y 232 B por eslabón). O sea que lo que
quedaba era trabajo, no una decisión.

**Se cierra por las DOS puertas a la vez** —ruta JSON y pantalla del panel—, por
lo mismo que en imagenología: si una anotara y la otra no, el rastro de un mismo
hecho quedaría partido según qué frontal lo abrió.

**Dónde va cada forma**, y la regla en una frase: *se anota la lectura que
devuelve datos identificables de paciente; lo que no los devuelve, no*.

| ruta | forma |
|---|---|
| `GET /api/urgencias/cola`, `/api/hospitalizacion`, `/api/incapacidades`, `/api/remisiones`, `/api/citas` | `Coleccion`, **sin ids** |
| los mismos por `/panel/…` | idéntico, con los MISMOS argumentos |
| `GET /api/incapacidades/{historia_id}`, `/api/remisiones/{id}` | `Recurso`, con los dos ids |
| `/api/incapacidades/resumen`, `/api/remisiones/resumen` | `Coleccion` con el verbo **`resumir`** |
| `/api/hospitalizacion/camas` y `/medicos`, y las seis de `/api/agenda/*` | **no anotan** |

**Aquí NO hay caso `DeUnTitular`, y se dice en vez de fabricarlo**: ninguno de los
cinco acepta `paciente_id` como filtro —urgencias no filtra, y los demás filtran
por estado, fecha o texto—. La forma existe y se usa donde hay un titular en la
petición (`/api/documentos?paciente_id=…`, la bandeja de imagenología); inventarle
uno a estas rutas sería un campo que nadie manda.

**El RECUENTO anota, y era la tentación evidente de no hacerlo** («son tres
números»). Acepta el MISMO `buscar` que su listado, así que
`/api/incapacidades/resumen?buscar=<documento>` contesta cuántas incapacidades
tiene esa persona y cuántos días — sobre alguien nombrado en la petición. Sin
eslabón eso se puede preguntar tantas veces como se quiera sin dejar rastro. Y no
duplica nada, porque la unidad es **un eslabón por PETICIÓN, no por consulta a la
base**: la pantalla pide listado y recuento dentro de una sola petición y deja
UNO; quien llame a las dos rutas JSON hizo dos accesos y deja dos.

**El `filtro` guarda el texto de la búsqueda a propósito.** Suele ser el nombre o
el documento de un paciente, y esconderlo dejaría el eslabón en «alguien listó
incapacidades», que no sirve para auditar: lo que hace falta saber es **a quién
fue a buscar**. Una colección no lleva ids, así que no entra en el rastro de
ningún titular; ese texto solo se lee desde la auditoría de la propia institución.
Y sin filtros el campo va **nulo**, no en blanco: «se miró todo» es un hecho
distinto de «se miró con criterios vacíos» (directiva 20).

**Dos structs ganaron `paciente_id`**: `incapacidad::Incapacidad` y
`referencia::Remision`. No lo pedía ninguna pantalla —enseñan el nombre y el
documento—, lo pide la bitácora, que ata el eslabón al titular por su **id**.
`vista/incapacidades.rs` llevaba escrita la nota de que por eso no anotaba; ahora
anota y la nota se fue.

**Los nombres de recurso son CONSTANTES compartidas** (`RECURSO_*` en `rutas.rs`),
y con ellas se reescribieron también las tres de imagenología. Se tecleaban a mano
en los dos llamantes, y eso deja la promesa «el panel anota igual que su ruta»
sostenida por que nadie se equivoque al escribir: una errata en uno de los dos
parte el rastro en dos recursos distintos y **no da error en ninguna parte**. Las
pruebas siguen comparando contra la cadena literal —el artefacto—, así que
cambiarle el valor a la constante se pone rojo.

**Diez pruebas y diez mutantes muertos.** Uno **sobrevivió** en la primera vuelta
y valió por todos: cambiar en la pantalla la descripción del filtro por una propia
no ponía roja ninguna prueba, y el caso no es teórico —los dos frontales llaman al
mismo criterio por nombres distintos, `?q=` en la pantalla y `?buscar=` en la
API—. De ahí salió `las_dos_puertas_describen_el_mismo_filtro_igual`.

**La pareja que hace que el banco discrimine**: `lo_que_no_lleva_dato_de_paciente_no_deja_eslabon`.
Sin ella, «anota» y «anota todo, siempre» se ven igual — y anotar el mapa de camas
o la lista de profesionales metería un eslabón por pintado que no dice nada de
ningún titular. Las camas y los médicos son del prestador; **la agenda es de quien
ATIENDE, no de quien es atendido**, y por eso las seis rutas de `/api/agenda/*` se
comprobaron una por una antes de dejarlas fuera: devuelven profesionales, franjas
y recuentos, ni un dato de paciente. Lo que en la nota vieja se llamaba «agenda»
era, medido, `/api/citas`.

**Cruzada la frontera** (directiva 28), sobre el binario de *release* con la
demostración sembrada y entrando por el formulario: las cinco pantallas dejaron
**exactamente 5** eslabones y las cuatro rutas sin dato de paciente **0**. Con dos
citas reales sembradas, las dos puertas escribieron el eslabón **idéntico** —
`n: 2`, `filtro: fecha=2026-08-10`, `pacientes: []`, sin titular—. Tamaño medio
medido ahí: **234 bytes**.

`/panel/remisiones/{id}` **no existe** todavía, y la prueba lo afirma con un 404
en vez de dejarlo en un `if` que pasaría existiera o no: el día que se añada la
ficha, se pone roja y obliga a cablear su eslabón.

### La obligación del informe radiológico está en la FIRMA (2026-08-08)

`medico-datos::diagnostico`. Los tres campos del informe —técnica, hallazgos e
impresión diagnóstica— son obligatorios según el producto, y esa obligación vivía
**solo en el navegador**: Angular deshabilitaba el botón y el panel en `maud`
pone `required`, que son la misma barrera de pantalla. Por
`POST /api/imagenes/ordenes/{id}/informe` con los tres en blanco se guardaba un
informe que no dice nada, y quedaba en la historia clínica **como un informe
redactado**. Es la misma familia que el mínimo de contraseña de #49: el
mecanismo existía en la pantalla y no en el servidor.

**Lo que había que decidir primero, y el código lo dejaba abierto.**
`actualizar_informe` no usa `COALESCE` **a propósito** —su doc-comment lo dice:
«un borrado deliberado ("esto no era un hallazgo") tiene que poder guardarse
vacío»—. Escribir la regla en el guardado habría roto ese caso de uso real
(directiva 21). Y hay una razón de más, que es la de la directiva 84: un borrador
es trabajo a medias —el radiólogo escribe la técnica, vuelve a mirar las placas y
luego redacta los hallazgos—, así que bloquear el guardado **enseña a teclear un
punto para poder salvar**. Lo lento se rodea, y lo que se cuela es un dato falso.

**La puerta va donde el acto es irreversible: la FIRMA.** Un informe firmado
acredita autoría (Ley 527/1999) y es lo que lo convierte en documento; en blanco
sería un informe que no dice nada, firmado. Se para ahí y **se dice QUÉ falta**,
por su nombre — «no se pudo firmar» manda a ir campo por campo adivinando.

- La regla es `InformeImagen::faltan_para_firmar`, al lado del dato que juzga.
- La usan `firmar_informe` **y la pantalla**, que por eso no ofrece el botón
  cuando falta algo y dice cuál: con dos copias de la lista, una se queda atrás
  y el botón lleva a un rechazo.
- Por `/api` es **422 y no 409**: lo que falla es el contenido que se mandó, no
  un conflicto de estado.
- Los espacios no cuentan como contenido (`trim`), y tiene su prueba.

Tres pruebas y **cinco mutantes muertos**. Uno sobrevivió en la primera pasada y
señalaba un hueco REAL de la prueba, no del código: mi caso solo cubría el ALTA
del borrador vacío, y **el vaciado deliberado —que es el caso por el que la
puerta va en la firma— no estaba medido**. Añadido: se escribe el informe
completo, se vacía un campo y se comprueba que queda vacío. El mutante que pone
la obligación en el guardado muere ahí.

**Y quedaba un hermano**: `historia::firmar` tampoco miraba si la historia dice
algo. **CERRADO el 2026-08-08**, ver
[La historia clínica tampoco se firmaba en blanco](#la-historia-clínica-tampoco-se-firmaba-en-blanco-2026-08-08).

### La historia clínica tampoco se firmaba en blanco (2026-08-08)

`historia::faltan_para_firmar`, hermana de la del informe radiológico. `firmar`
no miraba el contenido: marcaba `firmada = 1`, calculaba la firma criptográfica y
cerraba. Una historia con el motivo, el diagnóstico y el plan en blanco se
firmaba sin queja y quedaba en el expediente **como un documento firmado que no
dice nada**.

No era hipotético: `POST /panel/historias/paciente/{id}` abre la historia con
`diagnostico_principal` y `plan_tratamiento` VACÍOS a propósito —abrirla con un
diagnóstico inventado sería el `890201` otra vez—, así que el camino normal del
panel produce exactamente ese borrador.

**Lo que había que decidir primero, y por eso no se cerró con su hermano.** En el
informe los tres campos los declara el producto y no hay duda; aquí **qué es
indispensable para poder firmar es un juicio clínico y legal**. Medido antes de
elegir, y el resultado es que **no había prior art que copiar**:

| fuente | qué exige hoy para cerrar una historia |
|---|---|
| **VFP vivo** (`Fuentes/forms/historia.SCT`) | `campos_obligatorios = txtcodigo, txtcedula` — **nada clínico**. Y su bucle recorre las comas pero siempre lee `GETWORDNUM(…, 1, ",")`, así que en la práctica solo comprueba el código |
| **Python vivo** (`backend/routers/historias.py:187`) | nada: hashea `{id, paciente_id, medico_id}` y firma |
| **Resolución 948**, archivo AC | `codDiagnosticoPrincipal` es obligatorio: sin él la atención no se reporta ni se cobra |

**La lista la fijó Juan el 2026-08-08: motivo de consulta, diagnóstico principal
y plan de tratamiento.** No hay consulta sin motivo, ni siquiera un control; el
diagnóstico lo exige la norma para reportar; y la conducta es lo que el paciente
se lleva.

**`enfermedad_actual` queda FUERA a propósito, y ésa es la parte fina.**
`HistoriaNueva` la declara entre los clínicamente obligatorios —es uno de sus
cuatro `String` sin `Option`—, así que lo obvio era exigir los cuatro. Pero un
control de crecimiento y desarrollo o un control prenatal **no tienen
enfermedad**, y exigirla enseña a escribir «ninguna»: el dato falso que este
repositorio persigue, cambiado por fricción (directiva 21). Lo fija
`el_control_del_sano_se_firma_sin_enfermedad_actual`, y su mutante —añadirla a la
lista— muere ahí y solo ahí.

Lo demás es la forma que ya funciona, copiada entera:

- la regla vive al lado del dato, en `medico-datos`, y la usan `firmar` **y la
  pantalla** —que por eso deja de ofrecer el botón y dice qué falta, por su
  nombre: aquí las secciones son ocho y «no se pudo firmar» manda a recorrerlas—;
- por `/api` es **422 y no 409**: falla el contenido, no un conflicto de estado;
- el borrador **se sigue guardando**, incluido el vaciado deliberado, que es el
  caso por el que la puerta va en la firma;
- los espacios no cuentan como contenido, y su caso rojo entra por `/api`, que es
  el único camino que escribe la columna sin recortar —el panel usa `texto()`,
  que hace `trim`—.

Y `ResultadoFirma` deja de ser `Copy` porque `Incompleto` lleva QUÉ falta. El arm
del panel va **explícito y no bajo el comodín**: con `_ => averia()`, «le falta el
diagnóstico» se contestaba con la avería del despliegue, que es el mismo error
que este arreglo viene a cerrar.

Siete pruebas —tres en `medico-datos`, cuatro entre las dos puertas— y **ocho
mutantes muertos**. Cuatro «sobrevivieron» en la primera vuelta y era el arnés:
el banco corría `cargo test … -- --exact` con un nombre de módulo parcial, así que
**no corría ninguna prueba** y el `exit 0` pasaba por verde (directiva 30). El
banco lleva ahora su positivo de control —la línea `running N tests` con N ≥ 1— y
un tercer veredicto, NO CONCLUYENTE, distinto de «sobrevivió».

### El banco de navegador habla CDP y entra por el formulario (2026-08-08)

`crates/medico-api/tests/comun/mod.rs`. Cierra la tarea #2, que con Angular
retirado había pasado de laguna cómoda a **la** red que faltaba: de las 29
pantallas, 28 exigen sesión y **ninguna se había dibujado nunca** en un navegador
—`chromium --dump-dom` no admite mandar una cookie, no existe `--header`—.

**Se habla CDP por TUBERÍA, no por WebSocket.** Chromium ofrece los dos caminos;
el WebSocket habría obligado a escribir un cliente RFC 6455 —apretón de manos,
enmascarado, tramas— dentro de un banco de pruebas: cien líneas de protocolo que
no son el producto. Con `--remote-debugging-pipe` el mensaje es
`serde_json::to_vec` más un byte cero. El precio es **`libc` como dependencia de
desarrollo**, y solo por dos llamadas: Chromium exige la tubería en los
descriptores **3 y 4 exactos**, y colocarlos ahí en el hijo necesita `dup2` en el
`pre_exec`. Sin eso arranca y dice *«Remote debugging pipe file descriptors are
not open»* — medido, no supuesto.

**La sesión NO se inyecta: se ENTRA por el formulario**, como entra una persona.
Poner la cookie a mano habría sido más corto y habría medido menos: una cookie
mal marcada —`Secure` sobre http, un `SameSite` equivocado, una ruta que no
cubre `/panel`— la acepta igual un `curl` y la rechaza el navegador, **en
silencio**. Entrando de verdad, el acceso queda medido de paso. Y el usuario lo
siembra `medico-api sembrar-demo`, que es el camino del propio producto:
sembrarlo a mano con `sqlx` habría fabricado un estado que el binario no produce
nunca, y entonces la prueba mediría el arnés.

**Una trampa de descriptores que costó una vuelta**, y va escrita porque no se
ve: `pipe()` devuelve los números libres MÁS BAJOS, así que los dos extremos
nuestros caen a menudo justo en el 3 y el 4. Duplicar ahí los del navegador sin
apartar antes los propios los machaca, y el arnés acaba **leyendo lo que él mismo
escribió** — un eco que se lee como una respuesta. Se apartan con `dup` antes de
tocar nada.

**Lo que cierra, y es el motivo de todo esto**: el `hx-get` de `/panel/rips` —el
único intercambio del panel— **ocurre de verdad**. Era el trozo del producto que
ningún banco tocaba: con `hx-target="#resultados"`, una letra de más, el servidor
responde el mismo 200 con el mismo fragmento, htmx no encuentra dónde meterlo y
no pasa nada. Ni error, ni pantalla en blanco: el botón no hace nada. Ese mutante
—la letra de más— es el primero del banco y muere.

**Y se comprueba que la hoja se APLICA, no solo que se enlaza.** Importa desde
que la CSP cerró `style-src 'unsafe-inline'` el mismo día: si algo del panel
dependiera de un estilo en línea ya no se vería «raro», **no se aplicaría**, y el
HTML sería exactamente el mismo. Se mira el `getComputedStyle` de `.tarjeta`.

Cinco pruebas nuevas —entrar y dibujar, el intercambio, el control de sesión, la
clave equivocada— y **cinco mutantes muertos**: el destino con la errata, el
fragmento que devuelve una página entera, la hoja sin la regla, `boi` sin su
marca y el CSS servido vacío.

**Lo que sigue sin cubrir, y no se disimula**: esto dibuja y ejecuta, **no mira
píxeles**. Una tabla descuadrada, un color sin contraste o un formulario que se
sale en un móvil no lo ve nadie todavía. Y no recorre las 29 pantallas: recorre
una muestra, y vale para las demás porque **todas salen de la MISMA
`vista::pagina`**, con los mismos `<script>` y la misma CSP.

**Falta extraerlo** (directiva 9): es el tercer proyecto con banco de navegador
—nexus, informes, medico— y esta es la primera versión que habla CDP, así que lo
que hay que compartir ya no es «arrancar y volcar» sino el cliente entero.
informes lleva la misma nota desde el 2026-08-06.

### Angular fuera: el panel es el producto (2026-08-08)

Retirado el SPA entero el mismo día en que se migró la pantalla 29 de 29. Decisión
de Juan: *«nada absolutamente nada en Angular»*.

| se fue | qué era |
|---|---|
| `frontend/` | 482 MB, 98 archivos TypeScript |
| `crates/medico-api/src/estaticos.rs` | servía el SPA por el `fallback` |
| `crates/medico-api/build.rs` | rompía el *release* si faltaba `index.html` |
| `rust-embed` | dependencia |
| `MEDICO_SIN_INTERFAZ` | la bandera de escape de lo anterior |
| `crates/medico-api/estaticos/` y su regla del `.gitignore` | la carpeta donde caía `ng build` |
| el trabajo `frontend` del CI | `npm ci` + `ng build` + vitest |
| dos de los tres pasos de `construir.sh` | ahora es un `cargo build --release` |
| `AngularSPA` en `backend/main.py` | el montaje del Python, y su prueba |
| `TODAVIA_EN_ANGULAR` y su guardián | ya estaba vacía |
| `antecedentes_paridad_con_angular.rs` | su razón era la convivencia |

**LA RAÍZ.** `/` la servía el comodín del SPA; ahora **redirige a `/panel`**, y
basta una regla para los dos casos: quien no tenga sesión rebota de ahí a
`/panel/entrada`, porque de eso ya se encarga `EnSesion`. Y al irse el comodín,
**lo que no case es un 404 de verdad** — antes una errata de URL devolvía la
aplicación entera con 200, así que un `<script src>` mal escrito recibía HTML,
el navegador lo descartaba por el MIME y la pantalla se veía bien sin que nada
fallara. Lo sujeta `la_raiz_lleva_al_panel_y_lo_que_no_existe_es_404`, con las
dos mitades.

**El `.fallback(` se conserva como MARCA, y esto es una trampa que había que
ver.** `modulos::sirve` y el guardián de la licencia leen el router «desde
`Router::new()` hasta `.fallback(`» y ese `.expect()` **entra en pánico al
arrancar** si el marcador desaparece. O sea que quitar el comodín a secas habría
tumbado el binario en el sitio menos evidente. Hoy el fallback es un 404 honesto
en texto plano, sin decir qué prefijos existen — un 404 que enumera es un
inventario del producto para cualquiera desde fuera.

**El `'unsafe-inline'` de `style-src` SE CERRÓ**, que es la ganancia de seguridad
del cambio y estaba anticipada: llevaba ahí por Angular —escribía estilos en
tiempo de ejecución y usaba atributos `style="…"`, y un nonce no lo arregla
porque vale para elementos `<style>` y no para atributos—. `piezas::barras` se
dibujó en SVG en vez de con `style="width:X%"` justo para no dar una razón nueva
de dejarlo abierto, y `ningun_panel_escribe_estilos_en_linea` lo vigilaba
esperando este día. Hoy la CSP es `script-src 'self'; style-src 'self'`.
**Y cambia lo que significa un estilo en línea**: antes se veía raro, ahora **no
se aplica**, así que esa prueba dejó de ser preventiva.

**El build dejó de necesitar Node**, que es la razón de fondo del cambio de stack
(directiva 26): `rustc` compila las plantillas de `maud` con los tipos del
dominio dentro, y htmx, `boi.js` y `panel.css` van con `include_str!` desde
`crates/medico-api/panel/`, **que sí está versionado**. El recuento de
dependencias de frontal del producto entero es **UNA**.

**Se midió lo que informes enseñó a medir**: el árbol de git extraído a otro
directorio (`git archive` + `tar -x`) **compila**. Hace falta porque en local
todo sale verde aunque un archivo no esté en git — a informes le pasó, con el
`.gitignore` tragándose dos activos del panel, y el CI no lo vio porque solo
corre en PR. Aquí no se repitió, y no por suerte: lo vendorizado nunca vivió en
la carpeta ignorada.

**El backend Python queda como API.** Servía el mismo `dist/` en `/` (decisión
#73), y sin `frontend/` ese montaje apuntaba a un directorio que no existe. Lo
que ese proceso es hoy: **quien genera el FEV-RIPS**, que es lo único sin portar
y por lo que le pagan al prestador. Su comodín se fue también, así que
`GET /api/pacintes` con la errata ya no puede devolver HTML con 200.

**Lo que se PIERDE, y no se disimula**: el buscador incremental de medicamentos y
de CUPS —en el panel los dos códigos se teclean—, y los gráficos de anillo y
tarta de estadísticas, que hoy son barras y columnas en SVG con el número al
lado. Un anillo de cinco porciones se lee peor que cinco barras con su
porcentaje, pero es un cambio de aspecto que quien usaba la pantalla va a notar.

**Y lo que sigue SIN medirse, que es lo que hay que decir en voz alta**: el
banco de navegador (`tests/pantalla.rs`) no alcanza las pantallas con sesión
—`chromium --dump-dom` no manda cookies—, así que de las 29 solo la entrada se
ha dibujado en un navegador de verdad. Con Angular retirado eso deja de ser una
tarea pendiente cómoda y pasa a ser **la** red que falta (tarea #2): ya no hay
frontal anterior al que volver.

### La licencia por módulo se aplica EN EL SERVIDOR (2026-08-07, #534)

`crates/medico-api/src/licencia.rs`. Hasta ese día `modulos_habilitados` solo
alimentaba `GET /api/modulos`, o sea el menú: las rutas de los 23 módulos
construidos respondían a **cualquier usuario autenticado de la institución
aunque no hubiera licenciado el módulo**, y bastaba escribir la URL. El frontal
afirmaba lo contrario —el docstring de `moduloGuard` decía «el backend ya
responde 403»—, así que la única barrera contra `/api/estadisticas/financiero`
vivía en el navegador. Lo señalaron por separado las dos revisiones de la rama de
estadísticas.

**Es una CAPA (`route_layer`), no un extractor por manejador**, y esa es la
decisión que más pesa. La ficha proponía `ModuloHabilitado<X>` al lado de
`Autorizado<X>`; son 193 endpoints, y el control que se escribe a mano es el que
se olvida en la ruta siguiente — que es literalmente el argumento con el que
nació `Autorizado`. El backend Python ya lo había resuelto igual
(`requiere_modulo` a nivel de router). Con la capa, una ruta nueva bajo
`/api/nutricion/…` queda cubierta al registrarla.

**Va con `route_layer` y no con `layer`**: `route_layer` no envuelve el
`fallback`, así que la interfaz embebida se sirve igual. Con `layer` habría que
licenciar el `index.html` y a un cliente al que le falte un módulo no le
arrancaría la aplicación.

**De dónde sale «qué módulo es esta ruta»: del `prefijo` que el catálogo YA
declara**, no de una segunda lista. Ese dato existía y ya estaba vigilado por
`el_catalogo_no_miente_sobre_lo_que_el_binario_sirve` (directiva 1).
`modulo::modulos_de_ruta` devuelve **varios** —`/api/documentos` la sirven cuatro
módulos y basta tener uno— y gana el prefijo **más largo**, con frontera de
segmento: `starts_with` a secas habría dejado que la licencia de `CITAS` abriera
`/api/citaseleccion`.

Los tres estados se siguen distinguiendo, y el 404 sale gratis:

| estado | respuesta | quién la produce |
|---|---|---|
| `Habilitado` | la del manejador | la ruta existe y la licencia la cubre |
| `Pendiente` | **404** | licenciado y sin construir: no hay ruta registrada |
| `NoIncluido` | **403** | la capa, y **dice qué módulo falta** |

**Dos trampas ya pagadas, y las dos las cazó el banco, no la lectura:**

- **El prefijo que MIENTE.** `/api/imagenes/{id}` y `/api/imagenes/{id}/archivo`
  NO son del módulo de imagenología: son los **adjuntos de la historia clínica**.
  Bajo `IMAGENES`, un consultorio que no compró imagenología habría dejado de
  poder abrir los adjuntos de sus propias historias — la licencia rompiendo la
  historia clínica. Van en `LA_RUTA_MIENTE_SOBRE_SU_MODULO`, y **el guardián de
  fuente encontró tres más en su primera corrida**: `/api/formulas/{id}`,
  `/api/ordenes/{id}` y `/api/ordenes/{id}/estado`, que se crean bajo
  `/api/historias/{id}/…` y se editan por una ruta hermana plana. Estaban a la
  vista en el router y no las vio nadie.
- **El sesgo a cerrar (directiva 21).** La primera versión devolvía 403 al
  usuario **sin institución**, razonando que «no se puede saber» se niega. Lo
  puso en rojo `un_paciente_no_ve_la_clinica`, y la prueba tenía razón: un
  paciente recibe **404 y nunca 403**, que es una decisión de seguridad tomada
  hace tiempo. La capa se colaba delante de una respuesta más específica y ya
  correcta. Hoy pasa: quien no tiene institución no ve una fila de nadie porque
  el ámbito ya lo niega, y aquí no hay licencia que consultar.

**Sin caché a propósito.** Se lee la licencia en cada petición —una consulta
indexada sobre una tabla pequeña, y `usuario_de` ya hace una lectura por
petición—. Una caché habría que invalidarla al licenciar, y ese olvido es un
cliente que pagó y no lo ve.

**Sin sesión se deja pasar**, para que conteste el 401 del manejador: un 403 aquí
convertiría la capa en un inventario de lo contratado para cualquiera desde
fuera.

**Y el guardián tenía un punto ciego, que encontró la revisión de seguridad de
este mismo cambio.** `route_layer` envuelve **solo los endpoints que ya existían
cuando se aplica**: una `.route(` escrita debajo se sirve sin licencia. Medido
sobre el binario —ruta de NUTRICION bajo la capa → **200** contra una institución
que no lo compró, con la de al lado en 403— y **las 17 pruebas del guardián en
verde**, porque preguntaba si la ruta estaba DECIDIDA, no si la capa llegaba a
ella. Son dos preguntas distintas y hacen falta las dos. Peor: el comentario del
router llegó a INVITAR a escribir ahí. Hoy lo impide
`la_capa_va_despues_de_la_ultima_ruta`, que compara posiciones en el fuente sobre
la función ENTERA —también lo que vaya tras el `.fallback(`— y **con los
comentarios despojados**, porque se puso roja en su primera corrida contra la
prosa que este cambio acababa de escribir.

**El guardián principal** es `ninguna_ruta_de_modulo_se_queda_sin_licencia`: toda ruta
registrada cae en uno de tres sitios —la reclama un módulo, su prefijo miente y
se dice cuál es el suyo, o está en `FUERA_DEL_LICENCIAMIENTO` con el motivo
escrito—. Lo que no puede es no estar en ninguno, porque entonces sirve sin
puerta y nadie se entera, que es como #534 llegó hasta aquí con 760 pruebas en
verde. Sus parejas: que ninguna exención sobre y que las rutas que mienten sigan
existiendo.

**Y el fixture del banco cambió, no la puerta.** `estado_de_prueba` le licencia
el catálogo ENTERO a la institución de prueba: al cablear la capa se pusieron
rojas **54 pruebas de golpe** —miden aislamiento y capacidades, y su institución
arrancaba con el defecto de la `0032`, tres módulos—. Ése fue el positivo de
control de que la puerta muerde. La licencia se prueba aparte, en
`mod licencia_por_modulo`, con instituciones a las que les falta algo a
propósito. Medido con mutantes: retirando la capa mueren 6 de 8; retirando la
excepción del adjunto muere justo la suya y el guardián de fuente.

**Tres decisiones COMERCIALES que este cambio deja tomadas, y hay que mirarlas de
frente** (las midió la revisión; ninguna es fuga entre instituciones):

1. **`AUDITORIA` pasa a ser de pago.** Una institución con el defecto de la
   `0032` recibe ahora **403** en `GET /api/admin/auditoria`, que antes
   respondía. El derecho del PACIENTE no queda detrás de la caja —su rastro sigue
   bajo `PACIENTES`, y `/api/acceso/verificar` sigue libre—, pero la pantalla con
   la que el prestador responde a una inspección de la Ley 1581, sí.
2. **`/api/documentos` la abren cualquiera de sus cuatro módulos**, así que quien
   compre solo `EVOLUCIONES` puede leer, crear y **firmar** epicrisis y notas
   operatorias. Está documentado en `modulo.rs` y es la regla correcta para una
   espina genérica; si los cuatro se venden por separado, el filtro tendría que
   mirar el campo `tipo` y no la ruta.
3. **La licencia de `IMAGENES` protege el texto del informe, no las placas**, y
   `/api/ordenes/{id}/estado` deja cerrar una orden de laboratorio sin tener
   `LABORATORIO`. Escrito en `LA_RUTA_MIENTE_SOBRE_SU_MODULO` con su motivo.

Y una pregunta abierta para Juan: **`TURNOS` está en el catálogo como pendiente
mientras `/api/agenda/turnos` responde gratis**. Si son la misma cosa, hay un
módulo vendible regalado y un cliente leyendo «aún no está» sobre algo que ya
usa; si `TURNOS` es el turnero de sala y no la agenda médica, no hay nada que
hacer.

### El CUPS de la consulta se CAPTURA (2026-08-07, prerrequisito de #331)

Al ir a hacer el punto 4 de #331 —que la pantalla repida las finalidades al
cambiar el CUPS— salió que **no se podía**: la pantalla nunca capturó el CUPS. En
todo el frontal `cod_consulta_cups` aparecía **una sola vez, dentro de un
comentario**.

Es la misma forma que P11/P12 y que los tres campos del 2026-08-02: la columna
existe (`0005_historia.sql:68`), la API Rust la acepta, el AC la exige, y ninguna
pantalla la escribe. El Python vivo la rellena con `"890201"` por defecto — el
caso canónico del «dato con la forma correcta que nadie afirmó», que este
repositorio ya cita como ejemplo en `zona.rs` y en `estadisticas.rs`.

**Lo que se construyó**, y el lado servidor es permanente (no se tira al migrar):

| pieza | dónde |
|---|---|
| `catalogo::buscar_cups(pool, q, ambito)` | acotado por `ambito_rips`, máx. 50, comodines escapados |
| `catalogo::cups_es_de_ambito` | la puerta de la ESCRITURA |
| `GET /api/cups/buscar?q=…&ambito=AC` | el buscador; **sin `ambito` es 400** |
| buscador + reencadenado | `historias.component.ts` (Angular, ver abajo) |

**El ámbito es obligatorio y un valor raro es 400, no una lista vacía.** Vacío en
un buscador se lee como «ese procedimiento no existe», y quien lo lea así se
inventará el código o dejará el defecto — que es exactamente de donde venimos. Es
un enum y no una cadena: `"ac"` en minúscula habría devuelto vacío sin un error.

**Y la escritura tiene su propia puerta.** El buscador solo ofrece los del
archivo de consultas, pero eso es cortesía de pantalla: sin comprobarlo en
`crear_historia` **y** en `actualizar_historia`, el cuerpo mete cualquier cadena.
Un CUPS de procedimientos responde **422 con el motivo escrito**, para poder
distinguir «no existe» de «existe pero es de otro archivo». Sin CUPS sigue
entrando: el campo es opcional, y que falte es una falta que el cierre anota —
mejor que un `890201` que nadie eligió.

**En Angular y no en `maud`, a sabiendas.** La historia clínica son 2 083 líneas
y es la pantalla más grande de las 27 que faltan por migrar; hacerlo aquí son
~120 líneas que se tiran ese día, y esperar dejaba el defecto vivo sin fecha. El
servidor —que es el 70 % del trabajo— no se tira.

> **Cumplido el 2026-08-08**: la historia ya está en `maud` y el CUPS se captura
> en `/panel/historias/{id}/diagnostico`. Lo que se tiró fueron las ~120 líneas
> del buscador incremental, como estaba previsto; el servidor se reusó entero.
> **Y el punto 4 de #331 sigue abierto**: en el panel el CUPS es un campo de
> texto, así que no hay «volver a pedir las finalidades al cambiarlo» — la lista
> se acota al recargar la sección, que es cuando el servidor ya sabe el código.

**Y salió un defecto en el ARNÉS, no en el código** (directiva 33). Mi prueba
nueva usó el mismo cuerpo JSON que las de capacidades y dio **422** donde
esperaba 201: a ese cuerpo le falta `medico_id`, que es obligatorio, así que
axum ni deserializa. Eso significaba que
`assert_ne!(POST /api/historias, FORBIDDEN, "la médica no pudo crear una
historia")` —el control POSITIVO de ese par— **se cumplía con un error de
deserialización**, y habría seguido verde con `crear_historia` roto del todo.
Cambiado a `assert_eq!(CREATED)` con el cuerpo completo.

### La finalidad se valida contra el CUPS del servicio (2026-08-07, #331)

`medico-dominio::muv::finalidades_de_cups` y `finalidad_admitida_para`, con
`REFs_FinalidadCups` empotrado (`datos/muv_finalidad_cups.json`).

Hasta ahora la validación llegaba a «el código existe en la tabla». El MUV
comprueba algo más: que la finalidad valga **para ese procedimiento**. O sea que
se podía elegir una finalidad válida y un CUPS válido cuya combinación el
validador rechaza, y no enterarse hasta el rechazo — semanas después, sobre el
archivo del mes entero.

**LA MEDICIÓN MANDÓ EL DISEÑO, y la ficha ordenaba hacerla primero**
(directiva 18):

| | |
|---|---|
| filas del maestro | 990 — pero **890 parejas distintas**: 91 repetidas, con `Id` propio cada una |
| CUPS distintos que cubre | **308** |
| CUPS del catálogo del MUV | **13 640** |
| **cobertura** | **2,26 %** |
| CUPS de la tabla ausentes del catálogo | 0 |

**La tabla es PARCIAL, y por mucho.** Tratar «no está» como «rechazado»
bloquearía el 97,74 % de los procedimientos legítimos: el sesgo a cerrar de la
directiva 21 en su forma más cara. Por eso `finalidad_admitida_para` devuelve
**tres estados** y `None` no es `Some(false)` — es el mismo patrón que
`SIN_DUENIO` en el aislamiento, y aquí el tercer caso es el MAYORITARIO.

**Donde la tabla habla, aprieta**, que es lo que hace que valga la pena: de los
308 CUPS cubiertos, **250 admiten solo dos finalidades**, y `890201` —el que este
producto usa en la consulta— pasa de 34 opciones a **9**. Su vecino `890202`
**no** está cubierto, lo que descarta el atajo de «las consultas siempre están».

**`GET /api/finalidades?cups=…` ANOTA, no filtra, y eso se aparta de la ficha.**
Filtrar destruye la distinción que la medición hace obligatoria: para el 97,74 %
la respuesta filtrada serían las 34, indistinguible de «filtré y valen todas» —
la pantalla no sabría si la lista es una respuesta o una rendición. Cada finalidad
sale con `admitida_para_cups: true | false | null`. De paso el contrato no se
rompe: sigue siendo el array de siempre con un campo más, así que el cliente
Angular actual funciona igual.

**El AC valida la pareja**, y solo cuando la tabla cubre ese CUPS. El mensaje
dice **qué finalidades admite** ese procedimiento: sin la lista, quien lo lee
tiene que ir a buscarla a un maestro de MinSalud, y en la práctica no lo hace.

**Y el orden de las dos comprobaciones es la regla, no un detalle.** La pareja se
juzga solo si la finalidad EXISTE. Sin eso, un `10` —el defecto que nadie
eligió— producía DOS faltas sobre el mismo campo, y la segunda mandaba a cambiar
el CUPS cuando lo que falta es que alguien elija la finalidad. Lo cazó
`el_defecto_diez_no_produce_ademas_la_queja_de_la_pareja`, escrita antes que la
condición y roja a la primera.

Mutantes: la regla devolviendo siempre `None` mata 2 pruebas; **colapsar el
tercer estado a `false`** —el error peligroso— mata
`un_cups_que_la_tabla_no_cubre_no_se_juzga` mostrando el desastre literal en el
mensaje, *«Para ese procedimiento admite: .»*, con la lista vacía.

**Lo que NO se hizo, y queda en la ficha**: el punto 4 —que la pantalla vuelva a
pedir la lista al cambiar el CUPS— sigue sin hacerse, y desde el 2026-08-08 por
otra razón: la historia ya está en `maud`, y ahí el CUPS es un campo de texto que
acota la lista **al recargar la sección**, no al teclear. Hacerlo en vivo es un
`hx-get`, y esta pantalla no tiene ninguno todavía. Y lo que #331
apunta al final sobre `causa_motivo` (filtrar por `AplicaAU`/`AplicaAH`) sigue
esperando a que urgencias y hospitalización se porten.

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
- **Y desde la migración 0075, el `tipo_diagnostico` de ingreso y de egreso**
  (`RIPSTipoDiagnosticoPrincipalVersion2`), que el perfil RDA exige
  (`ExtensionDiagnosisType`, min=1) aunque la 948 no lo pida: se captura en
  `ingresar` (ingreso) y `egreso` (egreso) de hospitalización, y en el `egreso`
  de urgencias (solo ingreso), NULLABLE sin default y vacío→NULL (D20).

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
generación del RIPS sea byte a byte idéntica. En Rust están el recolector de
datos faltantes y los CINCO documentos, y **desde el 2026-08-08 también el
FEV-RIPS** —el documento ÚNICO que funde los cinco—, probado byte a byte contra
el `fev_rips.py` del backend: ver
[FEV-RIPS en Rust](#fev-rips-en-rust-lo-último-que-quedaba-fuera-del-estándar-2026-08-08).

**Lo que eso cumple y lo que no**: la TRANSFORMACIÓN es byte a byte idéntica. Lo
que sigue sin comparar entre los dos sistemas es el extremo a extremo —cada uno
leyendo la base con su capa de datos— y ésa es la última medición que falta
antes de apagar el Python.

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

## La bitácora de acceso (Ley 1581) — las reglas completas

> La forma corta —las tres variantes y sus helpers— está en `CLAUDE.md`.

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
| `GET /api/imagenes/ordenes` (sin filtro) | `Coleccion` |
| `GET /api/imagenes/ordenes?paciente_id=…` | `DeUnTitular` |
| `GET /api/imagenes/ordenes/{id}` y `/informe` | `Recurso` |
| `GET /api/urgencias/cola`, `/api/hospitalizacion`, `/api/incapacidades`, `/api/remisiones`, `/api/citas` | `Coleccion` |
| `GET /api/incapacidades/resumen`, `/api/remisiones/resumen` | `Coleccion`, acción `resumir` |
| `GET /api/incapacidades/{historia_id}`, `GET /api/remisiones/{id}` | `Recurso` |
| `GET /api/rips` | `Coleccion` con `pacientes` |

Las siete últimas, y sus pantallas del panel, entraron el 2026-08-08. **Y con
ellas la otra mitad de la regla, que es la que hace que la tabla signifique
algo**: `GET /api/hospitalizacion/camas`, `/medicos` y las seis de
`/api/agenda/*` **no** anotan — no devuelven dato de ningún paciente.

`/api/documentos` sale dos veces a propósito: **es la regla la que decide, no el
caso** (directiva 23). Con `paciente_id` es el expediente de un titular; sin él
atraviesa titulares. La regla, en una frase: *se anota como `DeUnTitular` toda
lectura que devuelve un conjunto de registros clínicos de UN paciente
identificado en la petición.*

La exportación se anota **solo cuando el archivo llegó a salir** —un 409 por
`parcial`, un 422 por datos faltantes o un 403 por rol no dejan rastro, porque una
bitácora que registra lo que no ocurrió es peor que una que no registra—; y una
ficha de otra institución tampoco, porque el 404 es anterior al acceso.

**Los listados clínicos que atraviesan pacientes ya anotan** —urgencias,
hospitalización, incapacidades, remisiones y citas, cerrados el 2026-08-08 por
las dos puertas—: ver
[La bitácora alcanza los cinco listados](#la-bitácora-alcanza-los-cinco-listados-clínicos-que-faltaban-2026-08-08),
que trae además la regla del RECUENTO (`resumir`) y por qué las camas, los
profesionales y `/api/agenda/*` **no** anotan.

**Lo que sigue fuera, dicho para que no pase por cubierto**: las sublistas de una
historia (`/api/historias/{id}/formulas`, `/ordenes`) quedan fuera por la regla —el
paciente no va en la petición, se deriva—, y `/panel/remisiones/{id}` no existe
todavía, así que su ficha no tiene eslabón que cablear (lo afirma una prueba, que
se pondrá roja el día que la pantalla se añada).

El contenido ilegible no puede acortar el rastro, y el guardián sale gratis: el
índice de expresión `json_extract(contenido,'$.paciente_id')` de la migración
`0015` hay que evaluarlo para escribir la fila, así que SQLite rechaza un
`contenido` que no sea JSON.

## Arquitectura del backend (histórico)

> **HISTÓRICO desde el 2026-08-09.** `main.py`, los `routers/` y sus pruebas por
> `TestClient` se RETIRARON (#598): el producto es el binario Rust y el FEV-RIPS
> ya se genera y prueba en Rust. Lo que SIGUE VIVO del Python es la cadena
> RIPS/FEV-RIPS como módulos —`rips_completo.py`, `fev_rips.py`, `models.py`,
> `database.py`, `cie11.py`, `cum.py` y los catálogos—, **y nada más**.
>
> **CORREGIDO el 2026-08-10, con medición.** Aquí decía que `core/` y `schemas/`
> «quedan como dependencias de esa cadena», y es falso: partiendo de las 13 raíces
> de la cadena viva y siguiendo los `import` transitivamente, de los **60** módulos
> de `backend/` (sin contar `tests/`) solo **17** son alcanzables. Los otros **43**
> no los llama nadie: los 23 de `schemas/`, los **6 de `core/`** —que importan
> `seed.py`, `crear_usuarios_multi_inst.py`, `seguridad.py` y las pruebas, ninguno
> de ellos la cadena— y doce sueltos, entre los que hay cinco módulos de dominio
> clínico (`consentimientos`, `incapacidades`, `remisiones`, `notas_operatorias`,
> `contrarreferencias`) **ya portados a `medico-dominio`**. Podarlos es una
> decisión aparte, no un descuido; lo que no puede quedar es escrito que están
> vivos. El
> diagrama de abajo describe la forma que TENÍA la API; se conserva porque la capa
> de aislamiento y los modelos siguen siendo la referencia de la cadena viva.

```
main.py            [RETIRADO] lifespan, CORS, registro de routers
database.py        engine, pool=20, _run_migrations() (52 ALTER idempotentes)
models.py          83 tablas SQLModel, 25 enums (2 399 líneas, ~99 kB)
core/
  config.py        secretos: falla al arrancar, no se degrada
  passwords.py     Argon2id con migración oportunista desde bcrypt
  aislamiento.py   obtener(): el `db.get` seguro entre instituciones
  security.py      [PODADO 2026-08-10] JWT de la API retirada
  deps.py          [PODADO 2026-08-10] dependencias de FastAPI
routers/           [RETIRADO 2026-08-09] eran 28 reales + stubs.py
schemas/           [PODADO 2026-08-10] Pydantic de petición/respuesta
```

### La poda del 2026-08-10 — qué se fue y qué se quedó, con su motivo

Al verificar #274 se midió que la mitad de `backend/` no la llamaba nadie. **La
primera cuenta dio 43 huérfanos y estaba mal**: contaba solo desde la cadena
RIPS/FEV, y `importer.py`, `seed.py` y `reset_db.py` son puntos de entrada
**manuales** documentados en `CLAUDE.md` — por ellos siguen vivos `core/config`,
`core/passwords` y `seguridad.py`. «No alcanzable desde la cadena» no es
«muerto» (directiva 23). Con las raíces correctas: **35 candidatos**.

**Se borraron 33 archivos** (`pytest` pasó de 561 a 499, y las 62 que faltan son
las de los tres bancos borrados; 0 fallos antes y después):

| qué | por qué |
|---|---|
| `schemas/` entero (23) | Pydantic de petición/respuesta: solo servía a los `routers/` |
| `core/deps.py`, `core/security.py` | dependencias de FastAPI y JWT de la API que ya no existe |
| `auditoria.py`, `migrar_campos_rips.py`, `crear_usuarios_multi_inst.py` | atajos y guiones one-shot de la etapa API |
| `consentimientos.py`, `incapacidades.py` | **con par verificado en Rust**: `dominio::consentimiento` y la regla `fecha_fin = inicio + (dias-1)` de `datos::incapacidad` |
| 3 bancos (`test_jwt_algoritmo`, `test_licencia_vigencia`, `test_consentimientos`) | probaban exactamente lo anterior |

**Se conservaron cinco, y el motivo es la parte que importa** — no son olvido:

| qué | por qué NO se borró |
|---|---|
| `core/__init__.py` | falso positivo del analizador: un `__init__` se importa implícitamente, y `core/config` y `core/passwords` están vivos |
| `core/aislamiento.py` | `COMPARTIDOS` y `ENLACES` son la referencia de qué modelo cuelga de qué dueño, y `test_aislamiento_cobertura.py` la mide |
| `notas_operatorias.py` | **sus reglas NO tienen par en Rust**: la duración quirúrgica con tolerancia de 1 min, los cuatro estados de salida y los avisos vienen de VFP y solo existen aquí |
| `contrarreferencias.py` | igual: `cerrar_remision`, `marcar_recibida` y `requiere_control` no están en `datos::referencia`, que solo trae `destinos` y `esta_abierta` |
| `remisiones.py` | lo **importa** `contrarreferencias.py`. Iba en la lista de borrar y lo salvó el control previo, no la lectura |

Portar a Rust las reglas de los dos primeros es trabajo pendiente; hasta que
exista, borrarlos sería perder una migración de VFP que nadie más tiene.

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

> Esta tabla describe el **Python**. El binario Rust —que es el estándar— no lo
> aplicaba en ninguna parte hasta el 2026-08-07; ver
> [La licencia por módulo se aplica EN EL SERVIDOR](#la-licencia-por-módulo-se-aplica-en-el-servidor-2026-08-07-534).
> El criterio de «a nivel de router y no endpoint por endpoint» es exactamente
> el que se reusó allí para elegir una capa en vez de un extractor.

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

## Procedencia: los cuatro casos corregidos

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

**Y desde el 2026-08-07 la búsqueda por contrato filtra por INSTITUCIÓN** (#501).
`_contratos_vigentes` no lo hacía: barría los contratos de todos los prestadores
de la base y se quedaba con el primero cuyo CUPS casara. Medido con dos IPS que
facturan a la misma EPS y pactaron 50 000 y 99 000 por el mismo código —**las dos
recibían 50 000**, el del vecino—. Es la misma familia que el `hasattr` de
`filtrar_por_institucion`: la llamada estaba escrita, `resolver` ya recibía
`institucion_id` y solo lo usaba para la tarifa propia. Con UNA sola institución
en la base los dos comportamientos dan el mismo número, que es por qué no lo vio
nadie. Sin institución devuelve **vacío**, no «todos»: el valor se congela en la
historia, así que un precio inventado entra en la factura y ya no se distingue de
uno pactado (directiva 20). Lo sujeta
`test_cada_prestador_cobra_SU_tarifa_y_no_la_del_vecino`, con las dos mitades.

**Ojo al alcance, y hay que decirlo**: `tarifador.py` **no tiene un solo llamador
en producción** —solo pruebas—, así que era una mina, no una fuga en curso. Y el
estándar no lo hereda: la facturación en Rust filtra los contratos por dueño
desde que nació y no tiene tarifador.

**Regla dura**: nada que sugiera un modelo se escribe solo en la historia clínica.
Propone, un humano confirma, y queda registrado cuál fue cuál.

## Las derivaciones, y el `DuroSinAval` que por fin se dispara (2026-08-21, #84)

Port de `backend/derivaciones.py` a `medico-dominio/src/derivaciones.rs`. Era el
pendiente (1) de #84: `procedencia.rs` sabía distinguir `DERIVADO` y auditar
«duro sin aval», pero **nada en producción escribía `DERIVADO`**, así que esa
falla no podía dispararse nunca.

**Por qué no había objetivo obvio, y cómo se resolvió.** El Python deriva el
municipio del **texto libre de la ciudad**; el `Paciente` de Rust guarda
`cod_municipio` (código DIVIPOLA) directo y no tenía ese texto. No es que
faltara el `match`: faltaba la FUENTE. Se añadió (`ciudad_residencia` /
`departamento_residencia`, migración `0069`) porque los pacientes del VFP
(#16/#65) entran SIN código, y tirar su ciudad obligaría a recapturar el padrón
a mano. Se descartaron los otros candidatos con su motivo: `valor_servicio` no
se deriva porque Rust **no tiene tarifador** a propósito; el CIE-11 se deriva del
CIE-10 pero ese CIE-10 lo confirmó un humano, así que marcarlo `DERIVADO` haría
de `DuroSinAval` un falso positivo.

**La lógica vive en el DOMINIO y es pura.** `municipio_desde_texto` recibe el
catálogo entero y hace ÉL el filtro por subcadena, plegando tildes — porque el
`LIKE` de la base **no pliega**, y con él «Medellin» no encontraría «Medellín».
Devuelve `None` cuando el nombre es ambiguo (hay diez municipios que contienen
«San Juan», ninguno se llama solo así), y el departamento desambigua
(«San Juan» + «La Guajira» → 44650, San Juan del Cesar, que pese al nombre es de
La Guajira — cazado por una prueba que primero puse en «Cesar» y salió roja). No
aproxima: elegir el primero de varios homónimos es exactamente el defecto que se
combate (directiva 20).

**El enchufe ya estaba puesto.** `documentos::usuario_base` auditaba los campos
del paciente con `rec.en_registro(None)` y un comentario: «su procedencia se
auditará cuando esa captura la escriba (incremento futuro)». Ese futuro llegó:
ahora audita contra `paciente.procedencias` (columna nueva). Como el municipio es
DURO, uno DERIVADO y no mirado sale «duro sin aval» en el informe de cierre —la
recompensa, con su pareja: el mismo municipio CONFIRMADO no se marca—.

**La captura del paciente marca `CONFIRMADO` lo que teclea una persona**, y esa
es la diferencia con la fricción de la historia: en la historia un campo DURO sin
toque explícito se omite, pero el formulario del paciente **es** el toque —lo
llenó un humano—, así que sus campos van confirmados y solo el municipio derivado
queda sin aval. `derivar_municipio` no pisa un código ya puesto (idempotente), y
editar un paciente re-afirma lo tecleado (revisar en el formulario un municipio
derivado lo sube a confirmado, que es correcto: un humano lo miró).

Cinco constructores de `FuentePaciente` (AC/AP/AM/AU/AH) llevan ahora
`p.procedencias`; sin los cinco, un paciente auditado en el AC quedaría ciego en
el AP, un hueco invisible. Pruebas: la pareja pura en `derivaciones.rs`, la
recompensa en `documentos.rs` (`un_municipio_derivado_sale_duro_sin_aval`) y la
derivación de extremo a extremo contra el catálogo real en `paciente.rs`.

**Tres cosas que cerró la revisión de seguridad del mismo día** (dos revisores
independientes, ambos «sin hallazgos», pero las observaciones valían):

1. **`derivar_municipio` lleva ahora el `Ambito`.** Era `pub` y filtraba solo por
   `id`: seguro porque sus dos llamantes acotan antes, pero un tercero que le
   pasara un id del cliente sin acotar la volvía una escritura cruzada de
   institución. Ahora el SELECT y el UPDATE llevan `AND institucion_id = ?`, como
   toda consulta de paciente. Prueba: `derivar_municipio_respeta_el_ambito`.
2. **Editar el paciente ya NO promueve en silencio un municipio derivado a
   `confirmado`.** `procedencias_de_captura` marca `confirmado` todo campo
   presente, y la pantalla de edición pre-rellena el municipio: guardar tras
   tocar otra cosa lo afirmaba sin que nadie lo mirara —desactivando el «duro sin
   aval» que esta pieza existe para disparar—. Regla nueva en `actualizar`: si el
   código no cambió y ya estaba `derivado`, sigue `derivado`; solo un cambio de
   valor cuenta como aval humano. Prueba con su pareja:
   `editar_no_promueve_un_municipio_derivado_a_confirmado`.
3. **La costura DB → `FuentePaciente.procedencias` tiene ahora caso rojo.** Un
   mis-wire futuro (mapear a `None` o al `procedencias` de la historia) fallaría
   ABIERTO y en silencio: `en_registro(None)` salta la auditoría y ningún test se
   pondría rojo. Lo cierra `el_municipio_derivado_leido_de_la_base_sale_duro_sin_aval`
   en `rips.rs`, que lee un municipio derivado de la base y cruza hasta el
   `DuroSinAval`.

**El defecto del padrón (B2), RESUELTO el 2026-08-22 quitando la causa
(directiva 1).** La migración `0069` nacía con `procedencias TEXT NOT NULL
DEFAULT '{}'`, y eso habría marcado `SinRespaldo` la demografía de TODO el padrón
pre-migración en el primer cierre. No era una decisión de volumen: era una
afirmación FALSA. El auditor (`rips::Recolector::en_registro`) lee **tres**
estados, no dos, y estaba escrito desde el principio:

- **blanco/NULL** → «fila anterior a la captura»: se SALTA (despliegue
  incremental). Es la verdad de un paciente del VFP (#16/#65), que nunca pasó por
  la captura nueva: su procedencia es *desconocida*, no *vacía*.
- **`'{}'`** → «capturado bajo el sistema nuevo y NADA avalado»: audita todos sus
  campos con valor como `SinRespaldo`.

Poner `'{}'` de defecto metía al padrón legado en el segundo estado —capturado y
vacío— cuando la verdad es el primero. El arreglo no es una migración que tape el
ruido: es **`DEFAULT ''`**, que deja esas filas en «pre-captura» —el estado que
`en_registro` ya sabe saltar—. Se queda `NOT NULL` (blanco, no NULL) para que
cada lector siga leyendo `String` sin decidir qué hacer con la ausencia; el
blanco YA es el centinela que `Respaldos::desde_json` y `en_registro` entienden.
Es honesto (no afirma nada de esos campos), no bloquea, y **se auto-sana**: en
cuanto un paciente se crea o se edita, `crear`/`actualizar` le escriben su
procedencia real. El municipio DERIVADO sigue saliendo `DuroSinAval` en cuanto se
deriva, que es la señal que valía.

Un ajuste de borde que el `''` obligó: `derivar_municipio` empezaba de
`serde_json::from_str(&procedencias)`, que revienta con `''`. Ahora el BLANCO
empieza de un mapa vacío (es el centinela pre-captura), y solo el JSON CORRUPTO
(`'{roto'`) se propaga como error —no se traga en silencio— (directivas 20/23).
Pruebas con su pareja: `el_padron_legado_en_blanco_no_ensucia_el_cierre`
(documentos), el caso `''` vs `'{}'` en `audita_procedencia_de_los_campos_presentes`
(recolector), y `el_blanco_deriva_pero_el_json_corrupto_revienta` (paciente).

## La demografía del PatientRDA se captura (2026-08-22, #84 → desbloquea #603)

El perfil `PatientRDA` (#603) exige cuatro campos demográficos que `paciente`
no tenía: **nacionalidad, pertenencia étnica (grupo + comunidad), discapacidad e
identidad de género**. El hallazgo estaba anotado desde el 2026-08-15 («construir
el RDA destapó cuatro campos que medico no captura»), y era trabajo de #84: la
CAPTURA. Migración `0070` con las cinco columnas (la etnia son dos: el grupo
codificado y la comunidad en texto libre, que es como el perfil la define —
`ExtensionPatientEthnicCommunity` no lleva binding—).

**El ensamblador del RDA ya estaba listo.** `ihce::actores::Paciente` tenía los
huecos para los cuatro y los emite cuando llegan (probado); lo que faltaba era
alimentarlos, no ensamblar. Por eso el alcance de esta pasada (elegido por Juan)
fue **almacenamiento + catálogos + API**, no la pantalla ni el cableado del RDA a
producción —encolar de verdad manda datos clínicos a la plataforma nacional y es
paso de #603 con revisión de seguridad—.

**Los `display` salen del artefacto, no de memoria** (la lección del `tableta=1`):
`ihce::terminologia` sirve ahora tres CodeSystems vendorizados más —
`ColombianEthnicGroup` (1-6, 99), `ColombianDisabilityClassification` (01-08) y
`ColombianGenderIdentity` (01-05)— con su positivo de control (tamaño exacto del
catálogo) y su pareja (un código que no existe da `None`, no se rellena).
`nacionalidad` es un código ISO 3166-1 numérico **sin catálogo**, igual que
`cod_pais`: el IHCE vendoriza el descriptor de formato `ISO31661` pero NO la lista
de países, así que inventar un catálogo aquí sería justo lo que la disciplina
prohíbe.

La captura marca la procedencia de los cuatro `confirmado` (una persona los
tecleó), aunque el bloque US del RIPS no los audita —son del RDA—: se registra
por consistencia, para tener la evidencia el día que el RDA o una extensión los
mire. Prueba con pareja: `la_demografia_del_rda_se_captura_y_su_procedencia_queda`
(un paciente sin ellos los deja `None` y no ensucia la procedencia).

**Lo que NO se hizo, y por qué**: (a) la pantalla de captura del paciente —hoy no
existe formulario `maud`, se captura por API—; (b) el cableado `actores`→`cola_rda`
a producción (#603, gated). Y de los otros pendientes de #84: más derivaciones
(país/zona) está BLOQUEADO —`zona` no deriva del municipio, no hay importador
VFP—, y medir propuestas aceptadas está BLOQUEADO —ninguna pantalla genera
`propuesto` hoy—.

## La firma de los anexos, y el guardián que faltaba (2026-08-16, #84)

**Cuatro olvidos en la misma lista, y no eran cuatro defectos: era uno.**
`resumen_de_anexos` decide qué columnas de `formula`, `orden` e
`imagen_diagnostica` entran en lo que firma la historia, y se le olvidaron
`unidad` y `observaciones` (revisión del cierre), `procedencias` (directiva 25
sobre `9f8c147d`) y las **trece columnas de dinero de la migración 0041**
(directiva 25 sobre `1c570350`). La causa común es que **nadie vigilaba esa
lista**, mientras `como_nueva` tiene su guardián desde el principio — y el modo
de fallo es el que no da error: la columna nueva entra por una migración, nadie
se acuerda de esta función, y un `UPDATE` contra la base la cambia sin romper la
firma.

Las trece eran las peores porque son DINERO y se leen **en vivo** al cerrar el
mes (`rips.rs`, sin copia congelada): `UPDATE formula SET valor_unitario =
4800000` sobre un respaldo cambiaba lo que se le cobra a la EPS y
`verificar-firma` seguía contestando `Valida`. Y `valor_pago_moderador` sale al
AM con `unwrap_or` **sin pasar por el recolector**, así que era un campo a la vez
no firmado y no auditado: dos capas ausentes sobre la misma cifra.

**Lo que se arregló no es la lista, son las dos causas.**

1. **Las listas `FIRMADAS_*` CONSTRUYEN el `SELECT`.** Antes había una consulta
   escrita a mano y una tupla de N campos alineados por POSICIÓN —el alias
   `FilaFormula` existía porque clippy rechaza las tuplas de seis, «con razón: en
   el `map` de abajo el orden de los campos es lo único que las mantiene
   alineadas»—. Eso es lo que hacía cada olvido invisible. Hoy la lista es la
   consulta, y `json_array` lo calcula SQLite, así que los tipos se conservan sin
   tupla ninguna. JSON1 está compilado en el SQLite vendorizado
   (`-DSQLITE_ENABLE_JSON1`) y ya se usaba en `administracion.rs`.

2. **`toda_columna_de_los_anexos_esta_declarada`.** Lee `PRAGMA table_info` de
   las tres tablas y exige que TODA columna esté en la lista firmada **o** en la
   de excluidas **con su motivo escrito** —de más de 30 caracteres, porque un
   motivo en blanco no es un motivo (directiva 20)—. Comprueba además las dos
   direcciones contrarias: una columna en las dos listas, y una columna declarada
   que ya no existe en el esquema (que reventaría al firmar, en producción, no
   aquí). Con positivo de control: si el `PRAGMA` devolviera vacío —tabla
   renombrada, migración que no corrió— el bucle no compararía nada y saldría
   verde sin haber mirado.

   Validado contra su mutante, que es **la versión anterior de la lista** y no
   uno inventado (directiva 33): con las trece columnas fuera, canta
   `formula.fecha_formula` por su nombre.

**Lo que queda FUERA, con su motivo, porque una exclusión sin motivo escrito es
un olvido con otra ropa**: `historia_id` (es el `WHERE`), `formula.dispensada`
(la mueve farmacia después de firmar), `orden.estado` (flujo posterior),
`imagen_diagnostica.ruta_archivo` (sellar los bytes obligaría a descifrar el
archivo en cada verificación), `cifrada`, `fecha_subida` y `origen` —esta última
porque cambiarla mete o saca la fila de la lista, y eso rompe la firma por sí
solo—.

**`"v"` subió a 2, y la lección es que no subió sola.** El commit anterior amplió
las fórmulas de 8 a 9 elementos y dejó el `1` puesto: **dos formatos distintos
afirmando ser la misma versión**, que es exactamente el fail-open que ese campo
existe para impedir — el día que alguien escriba un verificador que ramifique por
versión, elegiría el layout de 8 para una firma de 9 y `procedencias` saldría de
la verificación sin que nada fallara. El comentario del código decía que entraba
«hoy porque hay cero firmas», y esa condición no se volvió a comprobar.

**Consecuencia de despliegue, declarada**: toda firma emitida con el formato
anterior verifica `Alterada`. Es correcto —no sellaba lo que decía sellar— y se
dice antes de subirlo, no después.

### El aval que se heredaba, y la casilla que no hacía nada

`fusionar_procedencias` partía del mapa existente, hacía `extend` con lo que la
captura afirmó y borraba solo los campos **vacíos**. Un campo DURO con valor y
sin señal lo OMITE `procedencias_de_captura` (`Friccion::Dura → None`), así que
el `extend` no lo tocaba y el `confirmado` anterior **sobrevivía pegado a un
diagnóstico que acababa de cambiar**.

Es la política que ese mismo commit declara inaceptable por escrito —«conservar
el mapa anterior sería PEOR que el fallo»— aplicada al `PUT` de la API y dejada
abierta en la puerta del PANEL, que es la única por la que un humano escribe
avales de verdad. Y el filtro a `confirmado` la **ensanchó**: antes un `derivado`
explícito degradaba el aval; con el filtro se descarta y el viejo se quedaba, de
modo que no quedaba ninguna forma —desde el producto— de retirar un aval de un
campo que sigue teniendo valor.

Los dos revisores independientes llegaron a él por separado, con confianza 8 los
dos. La prueba que existía no lo veía porque sus cinco casos pasan
`existente = None`, que es justo por donde se coló.

### Tres menores del mismo informe

- **La cantidad se valida en la frontera de `serde`, no en cada puerta.** El
  panel había ganado su guarda y las dos rutas JSON no validaban NADA;
  `cantidad: -5` es entera, así que `cantidad_entera` la emitía al AM por el
  camino corto **con `rec.presente()`** — negativa en lo que se le cobra a la
  EPS, y auditada como si alguien la hubiera afirmado. Repetir la guarda en cada
  ruta es una puerta nueva esperando a que se olvide, que es literalmente cómo
  nació el defecto (directiva 1).
- **`diagnostico_relacionado1/2/3` llevaban casilla y no se auditaban jamás**:
  salían con `vacio_a_nulo` directo, sin pasar por el recolector. Una casilla que
  promete un efecto que no ocurre gasta la atención del clínico en algo que nadie
  mira — la simétrica del arreglo que `cantidad` recibió el día anterior. Ahora
  pasan por `rec.presente()` **solo si tienen valor**: son opcionales, y exigir
  tres diagnósticos en toda consulta sería el sesgo a cerrar (directiva 21).
- **`Respaldos::desde_json(...).unwrap_or_default()`** silenciaba una columna
  corrupta —la pantalla la dibujaba como «ningún campo avalado» sin decir nada—,
  el mismo silenciador que `en_registro` acababa de quitar en el recolector. Hoy
  degrada igual y GRITA. Y `avalado` pasó de `avalado_por_humano()` a
  `== Confirmado`, que mata el blanqueo de las filas antiguas con `no_aplica`.

## Fidelidad: los antecedentes contrastados contra la pantalla

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

## Frontend (histórico): el SPA de Angular

**No hay ninguno**, y esa es la descripción exacta desde el 2026-08-08. Las 29
pantallas se tejen en el servidor con `maud` + htmx dentro del mismo binario
(directiva 26); ver
[El panel en `maud`](#el-panel-en-maud--la-migración-al-estándar-nuevo-2026-08-07-en-curso).

Lo que había aquí describía el SPA de Angular 22 + Material —30 páginas, 50
entradas en `app.routes.ts`, el marco de formularios extraído de los 37 de VFP— y
se retiró con él. **`MIGRACION_MATERIAL.md` y `MIGRACION_FORMULARIOS.md` se
conservan como historia del repositorio, no como instrucciones vigentes**: la
receta de PrimeNG → Material ya no se aplica a nada.

### El despliegue es un archivo, y ahora es literal

Antes decía «un proceso y un directorio»: `npm run build` producía estáticos que
servía el mismo FastAPI (decisión #73, «el costo de Angular es de compilación, no
de despliegue»), y el binario Rust los llevaba dentro con `rust-embed`. Eran **un
binario MÁS un árbol de JavaScript que había que construir antes**.

Hoy no hay árbol que construir: `rustc` compila las plantillas, y htmx, `boi.js`
y `panel.css` van con `include_str!` desde `crates/medico-api/panel/`, que está
versionado. `./construir.sh` es un solo `cargo build --release`.

**Y el comodín del SPA se fue con él.** Servía `index.html` con **200** a
cualquier ruta que nadie reconociera —en los dos sistemas—, así que
`GET /api/pacintes` con la errata devolvía HTML donde el cliente espera JSON.
Los dos tenían su lista de prefijos del backend para taparlo; hoy no hay comodín
que tapar: **lo que no case es un 404**, y `/` redirige a `/panel`. Lo sujeta
`la_raiz_lleva_al_panel_y_lo_que_no_existe_es_404`, que comprueba las dos mitades
—incluida que ninguna respuesta traiga ya una página entera—.

## Revisión de tareas tras el cambio de estándar (2026-08-08)

Repasadas **las 35 fichas de medico** de `PENDIENTES.md` más las 7 de esta
sesión, contra el código y no de memoria. La pregunta era cuáles cierra el cambio
de estándar (directiva 26: `maud` + htmx, Angular fuera).

**No se pueden cerrar desde aquí**: las tareas se indexan por SESIÓN, y las `#5xx`
y `#2xx` son de otras (comprobado: `TaskGet 547` → *task not found*). Por eso la
determinación se escribe en este archivo, que sí lo lee quien trabaje en medico.

### Cierran POR el cambio de estándar (5)

| ficha | determinación |
|---|---|
| **#547** migrar el frontal Angular a maud+htmx (23 749 loc) | **HECHA.** 29/29 pantallas, y Angular retirado el mismo día (`0588431`) |
| **#559** transición maud+htmx: nexus → chasqui → inventario → informes → medico | **COMPLETA.** medico era la última pata |
| **#255** PrimeNG → Angular Material | **CIERRA POR RETIRADA.** Se hizo el 2026-08-02 y el objeto ya no existe. La lección se queda: ninguna dependencia de pantalla entra si puede exigir clave |
| **#254** una sola versión de Angular en todos los desarrollos | **CIERRA PARA MEDICO E INFORMES**, que ya no lo tienen. **Sigue viva si algún otro desarrollo conserva Angular** — hay que mirarlo antes de darla por muerta |
| **#236** módulos con pantalla y licencia sin backend en Rust | **CIERRA.** Su medida era «lo que la interfaz Angular pide y el binario no sirve», y ya no hay interfaz externa que pida. Última medición: 197/197 (2026-08-06) |

### Cierran a MEDIAS por el cambio (2)

| ficha | qué cierra | qué queda |
|---|---|---|
| **#249** las pantallas que faltan | la mitad de código: 29/29 | re-derivar las prioridades **con el prestador**, que ya está en «Preguntas abiertas» |
| **#567** las cuatro observaciones de #49 | **O2**: «que la pantalla no presente como si mandaran las columnas de la política» — hecho en `/panel/admin/seguridad` | **O6**: leer `requiere_cambio_password` al entrar y forzar el cambio. Decía «necesita pantalla»; ya hay pantalla, así que pasa de BLOQUEADA a hacible |

### Enunciado caduco, ficha viva (1)

**#274** dice «Migrar medico a Rust **con Angular embebido en el binario**». El
objetivo ya no lleva Angular. La ficha sigue viva por lo de siempre: **FEV-RIPS
no está portado**, y sin él no se apaga el Python.

### Lo que el cambio NO cierra, y conviene decirlo

Las siete de esta sesión (#586–#590 y las dos suyas) **no las toca**, y una
empeora: **#586** —el banco de navegador no alcanza las pantallas con sesión— pasa
de tarea cómoda a la red que falta, porque ya no hay frontal anterior al que
volver. Y **#587** sigue igual de vigente: los tres campos obligatorios del
informe radiológico se exigían en el navegador de Angular y ahora se exigen con
`required` en el del panel, que es exactamente lo mismo de flojo.

Tampoco toca las de datos y norma, que son la mayoría: #16/#65 (VFP), #67 y #65.7
(FEV-RIPS), #77 (CUM a base), #88 (barrido de aislamiento), #84 (captura
asistida), #104 (talla ideal), #250, #252, #186, #212, #492, #501, #531, #533,
#535, #537, #538, #539, ni las de infraestructura (#310, #560) ni las de producto
(#19, #20, #22).

### El hallazgo que sí cambia el encuadre de tres fichas

**El API HTTP del Python se quedó sin cliente desplegado.** Angular era el único,
y el despliegue arranca **solo** `medico-api` (`despliegue/medico-demo.service`:
`ExecStart=/opt/medico-demo/medico-api`; ningún `uvicorn` en `despliegue/`). Lo
que de ese sistema se usa es la cadena RIPS → FEV-RIPS → MUV, que se invoca como
MÓDULOS y guiones, no por HTTP.

Eso no vuelve falsas a **#531** (`cast(fecha, Date)` en `estadisticas.py`),
**#501** (facturación sin aislar en el Python vivo) ni la mitad Python de
**#535**: los defectos están ahí y son reales. Lo que cambia es que hoy están en
código que **nadie llama**, así que arreglarlos o retirar esos routers es una
decisión de Juan, no una urgencia. Se anota en vez de resolverse porque decidirlo
por mi cuenta sería borrar trabajo ajeno (directiva 22 aplicada al inventario).

