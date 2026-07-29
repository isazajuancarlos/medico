//! Pacientes. Toda consulta lleva el ámbito, y el ámbito no se puede omitir.

use chrono::Datelike;
use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Paciente {
    pub id: i64,
    pub tipo_documento: String,
    pub numero_documento: String,
    pub nombre1: String,
    pub nombre2: Option<String>,
    pub apellido1: String,
    pub apellido2: Option<String>,
    pub fecha_nacimiento: String,
    pub sexo: String,
    pub cod_municipio: Option<String>,
    pub celular: Option<String>,
    pub email: Option<String>,
    pub direccion: Option<String>,
    pub tipo_usuario: Option<String>,
    pub cod_pais: Option<String>,
    pub zona: Option<String>,
    pub entidad_id: Option<i64>,
    // Antecedentes: objetos JSON de estructura abierta. `Json<Value>` los sirve
    // como objetos reales (no como cadenas escapadas) y los lee del TEXT tal cual.
    pub ant_familiares: Json<serde_json::Value>,
    pub ant_personales: Json<serde_json::Value>,
    pub ant_ginecologicos: Json<serde_json::Value>,
    pub ant_obstetricos: Json<serde_json::Value>,
    pub institucion_id: i64,
}

const COLUMNAS: &str = "id, tipo_documento, numero_documento, nombre1, nombre2, \
                        apellido1, apellido2, fecha_nacimiento, sexo, cod_municipio, \
                        celular, email, direccion, tipo_usuario, cod_pais, zona, entidad_id, \
                        ant_familiares, ant_personales, ant_ginecologicos, ant_obstetricos, \
                        institucion_id";

/// El `Ambito` es un parámetro OBLIGATORIO, no opcional.
///
/// En Python el equivalente era `institucion_id: Optional[int] = None`, que al
/// omitirse dejaba de filtrar. Eso ya causó una fuga real: `rips_completo.py`
/// filtraba solo si se lo pedían y `routers/rips.py` no se lo pedía, y nadie lo
/// notó. Aquí omitirlo no compila — que es la única versión de esa regla que no
/// depende de que alguien se acuerde.
pub async fn listar(pool: &crate::Pool, ambito: Ambito) -> Result<Vec<Paciente>, sqlx::Error> {
    match ambito {
        Ambito::Superadmin => {
            sqlx::query_as::<_, Paciente>(&format!("SELECT {COLUMNAS} FROM paciente ORDER BY id"))
                .fetch_all(pool)
                .await
        }
        Ambito::Institucion(id) => {
            sqlx::query_as::<_, Paciente>(&format!(
                "SELECT {COLUMNAS} FROM paciente WHERE institucion_id = ? ORDER BY id"
            ))
            .bind(id)
            .fetch_all(pool)
            .await
        }
        // No es una consulta que devuelve vacío: es no preguntar. Un usuario sin
        // institución y sin ser superadmin no tiene ningún conjunto legítimo.
        Ambito::Ninguno => Ok(Vec::new()),
    }
}

