//! El freno al ensayo de contraseñas.
//!
//! # La regla, en una frase
//!
//! Tras cinco fallos seguidos desde el mismo origen contra el mismo correo, el
//! siguiente intento espera; la espera se dobla con cada fallo y se detiene a los
//! cinco minutos. Un acierto lo borra todo.
//!
//! # Lo que se rechazó al diseñarla
//!
//! **Bloquear la cuenta.** Un bloqueo que exige a alguien desbloquear convierte
//! cinco peticiones de un desconocido en una historia clínica inaccesible a las
//! tres de la mañana. La regla se probó contra «¿qué cliente legítimo necesita
//! justo esto?»: la respuesta es que ninguno necesita quedarse fuera, y todos
//! necesitan que el diccionario no avance.
//!
//! **Contar solo por cuenta.** Con la cuenta sola, el atacante remoto elige a
//! quién dejar fuera. Con el origen dentro de la clave se frena a sí mismo.
//!
//! **Un tope alto y creciente sin límite.** Doblar sin techo es un bloqueo con
//! otro nombre: a los quince fallos serían horas. El techo son cinco minutos, y
//! eso ya baja un diccionario a menos de trescientos intentos al día.
//!
//! # Lo que este freno NO resuelve, y no debe aparentar que sí
//!
//! El relleno de credenciales desde muchas máquinas contra muchas cuentas: cada
//! par (correo, origen) falla una vez y ninguno llega al umbral. Contra eso la
//! respuesta es el segundo factor, no un freno más agresivo — que solo añadiría
//! el bloqueo que aquí se rechazó. Este módulo es el prerrequisito de ese
//! segundo factor, no su sustituto.

use chrono::{DateTime, Duration, Utc};

use crate::Pool;

/// Fallos seguidos que se admiten sin espera.
///
/// Cinco: equivocarse tres o cuatro veces al teclear es corriente, y a la quinta
/// ya no lo es. El sexto intento espera quince segundos, que un humano ni
/// registra como castigo y a un programa le arruina el ritmo.
pub const UMBRAL: i64 = 5;

/// Espera tras el primer fallo por encima del umbral. Se dobla con cada uno.
pub const BASE_SEGUNDOS: i64 = 15;

/// Techo de la espera. Ver el módulo: sin techo, el freno es un bloqueo.
pub const TOPE_SEGUNDOS: i64 = 300;

/// Cuánto se conserva un intento. Lo anterior no dice nada del ritmo actual y
/// solo haría crecer la tabla sin límite.
pub const RETENCION_HORAS: i64 = 24;

/// El estado del freno para un (correo, origen) en un instante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estado {
    /// Fallos seguidos desde el último acierto (o desde el borde de retención).
    pub fallos: i64,
    /// Cuánto falta para poder volver a probar. Cero = adelante.
    pub espera: Duration,
}

impl Estado {
    pub fn frenado(&self) -> bool {
        self.espera > Duration::zero()
    }

    /// Segundos que faltan, redondeados hacia arriba, para la cabecera
    /// `Retry-After`. Nunca cero cuando hay freno: un `Retry-After: 0` invita a
    /// reintentar de inmediato, que es lo contrario de lo que se pide.
    pub fn segundos_restantes(&self) -> u64 {
        if !self.frenado() {
            return 0;
        }
        // Redondeo hacia arriba a mano: `div_ceil` sobre enteros con signo
        // todavía no está estabilizado y esto no justifica exigir una versión
        // concreta del compilador.
        let ms = self.espera.num_milliseconds().max(1);
        ((ms + 999) / 1000).max(1) as u64
    }
}

/// La espera que corresponde a `fallos` fallos seguidos. Función pura: es la
/// regla, y se prueba sin base de datos.
pub fn espera_de(fallos: i64) -> Duration {
    // `<` y no `<=`: con `<=`, cinco fallos dejaban pasar un sexto intento libre
    // y el freno no aparecía hasta el séptimo. La diferencia es un intento, pero
    // era un intento entre lo que dice este módulo y lo que hacía — y dos
    // maneras de contar la misma regla garantizan que dentro de un año nadie
    // sepa cuál era. Lo cazó la prueba de la RUTA, no las de aquí.
    if fallos < UMBRAL {
        return Duration::zero();
    }
    // `saturating_*` y el techo antes de convertir: con 64 fallos, `1 << 63` ya
    // es desbordamiento, y un desbordamiento aquí daría una espera negativa —
    // es decir, ningún freno justo cuando más se está intentando.
    let exceso = (fallos - UMBRAL).min(32) as u32;
    let segundos = BASE_SEGUNDOS.saturating_mul(1i64 << exceso).min(TOPE_SEGUNDOS);
    Duration::seconds(segundos)
}

