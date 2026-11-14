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
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

pub const NOMBRE_COOKIE: &str = "medico_sesion";

/// La cookie del primer paso cuando la cuenta tiene segundo factor.
///
/// Cookie APARTE y no la misma con otro contenido: si compartieran nombre, abrir
/// el pendiente pisaría una sesión válida que ya estuviera abierta en ese
/// navegador —quien tenga dos pestañas y vuelva a la pantalla de entrada se
/// quedaría fuera de la que ya tenía—.
pub const NOMBRE_COOKIE_PENDIENTE: &str = "medico_pendiente";

/// Cuánto vive el token pendiente. Cinco minutos: lo que se tarda en sacar el
/// teléfono y teclear seis dígitos, y no más. Es una credencial parcial —quien
/// la robe solo necesita el código— así que no hereda la hora de la sesión.
pub const MINUTOS_PENDIENTE: i64 = 5;

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

/// El prefijo que separa los DOS tipos de token que firma el mismo secreto.
///
/// # Por qué un dominio de firma y no un cuarto campo en la carga
///
/// La forma obvia sería añadir `pendiente=1` a la carga y mirarlo al verificar.
/// Eso deja que un token pendiente y una sesión compartan firma válida, y la
/// separación pasa a depender de que TODOS los sitios que verifican se acuerden
/// de mirar ese campo: el día que uno no lo mire, un token pendiente —que se
/// entrega ANTES del segundo factor— entra como sesión completa.
///
/// Con el prefijo, la separación la hace el HMAC. Un pendiente presentado como
/// sesión no falla una comprobación: **falla la firma**, que es el camino por el
/// que ya se rechaza todo lo demás y el que no se puede olvidar de mirar. Lo ata
/// `un_token_pendiente_no_vale_como_sesion_ni_al_reves`.
const DOMINIO_PENDIENTE: &str = "pendiente";

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

/// El token del primer paso: la contraseña ya se comprobó, el código no.
///
/// **No es una sesión y no puede llegar a serlo**: va firmado en otro dominio
/// (ver [`DOMINIO_PENDIENTE`]) y en otra cookie. Lo único que abre es
/// `/api/2fa/verificar`.
pub fn emitir_pendiente(
    secreto: &[u8],
    usuario_id: i64,
    token_version: i64,
    pendiente_version: i64,
) -> String {
    let caduca = ahora() + MINUTOS_PENDIENTE * 60;
    let carga = format!("{usuario_id}|{token_version}|{caduca}|{pendiente_version}");
    let firma = firmar(secreto, &format!("{DOMINIO_PENDIENTE}|{carga}"));
    format!("{carga}|{firma}")
}

/// Lee y verifica un token pendiente. `None` ante cualquier duda, igual que
/// [`verificar`] — y en particular ante una firma de sesión, que no cuadra aquí.
pub fn verificar_pendiente(secreto: &[u8], valor: &str) -> Option<Pendiente> {
    let (carga, firma) = valor.rsplit_once('|')?;
    let esperada = firmar(secreto, &format!("{DOMINIO_PENDIENTE}|{carga}"));
    if !constante_iguales(esperada.as_bytes(), firma.as_bytes()) {
        return None;
    }
    let mut campos = carga.splitn(4, '|');
    let p = Pendiente {
        sesion: Sesion {
            usuario_id: campos.next()?.parse().ok()?,
            token_version: campos.next()?.parse().ok()?,
            caduca_en: campos.next()?.parse().ok()?,
        },
        version: campos.next()?.parse().ok()?,
    };
    if p.sesion.caduca_en <= ahora() {
        return None;
    }
    Some(p)
}

/// Un pendiente verificado: la sesión que abriría, y **el número que hay que
/// consumir en la base** para que no valga dos veces.
///
/// La firma sola no basta y por eso esto no es un `Sesion` a secas: un HMAC
/// autocontenido es reenviable por cualquiera que conserve su valor, y el
/// borrado de la cookie solo lo obedece un navegador. El servidor tiene que
/// tener la última palabra — ver `segundofactor::consumir_pendiente`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pendiente {
    pub sesion: Sesion,
    pub version: i64,
}

/// `Set-Cookie` del pendiente. Mismos atributos que la sesión, por lo mismo.
pub fn cabecera_pendiente_abrir(valor: &str, tras_proxy: bool) -> String {
    format!(
        "{NOMBRE_COOKIE_PENDIENTE}={valor}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
        MINUTOS_PENDIENTE * 60,
        if tras_proxy { "; Secure" } else { "" }
    )
}

