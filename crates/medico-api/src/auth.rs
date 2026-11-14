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
    /// Las credenciales que esta instalación publica en su pantalla de acceso,
    /// si —y solo si— el despliegue pidió ser una demostración. `None` en la
    /// instalación de un cliente, que es el defecto.
    pub demo: Option<crate::config::Demo>,
    /// Las matrices de capacidades por institución, ya leídas.
    ///
    /// Va en el estado y no se abre por petición porque `Autorizado<C>` la
    /// consulta en CADA ruta autorizada. `Arc` porque el estado se clona una vez
    /// por petición y la caché tiene que ser la misma para todas — una copia por
    /// petición no cachearía nada y, peor, no se enteraría de una invalidación.
    pub capacidades: std::sync::Arc<medico_datos::capacidad::Cache>,
    /// El entorno de atención (`EntornoAtencion`) que esta sede emite en cada RDA
    /// de encuentro (#603 fase 4). Config del prestador, ya validada contra el
    /// catálogo al arrancar; el productor del RDA la recibe como parámetro. Ver
    /// [`crate::config::Config::entorno_atencion`].
    pub entorno_atencion: String,
    /// La configuración del envío al MUV, resuelta UNA VEZ al arrancar. `None` =
    /// esta instalación todavía sube el archivo a mano, que es el caso normal.
    ///
    /// Va en el estado y no se lee del entorno dentro del manejador por dos
    /// razones, y la segunda es la que importa: el proceso anuncia al arrancar si
    /// el envío está configurado —un positivo de control, en vez de descubrirlo
    /// el día del cierre—, y la ruta se puede PROBAR contra un validador de
    /// mentira sin tocar variables globales del proceso de pruebas.
    pub muv: Option<std::sync::Arc<crate::muv::Config>>,
    /// Si el transporte del RDA al mecanismo IHCE (#603 F4.6) está configurado,
    /// resuelto UNA VEZ al arrancar (`ihce::config_desde_entorno().is_some()`).
    /// `false` = esta instalación todavía no tiene credenciales de sandbox, que es
    /// el caso normal.
    ///
    /// Misma decisión que [`muv`](Self::muv): la pantalla que EMITE el RD
    /// (`vista::rda`) lo consulta aquí y no en el entorno, para no encolar en
    /// silencio un Bundle que nadie va a drenar —un falso «enviado» (directiva 20)—
    /// y para poder PROBAR las dos ramas sin tocar variables globales. La tarea de
    /// fondo que de verdad envía la arranca `main` con el `ClienteIhce` real.
    pub ihce_configurado: bool,
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
/// **Devuelve QUIÉN entró, no su cookie**, y la separación es del segundo factor.
///
/// Hasta el 2026-08-12 esta función emitía la sesión y se llamaba `autenticar`.
/// Desde que hay DOS formas de token —la sesión y el **pendiente** del primer
/// paso—, «comprobar la contraseña» y «decidir qué credencial se emite» dejaron
/// de ser la misma decisión: quien llama necesita saber a quién le acaba de
/// abrir para preguntar si esa cuenta exige código. La envoltura que emitía la
/// cookie se quedó sin un solo llamador de producción y se quitó en vez de
/// silenciarla (directiva 1).
pub async fn autenticar_usuario(
    pool: &medico_datos::Pool,
    email: &str,
    clave: &str,
) -> Option<medico_datos::usuario::Usuario> {
    let encontrado = usuario::por_email(pool, email).await.ok().flatten();

    // El `Option` se resuelve ANTES de verificar, y las dos ramas verifican. Con
    // un `?` aquí volveríamos a tener dos caminos de duración distinta.
    let hash = match &encontrado {
        Some(u) if u.activo => u.password_hash.clone(),
        _ => claves::hash_de_descarte().to_string(),
    };

    // La CADENA de bachué reconoce el esquema del hash guardado por su forma, lo
    // verifica y —al acertar— decide la migración: un Argon2 débil se renueva a la
    // política de hoy (nunca se abarata), un hash de legado migra a Argon2id. Es la
    // absorción de tumbaga (fase 2, 2026-09-02): antes medico verificaba y NO
    // migraba nada al entrar.
    let cadena = bachue_password::Cadena::nueva();
    let resultado = match cadena.verificar(clave, &hash) {
        Ok(r) => r,
        Err(bachue_password::EsquemaDesconocido) => {
            // El hash guardado no lo reconoce NINGÚN esquema: es una AVERÍA de
            // datos (un esquema retirado sin migrar, un hash corrupto), no un
            // intento fallido (directiva 20). Se registra —un usuario legítimo
            // quedó fuera y hay que restablecerle la contraseña— y se niega el
            // acceso. Pero antes se gasta un Argon2 contra el descarte: sin esto,
            // una cuenta con hash ilegible respondería más rápido que una sana y
            // ese reloj diría qué correos existen (el mismo oráculo que el
            // hash_de_descarte cierra para el correo inexistente).
            if let Some(u) = &encontrado
                && u.activo
            {
                eprintln!(
                    "⚠️  usuario {}: el hash guardado no lo reconoce ningún esquema de \
                     la cadena; no puede entrar hasta restablecer la contraseña",
                    u.id
                );
            }
            let _ = cadena.verificar(clave, claves::hash_de_descarte());
            return None;
        }
    };

    match encontrado {
        Some(u) if u.activo && resultado.valida => {
            // Compare-and-swap: migrar el hash SOLO si en la base sigue siendo el
            // que se verificó. Cero filas afectadas ⇒ la contraseña cambió
            // entretanto (otra pestaña, un restablecimiento tras una fuga): NO se
            // pisa —reescribir `Argon2(clave_vieja)` encima de `Hash(clave_nueva)`
            // revertiría la rotación en silencio—. Es best-effort: si el UPDATE
            // falla, el login entra igual y la migración se reintenta al próximo
            // acceso. NO sube `token_version`: no es un cambio de contraseña, es la
            // misma escrita mejor (las sesiones vivas siguen valiendo).
            if let Some(nuevo) = resultado.hash_nuevo
                && let Err(err) = usuario::migrar_hash(pool, u.id, &hash, &nuevo).await
            {
                eprintln!("⚠️  no se pudo migrar el hash del usuario {} al entrar: {err}", u.id);
            }
            Some(u)
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
            // El gate de #538 también cierra la API: un usuario cuya contraseña
            // restableció el administrador (`requiere_cambio_password`) no puede
            // usar NINGUNA ruta `/api/*` hasta cambiarla —si no, firmaría por
            // `/api/historias/{id}/firmar` lo mismo que el panel le impide—. La
            // cambia por `/panel/cambiar-clave`, que usa `EnSesion` y no este
            // extractor, así que no queda encerrado. El check NO va en
            // `usuario_de` a propósito: lo comparte el panel, y ahí gatearía
            // también la pantalla de cambio.
            Some(u) if u.requiere_cambio_password => Err(StatusCode::FORBIDDEN),
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

/// Administrar la institución: ficha, configuración, política de seguridad,
/// campos dinámicos y maestros.
pub struct AdministrarInstitucion;
impl Exige for AdministrarInstitucion {
    const QUE: medico_dominio::rol::Capacidad =
        medico_dominio::rol::Capacidad::AdministrarInstitucion;
}

/// Atender en urgencias y hospitalización: triage, ingreso, egreso.
pub struct AtenderUrgencias;
impl Exige for AtenderUrgencias {
    const QUE: medico_dominio::rol::Capacidad =
        medico_dominio::rol::Capacidad::AtenderUrgenciasHospitalizacion;
}

/// La nota de enfermería, el plan de cuidados y la administración de
/// medicamentos. Capacidad PROPIA: `EscribirClinico` es médico, y con ella una
/// enfermera no podía escribir su propia nota.
pub struct EscribirEnfermeria;
impl Exige for EscribirEnfermeria {
    const QUE: medico_dominio::rol::Capacidad = medico_dominio::rol::Capacidad::EscribirEnfermeria;
}

/// La nota de trabajo social. Capacidad propia, como enfermería.
pub struct EscribirTrabajoSocial;
impl Exige for EscribirTrabajoSocial {
    const QUE: medico_dominio::rol::Capacidad =
        medico_dominio::rol::Capacidad::EscribirTrabajoSocial;
}

/// Valoraciones y planes de nutrición.
pub struct EscribirNutricion;
impl Exige for EscribirNutricion {
    const QUE: medico_dominio::rol::Capacidad = medico_dominio::rol::Capacidad::EscribirNutricion;
}

/// Planes y sesiones de rehabilitación.
pub struct EscribirRehabilitacion;
impl Exige for EscribirRehabilitacion {
    const QUE: medico_dominio::rol::Capacidad =
        medico_dominio::rol::Capacidad::EscribirRehabilitacion;
}

/// Remisiones y contrarreferencia.
pub struct GestionarRemision;
impl Exige for GestionarRemision {
    const QUE: medico_dominio::rol::Capacidad = medico_dominio::rol::Capacidad::GestionarRemision;
}

/// Resultados de laboratorio y perfiles de examen.
pub struct EscribirLaboratorio;
impl Exige for EscribirLaboratorio {
    const QUE: medico_dominio::rol::Capacidad = medico_dominio::rol::Capacidad::EscribirLaboratorio;
}

/// Informes de imágenes diagnósticas.
pub struct EscribirImagenologia;
impl Exige for EscribirImagenologia {
    const QUE: medico_dominio::rol::Capacidad =
        medico_dominio::rol::Capacidad::EscribirImagenologia;
}

/// Definir la agenda de un profesional: meses, franjas y bloqueos. **Leer** los
/// turnos libres NO la exige: quien agenda desde el mostrador tiene que verlos.
pub struct GestionarAgenda;
impl Exige for GestionarAgenda {
    const QUE: medico_dominio::rol::Capacidad = medico_dominio::rol::Capacidad::GestionarAgenda;
}

/// Generar el RIPS de un periodo: el archivo con el que se factura.
pub struct GenerarRips;
impl Exige for GenerarRips {
    const QUE: medico_dominio::rol::Capacidad = medico_dominio::rol::Capacidad::GenerarRips;
}

/// Parque de ambulancias y despacho de traslados.
pub struct GestionarAmbulancias;
impl Exige for GestionarAmbulancias {
    const QUE: medico_dominio::rol::Capacidad =
        medico_dominio::rol::Capacidad::GestionarAmbulancias;
}

/// Farmacia: alta de items, entradas de stock y dispensación. **Leer** el
/// inventario no la exige: basta estar autenticado.
pub struct GestionarFarmacia;
impl Exige for GestionarFarmacia {
    const QUE: medico_dominio::rol::Capacidad = medico_dominio::rol::Capacidad::GestionarFarmacia;
}

/// Facturar: contratos, facturas, sus líneas y las glosas.
///
/// La exigen las ESCRITURAS y no las lecturas, igual que en farmacia: quien
/// consulta en ventanilla el estado de una factura para responderle al paciente
/// no tiene por qué poder radicarla ni anularla.
pub struct GestionarFacturacion;
impl Exige for GestionarFacturacion {
    const QUE: medico_dominio::rol::Capacidad =
        medico_dominio::rol::Capacidad::GestionarFacturacion;
}

/// Dar de alta al personal y decidir quién puede qué. Es la MISMA capacidad a
/// propósito: partirlas crearía la figura del administrador que nombra pero no
/// autoriza, que no existe en ninguna IPS.
///
/// Que esta capacidad se consulte contra la matriz —igual que las demás— es lo
/// que la hace configurable, y también lo que obliga al guardarraíl de
/// [`medico_dominio::ajustes::Ajustes::validar`]: dejarla sin ningún rol tapia
/// la puerta desde la que se arregla todo lo demás.
pub struct GestionarUsuarios;
impl Exige for GestionarUsuarios {
    const QUE: medico_dominio::rol::Capacidad = medico_dominio::rol::Capacidad::GestionarUsuarios;
}

/// Crear instituciones, fijarles licencia y decidir qué módulos compraron.
///
/// Vive POR ENCIMA de la institución y por eso `Capacidad::GestionarInstituciones`
/// no se puede configurar desde la matriz de una: un administrador que se la
/// concediera estaría regalándose el producto entero. En la práctica es la
/// capacidad del vendedor, no la del cliente.
pub struct GestionarInstituciones;
impl Exige for GestionarInstituciones {
    const QUE: medico_dominio::rol::Capacidad =
        medico_dominio::rol::Capacidad::GestionarInstituciones;
}

impl<C: Exige> FromRequestParts<Estado> for Autorizado<C> {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, estado: &Estado) -> Result<Self, StatusCode> {
        let usuario = usuario_de(estado, &parts.headers).await.ok_or(StatusCode::UNAUTHORIZED)?;
        // Igual que en `Autenticado`: con la contraseña por restablecer, ninguna
        // ruta de la API hasta cambiarla (#538). El panel usa otro extractor.
        if usuario.requiere_cambio_password {
            return Err(StatusCode::FORBIDDEN);
        }
        let Some(rol) = usuario.rol() else {
            return Err(StatusCode::FORBIDDEN);
        };

        // El defecto nacional, salvo que la institución del usuario lo haya
        // apartado. Quien no pertenece a ninguna —el superadmin— no tiene
        // matriz que consultar: su respuesta es el defecto, y esa es justo la
        // razón de que se le deje fuera de la matriz (ver
        // `RolUsuario::configurable_por_institucion`).
        let permitido = match usuario.institucion_id {
            Some(institucion_id) => {
                // Si la matriz no se puede leer, la petición FALLA. Caer al
                // defecto sería devolverle al rol justo lo que esta institución
                // le había quitado, y sin que nadie se entere (directiva 20).
                // 503 y no 403: el usuario no tiene nada que corregir.
                let ajustes = estado
                    .capacidades
                    .ajustes(&estado.pool, institucion_id)
                    .await
                    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
                ajustes.puede(rol, C::QUE)
            }
            None => rol.puede(C::QUE),
        };

        // 403 y no 404: dentro de la propia institución la existencia del recurso
        // no es secreto —el usuario puede listarlo—, y decirle «no existe» cuando
        // lo que pasa es que no le corresponde manda a buscar un fallo que no hay.
        // El 404 sigue siendo para lo AJENO, que es donde sí se oculta.
        if permitido {
            Ok(Autorizado(usuario, std::marker::PhantomData))
        } else {
            Err(StatusCode::FORBIDDEN)
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
        let u = autenticar_usuario(&pool, "ruiz@x.co", "clave123").await.expect("la clave era buena");
        assert_eq!(u.id, 2);
        // Y la cookie que se emite con ese usuario es una sesión válida suya.
        let cookie = sesion::emitir(SECRETO, u.id, u.token_version, 60);
        assert_eq!(sesion::verificar(SECRETO, &cookie).unwrap().usuario_id, 2);
    }

    #[tokio::test]
    async fn la_clave_incorrecta_no_entra() {
        let pool = base().await;
        assert!(autenticar_usuario(&pool, "ruiz@x.co", "otra").await.is_none());
    }

    /// Construye un hash Argon2id DÉBIL (mucho menos trabajo que la política de
    /// hoy) para los casos de migración. `argon2` directo A PROPÓSITO: es un arnés
    /// de prueba, no producción —el hash real lo hace bachué/guaca—.
    fn hash_argon2_debil(clave: &str) -> String {
        use argon2::password_hash::{PasswordHasher, SaltString};
        use argon2::{Algorithm, Argon2, Params, Version};
        let sal = SaltString::encode_b64(&[3u8; 16]).expect("sal de prueba");
        Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::new(8, 1, 1, None).unwrap())
            .hash_password(clave.as_bytes(), &sal)
            .expect("hash débil")
            .to_string()
    }

    async fn hash_guardado(pool: &medico_datos::Pool, id: i64) -> String {
        sqlx::query_scalar::<_, String>("SELECT password_hash FROM usuario WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn insertar_con_hash(pool: &medico_datos::Pool, id: i64, email: &str, hash: &str) {
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) \
             VALUES (?,'Prueba',?,?,'medico',1,1)",
        )
        .bind(id)
        .bind(email)
        .bind(hash)
        .execute(pool)
        .await
        .unwrap();
    }

    /// **La absorción de tumbaga, de punta a punta (directiva 28).** Un usuario con
    /// un hash Argon2 DÉBIL entra con su clave, y AL ENTRAR el hash se migra en la
    /// base a la política de hoy —sin que el usuario cambie nada—; el hash migrado
    /// sigue dejándolo entrar y ya no se vuelve a tocar. Antes de la fase 2 medico
    /// verificaba y NO migraba: el débil se quedaba débil para siempre.
    #[tokio::test]
    async fn un_hash_debil_se_migra_al_entrar_y_el_migrado_sirve() {
        let pool = base().await;
        let debil = hash_argon2_debil("clave-larga-de-prueba");
        insertar_con_hash(&pool, 5, "debil@x.co", &debil).await;

        let u = autenticar_usuario(&pool, "debil@x.co", "clave-larga-de-prueba")
            .await
            .expect("entra con su clave");
        assert_eq!(u.id, 5);

        let guardado = hash_guardado(&pool, 5).await;
        assert_ne!(guardado, debil, "el hash débil se reescribió al entrar");
        assert!(guardado.starts_with("$argon2"), "migró a Argon2id: {guardado}");

        // El migrado deja entrar con la MISMA clave y ya está al día: no se re-migra.
        let u2 = autenticar_usuario(&pool, "debil@x.co", "clave-larga-de-prueba")
            .await
            .expect("entra con el hash migrado");
        assert_eq!(u2.id, 5);
        assert_eq!(
            hash_guardado(&pool, 5).await,
            guardado,
            "el migrado ya está al día: no se vuelve a tocar"
        );
    }

    /// PAREJA discriminante (directiva 33): una clave MALA contra el mismo hash
    /// débil no entra y NO migra. Sin el guard de validez, el intento del atacante
    /// reescribiría el hash (una migración disparada por una clave incorrecta).
    #[tokio::test]
    async fn una_clave_mala_contra_un_hash_debil_no_entra_ni_migra() {
        let pool = base().await;
        let debil = hash_argon2_debil("la-correcta-de-verdad");
        insertar_con_hash(&pool, 5, "debil@x.co", &debil).await;

        assert!(
            autenticar_usuario(&pool, "debil@x.co", "la-incorrecta").await.is_none(),
            "una clave mala no entra"
        );
        assert_eq!(
            hash_guardado(&pool, 5).await,
            debil,
            "una clave mala NUNCA reescribe el hash"
        );
    }

    /// Un hash ya AL DÍA (el que produce el propio sistema) no se toca al entrar:
    /// no hay migración que hacer, la base no cambia. Es la pareja del test del
    /// débil: juntos discriminan un "migra siempre" de un "migra solo si hace falta".
    #[tokio::test]
    async fn un_hash_al_dia_no_se_reescribe_al_entrar() {
        let pool = base().await; // el usuario 2 tiene un hash al día ("clave123")
        let antes = hash_guardado(&pool, 2).await;
        let u = autenticar_usuario(&pool, "ruiz@x.co", "clave123").await.expect("entra");
        assert_eq!(u.id, 2);
        assert_eq!(
            hash_guardado(&pool, 2).await,
            antes,
            "un hash al día no se reescribe al entrar"
        );
    }

    /// Un hash que NINGÚN esquema de la cadena reconoce niega el acceso —es una
    /// AVERÍA de datos, no una clave mala (directiva 20)— y no se confunde con un
    /// éxito. Pareja: un hash sano con su clave sí entra, así que no es un "niega
    /// a todos".
    #[tokio::test]
    async fn un_hash_que_ningun_esquema_reconoce_no_deja_entrar() {
        let pool = base().await;
        insertar_con_hash(&pool, 5, "roto@x.co", "no-es-un-hash-de-ningun-esquema").await;
        assert!(
            autenticar_usuario(&pool, "roto@x.co", "lo-que-sea").await.is_none(),
            "un hash que ningún esquema reconoce no deja entrar"
        );
        assert!(
            autenticar_usuario(&pool, "ruiz@x.co", "clave123").await.is_some(),
            "un hash sano con su clave sí entra (no es un «niega a todos»)"
        );
    }

    /// **La rama del hash ILEGIBLE cuesta lo mismo que un login normal
    /// (anti-timing, directiva 33).** Un hash que ningún esquema reconoce se niega,
    /// pero ANTES gasta un Argon2 contra el descarte (`auth.rs`, rama
    /// `EsquemaDesconocido`) para no responder más rápido que una cuenta sana —si
    /// lo hiciera, el reloj diría qué cuentas tienen un hash ilegible—.
    ///
    /// Esta prueba es la PAREJA de esa línea, y NACE porque el concilio la reclamó:
    /// dos revisores (Sonnet 5 y Fable 5.1, 2026-09-02) mutaron ese `let _ =
    /// cadena.verificar(clave, hash_de_descarte())` y el banco siguió VERDE —nadie
    /// medía el tiempo de esa rama, así que un «limpiar código muerto» reabriría el
    /// oráculo en silencio—. Con ella, quitar esa línea deja la rama en microsegundos
    /// y la aserción se rompe. Margen amplio (40 %) y mejor de 3 corridas, como
    /// `bachue_clave::un_correo_inexistente_cuesta_lo_mismo_que_uno_existente`, para
    /// no ser frágil ante el ruido del planificador (en debug el ruido es grande).
    #[tokio::test]
    async fn un_hash_ilegible_cuesta_lo_mismo_que_un_login_normal() {
        use std::time::Instant;
        let pool = base().await; // usuario 2: hash al día
        insertar_con_hash(&pool, 5, "roto@x.co", "no-es-un-hash-de-ningun-esquema").await;
        // Calienta la generación perezosa del hash de descarte, fuera de la medición.
        let _ = autenticar_usuario(&pool, "nadie@x.co", "x").await;

        // Los dos caminos verifican con clave MALA (ninguno entra); lo que se mide es
        // el TIEMPO. El normal corre un Argon2 sobre el hash sano; el ilegible corre
        // un Argon2 sobre el descarte. Deben costar lo mismo.
        let mut normal = u128::MAX;
        for _ in 0..3 {
            let t = Instant::now();
            assert!(autenticar_usuario(&pool, "ruiz@x.co", "clave-mala").await.is_none());
            normal = normal.min(t.elapsed().as_micros());
        }
        let mut ilegible = u128::MAX;
        for _ in 0..3 {
            let t = Instant::now();
            assert!(autenticar_usuario(&pool, "roto@x.co", "clave-mala").await.is_none());
            ilegible = ilegible.min(t.elapsed().as_micros());
        }
        assert!(
            ilegible * 100 >= normal * 40,
            "la rama del hash ilegible ({ilegible} µs) es demasiado rápida frente al \
             login normal ({normal} µs): ¿se saltó el Argon2 de descarte?"
        );
    }

    #[tokio::test]
    async fn el_correo_que_no_existe_da_lo_mismo() {
        let pool = base().await;
        assert!(autenticar_usuario(&pool, "nadie@x.co", "clave123").await.is_none());
    }

    #[tokio::test]
    async fn el_inactivo_no_entra_aunque_la_clave_sea_correcta() {
        let pool = base().await;
        assert!(autenticar_usuario(&pool, "ex@x.co", "clave123").await.is_none());
    }

    #[tokio::test]
    async fn usuario_de_una_cookie_valida_es_el_usuario() {
        let pool = base().await;
        let estado = Estado { pool, secreto: SECRETO.to_vec(), minutos: 60, dir_imagenes: std::env::temp_dir(), clave_firma: None, clave_verificacion: None, clave_reposo: None, tras_proxy: false, demo: None, entorno_atencion: "05".into(), capacidades: std::sync::Arc::new(medico_datos::capacidad::Cache::nueva()), muv: None, ihce_configurado: false };
        let cookie = sesion::emitir(SECRETO, 2, 0, 60);
        let mut cabeceras = HeaderMap::new();
        cabeceras.insert(header::COOKIE, format!("medico_sesion={cookie}").parse().unwrap());
        let u = usuario_de(&estado, &cabeceras).await.unwrap();
        assert_eq!(u.id, 2);
    }

    #[tokio::test]
    async fn sin_cookie_no_hay_usuario() {
        let pool = base().await;
        let estado = Estado { pool, secreto: SECRETO.to_vec(), minutos: 60, dir_imagenes: std::env::temp_dir(), clave_firma: None, clave_verificacion: None, clave_reposo: None, tras_proxy: false, demo: None, entorno_atencion: "05".into(), capacidades: std::sync::Arc::new(medico_datos::capacidad::Cache::nueva()), muv: None, ihce_configurado: false };
        assert!(usuario_de(&estado, &HeaderMap::new()).await.is_none());
    }

    /// Una cookie firmada con OTRO secreto no vale — el token no basta, hay que
    /// haberlo firmado con el secreto del servidor.
    #[tokio::test]
    async fn una_cookie_de_otro_secreto_no_entra() {
        let pool = base().await;
        let estado = Estado { pool, secreto: SECRETO.to_vec(), minutos: 60, dir_imagenes: std::env::temp_dir(), clave_firma: None, clave_verificacion: None, clave_reposo: None, tras_proxy: false, demo: None, entorno_atencion: "05".into(), capacidades: std::sync::Arc::new(medico_datos::capacidad::Cache::nueva()), muv: None, ihce_configurado: false };
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
            demo: None,
            entorno_atencion: "05".into(),
            capacidades: std::sync::Arc::new(medico_datos::capacidad::Cache::nueva()),
            muv: None,
            ihce_configurado: false,
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
        let pu = autenticar_usuario(&pool, "ruiz@x.co", "clave123").await.unwrap();
        let previa = sesion::emitir(SECRETO, pu.id, pu.token_version, 60);
        assert!(usuario_de(&estado, &con_cookie(&previa)).await.is_some(), "válida antes de revocar");

        assert!(medico_datos::usuario::revocar_sesiones(&pool, 2).await.unwrap());

        assert!(usuario_de(&estado, &con_cookie(&previa)).await.is_none(), "la revocación invalida la cookie previa");
        let nu = autenticar_usuario(&pool, "ruiz@x.co", "clave123").await.unwrap();
        let nueva = sesion::emitir(SECRETO, nu.id, nu.token_version, 60);
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
