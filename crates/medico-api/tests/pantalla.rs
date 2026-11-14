// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! **La pantalla del panel, DIBUJADA en un navegador de verdad.**
//!
//! El resto del banco de este crate le habla al router con `oneshot` y afirma
//! sobre lo que devuelve axum. Eso mide el CÓDEC. Desde que el panel es `maud` +
//! htmx (directiva 26) la pantalla la termina de construir el navegador: un
//! `<script type="module">` bloqueado por la CSP, servido con el MIME
//! equivocado o que revienta al importar **devuelve el mismo 200 con el mismo
//! cuerpo**.
//!
//! Es la directiva 28 —cruzar la frontera del programa— y hasta hoy este
//! repositorio no la cumplía para la pantalla: tenía 390 pruebas de frontend en
//! `vitest`, que montan componentes de Angular en jsdom, y **cero** que
//! arrancaran el binario y lo pintaran. Ya costó una vez: 366 pruebas en verde,
//! `ng build` en verde, y el producto se veía con serifas porque el tema traía
//! `Roboto` y Roboto no viajaba en el binario.
//!
//! ## Alcanza las pantallas CON sesión desde el 2026-08-08
//!
//! Hasta ese día solo llegaba a las de sin sesión: `chromium --dump-dom` no
//! admite mandar una cookie —no existe `--header`— y las 28 pantallas restantes
//! no se habían dibujado nunca. Con Angular retirado eso dejó de ser una laguna
//! cómoda: ya no hay frontal anterior al que volver.
//!
//! Lo cierra [`comun`], que habla **CDP por tubería**. Y la sesión no se
//! inyecta: se **ENTRA por el formulario**, como entra una persona, así que el
//! acceso queda medido de paso.
//!
//! Las pruebas de aquí siguen usando `--dump-dom` donde basta —es más simple, y
//! una pieza que no hace falta no se usa por tenerla—; las que necesitan sesión
//! o una interacción usan el navegador dirigido.
//!
//! ## Lo que este banco sigue sin cubrir
//!
//! **Dibuja y ejecuta; no mira píxeles.** Que una tabla salga descuadrada, que
//! un color no contraste o que un formulario quede fuera de la pantalla en un
//! móvil no lo ve nadie todavía.
//!
//! Y no recorre las 29 pantallas: recorre una muestra. Que la cadena de módulos
//! corra bajo la CSP se comprueba una vez, y vale para todas porque **todas
//! salen de la MISMA `vista::pagina`**, con los mismos `<script>` y la misma
//! CSP.

