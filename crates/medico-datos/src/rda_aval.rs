// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! **La bitácora del aval del `nilknown`** (#603 F4.5, requisito #2).
//!
//! Una sección vacía de un RDA solo emite `emptyReason=nilknown` —«revisado, no
//! hay nada conocido»— si un CLÍNICO lo confirmó (la puerta de #603, decidida por
//! Juan el 2026-08-23). El dominio lo modela con un tipo infalsificable
//! ([`medico_dominio::ihce::Aval`]): avalar exige nombrar quién y cuándo, por
//! sección. Esta es la mitad DATOS de ese contrato: la tabla `aval_seccion_rda`
//! (migración 0072) donde ese acto queda registrado y de donde el productor lo
//! LEE para construir el aval.
//!
//! # Quién escribe la cadena `acceso`
//!
//! Aquí NO se registra en la cadena `acceso` (Ley 1581). Ese registro —encadenado
//! por hash y firmado— lo hace la RUTA que llama a [`avalar_secciones`]: tiene la
//! clave de firma y el rol/institución de la sesión, como todos los demás accesos
//! del sistema (`rutas::registrar_acceso`). No se abre una segunda cadena: se usa
//! la que ya existe (decisión de Juan, F4.5).

use medico_dominio::Ambito;

/// Una sección avalada, leída de la bitácora. El productor la convierte en una
/// [`medico_dominio::ihce::Confirmacion`] para construir el `Aval` del RDA.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SeccionAvalada {
    /// El código LOINC de la sección (p. ej. `48765-2`, alergias).
    pub seccion_codigo: String,
    /// El clínico que la confirmó vacía.
    pub usuario_id: i64,
    /// El instante de la confirmación, ISO-8601 con huso (zona del prestador).
    pub sello: String,
}

/// Un error de avalar que no es «faltan datos»: la base falló, o la historia no
/// es de este ámbito (aislamiento). Se implementa a mano, como `ErrorRda`, para
/// no traer `thiserror` a `medico-datos`.
#[derive(Debug)]
pub enum ErrorAval {
    Bd(sqlx::Error),
    /// El encuentro no existe en el ámbito del clínico: no se avala el encuentro
    /// de otra institución. Lleva el tipo y el id para que el mensaje diga cuál.
    EncuentroNoExiste { tipo: String, id: i64 },
    /// El `tipo_encuentro` no es uno de los tres conocidos ('consulta-externa',
    /// 'hospitalizacion', 'urgencias'). Falla ruidosamente (directiva 20): un tipo
    /// mal escrito no debe avalar «por si acaso».
    EncuentroDesconocido(String),
    /// El usuario al que se atribuiría el aval no es un clínico ACTIVO de este
    /// ámbito. Cierra el flanco simétrico al de la historia (refuerzo del concilio,
    /// directiva 23): acotar la historia y no el avalador dejaría atribuir la
    /// afirmación de un `nilknown` a un usuario de otra sede o inactivo.
    AvaladorDeOtraInstitucion(i64),
}

impl std::fmt::Display for ErrorAval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorAval::Bd(e) => write!(f, "error de base registrando el aval: {e}"),
            ErrorAval::EncuentroNoExiste { tipo, id } => {
                write!(f, "no existe el encuentro {tipo} {id} en esta institución (o no es suyo)")
            }
            ErrorAval::EncuentroDesconocido(t) => {
                write!(f, "tipo de encuentro desconocido: «{t}»")
            }
            ErrorAval::AvaladorDeOtraInstitucion(id) => write!(
                f,
                "el usuario {id} no es un clínico activo de esta institución: no se le atribuye el aval de un nilknown"
            ),
        }
    }
}

impl std::error::Error for ErrorAval {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ErrorAval::Bd(e) => Some(e),
            ErrorAval::EncuentroNoExiste { .. }
            | ErrorAval::EncuentroDesconocido(_)
            | ErrorAval::AvaladorDeOtraInstitucion(_) => None,
        }
    }
}

