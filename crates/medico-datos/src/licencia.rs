//! La licencia comercial de una institución: cupo de usuarios y vigencia.
//!
//! Regla que no se rompe: **la licencia limita la EXPANSIÓN, nunca la lectura.**
//! Nada de aquí se llama en el login ni en los listados. Una licencia vencida
//! impide crear usuarios, pero jamás ciega la historia clínica: dejar a un
//! hospital sin sus registros por un asunto comercial lo pone a incumplir ante
//! el paciente y la Supersalud.

/// Estado de la licencia de una institución en una fecha dada.
///
/// **Son DOS cupos y no uno** (directiva 32): «cuánta gente cabe» y «cuántos de
/// esos pueden atender» son magnitudes distintas que no se cambian una por otra,
/// y hasta el 2026-08-06 aquí solo estaba la primera. `max_medicos` es
/// `Option` porque `NULL` en la base significa «este plan no pactó cupo de
/// médicos», no «cero»: ver la migración `0049`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Estado {
    SinLicencia,
    /// Con la fecha en que venció: «vencida» a secas manda a buscarla a la base.
    Vencida { vence: String },
    Vigente {
        max_usuarios: i64,
        max_medicos: Option<i64>,
        /// El último día válido, o `None` si es perpetua.
        ///
        /// **Viaja aunque la puerta no la mire**, y esa es la lección: la
        /// consulta ya leía la columna y la tiraba, así que la pantalla que
        /// edita el plan no podía rellenar el campo — y un formulario que sale
        /// con la fecha en blanco convierte «subir el cupo» en «además, hacerla
        /// perpetua», sin que nadie lo pida y sin que nada falle.
        vence: Option<String>,
    },
}

/// El estado de la licencia de `institucion_id` frente a `hoy` (YYYY-MM-DD). La
/// comparación es de cadena, que en ese formato es cronológica.
pub async fn estado(pool: &crate::Pool, institucion_id: i64, hoy: &str) -> Result<Estado, sqlx::Error> {
    let fila: Option<(i64, Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT max_usuarios, max_medicos, vence FROM licencia WHERE institucion_id = ?",
    )
    .bind(institucion_id)
    .fetch_optional(pool)
    .await?;
    Ok(match fila {
        None => Estado::SinLicencia,
        // `vence` es el último día válido: vencida solo si venció ANTES de hoy.
        Some((_, _, Some(vence))) if vence.as_str() < hoy => Estado::Vencida { vence },
        Some((max, medicos, vence)) => Estado::Vigente {
            max_usuarios: max,
            max_medicos: medicos,
            vence,
        },
    })
}

/// Fija (crea o reemplaza) la licencia de una institución. La usa el superadmin
/// al dar de alta o renovar un plan.
///
/// `max_medicos = None` deja el plan **sin** cupo de médicos, que es lo que
/// tienen todas las licencias anteriores al 2026-08-06: no se rellena con un
/// número inventado.
pub async fn fijar(
    pool: &crate::Pool,
    institucion_id: i64,
    max_usuarios: i64,
    max_medicos: Option<i64>,
    vence: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO licencia (institucion_id, max_usuarios, max_medicos, vence) \
         VALUES (?, ?, ?, ?) \
         ON CONFLICT(institucion_id) DO UPDATE SET max_usuarios = excluded.max_usuarios, \
         max_medicos = excluded.max_medicos, vence = excluded.vence",
    )
    .bind(institucion_id)
    .bind(max_usuarios)
    .bind(max_medicos)
    .bind(vence)
    .execute(pool)
    .await?;
    Ok(())
}

/// Si se puede o no agregar un usuario a una institución, y por qué no. El cupo
/// se cuenta contra los usuarios ACTIVOS reales (un inactivo no consume cupo),
/// no contra un campo guardado.
///
/// `CupoDeMedicosLleno` es un caso **aparte** de `CupoLleno` y no un matiz del
/// mismo: quien lo recibe todavía tiene sitio para recepcionistas, regentes y
/// auxiliares, y lo que ha de comprar es otra cosa. Meterlos en el mismo cajón
/// haría que el prestador ampliara el plan equivocado.
#[derive(Debug, PartialEq, Eq)]
pub enum Puerta {
    Abierta,
    SinLicencia,
    Vencida,
    CupoLleno,
    CupoDeMedicosLleno,
}