// El arnés es `mirilla`, la pieza extraída de este mismo repositorio y de
// `informes` (directiva 9). Aquí vivía una copia de 319 líneas cuya única
// diferencia con la otra era la ruta de la entrada.
use mirilla::{buscar_chromium, Navegador};
use std::process::{Child, Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_medico-api");
const SECRETO: &str = "un-secreto-de-sesion-suficientemente-largo-para-esta-prueba";

/// Pinta `url` y devuelve el DOM ya construido, después de que el JavaScript de
/// la página haya corrido.
///
/// Un perfil ÚNICO por llamada: con uno compartido, dos pruebas en paralelo
/// chocan y Chromium aborta con «Failed to create SingletonLock».
fn dibujar(chromium: &str, url: &str) -> String {
    static SIGUIENTE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SIGUIENTE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let perfil = std::env::temp_dir().join(format!("medico-perfil-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&perfil);
    let salida = Command::new(chromium)
        .args([
            "--headless=new",
            "--no-sandbox",
            "--disable-gpu",
            "--disable-dev-shm-usage",
            &format!("--user-data-dir={}", perfil.display()),
            // Deja correr el JavaScript antes de volcar: sin esto se mide el
            // HTML que llegó, no la pantalla que quedó.
            "--virtual-time-budget=4000",
            "--dump-dom",
            url,
        ])
        .output()
        .expect("Chromium no arrancó");
    let _ = std::fs::remove_dir_all(&perfil);
    assert!(
        salida.status.success(),
        "Chromium salió con error: {}",
        String::from_utf8_lossy(&salida.stderr)
    );
    String::from_utf8_lossy(&salida.stdout).into_owned()
}

struct Servidor {
    hijo: Child,
    puerto: u16,
    #[allow(dead_code)]
    dir: tempfile::TempDir,
}

impl Drop for Servidor {
    fn drop(&mut self) {
        let _ = self.hijo.kill();
        let _ = self.hijo.wait();
    }
}

/// Arranca el binario REAL en un puerto efímero y espera a su aviso de que está
/// escuchando — no a que el archivo de la base exista, que es una señal anterior
/// a las migraciones.
fn arrancar(extra: &[(&str, &str)]) -> Servidor {
    arrancar_con(extra, false)
}

/// El correo y la clave de la demostración. Se usan para ENTRAR de verdad.
/// La ruta de la entrada: **lo único de este producto que el arnés necesita
/// saber**. Es el parámetro que justificó extraer `mirilla` — informes entra
/// por `/entrar` y medico por aquí, y de ~309 líneas de arnés era la única
/// diferencia entre las dos copias.
const ENTRADA: &str = "/panel/entrada";
const CORREO: &str = "demo@medico.local";
const CLAVE: &str = "clave-de-demostracion-larga";

/// Arranca con una institución y un usuario ya sembrados, por el **camino del
/// propio producto**: `medico-api sembrar-demo`, que es lo que corre el
/// despliegue. Sembrar a mano con `sqlx` habría fabricado un estado que el
/// binario no produce nunca, y entonces la prueba mediría el arnés.
fn arrancar_sembrado() -> Servidor {
    arrancar_con(&[], true)
}

fn arrancar_con(extra: &[(&str, &str)], sembrar: bool) -> Servidor {
    let dir = tempfile::tempdir().expect("carpeta temporal");
    let base = dir.path().join("medico.db");

    if sembrar {
        let salida = Command::new(BIN)
            .arg("sembrar-demo")
            .current_dir(dir.path())
            .env("SECRET_KEY", SECRETO)
            .env("DATABASE_URL", format!("sqlite://{}?mode=rwc", base.display()))
            .env("MEDICO_DEMO_CORREO", CORREO)
            .env("MEDICO_DEMO_CLAVE", CLAVE)
            .output()
            .expect("no se pudo sembrar la demostración");
        assert!(
            salida.status.success(),
            "`sembrar-demo` falló: {}",
            String::from_utf8_lossy(&salida.stderr)
        );
    }

    let mut cmd = Command::new(BIN);
    cmd.current_dir(dir.path())
        .env("SECRET_KEY", SECRETO)
        .env("DATABASE_URL", format!("sqlite://{}?mode=rwc", base.display()))
        // Puerto 0 = que el sistema elija. El binario dice el REAL en su log.
        .env("PORT", "0")
        .env("MEDICO_ESCUCHA", "127.0.0.1")
        // Sin proxy declarado: aquí no hay nginx que escriba `X-Real-IP`, y con
        // la bandera puesta el acceso respondería 503 — que es correcto en
        // producción y rompería esta prueba por el motivo equivocado.
        .env("MEDICO_TRAS_PROXY", "false")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    for (k, v) in extra {
        cmd.env(k, v);
    }
    let mut hijo = cmd.spawn().expect("no se pudo arrancar medico-api");

    let salida = hijo.stdout.take().expect("se pidió stdout por tubería");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for linea in std::io::BufReader::new(salida).lines().map_while(Result::ok) {
            if let Some(i) = linea.find("escuchando en http://") {
                let cola = &linea[i..];
                if let Some(p) = cola.rsplit(':').next().and_then(|s| s.trim().parse::<u16>().ok())
                {
                    let _ = tx.send(p);
                    return;
                }
            }
        }
    });
    let puerto = rx
        .recv_timeout(std::time::Duration::from_secs(60))
        .expect("el binario no dijo su puerto en 60 s");

    Servidor { hijo, puerto, dir }
}

/// **La prueba de que la cadena de módulos CORRE**, y no solo de que se sirve.
///
/// `boi.js` es un `<script type="module">`. Si la CSP lo bloqueara, si el MIME
/// llegara mal o si reventara al importar, la respuesta sería exactamente la
/// misma —200, mismo cuerpo— y el DOM volcado sería idéntico al que se pidió.
/// Por eso no basta con afirmar sobre el HTML: hay que preguntarle al
/// NAVEGADOR si el módulo se ejecutó.
///
/// **La aguja se valida contra el ARTEFACTO** (directiva 33): no se inventa.
/// `boi.js` escribe `data-boi="listo"` en `<html>` al engancharse, y lo hace
/// exactamente para esto — su propio comentario lo dice: «es lo único que un E2E
/// puede mirar para distinguir "boi cargó" de "el `<script type=module>` está en
/// el HTML y el navegador no lo ejecutó"». Aquí se comprueba que la marca esté
/// en el DOM VOLCADO, y abajo se comprueba que esa marca de verdad viene del
/// archivo servido y no de una constante de esta prueba.
#[test]
fn la_entrada_se_dibuja_y_boi_se_ejecuta() {
    let chromium = buscar_chromium();
    let s = arrancar(&[]);
    let dom = dibujar(&chromium, &format!("http://127.0.0.1:{}/panel/entrada", s.puerto));

    // 0. **LO QUE DE VERDAD MIDE ESTA PRUEBA**: el módulo se EJECUTÓ. Sin esta
    //    línea, todo lo demás lo pasaría igual una pantalla cuyo JavaScript no
    //    corre — que es el fallo silencioso que este banco existe para cazar.
    assert!(
        dom.contains("data-boi=\"listo\"") || dom.contains("data-boi"),
        "`boi.js` NO se ejecutó: está enlazado pero el navegador no lo corrió \
         (CSP, MIME o un fallo al importar). DOM:\n{dom}"
    );

    // 1. La pantalla EXISTE y es la de entrada.
    assert!(
        dom.contains("Entrar") && dom.contains("name=\"clave\""),
        "no se dibujó el formulario de entrada. DOM:\n{dom}"
    );
    // 2. La hoja de estilos se enlazó (y por tanto se sirvió con su MIME: un
    //    CSS con el tipo equivocado lo descarta el navegador en silencio).
    assert!(dom.contains("/panel/panel.css"), "no se enlazó la hoja del panel");
    // 3. Los dos módulos vendorizados están, y son los LOCALES: si alguien los
    //    cambiara por un CDN, esta línea se pone roja — y ése es justo el
    //    cambio que no puede colarse en un producto que corre sin internet.
    assert!(dom.contains("/panel/htmx.min.js"), "htmx no se enlazó local");
    assert!(dom.contains("/panel/boi.js"), "boi no se enlazó local");
    assert!(dom.contains("/panel/boi.css"), "boi.css (la estética) no se enlazó local");
    assert!(
        !dom.contains("cdn.") && !dom.contains("unpkg") && !dom.contains("jsdelivr"),
        "hay un script de un CDN en la pantalla: este producto corre sin internet"
    );
    // 4. **La contraseña NO viaja de vuelta.** Un formulario que devuelve la
    //    clave en un `value` la deja en el historial y en cualquier proxy.
    assert!(
        !dom.contains(&format!("value=\"{SECRETO}\"")),
        "la pantalla devolvió un secreto en un value"
    );
}

/// **La pareja del anterior** (directiva 33). Sin ésta, un DOM en blanco o una
/// pantalla que dijera «Entrar» para todo pasaría igual: hay que comprobar que
/// el instrumento distingue.
///
/// Se pide una ruta del panel SIN sesión y tiene que acabar en la ENTRADA, no
/// en un 401 en crudo ni en la pantalla de pacientes. Es exactamente el defecto
/// que el banco de navegador de informes cazó en su primera corrida.
#[test]
fn sin_sesion_una_pantalla_del_panel_lleva_a_la_entrada() {
    let chromium = buscar_chromium();
    let s = arrancar(&[]);
    let dom = dibujar(&chromium, &format!("http://127.0.0.1:{}/panel/pacientes", s.puerto));

    assert!(
        dom.contains("name=\"clave\""),
        "sin sesión, /panel/pacientes no llevó a la entrada. DOM:\n{dom}"
    );
    // Y el control en la otra dirección: NO se coló el contenido protegido.
    assert!(
        !dom.contains("Documento") || !dom.contains("tabla"),
        "se dibujó el listado de pacientes sin sesión"
    );
}

/// **La aguja viene del artefacto, no de esta prueba.**
///
/// `data-boi="listo"` solo sirve como control si de verdad la escribe el `boi.js`
/// que el binario SIRVE. Si mañana alguien vendorizara una versión sin esa
/// línea, la prueba de arriba se pondría roja y se leería como «el navegador no
/// ejecuta módulos» —conclusión falsa y cara—. Esta comprueba que la marca está
/// en el archivo servido, así que el rojo de arriba se puede creer.
///
/// Es la lección del medidor de residuo que dio cero cuatro veces: el control
/// plantaba la misma aguja inventada que la medición buscaba.
#[test]
fn la_marca_de_boi_existe_en_el_archivo_que_el_binario_sirve() {
    let s = arrancar(&[]);
    let url = format!("http://127.0.0.1:{}/panel/boi.js", s.puerto);
    let cuerpo = Command::new("curl")
        .args(["-s", "--max-time", "15", &url])
        .output()
        .expect("curl");
    let js = String::from_utf8_lossy(&cuerpo.stdout);
    assert!(
        js.contains("data-boi"),
        "el `boi.js` servido no escribe la marca `data-boi`: la prueba de \
         ejecución de arriba estaría buscando una aguja que ya no existe"
    );
    // Y el control de que se sirvió ALGO: un 404 vacío también «no contiene» la
    // marca, y habría hecho pasar la aserción de arriba por el motivo contrario.
    assert!(
        js.contains("export function registrar"),
        "no se sirvió boi.js: se recibieron {} bytes",
        js.len()
    );

    // Y la mitad ESTÉTICA: la ruta /panel/boi.css sirve el CSS con sus tokens. El
    // <link> en el <head> ya lo cubre `la_entrada_se_dibuja`; esto cubre que la
    // RUTA sirva —un <link> a un /panel/boi.css que diera 404 dejaría la pantalla
    // sin estilos EN SILENCIO, el mismo modo de fallo que un <script> 404—.
    let css_url = format!("http://127.0.0.1:{}/panel/boi.css", s.puerto);
    let css = Command::new("curl")
        .args(["-s", "--max-time", "15", &css_url])
        .output()
        .expect("curl");
    let css = String::from_utf8_lossy(&css.stdout);
    assert!(
        css.contains("--boi-acento"),
        "no se sirvió boi.css (o no trae sus tokens): {} bytes",
        css.len()
    );
    // Autocontenido (sin CDN): se mide sobre la hoja servida, no solo sobre el
    // DOM que la enlaza. Las fuentes van como `data:` (local); un `@import` o una
    // `url(http…)` remota rompería la vendorización sin que nada lo viera.
    let css_min = css.to_lowercase();
    assert!(!css_min.contains("@import"), "boi.css trae un @import: no es autocontenido");
    assert!(!css_min.contains("url(http"), "boi.css referencia un recurso remoto: no es autocontenido");

    // Y la CSP admite las fuentes `data:` que boi.css empotra: con `font-src
    // 'self'` a secas el navegador bloquea IBM Plex EN SILENCIO (medido en el
    // concilio 2026-09-29). Guarda la regresión de re-quitar `data:`.
    let cab = Command::new("curl")
        .args(["-s", "-D", "-", "-o", "/dev/null", "--max-time", "15",
               &format!("http://127.0.0.1:{}/entrar", s.puerto)])
        .output()
        .expect("curl");
    let cab = String::from_utf8_lossy(&cab.stdout).to_lowercase();
    assert!(
        cab.contains("font-src 'self' data:"),
        "la CSP no permite fuentes data:: boi.css no cargaría IBM Plex. Cabeceras:\n{cab}"
    );
}

/// **El comportamiento de `boi` se REGISTRA en el navegador.**
///
/// `confirmar-doble` es un segundo `<script type="module">` que importa
/// `../boi.js`. Si esa ruta relativa no resolviera —el fallo más fácil de
/// cometer al vendorizar, porque en el repositorio de `boi` el archivo vive un
/// nivel por debajo y aquí había que servirlo igual— el módulo reventaría al
/// importar, **en silencio**: misma respuesta, mismo DOM, y el botón «Cerrar
/// todas las sesiones» actuaría a la PRIMERA pulsación.
///
/// Se comprueba pidiendo el archivo tal cual y afirmando que su import apunta a
/// donde el binario sirve `boi.js`. No es el navegador ejecutándolo —eso solo se
/// vería con sesión, que este arnés no alcanza— y se dice para no dar por
/// cubierto lo que no lo está.
#[test]
fn el_comportamiento_de_boi_se_sirve_y_su_import_resuelve() {
    let s = arrancar(&[]);
    let pedir = |ruta: &str| {
        let salida = Command::new("curl")
            .args(["-s", "-w", "\n%{http_code}", "--max-time", "15",
                   &format!("http://127.0.0.1:{}{ruta}", s.puerto)])
            .output()
            .expect("curl");
        String::from_utf8_lossy(&salida.stdout).into_owned()
    };

    let js = pedir("/panel/comportamientos/confirmar-doble.js");
    assert!(js.trim_end().ends_with("200"), "el comportamiento no se sirvió: {js}");
    assert!(
        js.contains("data-confirmar"),
        "no es el comportamiento esperado: {}",
        &js[..js.len().min(300)]
    );
    // El import relativo del archivo vendorizado, sin tocar: `../boi.js` desde
    // `/panel/comportamientos/` resuelve a `/panel/boi.js`. Se comprueba que esa
    // ruta EXISTE, que es lo que hace válido el import.
    assert!(js.contains("from \"../boi.js\""), "el import no es el del artefacto compartido");
    let boi = pedir("/panel/boi.js");
    assert!(
        boi.trim_end().ends_with("200"),
        "`../boi.js` no resuelve desde el comportamiento: el módulo reventaría al importar"
    );

    // La pareja: una ruta hermana que NO existe da 404, o sea que el 200 de
    // arriba significa algo. Sin esto, un servidor que respondiera 200 a todo
    // pasaría esta prueba entera.
    let inventado = pedir("/panel/comportamientos/no-existe.js");
    assert!(
        inventado.trim_end().ends_with("404"),
        "una ruta inventada bajo el panel devolvió algo distinto de 404: {inventado}"
    );
}

/// El binario arranca y sirve su hoja de estilos.
///
/// Es la pareja que hace útiles a las de arriba: comprueba que el DOM volcado
/// CAMBIA según lo que se pida, o sea que el banco mide la pantalla y no una
/// constante.
#[test]
fn el_binario_arranca_y_sirve_el_panel_con_su_hoja() {
    let chromium = buscar_chromium();
    let s = arrancar(&[]);

    // La hoja de estilos, pedida como la pide el navegador.
    let dom = dibujar(&chromium, &format!("http://127.0.0.1:{}/panel/panel.css", s.puerto));
    assert!(
        dom.contains("--acento") || dom.contains("tarjeta"),
        "la hoja del panel no se sirvió: {}",
        &dom[..dom.len().min(400)]
    );
}

// ── Con sesión: lo que hasta el 2026-08-08 no se había dibujado nunca ────────

/// **Se ENTRA de verdad y una pantalla con sesión se dibuja.**
///
/// Es la mitad que faltaba del banco. `--dump-dom` no puede mandar una cookie,
/// así que las 28 pantallas del panel que exigen sesión no habían pasado por un
/// navegador ni una vez — y desde que Angular salió, el panel es el producto
/// entero.
///
/// La sesión no se inyecta: se rellena el formulario y se envía. Así esta prueba
/// mide TAMBIÉN el acceso, que es más de lo que daría una cookie puesta a mano:
/// una cookie mal marcada —`Secure` sobre http, un `SameSite` equivocado, una
/// ruta que no cubre `/panel`— la aceptaría igual un `curl` y la rechazaría el
/// navegador, en silencio.
#[test]
fn se_entra_por_el_formulario_y_el_panel_se_dibuja() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);

    nav.entrar(&base, ENTRADA, CORREO, CLAVE);
    assert_eq!(
        nav.texto("location.pathname"),
        "/panel",
        "tras entrar no se aterrizó en el panel"
    );

    // Una pantalla CON sesión, dibujada por primera vez en un navegador.
    nav.ir(&format!("{base}/panel/pacientes"));
    assert_eq!(nav.texto("document.querySelector('h1').textContent"), "Pacientes");
    assert!(
        nav.evaluar("document.documentElement.dataset.boi === 'listo'") == serde_json::json!(true),
        "`boi.js` no se ejecutó en una pantalla con sesión"
    );

    // **La hoja de estilos se APLICA**, no solo se enlaza. Importa desde que la
    // CSP cerró `style-src 'unsafe-inline'` el 2026-08-08: si algo del panel
    // dependiera de un estilo en línea, ya no se vería «raro» — no se aplicaría,
    // y el HTML sería exactamente el mismo.
    let relleno = nav.texto(
        "getComputedStyle(document.querySelector('.tarjeta')).padding",
    );
    assert!(
        relleno != "0px" && !relleno.is_empty(),
        "la hoja del panel no se aplicó: `.tarjeta` salió con el relleno por \
         defecto del navegador ({relleno})"
    );
}

