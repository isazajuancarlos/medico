//! Acceso a la base. Nadie de aquí para arriba escribe SQL.

pub mod acceso;
pub mod agenda;
pub mod ambulancia;
pub mod capacidad;
pub mod catalogo;
pub mod cita;
pub mod clinico;
pub mod consentimiento;
pub mod diagnostico;
pub mod divipola;
pub mod documento;
pub mod estadisticas;
pub mod facturacion;
pub mod formula;
pub mod freno;
pub mod historia;
pub mod hospitalizacion;
pub mod imagen;
pub mod incapacidad;
pub mod institucion;
pub mod inventario;
pub mod licencia;
pub mod nutricion;
pub mod orden;
pub mod paciente;
pub mod proveedor;
pub mod referencia;
pub mod rehabilitacion;
pub mod rips;
pub mod urgencias;
pub mod usuario;

pub type Pool = sqlx::SqlitePool;

/// Qué día es HOY para el prestador, en `YYYY-MM-DD`.
///
/// **No es `Utc::now().date_naive()`**, y la diferencia no es cosmética: entre
/// las 19:00 y la medianoche de Bogotá (UTC−5) esa expresión devuelve el día
/// SIGUIENTE. Todo lo que se fecha con ella en la última franja de la tarde
/// —justo cuando cierra una farmacia— queda un día adelantado.
///
/// # Por qué existía el fallo, que es la parte que interesa
///
/// El 2026-08-03 se barrió el proyecto por este mismo motivo y se arreglaron
/// cuatro sitios. Aquel barrido buscó **`Local`**, y estos dos usaban **`Utc`**:
/// la pareja del caso ruidoso no se miró, así que sobrevivieron
/// `inventario::hoy()` y `ambulancia::hoy()` (directiva 23 — un falso negativo
/// que comparte causa con el ruidoso y no se queja nunca).
///
/// En el portátil de un consultorio, con el reloj en hora local, el error no
/// aparece. En el VPS de la demostración, que corre en `Etc/UTC`, aparece todos
/// los días.
///
/// # Lo que SÍ debe seguir usando `Utc::now()`
///
/// Un **instante** que se guarda (`to_rfc3339_opts`) va en UTC y así está bien:
/// es un punto en la línea del tiempo, no un día del calendario, y
/// [`medico_dominio::zona::desde_utc`] existe justamente para leerlo después en
/// la zona del prestador. Lo que no se puede es derivar una FECHA de él.
pub fn hoy() -> String {
    medico_dominio::zona::hoy().to_string()
}

/// El instante en que ocurre algo, en UTC y con segundos: `2026-08-05T21:04:00Z`.
///
/// Es el otro lado de [`hoy`], y por eso está justo aquí: un **instante** que se
/// guarda va en UTC a propósito —es un punto en la línea del tiempo, no un día
/// del calendario— y se lee después en la zona del prestador con
/// [`medico_dominio::zona::desde_utc`]. Lo que no se puede es derivar una FECHA
/// de él, que es el error que costó la tarde del 2026-08-03.
///
/// Vive en `lib.rs` desde que hicieron falta tres copias: `diagnostico.rs` y
/// `ambulancia.rs` la tenían idéntica en privado, y una tercera en
/// `consentimiento.rs` era el momento en que dos de ellas empiezan a divergir
/// —basta que alguien cambie `Secs` por `Millis` en una— y las fechas de dos
/// módulos dejan de poder compararse (directiva 9).
pub fn ahora() -> String {
    use chrono::{SecondsFormat, Utc};
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// El trozo de `WHERE` que acota una consulta al ámbito de quien pregunta, y el
/// valor que hay que enlazar (o `None` si no hay ninguno).
///
/// **Devolver `None` significa «no ve nada», y el llamante tiene que cortar ahí.**
/// Ese es el diseño: `Ambito::Ninguno` no produce un filtro laxo ni un `1 = 1`,
/// produce la ausencia de consulta. El `let … else` de cada función es lo que lo
/// hace imposible de olvidar — no compila si no se decide qué devolver.
///
/// # Por qué vive aquí desde el 2026-08-05
///
/// Había SEIS copias byte a byte de esta función —`proveedor`, `rehabilitacion`,
/// `ambulancia`, `diagnostico`, `inventario`, `nutricion`—, comprobado con
/// `md5sum`: las seis daban `6fad6250d033`. Al escribir `facturacion` tocaba
/// añadir la séptima, y una regla de aislamiento repetida siete veces es una
/// regla que se arregla en seis sitios y se olvida en el séptimo (directiva 1: si
/// vas a añadir un mecanismo para sostener lo que creó otro, quita el primero).
///
/// No es refactor por estética: es que esta función decide quién ve los datos de
/// quién. El día que haya que cambiarla —un ámbito nuevo, una institución con
/// sedes— tiene que haber UN sitio donde cambiarla.
pub fn filtro_ambito(
    ambito: medico_dominio::Ambito,
    col: &str,
) -> Option<(String, Option<i64>)> {
    use medico_dominio::Ambito;
    match ambito {
        Ambito::Superadmin => Some(("1 = 1".into(), None)),
        Ambito::Institucion(i) => Some((format!("{col} = ?"), Some(i))),
        Ambito::Ninguno => None,
    }
}

/// Resultado de editar o borrar algo que puede estar firmado. `Firmada` NO es
/// `NoExiste`: existe y es del ámbito, pero está cerrado. Lo comparten la
/// historia y los documentos clínicos, que aplican la misma inmutabilidad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resultado {
    NoExiste,
    Firmada,
    Hecho,
}

