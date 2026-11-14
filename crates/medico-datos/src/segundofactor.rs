//! El segundo factor: aquí las columnas, la regla en guaca.
//!
//! # Qué vive aquí y qué no
//!
//! Igual que [`crate::freno`], y por la misma razón. La REGLA —cuántas ventanas
//! se toleran, qué cuenta como reutilización, qué se hace con un secreto
//! ilegible— vive en [`guaca::dosfactores`], que subió desde informes el
//! 2026-08-12 justo porque medico es el segundo proyecto que la necesita
//! (directiva 9). Aquí queda lo que de verdad es de medico: dónde se guarda el
//! secreto, cómo se quema el contador y dónde viven los códigos de recuperación.
//!
//! # El secreto NO se guarda en claro, y por eso hace falta la frase de reposo
//!
//! Se envuelve con [`guaca::reposo`] usando `MEDICO_STORAGE_KEY`. Sin esa frase
//! el alta **se niega a empezar** en vez de guardar el secreto tal cual: un
//! secreto TOTP en claro dentro de la base es un adorno frente al único atacante
//! que importa aquí, el que ya se llevó el archivo. Y en medico ese archivo es
//! una historia clínica.
//!
//! # Los tres estados de una cuenta, y por qué son tres
//!
//! `sin_secreto` → `enrolando` (hay secreto, no hay confirmación) → `activo`.
//! El de en medio existe porque entre generar el secreto y comprobar que la
//! persona lo leyó bien hay un paso, y quien cierre la pantalla del QR a medias
//! no puede quedarse fuera de su propia cuenta (directiva 32: el secreto y la
//! vigencia son dos hechos, no uno).

use chrono::{DateTime, Utc};

use crate::Pool;

/// La regla se reexporta para que quien la use no tenga que saber dónde vive.
/// Lo que NO se hace es copiar sus valores: son los de guaca o no son.
pub use guaca::dosfactores::{
    generar_codigos_recuperacion, generar_secreto, parece_codigo_de_recuperacion, uri_otpauth,
    verificar, Veredicto, DIGITOS, PASO_S, VENTANAS_TOLERADAS,
};

/// Cuántos códigos de recuperación se entregan en el enrolamiento.
///
/// Diez, como informes. Es un número de conveniencia y no de seguridad —cada uno
/// son 80 bits—: bastan para varios años de teléfonos perdidos sin convertir la
/// hoja impresa en un listado que nadie guarda entero.
pub const CODIGOS_DE_RECUPERACION: usize = 10;

/// En qué punto está una cuenta respecto del segundo factor.
///
/// **Tres variantes y no un booleano.** `Enrolando` no es «activo» —el acceso no
/// pide código todavía— pero tampoco es «sin secreto»: hay uno guardado que hay
/// que sustituir si se vuelve a empezar. Colapsarlos dejaría a quien cerró la
/// pantalla del QR con un secreto huérfano que nada limpia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Situacion {
    /// No hay secreto: el acceso es de un solo paso.
    SinSecreto,
    /// Hay secreto y todavía no se confirmó. El acceso sigue siendo de un paso.
    Enrolando,
    /// Confirmado en esa fecha. El acceso pide código.
    Activo { desde: String },
}

impl Situacion {
    /// ¿El acceso tiene que pedir un segundo paso? Solo si está `Activo`.
    ///
    /// Existe como método y no como comparación suelta para que la pregunta se
    /// escriba UNA vez: repartida por los manejadores, el día que aparezca una
    /// cuarta variante habrá sitios que la traten como activa y sitios que no.
    pub fn exige_codigo(&self) -> bool {
        matches!(self, Situacion::Activo { .. })
    }
}

/// Las tres columnas tal y como salen de la base: el blob cifrado, la marca de
/// confirmación y el último contador. Tiene nombre para que la firma se lea —y
/// porque clippy tiene razón: una tupla de tres `Option` anidados no dice cuál
/// es cuál en el sitio donde importa.
type FilaCruda = (Option<Vec<u8>>, Option<String>, Option<i64>);