/// **EL INTERCAMBIO DE HTMX OCURRE, medido en el navegador.**
///
/// `/panel/rips` pide la revisión del periodo con `hx-get` y la mete en
/// `#resultado`. Era el único intercambio del panel y **el único trozo del
/// producto que ningún banco tocaba**: con `hx-target="#resultados"` —una letra
/// de más— el servidor responde el mismo 200 con el mismo fragmento, htmx no
/// encuentra dónde meterlo y **no pasa nada**. Ni error, ni pantalla en blanco:
/// el botón no hace nada.
///
/// Lo que ya estaba cubierto por el banco de rutas es que el destino declarado
/// EXISTE en la propia pantalla y que lo que vuelve es un fragmento. Lo que
/// faltaba —y esto cierra— es que htmx haga el intercambio de verdad.
///
/// La pareja va dentro: se mira el destino ANTES de pulsar. Sin eso, un destino
/// que ya viniera lleno del servidor pasaría la prueba sin que htmx hiciera
/// nada.
#[test]
fn el_intercambio_de_htmx_del_rips_ocurre_en_el_navegador() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);
    nav.entrar(&base, ENTRADA, CORREO, CLAVE);

    nav.ir(&format!("{base}/panel/rips"));
    // El destino existe y está VACÍO: la mitad de control.
    assert!(
        nav.evaluar("!!document.querySelector('#resultado')") == serde_json::json!(true),
        "no está el destino del intercambio en la pantalla"
    );
    assert_eq!(
        nav.texto("document.querySelector('#resultado').innerHTML.trim()"),
        "",
        "el destino ya venía lleno del servidor: esta prueba no mediría el intercambio"
    );

    nav.evaluar(
        "document.querySelector('input[name=inicio]').value = '2026-08-01';\
         document.querySelector('input[name=fin]').value = '2026-08-31';\
         document.querySelector('form[hx-get] button[type=submit]').click();",
    );
    nav.esperar(
        "document.querySelector('#resultado').innerHTML.trim().length > 0",
        "htmx no hizo el intercambio: el destino sigue vacío tras pulsar",
    );

    // Y lo que entró es la REVISIÓN, no cualquier cosa: un fragmento equivocado
    // llenaría el destino igual.
    let dentro = nav.texto("document.querySelector('#resultado').textContent");
    assert!(
        dentro.contains("2026-08-01") && dentro.contains("2026-08-31"),
        "el fragmento insertado no es la revisión del periodo pedido: {dentro}"
    );
    // Y no se coló una página entera dentro del `<div>`.
    assert!(
        !nav.texto("document.querySelector('#resultado').innerHTML").contains("<nav"),
        "se insertó una página entera en el destino, no un fragmento"
    );
}

/// **El caso rojo del arnés** (directiva 33): sin él, las dos de arriba estarían
/// verdes con un instrumento que dice que sí a todo.
///
/// Se pide una pantalla del panel SIN entrar y tiene que acabar en la entrada.
/// Es el mismo navegador dirigido, la misma secuencia, y el resultado tiene que
/// ser el contrario.
#[test]
fn el_navegador_dirigido_distingue_tener_sesion_de_no_tenerla() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);

    // SIN entrar.
    nav.ir(&format!("{base}/panel/pacientes"));
    assert_eq!(
        nav.texto("location.pathname"),
        "/panel/entrada",
        "sin sesión, /panel/pacientes no llevó a la entrada"
    );
    assert!(
        nav.evaluar("!document.querySelector('.tabla')") == serde_json::json!(true),
        "se dibujó el listado de pacientes sin sesión"
    );

    // Y ahora sí: la MISMA URL, con sesión, enseña la pantalla. Es la pareja
    // que hace creíble el negativo de arriba.
    nav.entrar(&base, ENTRADA, CORREO, CLAVE);
    nav.ir(&format!("{base}/panel/pacientes"));
    assert_eq!(nav.texto("location.pathname"), "/panel/pacientes");
}

/// **Una clave equivocada NO abre sesión**, medido en el navegador.
///
/// Es la otra pareja del acceso: `entrar` da por bueno cualquier cambio de URL,
/// así que sin esto un servidor que dejara pasar a todo el mundo pasaría las
/// pruebas de arriba.
#[test]
fn una_clave_equivocada_no_entra() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);

    nav.ir(&format!("{base}/panel/entrada"));
    nav.evaluar(
        "document.querySelector('input[name=email]').value = 'demo@medico.local';\
         document.querySelector('input[name=clave]').value = 'la-que-no-es';\
         document.querySelector('form').submit();",
    );
    nav.esperar(
        "document.querySelector('.aviso') !== null || document.querySelector('input[name=clave]') !== null",
        "la pantalla no contestó nada a un acceso fallido",
    );
    assert_eq!(
        nav.texto("location.pathname"),
        "/panel/entrada",
        "una clave equivocada abrió sesión"
    );
    // Y la puerta sigue cerrada de verdad, no solo de aspecto.
    nav.ir(&format!("{base}/panel/pacientes"));
    assert_eq!(nav.texto("location.pathname"), "/panel/entrada");
}

/// **El buscador de paciente, TECLEADO en un navegador de verdad.**
///
/// Es la prueba que la migración del `<select>` al buscador necesita y que
/// ninguna afirmación sobre el HTML puede dar: el intercambio lo hace htmx en el
/// navegador, y un fragmento correcto que se inserta en el sitio equivocado —o
/// que no se inserta— devuelve exactamente el mismo 200 con el mismo cuerpo
/// (directiva 28).
///
/// Se siembran los pacientes por la ruta JSON con la sesión ya abierta, que es
/// como los crearía un cliente: `sembrar-demo` deja el padrón vacío, y con cero
/// pacientes esto se pondría verde sin mirar nada.
#[test]
fn el_buscador_de_paciente_intercambia_la_lista_al_teclear() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);
    nav.entrar(&base, ENTRADA, CORREO, CLAVE);

    // Veinticinco pacientes: más que el techo de veinte, que es lo que hace
    // visible el recorte. Uno con un apellido inconfundible para buscarlo.
    nav.ir(&format!("{base}/panel/pacientes"));
    nav.evaluar(
        r#"window.__sembrado_ok = false;
           (async () => {
             const j = {'Content-Type': 'application/json'};
             for (let i = 1; i <= 25; i++) {
               await fetch('/api/pacientes', {
                 method: 'POST', headers: j,
                 body: JSON.stringify({
                   tipo_documento: 'CC', numero_documento: String(2000 + i),
                   nombre1: 'Paciente' + i, apellido1: 'Zapata',
                   fecha_nacimiento: '1990-01-01', sexo: 'F'
                 })
               });
             }
             // Y una ambulancia que pueda salir: sin ella el despacho no dibuja
             // el formulario y esto mediría una pantalla vacía. `sembrar-demo`
             // trae salas de quirófano pero ni pacientes ni ambulancias.
             const r = await fetch('/api/ambulancias', {
               method: 'POST', headers: j,
               body: JSON.stringify({placa: 'ABC123', tipo: 'TAB', vence_soat: '2030-01-01'})
             });
             window.__sembrado_ok = r.ok;
           })();"#,
    );
    nav.esperar(
        "window.__sembrado_ok === true",
        "no se pudieron sembrar los pacientes y la ambulancia por la ruta JSON",
    );

    nav.ir(&format!("{base}/panel/ambulancias/despachar"));
    // CONTROL de que se está midiendo el buscador y no una pantalla vacía: el
    // campo existe y la lista trae el techo, no las 25.
    nav.esperar(
        "document.querySelector('#q_paciente') !== null",
        "la pantalla de despacho no dibujó el buscador de paciente",
    );
    let al_abrir = nav
        .evaluar("document.querySelectorAll('#paciente_id option').length")
        .as_i64()
        .unwrap_or(0);
    assert!(
        (2..=21).contains(&al_abrir),
        "al abrir salieron {al_abrir} opciones: o no hay padrón o se trajo entero"
    );

    // Y AHORA LO QUE SOLO SE VE EN EL NAVEGADOR: se teclea y la lista cambia.
    nav.evaluar(
        "const c = document.querySelector('#q_paciente');\
         c.value = 'Paciente7';\
         c.dispatchEvent(new Event('input', {bubbles: true}));",
    );
    nav.esperar(
        "document.querySelectorAll('#paciente_id option').length === 2",
        "htmx no intercambió la lista al teclear (el hx-swap no llegó a su destino)",
    );
    let texto = nav.texto("document.querySelector('#paciente_id').textContent");
    assert!(texto.contains("Paciente7"), "la lista intercambiada no trae al buscado: {texto}");
    assert!(!texto.contains("Paciente8"), "la lista no se acotó: {texto}");

    // Y el fragmento entró DENTRO del formulario, no en su sitio: si htmx
    // hubiera reemplazado la página, el resto del formulario habría desaparecido.
    assert!(
        nav.evaluar("document.querySelector('input[name=origen]') !== null")
            == serde_json::json!(true),
        "el intercambio se llevó por delante el resto del formulario"
    );
}

// ── El segundo factor, DIBUJADO (#212) ──────────────────────────────────────

/// Segundos desde la época. Lo pide el TOTP en dos sitios de la prueba de abajo,
/// y escribirlo dos veces invita a que uno de los dos se quede atrás.
fn unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("el reloj va hacia delante")
        .as_secs()
}