impl From<sqlx::Error> for ErrorAval {
    fn from(e: sqlx::Error) -> Self {
        ErrorAval::Bd(e)
    }
}

/// Las secciones que un clínico avaló vacías para esta historia, ordenadas por
/// código. El productor del RDA las lee para construir el aval; una lista vacía
/// significa «nadie confirmó nada» → toda sección vacía es una falta.
pub async fn secciones_avaladas(
    pool: &crate::Pool,
    tipo_encuentro: &str,
    encuentro_id: i64,
) -> Result<Vec<SeccionAvalada>, sqlx::Error> {
    sqlx::query_as(
        "SELECT seccion_codigo, usuario_id, sello FROM aval_seccion_rda \
         WHERE tipo_encuentro = ? AND encuentro_id = ? ORDER BY seccion_codigo",
    )
    .bind(tipo_encuentro)
    .bind(encuentro_id)
    .fetch_all(pool)
    .await
}

/// Registra el aval de una o más secciones vacías de una historia, atribuido al
/// clínico que las confirma. #603 F4.5, requisito #2.
///
/// - **AISLAMIENTO**: la historia tiene que ser del `ambito` del clínico; si no,
///   se niega. Reusa la puerta de `historia::obtener`, que devuelve `None` fuera
///   del ámbito — no se avala la historia de otra institución.
/// - `sello` lo pone el llamante (ISO-8601 con huso, zona del prestador): el
///   módulo no toca el reloj, igual que `acceso`.
/// - Reavalar una sección **reemplaza** su fila (un solo aval vigente por
///   sección); el histórico del acto vive en la cadena `acceso`, append-only.
/// - Las secciones se escriben en UNA transacción: o se avalan todas, o ninguna.
///
/// Un código de sección que no corresponda a ninguna del RDA es inocuo: el aval
/// solo emite `nilknown` para secciones que existen y están vacías, así que una
/// fila con un código inventado no afirma nada (falla cerrado). La pantalla de
/// captura solo ofrece las secciones vacías reales del encuentro.
/// Las DOS guardas que todo cambio del aval comparte (directiva 9): la historia
/// tiene que ser de este ámbito, y el usuario al que se atribuye el aval tiene que
/// ser un clínico ACTIVO de este ámbito.
///
/// La segunda es el refuerzo del concilio (directiva 23): acotar la historia y no
/// el avalador dejaría el mismo hueco «dos de tres» que el productor cerró para el
/// profesional — un llamante podría atribuir la afirmación de un `nilknown` a un
/// usuario de otra sede, a un inactivo o al superadmin (que `es_del_ambito` niega).
/// La barrera vive aquí, en la capa que escribe, no en la confianza en el llamante.
async fn asegurar_ambito(
    pool: &crate::Pool,
    ambito: Ambito,
    usuario_id: i64,
    tipo_encuentro: &str,
    encuentro_id: i64,
) -> Result<(), ErrorAval> {
    // El encuentro tiene que existir y ser de este ámbito. Cada tipo vive en su
    // propia tabla y valida el aislamiento a su manera (historia por su paciente,
    // hospitalización por su cama, urgencias por su paciente). Un tipo desconocido
    // NO pasa (directiva 20): mejor fallar que avalar sobre nada.
    // El aislamiento del aval usa EXACTAMENTE la misma lectura que el productor del
    // RDA (`rda_encuentro::interno_por_tipo` → `*::obtener`, todas filtradas por la
    // institución del PACIENTE): una sola vía de aislamiento por encuentro, no dos
    // criterios (directiva 1). Antes hospitalización se comprobaba por la CAMA
    // (`es_del_ambito`) y el productor por el paciente — dos predicados para la misma
    // pregunta, que solo coinciden porque `ingresar` los ata (asimetría latente que
    // el concilio del 2026-09-17 marcó). Un tipo desconocido falla ruidosamente (D20).
    let del_ambito = match tipo_encuentro {
        crate::rda_encuentro::OPERACION_CONSULTA_EXTERNA => {
            crate::historia::obtener(pool, ambito, encuentro_id).await?.is_some()
        }
        crate::rda_encuentro::OPERACION_HOSPITALIZACION => {
            crate::hospitalizacion::obtener(pool, ambito, encuentro_id).await?.is_some()
        }
        crate::rda_encuentro::OPERACION_URGENCIAS => {
            crate::urgencias::obtener(pool, ambito, encuentro_id).await?.is_some()
        }
        otro => return Err(ErrorAval::EncuentroDesconocido(otro.to_string())),
    };
    if !del_ambito {
        return Err(ErrorAval::EncuentroNoExiste {
            tipo: tipo_encuentro.to_string(),
            id: encuentro_id,
        });
    }
    if !crate::usuario::es_del_ambito(pool, ambito, usuario_id).await? {
        return Err(ErrorAval::AvaladorDeOtraInstitucion(usuario_id));
    }
    Ok(())
}

