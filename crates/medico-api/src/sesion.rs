//! La sesión vive en una cookie firmada, no en un token que el JavaScript lea.
//!
//! El binario sirve la propia interfaz, así que no hay segundo origen: la cookie
//! viaja sola, `HttpOnly` (el JavaScript no puede leerla) y `SameSite=Strict`
//! (no viaja desde otro sitio, no hay CSRF que resolver). Un JWT en
//! `localStorage` sería lo contrario: cualquier XSS lo exfiltra y el servidor no
//! puede invalidarlo antes de que caduque.
//!
//! La cookie lleva `id|token_version|caducidad|firma` (HMAC-SHA256, comparada en
//! tiempo constante). NO lleva el rol ni la institución: esos se leen de la base
//! en cada petición, porque un rol dentro de la cookie sobrevive a que un
//! administrador lo revoque. El `token_version` es la revocación selectiva (M3):
//! `auth` lo compara con el de la base y rechaza el que quedó por detrás.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::{header, HeaderMap};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const NOMBRE_COOKIE: &str = "medico_sesion";

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sesion {
    pub usuario_id: i64,
    /// La versión de sesión con la que se emitió el token (M3). `auth` la compara
    /// con la del usuario en la base: si quedó por detrás, el token está revocado.
    pub token_version: i64,
    pub caduca_en: i64,
}

fn ahora() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn firmar(secreto: &[u8], carga: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secreto).expect("HMAC acepta cualquier longitud");
    mac.update(carga.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

/// Construye el valor de la cookie para un usuario, atada a su `token_version`
/// actual (M3). Si luego se revoca (sube la versión en la base), este token queda
/// por detrás y `auth` lo rechaza.
pub fn emitir(secreto: &[u8], usuario_id: i64, token_version: i64, minutos: i64) -> String {
    let caduca = ahora() + minutos * 60;
    let carga = format!("{usuario_id}|{token_version}|{caduca}");
    let firma = firmar(secreto, &carga);
    format!("{carga}|{firma}")
}

/// Lee y **verifica**. `None` ante cualquier duda: firma que no cuadra, formato
/// raro o caducada. No hay mensaje distinto por caso — decir cuál falló solo le
/// sirve a quien está probando.
pub fn verificar(secreto: &[u8], valor: &str) -> Option<Sesion> {
    let (carga, firma) = valor.rsplit_once('|')?;
    let esperada = firmar(secreto, carga);
    if !constante_iguales(esperada.as_bytes(), firma.as_bytes()) {
        return None;
    }
    let mut campos = carga.splitn(3, '|');
    let sesion = Sesion {
        usuario_id: campos.next()?.parse().ok()?,
        token_version: campos.next()?.parse().ok()?,
        caduca_en: campos.next()?.parse().ok()?,
    };
    if sesion.caduca_en <= ahora() {
        return None;
    }
    Some(sesion)
}

fn constante_iguales(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `Set-Cookie` de una sesión abierta. `Secure` se añade en el proxy: el perfil
/// de una sola máquina se sirve por HTTP en `localhost`, y ahí una cookie
/// `Secure` no se guarda.
pub fn cabecera_abrir(valor: &str, minutos: i64) -> String {
    format!(
        "{NOMBRE_COOKIE}={valor}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}",
        minutos * 60
    )
}

pub fn cabecera_cerrar() -> String {
    format!("{NOMBRE_COOKIE}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0")
}

pub fn de_cabeceras(cabeceras: &HeaderMap) -> Option<String> {
    let crudo = cabeceras.get(header::COOKIE)?.to_str().ok()?;
    crudo
        .split(';')
        .filter_map(|p| p.trim().split_once('='))
        .find(|(k, _)| *k == NOMBRE_COOKIE)
        .map(|(_, v)| v.to_string())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const SECRETO: &[u8] = b"un secreto de al menos treinta y dos bytes";

    #[test]
    fn una_sesion_emitida_se_verifica() {
        let v = emitir(SECRETO, 42, 7, 60);
        let s = verificar(SECRETO, &v).unwrap();
        assert_eq!(s.usuario_id, 42);
        assert_eq!(s.token_version, 7, "la versión viaja en el token y se recupera");
    }

    /// La prueba que discrimina: sin ella, un verificador que siempre devuelve
    /// `Some` pasaría la anterior.
    #[test]
    fn cambiar_el_usuario_invalida_la_firma() {
        let v = emitir(SECRETO, 42, 0, 60);
        let manipulada = v.replacen("42|", "1|", 1);
        assert!(verificar(SECRETO, &manipulada).is_none());
    }

    /// Subir la versión en el token sin re-firmar no cuela: la versión va DENTRO
    /// de la carga firmada, así que un atacante no puede fabricarse una sesión a
    /// la versión nueva para sortear una revocación.
    #[test]
    fn manipular_la_version_invalida_la_firma() {
        let v = emitir(SECRETO, 42, 0, 60);
        let manipulada = v.replacen("42|0|", "42|9|", 1);
        assert!(verificar(SECRETO, &manipulada).is_none());
    }

    #[test]
    fn otro_secreto_no_vale() {
        let v = emitir(SECRETO, 42, 0, 60);
        assert!(verificar(b"otro secreto igualmente largo de 32+", &v).is_none());
    }

    #[test]
    fn una_sesion_caducada_no_entra() {
        let v = emitir(SECRETO, 42, 0, -1);
        assert!(verificar(SECRETO, &v).is_none());
    }
}