/// **El ciclo entero del segundo factor, en un navegador de verdad.**
///
/// Es la directiva 28 aplicada a lo más delicado que tiene el producto: la
/// puerta. Todo esto está probado por ruta —23 pruebas—, y aun así ninguna de
/// ellas puede decir si la persona LLEGA a ver el QR, si el segundo paso se
/// dibuja o si la cookie pendiente sobrevive al viaje. Un `Set-Cookie` mal
/// marcado lo acepta un cliente HTTP y lo tira el navegador **en silencio**, que
/// es exactamente el fallo que este banco existe para cazar (y del que ya hubo
/// uno: dos `Set-Cookie` en un array se pisaban, y la respuesta seguía siendo
/// 200).
///
/// Se recorre el camino completo: activar → escanear → confirmar → salir →
/// volver a entrar en DOS pasos. Sin la última mitad, esto probaría que la
/// pantalla se pinta y no que el segundo factor sirve para algo.
#[test]
fn el_segundo_factor_se_activa_y_el_acceso_pasa_a_dos_pasos() {
    use totp_rs::{Algorithm, Secret, TOTP};

    let chromium = buscar_chromium();
    // Sin frase de reposo NO hay segundo factor: el secreto no se guarda en
    // claro «mientras tanto». Se pasa como el despliegue la pasaría.
    let s = arrancar_con(
        &[("MEDICO_STORAGE_KEY", "una frase de reposo larga para el banco")],
        true,
    );
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);

    nav.entrar(&base, ENTRADA, CORREO, CLAVE);

    // 1. La pantalla ofrece activar —y lo hace porque HAY frase de reposo: sin
    //    ella diría por qué no se puede, que es la otra mitad y se prueba por
    //    ruta.
    nav.ir(&format!("{base}/panel/segundo-factor"));
    assert!(
        nav.evaluar("!!document.querySelector('form[action=\"/panel/segundo-factor/empezar\"]')")
            == serde_json::json!(true),
        "la pantalla no ofrece activar el segundo factor. DOM:\n{}",
        nav.dom()
    );

    nav.texto(
        "document.querySelector('form[action=\"/panel/segundo-factor/empezar\"]').submit(); ''",
    );
    nav.esperar(
        "document.querySelector('code.secreto') !== null",
        "no se llegó a la pantalla del QR",
    );

    // 2. **EL QR SE DIBUJA.** No que el SVG esté en el HTML: que el navegador lo
    //    haya PINTADO. Un `<svg>` con el MIME equivocado o mal cerrado se queda
    //    en el DOM y no ocupa un pixel, y eso es indistinguible de un QR bueno
    //    mirando la respuesta.
    let ancho = nav.texto(
        "String(document.querySelector('.qr svg')?.getBoundingClientRect().width ?? 0)",
    );
    let ancho: f64 = ancho.parse().unwrap_or(0.0);
    assert!(
        ancho > 50.0,
        "el QR no se pintó (ancho {ancho}px). Un SVG en el DOM que el navegador \
         no dibuja se ve igual que uno bueno en la respuesta"
    );

    // 3. El secreto que la pantalla ofrece para teclear a mano es el que sirve.
    //    De aquí sale el código, así que si el URI del QR y este secreto
    //    divergieran, el paso siguiente fallaría.
    let secreto = nav.texto("document.querySelector('code.secreto').textContent.trim()");
    assert!(!secreto.is_empty(), "no se enseñó el secreto para enrolar a mano");

    let bytes = Secret::Encoded(secreto).to_bytes().expect("el secreto es base32");
    let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes).expect("TOTP válido");
    let ahora = unix();
    // La ventana en la que se confirma. Se guarda porque su código QUEDA
    // QUEMADO: el antirreplay no lo deja volver a valer, ni siquiera al dueño.
    let ventana_confirmada = ahora / 30;

    nav.texto(&format!(
        "document.querySelector('input[name=codigo]').value = {};\
         document.querySelector('form[action=\"/panel/segundo-factor/confirmar\"]').submit(); ''",
        serde_json::json!(totp.generate(ahora))
    ));
    nav.esperar(
        "document.querySelector('ul.codigos') !== null",
        "no se llegó a la pantalla de los códigos de recuperación",
    );

    // 4. Los DIEZ códigos, dibujados y visibles. Una lista en el DOM con
    //    `display:none` sería una entrega que nadie recibe.
    let cuantos = nav.texto("String(document.querySelectorAll('ul.codigos li code').length)");
    assert_eq!(cuantos, "10", "no se dibujaron los diez códigos de recuperación");
    let primero = nav.texto(
        "document.querySelector('ul.codigos li code').textContent.trim()",
    );
    assert!(primero.contains('-') && primero.len() == 17, "código raro: {primero:?}");

    // 5. **Y AHORA LA MITAD QUE IMPORTA**: salir y volver a entrar. Con la
    //    contraseña sola ya no se llega al panel.
    nav.ir(&format!("{base}/panel"));
    nav.texto("document.querySelector('form[action=\"/panel/salir\"]').submit(); ''");
    nav.esperar("location.pathname === '/panel/entrada'", "no se salió de la sesión");

    nav.entrar(&base, ENTRADA, CORREO, CLAVE);
    assert_eq!(
        nav.texto("location.pathname"),
        "/panel/entrada/codigo",
        "la contraseña sola llevó al panel: el segundo factor no se está pidiendo"
    );
    assert!(
        nav.evaluar("!!document.querySelector('input[name=codigo]')") == serde_json::json!(true),
        "el segundo paso no se dibujó. DOM:\n{}",
        nav.dom()
    );

    // 6. El código del mismo secreto abre la sesión de verdad.
    //
    //    **Se espera a que el autenticador cambie de código**, y no es una
    //    comodidad del banco: el del enrolamiento se quemó al confirmar, y el
    //    antirreplay no lo deja volver a valer ni siquiera al dueño. La primera
    //    versión de esta prueba generaba el código otra vez sin esperar, caía en
    //    la MISMA ventana de 30 s y el servidor lo rechazaba — con razón. Que
    //    haya que esperar aquí es la prueba de que el antirreplay está puesto en
    //    el camino real, no solo en su banco de unidad.
    while unix() / 30 <= ventana_confirmada {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let ahora = unix();
    nav.texto(&format!(
        "document.querySelector('input[name=codigo]').value = {};\
         document.querySelector('form').submit(); ''",
        serde_json::json!(totp.generate(ahora))
    ));
    nav.esperar(
        "location.pathname === '/panel'",
        "el código correcto no abrió el panel",
    );
    assert!(
        nav.evaluar("document.documentElement.dataset.boi === 'listo'") == serde_json::json!(true),
        "se entró pero el panel no terminó de cargar"
    );
}

/// **LA CASILLA DE PROCEDENCIA DE LA FÓRMULA SE DIBUJA, Y LO QUE MARCA LLEGA A
/// LA BASE** (#84).
///
/// Es la mitad que le faltaba a esta característica desde que existe. La
/// procedencia tenía banco de dominio (`procedencia.rs`), banco de fusión
/// (`historias.rs`) y banco de auditoría (`recolector.rs`), y los tres van de
/// estructura a estructura: miden el códec. **Ninguno dibujaba un control.** Un
/// `input` con el `name` mal escrito, dentro de un `@if f.editable` que no se
/// cumple, o fuera del `<form>` que se envía, devuelve el mismo 200 con el mismo
/// cuerpo y deja la procedencia vacía para siempre — y como «vacía» es
/// exactamente lo que significa «nadie lo afirmó», el fallo no se distingue del
/// funcionamiento normal (directiva 30).
///
/// # Las dos listas (directiva 33)
///
/// Se crean DOS fórmulas por el mismo camino y en la misma sesión: una con la
/// casilla marcada y otra sin marcar. La primera tiene que llegar con su
/// procedencia; la segunda, con `null`. Con solo la primera pasaría igual un
/// servidor que escribiera «confirmado» siempre —que es justo el valor por
/// defecto que #84 existe para desterrar—; con solo la segunda, uno que no
/// escribiera nunca.
///
/// El paciente y la historia se crean por la API del propio producto y con la
/// MISMA cookie de sesión del navegador (las dos superficies comparten
/// `usuario_de`), no con `sqlx` a mano: sembrar el estado por fuera mediría el
/// arnés.
#[test]
fn la_casilla_de_procedencia_de_la_formula_se_dibuja_y_lo_marcado_llega_a_la_base() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);
    nav.entrar(&base, ENTRADA, CORREO, CLAVE);

    // 1. Un paciente y su historia, por la API del producto.
    nav.evaluar(
        r#"window.__historia = null;
           (async () => {
             const yo = await (await fetch('/api/yo')).json();
             const p = await fetch('/api/pacientes', {
               method: 'POST', headers: {'Content-Type': 'application/json'},
               body: JSON.stringify({tipo_documento:'CC', numero_documento:'80808080',
                 nombre1:'Ana', apellido1:'Ruiz', fecha_nacimiento:'1990-01-01', sexo:'F'})});
             if (!p.ok) { window.__historia = 'paciente ' + p.status + ': ' + await p.text(); return; }
             const pj = await p.json();
             const h = await fetch('/api/historias', {
               method: 'POST', headers: {'Content-Type': 'application/json'},
               body: JSON.stringify({paciente_id: pj.id, medico_id: yo.id, motivo_consulta:'tos',
                 enfermedad_actual:'tres días', diagnostico_principal:'J00',
                 plan_tratamiento:'reposo'})});
             if (!h.ok) { window.__historia = 'historia ' + h.status + ': ' + await h.text(); return; }
             const hj = await h.json();
             window.__historia = hj.id;
           })();"#,
    );
    nav.esperar(
        "window.__historia !== null",
        "no se pudo crear la historia por la API con la cookie del navegador",
    );
    let crudo = nav.texto("String(window.__historia)");
    let historia: i64 = crudo
        .parse()
        .unwrap_or_else(|_| panic!("no se creó la historia: {crudo}"));

    // 2. La pantalla de fórmulas, DIBUJADA. El control tiene que existir y estar
    //    DENTRO del formulario que se envía: fuera de él, el navegador no lo manda
    //    y el servidor recibe exactamente lo mismo que si nadie lo hubiera marcado.
    nav.ir(&format!("{base}/panel/historias/{historia}/formulas"));
    assert_eq!(
        nav.evaluar(
            "(() => { const c = document.querySelector('input[name=procedencia_formula]');\
              return !!c && !!c.form && c.form.getAttribute('action').endsWith('/formulas'); })()"
        ),
        serde_json::json!(true),
        "la casilla de procedencia no está dibujada dentro del formulario de la fórmula:\n{}",
        nav.dom()
    );

    // 3. Con la casilla MARCADA.
    let rellenar = |marcar: bool, cum: &str| {
        format!(
            "document.querySelector('input[name=medicamento]').value = 'Acetaminofén 500 mg';\
             document.querySelector('input[name=codigo_cum]').value = '{cum}';\
             document.querySelector('input[name=dosis]').value = '1 cada 8 horas';\
             document.querySelector('input[name=cantidad]').value = '30';\
             document.querySelector('input[name=dias_tratamiento]').value = '10';\
             document.querySelector('input[name=procedencia_formula]').checked = {marcar};\
             document.querySelector('input[name=procedencia_formula]').form.submit(); ''"
        )
    };
    // LA ESPERA CUENTA FILAS, no mira si la casilla existe.
    //
    // Esperar `!!input[name=procedencia_formula]` se cumplía contra el documento
    // VIEJO —que también la tiene— así que seguía adelante antes de que la
    // navegación terminara, y el relleno siguiente corría sobre una página a
    // medio cargar: `document.querySelector('input[name=medicamento]')` en
    // `null`. La prueba fallaba una de cada varias veces, que es la peor clase
    // de prueba: la que enseña a re-correr en vez de a mirar.
    //
    // El número de filas SÍ distingue la página nueva de la vieja.
    let filas_visibles =
        "document.querySelectorAll('table tbody tr').length";
    nav.texto(&rellenar(true, "19900001-1"));
    nav.esperar(
        &format!("{filas_visibles} === 1"),
        "tras añadir la primera fórmula, la pantalla no la lista",
    );

    // 4. Y la segunda SIN marcar, por el mismo camino.
    nav.texto(&rellenar(false, "19900002-2"));
    nav.esperar(
        &format!("{filas_visibles} === 2"),
        "tras añadir la segunda fórmula, la pantalla no la lista",
    );

    // 5. Lo que quedó en la base, leído por la API.
    nav.evaluar(&format!(
        "window.__filas = null;\
         fetch('/api/historias/{historia}/formulas')\
           .then(r => r.json())\
           .then(j => window.__filas = JSON.stringify(\
             j.map(f => [f.codigo_cum, f.procedencias])));"
    ));
    nav.esperar("window.__filas !== null", "no se pudieron releer las fórmulas");
    let filas = nav.texto("window.__filas");
    let filas: Vec<(Option<String>, Option<String>)> =
        serde_json::from_str(&filas).expect("las fórmulas");
    assert_eq!(filas.len(), 2, "se esperaban las dos fórmulas: {filas:?}");

    let marcada = &filas[0].1;
    let sin_marcar = &filas[1].1;
    let marcada = marcada.as_deref().unwrap_or_else(|| {
        panic!("la fórmula con la casilla MARCADA llegó sin procedencia: {filas:?}")
    });
    for campo in ["codigo_cum", "cantidad", "dias_tratamiento"] {
        assert!(
            marcada.contains(&format!("\"{campo}\":\"confirmado\"")),
            "falta `{campo}` en la procedencia de la fórmula confirmada: {marcada}"
        );
    }
    assert_eq!(
        sin_marcar.as_deref(),
        Some("{}"),
        "sin marcar la casilla NADIE afirmó nada: la columna lleva `{{}}` —que MANTIENE \
         la fórmula dentro de la auditoría— y ningún «confirmado». Esta línea esperaba \
         `null`, y con `null` el recolector saltaba el medicamento entero: la prueba \
         daba por correcto justo el estado que dejaba el mecanismo inerte ({filas:?})"
    );
}


