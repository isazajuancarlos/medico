//! Acceso a la base. Nadie de aquí para arriba escribe SQL.

pub mod acceso;
pub mod agenda;
pub mod ambulancia;
pub mod capacidad;
pub mod catalogo;
pub mod cita;
pub mod clinico;
pub mod diagnostico;
pub mod divipola;
pub mod documento;
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

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opciones)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
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
