//! Contraseñas: hash y verificación, TODO a través de bachué —una sola
//! implementación de Argon2 en el universo (guaca, dentro de `bachue-password`)—.
//!
//! Hasta el 2026-09-02 este módulo tenía su PROPIA copia de Argon2
//! (`argon2::Argon2::default()`), gemela byte a byte de `guaca::claves`, de lo que
//! hacía informes y del Python retirado (#598). La **fase 2 de la absorción de
//! tumbaga en bachué** la retiró: aquí queda una fachada delgada, y el login
//! (`auth::autenticar_usuario`) usa la [`bachue_password::Cadena`] DIRECTAMENTE
//! para, al acertar, renovar un Argon2 débil o migrar un hash de legado —con
//! compare-and-swap—. Este módulo es el camino de ESCRITURA y la verificación
//! suelta (confirmar la clave actual antes de cambiarla), que no migran.
//!
//! Que la política de hoy la fije guaca —y no un `Argon2::default()` local— es lo
//! que impide el descuadre futuro: si guaca sube sus parámetros, el hash nuevo y
//! el umbral de renovación suben JUNTOS. Con dos fuentes, cada login intentaría
//! renovar hacia una política que la escritura local nunca alcanza.

/// Hash de una contraseña NUEVA. Siempre Argon2id, vía guaca dentro de bachué.
/// `None` solo si el hasher falla (no debería); el llamante lo trata como fallo,
/// no lo ignora.
pub fn hashear(clave: &str) -> Option<String> {
    bachue_password::Cadena::nueva().cifrar(clave)
}

/// Un hash Argon2 real contra el que verificar cuando NO hay usuario.
///
/// No protege ninguna cuenta: su única función es que comprobar una contraseña
/// cueste lo mismo exista el correo o no. Sin él, `autenticar` respondía al
/// instante ante un correo inexistente y tras ~100 ms ante uno real, y esa
/// diferencia se mide desde fuera con un bucle de `curl`: es un catálogo de los
/// correos válidos servido por el propio login que promete no darlo.
///
/// Se GENERA con el mismo `hashear` en lugar de ir escrito como literal, y esa
/// decisión es el punto: un literal con un solo carácter mal deja de parsear,
/// `verificar` devuelve `false` de inmediato sin ejecutar Argon2, y la
/// diferencia de tiempo que este hash existe para borrar vuelve — sin que nada
/// falle ni se note. Generándolo, es válido por construcción.
///
/// No es un secreto: conocerlo no ayuda a nadie, porque su contraseña no abre
/// ninguna cuenta. Se calcula una vez, la primera vez que hace falta.
pub fn hash_de_descarte() -> &'static str {
    static DESCARTE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DESCARTE.get_or_init(|| {
        hashear("ninguna-cuenta-usa-esta-clave")
            .expect("el hasher de Argon2 no puede fallar sobre una cadena fija")
    })
}

/// Verifica una contraseña contra su hash **sin migrar**. Es para confirmar la
/// clave actual antes de cambiarla, donde no hay nada que persistir; el login usa
/// la cadena, que además renueva. Un hash ilegible cuenta como «no coincide», no
/// como un error distinto: al que intenta entrar no se le dice en qué se equivocó.
pub fn verificar(clave: &str, hash: &str) -> bool {
    guaca::claves::verificar(clave, hash)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_hash_verifica_la_clave_correcta_y_solo_esa() {
        let h = hashear("secreta-de-verdad").unwrap();
        assert!(verificar("secreta-de-verdad", &h));
        assert!(!verificar("otra", &h), "aceptó una clave distinta");
    }

    #[test]
    fn un_hash_ilegible_no_deja_entrar() {
        assert!(!verificar("lo-que-sea", "esto-no-es-un-hash"));
    }

    #[test]
    fn el_hash_de_descarte_es_verificable_de_verdad() {
        // La comprobación que impide que este mecanismo se rompa en silencio: si
        // el hash de descarte no fuera un Argon2 real, `verificar` saldría antes de
        // ejecutar Argon2 y el trabajo que iguala los tiempos no se haría. Como
        // ahora se genera con `hashear` (guaca), es un PHC de Argon2 por
        // construcción; se comprueba su forma y que niega, y que es estable.
        let h = hash_de_descarte();
        assert!(
            h.starts_with("$argon2"),
            "el hash de descarte no es un Argon2: la verificación saldría al instante"
        );
        assert!(!verificar("lo-que-sea", h), "el hash de descarte dejó pasar algo");
        // Y es estable: dos llamadas dan el mismo, o cada intento fallido pagaría
        // un hasheo nuevo además de la verificación.
        assert_eq!(h, hash_de_descarte());
    }
}