/// El pendiente se QUEMA al usarse, valga o no: si sobreviviera al segundo paso
/// serviría para un segundo intento con la contraseña ya dada por buena.
pub fn cabecera_pendiente_cerrar(tras_proxy: bool) -> String {
    format!(
        "{NOMBRE_COOKIE_PENDIENTE}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{}",
        if tras_proxy { "; Secure" } else { "" }
    )
}

pub fn pendiente_de_cabeceras(cabeceras: &HeaderMap) -> Option<String> {
    de_cookie(cabeceras, NOMBRE_COOKIE_PENDIENTE)
}

fn constante_iguales(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `Set-Cookie` de una sesión abierta.
///
/// # `Secure` lo decide el despliegue, y ya lo tenía dicho
///
/// Antes esto no marcaba `Secure` nunca, y el comentario afirmaba que «se añade
/// en el proxy». **Ningún artefacto del repositorio lo añadía**: no hay vhost de
/// nginx para medico. Era un control declarado y no ejercido, que es justo la
/// forma en que nadie llega a implementarlo — quien lea el código cree que está
/// resuelto y sigue.
///
/// El resto del comentario sí era cierto y sigue mandando: el perfil de una sola
/// máquina se sirve por HTTP en `localhost`, y ahí el navegador **descarta** una
/// cookie `Secure`. Marcarla siempre dejaría ese despliegue sin poder entrar.
///
/// Así que la marca la decide `tras_proxy`, que es la única señal que el binario
/// tiene y que ya significa exactamente esto: *«hay un proxy inverso delante»* —
/// y en este despliegue el proxy inverso es donde termina el TLS.
///
/// **El límite, dicho:** un proxy inverso que sirviera HTTP en claro recibiría
/// una cookie `Secure` que el navegador tira, y nadie podría entrar. Es un fallo
/// RUIDOSO y en el primer intento de login, no uno silencioso — que es la
/// dirección correcta para equivocarse aquí. Un despliegue así no existe hoy;
/// el día que exista, hará falta su propia bandera.
pub fn cabecera_abrir(valor: &str, minutos: i64, tras_proxy: bool) -> String {
    format!(
        "{NOMBRE_COOKIE}={valor}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
        minutos * 60,
        if tras_proxy { "; Secure" } else { "" }
    )
}

/// La cookie de cierre lleva los MISMOS atributos que la de apertura, `Secure`
/// incluido. Un navegador solo reemplaza una cookie por otra si coinciden en
/// nombre, dominio y ruta; los atributos de seguridad no forman parte de esa
/// identidad, pero mantenerlos iguales evita el caso en que el cierre no
/// sustituye a la sesión abierta y esta sobrevive al «salir».
pub fn cabecera_cerrar(tras_proxy: bool) -> String {
    format!(
        "{NOMBRE_COOKIE}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{}",
        if tras_proxy { "; Secure" } else { "" }
    )
}

pub fn de_cabeceras(cabeceras: &HeaderMap) -> Option<String> {
    de_cookie(cabeceras, NOMBRE_COOKIE)
}

/// Una cookie por su nombre. Se factoriza porque desde el segundo factor son
/// dos, y dos copias de este recorrido se desincronizan en cuanto una aprenda a
/// tratar un separador raro y la otra no.
fn de_cookie(cabeceras: &HeaderMap, nombre: &str) -> Option<String> {
    let crudo = cabeceras.get(header::COOKIE)?.to_str().ok()?;
    crudo
        .split(';')
        .filter_map(|p| p.trim().split_once('='))
        .find(|(k, _)| *k == nombre)
        .map(|(_, v)| v.to_string())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const SECRETO: &[u8] = b"un secreto de al menos treinta y dos bytes";

    /// Una cookie REAL emitida el 2026-08-06, con caducidad puesta a doscientos
    /// años para que el vector no se pudra: caduca en 2226.
    const COOKIE_2026: &str = "42|7|8093198475|_4aj7wo1SU9EWZrGsSzmLrvqfZLoH8jSGHi2JRjgMfk";

    /// **EL VECTOR FIJO**, y la razón por la que hace falta está medida.
    ///
    /// Las otras cinco pruebas de este módulo emiten Y verifican con el MISMO
    /// binario, así que miden el códec contra sí mismo: pasarían igual si
    /// cambiara el separador, el orden de los campos, el motor de base64 o la
    /// generación de `hmac`/`sha2` — **y con ello se cerraría la sesión de todo
    /// el mundo a la vez, en silencio y sin un solo error en rojo**.
    ///
    /// Lo levantó la revisión de seguridad del bump de base64 (2026-08-06):
    /// `guaca::sesion`, de donde salió el diseño de esta cookie, SÍ lleva su
    /// vector (`TOKEN_2026`) con este mismo comentario; medico no lo heredó.
    ///
    /// Si se pone roja, **NO se regenera el literal sin pensarlo**: significa
    /// que las cookies vivas dejan de valer, y eso se decide y se comunica.
    /// **El NOMBRE de la cookie también es formato**, y el vector no lo mira.
    ///
    /// Lo midió la revisión de seguridad de esta rama con un mutante:
    /// renombrando `NOMBRE_COOKIE` a `medico_sess`, las diez pruebas siguieron
    /// VERDES. Y el daño es exactamente el mismo que el vector existe para
    /// impedir —«se cierra la sesión de todo el mundo a la vez, en silencio»—
    /// solo que por una vía que el vector no recorre: él solo ejercita
    /// `verificar` sobre la CARGA, y las pruebas de cabecera interpolan
    /// `{NOMBRE_COOKIE}` en vez de fijarlo, así que se mueven con él.
    ///
    /// Es la directiva 23 aplicada a la propia salvaguarda: se arregló el lado
    /// ruidoso y quedaba el ciego, que no se queja nunca.
    ///
    /// Si se pone roja, NO se cambia el literal: los navegadores de todos los
    /// usuarios llevan la cookie con el nombre viejo, y renombrarla los deja
    /// fuera. Eso se decide y se comunica.
    #[test]
    fn el_nombre_de_la_cookie_no_se_renombra_a_la_ligera() {
        assert_eq!(NOMBRE_COOKIE, "medico_sesion");
    }

    #[test]
    fn una_cookie_de_2026_sigue_verificando() {
        let s = verificar(SECRETO, COOKIE_2026)
            .expect("la cookie del 2026-08-06 dejó de verificar: cambió el formato");
        assert_eq!(s.usuario_id, 42);
        assert_eq!(s.token_version, 7, "el orden de los campos se movió");
    }

    /// Un vector con fecha caduca EN SILENCIO: el día que pase 2226 —o que
    /// alguien ponga el reloj adelante— la prueba de arriba fallaría por
    /// caducidad y no por formato, y se leería como una rotura que no es. Esto
    /// avisa antes, con diez años de margen, y dice qué hacer.
    #[test]
    fn el_vector_de_la_cookie_no_esta_a_punto_de_caducar() {
        let s = verificar(SECRETO, COOKIE_2026).unwrap();
        let diez_anios = 10 * 365 * 24 * 3600;
        assert!(
            s.caduca_en - ahora() > diez_anios,
            "al vector de la cookie le quedan menos de diez años: regenerarlo con \
             `emitir(SECRETO, 42, 7, 105_120_000)` y pegar el resultado, NO relajar \
             esta comprobación"
        );
    }

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

    // === LOS ATRIBUTOS DE LA COOKIE ======================================
    //
    // Antes había un comentario afirmando que «`Secure` se añade en el proxy» y
    // ningún artefacto lo añadía. Estas pruebas convierten la afirmación en un
    // hecho comprobable: si mañana alguien quita la marca, se ponen rojas.

    #[test]
    fn tras_un_proxy_la_cookie_va_marcada_secure() {
        let c = cabecera_abrir("x", 60, true);
        assert!(c.contains("; Secure"), "{c}");
        // Y el cierre igual, o la cookie de sesión podría sobrevivir al «salir».
        assert!(cabecera_cerrar(true).contains("; Secure"));
    }

    /// Sin proxy NO se marca, y no es un olvido: el perfil de una sola máquina
    /// se sirve por HTTP en `localhost`, donde el navegador DESCARTA una cookie
    /// `Secure`. Marcarla siempre dejaría ese despliegue sin poder entrar.
    #[test]
    fn en_una_sola_maquina_no_se_marca_y_es_a_proposito() {
        let c = cabecera_abrir("x", 60, false);
        assert!(!c.contains("Secure"), "{c}");
        assert!(!cabecera_cerrar(false).contains("Secure"));
    }

    /// Lo que NO depende del despliegue, en ninguna de las dos: `HttpOnly` cierra
    /// el paso a `document.cookie` y `SameSite=Strict` al CSRF. Que la marca de
    /// `Secure` sea condicional no puede arrastrarse a éstas.
    #[test]
    fn httponly_y_samesite_van_siempre_en_las_dos_cabeceras() {
        for tras_proxy in [true, false] {
            for c in [cabecera_abrir("x", 60, tras_proxy), cabecera_cerrar(tras_proxy)] {
                assert!(c.contains("HttpOnly"), "{c}");
                assert!(c.contains("SameSite=Strict"), "{c}");
            }
        }
    }

    // ───────────────────────── el token pendiente ─────────────────────────

    /// **La prueba que hace real la separación.** Un token pendiente no vale
    /// como sesión, y una sesión no vale como pendiente.
    ///
    /// Es la que justifica el dominio de firma en vez de un cuarto campo: aquí
    /// no se comprueba que alguien MIRE una bandera, se comprueba que el HMAC no
    /// cuadre. Con la bandera, esta prueba pasaría igual y la protección
    /// dependería de que ningún verificador futuro se olvide de leerla.
    #[test]
    fn un_token_pendiente_no_vale_como_sesion_ni_al_reves() {
        let pendiente = emitir_pendiente(SECRETO, 7, 1, 3);
        let sesion = emitir(SECRETO, 7, 1, 60);

        // Cada uno vale en su sitio…
        assert_eq!(verificar_pendiente(SECRETO, &pendiente).map(|p| p.sesion.usuario_id), Some(7));
        assert_eq!(verificar(SECRETO, &sesion).map(|s| s.usuario_id), Some(7));

        // …y NO en el del otro. Esto es lo que impide que el token del primer
        // paso —entregado ANTES del segundo factor— abra el panel.
        assert_eq!(verificar(SECRETO, &pendiente), None, "el pendiente entró como sesión");
        assert_eq!(verificar_pendiente(SECRETO, &sesion), None, "la sesión entró como pendiente");
        // Y el pendiente lleva su NÚMERO, que es lo que el servidor consume.
        assert_eq!(verificar_pendiente(SECRETO, &pendiente).unwrap().version, 3);
    }

    /// El pendiente lleva el `token_version` para que una revocación ocurrida
    /// entre los dos pasos lo alcance. Sin esto, quien perdiera el teléfono y
    /// pidiera revocar sesiones seguiría teniendo cinco minutos de puerta
    /// abierta con la contraseña ya dada por buena.
    #[test]
    fn el_pendiente_arrastra_la_version_de_sesion() {
        let p = emitir_pendiente(SECRETO, 7, 42, 3);
        assert_eq!(verificar_pendiente(SECRETO, &p).map(|p| p.sesion.token_version), Some(42));
    }

    /// Con otro secreto no vale, que es la comprobación de siempre pero también
    /// hay que hacerla en el camino nuevo.
    #[test]
    fn un_pendiente_firmado_con_otro_secreto_no_entra() {
        let p = emitir_pendiente(b"otro secreto completamente distinto y largo", 7, 1, 3);
        assert_eq!(verificar_pendiente(SECRETO, &p), None);
    }

    /// Las dos cookies tienen NOMBRES distintos: si compartieran uno, abrir el
    /// pendiente pisaría una sesión ya abierta en ese navegador.
    #[test]
    fn las_dos_cookies_no_se_pisan() {
        assert_ne!(NOMBRE_COOKIE, NOMBRE_COOKIE_PENDIENTE);
        let abrir = cabecera_pendiente_abrir("x", true);
        assert!(abrir.starts_with(NOMBRE_COOKIE_PENDIENTE));
        assert!(abrir.contains("HttpOnly") && abrir.contains("SameSite=Strict"));
        assert!(abrir.contains("; Secure"), "tras proxy tiene que ir Secure");
        // Y el cierre lleva los MISMOS atributos, o el navegador no lo sustituye.
        let cerrar = cabecera_pendiente_cerrar(true);
        assert!(cerrar.contains("Max-Age=0") && cerrar.contains("; Secure"));
    }

    /// El pendiente dura cinco minutos y no lo que dure la sesión: es una
    /// credencial PARCIAL, y a quien la robe solo le falta el código.
    #[test]
    fn el_pendiente_no_hereda_la_hora_de_la_sesion() {
        assert_eq!(MINUTOS_PENDIENTE, 5);
        let p = verificar_pendiente(SECRETO, &emitir_pendiente(SECRETO, 7, 1, 3)).unwrap().sesion;
        let s = verificar(SECRETO, &emitir(SECRETO, 7, 1, 60)).unwrap();
        assert!(p.caduca_en < s.caduca_en, "el pendiente dura tanto como la sesión");
    }
}