/// Anota un intento. Se llama SIEMPRE, con éxito o sin él.
pub async fn anotar(
    pool: &Pool,
    email: &str,
    origen: &str,
    exito: bool,
    ahora: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO intento_acceso (email, origen, momento, exito) VALUES (?, ?, ?, ?)",
    )
    .bind(email)
    .bind(origen)
    .bind(ahora.to_rfc3339())
    .bind(i64::from(exito))
    .execute(pool)
    .await?;

    // La poda se acota a ESTE (correo, origen) y a lo más viejo que la retención:
    // exactamente lo que esta llamada vuelve a escribir, y ni una fila más. Una
    // limpieza global aquí borraría el rastro de otras cuentas que nadie pidió
    // borrar, y eso no se vería hasta la segunda vuelta.
    let corte = (ahora - Duration::hours(RETENCION_HORAS)).to_rfc3339();
    sqlx::query(
        "DELETE FROM intento_acceso WHERE email = ? AND origen = ? AND momento < ?",
    )
    .bind(email)
    .bind(origen)
    .bind(corte)
    .execute(pool)
    .await?;
    Ok(())
}

/// El estado del freno para este (correo, origen) ahora mismo.
pub async fn estado(
    pool: &Pool,
    email: &str,
    origen: &str,
    ahora: DateTime<Utc>,
) -> Result<Estado, sqlx::Error> {
    let corte = (ahora - Duration::hours(RETENCION_HORAS)).to_rfc3339();

    // Desde cuándo se cuenta: el último acierto manda sobre el borde de
    // retención. Entrar bien borra la cuenta de fallos, que es lo que hace que
    // equivocarse un martes no penalice el miércoles.
    let ultimo_exito: Option<String> = sqlx::query_scalar(
        "SELECT MAX(momento) FROM intento_acceso \
         WHERE email = ? AND origen = ? AND exito = 1 AND momento >= ?",
    )
    .bind(email)
    .bind(origen)
    .bind(&corte)
    .fetch_one(pool)
    .await?;
    let desde = ultimo_exito.unwrap_or(corte);

    let filas: Vec<(String,)> = sqlx::query_as(
        "SELECT momento FROM intento_acceso \
         WHERE email = ? AND origen = ? AND exito = 0 AND momento > ? \
         ORDER BY momento DESC",
    )
    .bind(email)
    .bind(origen)
    .bind(&desde)
    .fetch_all(pool)
    .await?;

    let fallos = filas.len() as i64;
    let espera = match filas.first() {
        None => Duration::zero(),
        Some((ultimo,)) => {
            let castigo = espera_de(fallos);
            if castigo.is_zero() {
                Duration::zero()
            } else {
                // Una marca de tiempo ilegible no abre la puerta: se trata como
                // «ahora mismo», es decir, se cumple la espera entera. El camino
                // permisivo ante un dato roto es el que convierte un fallo de
                // datos en una vía de entrada.
                let cuando = DateTime::parse_from_rfc3339(ultimo)
                    .map(|t| t.with_timezone(&Utc))
                    .unwrap_or(ahora);
                let libre = cuando + castigo;
                if libre > ahora { libre - ahora } else { Duration::zero() }
            }
        }
    };

    Ok(Estado { fallos, espera })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    async fn base() -> Pool {
        crate::abrir("sqlite::memory:").await.unwrap()
    }

    #[test]
    fn hasta_el_umbral_no_hay_espera_y_despues_se_dobla() {
        // Los cinco fallos admitidos se responden sin espera; a partir del
        // quinto, el intento siguiente espera. El borde exacto está aquí y en la
        // prueba de la ruta, y tienen que decir lo mismo.
        for n in 0..UMBRAL {
            assert!(espera_de(n).is_zero(), "{n} fallos no deberían esperar");
        }
        assert_eq!(espera_de(UMBRAL).num_seconds(), BASE_SEGUNDOS);
        assert_eq!(espera_de(UMBRAL + 1).num_seconds(), BASE_SEGUNDOS * 2);
        assert_eq!(espera_de(UMBRAL + 2).num_seconds(), BASE_SEGUNDOS * 4);
    }

    #[test]
    fn la_espera_tiene_techo_y_no_se_desborda() {
        assert_eq!(espera_de(100).num_seconds(), TOPE_SEGUNDOS);
        // El caso que un `1 << n` sin cuidado convertiría en espera NEGATIVA, o
        // sea en ausencia de freno justo con el atacante más insistente.
        assert_eq!(espera_de(i64::MAX).num_seconds(), TOPE_SEGUNDOS);
        assert!(espera_de(i64::MAX) > Duration::zero());
    }

    #[tokio::test]
    async fn cuatro_fallos_no_frenan_y_el_quinto_si() {
        let pool = base().await;
        let t0 = Utc::now();
        for i in 0..(UMBRAL - 1) {
            anotar(&pool, "a@x.co", "1.2.3.4", false, t0 + Duration::seconds(i)).await.unwrap();
        }
        let e = estado(&pool, "a@x.co", "1.2.3.4", t0 + Duration::seconds(UMBRAL)).await.unwrap();
        assert_eq!(e.fallos, UMBRAL - 1);
        assert!(!e.frenado(), "cuatro fallos ya frenaban: eso castiga a quien teclea mal");

        anotar(&pool, "a@x.co", "1.2.3.4", false, t0 + Duration::seconds(UMBRAL)).await.unwrap();
        let e = estado(&pool, "a@x.co", "1.2.3.4", t0 + Duration::seconds(UMBRAL + 1)).await.unwrap();
        assert!(e.frenado(), "el quinto fallo no frenó el intento siguiente");
        assert!(e.segundos_restantes() >= 1);
    }

    #[tokio::test]
    async fn el_freno_es_de_quien_falla_y_no_del_dueno_de_la_cuenta() {
        // La prueba que justifica meter el origen en la clave: por muchas veces
        // que falle un desconocido contra el correo de la doctora, la doctora
        // entra desde su estación. Sin esto, cinco peticiones de fuera dejan una
        // historia clínica inaccesible.
        let pool = base().await;
        let t0 = Utc::now();
        for i in 0..20 {
            anotar(&pool, "doctora@x.co", "203.0.113.9", false, t0 + Duration::seconds(i))
                .await
                .unwrap();
        }
        let atacante = estado(&pool, "doctora@x.co", "203.0.113.9", t0 + Duration::seconds(21))
            .await
            .unwrap();
        assert!(atacante.frenado(), "el que insiste no quedó frenado");

        let ella = estado(&pool, "doctora@x.co", "10.0.0.5", t0 + Duration::seconds(21))
            .await
            .unwrap();
        assert!(!ella.frenado(), "la dueña de la cuenta quedó fuera por culpa de otro");
    }

    #[tokio::test]
    async fn entrar_bien_borra_la_cuenta_de_fallos() {
        let pool = base().await;
        let t0 = Utc::now();
        for i in 0..10 {
            anotar(&pool, "a@x.co", "1.2.3.4", false, t0 + Duration::seconds(i)).await.unwrap();
        }
        assert!(estado(&pool, "a@x.co", "1.2.3.4", t0 + Duration::seconds(11)).await.unwrap().frenado());

        anotar(&pool, "a@x.co", "1.2.3.4", true, t0 + Duration::seconds(600)).await.unwrap();
        let e = estado(&pool, "a@x.co", "1.2.3.4", t0 + Duration::seconds(601)).await.unwrap();
        assert_eq!(e.fallos, 0, "los fallos previos al acierto siguen contando");
        assert!(!e.frenado());
    }

    #[tokio::test]
    async fn la_espera_se_agota_sola() {
        let pool = base().await;
        let t0 = Utc::now();
        // Justo los fallos del umbral: la espera es la base, contada desde el
        // ÚLTIMO fallo. Con más fallos la espera se dobla y este cálculo tendría
        // que doblarse también — de ahí que las cuentas salgan de las constantes.
        for i in 0..UMBRAL {
            anotar(&pool, "a@x.co", "1.2.3.4", false, t0 + Duration::seconds(i)).await.unwrap();
        }
        let ultimo_fallo = t0 + Duration::seconds(UMBRAL - 1);
        assert!(estado(&pool, "a@x.co", "1.2.3.4", ultimo_fallo).await.unwrap().frenado());
        // Pasada la espera, se puede volver a probar SIN que nadie desbloquee
        // nada: es lo que separa un freno de un bloqueo.
        let luego = ultimo_fallo + Duration::seconds(BASE_SEGUNDOS + 1);
        assert!(!estado(&pool, "a@x.co", "1.2.3.4", luego).await.unwrap().frenado());
    }

    #[tokio::test]
    async fn un_correo_que_no_existe_se_frena_igual() {
        // Si solo se frenaran las cuentas reales, recibir el freno diría que el
        // correo existe — justo lo que el login evita dando el mismo error para
        // todo. Aquí no hay `usuario_id`: se cuenta la cadena que se escribió.
        let pool = base().await;
        let t0 = Utc::now();
        for i in 0..6 {
            anotar(&pool, "no-existe@x.co", "1.2.3.4", false, t0 + Duration::seconds(i))
                .await
                .unwrap();
        }
        assert!(estado(&pool, "no-existe@x.co", "1.2.3.4", t0 + Duration::seconds(7))
            .await
            .unwrap()
            .frenado());
    }

    #[tokio::test]
    async fn la_poda_solo_toca_a_su_propia_pareja() {
        let pool = base().await;
        let ahora = Utc::now();
        let viejo = ahora - Duration::hours(RETENCION_HORAS + 1);
        anotar(&pool, "otra@x.co", "9.9.9.9", false, viejo).await.unwrap();
        anotar(&pool, "a@x.co", "1.2.3.4", false, viejo).await.unwrap();

        // Anotar para (a@x.co, 1.2.3.4) poda lo viejo DE ESA pareja...
        anotar(&pool, "a@x.co", "1.2.3.4", false, ahora).await.unwrap();
        let quedan: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM intento_acceso WHERE email = 'a@x.co'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(quedan, 1, "no podó lo caducado de su propia pareja");

        // ...y NO toca el de otra cuenta, que nadie pidió borrar.
        let ajenos: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM intento_acceso WHERE email = 'otra@x.co'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(ajenos, 1, "la poda se llevó filas de otra cuenta");
    }

    /// Directiva #8: 120 intentos concurrentes contra la misma pareja no pierden
    /// ninguno ni se estorban, y el freno queda puesto. Con plazo, para que un
    /// interbloqueo no se lea como lentitud.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn ciento_veinte_intentos_a_la_vez() {
        let pool = base().await;
        let t0 = Utc::now();
        let plazo = std::time::Duration::from_secs(60);

        let mut tareas = Vec::new();
        for i in 0..120i64 {
            let p = pool.clone();
            tareas.push(tokio::spawn(async move {
                anotar(&p, "a@x.co", "1.2.3.4", false, t0 + Duration::milliseconds(i)).await
            }));
        }
        for t in tareas {
            tokio::time::timeout(plazo, t)
                .await
                .expect("un intento no respondió a tiempo: ¿interbloqueo?")
                .expect("la tarea no debía entrar en panic")
                .expect("una anotación falló");
        }

        let e = estado(&pool, "a@x.co", "1.2.3.4", t0 + Duration::seconds(1)).await.unwrap();
        assert_eq!(e.fallos, 120, "se perdieron intentos bajo concurrencia");
        // `espera` es lo que FALTA, no el castigo: el último fallo se anotó unos
        // milisegundos después de t0 y se consulta un segundo más tarde, así que
        // el restante es algo menor que el tope. Comparar el restante con el tope
        // era comparar dos magnitudes distintas.
        assert_eq!(espera_de(e.fallos).num_seconds(), TOPE_SEGUNDOS, "el castigo no llegó al tope");
        assert!(e.frenado(), "120 intentos seguidos no dejaron freno puesto");
        assert!(
            e.espera <= Duration::seconds(TOPE_SEGUNDOS)
                && e.espera > Duration::seconds(TOPE_SEGUNDOS - 5),
            "restante fuera de rango: {:?}",
            e.espera
        );
    }
}