/// Lo que hay guardado de una cuenta, ya descifrado.
pub struct Guardado {
    pub situacion: Situacion,
    /// El secreto en base32, o `None` si no hay o no se pudo descifrar.
    pub secreto: Option<String>,
    pub ultimo_contador: Option<i64>,
}

/// En qué punto está la cuenta. No descifra nada: es la pregunta barata que hace
/// el primer paso del acceso para saber si hay segundo.
pub async fn situacion(pool: &Pool, usuario_id: i64) -> Result<Situacion, sqlx::Error> {
    let fila: Option<(Option<Vec<u8>>, Option<String>)> =
        sqlx::query_as("SELECT totp_secreto_cifrado, totp_confirmado_en FROM usuario WHERE id = ?")
            .bind(usuario_id)
            .fetch_optional(pool)
            .await?;
    Ok(match fila {
        Some((Some(_), Some(desde))) => Situacion::Activo { desde },
        Some((Some(_), None)) => Situacion::Enrolando,
        _ => Situacion::SinSecreto,
    })
}

/// El secreto descifrado y el último contador, para verificar un código.
///
/// **Un secreto que no se deja descifrar sale como `None`, no como error.** Es
/// la misma distinción que hace [`Veredicto::SecretoIlegible`]: una frase de
/// reposo rotada o un blob alterado no es un fallo de quien teclea, y tratarlo
/// como «código incorrecto» dejaría a esa persona probando contra una avería del
/// servidor hasta que el freno la parase.
pub async fn guardado(
    pool: &Pool,
    usuario_id: i64,
    clave_reposo: Option<&str>,
) -> Result<Guardado, sqlx::Error> {
    let fila: Option<FilaCruda> = sqlx::query_as(
        "SELECT totp_secreto_cifrado, totp_confirmado_en, totp_ultimo_contador \
         FROM usuario WHERE id = ?",
    )
    .bind(usuario_id)
    .fetch_optional(pool)
    .await?;

    let Some((cifrado, confirmado, contador)) = fila else {
        return Ok(Guardado {
            situacion: Situacion::SinSecreto,
            secreto: None,
            ultimo_contador: None,
        });
    };

    let situacion = match (&cifrado, &confirmado) {
        (Some(_), Some(d)) => Situacion::Activo { desde: d.clone() },
        (Some(_), None) => Situacion::Enrolando,
        _ => Situacion::SinSecreto,
    };

    let secreto = match (cifrado, clave_reposo) {
        (Some(blob), Some(frase)) => guaca::reposo::descifrar(&blob, frase)
            .ok()
            .and_then(|b| String::from_utf8(b).ok()),
        _ => None,
    };

    Ok(Guardado {
        situacion,
        secreto,
        ultimo_contador: contador,
    })
}

/// Guarda un secreto nuevo y deja la cuenta ENROLANDO.
///
/// Borra la confirmación y el contador a propósito: volver a empezar el
/// enrolamiento tiene que invalidar el factor anterior entero. Dejar el contador
/// viejo con un secreto nuevo haría que los primeros códigos del teléfono nuevo
/// salieran «reutilizados».
///
/// Devuelve `Err` si no hay frase de reposo: el secreto NO se guarda en claro
/// «mientras tanto» (directiva 20).
pub async fn iniciar_enrolamiento(
    pool: &Pool,
    usuario_id: i64,
    secreto_b32: &str,
    clave_reposo: Option<&str>,
) -> Result<(), NoSePudoEnrolar> {
    let Some(frase) = clave_reposo else {
        return Err(NoSePudoEnrolar::SinReposo);
    };
    let blob = guaca::reposo::cifrar(secreto_b32.as_bytes(), frase);
    sqlx::query(
        "UPDATE usuario SET totp_secreto_cifrado = ?, totp_confirmado_en = NULL, \
         totp_ultimo_contador = NULL WHERE id = ?",
    )
    .bind(blob)
    .bind(usuario_id)
    .execute(pool)
    .await
    .map_err(NoSePudoEnrolar::Base)?;
    Ok(())
}