/// Abre la base y aplica las migraciones pendientes.
///
/// `create_if_missing` está a propósito: el despliegue en sitio es un binario y
/// nada más, y pedirle a un consultorio que cree la base a mano antes de
/// arrancar es justo el paso donde se pierde una instalación.
///
/// Las dos opciones que NO son cosméticas:
///
/// - **`foreign_keys(true)`.** SQLite las trae APAGADAS por defecto, por
///   compatibilidad hacia atrás. Con ellas apagadas, un paciente puede apuntar
///   a una institución que no existe; esa fila queda fuera del alcance de todo
///   filtro por institución, así que no la ve nadie —ni siquiera para
///   corregirla— y sigue ahí. Ver `las_claves_foraneas_estan_activas`.
/// - **WAL.** Lectores concurrentes sin bloquear al escritor. No convierte a
///   SQLite en multi-escritor —ese techo se aceptó con los ojos abiertos en
///   `decisiones/0008`— pero es la diferencia entre un consultorio que va bien
///   y uno que se traba al consultar mientras alguien guarda.
pub async fn abrir(url: &str) -> Result<Pool, sqlx::Error> {
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use std::str::FromStr;

    let opciones = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);

    // LAS MIGRACIONES CORREN CON LAS CLAVES FORÁNEAS APAGADAS, Y DESPUÉS SE
    // COMPRUEBA QUE NO SE ROMPIÓ NADA.
    //
    // Es lo que exige el procedimiento de 12 pasos que SQLite documenta para
    // reconstruir una tabla, y hasta el 2026-08-05 este proyecto creía tenerlo
    // porque la migración 0045 empieza con `PRAGMA foreign_keys = OFF`. **Esa
    // línea es INERTE**: `sqlx` envuelve CADA migración en su propia transacción
    // (`sqlx-sqlite-0.8.6/src/migrate.rs:136`), y SQLite documenta que ese
    // pragma es un no-op dentro de una transacción.
    //
    // La consecuencia se midió con el migrador real y su control, no se dedujo:
    // el `DROP TABLE factura` de la 0045 dispara el `ON DELETE CASCADE` de
    // `detalle_factura` y `glosa`, y las vacía —6→0 y 3→0—. Con las foráneas
    // apagadas de verdad, las filas sobreviven. Y la migración se registra como
    // aplicada CON ÉXITO: ni un error, y `PRAGMA foreign_key_check` sale limpio
    // después, así que nada delata la pérdida.
    //
    // Hoy no se perdió una sola fila —esas tablas nacieron en la misma rama y no
    // hay binario Rust desplegado— pero la migración es inmutable y se queda en
    // el repositorio como la receta que alguien copiará para la próxima tabla
    // poblada. Se arregla aquí, que es donde se puede.
    //
    // `-- no-transaction` NO sirve: `sqlx-postgres` lo respeta, `sqlx-sqlite`
    // 0.8.6 no lo menciona. `PRAGMA defer_foreign_keys` TAMPOCO: difiere las
    // violaciones, no las acciones `CASCADE` (medido: 6→0 igual).
    //
    // Y apagarlas no se queda gratis: `foreign_key_check` después es lo que
    // convierte «confío en que las migraciones estén bien» en una comprobación.
    // Si una dejó huérfanos, el arranque FALLA en vez de servir una base rota.
    // EN MEMORIA NO SE SEPARA EL MIGRADOR, y esto no es un caso especial gratuito:
    // con `sqlite::memory:` **cada conexión es una base distinta**, así que un
    // pool aparte migraría una base y el pool real abriría otra vacía. Lo aprendí
    // rompiendo 121 pruebas de golpe con la primera versión de este arreglo.
    //
    // Y no pierde nada: una base en memoria nace vacía en cada corrida, así que
    // la reconstrucción de tabla no tiene filas hijas que llevarse. El peligro
    // que esto ataja es el de una base EN DISCO con datos.
    let en_memoria = url.contains(":memory:") || url.contains("mode=memory");

    let migrador = if en_memoria {
        // Las MISMAS 8 conexiones que el pool real: este pool ES el que se
        // devuelve, así que estrecharlo a una cambiaba la concurrencia de todas
        // las pruebas — y una se cayó con 500 por esperar un turno que no
        // llegaba. El arreglo de un problema no puede traer otro de regalo.
        SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(opciones.clone())
            .await?
    } else {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str(url)?
                    .create_if_missing(true)
                    .foreign_keys(false)
                    .journal_mode(SqliteJournalMode::Wal),
            )
            .await?
    };

    sqlx::migrate!("./migrations").run(&migrador).await?;

    // El `?` NO es cosmético y sustituye a un `.unwrap_or_default()`: con él, una
    // consulta que FALLA devolvía el vector vacío, o sea el mismo valor que «no
    // hay huérfanas». La comprobación se saltaba sola y la base se abría dando el
    // veredicto bueno sin haberlo medido (directiva 30: un negativo solo vale si
    // el camino que lo produciría llegó a ejecutarse). Costaba un carácter.
    let huerfanos: Vec<(String,)> = match sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(&migrador)
        .await
    {
        Ok(filas) => filas,
        Err(e) => {
            migrador.close().await;
            return Err(e);
        }
    };
    if !huerfanos.is_empty() {
        let tablas: Vec<&str> = huerfanos.iter().map(|(t,)| t.as_str()).collect();
        migrador.close().await;
        return Err(sqlx::Error::Protocol(format!(
            "las migraciones dejaron filas huérfanas en {tablas:?}. La base NO se abre: \
             servirla así entrega registros que no puede alcanzar ningún filtro."
        )));
    }
    if en_memoria {
        // Se devuelve el MISMO pool: abrir otro daría una base vacía.
        return Ok(migrador);
    }
    migrador.close().await;

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opciones)
        .await?;

    Ok(pool)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// `hoy()` es la del PRESTADOR, no la del reloj de la máquina.
    ///
    /// Hoy es tautológico —una línea que delega— y esa es la intención: pinta la
    /// delegación para que quitarla se note. El mecanismo de verdad lo prueba
    /// `medico_dominio::zona`, con instantes FIJOS y no con la hora actual
    /// (`la_atencion_de_la_noche_cambia_de_dia_al_guardarse`).
    #[test]
    fn hoy_es_la_fecha_del_prestador() {
        assert_eq!(hoy(), medico_dominio::zona::hoy().to_string());
    }

    /// GUARDIÁN: nadie vuelve a derivar una FECHA de `Utc`.
    ///
    /// Existe porque el barrido del 2026-08-03 buscó `Local` y estos dos usaban
    /// `Utc`, así que sobrevivieron sin que nada fallara —`inventario::hoy()` y
    /// `ambulancia::hoy()`, encontrados el 2026-08-05—. Un barrido a mano se
    /// hace una vez; esto se ejecuta en cada corrida.
    ///
    /// **Discrimina, y esa es la mitad que importa**: `Utc::now()` para guardar
    /// un INSTANTE (`to_rfc3339_opts`) es correcto y abunda en este crate — es
    /// un punto en la línea del tiempo, no un día del calendario—. El guardián
    /// solo caza `.date_naive()`, y el control de abajo comprueba que el patrón
    /// permitido sigue estando presente: si algún día no lo estuviera, este
    /// guardián estaría midiendo un árbol vacío y diría que sí a todo.
    #[test]
    fn ninguna_fecha_se_deriva_de_utc() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut culpables = Vec::new();
        let mut instantes = 0usize;
        let mut leidos = 0usize;

        for entrada in std::fs::read_dir(&dir).expect("no se pudo leer src/") {
            let ruta = entrada.expect("entrada ilegible").path();
            if ruta.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let texto = std::fs::read_to_string(&ruta).expect("archivo ilegible");
            leidos += 1;
            for (n, linea) in texto.lines().enumerate() {
                // Los comentarios NO cuentan. La primera versión de este
                // guardián se cazó a sí mismo dos veces —su propio doc de
                // `hoy()` y la línea de abajo— y arreglarlo caso a caso habría
                // sido excluir este archivo, que es justo como un guardián deja
                // de vigilarse. Se arregla en la REGLA: se mira código.
                let limpia = linea.trim_start();
                if limpia.starts_with("//") || limpia.starts_with("*") {
                    continue;
                }
                // La aguja va PARTIDA a propósito, para que no exista literal en
                // el código de su propio guardián. Escribirla entera es la otra
                // mitad del mismo falso positivo.
                let aguja = concat!("Utc::now()", ".date_naive()");
                if linea.contains(aguja) {
                    culpables.push(format!("{}:{}", ruta.display(), n + 1));
                }
                // El control: guardar un INSTANTE en UTC, que sí está bien.
                if linea.contains("Utc::now()") && linea.contains("to_rfc3339") {
                    instantes += 1;
                }
            }
        }

        assert!(leidos > 10, "solo se leyeron {leidos} archivos: el guardián no está mirando el árbol");
        assert!(
            instantes > 0,
            "CONTROL EN ROJO: no se encontró ni un `Utc::now().to_rfc3339…`. \
             O el crate dejó de guardar instantes, o este guardián no está \
             leyendo lo que cree — en los dos casos su verde no significa nada."
        );
        assert!(
            culpables.is_empty(),
            "una fecha derivada de UTC en {culpables:?} — usa `medico_datos::hoy()`, \
             que respeta la zona del prestador. Guardar un instante con \
             `Utc::now().to_rfc3339_opts(…)` sí es correcto y no lo caza esto."
        );
    }
}

