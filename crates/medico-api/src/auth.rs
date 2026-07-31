//! Autenticación: quién entra (login) y quién es en cada petición (extractor).
//!
//! La lógica vive en funciones que se prueban sin servidor (`autenticar`,
//! `usuario_de`); el extractor `Autenticado` es una capa fina de axum encima.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use medico_datos::usuario::{self, Usuario};

use crate::{claves, sesion};

/// Lo que comparten todas las rutas: la base y el secreto de sesión.
#[derive(Clone)]
pub struct Estado {
    pub pool: medico_datos::Pool,
    pub secreto: Vec<u8>,
    pub minutos: i64,
    /// Carpeta donde se guardan los archivos de las imágenes diagnósticas.
    pub dir_imagenes: std::path::PathBuf,
    /// Clave privada de firma (hex) y clave pública de verificación (hex). `None`
    /// si no se configuraron: la firma criptográfica queda desactivada con gracia.
    pub clave_firma: Option<String>,
    pub clave_verificacion: Option<String>,
    /// Frase para cifrar en reposo los archivos. `None` = se guardan en claro.
    pub clave_reposo: Option<String>,
    /// El despliegue AFIRMA estar detrás de un proxy inverso. Solo entonces se
    /// cree `X-Real-IP` para identificar el origen de un intento de acceso.
    ///
    /// **`X-Real-IP`, no `X-Forwarded-For`.** Nuestro nginx escribe la segunda
    /// con `$proxy_add_x_forwarded_for`, que AÑADE: lo que mande el cliente queda
    /// delante de su IP real, así que su primer salto lo elige él. `X-Real-IP` se
    /// escribe con `$remote_addr` y pisa lo que venga. Ver `rutas::origen_de`.
    ///
    /// Sin proxy declarado, el origen es la dirección del zócalo. Si se declara y
    /// la cabecera no llega, el acceso responde 503 en vez de rellenar el dato.
    pub tras_proxy: bool,
}

/// El login. Devuelve el valor de la cookie de sesión, o `None`. **Mismo
/// resultado para «no existe», «inactivo» y «clave incorrecta»**: distinguirlos
/// le diría a quien prueba qué correos existen.
///
/// # Y mismo TIEMPO, que es la mitad que faltaba
///
/// El mensaje era el mismo desde el principio; la duración no. Con un correo
/// inexistente esto respondía en cuanto la consulta volvía vacía, mientras que
/// con uno real gastaba los ~100 ms de Argon2. Esa diferencia es medible desde
/// fuera con un `curl` en un bucle, así que la promesa de arriba se cumplía en
/// la letra y se incumplía en la práctica: quien probara mil correos sabría
/// cuáles existen.
///
/// Por eso, cuando no hay usuario o está inactivo, se verifica igualmente contra
/// un hash de descarte. El trabajo se hace y se tira.
pub async fn autenticar(
    pool: &medico_datos::Pool,
    secreto: &[u8],
    minutos: i64,
    email: &str,
    clave: &str,
) -> Option<String> {
    let encontrado = usuario::por_email(pool, email).await.ok().flatten();

    // El `Option` se resuelve ANTES de verificar, y las dos ramas verifican. Con
    // un `?` aquí volveríamos a tener dos caminos de duración distinta.
    let hash = match &encontrado {
        Some(u) if u.activo => u.password_hash.clone(),
        _ => claves::hash_de_descarte().to_string(),
    };
    let coincide = claves::verificar(clave, &hash);

    match encontrado {
        Some(u) if u.activo && coincide => {
            Some(sesion::emitir(secreto, u.id, u.token_version, minutos))
        }
        _ => None,
    }
}

