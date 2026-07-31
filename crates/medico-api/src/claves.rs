//! Contraseñas: se guardan como hash Argon2 y se verifican; nunca en claro.
//!
//! Es la misma familia (Argon2) que el sistema Python usa. Los hashes que ya
//! existen se verifican tal cual: `PasswordHash` lee los parámetros del propio
//! hash, así que un hash producido con otra configuración sigue verificándose.

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, SaltString};
use argon2::{Argon2, PasswordVerifier};

/// Hash Argon2 de una contraseña, con sal aleatoria. `None` si el hasher falla
/// (no debería), para no propagar un `Result` por una rama que no ocurre.
pub fn hashear(clave: &str) -> Option<String> {
    let sal = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(clave.as_bytes(), &sal)
        .ok()
        .map(|h| h.to_string())
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

/// Verifica una contraseña contra su hash. Un hash ilegible cuenta como «no
/// coincide», no como un error distinto: al que intenta entrar no se le dice en
/// qué se equivocó.
pub fn verificar(clave: &str, hash: &str) -> bool {
    let Ok(analizado) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(clave.as_bytes(), &analizado)
        .is_ok()
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
        // el hash de descarte no PARSEA, `verificar` sale antes de ejecutar
        // Argon2 y el trabajo que iguala los tiempos no se hace. El resultado
        // sería el mismo `false` de siempre y nadie lo notaría.
        let h = hash_de_descarte();
        assert!(
            PasswordHash::new(h).is_ok(),
            "el hash de descarte no parsea: la verificación saldría al instante"
        );
        assert!(!verificar("lo-que-sea", h), "el hash de descarte dejó pasar algo");
        // Y es estable: dos llamadas dan el mismo, o cada intento fallido pagaría
        // un hasheo nuevo además de la verificación.
        assert_eq!(h, hash_de_descarte());
    }
}
