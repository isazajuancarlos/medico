// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! La matriz de capacidades de cada institución: leerla, guardarla y tenerla a
//! mano.
//!
//! Se consulta en CADA petición autorizada, así que no puede costar un viaje a
//! la base cada vez. De ahí la caché; y la caché vive aquí, pegada a la
//! escritura, porque la invalidación es la mitad difícil y separarla de la
//! escritura es la manera segura de olvidarla.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use medico_dominio::ajustes::Ajustes;
use medico_dominio::rol::{Capacidad, RolUsuario};

/// Lee de la base las excepciones de una institución.
pub async fn leer(pool: &crate::Pool, institucion_id: i64) -> Result<Ajustes, sqlx::Error> {
    let filas: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT capacidad, rol, permitido FROM capacidad_rol WHERE institucion_id = ?",
    )
    .bind(institucion_id)
    .fetch_all(pool)
    .await?;
    Ok(Ajustes::desde_filas(
        filas.into_iter().map(|(c, r, p)| (c, r, p != 0)),
    ))
}

/// Escribe un conjunto de excepciones YA VALIDADO con
/// [`Ajustes::validar`](medico_dominio::ajustes::Ajustes::validar).
///
/// Toma la lista de cambios y no el `Ajustes` entero a propósito: guardar el
/// estado completo obligaría a decidir qué hacer con las filas que no vienen, y
/// la respuesta correcta —dejarlas— es justo la que se olvida. Un cambio dice
/// qué casilla se tocó.
///
/// Va en UNA transacción: media matriz aplicada es un estado de autorización
/// que nadie decidió.
pub async fn guardar(
    pool: &crate::Pool,
    institucion_id: i64,
    cambios: &[(Capacidad, RolUsuario, bool)],
    quien_id: i64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for (capacidad, rol, permitido) in cambios {
        sqlx::query(
            "INSERT INTO capacidad_rol (institucion_id, capacidad, rol, permitido, cambiado_por)
             VALUES (?,?,?,?,?)
             ON CONFLICT (institucion_id, capacidad, rol)
             DO UPDATE SET permitido = excluded.permitido,
                           cambiado_por = excluded.cambiado_por,
                           cambiado_en = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
        )
        .bind(institucion_id)
        .bind(capacidad.como_str())
        .bind(rol.como_str())
        .bind(i64::from(*permitido))
        .bind(quien_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

/// Devuelve una casilla al defecto, borrando la excepción.
///
/// No es lo mismo que marcarla con el valor que hoy tiene el defecto: eso la
/// CONGELA, y dejaría de moverse el día que el defecto cambie. La pantalla
/// necesita las dos operaciones y por eso son dos.
pub async fn devolver_al_defecto(
    pool: &crate::Pool,
    institucion_id: i64,
    capacidad: Capacidad,
    rol: RolUsuario,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM capacidad_rol WHERE institucion_id = ? AND capacidad = ? AND rol = ?")
        .bind(institucion_id)
        .bind(capacidad.como_str())
        .bind(rol.como_str())
        .execute(pool)
        .await
        .map(|_| ())
}

/// Las matrices ya leídas, por institución.
///
/// # Por qué no caduca por tiempo
///
/// Una caché con vencimiento tendría dos maneras de estar al día —que venza y
/// que se invalide— y la primera taparía los fallos de la segunda: un olvido en
/// la invalidación se vería como «tarda un minuto en aplicarse» en vez de como
/// un fallo. Aquí solo hay una manera, así que si falla, se nota.
///
/// El coste de esa decisión es que un cambio hecho DIRECTAMENTE en la base no
/// se ve hasta reiniciar. Es el precio correcto: la API es la vía, y tocar la
/// base a mano ya obliga a saber lo que se hace.
#[derive(Debug, Default)]
pub struct Cache {
    mapa: RwLock<HashMap<i64, Arc<Ajustes>>>,
}

impl Cache {
    pub fn nueva() -> Self {
        Self::default()
    }

    /// Los ajustes de una institución, leyéndolos si aún no están.
    ///
    /// **Si la lectura falla, falla la petición** (directiva 20). Caer al
    /// defecto sería peor que un error: devolvería a un rol justo la capacidad
    /// que esta institución le había quitado, y nadie se enteraría.
    pub async fn ajustes(
        &self,
        pool: &crate::Pool,
        institucion_id: i64,
    ) -> Result<Arc<Ajustes>, sqlx::Error> {
        // El candado NUNCA se sostiene cruzando un `await`: se mira, se suelta,
        // se consulta, se vuelve a tomar. Dos peticiones simultáneas de la misma
        // institución pueden leer las dos de la base; es trabajo repetido una
        // vez, no una incoherencia.
        if let Some(a) = self
            .mapa
            .read()
            .expect("el candado de la caché no se envenena: dentro no hay pánico posible")
            .get(&institucion_id)
        {
            return Ok(Arc::clone(a));
        }
        let ajustes = Arc::new(leer(pool, institucion_id).await?);
        self.mapa
            .write()
            .expect("el candado de la caché no se envenena")
            .insert(institucion_id, Arc::clone(&ajustes));
        Ok(ajustes)
    }

    /// Olvida lo que sabía de una institución. Se llama tras CADA escritura.
    pub fn invalidar(&self, institucion_id: i64) {
        self.mapa
            .write()
            .expect("el candado de la caché no se envenena")
            .remove(&institucion_id);
    }

    /// Cuántas instituciones hay cargadas. Solo para pruebas.
    pub fn cargadas(&self) -> usize {
        self.mapa.read().expect("el candado de la caché no se envenena").len()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    async fn base() -> crate::Pool {
        static N: AtomicU64 = AtomicU64::new(0);
        let id = N.fetch_add(1, Ordering::Relaxed);
        let url = format!(
            "sqlite:file:capacidad_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            id
        );
        let pool = crate::abrir(&url).await.unwrap();
        for i in [1_i64, 2] {
            sqlx::query("INSERT INTO institucion (id, nombre, nit) VALUES (?,?,?)")
                .bind(i)
                .bind(format!("IPS {i}"))
                .bind(format!("90{i}"))
                .execute(&pool)
                .await
                .unwrap();
        }
        // Usuarios reales: `cambiado_por` tiene clave foránea, y pasarle un id
        // inventado hacía fallar la escritura. La restricción está bien; era la
        // prueba la que mentía sobre quién hizo el cambio.
        for (id, inst) in [(ADMIN_1, 1_i64), (ADMIN_1_BIS, 1), (ADMIN_2, 2)] {
            sqlx::query(
                "INSERT INTO usuario (id, nombre, email, password_hash, rol, institucion_id)
                 VALUES (?,?,?,'x','admin',?)",
            )
            .bind(id)
            .bind(format!("Admin {id}"))
            .bind(format!("admin{id}@ips.test"))
            .bind(inst)
            .execute(&pool)
            .await
            .unwrap();
        }
        pool
    }

    const ADMIN_1: i64 = 7;
    const ADMIN_1_BIS: i64 = 8;
    const ADMIN_2: i64 = 9;

    #[tokio::test]
    async fn sin_filas_no_hay_excepciones() {
        let pool = base().await;
        let a = leer(&pool, 1).await.unwrap();
        assert_eq!(a.cuantos(), 0);
        assert!(a.puede(RolUsuario::Medico, Capacidad::EscribirClinico));
    }

    #[tokio::test]
    async fn lo_guardado_se_lee() {
        let pool = base().await;
        guardar(
            &pool,
            1,
            &[
                (Capacidad::EscribirClinico, RolUsuario::Laboratorista, true),
                (Capacidad::EscribirNutricion, RolUsuario::Medico, false),
            ],
            ADMIN_1,
        )
        .await
        .unwrap();

        let a = leer(&pool, 1).await.unwrap();
        assert!(a.puede(RolUsuario::Laboratorista, Capacidad::EscribirClinico));
        assert!(!a.puede(RolUsuario::Medico, Capacidad::EscribirNutricion));
    }

    /// El aislamiento, que en este sistema es la propiedad que más caro sale
    /// romper: la matriz de una institución no toca la de la otra.
    #[tokio::test]
    async fn la_matriz_de_una_institucion_no_alcanza_a_la_otra() {
        let pool = base().await;
        guardar(
            &pool,
            1,
            &[(Capacidad::EscribirClinico, RolUsuario::Laboratorista, true)],
            ADMIN_1,
        )
        .await
        .unwrap();

        assert!(leer(&pool, 1)
            .await
            .unwrap()
            .puede(RolUsuario::Laboratorista, Capacidad::EscribirClinico));
        assert!(
            !leer(&pool, 2)
                .await
                .unwrap()
                .puede(RolUsuario::Laboratorista, Capacidad::EscribirClinico),
            "la institución 2 sigue el defecto"
        );
    }

    #[tokio::test]
    async fn guardar_dos_veces_la_misma_casilla_la_actualiza() {
        let pool = base().await;
        let casilla = (Capacidad::EscribirClinico, RolUsuario::Laboratorista, true);
        guardar(&pool, 1, &[casilla], ADMIN_1).await.unwrap();
        guardar(&pool, 1, &[(casilla.0, casilla.1, false)], ADMIN_1_BIS).await.unwrap();

        let a = leer(&pool, 1).await.unwrap();
        assert_eq!(a.cuantos(), 1, "se actualiza, no se duplica");
        assert!(!a.puede(RolUsuario::Laboratorista, Capacidad::EscribirClinico));
    }

    /// Devolver al defecto BORRA la fila, y eso no es lo mismo que marcarla con
    /// el valor que hoy tiene el defecto: la borrada sigue al defecto si el
    /// defecto cambia, la marcada se queda donde estaba.
    #[tokio::test]
    async fn devolver_al_defecto_borra_la_excepcion() {
        let pool = base().await;
        guardar(
            &pool,
            1,
            &[(Capacidad::EscribirClinico, RolUsuario::Medico, false)],
            ADMIN_1,
        )
        .await
        .unwrap();
        assert_eq!(leer(&pool, 1).await.unwrap().cuantos(), 1);

        devolver_al_defecto(&pool, 1, Capacidad::EscribirClinico, RolUsuario::Medico)
            .await
            .unwrap();

        let a = leer(&pool, 1).await.unwrap();
        assert_eq!(a.cuantos(), 0, "no queda fila, no queda una fila con el valor del defecto");
        assert!(a.puede(RolUsuario::Medico, Capacidad::EscribirClinico));
    }

    // === LA CACHÉ =========================================================

    #[tokio::test]
    async fn la_cache_devuelve_lo_mismo_que_la_base() {
        let pool = base().await;
        guardar(
            &pool,
            1,
            &[(Capacidad::EscribirClinico, RolUsuario::Laboratorista, true)],
            ADMIN_1,
        )
        .await
        .unwrap();

        let cache = Cache::nueva();
        assert_eq!(cache.cargadas(), 0);
        let a = cache.ajustes(&pool, 1).await.unwrap();
        assert!(a.puede(RolUsuario::Laboratorista, Capacidad::EscribirClinico));
        assert_eq!(cache.cargadas(), 1);
        // La segunda no vuelve a la base y da lo mismo.
        let b = cache.ajustes(&pool, 1).await.unwrap();
        assert_eq!(a, b);
    }

    /// **La prueba que importa de una caché**: que sin invalidar sirve lo viejo
    /// —o sea, que de verdad cachea— y que al invalidar sirve lo nuevo. Sin la
    /// primera mitad, una caché que no cachea pasaría igual.
    #[tokio::test]
    async fn sin_invalidar_sirve_lo_viejo_y_al_invalidar_lo_nuevo() {
        let pool = base().await;
        let cache = Cache::nueva();

        // Estado inicial: el defecto, que NO deja escribir al laboratorista.
        assert!(!cache
            .ajustes(&pool, 1)
            .await
            .unwrap()
            .puede(RolUsuario::Laboratorista, Capacidad::EscribirClinico));

        // Cambia la base por detrás, sin invalidar.
        guardar(
            &pool,
            1,
            &[(Capacidad::EscribirClinico, RolUsuario::Laboratorista, true)],
            ADMIN_1,
        )
        .await
        .unwrap();
        assert!(
            !cache
                .ajustes(&pool, 1)
                .await
                .unwrap()
                .puede(RolUsuario::Laboratorista, Capacidad::EscribirClinico),
            "sigue sirviendo lo cacheado: si esto fallara, la caché no cachea"
        );

        cache.invalidar(1);
        assert!(
            cache
                .ajustes(&pool, 1)
                .await
                .unwrap()
                .puede(RolUsuario::Laboratorista, Capacidad::EscribirClinico),
            "tras invalidar, la caché sirve lo nuevo"
        );
    }

    #[tokio::test]
    async fn invalidar_una_institucion_no_tira_la_otra() {
        let pool = base().await;
        let cache = Cache::nueva();
        cache.ajustes(&pool, 1).await.unwrap();
        cache.ajustes(&pool, 2).await.unwrap();
        assert_eq!(cache.cargadas(), 2);
        cache.invalidar(1);
        assert_eq!(cache.cargadas(), 1, "la 2 sigue cargada");
    }

    /// Directiva 8: 100+ lecturas concurrentes sobre la misma institución no se
    /// estorban ni devuelven cosas distintas, con plazo para distinguir un
    /// interbloqueo de lentitud (la lección de #233).
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn cien_lecturas_concurrentes_dan_todas_lo_mismo() {
        let pool = base().await;
        guardar(
            &pool,
            1,
            &[(Capacidad::EscribirClinico, RolUsuario::Laboratorista, true)],
            ADMIN_1,
        )
        .await
        .unwrap();
        let cache = Arc::new(Cache::nueva());

        let mut tareas = Vec::new();
        for _ in 0..8 {
            let c = Arc::clone(&cache);
            let p = pool.clone();
            tareas.push(tokio::spawn(async move {
                let mut vistos = Vec::new();
                for _ in 0..13 {
                    let a = c.ajustes(&p, 1).await?;
                    vistos.push(a.puede(RolUsuario::Laboratorista, Capacidad::EscribirClinico));
                }
                Ok::<Vec<bool>, sqlx::Error>(vistos)
            }));
        }

        let plazo = std::time::Duration::from_secs(60);
        let mut total = 0;
        for t in tareas {
            let vistos = tokio::time::timeout(plazo, t)
                .await
                .expect("una lectura no respondió a tiempo: ¿interbloqueo?")
                .expect("la tarea no debía entrar en panic")
                .expect("leer la matriz no debe fallar");
            assert!(vistos.iter().all(|&v| v), "alguna lectura dio el defecto");
            total += vistos.len();
        }
        assert!(total >= 100, "solo se hicieron {total} lecturas");
        assert_eq!(cache.cargadas(), 1);
    }
}