/// **Qué asientos ocupa el cambio que se va a hacer.** Son dos y no uno, y
/// colapsarlos en un booleano produce un falso bloqueo que ya estuvo escrito
/// aquí durante media hora (directiva 32).
///
/// | operación | usuario | médico |
/// |---|---|---|
/// | alta de una recepcionista | sí | no |
/// | alta de un médico | sí | **sí** |
/// | reactivar a quien se fue | sí | según su rol |
/// | **ascender a médico a quien ya está activo** | **no** | **sí** |
///
/// La última fila es la que obliga a separarlos: esa persona ya está contada
/// como usuario, así que exigirle asiento de usuario dejaría a una IPS con el
/// plan justo sin poder ascender a nadie — una regla que rechaza la operación
/// normal de un cliente legítimo está mal por bien argumentada que esté
/// (directiva 21).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asiento {
    pub de_usuario: bool,
    pub de_medico: bool,
}

impl Asiento {
    /// Un alta: siempre ocupa asiento de usuario, y de médico según el rol.
    pub fn alta(es_medico: bool) -> Self {
        Self { de_usuario: true, de_medico: es_medico }
    }
}

/// La puerta para un alta, una reactivación o un ascenso.
///
/// Los asientos van como parámetro y no se deducen aquí: quien llama es el único
/// que sabe qué había antes y qué va a quedar escrito.
pub async fn puerta_para_usuario(
    pool: &crate::Pool,
    institucion_id: i64,
    hoy: &str,
    asiento: Asiento,
) -> Result<Puerta, sqlx::Error> {
    match estado(pool, institucion_id, hoy).await? {
        Estado::SinLicencia => Ok(Puerta::SinLicencia),
        Estado::Vencida { .. } => Ok(Puerta::Vencida),
        Estado::Vigente {
            max_usuarios,
            max_medicos,
            ..
        } => {
            if asiento.de_usuario {
                let activos: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM usuario WHERE institucion_id = ? AND activo = 1",
                )
                .bind(institucion_id)
                .fetch_one(pool)
                .await?;
                if activos >= max_usuarios {
                    return Ok(Puerta::CupoLleno);
                }
            }
            // El segundo cupo solo existe si el plan lo pactó. `None` no es
            // cero: es «no hay segunda cota que comprobar».
            let (Some(tope), true) = (max_medicos, asiento.de_medico) else {
                return Ok(Puerta::Abierta);
            };
            let medicos: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM usuario \
                 WHERE institucion_id = ? AND activo = 1 AND rol = 'medico'",
            )
            .bind(institucion_id)
            .fetch_one(pool)
            .await?;
            Ok(if medicos >= tope {
                Puerta::CupoDeMedicosLleno
            } else {
                Puerta::Abierta
            })
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const HOY: &str = "2026-07-28";
    const AYER: &str = "2026-07-27";
    const MANANA: &str = "2026-07-29";

    /// Institución 1 con dos usuarios ACTIVOS y uno inactivo (que no cuenta cupo).
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id, nombre, nit) VALUES (1,'Uno','900')")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id, activo)
             VALUES (1,'A','a@x.co','h','medico',1,1),
                    (2,'B','b@x.co','h','medico',1,1),
                    (3,'C','c@x.co','h','medico',1,0)",
        )
        .execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn sin_fila_es_sin_licencia() {
        let pool = base().await;
        assert_eq!(estado(&pool, 1, HOY).await.unwrap(), Estado::SinLicencia);
    }

    /// Un plan de 10 usuarios SIN cupo de médicos pactado, que es la forma de
    /// todas las licencias anteriores al 2026-08-06.
    fn diez(vence: Option<&str>) -> Estado {
        Estado::Vigente {
            max_usuarios: 10,
            max_medicos: None,
            vence: vence.map(str::to_string),
        }
    }

    #[tokio::test]
    async fn vence_ayer_esta_vencida_y_vence_hoy_sigue_vigente() {
        let pool = base().await;
        fijar(&pool, 1, 10, None, Some(AYER)).await.unwrap();
        assert_eq!(
            estado(&pool, 1, HOY).await.unwrap(),
            Estado::Vencida { vence: AYER.into() }
        );
        // El último día es válido: vence hoy, hoy sigue vigente.
        fijar(&pool, 1, 10, None, Some(HOY)).await.unwrap();
        assert_eq!(estado(&pool, 1, HOY).await.unwrap(), diez(Some(HOY)));
        // Perpetua (NULL) siempre vigente.
        fijar(&pool, 1, 10, None, None).await.unwrap();
        assert_eq!(estado(&pool, 1, HOY).await.unwrap(), diez(None));
    }

    /// **La fecha viaja con el estado, y una perpetua se distingue de una que
    /// vence hoy.** Hasta el 2026-08-08 las dos daban el MISMO `Estado`: la
    /// consulta leía la columna y la tiraba. Con eso, la pantalla que edita el
    /// plan salía con el campo en blanco, y guardar un cupo nuevo convertía la
    /// licencia en perpetua sin que nadie lo pidiera.
    #[tokio::test]
    async fn la_fecha_de_vencimiento_viaja_con_el_estado() {
        let pool = base().await;
        fijar(&pool, 1, 10, None, Some("2030-12-31")).await.unwrap();
        let con_fecha = estado(&pool, 1, HOY).await.unwrap();
        fijar(&pool, 1, 10, None, None).await.unwrap();
        let perpetua = estado(&pool, 1, HOY).await.unwrap();
        assert_ne!(con_fecha, perpetua, "una perpetua y una con fecha se ven igual");
        let Estado::Vigente { vence, .. } = con_fecha else { panic!("debía estar vigente") };
        assert_eq!(vence.as_deref(), Some("2030-12-31"));
        let Estado::Vigente { vence, .. } = perpetua else { panic!("debía estar vigente") };
        assert_eq!(vence, None, "una perpetua no puede inventarse una fecha");
    }

    #[tokio::test]
    async fn la_puerta_distingue_los_cuatro_casos() {
        let pool = base().await;
        // Sin licencia.
        assert_eq!(puerta_para_usuario(&pool, 1, HOY, Asiento::alta(false)).await.unwrap(), Puerta::SinLicencia);
        // Vencida.
        fijar(&pool, 1, 10, None, Some(AYER)).await.unwrap();
        assert_eq!(puerta_para_usuario(&pool, 1, HOY, Asiento::alta(false)).await.unwrap(), Puerta::Vencida);
        // Vigente con cupo (2 activos < 10).
        fijar(&pool, 1, 10, None, Some(MANANA)).await.unwrap();
        assert_eq!(puerta_para_usuario(&pool, 1, HOY, Asiento::alta(false)).await.unwrap(), Puerta::Abierta);
        // Cupo lleno: max 2 y hay 2 activos (el inactivo no cuenta).
        fijar(&pool, 1, 2, None, Some(MANANA)).await.unwrap();
        assert_eq!(puerta_para_usuario(&pool, 1, HOY, Asiento::alta(false)).await.unwrap(), Puerta::CupoLleno);
    }

    /// El inactivo no consume cupo: con max 3 y 2 activos + 1 inactivo, abre.
    #[tokio::test]
    async fn el_inactivo_no_consume_cupo() {
        let pool = base().await;
        fijar(&pool, 1, 3, None, Some(MANANA)).await.unwrap();
        assert_eq!(puerta_para_usuario(&pool, 1, HOY, Asiento::alta(false)).await.unwrap(), Puerta::Abierta);
    }

    // ── El segundo cupo ──────────────────────────────────────────────────────

    /// **La pareja del hueco.** La base tiene 2 médicos activos; con un plan de
    /// 20 usuarios y 2 médicos, el tercer médico NO entra y un no-médico SÍ.
    ///
    /// Sin las dos mitades esto no mide nada: solo la primera pasaría igual con
    /// una regla que cierre a todo el mundo, y solo la segunda con la regla de
    /// antes, que no miraba el rol.
    #[tokio::test]
    async fn el_cupo_de_medicos_frena_al_medico_y_deja_pasar_al_resto() {
        let pool = base().await;
        fijar(&pool, 1, 20, Some(2), Some(MANANA)).await.unwrap();
        assert_eq!(
            puerta_para_usuario(&pool, 1, HOY, Asiento::alta(true)).await.unwrap(),
            Puerta::CupoDeMedicosLleno,
            "entró un tercer médico con el plan en 2"
        );
        assert_eq!(
            puerta_para_usuario(&pool, 1, HOY, Asiento::alta(false)).await.unwrap(),
            Puerta::Abierta,
            "una recepcionista no consume cupo de médicos"
        );
        // Y con sitio, el médico entra: la puerta no está cerrada por estar.
        fijar(&pool, 1, 20, Some(3), Some(MANANA)).await.unwrap();
        assert_eq!(puerta_para_usuario(&pool, 1, HOY, Asiento::alta(true)).await.unwrap(), Puerta::Abierta);
    }

    /// **`NULL` no es cero.** Una licencia sin cupo de médicos pactado —las de
    /// todos los clientes que ya existen— no frena a ningún médico. Lo contrario
    /// dejaría a un prestador vivo sin poder dar de alta a nadie que atienda, en
    /// la primera migración.
    #[tokio::test]
    async fn sin_cupo_pactado_no_hay_segunda_cota() {
        let pool = base().await;
        fijar(&pool, 1, 20, None, Some(MANANA)).await.unwrap();
        assert_eq!(puerta_para_usuario(&pool, 1, HOY, Asiento::alta(true)).await.unwrap(), Puerta::Abierta);
        // Y el CONTROL, que es lo que distingue «no hay cota» de «la cota no
        // funciona»: con la misma base y cupo 1, el mismo médico se frena.
        fijar(&pool, 1, 20, Some(1), Some(MANANA)).await.unwrap();
        assert_eq!(
            puerta_para_usuario(&pool, 1, HOY, Asiento::alta(true)).await.unwrap(),
            Puerta::CupoDeMedicosLleno
        );
    }

    /// El cupo de USUARIOS manda primero: sin sitio para una persona más, da
    /// igual qué rol traiga. Si se comprobaran al revés, un médico recibiría
    /// «amplíe el cupo de médicos» cuando lo que le falta es cupo de usuarios,
    /// y ampliaría el plan equivocado.
    #[tokio::test]
    async fn el_cupo_de_usuarios_se_comprueba_antes() {
        let pool = base().await;
        fijar(&pool, 1, 2, Some(1), Some(MANANA)).await.unwrap();
        assert_eq!(
            puerta_para_usuario(&pool, 1, HOY, Asiento::alta(true)).await.unwrap(),
            Puerta::CupoLleno,
            "los dos cupos están llenos y gana el de médicos"
        );
    }

    /// Un plan de cero médicos es un imposible, no un inusual: lo rechaza el
    /// `CHECK` del esquema (directiva 21). El negativo también.
    #[tokio::test]
    async fn un_cupo_de_cero_medicos_no_se_puede_guardar() {
        let pool = base().await;
        assert!(fijar(&pool, 1, 10, Some(0), None).await.is_err());
        assert!(fijar(&pool, 1, 10, Some(-1), None).await.is_err());
        // La pareja: uno positivo sí entra.
        assert!(fijar(&pool, 1, 10, Some(1), None).await.is_ok());
    }
}