/// **EL CUERPO JSON NO PUEDE FABRICAR PROCEDENCIA, NI APAGAR LA AUDITORÍA** (#84).
///
/// Lo destapó la revisión de la directiva 25 sobre el cambio de la fórmula: allí
/// la puerta estaba cerrada con un mapa tipado, y en `historia` —el portador
/// ORIGINAL, vivo desde la migración 0005 y el que alimenta el AC— `procedencias`
/// era una cadena LIBRE del cuerpo enchufada directa al `.bind`.
///
/// Dos caminos, y el segundo es el que ninguna pantalla puede producir:
///   · certificar en bloque como «confirmado» lo que nadie miró;
///   · mandar un JSON ilegible, que dejaba la historia **exenta de toda la
///     auditoría**, sin un mensaje y sin diferencia observable frente a una
///     historia limpia.
///
/// Se prueba por HTTP y no en unidad porque lo que se afirma es del serde de la
/// ruta: `#[serde(skip_deserializing)]` no se ve en ninguna firma, y quitarlo
/// devuelve un binario que compila igual y responde igual.
///
/// Con su pareja: el panel SÍ escribe procedencia por su camino, así que la
/// puerta cerrada no es una puerta tapiada.
#[test]
fn el_cuerpo_json_no_escribe_la_procedencia_de_la_historia() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);
    nav.entrar(&base, ENTRADA, CORREO, CLAVE);

    // Dos historias por la API: una intentando FABRICAR el aval, otra intentando
    // APAGAR la auditoría con un JSON que no parsea.
    nav.evaluar(
        r#"window.__ids = null;
           (async () => {
             const yo = await (await fetch('/api/yo')).json();
             const p = await fetch('/api/pacientes', {
               method: 'POST', headers: {'Content-Type': 'application/json'},
               body: JSON.stringify({tipo_documento:'CC', numero_documento:'90909090',
                 nombre1:'Beto', apellido1:'Paz', fecha_nacimiento:'1985-01-01', sexo:'M'})});
             const pj = await p.json();
             const crear = async (proc) => {
               const r = await fetch('/api/historias', {
                 method: 'POST', headers: {'Content-Type': 'application/json'},
                 body: JSON.stringify({paciente_id: pj.id, medico_id: yo.id,
                   motivo_consulta:'x', enfermedad_actual:'y',
                   diagnostico_principal:'J00', plan_tratamiento:'z',
                   procedencias: proc})});
               return (await r.json()).id;
             };
             const falsa = await crear('{"diagnostico_principal":"confirmado","finalidad":"confirmado"}');
             const rota  = await crear('{roto');
             window.__ids = JSON.stringify([falsa, rota]);
           })();"#,
    );
    nav.esperar("window.__ids !== null", "no se crearon las historias por la API");
    let ids: Vec<i64> = serde_json::from_str(&nav.texto("window.__ids")).expect("los ids");
    assert_eq!(ids.len(), 2);

    // Lo que quedó guardado. Se lee por la API, que devuelve la historia entera.
    for (id, que) in [(ids[0], "el aval fabricado"), (ids[1], "el JSON ilegible")] {
        nav.evaluar(&format!(
            "window.__p = null;             fetch('/api/historias/{id}').then(r => r.json())               .then(j => window.__p = JSON.stringify(j.procedencias ?? null));"
        ));
        nav.esperar("window.__p !== null", "no se pudo releer la historia");
        let p = nav.texto("window.__p");
        // `{}` y NO `null`, y esta línea decía `null`: codificaba el defecto como
        // correcto. Lo cazaron los tres revisores de la directiva 25 sobre
        // `9f8c147d` — «hubo captura y nadie afirmó nada» es `{}`; `NULL` es
        // «fila anterior a la captura» y EXIME el registro de toda la auditoría.
        // Una historia creada por la API nacía exenta para siempre.
        assert_eq!(
            p, "\"{}\"",
            "{que}: la API no puede afirmar, pero tampoco EXIMIR — la columna \
             tiene que quedar en `{{}}`, nunca en `null` ({p})"
        );
    }

    // LA PAREJA: por el panel sí se escribe. Sin esto, un campo simplemente roto
    // —que nunca guardara nada— pasaría esta prueba con nota.
    nav.evaluar(&format!(
        "window.__listo = null;         fetch('/panel/historias/{}/diagnostico', {{           method: 'POST',           headers: {{'Content-Type': 'application/x-www-form-urlencoded'}},           body: 'diagnostico_principal=J00&procedencia_diagnostico_principal=confirmado'         }}).then(() => window.__listo = 1);",
        ids[0]
    ));
    nav.esperar("window.__listo !== null", "el panel no respondió al guardar el diagnóstico");
    nav.evaluar(&format!(
        "window.__q = null;         fetch('/api/historias/{}').then(r => r.json())           .then(j => window.__q = JSON.stringify(j.procedencias ?? null));",
        ids[0]
    ));
    nav.esperar("window.__q !== null", "no se pudo releer la historia tras el panel");
    let q = nav.texto("window.__q");
    assert!(
        q.contains("confirmado"),
        "el panel tiene que poder escribir la procedencia; si no, esto no mide una \
         puerta cerrada sino una tapiada ({q})"
    );
}


/// **UNA CANTIDAD QUE NO ES UN NÚMERO SE RECHAZA; NO SE GUARDA COMO CERO** (#84).
///
/// Lo trajo la revisión de la directiva 25. El manejador hacía
/// `decimal(...).unwrap_or(0.0)` —relleno de manual (directiva 20)— mientras la
/// procedencia se decidía sobre el TEXTO del formulario: `cantidad=abc` con la
/// casilla marcada guardaba una cantidad **cero**, la mandaba al AM y la
/// certificaba como afirmada por un humano. Y desde que `cantidad_entera` llama a
/// `rec.presente`, además salía de la auditoría como limpia.
///
/// El navegador no deja llegar ahí (`type=number required`), así que la petición
/// se hace a mano desde la MISMA sesión: el camino que hay que cerrar no es la
/// pantalla, es el manejador.
///
/// Su pareja va dentro: la misma petición con una cantidad buena SÍ crea la
/// fórmula. Sin ella, un manejador que rechazara todo pasaría igual.
#[test]
fn la_cantidad_que_no_es_numero_no_se_guarda_como_cero() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);
    nav.entrar(&base, ENTRADA, CORREO, CLAVE);

    nav.evaluar(
        r#"window.__historia = null;
           (async () => {
             const yo = await (await fetch('/api/yo')).json();
             const p = await fetch('/api/pacientes', {
               method: 'POST', headers: {'Content-Type': 'application/json'},
               body: JSON.stringify({tipo_documento:'CC', numero_documento:'70707070',
                 nombre1:'Cira', apellido1:'Mora', fecha_nacimiento:'1975-01-01', sexo:'F'})});
             const pj = await p.json();
             const h = await fetch('/api/historias', {
               method: 'POST', headers: {'Content-Type': 'application/json'},
               body: JSON.stringify({paciente_id: pj.id, medico_id: yo.id,
                 motivo_consulta:'x', enfermedad_actual:'y',
                 diagnostico_principal:'J00', plan_tratamiento:'z'})});
             window.__historia = (await h.json()).id;
           })();"#,
    );
    nav.esperar("window.__historia !== null", "no se pudo crear la historia");
    let historia: i64 = nav.texto("String(window.__historia)").parse().expect("id");

    let enviar = |nav: &mut Navegador, cantidad: &str| {
        nav.evaluar(&format!(
            "window.__ok = null;             fetch('/panel/historias/{historia}/formulas', {{               method: 'POST',               headers: {{'Content-Type': 'application/x-www-form-urlencoded'}},               body: 'medicamento=Acetamin&codigo_cum=19900001-1&dosis=1&cantidad={cantidad}&procedencia_formula=confirmado'             }}).then(() => window.__ok = 1);"
        ));
        nav.esperar("window.__ok !== null", "el panel no respondió");
    };

    enviar(&mut nav, "abc");
    nav.evaluar(&format!(
        "window.__f = null;         fetch('/api/historias/{historia}/formulas').then(r => r.json())           .then(j => window.__f = JSON.stringify(j.map(x => [x.cantidad, x.procedencias])));"
    ));
    nav.esperar("window.__f !== null", "no se pudieron releer las fórmulas");
    assert_eq!(
        nav.texto("window.__f"),
        "[]",
        "una cantidad ilegible no puede crear una fórmula: se guardaba como 0.0 y \
         encima certificada"
    );

    // LA PAREJA: con una cantidad buena sí entra, y con su procedencia.
    enviar(&mut nav, "30");
    nav.evaluar(&format!(
        "window.__g = null;         fetch('/api/historias/{historia}/formulas').then(r => r.json())           .then(j => window.__g = JSON.stringify(j.map(x => [x.cantidad, x.procedencias])));"
    ));
    nav.esperar("window.__g !== null", "no se pudieron releer las fórmulas");
    let g = nav.texto("window.__g");
    assert!(g.contains("30"), "la cantidad buena tiene que entrar: {g}");
    assert!(
        g.contains("\\\"cantidad\\\":\\\"confirmado\\\""),
        "y con su procedencia: {g}"
    );
}