/// El usuario de una petición, a partir de sus cabeceras. Relee el usuario de la
/// base (no confía en el token para el rol) y rechaza al inactivo. `None` si no
/// hay cookie válida.
pub async fn usuario_de(estado: &Estado, cabeceras: &HeaderMap) -> Option<Usuario> {
    let cookie = sesion::de_cabeceras(cabeceras)?;
    let sesion = sesion::verificar(&estado.secreto, &cookie)?;
    let usuario = usuario::por_id(&estado.pool, sesion.usuario_id).await.ok().flatten()?;
    if !usuario.activo {
        return None;
    }
    // Revocación selectiva (M3): un token emitido antes de subir la versión queda
    // por detrás. Se comprueba aquí, en la relectura del usuario que ya se hacía —
    // sin estado nuevo en el servidor.
    if sesion.token_version < usuario.token_version {
        return None;
    }
    Some(usuario)
}

/// Extractor: una ruta que pide `Autenticado` solo se ejecuta con sesión válida;
/// si no, axum responde 401 sin entrar al handler.
pub struct Autenticado(pub Usuario);

impl FromRequestParts<Estado> for Autenticado {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, estado: &Estado) -> Result<Self, StatusCode> {
        match usuario_de(estado, &parts.headers).await {
            Some(u) => Ok(Autenticado(u)),
            None => Err(StatusCode::UNAUTHORIZED),
        }
    }
}

/// Extractor que además de autenticar **exige una capacidad**.
///
/// # Por qué un extractor y no un `if` dentro del manejador
///
/// La comprobación que se pone a mano es la que se olvida en la ruta siguiente:
/// de 62 rutas registradas, seis comprobaban el rol y ninguna sobre un recurso
/// clínico. Con esto, lo que la ruta necesita queda **en su firma** —
/// `Autorizado<Firmar>` en vez de `Autenticado`—, así que se lee en el listado
/// del router y no hay que ir a mirar el cuerpo para saber qué protege.
///
/// Es el mismo argumento que ya hace `Ambito` siendo un parámetro obligatorio
/// en vez de un filtro opcional: el control que se puede omitir, se omite.
pub struct Autorizado<C: Exige>(pub Usuario, std::marker::PhantomData<C>);

/// Qué capacidad exige un extractor. Un tipo por capacidad, sin datos: sirve
/// para escribirla en la firma del manejador.
pub trait Exige {
    const QUE: medico_dominio::rol::Capacidad;
}

/// Firmar una historia o un documento clínico.
pub struct Firmar;
impl Exige for Firmar {
    const QUE: medico_dominio::rol::Capacidad = medico_dominio::rol::Capacidad::FirmarClinico;
}

/// Borrar una historia o un documento clínico.
pub struct Borrar;
impl Exige for Borrar {
    const QUE: medico_dominio::rol::Capacidad = medico_dominio::rol::Capacidad::BorrarClinico;
}

/// Escribir en la historia clínica: crearla, modificarla, añadirle documentos,
/// fórmulas u órdenes.
pub struct EscribirClinico;
impl Exige for EscribirClinico {
    const QUE: medico_dominio::rol::Capacidad = medico_dominio::rol::Capacidad::EscribirClinico;
}

/// Crear o modificar el registro de un paciente y sus antecedentes.
pub struct EscribirPaciente;
impl Exige for EscribirPaciente {
    const QUE: medico_dominio::rol::Capacidad = medico_dominio::rol::Capacidad::EscribirPaciente;
}