pub async fn avalar_secciones(
    pool: &crate::Pool,
    ambito: Ambito,
    usuario_id: i64,
    tipo_encuentro: &str,
    encuentro_id: i64,
    sello: &str,
    secciones: &[&str],
) -> Result<(), ErrorAval> {
    asegurar_ambito(pool, ambito, usuario_id, tipo_encuentro, encuentro_id).await?;
    let mut tx = pool.begin().await?;
    for s in secciones {
        // `DO NOTHING`, no `DO UPDATE`: la atribución es de quien confirmó la sección
        // POR PRIMERA VEZ (ver la nota de `fijar_avales`). Si ya estaba avalada, se
        // conserva su usuario/sello; re-atestar exige desavalar y avalar de nuevo.
        sqlx::query(
            "INSERT INTO aval_seccion_rda (tipo_encuentro, encuentro_id, seccion_codigo, usuario_id, sello) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT(tipo_encuentro, encuentro_id, seccion_codigo) DO NOTHING",
        )
        .bind(tipo_encuentro)
        .bind(encuentro_id)
        .bind(s)
        .bind(usuario_id)
        .bind(sello)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// FIJA el conjunto de secciones avaladas de una historia a EXACTAMENTE `deseadas`
/// (#603 F4.5-C): avala las que están y RETIRA las que ya no. Es la semántica de la
/// pantalla de casillas —la lista que llega ES el conjunto completo, como
/// `institucion::fijar_modulos`—, no un añadido incremental.
///
/// **Retirar un aval es SEGURO por construcción**: solo puede hacer que una sección
/// vuelva a «sin confirmar» → el RDA no se emite hasta reavalarla, que es el lado
/// conservador (nunca produce un `nilknown` falso). Por eso la pantalla puede
/// ofrecer desmarcar sin riesgo.
///
/// **La atribución es de quien confirmó la sección POR PRIMERA VEZ** (`DO NOTHING`,
/// no `DO UPDATE`), y esto es la corrección de un hallazgo del concilio (2026-09-04):
/// como el `<form>` reenvía TODAS las casillas marcadas —también las que otro clínico
/// avaló y este no tocó—, un `DO UPDATE` reatribuía silenciosamente esas secciones a
/// quien guardara, falseando el requisito #1 de la puerta del `nilknown` («quién/cuándo
/// confirmó», con peso legal). Con `DO NOTHING`, una sección que ya está avalada
/// conserva su avalador y su sello; RE-ATESTAR con otro usuario/sello exige desmarcarla
/// (la retira el `DELETE`) y volver a marcarla (la reinserta el paso 2). La cadena
/// `acceso` registra además el ACTO de guardar (a nivel de historia, no de sección).
///
/// El llamante debe pasar en `deseadas` SOLO códigos de secciones realmente
/// avalables de esta historia (la ruta los filtra contra `secciones_avalables`); un
/// código ajeno es inocuo (no casa ninguna sección real), pero filtrar mantiene la
/// tabla limpia.
pub async fn fijar_avales(
    pool: &crate::Pool,
    ambito: Ambito,
    usuario_id: i64,
    tipo_encuentro: &str,
    encuentro_id: i64,
    sello: &str,
    deseadas: &[&str],
) -> Result<(), ErrorAval> {
    asegurar_ambito(pool, ambito, usuario_id, tipo_encuentro, encuentro_id).await?;
    let mut tx = pool.begin().await?;
    // 1) Retirar las que ya no están marcadas. `NOT IN (lista)` se arma con binds,
    // no interpolando: los códigos vienen del cliente. Con la lista vacía, retira
    // TODAS (desavalar todo), que es correcto.
    let marcadores = deseadas.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql_borrar = format!(
        "DELETE FROM aval_seccion_rda WHERE tipo_encuentro = ? AND encuentro_id = ? \
         AND seccion_codigo NOT IN ({marcadores})"
    );
    let mut borrar = sqlx::query(&sql_borrar).bind(tipo_encuentro).bind(encuentro_id);
    for s in deseadas {
        borrar = borrar.bind(s);
    }
    borrar.execute(&mut *tx).await?;
    // 2) Avalar las marcadas. `DO NOTHING`: si la sección ya estaba avalada, conserva
    // la atribución de quien la confirmó primero (ver la nota de arriba); solo las que
    // el DELETE retiró y vuelven a marcarse se reinsertan con el usuario/sello de este
    // acto. Así el reenvío de una casilla ajena ya marcada no reatribuye nada.
    for s in deseadas {
        sqlx::query(
            "INSERT INTO aval_seccion_rda (tipo_encuentro, encuentro_id, seccion_codigo, usuario_id, sello) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT(tipo_encuentro, encuentro_id, seccion_codigo) DO NOTHING",
        )
        .bind(tipo_encuentro)
        .bind(encuentro_id)
        .bind(s)
        .bind(usuario_id)
        .bind(sello)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Estos tests operan sobre una `historia`, así que el encuentro es una
    /// consulta externa. Los avales de hospitalización/urgencias los prueban los
    /// productores de esas piezas (#603).
    const TIPO: &str = crate::rda_encuentro::OPERACION_CONSULTA_EXTERNA;

    /// Base con una institución, un usuario clínico y un paciente, y devuelve el
    /// id de una historia recién creada de la institución 1.
    async fn base_con_historia() -> (crate::Pool, i64) {
        static N: AtomicU64 = AtomicU64::new(0);
        let id = N.fetch_add(1, Ordering::Relaxed);
        let url =
            format!("sqlite:file:aval_{}_{}?mode=memory&cache=shared", std::process::id(), id);
        let pool = crate::abrir(&url).await.unwrap();
        sqlx::query(
            "INSERT INTO institucion (id, nombre, nit, codigo_habilitacion) \
             VALUES (1,'IPS Uno','900123456','0500100001'), (2,'IPS Dos','900999999','0500100002')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) \
             VALUES (5,'Carlos','c@x.co','h','medico',1,1), \
                    (6,'Diana','d@x.co','h','medico',1,1), \
                    (9,'Ajeno','a@x.co','h','medico',2,1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO paciente (id, institucion_id, tipo_documento, numero_documento, nombre1, apellido1, sexo, fecha_nacimiento) \
                     VALUES (10, 1, 'CC', '123', 'Ana', 'Ruiz', 'F', '1990-01-01')")
            .execute(&pool)
            .await
            .unwrap();
        // `historia` no tiene `institucion_id`: el ámbito se filtra vía el paciente.
        let hid: i64 = sqlx::query_scalar(
            "INSERT INTO historia (codigo_historia, paciente_id, medico_id, fecha, \
                 motivo_consulta, enfermedad_actual, diagnostico_principal, plan_tratamiento, \
                 finalidad, causa_externa, tipo_diagnostico, cod_consulta_cups, \
                 modalidad_grupo_servicio, grupo_servicios) \
             VALUES ('H-1', 10, 5, '2026-09-03T10:00:00-05:00', \
                 'dolor', 'gripa', 'A000', 'reposo', \
                 '01', '22', '01', '890201', '01', '01') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        (pool, hid)
    }

    #[tokio::test]
    async fn lo_avalado_se_lee_con_su_atribucion() {
        let (pool, hid) = base_con_historia().await;
        let ambito = Ambito::institucion(1);
        avalar_secciones(&pool, ambito, 5, TIPO, hid, "2026-09-03T10:05:00-05:00", &["48765-2", "10160-0"])
            .await
            .unwrap();
        let filas = secciones_avaladas(&pool, TIPO, hid).await.unwrap();
        assert_eq!(filas.len(), 2, "se avalaron dos secciones");
        let alergias = filas.iter().find(|f| f.seccion_codigo == "48765-2").unwrap();
        assert_eq!(alergias.usuario_id, 5, "queda registrado QUIÉN confirmó");
        assert_eq!(alergias.sello, "2026-09-03T10:05:00-05:00", "y CUÁNDO");
    }

    /// **Reavalar una sección ya avalada CONSERVA su atribución original** (corrección
    /// del hallazgo del concilio, 2026-09-04): la atestación es de quien confirmó la
    /// sección primero. Su pareja roja: con el viejo `DO UPDATE` el sello cambiaría al
    /// del segundo acto; con `DO NOTHING` se conserva el primero. Re-atestar de verdad
    /// exige desavalar (retirar la fila) y avalar de nuevo, no reenviar la casilla.
    #[tokio::test]
    async fn reavalar_una_seccion_conserva_la_atribucion_original() {
        let (pool, hid) = base_con_historia().await;
        let ambito = Ambito::institucion(1);
        avalar_secciones(&pool, ambito, 5, TIPO, hid, "2026-09-03T10:00:00-05:00", &["48765-2"])
            .await
            .unwrap();
        // Diana (6) reenvía la misma sección, ya avalada por Carlos (5): NO reatribuye.
        avalar_secciones(&pool, ambito, 6, TIPO, hid, "2026-09-03T11:00:00-05:00", &["48765-2"])
            .await
            .unwrap();
        let filas = secciones_avaladas(&pool, TIPO, hid).await.unwrap();
        assert_eq!(filas.len(), 1, "un solo aval vigente por sección");
        assert_eq!(filas[0].usuario_id, 5, "la atribución la conserva Carlos, que confirmó primero");
        assert_eq!(filas[0].sello, "2026-09-03T10:00:00-05:00", "y su sello original");
    }

    /// **EL AVALADOR ACOTADO AL ÁMBITO** (refuerzo del concilio, D23) y su pareja:
    /// un usuario de OTRA institución (9, de la 2) NO puede recibir la atribución del
    /// aval de una historia de la 1, aunque el ámbito y la historia sean de la 1.
    /// Cierra el flanco simétrico al de la historia; sin él, la afirmación de un
    /// `nilknown` quedaría atribuida a un clínico de otra sede.
    #[tokio::test]
    async fn un_avalador_de_otra_institucion_no_recibe_la_atribucion() {
        let (pool, hid) = base_con_historia().await;
        // Rojo, por la vía real: el usuario 9 es de la institución 2.
        let e = avalar_secciones(&pool, Ambito::institucion(1), 9, TIPO, hid, "2026-09-03T10:00:00-05:00", &["48765-2"])
            .await
            .expect_err("un avalador de otra institución no debe recibir la atribución");
        assert!(matches!(e, ErrorAval::AvaladorDeOtraInstitucion(9)), "avalador ajeno: {e:?}");
        assert!(secciones_avaladas(&pool, TIPO, hid).await.unwrap().is_empty(), "el aval rechazado no deja fila");
        // Verde: el clínico de la propia institución (5) sí avala.
        avalar_secciones(&pool, Ambito::institucion(1), 5, TIPO, hid, "2026-09-03T10:00:00-05:00", &["48765-2"])
            .await
            .unwrap();
        assert_eq!(secciones_avaladas(&pool, TIPO, hid).await.unwrap().len(), 1);
    }

    /// **`fijar_avales` fija el conjunto EXACTO** (semántica de casillas): marcar
    /// {A,B} y luego {B,C} deja {B,C} —A se retira, C se añade—; y vaciar retira
    /// todo. Su pareja es que retirar es seguro: la sección vuelve a «sin avalar».
    #[tokio::test]
    async fn fijar_avales_deja_exactamente_lo_marcado() {
        let (pool, hid) = base_con_historia().await;
        let a = Ambito::institucion(1);
        let t = "2026-09-03T10:00:00-05:00";
        fijar_avales(&pool, a, 5, TIPO, hid, t, &["48765-2", "10160-0"]).await.unwrap();
        let mut c: Vec<String> =
            secciones_avaladas(&pool, TIPO, hid).await.unwrap().into_iter().map(|s| s.seccion_codigo).collect();
        c.sort();
        assert_eq!(c, vec!["10160-0", "48765-2"], "las dos marcadas");
        // Reencuadre: {B,C}. A (48765-2) sale, C (74208-0) entra, B (10160-0) queda.
        fijar_avales(&pool, a, 5, TIPO, hid, t, &["10160-0", "74208-0"]).await.unwrap();
        let mut c2: Vec<String> =
            secciones_avaladas(&pool, TIPO, hid).await.unwrap().into_iter().map(|s| s.seccion_codigo).collect();
        c2.sort();
        assert_eq!(c2, vec!["10160-0", "74208-0"], "A retirada, C añadida, B intacta");
        // Vaciar retira todo (desavalar todo) — el lado conservador.
        fijar_avales(&pool, a, 5, TIPO, hid, t, &[]).await.unwrap();
        assert!(secciones_avaladas(&pool, TIPO, hid).await.unwrap().is_empty(), "vaciar desavala todo");
        // Y respeta el aislamiento del avalador igual que avalar_secciones.
        let e = fijar_avales(&pool, a, 9, TIPO, hid, t, &["48765-2"]).await.expect_err("usuario ajeno");
        assert!(matches!(e, ErrorAval::AvaladorDeOtraInstitucion(9)), "{e:?}");
    }

    /// **Reenviar una casilla ya avalada por OTRO clínico NO reatribuye la sección**
    /// (pareja roja del hallazgo del concilio, 2026-09-04, verificado por sonnet). El
    /// `<form>` HTML reenvía todas las casillas marcadas, así que cuando Diana guarda
    /// para avalar SU sección, el POST arrastra también la de Carlos ya marcada. La
    /// atestación de esa sección debe seguir siendo de Carlos —quien de verdad la
    /// confirmó—, no de Diana. Con el viejo `DO UPDATE` este test fallaba (48765-2
    /// quedaba atribuida a 6); con `DO NOTHING` se conserva a 5.
    #[tokio::test]
    async fn reenviar_una_casilla_ajena_ya_marcada_no_reatribuye_la_seccion() {
        let (pool, hid) = base_con_historia().await;
        let a = Ambito::institucion(1);
        // Carlos (5) avala de verdad la sección A en T1.
        fijar_avales(&pool, a, 5, TIPO, hid, "2026-09-03T10:00:00-05:00", &["48765-2"]).await.unwrap();
        // Diana (6) abre la pantalla —ve A ya marcada— y avala SU sección B; el form
        // reenvía A (premarcada) + B. Fija {A,B} atribuido a Diana en T2.
        fijar_avales(&pool, a, 6, TIPO, hid, "2026-09-03T11:30:00-05:00", &["48765-2", "10160-0"])
            .await
            .unwrap();
        let filas = secciones_avaladas(&pool, TIPO, hid).await.unwrap();
        let a_sec = filas.iter().find(|f| f.seccion_codigo == "48765-2").expect("A sigue avalada");
        let b_sec = filas.iter().find(|f| f.seccion_codigo == "10160-0").expect("B avalada por Diana");
        assert_eq!(a_sec.usuario_id, 5, "A conserva a Carlos, que la confirmó; no la reatribuye Diana");
        assert_eq!(a_sec.sello, "2026-09-03T10:00:00-05:00", "y su sello original");
        assert_eq!(b_sec.usuario_id, 6, "B sí es de Diana, que la marcó de verdad");
        assert_eq!(b_sec.sello, "2026-09-03T11:30:00-05:00", "con su sello");
    }

    /// AISLAMIENTO y su pareja (directiva 33): la institución 2 NO puede avalar la
    /// historia de la 1, y la 1 SÍ. Sin el caso rojo, la puerta podría no
    /// engancharse y un clínico avalaría secciones de otra sede.
    #[tokio::test]
    async fn otra_institucion_no_avala_la_historia_ajena() {
        let (pool, hid) = base_con_historia().await;
        // Rojo, por la vía real: la institución 2 no ve la historia de la 1.
        let e = avalar_secciones(&pool, Ambito::institucion(2), 5, TIPO, hid, "2026-09-03T10:00:00-05:00", &["48765-2"])
            .await
            .expect_err("la institución 2 no debe poder avalar la historia de la 1");
        assert!(matches!(e, ErrorAval::EncuentroNoExiste { .. }), "aislamiento: {e:?}");
        // Y nada se escribió.
        assert!(secciones_avaladas(&pool, TIPO, hid).await.unwrap().is_empty(), "el aval rechazado no deja fila");
        // Verde: la institución dueña sí avala.
        avalar_secciones(&pool, Ambito::institucion(1), 5, TIPO, hid, "2026-09-03T10:00:00-05:00", &["48765-2"])
            .await
            .unwrap();
        assert_eq!(secciones_avaladas(&pool, TIPO, hid).await.unwrap().len(), 1);
    }

    // ── Hallazgos del concilio del 2026-09-17: el aval de hosp/urg no tenía caso
    //    rojo (la separación por tipo ni el aislamiento al ESCRIBIR). Estas tres
    //    pruebas son las que faltaban (D33): fable las escribió, opus y sonnet las
    //    corroboraron por mutación. Discriminan los mutantes que dejaban la suite
    //    verde: `secciones_avaladas` sin filtrar por tipo, y la rama de hosp/urg de
    //    `asegurar_ambito` cableada a `true` o al tipo equivocado.

    /// Siembra una hospitalización del paciente 10 (institución 1) y devuelve su id.
    async fn sembrar_hospitalizacion_de_la_1(pool: &crate::Pool) -> i64 {
        let cama: i64 = sqlx::query_scalar(
            "INSERT INTO cama (institucion_id, numero, piso, servicio, estado) \
             VALUES (1,'101','1','GENERAL','OCUPADA') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query_scalar(
            "INSERT INTO hospitalizacion (paciente_id, cama_id, medico_tratante_id, fecha_ingreso, via_ingreso, estado) \
             VALUES (10, ?, 5, '2026-09-03T08:00:00-05:00', 'PROGRAMADA', 'ACTIVA') RETURNING id",
        )
        .bind(cama)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    /// Siembra una atención de urgencias del paciente 10 (institución 1) y su id.
    async fn sembrar_urgencia_de_la_1(pool: &crate::Pool) -> i64 {
        sqlx::query_scalar(
            "INSERT INTO atencion_urgencias (paciente_id, triage, motivo_consulta, enfermera_triage_id, fecha_ingreso) \
             VALUES (10, 'TRIAGE_2', 'dolor', 5, '2026-09-03T09:00:00-05:00') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    /// **La SEPARACIÓN POR TIPO de la llave nueva** (D33). Un aval de consulta con un
    /// `id` numérico NO se lee bajo otro tipo con el MISMO id — y las secuencias de
    /// `historia`/`hospitalizacion` son independientes, así que ese id colisiona en
    /// instalaciones nuevas. Mutante que cazaría: `secciones_avaladas` sin el
    /// `WHERE tipo_encuentro = ?`.
    #[tokio::test]
    async fn el_aval_de_un_tipo_no_vale_para_otro_con_el_mismo_id() {
        let (pool, hid) = base_con_historia().await;
        let a = Ambito::institucion(1);
        avalar_secciones(&pool, a, 5, TIPO, hid, "2026-09-03T10:00:00-05:00", &["48765-2"])
            .await
            .unwrap();
        let otro = crate::rda_encuentro::OPERACION_HOSPITALIZACION;
        assert!(
            secciones_avaladas(&pool, otro, hid).await.unwrap().is_empty(),
            "un aval de consulta NO puede leerse como hospitalización con el mismo id"
        );
        assert_eq!(
            secciones_avaladas(&pool, TIPO, hid).await.unwrap().len(),
            1,
            "control: con su propio tipo sí se lee"
        );
    }

    /// **AISLAMIENTO al ESCRIBIR el aval de HOSPITALIZACIÓN** (D33). El avalador es de
    /// la institución 2 (usuario 9) para AISLAR la guarda del encuentro de la del
    /// avalador: así solo la comprobación del encuentro puede rechazar. Mutante que
    /// cazaría: la rama de hospitalización de `asegurar_ambito` cableada a `true`.
    #[tokio::test]
    async fn el_aval_de_hospitalizacion_respeta_el_ambito() {
        let (pool, _hid) = base_con_historia().await;
        let hosp = sembrar_hospitalizacion_de_la_1(&pool).await;
        let op = crate::rda_encuentro::OPERACION_HOSPITALIZACION;
        // Rojo, por la vía real: un clínico de la institución 2 no avala la
        // hospitalización de la 1 (el encuentro no es de su ámbito).
        let e = avalar_secciones(&pool, Ambito::institucion(2), 9, op, hosp, "2026-09-03T10:00:00-05:00", &["48765-2"])
            .await
            .expect_err("la institución 2 no debe avalar la hospitalización de la 1");
        assert!(matches!(e, ErrorAval::EncuentroNoExiste { .. }), "aislamiento hosp: {e:?}");
        assert!(secciones_avaladas(&pool, op, hosp).await.unwrap().is_empty(), "el aval rechazado no deja fila");
        // Verde: la institución dueña sí.
        avalar_secciones(&pool, Ambito::institucion(1), 5, op, hosp, "2026-09-03T10:00:00-05:00", &["48765-2"])
            .await
            .unwrap();
        assert_eq!(secciones_avaladas(&pool, op, hosp).await.unwrap().len(), 1);
    }

    /// **AISLAMIENTO al ESCRIBIR el aval de URGENCIAS** (D33), la gemela de la
    /// anterior. Mutante que cazaría: la rama de urgencias de `asegurar_ambito`
    /// cableada a `true`.
    #[tokio::test]
    async fn el_aval_de_urgencias_respeta_el_ambito() {
        let (pool, _hid) = base_con_historia().await;
        let urg = sembrar_urgencia_de_la_1(&pool).await;
        let op = crate::rda_encuentro::OPERACION_URGENCIAS;
        let e = avalar_secciones(&pool, Ambito::institucion(2), 9, op, urg, "2026-09-03T10:00:00-05:00", &["48765-2"])
            .await
            .expect_err("la institución 2 no debe avalar la urgencia de la 1");
        assert!(matches!(e, ErrorAval::EncuentroNoExiste { .. }), "aislamiento urgencias: {e:?}");
        assert!(secciones_avaladas(&pool, op, urg).await.unwrap().is_empty(), "el aval rechazado no deja fila");
        avalar_secciones(&pool, Ambito::institucion(1), 5, op, urg, "2026-09-03T10:00:00-05:00", &["48765-2"])
            .await
            .unwrap();
        assert_eq!(secciones_avaladas(&pool, op, urg).await.unwrap().len(), 1);
    }
}