/// Por qué no se pudo empezar el enrolamiento. **Dos variantes y no una**.
///
/// La primera versión devolvía un `SinReposo` a secas y mapeaba el error de
/// `sqlx` a ESE MISMO valor, así que una base caída se codificaba como «falta
/// `MEDICO_STORAGE_KEY`». El tipo mentía sobre lo que llevaba dentro, y las
/// cuatro funciones hermanas de este módulo —`confirmar`, `desactivar`,
/// `guardar_codigos`, `quemar_contador`— ya devuelven `sqlx::Error` sin
/// disfrazarlo. Lo señaló la revisión en la nube del 2026-08-12.
///
/// Hoy el único llamador manda las dos al mismo sitio, así que nada se ve por
/// fuera; se separan igualmente porque el día que una pantalla quiera decir
/// «configure la frase» frente a «vuelva a intentarlo», el dato tiene que estar.
#[derive(Debug)]
pub enum NoSePudoEnrolar {
    /// No hay `MEDICO_STORAGE_KEY`: el secreto no se guarda en claro.
    SinReposo,
    /// La base no respondió. **No es un problema de configuración.**
    Base(sqlx::Error),
}

/// Da por confirmado el enrolamiento y quema el primer contador.
pub async fn confirmar(
    pool: &Pool,
    usuario_id: i64,
    contador: u64,
    ahora: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE usuario SET totp_confirmado_en = ?, totp_ultimo_contador = ? \
         WHERE id = ? AND totp_secreto_cifrado IS NOT NULL",
    )
    .bind(ahora.to_rfc3339())
    .bind(contador as i64)
    .bind(usuario_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Quema el contador, y **es aquí donde se decide si el código vale**.
///
/// Devuelve `true` solo si esta petición fue la que lo quemó.
///
/// # Por qué la comprobación va DENTRO del `UPDATE`
///
/// Es el hallazgo de la revisión de seguridad de informes, y la única razón por
/// la que este módulo no lo repite. La forma obvia —leer el contador,
/// verificarlo con [`verificar`], escribir el nuevo— deja entre la lectura y la
/// escritura el turno del semáforo y un Argon2id: cientos de milisegundos. Con
/// eso, **tres peticiones simultáneas con el mismo código abren tres sesiones**
/// (medido allí: `[200,200,200]`).
///
/// Poniendo `totp_ultimo_contador < ?` en el `WHERE` y mirando `rows_affected`,
/// la carrera la resuelve la base: gana una y las otras dos ven cero filas. Es
/// el mismo patrón que `usado_en IS NULL` en los códigos de recuperación.
pub async fn quemar_contador(
    pool: &Pool,
    usuario_id: i64,
    contador: u64,
) -> Result<bool, sqlx::Error> {
    let filas = sqlx::query(
        "UPDATE usuario SET totp_ultimo_contador = ? \
         WHERE id = ? AND (totp_ultimo_contador IS NULL OR totp_ultimo_contador < ?)",
    )
    .bind(contador as i64)
    .bind(usuario_id)
    .bind(contador as i64)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(filas == 1)
}

/// Emite el número de un pendiente nuevo, invalidando el anterior.
///
/// Devuelve el valor que va DENTRO del token. Al incrementar, cualquier
/// pendiente vivo de esa cuenta deja de valer: solo hay uno a la vez, que es lo
/// que una persona necesita y lo que un atacante no puede acumular.
pub async fn abrir_pendiente(pool: &Pool, usuario_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query("UPDATE usuario SET pendiente_version = pendiente_version + 1 WHERE id = ?")
        .bind(usuario_id)
        .execute(pool)
        .await?;
    sqlx::query_scalar("SELECT pendiente_version FROM usuario WHERE id = ?")
        .bind(usuario_id)
        .fetch_one(pool)
        .await
}

/// **Consume** un pendiente. `true` solo si esta petición fue la que lo gastó.
///
/// # Por qué esto tenía que existir
///
/// Borrar la cookie con `Max-Age=0` no consume nada: lo obedece un navegador y
/// lo ignora cualquier cliente que hable HTTP directamente, que reenvía el mismo
/// valor crudo con otro código hasta que caduca la firma. El «quemado» vivía en
/// el cliente, que es justo donde no puede vivir.
///
/// Se consume SIEMPRE que se presenta —valga el código o no—, y por eso se llama
/// antes de mirarlo: si solo se gastara al fallar, acertar dejaría el token vivo.
///
/// Mismo patrón que [`quemar_contador`]: el `WHERE` comprueba y el
/// `rows_affected` decide, así que dos peticiones simultáneas con el mismo
/// pendiente las resuelve la base y no el orden en que lleguen.
pub async fn consumir_pendiente(
    pool: &Pool,
    usuario_id: i64,
    version: i64,
) -> Result<bool, sqlx::Error> {
    let filas = sqlx::query(
        "UPDATE usuario SET pendiente_version = pendiente_version + 1 \
         WHERE id = ? AND pendiente_version = ?",
    )
    .bind(usuario_id)
    .bind(version)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(filas == 1)
}

/// Retira el segundo factor entero: secreto, confirmación, contador y códigos.
///
/// Los códigos de recuperación se BORRAN aquí, y es la única vez que se borran:
/// un código gastado conserva su fila para que quede constancia de que alguien
/// entró por ahí, pero cuando el factor se retira esos hashes ya no protegen
/// nada y dejarlos sería guardar credenciales vivas de una puerta que no existe.
pub async fn desactivar(pool: &Pool, usuario_id: i64) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE usuario SET totp_secreto_cifrado = NULL, totp_confirmado_en = NULL, \
         totp_ultimo_contador = NULL WHERE id = ?",
    )
    .bind(usuario_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM codigo_recuperacion WHERE usuario_id = ?")
        .bind(usuario_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

/// Sustituye los códigos de recuperación por una tanda nueva, ya hasheados.
///
/// Sustituye y no añade: entregar diez códigos nuevos sin invalidar los viejos
/// dejaría veinte puertas abiertas donde la pantalla anuncia diez.
pub async fn guardar_codigos(
    pool: &Pool,
    usuario_id: i64,
    hashes: &[String],
    ahora: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    // **`AND usado_en IS NULL`, y esa cláusula es de la Ley 1581.** Sin ella
    // este `DELETE` se llevaba también las filas ya gastadas, que son la ÚNICA
    // prueba en todo el sistema de que alguien entró por un código de
    // recuperación — la bitácora de `acceso` cubre el dato de paciente, no los
    // eventos de autenticación—. Y se disparaba en el camino NORMAL: cambiar de
    // teléfono pasa por aquí.
    //
    // Lo encontró la revisión en la nube del 2026-08-12, y lo que la hace grave
    // es que contradecía por escrito a esta misma tanda de trabajo: la migración
    // `0058` declara que borrar una fila con `usado_en` «perdería la única
    // prueba de que alguien entró por ahí», y el docstring de `desactivar` dice
    // que allí «es la ÚNICA vez que se borran». Las dos afirmaciones eran falsas
    // por esta línea.
    sqlx::query("DELETE FROM codigo_recuperacion WHERE usuario_id = ? AND usado_en IS NULL")
        .bind(usuario_id)
        .execute(&mut *tx)
        .await?;
    for h in hashes {
        sqlx::query(
            "INSERT INTO codigo_recuperacion (usuario_id, hash, creado_en) VALUES (?, ?, ?)",
        )
        .bind(usuario_id)
        .bind(h)
        .bind(ahora.to_rfc3339())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

/// Gasta un código de recuperación si alguno casa. `true` = entró por ahí.
///
/// # Dos cosas que no se hacen de la forma obvia
///
/// **Se prueban TODOS los sin gastar**, uno a uno, en vez de buscar por hash:
/// Argon2 lleva sal por fila, así que el hash de lo tecleado no es igual al
/// guardado y no hay `WHERE hash = ?` posible. Son diez verificaciones de
/// Argon2id en el peor caso — el mismo coste que un acceso normal, y solo lo
/// paga quien de verdad está usando un código de recuperación.
///
/// **El gasto va en un `UPDATE` con `usado_en IS NULL`**, mirando
/// `rows_affected`, por lo mismo que el contador TOTP: dos peticiones
/// simultáneas con el mismo código gastarían la misma fila dos veces y abrirían
/// dos sesiones.
///
/// # Lo que esto cuesta, medido y no estimado
///
/// Un Argon2id son **414 ms** en esta máquina (2026-08-12), así que entrar con
/// el último código de la lista cuesta **~4,1 s** y bloquea un hilo del runtime
/// ese rato: `guaca::claves::verificar` es CPU síncrona dentro de un `async fn`.
/// No es un patrón nuevo —`auth.rs:90` ya hace un Argon2 así en cada acceso, y
/// medico no usa `spawn_blocking` en ninguna parte—, pero aquí son hasta diez
/// seguidos en vez de uno, y conviene que el número esté escrito.
///
/// **Se conserva Argon2 a propósito**, aunque para un código de 80 bits
/// aleatorios un hash rápido bastaría —Argon2 existe para encarecer el ensayo de
/// secretos de BAJA entropía, no de éstos, y con SHA-256 esto sería un
/// `WHERE hash = ?` en vez de diez comparaciones—. Cambiarlo tocaría también a
/// `guaca` y a informes, y la razón de haber subido la regla a guaca es
/// justamente que las dos aplicaciones no diverjan. Queda como determinación
/// para decidir en los tres a la vez, no aquí por mi cuenta.
pub async fn gastar_codigo(
    pool: &Pool,
    usuario_id: i64,
    tecleado: &str,
    ahora: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    let filas: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, hash FROM codigo_recuperacion WHERE usuario_id = ? AND usado_en IS NULL",
    )
    .bind(usuario_id)
    .fetch_all(pool)
    .await?;

    let tecleado = tecleado.trim();
    for (id, hash) in filas {
        if guaca::claves::verificar(tecleado, &hash) {
            let gastadas = sqlx::query(
                "UPDATE codigo_recuperacion SET usado_en = ? WHERE id = ? AND usado_en IS NULL",
            )
            .bind(ahora.to_rfc3339())
            .bind(id)
            .execute(pool)
            .await?
            .rows_affected();
            return Ok(gastadas == 1);
        }
    }
    Ok(false)
}

/// Cuántos códigos de recuperación quedan sin gastar. Es lo que la pantalla
/// enseña para que nadie descubra que se le acabaron el día que pierde el
/// teléfono.
pub async fn codigos_sin_usar(pool: &Pool, usuario_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM codigo_recuperacion WHERE usuario_id = ? AND usado_en IS NULL",
    )
    .bind(usuario_id)
    .fetch_one(pool)
    .await
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Las pruebas de la REGLA pura —ventanas toleradas, secreto ilegible, la
    /// vida útil de 90 segundos— se fueron con ella a guaca, donde se corren una
    /// sola vez. Aquí se prueba lo que aquí se decide: que el secreto no toca el
    /// disco en claro, que el contador se quema de forma que una carrera no abra
    /// dos sesiones, y que un código de recuperación no se gasta dos veces.
    const FRASE: &str = "una frase de reposo de prueba, larga de sobra";

    async fn base() -> Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id) \
             VALUES (1, 'Quien Sea', 'a@b.co', 'x', 'MEDICO', NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn ahora() -> DateTime<Utc> {
        DateTime::from_timestamp(1_770_000_000, 0).unwrap()
    }

    /// Los tres estados, y que el de en medio EXISTE. Sin él, quien cierra la
    /// pantalla del QR a medias se queda fuera de su propia cuenta.
    #[tokio::test]
    async fn el_enrolamiento_a_medias_no_pide_codigo_todavia() {
        let pool = base().await;
        assert_eq!(situacion(&pool, 1).await.unwrap(), Situacion::SinSecreto);
        assert!(!situacion(&pool, 1).await.unwrap().exige_codigo());

        let s = generar_secreto();
        iniciar_enrolamiento(&pool, 1, &s, Some(FRASE)).await.unwrap();
        assert_eq!(situacion(&pool, 1).await.unwrap(), Situacion::Enrolando);
        assert!(
            !situacion(&pool, 1).await.unwrap().exige_codigo(),
            "con el secreto guardado y SIN confirmar, el acceso sigue siendo de un paso"
        );

        confirmar(&pool, 1, 59_000_000, ahora()).await.unwrap();
        assert!(situacion(&pool, 1).await.unwrap().exige_codigo());
    }

    /// **El secreto no toca el disco en claro.** El caso rojo de esto es leer la
    /// columna cruda y buscar el secreto dentro: si alguien sustituyera el
    /// cifrado por un `INSERT` directo, esta prueba lo dice.
    #[tokio::test]
    async fn el_secreto_se_guarda_cifrado_y_vuelve_igual() {
        let pool = base().await;
        let s = generar_secreto();
        iniciar_enrolamiento(&pool, 1, &s, Some(FRASE)).await.unwrap();

        let crudo: Vec<u8> = sqlx::query_scalar("SELECT totp_secreto_cifrado FROM usuario WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        let como_texto = String::from_utf8_lossy(&crudo).into_owned();
        assert!(
            !como_texto.contains(&s),
            "el secreto está EN CLARO en la base: {como_texto:?}"
        );

        // Y la pareja: con la frase correcta vuelve idéntico.
        let g = guardado(&pool, 1, Some(FRASE)).await.unwrap();
        assert_eq!(g.secreto.as_deref(), Some(s.as_str()));
    }

    /// Con la frase EQUIVOCADA no se descifra, y eso no es «código incorrecto»:
    /// es que el servidor no puede leer su propio secreto. Sale `None` para que
    /// el manejador lo distinga (`Veredicto::SecretoIlegible`).
    #[tokio::test]
    async fn una_frase_de_reposo_distinta_no_descifra_y_no_revienta() {
        let pool = base().await;
        iniciar_enrolamiento(&pool, 1, &generar_secreto(), Some(FRASE)).await.unwrap();
        let g = guardado(&pool, 1, Some("otra frase completamente distinta")).await.unwrap();
        assert_eq!(g.secreto, None);
        assert_eq!(g.situacion, Situacion::Enrolando, "la situación se sabe igual");
    }

    /// Sin frase de reposo NO se empieza, en vez de guardar el secreto en claro.
    #[tokio::test]
    async fn sin_frase_de_reposo_el_alta_se_niega_a_empezar() {
        let pool = base().await;
        assert!(matches!(
            iniciar_enrolamiento(&pool, 1, &generar_secreto(), None).await,
            Err(NoSePudoEnrolar::SinReposo)
        ));
        assert_eq!(
            situacion(&pool, 1).await.unwrap(),
            Situacion::SinSecreto,
            "no quedó ni medio secreto guardado"
        );
    }

    /// **LA PRUEBA DEL HALLAZGO DE INFORMES.** Cien peticiones simultáneas con el
    /// MISMO contador tienen que quemar UNA sola.
    ///
    /// Es la directiva 8 sobre el mecanismo que sostiene todo el antirreplay. Con
    /// la forma obvia —leer, verificar, escribir— allí se midieron tres sesiones
    /// abiertas con un solo código; aquí la carrera la resuelve la base.
    #[tokio::test]
    async fn cien_peticiones_con_el_mismo_codigo_queman_una_sola() {
        let pool = base().await;
        iniciar_enrolamiento(&pool, 1, &generar_secreto(), Some(FRASE)).await.unwrap();
        confirmar(&pool, 1, 59_000_000, ahora()).await.unwrap();

        let contador = 59_000_001;
        let mut tareas = Vec::new();
        for _ in 0..100 {
            let p = pool.clone();
            tareas.push(tokio::spawn(async move { quemar_contador(&p, 1, contador).await }));
        }
        let mut ganaron = 0;
        for t in tareas {
            if t.await.unwrap().unwrap() {
                ganaron += 1;
            }
        }
        assert_eq!(ganaron, 1, "{ganaron} peticiones quemaron el mismo contador");
    }

    /// La pareja de la anterior: un contador que AVANZA sí se quema, y uno igual
    /// o menor no. Sin esto, un `quemar_contador` que devolviera siempre `false`
    /// pasaría la prueba de la carrera y cerraría el acceso a todo el mundo.
    #[tokio::test]
    async fn el_contador_tiene_que_avanzar_y_uno_nuevo_si_entra() {
        let pool = base().await;
        iniciar_enrolamiento(&pool, 1, &generar_secreto(), Some(FRASE)).await.unwrap();

        assert!(quemar_contador(&pool, 1, 100).await.unwrap(), "el primero entra");
        assert!(!quemar_contador(&pool, 1, 100).await.unwrap(), "el mismo, no");
        assert!(!quemar_contador(&pool, 1, 99).await.unwrap(), "uno más viejo, tampoco");
        assert!(quemar_contador(&pool, 1, 101).await.unwrap(), "el siguiente sí");
    }

    /// Volver a empezar el enrolamiento invalida el factor anterior ENTERO. Si el
    /// contador viejo sobreviviera, los primeros códigos del teléfono nuevo
    /// saldrían «reutilizados» y nadie sabría por qué.
    #[tokio::test]
    async fn reenrolar_borra_la_confirmacion_y_el_contador() {
        let pool = base().await;
        iniciar_enrolamiento(&pool, 1, &generar_secreto(), Some(FRASE)).await.unwrap();
        confirmar(&pool, 1, 59_000_000, ahora()).await.unwrap();

        iniciar_enrolamiento(&pool, 1, &generar_secreto(), Some(FRASE)).await.unwrap();
        let g = guardado(&pool, 1, Some(FRASE)).await.unwrap();
        assert_eq!(g.situacion, Situacion::Enrolando);
        assert_eq!(g.ultimo_contador, None, "el contador viejo sobrevivió al reenrolamiento");
    }

    // ───────────────────────── códigos de recuperación ─────────────────────────

    async fn con_codigos(pool: &Pool) -> Vec<String> {
        let claros = generar_codigos_recuperacion(CODIGOS_DE_RECUPERACION);
        let hashes: Vec<String> = claros.iter().filter_map(|c| guaca::claves::hashear(c)).collect();
        assert_eq!(hashes.len(), CODIGOS_DE_RECUPERACION);
        guardar_codigos(pool, 1, &hashes, ahora()).await.unwrap();
        claros
    }

    #[tokio::test]
    async fn un_codigo_de_recuperacion_vale_una_vez_y_deja_rastro() {
        let pool = base().await;
        let claros = con_codigos(&pool).await;
        assert_eq!(codigos_sin_usar(&pool, 1).await.unwrap(), 10);

        assert!(gastar_codigo(&pool, 1, &claros[3], ahora()).await.unwrap());
        assert_eq!(codigos_sin_usar(&pool, 1).await.unwrap(), 9);
        assert!(
            !gastar_codigo(&pool, 1, &claros[3], ahora()).await.unwrap(),
            "el mismo código entró dos veces"
        );

        // La fila NO se borra: es la prueba de que alguien entró por ahí.
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM codigo_recuperacion WHERE usuario_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(total, 10, "gastar un código borró su rastro");
    }

    /// La pareja: uno que no existe no entra, y no gasta ninguno de los buenos.
    #[tokio::test]
    async fn un_codigo_inventado_no_entra_ni_gasta_los_demas() {
        let pool = base().await;
        con_codigos(&pool).await;
        assert!(!gastar_codigo(&pool, 1, "AAAAAAAA-BBBBBBBB", ahora()).await.unwrap());
        assert!(!gastar_codigo(&pool, 1, "", ahora()).await.unwrap());
        assert_eq!(codigos_sin_usar(&pool, 1).await.unwrap(), 10);
    }

    /// La carrera, también aquí: veinte peticiones con el MISMO código de
    /// recuperación gastan una sola fila.
    ///
    /// **Veinte y no cien, y el motivo es una medición, no una comodidad.** Cada
    /// intento cuesta un Argon2id, medido hoy en esta máquina: **414 ms**. Cien
    /// tareas en el runtime de `#[tokio::test]` —que es de UN hilo— son 41
    /// segundos de CPU serializada, y la primera versión de esta prueba no falló
    /// por la carrera sino con `PoolTimedOut` a los 51 s. Un banco que revienta
    /// por su propio arnés no mide lo que dice medir.
    ///
    /// Con `multi_thread` y veinte, la ventana de carrera se ejerce igual —basta
    /// con que dos coincidan— y el banco tarda un par de segundos. El caso de
    /// cien queda donde SÍ es barato y donde de verdad estaba el hallazgo: en
    /// `cien_peticiones_con_el_mismo_codigo_queman_una_sola`, que no lleva Argon2.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn veinte_peticiones_con_el_mismo_codigo_de_recuperacion_gastan_una() {
        let pool = base().await;
        let claros = con_codigos(&pool).await;
        let uno = claros[0].clone();

        let mut tareas = Vec::new();
        for _ in 0..20 {
            let (p, c) = (pool.clone(), uno.clone());
            tareas.push(tokio::spawn(async move { gastar_codigo(&p, 1, &c, ahora()).await }));
        }
        let mut ganaron = 0;
        for t in tareas {
            if t.await.unwrap().unwrap() {
                ganaron += 1;
            }
        }
        assert_eq!(ganaron, 1, "{ganaron} peticiones gastaron el mismo código");
        assert_eq!(codigos_sin_usar(&pool, 1).await.unwrap(), 9);
    }

    /// Entregar una tanda nueva invalida la anterior: diez puertas, no veinte.
    ///
    /// **Y NO SE LLEVA EL RASTRO DE LOS GASTADOS**, que es la mitad que esta
    /// prueba no medía. Su primera versión solo miraba que quedaran diez sin
    /// usar, y ese aserto pasa igual con el `DELETE` que borra todo y con el que
    /// respeta `usado_en` — las dos dejan diez. Lo destapó la revisión en la
    /// nube del 2026-08-12: faltaba sembrar un código GASTADO antes de la
    /// segunda tanda y exigir que su fila siga ahí.
    #[tokio::test]
    async fn una_tanda_nueva_invalida_la_anterior_sin_borrar_el_rastro() {
        let pool = base().await;
        let viejos = con_codigos(&pool).await;

        // Alguien entró por el código 4. Esa fila es la única prueba de ello.
        assert!(gastar_codigo(&pool, 1, &viejos[3], ahora()).await.unwrap());
        let gastadas = |p: Pool| async move {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM codigo_recuperacion WHERE usuario_id = 1 \
                 AND usado_en IS NOT NULL",
            )
            .fetch_one(&p)
            .await
            .unwrap()
        };
        assert_eq!(gastadas(pool.clone()).await, 1);

        let nuevos = con_codigos(&pool).await;
        assert_eq!(codigos_sin_usar(&pool, 1).await.unwrap(), 10, "quedaron veinte");
        assert!(!gastar_codigo(&pool, 1, &viejos[0], ahora()).await.unwrap());
        assert!(gastar_codigo(&pool, 1, &nuevos[0], ahora()).await.unwrap());

        // Y el rastro de abril sigue ahí en septiembre.
        assert_eq!(
            gastadas(pool.clone()).await,
            2,
            "la tanda nueva se llevó la prueba de que alguien entró por un código"
        );
    }

    /// Retirar el factor lo retira ENTERO. Dejar los hashes sería guardar
    /// credenciales vivas de una puerta que ya no existe.
    #[tokio::test]
    async fn desactivar_no_deja_ni_secreto_ni_codigos() {
        let pool = base().await;
        iniciar_enrolamiento(&pool, 1, &generar_secreto(), Some(FRASE)).await.unwrap();
        confirmar(&pool, 1, 59_000_000, ahora()).await.unwrap();
        con_codigos(&pool).await;

        desactivar(&pool, 1).await.unwrap();
        assert_eq!(situacion(&pool, 1).await.unwrap(), Situacion::SinSecreto);
        assert_eq!(codigos_sin_usar(&pool, 1).await.unwrap(), 0);
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM codigo_recuperacion")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(total, 0, "quedaron hashes de una puerta que ya no existe");
    }
}