/// **UN `PUT` NO PUEDE BORRAR LO QUE EL PANEL CONFIRMÓ**, y su pareja: el panel sí
/// puede escribirlo.
///
/// Es el caso rojo que faltaba, y su ausencia dejó pasar el hallazgo mayor de la
/// revisión de la directiva 25 sobre `9f8c147d` — los TRES revisores lo
/// encontraron por separado, con confianza 8, 9 y 9.
///
/// El banco medía que el cuerpo JSON no ESCRIBIERA la procedencia
/// (`el_cuerpo_json_no_escribe_la_procedencia_de_la_historia`, arriba) y **nadie
/// medía que no la BORRARA**. Son las dos mitades de la directiva 32: `{}` y
/// `NULL` no significan lo mismo, y `NULL` exime el registro de toda la auditoría
/// de #84. Con `#[serde(skip_deserializing)]`, `HistoriaNueva.procedencias` llega
/// `None` en toda petición JSON, así que un `PUT` con el MISMO cuerpo que devuelve
/// el `GET` dejaba la columna en `NULL`, respondía 200, y sacaba la consulta del
/// cierre de mes sin cambiar un byte del RIPS ni dejar una anotación.
///
/// La prueba habla por el camino REAL —fetch desde la misma sesión, como una
/// integración cualquiera— y no por el manejador a pelo (directiva 28).
#[test]
fn un_put_de_la_api_no_borra_la_procedencia_que_el_panel_confirmo() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);
    nav.entrar(&base, ENTRADA, CORREO, CLAVE);

    nav.evaluar(
        r#"window.__h = null;
           (async () => {
             const yo = await (await fetch('/api/yo')).json();
             const p = await (await fetch('/api/pacientes', {
               method: 'POST', headers: {'Content-Type': 'application/json'},
               body: JSON.stringify({tipo_documento:'CC', numero_documento:'80808080',
                 nombre1:'Aura', apellido1:'Vega', fecha_nacimiento:'1980-03-03', sexo:'F'})})).json();
             const h = await (await fetch('/api/historias', {
               method: 'POST', headers: {'Content-Type': 'application/json'},
               body: JSON.stringify({paciente_id: p.id, medico_id: yo.id,
                 motivo_consulta:'x', enfermedad_actual:'y',
                 diagnostico_principal:'J00', plan_tratamiento:'z'})})).json();
             window.__h = h.id;
           })();"#,
    );
    nav.esperar("window.__h !== null", "no se pudo crear la historia");
    let h: i64 = nav.texto("String(window.__h)").parse().expect("id");

    // LA PAREJA, primero: por el panel SÍ se confirma. Sin esto, un sistema que
    // nunca escribiera nada pasaría la prueba de abajo con nota.
    nav.evaluar(&format!(
        "window.__ok = null;         fetch('/panel/historias/{h}/diagnostico', {{           method: 'POST',           headers: {{'Content-Type': 'application/x-www-form-urlencoded'}},           body: 'diagnostico_principal=J00&procedencia_diagnostico_principal=confirmado'         }}).then(() => window.__ok = 1);"
    ));
    nav.esperar("window.__ok !== null", "el panel no respondió");

    let leer = |nav: &mut Navegador| -> String {
        nav.evaluar(&format!(
            "window.__p = null;             fetch('/api/historias/{h}').then(r => r.json())               .then(j => window.__p = JSON.stringify(j.procedencias ?? null));"
        ));
        nav.esperar("window.__p !== null", "no se pudo releer la historia");
        nav.texto("window.__p")
    };

    let confirmado = leer(&mut nav);
    assert!(
        confirmado.contains("diagnostico_principal") && confirmado.contains("confirmado"),
        "el panel tiene que dejar el aval escrito: {confirmado}"
    );

    // Y AHORA EL ATAQUE: el viaje de ida y vuelta más inocente que existe — leer
    // por la API y devolver el MISMO cuerpo, sin tocar un solo valor clínico.
    nav.evaluar(&format!(
        "window.__put = null;         fetch('/api/historias/{h}').then(r => r.json()).then(j =>           fetch('/api/historias/{h}', {{             method: 'PUT',             headers: {{'Content-Type': 'application/json'}},             body: JSON.stringify(j)           }})).then(r => window.__put = r.status);"
    ));
    nav.esperar("window.__put !== null", "el PUT no respondió");

    let despues = leer(&mut nav);
    assert_ne!(
        despues, "null",
        "un PUT dejó la columna en NULL y con eso sacó la consulta de la auditoría \
         entera: es el defecto que #84 existe para impedir"
    );
    assert_eq!(
        despues, "\"{}\"",
        "la API no puede afirmar, así que lo que toca pierde su aval y se audita: \
         `{{}}`. Conservar el mapa sería PEOR — dejaría un «confirmado» pegado a un \
         valor que pudo cambiar en el mismo PUT. Quedó: {despues}"
    );
}

/// La gemela de la de arriba, para la FÓRMULA: un `PUT /api/formulas/{id}` no
/// puede borrar el aval que el panel escribió.
///
/// Existía la mitad de historia y no la de fórmula, y los tres revisores de la
/// directiva 25 sobre `9f8c147d` señalaron el mismo hueco: los dos
/// `&Default::default()` de `rutas.rs` son la ÚNICA declaración de política sobre
/// qué escribe la API JSON en esa columna, y no los medía nada. El camino de
/// regresión no necesita malicia — basta que alguien añada `procedencias` a
/// `FormulaNueva`, o que `json_de` vuelva a devolver `None` con el mapa vacío
/// (que es literalmente lo que hacía la primera versión de esta rama), y todo
/// sigue verde.
#[test]
fn un_put_de_la_api_no_borra_la_procedencia_de_la_formula() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);
    nav.entrar(&base, ENTRADA, CORREO, CLAVE);

    nav.evaluar(
        r#"window.__h = null;
           (async () => {
             const yo = await (await fetch('/api/yo')).json();
             const p = await (await fetch('/api/pacientes', {
               method: 'POST', headers: {'Content-Type': 'application/json'},
               body: JSON.stringify({tipo_documento:'CC', numero_documento:'90909090',
                 nombre1:'Blas', apellido1:'Nieto', fecha_nacimiento:'1990-09-09', sexo:'M'})})).json();
             const h = await (await fetch('/api/historias', {
               method: 'POST', headers: {'Content-Type': 'application/json'},
               body: JSON.stringify({paciente_id: p.id, medico_id: yo.id,
                 motivo_consulta:'x', enfermedad_actual:'y',
                 diagnostico_principal:'J00', plan_tratamiento:'z'})})).json();
             window.__h = h.id;
           })();"#,
    );
    nav.esperar("window.__h !== null", "no se pudo crear la historia");
    let h: i64 = nav.texto("String(window.__h)").parse().expect("id");

    // LA PAREJA: por el panel, con la casilla marcada, el aval SÍ se escribe.
    nav.evaluar(&format!(
        "window.__ok = null;         fetch('/panel/historias/{h}/formulas', {{           method: 'POST',           headers: {{'Content-Type': 'application/x-www-form-urlencoded'}},           body: 'medicamento=Ibuprofeno&codigo_cum=19900001-1&dosis=1&cantidad=20&procedencia_formula=confirmado'         }}).then(() => window.__ok = 1);"
    ));
    nav.esperar("window.__ok !== null", "el panel no respondió");

    let leer = |nav: &mut Navegador| -> String {
        nav.evaluar(&format!(
            "window.__f = null;             fetch('/api/historias/{h}/formulas').then(r => r.json())               .then(j => window.__f = JSON.stringify(j.map(x => x.procedencias ?? null)));"
        ));
        nav.esperar("window.__f !== null", "no se pudieron releer las fórmulas");
        nav.texto("window.__f")
    };

    let antes = leer(&mut nav);
    assert!(
        antes.contains("confirmado"),
        "el panel tiene que dejar el aval escrito: {antes}"
    );

    // EL ATAQUE: leer la fórmula por la API y devolverla igual.
    nav.evaluar(&format!(
        "window.__put = null;         fetch('/api/historias/{h}/formulas').then(r => r.json()).then(j =>           fetch('/api/formulas/' + j[0].id, {{             method: 'PUT',             headers: {{'Content-Type': 'application/json'}},             body: JSON.stringify(j[0])           }})).then(r => window.__put = r.status);"
    ));
    nav.esperar("window.__put !== null", "el PUT no respondió");

    let despues = leer(&mut nav);
    assert!(
        !despues.contains("null"),
        "un PUT dejó la columna en NULL y sacó la fórmula de la auditoría: {despues}"
    );
    assert!(
        despues.contains("{}") && !despues.contains("confirmado"),
        "la API no puede afirmar ni conservar un aval sobre valores que ese mismo \
         PUT pudo cambiar: la columna queda `{{}}` y la auditoría lo marca. Quedó: {despues}"
    );
}

/// **EL ALTA DE PACIENTE SE DIBUJA Y LA DEMOGRAFÍA DEL RDA LLEGA A LA BASE** (#84).
///
/// Hasta el 2026-08-22 el paciente solo nacía por la API; esta es la pantalla que
/// lo captura. Se prueba con NAVEGADOR porque una pantalla se dibuja (directiva
/// 28): el `<select>` de catálogo lo puebla el servidor, y afirmar sobre el HTML
/// crudo no diría si el navegador lo presentó ni si el envío del formulario llega.
///
/// La aguja es el DISPLAY OFICIAL derivado del código (`Indigena`, no `1`): sale
/// del CodeSystem vendorizado, no de esta prueba (directiva 33). El control de que
/// el catálogo cargó de verdad es que la opción `99`/`Ninguna…` existe — si el
/// `include_str!` no cargara, el selector saldría vacío y no habría ni `99` ni
/// `Indigena` que mostrar.
#[test]
fn el_alta_de_paciente_captura_la_demografia_del_rda() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);
    nav.entrar(&base, ENTRADA, CORREO, CLAVE);

    // 1. El formulario, DIBUJADO, con los catálogos servidos del CodeSystem.
    nav.ir(&format!("{base}/panel/pacientes/nuevo"));
    // El tipo de documento admite los de USUARIO: TI (un menor) tiene que estar,
    // aunque no sea de quien atiende. Es lo que distingue esta lista de la del
    // profesional.
    assert_eq!(
        nav.evaluar("!!document.querySelector('select[name=tipo_documento] option[value=TI]')"),
        serde_json::json!(true),
        "el selector de tipo de documento no ofrece TI (los de usuario):\n{}",
        nav.dom()
    );
    // La etnia trae el 99 con su display del catálogo — control de que cargó.
    assert_eq!(
        nav.evaluar(
            "(() => { const o = document.querySelector('select[name=pertenencia_etnica] option[value=\"99\"]'); \
              return !!o && o.textContent.includes('Ninguna'); })()"
        ),
        serde_json::json!(true),
        "el selector de etnia no salió del catálogo (falta el 99):\n{}",
        nav.dom()
    );

    // 2. Se rellena y se envía por el PROPIO formulario (no por la API).
    nav.texto(
        "document.querySelector('select[name=tipo_documento]').value='CC';\
         document.querySelector('input[name=numero_documento]').value='71717171';\
         document.querySelector('input[name=nombre1]').value='Luisa';\
         document.querySelector('input[name=apellido1]').value='Mesa';\
         document.querySelector('input[name=fecha_nacimiento]').value='1994-06-02';\
         document.querySelector('select[name=sexo]').value='F';\
         document.querySelector('input[name=nacionalidad]').value='170';\
         document.querySelector('select[name=pertenencia_etnica]').value='1';\
         document.querySelector('input[name=comunidad_etnica]').value='Emberá';\
         document.querySelector('select[name=discapacidad]').value='02';\
         document.querySelector('select[name=identidad_genero]').value='03';\
         // Se envía DESDE un campo del formulario (`.form`), no `querySelector('form')`:
         // el primer <form> de la página es el de cerrar sesión del menú.
         document.querySelector('input[name=numero_documento]').form.submit(); ''",
    );

    // 3. Acaba en la FICHA, que muestra el DISPLAY oficial, no el código crudo.
    nav.esperar(
        "location.pathname.match(/^\\/panel\\/pacientes\\/\\d+$/) !== null",
        "el alta no llevó a la ficha del paciente nuevo",
    );
    let dom = nav.dom();
    assert!(dom.contains("Indigena"), "la ficha no muestra el display de la etnia:\n{dom}");
    assert!(dom.contains("Emberá"), "la ficha no muestra la comunidad étnica");
    assert!(dom.contains("Discapacidad visual"), "la ficha no muestra el display de la discapacidad");
    assert!(dom.contains("Transgénero"), "la ficha no muestra el display de la identidad de género");

    // 4. En la base: la demografía, con procedencia CONFIRMADO (la tecleó una
    //    persona), releída por la API.
    let id = nav.texto("location.pathname.split('/').pop()");
    nav.evaluar(&format!(
        "window.__p = null;\
         fetch('/api/pacientes/{id}').then(r => r.json()).then(j => window.__p = JSON.stringify(j));"
    ));
    nav.esperar("window.__p !== null", "no se pudo releer el paciente");
    let p: serde_json::Value = serde_json::from_str(&nav.texto("window.__p")).expect("paciente");
    assert_eq!(p["pertenencia_etnica"], "1");
    assert_eq!(p["discapacidad"], "02");
    assert_eq!(p["identidad_genero"], "03");
    assert_eq!(p["comunidad_etnica"], "Emberá");
    assert_eq!(p["nacionalidad"], "170");
    let proc = p["procedencias"].as_str().unwrap_or("");
    for campo in ["pertenencia_etnica", "discapacidad", "identidad_genero", "comunidad_etnica", "nacionalidad"] {
        assert!(
            proc.contains(&format!("\"{campo}\":\"confirmado\"")),
            "falta la procedencia `confirmado` de {campo}: {proc}"
        );
    }
}