impl<C: Exige> FromRequestParts<Estado> for Autorizado<C> {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, estado: &Estado) -> Result<Self, StatusCode> {
        let usuario = usuario_de(estado, &parts.headers).await.ok_or(StatusCode::UNAUTHORIZED)?;
        // 403 y no 404: dentro de la propia institución la existencia del recurso
        // no es secreto —el usuario puede listarlo—, y decirle «no existe» cuando
        // lo que pasa es que no le corresponde manda a buscar un fallo que no hay.
        // El 404 sigue siendo para lo AJENO, que es donde sí se oculta.
        match usuario.rol() {
            Some(rol) if rol.puede(C::QUE) => Ok(Autorizado(usuario, std::marker::PhantomData)),
            _ => Err(StatusCode::FORBIDDEN),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use axum::http::header;

    const SECRETO: &[u8] = b"un secreto de prueba de mas de treinta y dos bytes";

    async fn base() -> medico_datos::Pool {
        let pool = medico_datos::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id, nombre, nit) VALUES (1,'Uno','900')")
            .execute(&pool).await.unwrap();
        let h = claves::hashear("clave123").unwrap();
        sqlx::query(
            "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo)
             VALUES (2,'Dra Ruiz','ruiz@x.co',?,'medico',1,1),
                    (3,'Retirado','ex@x.co',?,'medico',1,0)",
        )
        .bind(&h).bind(&h)
        .execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn autenticar_con_la_clave_correcta_da_cookie() {
        let pool = base().await;
        let cookie = autenticar(&pool, SECRETO, 60, "ruiz@x.co", "clave123").await;
        assert!(cookie.is_some());
        // La cookie es una sesión válida del usuario 2.
        let s = sesion::verificar(SECRETO, &cookie.unwrap()).unwrap();
        assert_eq!(s.usuario_id, 2);
    }

    #[tokio::test]
    async fn la_clave_incorrecta_no_entra() {
        let pool = base().await;
        assert!(autenticar(&pool, SECRETO, 60, "ruiz@x.co", "otra").await.is_none());
    }

    #[tokio::test]
    async fn el_correo_que_no_existe_da_lo_mismo() {
        let pool = base().await;
        assert!(autenticar(&pool, SECRETO, 60, "nadie@x.co", "clave123").await.is_none());
    }

    #[tokio::test]
    async fn el_inactivo_no_entra_aunque_la_clave_sea_correcta() {
        let pool = base().await;
        assert!(autenticar(&pool, SECRETO, 60, "ex@x.co", "clave123").await.is_none());
    }

    #[tokio::test]
    async fn usuario_de_una_cookie_valida_es_el_usuario() {
        let pool = base().await;
        let estado = Estado { pool, secreto: SECRETO.to_vec(), minutos: 60, dir_imagenes: std::env::temp_dir(), clave_firma: None, clave_verificacion: None, clave_reposo: None, tras_proxy: false };
        let cookie = sesion::emitir(SECRETO, 2, 0, 60);
        let mut cabeceras = HeaderMap::new();
        cabeceras.insert(header::COOKIE, format!("medico_sesion={cookie}").parse().unwrap());
        let u = usuario_de(&estado, &cabeceras).await.unwrap();
        assert_eq!(u.id, 2);
    }

    #[tokio::test]
    async fn sin_cookie_no_hay_usuario() {
        let pool = base().await;
        let estado = Estado { pool, secreto: SECRETO.to_vec(), minutos: 60, dir_imagenes: std::env::temp_dir(), clave_firma: None, clave_verificacion: None, clave_reposo: None, tras_proxy: false };
        assert!(usuario_de(&estado, &HeaderMap::new()).await.is_none());
    }

    /// Una cookie firmada con OTRO secreto no vale — el token no basta, hay que
    /// haberlo firmado con el secreto del servidor.
    #[tokio::test]
    async fn una_cookie_de_otro_secreto_no_entra() {
        let pool = base().await;
        let estado = Estado { pool, secreto: SECRETO.to_vec(), minutos: 60, dir_imagenes: std::env::temp_dir(), clave_firma: None, clave_verificacion: None, clave_reposo: None, tras_proxy: false };
        let ajena = sesion::emitir(b"otro secreto largo de mas de 32 bytes!", 2, 0, 60);
        let mut cabeceras = HeaderMap::new();
        cabeceras.insert(header::COOKIE, format!("medico_sesion={ajena}").parse().unwrap());
        assert!(usuario_de(&estado, &cabeceras).await.is_none());
    }

    fn estado_de(pool: medico_datos::Pool) -> Estado {
        Estado {
            pool,
            secreto: SECRETO.to_vec(),
            minutos: 60,
            dir_imagenes: std::env::temp_dir(),
            clave_firma: None,
            clave_verificacion: None,
            clave_reposo: None,
            tras_proxy: false,
        }
    }

    fn con_cookie(valor: &str) -> HeaderMap {
        let mut c = HeaderMap::new();
        c.insert(header::COOKIE, format!("medico_sesion={valor}").parse().unwrap());
        c
    }

    /// El corazón de M3: revocar sube la versión, y la cookie emitida ANTES deja de
    /// valer en la siguiente petición, mientras que un login posterior sí entra.
    #[tokio::test]
    async fn revocar_invalida_las_cookies_previas_pero_no_las_nuevas() {
        let pool = base().await;
        let estado = estado_de(pool.clone());
        let previa = autenticar(&pool, SECRETO, 60, "ruiz@x.co", "clave123").await.unwrap();
        assert!(usuario_de(&estado, &con_cookie(&previa)).await.is_some(), "válida antes de revocar");

        assert!(medico_datos::usuario::revocar_sesiones(&pool, 2).await.unwrap());

        assert!(usuario_de(&estado, &con_cookie(&previa)).await.is_none(), "la revocación invalida la cookie previa");
        let nueva = autenticar(&pool, SECRETO, 60, "ruiz@x.co", "clave123").await.unwrap();
        assert!(usuario_de(&estado, &con_cookie(&nueva)).await.is_some(), "el login posterior a la revocación vale");
    }

    /// Discrimina: revocar al usuario 2 no toca la sesión del 4.
    #[tokio::test]
    async fn revocar_a_uno_no_afecta_a_otro() {
        let pool = base().await;
        let h = claves::hashear("clave123").unwrap();
        sqlx::query("INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES (4,'Otro','otro@x.co',?,'medico',1,1)")
            .bind(&h).execute(&pool).await.unwrap();
        let estado = estado_de(pool.clone());
        let del_2 = sesion::emitir(SECRETO, 2, 0, 60);
        let del_4 = sesion::emitir(SECRETO, 4, 0, 60);

        assert!(medico_datos::usuario::revocar_sesiones(&pool, 2).await.unwrap());

        assert!(usuario_de(&estado, &con_cookie(&del_2)).await.is_none(), "la del 2 cae");
        assert!(usuario_de(&estado, &con_cookie(&del_4)).await.is_some(), "la del 4 sobrevive");
    }

    /// Directiva #8: bajo concurrencia, tras una revocación NINGUNA de 100 cookies
    /// previas sobrevive y las 100 nuevas entran — no hay ventana en la que una
    /// stale se cuele. Con timeout, para distinguir un interbloqueo de lentitud.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn cien_cookies_previas_caen_tras_revocar_bajo_concurrencia() {
        let pool = base().await;
        let estado = estado_de(pool.clone());
        // 100 cookies a la versión 0 (de fábrica) y 100 a la versión 1 (post-revocación).
        let previas: Vec<String> = (0..100).map(|_| sesion::emitir(SECRETO, 2, 0, 60)).collect();
        let nuevas: Vec<String> = (0..100).map(|_| sesion::emitir(SECRETO, 2, 1, 60)).collect();

        assert!(medico_datos::usuario::revocar_sesiones(&pool, 2).await.unwrap());

        let plazo = std::time::Duration::from_secs(60);
        let mut tareas = Vec::new();
        for c in previas {
            let e = estado.clone();
            tareas.push(tokio::spawn(async move { (false, usuario_de(&e, &con_cookie(&c)).await.is_some()) }));
        }
        for c in nuevas {
            let e = estado.clone();
            tareas.push(tokio::spawn(async move { (true, usuario_de(&e, &con_cookie(&c)).await.is_some()) }));
        }
        for t in tareas {
            let (deberia, entro) = tokio::time::timeout(plazo, t).await
                .expect("una verificación no respondió a tiempo: ¿interbloqueo?")
                .expect("la tarea no debía entrar en panic");
            assert_eq!(entro, deberia, "una cookie previa se coló o una nueva se rechazó");
        }
    }
}
