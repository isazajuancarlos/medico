//! El freno al ensayo de contraseñas: aquí las dos consultas, la regla en guaca.
//!
//! # Qué se movió y por qué
//!
//! Este módulo tenía la regla entera —umbral, techo, qué borra la cuenta de
//! fallos, qué hacer con una marca de tiempo ilegible— y informes no tenía
//! ninguna. Escribirla dos veces es garantizar que dentro de un año una de las
//! dos tenga el techo y la otra no; es exactamente la constatación que dio origen
//! a `guaca` («las copias YA habían divergido»).
//!
//! Así que la regla vive en [`guaca::freno`] y aquí queda lo que de verdad es de
//! medico: dónde se guardan los intentos y cómo se podan. La app trae sus filas,
//! `evaluar` decide.
//!
//! # La regla, para no tener que ir a buscarla
//!
//! Tras cinco fallos seguidos desde el mismo origen contra el mismo correo, el
//! siguiente intento espera; la espera se dobla con cada fallo y se detiene a los
//! cinco minutos. Un acierto lo borra todo. **No bloquea la cuenta**: un bloqueo
//! que exige a alguien desbloquear convierte cinco peticiones de un desconocido
//! en una historia clínica inaccesible a las tres de la mañana.
//!
//! El razonamiento completo —y lo que se rechazó al diseñarla— está en el módulo
//! de guaca, que es donde ahora se audita una sola vez.

use chrono::{DateTime, Utc};

use crate::Pool;

// La regla se reexporta para que quien ya la usaba —`rutas.rs` cuenta con
// `freno::UMBRAL` en sus pruebas— no tenga que saber que se mudó. Lo que NO se
// hace es copiar los valores: son los de guaca o no son.
pub use guaca::freno::{
    espera_de, Estado, BASE_SEGUNDOS, RETENCION_HORAS, TOPE_SEGUNDOS, UMBRAL,
};

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
    //
    // El borde sale de guaca y no de una resta local: con dos bordes de la misma
    // regla, la poda acaba borrando lo que el conteo todavía cuenta, o al revés.
    sqlx::query(
        "DELETE FROM intento_acceso WHERE email = ? AND origen = ? AND momento < ?",
    )
    .bind(email)
    .bind(origen)
    .bind(guaca::freno::corte_retencion(ahora).to_rfc3339())
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
    // Una sola consulta, sin `ORDER BY` ni filtro de fecha: antes eran dos —una
    // para el último acierto y otra para los fallos posteriores—, y el criterio
    // estaba repartido entre el SQL y el Rust. `evaluar` no depende del orden y
    // aplica la retención por su cuenta, así que aquí no queda ninguna decisión.
    let filas: Vec<(String, i64)> = sqlx::query_as(
        "SELECT momento, exito FROM intento_acceso WHERE email = ? AND origen = ?",
    )
    .bind(email)
    .bind(origen)
    .fetch_all(pool)
    .await?;

    let intentos: Vec<guaca::freno::Intento<'_>> = filas
        .iter()
        .map(|(momento, exito)| guaca::freno::Intento {
            momento: momento.as_str(),
            exito: *exito != 0,
        })
        .collect();
    Ok(guaca::freno::evaluar(&intentos, ahora))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use chrono::Duration;

    // Las pruebas de la REGLA pura —el borde del umbral, el techo, la marca de
    // tiempo ilegible— se fueron con ella a guaca, donde se corren en su CI. Aquí
    // se prueba lo que aquí se decide: que las filas se escriben, que la poda no
    // se pasa de su pareja, y que el conjunto sigue frenando a quien insiste sin
    // dejar fuera al dueño de la cuenta.

    async fn base() -> Pool {
        crate::abrir("sqlite::memory:").await.unwrap()
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

    #[tokio::test]
    async fn lo_que_la_poda_dejo_pasar_tampoco_lo_cuenta_el_freno() {
        // Los dos bordes son uno. Si un día la poda falla —o alguien toca el
        // `DELETE`—, el freno no debe empezar a contar fallos de anteayer: se
        // escriben veinte fallos viejos SIN pasar por `anotar`, que es la única
        // forma de que sobrevivan a la poda.
        let pool = base().await;
        let ahora = Utc::now();
        let viejo = (ahora - Duration::hours(RETENCION_HORAS + 2)).to_rfc3339();
        for _ in 0..20 {
            sqlx::query(
                "INSERT INTO intento_acceso (email, origen, momento, exito) VALUES (?,?,?,0)",
            )
            .bind("a@x.co")
            .bind("1.2.3.4")
            .bind(&viejo)
            .execute(&pool)
            .await
            .unwrap();
        }
        let e = estado(&pool, "a@x.co", "1.2.3.4", ahora).await.unwrap();
        assert_eq!(e.fallos, 0, "el freno contó fallos fuera de la retención");
        assert!(!e.frenado());
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