/// **LA PAREJA (directiva 33): sin un campo obligatorio, NO crea y lo dice.**
///
/// El `required` del HTML lo obedece un navegador y lo ignora cualquier cliente
/// que hable HTTP, así que la validación es del SERVIDOR. Se envía el formulario
/// con `form.submit()` —que se salta la validación del navegador— sin sexo, y
/// tiene que volver el formulario con el aviso y NO haberse creado nadie. Sin
/// esta prueba, un servidor que aceptara el alta a medias pasaría igual de verde.
#[test]
fn el_alta_sin_un_campo_obligatorio_no_crea_y_avisa() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);
    nav.entrar(&base, ENTRADA, CORREO, CLAVE);

    // Cuántos pacientes hay ANTES (control: al final tienen que ser los mismos).
    nav.evaluar(
        "window.__n = null; fetch('/api/pacientes').then(r => r.json()).then(j => window.__n = j.length);",
    );
    nav.esperar("window.__n !== null", "no se pudo contar el padrón");
    let antes: i64 = nav.texto("String(window.__n)").parse().expect("cuenta");

    nav.ir(&format!("{base}/panel/pacientes/nuevo"));
    // Todo menos el sexo, y se envía saltándose la validación del navegador.
    nav.texto(
        "document.querySelector('select[name=tipo_documento]').value='CC';\
         document.querySelector('input[name=numero_documento]').value='90909090';\
         document.querySelector('input[name=nombre1]').value='Sin';\
         document.querySelector('input[name=apellido1]').value='Sexo';\
         document.querySelector('input[name=fecha_nacimiento]').value='2000-01-01';\
         document.querySelector('select[name=sexo]').value='';\
         document.querySelector('input[name=numero_documento]').form.submit(); ''",
    );
    // Vuelve al formulario (sigue en /nuevo) con el aviso; no redirige a la ficha.
    nav.esperar(
        "document.body && document.body.textContent.includes('Falta el sexo')",
        "el servidor no rechazó el alta sin sexo",
    );
    assert_eq!(
        nav.evaluar("location.pathname"),
        serde_json::json!("/panel/pacientes/nuevo"),
        "tras el error el formulario debería seguir en /nuevo, no haber avanzado"
    );

    // Y el control: NO se creó nadie.
    nav.evaluar(
        "window.__m = null; fetch('/api/pacientes').then(r => r.json()).then(j => window.__m = j.length);",
    );
    nav.esperar("window.__m !== null", "no se pudo recontar el padrón");
    let despues: i64 = nav.texto("String(window.__m)").parse().expect("cuenta");
    assert_eq!(despues, antes, "un alta rechazada creó un paciente igual");
}

/// **EDITAR NO BORRA LO QUE LA PANTALLA NO CAPTURA** (#84, directiva 20).
///
/// `paciente::actualizar` reescribe TODAS las columnas, y la pantalla de edición
/// no captura `cod_pais`, `tipo_usuario` ni `zona`. Si el formulario los mandara
/// en `None`, editar el celular BORRARÍA el país del paciente —pérdida de dato
/// silenciosa, del tipo que no se ve hasta el cierre de mes—. La corrección es
/// que `guardar` los conserva del registro existente; esto lo ata.
///
/// Su pareja es implícita y real: si `a_nuevo` mandara `None` en vez de
/// conservar, esta prueba se pone roja exactamente en la línea del `cod_pais`.
#[test]
fn editar_un_paciente_no_borra_los_campos_que_la_pantalla_no_captura() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);
    nav.entrar(&base, ENTRADA, CORREO, CLAVE);

    // 1. Un paciente con campos que la pantalla NO captura, creado por la API.
    nav.evaluar(
        r#"window.__id = null;
           fetch('/api/pacientes', {
             method: 'POST', headers: {'Content-Type': 'application/json'},
             body: JSON.stringify({tipo_documento:'CC', numero_documento:'60606060',
               nombre1:'Cesar', apellido1:'Nieto', fecha_nacimiento:'1980-03-03', sexo:'M',
               cod_pais:'170', tipo_usuario:'02', zona:'U'})})
             .then(r => r.json()).then(j => window.__id = j.id);"#,
    );
    nav.esperar("window.__id !== null", "no se pudo crear el paciente base");
    let id = nav.texto("String(window.__id)");

    // 2. Se edita por el PANEL cambiando solo el celular.
    nav.ir(&format!("{base}/panel/pacientes/{id}/editar"));
    nav.esperar(
        "!!document.querySelector('input[name=celular]')",
        "no se dibujó el formulario de edición",
    );
    nav.texto(
        "document.querySelector('input[name=celular]').value='3001112233';\
         document.querySelector('input[name=celular]').form.submit(); ''",
    );
    nav.esperar(
        &format!("location.pathname === '/panel/pacientes/{id}'"),
        "la edición no volvió a la ficha",
    );

    // 3. Los campos NO capturados sobrevivieron; el celular sí cambió.
    nav.evaluar(&format!(
        "window.__p = null;\
         fetch('/api/pacientes/{id}').then(r => r.json()).then(j => window.__p = JSON.stringify(j));"
    ));
    nav.esperar("window.__p !== null", "no se pudo releer el paciente editado");
    let p: serde_json::Value = serde_json::from_str(&nav.texto("window.__p")).expect("paciente");
    assert_eq!(p["celular"], "3001112233", "el cambio del celular no se guardó");
    assert_eq!(p["cod_pais"], "170", "editar BORRÓ el país: los campos no capturados no se conservaron");
    assert_eq!(p["tipo_usuario"], "02", "editar BORRÓ el tipo de usuario");
    assert_eq!(p["zona"], "U", "editar BORRÓ la zona");
}