/// `None` significa «no existe **o** no es suyo». No se distingue a propósito:
/// ver `medico_dominio::Visibilidad`.
pub async fn obtener(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<Paciente>, sqlx::Error> {
    let fila = sqlx::query_as::<_, Paciente>(&format!(
        "SELECT {COLUMNAS} FROM paciente WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(fila.filter(|p| ambito.puede_ver(Some(p.institucion_id)) == Visibilidad::Si))
}

/// Los datos de un paciente sin su id ni su institución: la institución no la
/// pone el cliente, la pone el ámbito de quien lo crea.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PacienteNuevo {
    pub tipo_documento: String,
    pub numero_documento: String,
    pub nombre1: String,
    pub nombre2: Option<String>,
    pub apellido1: String,
    pub apellido2: Option<String>,
    pub fecha_nacimiento: String,
    pub sexo: String,
    pub cod_municipio: Option<String>,
    pub celular: Option<String>,
    pub email: Option<String>,
    pub direccion: Option<String>,
    pub tipo_usuario: Option<String>,
    pub cod_pais: Option<String>,
    pub zona: Option<String>,
    pub entidad_id: Option<i64>,
}

/// Crea un paciente EN la institución de quien lo crea. La institución sale del
/// ámbito, **nunca del cliente**. `None` si el ámbito no puede crear: crear un
/// paciente es un acto de una IPS concreta, así que ni el superadmin (sin una
/// institución objetivo) ni «ninguno» lo hacen por esta vía.
pub async fn crear(
    pool: &crate::Pool,
    ambito: Ambito,
    nuevo: &PacienteNuevo,
) -> Result<Option<i64>, sqlx::Error> {
    let institucion_id = match ambito {
        Ambito::Institucion(id) => id,
        Ambito::Superadmin | Ambito::Ninguno => return Ok(None),
    };
    let r = sqlx::query(
        "INSERT INTO paciente (tipo_documento, numero_documento, nombre1, nombre2, apellido1, \
         apellido2, fecha_nacimiento, sexo, cod_municipio, celular, email, direccion, \
         tipo_usuario, cod_pais, zona, entidad_id, institucion_id) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(&nuevo.tipo_documento)
    .bind(&nuevo.numero_documento)
    .bind(&nuevo.nombre1)
    .bind(&nuevo.nombre2)
    .bind(&nuevo.apellido1)
    .bind(&nuevo.apellido2)
    .bind(&nuevo.fecha_nacimiento)
    .bind(&nuevo.sexo)
    .bind(&nuevo.cod_municipio)
    .bind(&nuevo.celular)
    .bind(&nuevo.email)
    .bind(&nuevo.direccion)
    .bind(&nuevo.tipo_usuario)
    .bind(&nuevo.cod_pais)
    .bind(&nuevo.zona)
    .bind(nuevo.entidad_id)
    .bind(institucion_id)
    .execute(pool)
    .await?;
    Ok(Some(r.last_insert_rowid()))
}

/// Actualiza un paciente **si es visible para el ámbito**. `false` si no existe
/// o no es suyo —indistinguibles, como en `obtener`—. No cambia la institución:
/// un paciente no se traslada de IPS editándolo. El ámbito va en el `WHERE`, así
/// que un id ajeno sencillamente no coincide y no se toca ni una fila.
pub async fn actualizar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    datos: &PacienteNuevo,
) -> Result<bool, sqlx::Error> {
    let condicion_institucion = match ambito {
        Ambito::Superadmin => None,
        Ambito::Institucion(i) => Some(i),
        Ambito::Ninguno => return Ok(false),
    };
    let filtro = if condicion_institucion.is_some() {
        "id = ? AND institucion_id = ?"
    } else {
        "id = ?"
    };
    let sql = format!(
        "UPDATE paciente SET tipo_documento=?, numero_documento=?, nombre1=?, nombre2=?, \
         apellido1=?, apellido2=?, fecha_nacimiento=?, sexo=?, cod_municipio=?, celular=?, \
         email=?, direccion=?, tipo_usuario=?, cod_pais=?, zona=?, entidad_id=? WHERE {filtro}"
    );
    let mut q = sqlx::query(&sql)
        .bind(&datos.tipo_documento)
        .bind(&datos.numero_documento)
        .bind(&datos.nombre1)
        .bind(&datos.nombre2)
        .bind(&datos.apellido1)
        .bind(&datos.apellido2)
        .bind(&datos.fecha_nacimiento)
        .bind(&datos.sexo)
        .bind(&datos.cod_municipio)
        .bind(&datos.celular)
        .bind(&datos.email)
        .bind(&datos.direccion)
        .bind(&datos.tipo_usuario)
        .bind(&datos.cod_pais)
        .bind(&datos.zona)
        .bind(datos.entidad_id)
        .bind(id);
    if let Some(i) = condicion_institucion {
        q = q.bind(i);
    }
    Ok(q.execute(pool).await?.rows_affected() > 0)
}

/// La vista del paciente que necesita el encabezado del formulario de historia:
/// el nombre ya compuesto, la edad ya calculada y los antecedentes. No es la fila
/// cruda —son datos derivados listos para pintar—.
#[derive(Debug, Clone, Serialize)]
pub struct InfoPaciente {
    pub id: i64,
    pub identificacion: String,
    pub tipo_id: String,
    pub nombre_completo: String,
    pub fecha_nacimiento: String,
    pub sexo: String,
    pub edad: i64,
    pub celular: Option<String>,
    // El nombre de la EPS necesita el maestro de entidades, que aún no existe
    // (lo carga #77). Hasta entonces es `None` —ausente, no inventado—.
    pub entidad_nombre: Option<String>,
    pub ant_familiares: serde_json::Value,
    pub ant_personales: serde_json::Value,
    pub ant_ginecologicos: serde_json::Value,
    pub ant_obstetricos: serde_json::Value,
}

/// Años cumplidos entre `nacimiento` (YYYY-MM-DD) y hoy. `0` si la fecha no se
/// puede leer: es un dato derivado para mostrar, no una regla de negocio, así que
/// una fecha corrupta no debe tumbar la carga del formulario.
fn edad_en_anios(nacimiento: &str) -> i64 {
    let Ok(nac) = chrono::NaiveDate::parse_from_str(nacimiento.trim(), "%Y-%m-%d") else {
        return 0;
    };
    let hoy = chrono::Local::now().date_naive();
    let mut anios = hoy.year() - nac.year();
    if (hoy.month(), hoy.day()) < (nac.month(), nac.day()) {
        anios -= 1;
    }
    anios.max(0) as i64
}

fn nombre_completo(p: &Paciente) -> String {
    [
        Some(p.nombre1.as_str()),
        p.nombre2.as_deref(),
        Some(p.apellido1.as_str()),
        p.apellido2.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

/// El contexto del paciente para el formulario, si es visible para el ámbito.
/// `None` si no existe o es ajeno —la misma indistinguibilidad que `obtener`—.
pub async fn info(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<Option<InfoPaciente>, sqlx::Error> {
    let Some(p) = obtener(pool, ambito, id).await? else {
        return Ok(None);
    };
    Ok(Some(InfoPaciente {
        id: p.id,
        identificacion: p.numero_documento.clone(),
        tipo_id: p.tipo_documento.clone(),
        nombre_completo: nombre_completo(&p),
        edad: edad_en_anios(&p.fecha_nacimiento),
        fecha_nacimiento: p.fecha_nacimiento.clone(),
        sexo: p.sexo.clone(),
        celular: p.celular.clone(),
        entidad_nombre: None,
        ant_familiares: p.ant_familiares.0.clone(),
        ant_personales: p.ant_personales.0.clone(),
        ant_ginecologicos: p.ant_ginecologicos.0.clone(),
        ant_obstetricos: p.ant_obstetricos.0.clone(),
    }))
}

/// Los cuatro bloques de antecedentes en una actualización. Cada uno es opcional:
/// se actualiza SOLO lo que llega, para que guardar los ginecológicos no borre
/// los familiares. Un `None` no toca esa columna; un `Some({})` la vacía a
/// propósito —son cosas distintas y se distinguen—.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Antecedentes {
    pub ant_familiares: Option<serde_json::Value>,
    pub ant_personales: Option<serde_json::Value>,
    pub ant_ginecologicos: Option<serde_json::Value>,
    pub ant_obstetricos: Option<serde_json::Value>,
}

/// Actualiza los antecedentes de un paciente **visible para el ámbito**. `false`
/// si no existe o no es suyo —indistinguibles, como en `actualizar`—. El ámbito
/// va en el `WHERE`: un id ajeno no coincide y no se toca ni una fila. Solo se
/// escriben los bloques presentes; si no viene ninguno, no hay nada que cambiar
/// y se responde `true` cuando el paciente es visible.
pub async fn actualizar_antecedentes(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    ant: &Antecedentes,
) -> Result<bool, sqlx::Error> {
    let condicion_institucion = match ambito {
        Ambito::Superadmin => None,
        Ambito::Institucion(i) => Some(i),
        Ambito::Ninguno => return Ok(false),
    };

    // Se arma la lista de columnas a partir de lo que llegó. Nada de columnas
    // dinámicas que vengan del cliente: los nombres son literales de aquí, solo
    // se decide cuáles entran.
    let mut sets: Vec<&str> = Vec::new();
    if ant.ant_familiares.is_some() {
        sets.push("ant_familiares = ?");
    }
    if ant.ant_personales.is_some() {
        sets.push("ant_personales = ?");
    }
    if ant.ant_ginecologicos.is_some() {
        sets.push("ant_ginecologicos = ?");
    }
    if ant.ant_obstetricos.is_some() {
        sets.push("ant_obstetricos = ?");
    }

    // Sin bloques que escribir: solo confirmar que el paciente es visible, para
    // no responder `true` sobre uno ajeno.
    if sets.is_empty() {
        return Ok(obtener(pool, ambito, id).await?.is_some());
    }

    let filtro = if condicion_institucion.is_some() {
        "id = ? AND institucion_id = ?"
    } else {
        "id = ?"
    };
    let sql = format!("UPDATE paciente SET {} WHERE {filtro}", sets.join(", "));
    let mut q = sqlx::query(&sql);
    // El orden de los `bind` sigue el de los `push` de arriba.
    if let Some(v) = &ant.ant_familiares {
        q = q.bind(Json(v));
    }
    if let Some(v) = &ant.ant_personales {
        q = q.bind(Json(v));
    }
    if let Some(v) = &ant.ant_ginecologicos {
        q = q.bind(Json(v));
    }
    if let Some(v) = &ant.ant_obstetricos {
        q = q.bind(Json(v));
    }
    q = q.bind(id);
    if let Some(i) = condicion_institucion {
        q = q.bind(i);
    }
    Ok(q.execute(pool).await?.rows_affected() > 0)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    /// Base en memoria con dos instituciones y un paciente en cada una.
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO institucion (id, nombre, nit) VALUES (1,'Uno','900'),(2,'Dos','901')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, apellido1,
                                   fecha_nacimiento, sexo, cod_municipio, institucion_id)
             VALUES (10,'CC','111','Ana','Ruiz','1990-01-01','F','05001',1),
                    (20,'CC','222','Beto','Paz','1985-05-05','M','08001',2)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn listar_solo_devuelve_lo_de_la_institucion() {
        let pool = base().await;
        let ps = listar(&pool, Ambito::institucion(1)).await.unwrap();
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].id, 10);
    }

    /// LA PRUEBA QUE JUSTIFICA ESTE DISEÑO.
    ///
    /// Pedir por identificador el paciente de otra institución tiene que ser
    /// indistinguible de pedir uno que no existe. En Python esto se escapó una
    /// vez —`db.get` no pasaba por el filtro— y bastaba incrementar un número
    /// en la URL para leer, y con `PUT` modificar, la historia de otra
    /// institución.
    #[tokio::test]
    async fn obtener_el_de_otra_institucion_es_igual_que_uno_inexistente() {
        let pool = base().await;
        let ajeno = obtener(&pool, Ambito::institucion(1), 20).await.unwrap();
        let inexistente = obtener(&pool, Ambito::institucion(1), 9999).await.unwrap();
        assert!(ajeno.is_none(), "vio un paciente de otra institución");
        assert!(inexistente.is_none());
    }

    #[tokio::test]
    async fn obtener_el_propio_funciona() {
        let pool = base().await;
        let p = obtener(&pool, Ambito::institucion(1), 10)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(p.nombre1, "Ana");
    }

    #[tokio::test]
    async fn el_superadmin_ve_las_dos_instituciones() {
        let pool = base().await;
        assert_eq!(listar(&pool, Ambito::Superadmin).await.unwrap().len(), 2);
        assert!(obtener(&pool, Ambito::Superadmin, 20).await.unwrap().is_some());
    }

    /// Un usuario sin institución no ve nada, y en particular NO lo ve todo.
    #[tokio::test]
    async fn sin_ambito_no_se_ve_nada() {
        let pool = base().await;
        assert!(listar(&pool, Ambito::Ninguno).await.unwrap().is_empty());
        assert!(obtener(&pool, Ambito::Ninguno, 10).await.unwrap().is_none());
    }

    /// El cero a la izquierda del código de municipio sobrevive al viaje.
    #[tokio::test]
    async fn el_codigo_de_municipio_conserva_el_cero() {
        let pool = base().await;
        let p = obtener(&pool, Ambito::institucion(1), 10)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(p.cod_municipio.as_deref(), Some("05001"));
    }

    /// El mismo documento en dos instituciones son dos filas legítimas.
    #[tokio::test]
    async fn el_mismo_documento_puede_estar_en_dos_instituciones() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, apellido1,
                                   fecha_nacimiento, sexo, institucion_id)
             VALUES (30,'CC','111','Ana','Ruiz','1990-01-01','F',2)",
        )
        .execute(&pool)
        .await
        .expect("el mismo documento en otra institución debe poder guardarse");

        // …y repetirlo DENTRO de la misma sí es un error.
        let repetido = sqlx::query(
            "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, apellido1,
                                   fecha_nacimiento, sexo, institucion_id)
             VALUES (31,'CC','111','Ana','Ruiz','1990-01-01','F',1)",
        )
        .execute(&pool)
        .await;
        assert!(repetido.is_err(), "se aceptó un documento duplicado en la misma institución");
    }

    /// Las claves foráneas están ACTIVAS. En SQLite vienen apagadas por defecto,
    /// y con ellas apagadas un paciente puede apuntar a una institución que no
    /// existe: la fila queda fuera del alcance de todo filtro por institución y
    /// no la ve nadie, ni siquiera para corregirla.
    #[tokio::test]
    async fn las_claves_foraneas_estan_activas() {
        let pool = base().await;
        let huerfano = sqlx::query(
            "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, apellido1,
                                   fecha_nacimiento, sexo, institucion_id)
             VALUES (40,'CC','999','Nadie','Nadie','1990-01-01','F',777)",
        )
        .execute(&pool)
        .await;
        assert!(huerfano.is_err(), "se aceptó un paciente de una institución inexistente");
    }

    fn nuevo(numero: &str, nombre: &str) -> PacienteNuevo {
        PacienteNuevo {
            tipo_documento: "CC".into(),
            numero_documento: numero.into(),
            nombre1: nombre.into(),
            nombre2: None,
            apellido1: "Prueba".into(),
            apellido2: None,
            fecha_nacimiento: "2000-02-02".into(),
            sexo: "F".into(),
            cod_municipio: Some("05001".into()),
            celular: Some("3001234567".into()),
            email: None,
            direccion: None,
            tipo_usuario: Some("04".into()),
            cod_pais: Some("170".into()),
            zona: Some("01".into()),
            entidad_id: None,
        }
    }

    #[tokio::test]
    async fn crear_lo_pone_en_la_institucion_del_ambito() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), &nuevo("333", "Caro")).await.unwrap().unwrap();
        let p = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(p.institucion_id, 1);
        // …y NO es visible desde la otra institución.
        assert!(obtener(&pool, Ambito::institucion(2), id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn ni_el_superadmin_ni_ninguno_crean_por_esta_via() {
        let pool = base().await;
        assert!(crear(&pool, Ambito::Superadmin, &nuevo("444", "X")).await.unwrap().is_none());
        assert!(crear(&pool, Ambito::Ninguno, &nuevo("445", "Y")).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn actualizar_el_propio_cambia_los_datos() {
        let pool = base().await;
        assert!(actualizar(&pool, Ambito::institucion(1), 10, &nuevo("111", "Ana Maria")).await.unwrap());
        let p = obtener(&pool, Ambito::institucion(1), 10).await.unwrap().unwrap();
        assert_eq!(p.nombre1, "Ana Maria");
    }

    /// La prueba de aislamiento en la escritura: la institución 1 NO puede editar
    /// al paciente 20, que es de la 2. No se toca ni una fila, y el dato sigue.
    #[tokio::test]
    async fn actualizar_el_ajeno_no_toca_nada() {
        let pool = base().await;
        assert!(!actualizar(&pool, Ambito::institucion(1), 20, &nuevo("222", "HACKEADO")).await.unwrap());
        let p = obtener(&pool, Ambito::Superadmin, 20).await.unwrap().unwrap();
        assert_eq!(p.nombre1, "Beto", "un ajeno editó a un paciente de otra institución");
    }

    #[tokio::test]
    async fn sin_ambito_no_se_actualiza() {
        let pool = base().await;
        assert!(!actualizar(&pool, Ambito::Ninguno, 10, &nuevo("111", "Z")).await.unwrap());
    }

    // ── Antecedentes ─────────────────────────────────────────────────

    #[tokio::test]
    async fn un_paciente_nuevo_trae_antecedentes_vacios_no_nulos() {
        let pool = base().await;
        let p = obtener(&pool, Ambito::institucion(1), 10).await.unwrap().unwrap();
        assert_eq!(p.ant_familiares.0, json!({}));
        assert_eq!(p.ant_personales.0, json!({}));
    }

    #[tokio::test]
    async fn actualizar_un_bloque_no_borra_los_otros() {
        let pool = base().await;
        // Primero, los personales.
        let solo_personales = Antecedentes {
            ant_personales: Some(json!({"hta": true, "diabetes": false})),
            ..Default::default()
        };
        assert!(actualizar_antecedentes(&pool, Ambito::institucion(1), 10, &solo_personales).await.unwrap());
        // Luego, los familiares: los personales deben seguir ahí.
        let solo_familiares = Antecedentes {
            ant_familiares: Some(json!({"cancer": "madre"})),
            ..Default::default()
        };
        assert!(actualizar_antecedentes(&pool, Ambito::institucion(1), 10, &solo_familiares).await.unwrap());

        let p = obtener(&pool, Ambito::institucion(1), 10).await.unwrap().unwrap();
        assert_eq!(p.ant_personales.0, json!({"hta": true, "diabetes": false}));
        assert_eq!(p.ant_familiares.0, json!({"cancer": "madre"}));
    }

    /// Aislamiento en la escritura de antecedentes: la institución 1 no toca al
    /// paciente 20, que es de la 2.
    #[tokio::test]
    async fn actualizar_antecedentes_ajenos_no_toca_nada() {
        let pool = base().await;
        let ant = Antecedentes {
            ant_personales: Some(json!({"inyectado": true})),
            ..Default::default()
        };
        assert!(!actualizar_antecedentes(&pool, Ambito::institucion(1), 20, &ant).await.unwrap());
        let p = obtener(&pool, Ambito::Superadmin, 20).await.unwrap().unwrap();
        assert_eq!(p.ant_personales.0, json!({}), "un ajeno escribió antecedentes de otra institución");
    }

    /// Una actualización sin bloques no falla, pero tampoco confirma sobre un
    /// paciente ajeno: distingue el visible del que no lo es.
    #[tokio::test]
    async fn actualizacion_vacia_distingue_visible_de_ajeno() {
        let pool = base().await;
        let vacio = Antecedentes::default();
        assert!(actualizar_antecedentes(&pool, Ambito::institucion(1), 10, &vacio).await.unwrap());
        assert!(!actualizar_antecedentes(&pool, Ambito::institucion(1), 20, &vacio).await.unwrap());
    }
}
