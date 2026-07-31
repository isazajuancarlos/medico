//! Rompe la compilación de *release* si la interfaz no está empotrada.
//!
//! Por qué existe: `rust-embed` lee de disco en *debug* y empotra en *release*.
//! Con `estaticos/` vacía —el estado de un clon limpio, porque su contenido está
//! en `.gitignore`— `cargo build --release` producía **un binario sin interfaz y
//! no avisaba**: compilaba, arrancaba, servía la API y devolvía un 404 con texto
//! plano a cualquiera que abriera la raíz. El fallo solo se veía en el navegador
//! del cliente, que es el peor sitio posible para verlo.
//!
//! El aviso en el log no cuenta como fallar (directiva 20): aquí se cae la
//! compilación. Y se cae en el `build.rs` y no en un README, porque lo que puede
//! ser código no va en prosa (directiva 6).
//!
//! El camino permisivo existe pero hay que PEDIRLO: `MEDICO_SIN_INTERFAZ=1`
//! compila el binario a propósito sin interfaz (perfilar el servidor, un
//! artefacto de solo-API). Lo que no se admite es obtenerlo por descuido.

use std::path::Path;

fn main() {
    // Que cargo vuelva a evaluar esto cuando aparezca o desaparezca la interfaz,
    // y cuando se encienda o apague la salida explícita.
    println!("cargo:rerun-if-changed=estaticos");
    println!("cargo:rerun-if-changed=estaticos/index.html");
    println!("cargo:rerun-if-env-changed=MEDICO_SIN_INTERFAZ");

    let perfil = std::env::var("PROFILE").unwrap_or_default();
    if perfil != "release" {
        return;
    }

    if std::env::var_os("MEDICO_SIN_INTERFAZ").is_some() {
        println!(
            "cargo:warning=MEDICO_SIN_INTERFAZ: se compila a propósito un binario \
             sin interfaz Angular."
        );
        return;
    }

    // `index.html` y no la carpeta: la carpeta EXISTE siempre (lleva `.gitkeep`
    // versionado para que compile un clon limpio), así que comprobar su presencia
    // no distinguiría nada. El punto de entrada del SPA es lo que de verdad falta.
    if !Path::new("estaticos/index.html").exists() {
        panic!(
            "\n\n\
             La interfaz Angular NO está empotrada: falta \
             crates/medico-api/estaticos/index.html\n\n\
             Un binario de release sin interfaz arranca, sirve la API y responde \
             404 en la raíz.\n\
             Es un fallo que solo se ve en el navegador del cliente.\n\n\
             Construya el artefacto completo, que hace las dos cosas en el orden \
             correcto:\n\n    ./construir.sh\n\n\
             Si de verdad quiere un binario sin interfaz, pídalo:\n\n    \
             MEDICO_SIN_INTERFAZ=1 cargo build --release\n"
        );
    }
}