/// **LA PANTALLA DEL `nilknown` SE DIBUJA Y EL AVAL LLEGA A LA BASE** (#603 F4.5-C,
/// directiva 28: la pantalla se DIBUJA y se cruza la frontera hasta la fila real).
///
/// Se siembra una consulta en la BD de la demostración, se abre
/// `/panel/rda/consulta/{id}` en el navegador, se marca una sección vacía y se
/// envía; la pareja de control mira ANTES (la casilla existe y NO está marcada, y la
/// bitácora está vacía) para que el verde signifique que el POST hizo algo. El
/// desenlace se comprueba en la BASE —`aval_seccion_rda` con la fila—, no en el HTML:
/// un fragmento correcto insertado sin escribir nada daría el mismo DOM.
#[test]
fn la_pantalla_del_nilknown_dibuja_y_el_aval_llega_a_la_base() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let url = format!("sqlite://{}?mode=rwc", s.dir.path().join("medico.db").display());

    // Sembrar una consulta externa del demo (institución 1), con el usuario de la
    // demostración como médico tratante, y devolver su id.
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let hid: i64 = rt.block_on(async {
        let pool = medico_datos::abrir(&url).await.expect("abrir la base sembrada");
        sqlx::query("UPDATE institucion SET codigo_habilitacion='0500100001' WHERE id=1")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT OR IGNORE INTO cie10 (codigo,descripcion,categoria) VALUES ('J00','Rinofaringitis aguda','J00')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT OR IGNORE INTO cups (codigo,nombre,seccion,ambito_rips,quirurgico) VALUES ('890201','Consulta','89','AC',0)")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,institucion_id,tipo_documento,numero_documento,nombre1,apellido1,sexo,fecha_nacimiento) \
             VALUES (7001,1,'CC','7001','Ana','Ruiz','F','1990-01-01')",
        ).execute(&pool).await.unwrap();
        let medico: i64 = sqlx::query_scalar("SELECT id FROM usuario WHERE email = ?")
            .bind(CORREO).fetch_one(&pool).await.unwrap();
        let hid: i64 = sqlx::query_scalar(
            "INSERT INTO historia (codigo_historia, paciente_id, medico_id, fecha, \
                 motivo_consulta, enfermedad_actual, diagnostico_principal, plan_tratamiento, \
                 finalidad, causa_externa, tipo_diagnostico, cod_consulta_cups, \
                 modalidad_grupo_servicio, grupo_servicios) \
             VALUES ('HC-RDA', 7001, ?, '2026-09-03T10:00:00-05:00', \
                 'Tos', 'Gripa', 'J00', 'Reposo', '01', '22', '02', '890201', '01', '01') RETURNING id",
        ).bind(medico).fetch_one(&pool).await.unwrap();
        hid
    });

    let mut nav = Navegador::abrir(&chromium);
    nav.entrar(&base, ENTRADA, CORREO, CLAVE);

    // LA ENTRADA desde la historia (hallazgo SIN CASO ROJO de `sin-caso-rojo`): el
    // clínico llega a preparar el RDA por un enlace en la historia. Si su `href` se
    // rompe, la pantalla entera queda inalcanzable y ninguna prueba se enteraba. Su
    // pareja es implícita: el resto del test prueba que ESA url dibuja la pantalla,
    // así que verificar el enlace cierra el lazo —apunta a donde la pantalla vive—.
    nav.ir(&format!("{base}/panel/historias/{hid}"));
    assert!(
        nav.evaluar(&format!(
            "!!document.querySelector('a[href=\"/panel/rda/consulta/{hid}\"]')"
        )) == serde_json::json!(true),
        "la historia no ofrece el enlace para preparar el RDA: la pantalla queda inalcanzable"
    );

    nav.ir(&format!("{base}/panel/rda/consulta/{hid}"));

    // La pantalla se DIBUJA.
    assert_eq!(
        nav.texto("document.querySelector('h1').textContent"),
        "Preparar RDA — Consulta externa",
        "la pantalla del RDA no se dibujó"
    );

    // LA PUERTA DE EMITIR (#603 F4.6), rama «no configurado». El arnés arranca sin
    // `MEDICO_IHCE_*`, así que `ihce_configurado` es `false`: la sección de
    // transmisión tiene que DECIR que no está configurado y NO ofrecer el botón de
    // emitir —encolar en silencio un RDA que nadie drenaría sería un falso
    // «enviado» (directiva 20)—. La pareja discriminante: si la puerta no existiera,
    // con esta consulta (no lista) saldría el aviso «se podrá emitir cuando esté
    // listo», no el de «no configurado», así que exigir ese texto la aísla.
    assert!(
        nav.evaluar("document.body.textContent.includes('no está configurado en este despliegue')")
            == serde_json::json!(true),
        "la sección de transmisión no dice que el transporte IHCE no está configurado"
    );
    assert!(
        nav.evaluar("!document.querySelector('form[action$=\"/emitir\"]')") == serde_json::json!(true),
        "se ofreció el botón de emitir con el transporte IHCE sin configurar"
    );
    // Hay casillas de sección: la mitad de control (existen y NINGUNA marcada).
    assert!(
        nav.evaluar("document.querySelectorAll('input[name=seccion]').length > 0") == serde_json::json!(true),
        "no se dibujó ninguna casilla de sección"
    );
    assert!(
        nav.evaluar("[...document.querySelectorAll('input[name=seccion]')].every(b => !b.checked)") == serde_json::json!(true),
        "una casilla ya venía marcada: la prueba no mediría el aval"
    );
    // La bitácora está vacía ANTES (control).
    let antes: i64 = rt.block_on(async {
        let pool = medico_datos::abrir(&url).await.unwrap();
        sqlx::query_scalar("SELECT COUNT(*) FROM aval_seccion_rda WHERE tipo_encuentro = 'consulta-externa' AND encuentro_id = ?")
            .bind(hid).fetch_one(&pool).await.unwrap()
    });
    assert_eq!(antes, 0, "la bitácora del aval no estaba vacía antes de avalar");

    // Marcar la PRIMERA sección y enviar el formulario.
    let codigo = nav.texto("document.querySelector('input[name=seccion]').value");
    nav.evaluar(
        "const b = document.querySelector('input[name=seccion]');\
         b.checked = true; b.form.submit(); ''",
    );
    nav.esperar(
        "new URL(location.href).searchParams.get('aviso') === 'avalado'",
        "el POST del aval no volvió con el aviso «avalado»",
    );

    // EL DESENLACE, en la BASE: la fila del aval quedó escrita, atribuida al usuario
    // de la sesión, para la sección marcada.
    let (n, seccion): (i64, String) = rt.block_on(async {
        let pool = medico_datos::abrir(&url).await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM aval_seccion_rda WHERE tipo_encuentro = 'consulta-externa' AND encuentro_id = ?")
            .bind(hid).fetch_one(&pool).await.unwrap();
        let sec: String = sqlx::query_scalar("SELECT seccion_codigo FROM aval_seccion_rda WHERE tipo_encuentro = 'consulta-externa' AND encuentro_id = ?")
            .bind(hid).fetch_one(&pool).await.unwrap();
        (n, sec)
    });
    assert_eq!(n, 1, "el aval no dejó exactamente una fila en la base");
    assert_eq!(seccion, codigo, "la sección avalada no es la que se marcó en la pantalla");

    // Y al recargar, la casilla de esa sección vuelve MARCADA (persistió).
    nav.ir(&format!("{base}/panel/rda/consulta/{hid}"));
    assert!(
        nav.evaluar(&format!(
            "document.querySelector('input[name=seccion][value=\"{seccion}\"]').checked"
        )) == serde_json::json!(true),
        "la casilla avalada no volvió marcada tras recargar"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// MÓDULO DE PÍXELES (D28): la miniatura DICOM se DIBUJA en el navegador.
//
// El E2E HTTP ya prueba que `/api/imagenes/{id}/previa` devuelve un PNG para un
// DICOM subido; lo que SOLO se ve en el navegador es que el panel emite el `<img>`
// para un adjunto DICOM y que el navegador CARGA y DECODIFICA ese PNG (CSP, MIME,
// PNG válido). Un 200 con el fragmento correcto no distingue «se pintó» de «no se
// pintó» (directiva 28). Aquí se sube un DICOM sintético por la API con la cookie
// del navegador, se abre la historia y se exige `naturalWidth > 0`.

/// Un DICOM MONOCHROME2 8-bit sin comprimir (Explicit VR LE), `filas`×`columnas`.
/// SINTÉTICO, sin PHI. Igual que el de las pruebas del módulo, pero aquí (prueba de
/// integración, sin acceso a los helpers `#[cfg(test)]` del crate).
fn dicom_gris(filas: u16, columnas: u16) -> Vec<u8> {
    use dicom_core::{DataElement, PrimitiveValue, Tag, VR};
    use dicom_object::{FileMetaTableBuilder, InMemDicomObject};
    let us = |g: u16, e: u16, v: u16| DataElement::new(Tag(g, e), VR::US, PrimitiveValue::from(v));
    let pixeles: Vec<u8> = (0..usize::from(filas) * usize::from(columnas))
        .map(|i| (i % 256) as u8)
        .collect();
    let mut obj = InMemDicomObject::new_empty();
    obj.put(us(0x0028, 0x0002, 1)); // SamplesPerPixel
    obj.put(DataElement::new(Tag(0x0028, 0x0004), VR::CS, PrimitiveValue::from("MONOCHROME2")));
    obj.put(us(0x0028, 0x0010, filas)); // Rows
    obj.put(us(0x0028, 0x0011, columnas)); // Columns
    obj.put(us(0x0028, 0x0100, 8)); // BitsAllocated
    obj.put(us(0x0028, 0x0101, 8)); // BitsStored
    obj.put(us(0x0028, 0x0102, 7)); // HighBit
    obj.put(us(0x0028, 0x0103, 0)); // PixelRepresentation
    obj.put(DataElement::new(Tag(0x7FE0, 0x0010), VR::OB, PrimitiveValue::from(pixeles)));
    let archivo = obj
        .with_meta(
            FileMetaTableBuilder::new()
                .media_storage_sop_class_uid("1.2.840.10008.5.1.4.1.1.7")
                .media_storage_sop_instance_uid("1.2.3.4.5.6.7.8")
                .transfer_syntax("1.2.840.10008.1.2.1"),
        )
        .expect("meta");
    let mut bytes = Vec::new();
    archivo.write_all(&mut bytes).expect("serializar DICOM");
    bytes
}

#[test]
fn la_miniatura_dicom_se_dibuja_en_el_navegador() {
    let chromium = buscar_chromium();
    let s = arrancar_sembrado();
    let base = format!("http://127.0.0.1:{}", s.puerto);
    let mut nav = Navegador::abrir(&chromium);
    nav.entrar(&base, ENTRADA, CORREO, CLAVE);

    // DICOM sintético → literal `Uint8Array` para subirlo por `fetch` en el navegador.
    let arreglo = dicom_gris(8, 8)
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",");

    // Paciente + historia + subida del DICOM, todo por la API con la cookie ya puesta.
    nav.ir(&format!("{base}/panel/pacientes"));
    nav.evaluar(&format!(
        r#"window.__hdcm = null;
           (async () => {{
             try {{
               const yo = await (await fetch('/api/yo')).json();
               const p = await fetch('/api/pacientes', {{method:'POST',
                 headers:{{'Content-Type':'application/json'}},
                 body: JSON.stringify({{tipo_documento:'CC', numero_documento:'70707070',
                   nombre1:'Rx', apellido1:'Prueba', fecha_nacimiento:'1990-01-01', sexo:'M'}})}});
               if(!p.ok){{window.__hdcm='paciente '+p.status;return;}}
               const pj = await p.json();
               const h = await fetch('/api/historias', {{method:'POST',
                 headers:{{'Content-Type':'application/json'}},
                 body: JSON.stringify({{paciente_id:pj.id, medico_id:yo.id, motivo_consulta:'rx',
                   enfermedad_actual:'rx', diagnostico_principal:'J00', plan_tratamiento:'rx'}})}});
               if(!h.ok){{window.__hdcm='historia '+h.status;return;}}
               const hj = await h.json();
               const bytes = new Uint8Array([{arreglo}]);
               const fd = new FormData();
               fd.append('archivo', new Blob([bytes], {{type:'application/dicom'}}), 'placa.dcm');
               const u = await fetch('/api/historias/'+hj.id+'/imagenes', {{method:'POST', body: fd}});
               if(!u.ok){{window.__hdcm='subida '+u.status+': '+(await u.text());return;}}
               window.__hdcm = hj.id;
             }} catch(e) {{ window.__hdcm = 'error: '+e; }}
           }})();"#
    ));
    nav.esperar("window.__hdcm !== null", "no se pudo sembrar la historia con el DICOM");
    let crudo = nav.texto("String(window.__hdcm)");
    let historia: i64 = crudo
        .parse()
        .unwrap_or_else(|_| panic!("no se sembró el DICOM: {crudo}"));

    // La historia DIBUJADA. Primero que el panel EMITA el <img> de la miniatura para
    // el adjunto DICOM; luego que el navegador lo CARGUE y DECODIFIQUE (naturalWidth).
    nav.ir(&format!("{base}/panel/historias/{historia}/imagenes"));
    nav.esperar_hasta(
        "document.querySelector('img.miniatura-dicom') !== null",
        "el panel no dibujó el <img> de la miniatura para el adjunto DICOM",
        std::time::Duration::from_secs(20),
    );
    nav.esperar_hasta(
        "(() => { const m = document.querySelector('img.miniatura-dicom'); \
          return !!m && m.complete && m.naturalWidth > 0; })()",
        "la miniatura existe pero el navegador no cargó/decodificó /previa \
         (CSP, MIME o PNG inválido): el pixel DICOM no llegó a pintarse",
        std::time::Duration::from_secs(20),
    );
}