#[cfg(test)]
mod pruebas_migracion {
    /// LAS MIGRACIONES NO SE LLEVAN POR DELANTE LAS FILAS HIJAS.
    ///
    /// Es la prueba de la reconstrucción de tabla de la 0045. Se siembra
    /// `factura` con sus `detalle_factura` y `glosa` ANTES de que la 0045 corra,
    /// y se comprueba que siguen ahí después.
    ///
    /// Con el código anterior —que confiaba en el `PRAGMA foreign_keys = OFF`
    /// escrito DENTRO de la migración, inerte porque sqlx la envuelve en una
    /// transacción— esta prueba se pone roja: el `DROP TABLE factura` dispara el
    /// `ON DELETE CASCADE` y las dos tablas quedan en cero, con la migración
    /// registrada como aplicada con éxito y sin un solo error.
    ///
    /// La siembra va con SQL crudo y no por la API a propósito: hay que escribir
    /// en el estado INTERMEDIO, entre la 0044 y la 0045, y eso solo se alcanza
    /// aplicando las migraciones a mano hasta ahí.
    #[tokio::test]
    async fn una_reconstruccion_de_tabla_no_borra_las_filas_hijas() {
        use sqlx::sqlite::SqliteConnectOptions;
        use sqlx::ConnectOptions;
        use std::str::FromStr;

        let dir = std::env::temp_dir().join(format!("medico-mig-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let ruta = dir.join("m.db");
        let url = format!("sqlite://{}", ruta.display());

        // 1. Todas las migraciones MENOS la última, con las foráneas ACTIVAS,
        //    que es como estaba una base antes de este corte.
        {
            let mut c = SqliteConnectOptions::from_str(&url)
                .unwrap()
                .create_if_missing(true)
                .foreign_keys(true)
                .connect()
                .await
                .unwrap();
            let m = sqlx::migrate!("./migrations");
            let hasta_0044: Vec<_> = m
                .iter()
                .filter(|mi| mi.version <= 44)
                .cloned()
                .collect();
            for mi in &hasta_0044 {
                sqlx::query(&mi.sql).execute(&mut c).await.unwrap();
            }
            // El registro de sqlx, para que `abrir` continúe desde aquí.
            sqlx::query(
                "CREATE TABLE IF NOT EXISTS _sqlx_migrations (
                   version BIGINT PRIMARY KEY, description TEXT NOT NULL,
                   installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                   success BOOLEAN NOT NULL, checksum BLOB NOT NULL,
                   execution_time BIGINT NOT NULL)",
            )
            .execute(&mut c)
            .await
            .unwrap();
            for mi in &hasta_0044 {
                sqlx::query(
                    "INSERT INTO _sqlx_migrations
                     (version, description, success, checksum, execution_time)
                     VALUES (?,?,1,?,0)",
                )
                .bind(mi.version)
                .bind(mi.description.as_ref())
                .bind(mi.checksum.as_ref())
                .execute(&mut c)
                .await
                .unwrap();
            }

            // 2. Datos: una factura con su línea y su glosa.
            sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900')")
                .execute(&mut c)
                .await
                .unwrap();
            sqlx::query("INSERT INTO entidad (id,nit,nombre,codigo_eps) VALUES (1,'800','EPS','E1')")
                .execute(&mut c)
                .await
                .unwrap();
            sqlx::query(
                "INSERT INTO factura (id,institucion_id,numero_factura,entidad_id,fecha_factura,
                                      periodo_inicio,periodo_fin)
                 VALUES (1,1,'F-1',1,'2026-01-31','2026-01-01','2026-01-31')",
            )
            .execute(&mut c)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO detalle_factura (factura_id,servicio) VALUES (1,'Consulta'),(1,'Otra')",
            )
            .execute(&mut c)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO glosa (factura_id,item_glosado,motivo,valor_glosado,fecha_glosa)
                 VALUES (1,'x','y',10,'2026-02-01')",
            )
            .execute(&mut c)
            .await
            .unwrap();
        }

        // 3. `abrir` aplica la 0045, que reconstruye `factura`.
        let pool = crate::abrir(&url).await.expect("la base tiene que abrir");

        let detalles: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM detalle_factura")
            .fetch_one(&pool)
            .await
            .unwrap();
        let glosas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM glosa")
            .fetch_one(&pool)
            .await
            .unwrap();
        let facturas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM factura")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(facturas, 1, "la factura se perdió");
        assert_eq!(
            detalles, 2,
            "SE PERDIERON LAS LÍNEAS: el DROP TABLE de la reconstrucción disparó el \
             ON DELETE CASCADE. El `PRAGMA foreign_keys = OFF` escrito DENTRO de la \
             migración es inerte — sqlx la envuelve en una transacción."
        );
        assert_eq!(glosas, 1, "se perdieron las glosas, por lo mismo");

        // Y las foráneas quedan ACTIVAS en el pool que se devuelve: apagarlas es
        // solo para migrar. Sin esto, el arreglo habría cambiado una pérdida de
        // datos por una base sin integridad referencial.
        let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(fk, 1, "el pool devuelto tiene que traer las foráneas activas");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
