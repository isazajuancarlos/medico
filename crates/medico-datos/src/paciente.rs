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
    /// Ciudad y departamento en TEXTO LIBRE del sistema viejo (#16/#65). Son la
    /// FUENTE de la derivación del `cod_municipio` (`derivar_municipio`), no un
    /// dato que se muestre. Vacíos para todo paciente capturado con el código
    /// directo.
    pub ciudad_residencia: Option<String>,
    pub departamento_residencia: Option<String>,
    /// La procedencia de los datos del paciente, JSON `{campo: procedencia}`
    /// (migración 0069). Lo escribe la captura (todo lo tecleado va `confirmado`)
    /// y la derivación (el municipio derivado va `derivado`). Nunca lo pone el
    /// cliente — por eso no está en [`PacienteNuevo`].
    pub procedencias: String,
    /// Demografía del perfil PatientRDA (#603), capturada por la API (migración
    /// 0070). `nacionalidad` es un código ISO 3166-1 numérico como `cod_pais`;
    /// `pertenencia_etnica`, `discapacidad` e `identidad_genero` son códigos de
    /// catálogo (servidos por `ihce::terminologia`); `comunidad_etnica` es texto
    /// libre (el pueblo concreto). `None` hasta que se capturan; el RDA los omite.
    pub nacionalidad: Option<String>,
    pub pertenencia_etnica: Option<String>,
    pub comunidad_etnica: Option<String>,
    pub discapacidad: Option<String>,
    pub identidad_genero: Option<String>,
    // Antecedentes: objetos JSON de estructura abierta. `Json<Value>` los sirve
    // como objetos reales (no como cadenas escapadas) y los lee del TEXT tal cual.
    pub ant_familiares: Json<serde_json::Value>,
    pub ant_personales: Json<serde_json::Value>,
    pub ant_ginecologicos: Json<serde_json::Value>,
    pub ant_perinatales: Json<serde_json::Value>,
    pub institucion_id: i64,
}

const COLUMNAS: &str = "id, tipo_documento, numero_documento, nombre1, nombre2, \
                        apellido1, apellido2, fecha_nacimiento, sexo, cod_municipio, \
                        celular, email, direccion, tipo_usuario, cod_pais, zona, entidad_id, \
                        ciudad_residencia, departamento_residencia, procedencias, \
                        nacionalidad, pertenencia_etnica, comunidad_etnica, discapacidad, \
                        identidad_genero, \
                        ant_familiares, ant_personales, ant_ginecologicos, ant_perinatales, \
                        institucion_id";

/// Qué se busca y cuántos se traen. Los dos campos son `Option` y `None`
/// significa **sin restricción**, no «vacío»: `Criterio::default()` es el padrón
/// entero, que es lo que había antes de que esto existiera.
#[derive(Debug, Clone, Default)]
pub struct Criterio<'a> {
    /// Texto libre. Casa contra los CUATRO nombres y el documento, plegando
    /// mayúsculas y tildes (ver [`plegar`]).
    pub buscar: Option<&'a str>,
    /// Techo de filas devueltas. El total que CASAN se devuelve aparte.
    pub limite: Option<i64>,
}

/// Lo que casó y cuántos casaron, que **son dos cosas distintas** (directiva 32).
///
/// `filas` es lo que cabe bajo el `limite`; `total` es cuántos hay de verdad. Sin
/// el segundo, una pantalla que enseña veinte de ochenta y cuatro se lee como si
/// hubiera ochenta y cuatro… veinte, y quien busca a alguien que no sale no sabe
/// si es que no está o es que no cabía. Truncar en silencio es peor que no
/// truncar (directiva 22).
#[derive(Debug, Clone)]
pub struct Hallazgo {
    pub filas: Vec<Paciente>,
    pub total: i64,
}

/// Deja el texto en minúsculas y sin tildes, para comparar.
///
/// **Su gemela vive en SQL**, en el trigger de la migración 0051 que mantiene la
/// columna `paciente.plegado`, y las dos tienen que decir lo mismo: si divergen,
/// la pantalla encuentra lo que la consulta no, sin dar error. Lo fija
/// `la_columna_plegado_dice_lo_mismo_que_plegar`, que lee la columna que el
/// trigger escribe y la compara con esta función.
///
/// Se pliegan las tildes y no solo la caja porque en Colombia los apellidos las
/// llevan y **la gente teclea sin ellas**: sin esto, «paez» no encuentra a
/// «Páez» — y quien busca un paciente que existe concluye que no está.
pub fn plegar(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'Á' | 'à' | 'À' | 'ä' | 'Ä' | 'â' | 'Â' => 'a',
            'é' | 'É' | 'è' | 'È' | 'ë' | 'Ë' | 'ê' | 'Ê' => 'e',
            'í' | 'Í' | 'ì' | 'Ì' | 'ï' | 'Ï' | 'î' | 'Î' => 'i',
            'ó' | 'Ó' | 'ò' | 'Ò' | 'ö' | 'Ö' | 'ô' | 'Ô' => 'o',
            'ú' | 'Ú' | 'ù' | 'Ù' | 'ü' | 'Ü' | 'û' | 'Û' => 'u',
            'ñ' | 'Ñ' => 'n',
            // El resto SOLO se baja de caja si es ASCII, que es lo que hace
            // `LOWER` de SQLite. Bajar el Unicode entero (lo que hacía
            // `to_lowercase`) divergía de la columna en las letras fuera del mapa
            // —«Ý» quedaba «ý» en Rust y «Ý» en SQL—, así que la aguja no casaba
            // lo que la columna guardaba, sin que nada avisara. Lo cazó
            // `la_columna_plegado_dice_lo_mismo_que_plegar` al probar «Ýpsilon».
            otro => otro.to_ascii_lowercase(),
        })
        .collect()
}

// La GEMELA SQL de `plegar` ya no vive aquí: desde la migración 0051 el texto
// plegado se materializa en la columna `paciente.plegado`, que un trigger
// mantiene con esa misma regla. La búsqueda compara contra la columna en vez de
// plegar cada fila (#596). El nombre entero + el documento (que antes se armaba
// aquí como `NOMBRE_ENTERO` para buscar «ana ruiz» cruzando columnas) es lo que
// el trigger pliega y guarda.

/// El `Ambito` es un parámetro OBLIGATORIO, no opcional.
///
/// En Python el equivalente era `institucion_id: Optional[int] = None`, que al
/// omitirse dejaba de filtrar. Eso ya causó una fuga real: `rips_completo.py`
/// filtraba solo si se lo pedían y `routers/rips.py` no se lo pedía, y nadie lo
/// notó. Aquí omitirlo no compila — que es la única versión de esa regla que no
/// depende de que alguien se acuerde.
///
/// El padrón ENTERO, sin criterio. Sigue existiendo porque hay llamantes que lo
/// quieren de verdad —la ruta JSON `GET /api/pacientes`—, pero **una pantalla que
/// pinta un desplegable no debe usarla**: ver [`buscar`].
pub async fn listar(pool: &crate::Pool, ambito: Ambito) -> Result<Vec<Paciente>, sqlx::Error> {
    Ok(buscar(pool, ambito, &Criterio::default()).await?.filas)
}

/// Los pacientes del ámbito que casan con el criterio, y **cuántos casan**.
///
/// Nació el 2026-08-08 porque `listar` no aceptaba criterio y eso dejaba dos
/// defectos hermanos: `/panel/pacientes` filtraba EN MEMORIA sobre el padrón
/// entero, y cuatro pantallas pintaban un `<select>` con todas las fichas de la
/// institución. En un consultorio da igual; en un hospital con decenas de miles
/// es una página que no se abre — y la versión de Angular era peor, porque pedía
/// las primeras 500 y a partir de ahí un paciente **no se podía elegir y nada lo
/// decía**.
pub async fn buscar(
    pool: &crate::Pool,
    ambito: Ambito,
    c: &Criterio<'_>,
) -> Result<Hallazgo, sqlx::Error> {
    let inst = match ambito {
        Ambito::Superadmin => None,
        Ambito::Institucion(id) => Some(id),
        // No es una consulta que devuelve vacío: es no preguntar. Un usuario sin
        // institución y sin ser superadmin no tiene ningún conjunto legítimo.
        Ambito::Ninguno => return Ok(Hallazgo { filas: Vec::new(), total: 0 }),
    };

    let mut donde = String::from("WHERE 1 = 1");
    if inst.is_some() {
        donde.push_str(" AND institucion_id = ?");
    }
    // La aguja se pliega en Rust y la columna en SQL: las dos por la misma regla.
    //
    // Y se parte en PALABRAS, que se exigen todas. Con una sola aguja, «maría
    // páez» no encuentra a «María Fernanda Páez Gómez» —el segundo nombre queda
    // en medio— y así es justo como se busca a una persona: por nombre y
    // apellido. Cada palabra puede casar contra el nombre entero o contra el
    // documento; las palabras entre sí van con AND, que es lo que hace que
    // añadir una palabra ACOTE en vez de ampliar.
    let agujas: Vec<String> = c
        .buscar
        .map(|s| plegar(s).split_whitespace().map(|t| format!("%{t}%")).collect())
        .unwrap_or_default();
    // Contra la columna MATERIALIZADA `plegado` (nombre entero + documento, ya
    // plegado por el trigger de la 0051), no plegando cada fila en la consulta:
    // esto es lo que baja los ~145 ms a ~5,6 (#596). Como `plegado` junta nombre
    // y documento, una sola comparación por palabra cubre las dos.
    for _ in &agujas {
        donde.push_str(" AND plegado LIKE ?");
    }

    let mut sql = format!("SELECT {COLUMNAS} FROM paciente {donde} ORDER BY id");
    if c.limite.is_some() {
        sql.push_str(" LIMIT ?");
    }
    let mut q = sqlx::query_as::<_, Paciente>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    for a in &agujas {
        q = q.bind(a);
    }
    if let Some(l) = c.limite {
        q = q.bind(l);
    }
    let filas = q.fetch_all(pool).await?;

    // El TOTAL es el número que deja decir «hay 84, se enseñan 20» en vez de
    // truncar en silencio. **Y solo se pide cuando puede haber más**: si se
    // pidieron veinte y vinieron menos, ésas SON todas — no es una aproximación,
    // es aritmética. El `COUNT` recorre la tabla igual que la consulta, así que
    // ahorrarlo parte por la mitad el tiempo del caso normal, que es el de quien
    // ya escribió lo suficiente para acotar.
    let total = match c.limite {
        Some(l) if (filas.len() as i64) < l => filas.len() as i64,
        _ => {
            let cuenta = format!("SELECT COUNT(*) FROM paciente {donde}");
            let mut q = sqlx::query_scalar::<_, i64>(&cuenta);
            if let Some(i) = inst {
                q = q.bind(i);
            }
            for a in &agujas {
                q = q.bind(a);
            }
            q.fetch_one(pool).await?
        }
    };
    Ok(Hallazgo { filas, total })
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
    /// Ciudad y departamento en texto libre (#16/#65). Opcionales y NO se
    /// muestran: son la fuente para derivar el `cod_municipio` cuando no llega el
    /// código. `procedencias` no está aquí a propósito — lo pone el servidor,
    /// nunca el cliente.
    #[serde(default)]
    pub ciudad_residencia: Option<String>,
    #[serde(default)]
    pub departamento_residencia: Option<String>,
    /// Demografía del PatientRDA (#603, migración 0070). Opcionales: hoy nadie
    /// las captura, así que un cliente que no las mande deja los campos en `None`
    /// y el RDA los omite. `procedencias` sigue sin estar aquí — lo pone el
    /// servidor.
    #[serde(default)]
    pub nacionalidad: Option<String>,
    #[serde(default)]
    pub pertenencia_etnica: Option<String>,
    #[serde(default)]
    pub comunidad_etnica: Option<String>,
    #[serde(default)]
    pub discapacidad: Option<String>,
    #[serde(default)]
    pub identidad_genero: Option<String>,
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
         tipo_usuario, cod_pais, zona, entidad_id, ciudad_residencia, departamento_residencia, \
         nacionalidad, pertenencia_etnica, comunidad_etnica, discapacidad, identidad_genero, \
         procedencias, institucion_id) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
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
    .bind(&nuevo.ciudad_residencia)
    .bind(&nuevo.departamento_residencia)
    .bind(&nuevo.nacionalidad)
    .bind(&nuevo.pertenencia_etnica)
    .bind(&nuevo.comunidad_etnica)
    .bind(&nuevo.discapacidad)
    .bind(&nuevo.identidad_genero)
    // Lo que el humano tecleó en el formulario va `confirmado`: lo afirmó una
    // persona. El municipio derivado lo pisa después `derivar_municipio`.
    .bind(serde_json::Value::Object(procedencias_de_captura(nuevo)).to_string())
    .bind(institucion_id)
    .execute(pool)
    .await?;
    let id = r.last_insert_rowid();
    // Si no vino el código pero sí el texto de la ciudad, se deriva y se marca
    // `derivado` (no pisa un código confirmado: si vino, esto no hace nada).
    derivar_municipio(pool, ambito, id).await?;
    Ok(Some(id))
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

    // No PROMOVER en silencio un municipio DERIVADO a `confirmado` por el mero
    // reenvío del formulario (observación de la revisión de seguridad del
    // 2026-08-21): `procedencias_de_captura` marca `confirmado` todo campo
    // presente, y la pantalla de edición pre-rellena el municipio derivado, así
    // que guardar tras tocar otra cosa lo afirmaría sin que nadie lo mirara —y
    // desactivaría el «duro sin aval» que esta pieza existe para disparar—. Regla:
    // si el código NO cambió y ya estaba `derivado`, sigue `derivado`; solo un
    // cambio de valor cuenta como que un humano lo afirmó.
    let anterior = obtener(pool, ambito, id).await?;
    let mut proc = procedencias_de_captura(datos);
    if let Some(a) = &anterior {
        use medico_dominio::procedencia::{Procedencia, Respaldos};
        let sin_cambio = datos.cod_municipio.is_some() && datos.cod_municipio == a.cod_municipio;
        // El JSON ilegible NO se traga a un `None` en silencio: eso haría
        // `era_derivado=false` y PROMOVERÍA a `confirmado` un campo que el cierre
        // sacaría `SinRespaldo` —el falso negativo que la directiva 23 persigue—.
        // Se propaga como error, igual que `derivar_municipio` y `en_registro`
        // (los tres tratan el corrupto en la dirección segura). El BLANCO del
        // padrón legado no es corrupto: `desde_json` lo da como mapa vacío.
        let era_derivado = Respaldos::desde_json(Some(&a.procedencias))
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))?
            .de("cod_municipio")
            == Some(Procedencia::Derivado);
        if sin_cambio && era_derivado {
            proc.insert(
                "cod_municipio".to_string(),
                serde_json::Value::from(Procedencia::Derivado.como_str()),
            );
        }
    }

    let sql = format!(
        "UPDATE paciente SET tipo_documento=?, numero_documento=?, nombre1=?, nombre2=?, \
         apellido1=?, apellido2=?, fecha_nacimiento=?, sexo=?, cod_municipio=?, celular=?, \
         email=?, direccion=?, tipo_usuario=?, cod_pais=?, zona=?, entidad_id=?, \
         ciudad_residencia=?, departamento_residencia=?, nacionalidad=?, pertenencia_etnica=?, \
         comunidad_etnica=?, discapacidad=?, identidad_genero=?, procedencias=? WHERE {filtro}"
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
        .bind(&datos.ciudad_residencia)
        .bind(&datos.departamento_residencia)
        .bind(&datos.nacionalidad)
        .bind(&datos.pertenencia_etnica)
        .bind(&datos.comunidad_etnica)
        .bind(&datos.discapacidad)
        .bind(&datos.identidad_genero)
        // Lo tecleado va `confirmado`, salvo el municipio derivado que no cambió
        // (ajustado arriba en `proc` para que siga `derivado`).
        .bind(serde_json::Value::Object(proc).to_string())
        .bind(id);
    if let Some(i) = condicion_institucion {
        q = q.bind(i);
    }
    let toco = q.execute(pool).await?.rows_affected() > 0;
    // Solo si se tocó una fila (id visible para el ámbito): si el código no vino
    // pero sí el texto de la ciudad, se deriva y se marca `derivado`.
    if toco {
        derivar_municipio(pool, ambito, id).await?;
    }
    Ok(toco)
}

/// La procedencia con la que nace un paciente RECIÉN CAPTURADO, JSON
/// `{campo: procedencia}` (#84).
///
/// La demografía que trae valor va **`confirmado`**: la tecleó una persona en el
/// formulario, no la pre-rellenó el sistema, así que hay aval humano. Es la
/// diferencia con la fricción de la historia, donde un campo DURO sin toque
/// explícito se omite: aquí la captura del paciente **es** el toque. El municipio
/// DERIVADO lo pisa después [`derivar_municipio`] con `derivado`, que es el único
/// dato que no afirmó una persona.
///
/// Son de dos grupos: los que el bloque de identificación del RIPS AUDITA
/// (`tipo_usuario`, `cod_pais`, `cod_municipio`, `zona`) y los cuatro
/// demográficos del RDA (#603, migración 0070: `nacionalidad`,
/// `pertenencia_etnica`, `discapacidad`, `identidad_genero`). Estos últimos hoy
/// no los audita el RIPS —no están en su bloque US—, pero su procedencia se
/// registra igual: es el mismo «saber de dónde vino cada cosa», y deja lista la
/// evidencia el día que el RDA o una extensión del RIPS los mire. `comunidad_etnica`
/// es texto libre y va con los demás (una persona lo escribió).
fn procedencias_de_captura(nuevo: &PacienteNuevo) -> serde_json::Map<String, serde_json::Value> {
    use medico_dominio::procedencia::Procedencia;
    let confirmado = serde_json::Value::from(Procedencia::Confirmado.como_str());
    let mut mapa = serde_json::Map::new();
    let presentes: [(&str, &Option<String>); 9] = [
        ("tipo_usuario", &nuevo.tipo_usuario),
        ("cod_pais", &nuevo.cod_pais),
        ("cod_municipio", &nuevo.cod_municipio),
        ("zona", &nuevo.zona),
        ("nacionalidad", &nuevo.nacionalidad),
        ("pertenencia_etnica", &nuevo.pertenencia_etnica),
        ("comunidad_etnica", &nuevo.comunidad_etnica),
        ("discapacidad", &nuevo.discapacidad),
        ("identidad_genero", &nuevo.identidad_genero),
    ];
    for (campo, valor) in presentes {
        if valor.as_deref().map(str::trim).is_some_and(|s| !s.is_empty()) {
            mapa.insert(campo.to_string(), confirmado.clone());
        }
    }
    mapa
}

/// Deriva el `cod_municipio` desde el texto de la ciudad, si se puede, y lo marca
/// `derivado` en la procedencia del paciente. Conservador y sin aproximar:
///   - si el paciente **ya tiene** `cod_municipio`, no hace nada — no se pisa lo
///     que un humano confirmó;
///   - sin texto de ciudad, no hay de dónde derivar;
///   - si el nombre es ambiguo (varios «San Juan»),
///     [`derivaciones::municipio_desde_texto`] devuelve `None` y aquí tampoco se
///     toca nada.
///
/// Devuelve el código derivado, o `None` si no derivó. Idempotente: una segunda
/// llamada ve el `cod_municipio` ya puesto y no hace nada.
///
/// **Lleva el `Ambito` como toda consulta de paciente** (regla del módulo): el id
/// va con el filtro de institución en el SELECT y en el UPDATE, así que un id
/// ajeno no se lee ni se escribe aunque el llamante se equivoque. Hoy sus dos
/// llamantes (`crear`, `actualizar`) ya lo acotan; el ámbito aquí es la
/// barandilla que impide que un tercer llamante lo convierta en una escritura
/// cruzada de institución (observación de la revisión de seguridad del 2026-08-21).
pub async fn derivar_municipio(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<String>, sqlx::Error> {
    let inst = match ambito {
        Ambito::Superadmin => None,
        Ambito::Institucion(i) => Some(i),
        // Sin institución y sin ser superadmin no hay conjunto legítimo: no deriva.
        Ambito::Ninguno => return Ok(None),
    };
    let filtro = if inst.is_some() { "id = ? AND institucion_id = ?" } else { "id = ?" };

    #[derive(sqlx::FromRow)]
    struct Residencia {
        cod_municipio: Option<String>,
        ciudad_residencia: Option<String>,
        departamento_residencia: Option<String>,
        procedencias: String,
    }
    let sql_sel = format!(
        "SELECT cod_municipio, ciudad_residencia, departamento_residencia, procedencias \
         FROM paciente WHERE {filtro}"
    );
    let mut q = sqlx::query_as(&sql_sel).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    let fila: Option<Residencia> = q.fetch_optional(pool).await?;
    let Some(r) = fila else {
        return Ok(None);
    };
    if r.cod_municipio.is_some() {
        return Ok(None);
    }
    let Some(ciudad) = r.ciudad_residencia.as_deref().map(str::trim).filter(|s| !s.is_empty())
    else {
        return Ok(None);
    };
    let departamento = r.departamento_residencia;
    let procedencias = r.procedencias;

    let catalogo = crate::divipola::todos(pool).await?;
    let candidatos: Vec<medico_dominio::derivaciones::Candidato> = catalogo
        .into_iter()
        .map(|m| medico_dominio::derivaciones::Candidato {
            codigo: m.codigo,
            nombre: m.nombre,
            nombre_departamento: m.nombre_departamento,
        })
        .collect();
    let Some(codigo) = medico_dominio::derivaciones::municipio_desde_texto(
        Some(ciudad),
        departamento.as_deref(),
        &candidatos,
    ) else {
        return Ok(None);
    };

    // Marca el municipio `derivado` sin tocar el resto de la procedencia. El
    // BLANCO es el centinela «pre-captura» (el padrón legado nace así, migración
    // 0069): empieza de un mapa vacío, no revienta. Un JSON ILEGIBLE sí se
    // propaga como error de decodificación —no se traga a un mapa vacío en
    // silencio— para que el llamante lo vea (directivas 20 y 23: se enumera lo
    // que significa «empezar de cero», el resto va a error).
    let mut mapa: serde_json::Map<String, serde_json::Value> = match procedencias.trim() {
        "" => serde_json::Map::new(),
        s => serde_json::from_str(s).map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
    };
    mapa.insert(
        "cod_municipio".to_string(),
        serde_json::Value::from(medico_dominio::procedencia::Procedencia::Derivado.como_str()),
    );
    let procedencias = serde_json::Value::Object(mapa).to_string();
    let sql_upd =
        format!("UPDATE paciente SET cod_municipio = ?, procedencias = ? WHERE {filtro}");
    let mut u = sqlx::query(&sql_upd).bind(&codigo).bind(&procedencias).bind(id);
    if let Some(i) = inst {
        u = u.bind(i);
    }
    u.execute(pool).await?;
    Ok(Some(codigo))
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
    pub ant_perinatales: serde_json::Value,
}

/// Años cumplidos entre `nacimiento` (YYYY-MM-DD) y hoy. `0` si la fecha no se
/// puede leer: es un dato derivado para mostrar, no una regla de negocio, así que
/// una fecha corrupta no debe tumbar la carga del formulario.
fn edad_en_anios(nacimiento: &str) -> i64 {
    let Ok(nac) = chrono::NaiveDate::parse_from_str(nacimiento.trim(), "%Y-%m-%d") else {
        return 0;
    };
    // «Hoy» es el día del PRESTADOR, no el del servidor. La diferencia son las
    // horas que van de una medianoche a la otra, y ahí decide si el paciente ya
    // cumplió años: en un servidor en UTC, a las 20:00 de Colombia ya es el día
    // siguiente y el niño «cumple» un día antes. En pediatría el tramo «0 años»
    // va del recién nacido al de once meses, así que ese día cuenta.
    let hoy = medico_dominio::zona::hoy();
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
        ant_perinatales: p.ant_perinatales.0.clone(),
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
    pub ant_perinatales: Option<serde_json::Value>,
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
    if ant.ant_perinatales.is_some() {
        sets.push("ant_perinatales = ?");
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
    if let Some(v) = &ant.ant_perinatales {
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

    /// Un padrón con tildes, cajas mezcladas y homónimos: es donde el plegado
    /// se nota. «PÁEZ» en versales existe de verdad en las bases importadas de
    /// sistemas viejos.
    async fn padron() -> crate::Pool {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, nombre2,
                                   apellido1, apellido2, fecha_nacimiento, sexo, institucion_id)
             VALUES (11,'CC','333','María','Fernanda','Páez','Gómez','1991-02-02','F',1),
                    (12,'CC','444','Beto',NULL,'PÁEZ',NULL,'1970-03-03','M',1),
                    (13,'CC','555','Ana',NULL,'Rúa',NULL,'1980-04-04','F',1),
                    (14,'CC','666','Iñaki',NULL,'Muñoz',NULL,'1975-06-06','M',1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    async fn ids(pool: &crate::Pool, aguja: &str) -> Vec<i64> {
        let h = buscar(
            pool,
            Ambito::institucion(1),
            &Criterio { buscar: Some(aguja), limite: None },
        )
        .await
        .unwrap();
        assert_eq!(h.total, h.filas.len() as i64, "sin techo, total y filas son lo mismo");
        h.filas.iter().map(|p| p.id).collect()
    }

    /// **Se busca por nombre, por apellido, por los dos juntos y por documento**,
    /// y la caja y las tildes no importan.
    ///
    /// La aguja que cruza dos columnas —«maria paez»— es la que no encuentra
    /// nunca el patrón de `nombre1 LIKE ? OR apellido1 LIKE ?` que usan
    /// incapacidades y remisiones, y es como se busca a una persona.
    #[tokio::test]
    async fn la_busqueda_pliega_caja_y_tildes_y_cruza_columnas() {
        let pool = padron().await;
        assert_eq!(ids(&pool, "paez").await, vec![11, 12], "sin tilde no encontró a los Páez");
        assert_eq!(ids(&pool, "PÁEZ").await, vec![11, 12], "en versales tampoco");
        assert_eq!(ids(&pool, "  Páez ").await, vec![11, 12], "los espacios de sobra cuentan");
        assert_eq!(ids(&pool, "maria paez").await, vec![11], "la aguja que cruza dos columnas");
        assert_eq!(ids(&pool, "fernanda").await, vec![11], "el segundo nombre también busca");
        assert_eq!(ids(&pool, "gomez").await, vec![11], "y el segundo apellido");
        assert_eq!(ids(&pool, "munoz").await, vec![14], "la eñe se pliega igual");
        assert_eq!(ids(&pool, "555").await, vec![13], "por documento");

        // LA PAREJA: lo que NO casa no sale. Sin esto, un buscador que devolviera
        // el padrón entero pasaría todas las de arriba.
        assert_eq!(ids(&pool, "zzz").await, Vec::<i64>::new());
        assert_eq!(ids(&pool, "rua").await, vec![13], "y no arrastra a los demás");
    }

    /// **El trigger de la 0051 pliega igual que `plegar`**, comprobado leyendo la
    /// columna `plegado` que el trigger ESCRIBE y comparándola con `plegar`.
    ///
    /// Son dos copias de una regla —el trigger en SQL, `plegar` en Rust— y ése es
    /// el riesgo del plegado: si divergen, la pantalla encuentra lo que la
    /// consulta no y no hay error en ninguna parte. Se cruza la frontera (el
    /// trigger de verdad sobre una fila insertada), no una expresión regenerada.
    #[tokio::test]
    async fn la_columna_plegado_dice_lo_mismo_que_plegar() {
        let pool = base().await;
        // (nombre1, nombre2, apellido1, apellido2, documento) con tildes, eñe,
        // diéresis y cajas mezcladas. El segundo vacío va NULL, que es como el
        // trigger deja de meter un espacio (la vía normal de `crear`).
        type Caso = (
            &'static str,
            Option<&'static str>,
            &'static str,
            Option<&'static str>,
            &'static str,
        );
        let casos: &[Caso] = &[
            ("María", Some("Fernanda"), "Páez", Some("Gómez"), "9-ABÉ"),
            ("IÑAKI", None, "Muñoz", None, "555"),
            ("Rúa", Some("Über"), "Ñandú", Some("Ýpsilon"), "X1"),
        ];
        for (i, (n1, n2, a1, a2, doc)) in casos.iter().enumerate() {
            let id = 100 + i as i64;
            sqlx::query(
                "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, nombre2, \
                 apellido1, apellido2, fecha_nacimiento, sexo, institucion_id) \
                 VALUES (?, 'CC', ?, ?, ?, ?, ?, '1990-01-01', 'F', 1)",
            )
            .bind(id)
            .bind(doc)
            .bind(n1)
            .bind(*n2)
            .bind(a1)
            .bind(*a2)
            .execute(&pool)
            .await
            .unwrap();

            let en_columna: String = sqlx::query_scalar("SELECT plegado FROM paciente WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();

            // El texto que el trigger pliega: nombre entero + documento, con los
            // que faltan (NULL) sin dejar hueco doble.
            let mut entero = n1.to_string();
            if let Some(n) = n2 {
                entero.push(' ');
                entero.push_str(n);
            }
            entero.push(' ');
            entero.push_str(a1);
            if let Some(a) = a2 {
                entero.push(' ');
                entero.push_str(a);
            }
            entero.push(' ');
            entero.push_str(doc);

            assert_eq!(
                plegar(&entero),
                en_columna,
                "el trigger pliega distinto que `plegar` para {n1} {a1}"
            );
        }
    }

    /// **El techo acota las filas y NO el total**, que es lo que deja decir «hay
    /// 84, se enseñan 20». Un techo que también recortara la cuenta convertiría
    /// «hay más» en «no hay más», que es truncar en silencio.
    #[tokio::test]
    async fn el_techo_recorta_las_filas_y_deja_intacto_el_total() {
        let pool = padron().await;
        let h = buscar(
            &pool,
            Ambito::institucion(1),
            &Criterio { buscar: None, limite: Some(2) },
        )
        .await
        .unwrap();
        assert_eq!(h.filas.len(), 2, "el techo no recortó");
        assert_eq!(h.total, 5, "el total se dejó recortar por el techo");

        // La pareja: sin techo salen todas, y el total no cambia.
        let h = buscar(&pool, Ambito::institucion(1), &Criterio::default()).await.unwrap();
        assert_eq!(h.filas.len(), 5);
        assert_eq!(h.total, 5);
    }

    /// **El criterio no abre la puerta del ámbito.** Es la comprobación que hay
    /// que repetir cada vez que una consulta gana un parámetro: el filtro nuevo
    /// se compone con el aislamiento, no lo sustituye.
    #[tokio::test]
    async fn buscar_sigue_acotado_a_la_institucion() {
        let pool = padron().await;
        // «Paz» es el apellido del paciente 20, que es de la institución 2.
        let h = buscar(
            &pool,
            Ambito::institucion(1),
            &Criterio { buscar: Some("Paz"), limite: None },
        )
        .await
        .unwrap();
        assert_eq!(h.total, 0, "se coló un paciente de otra institución");

        // Y el control de que la aguja servía: desde la institución 2 sí sale.
        let h = buscar(
            &pool,
            Ambito::institucion(2),
            &Criterio { buscar: Some("Paz"), limite: None },
        )
        .await
        .unwrap();
        assert_eq!(h.filas.len(), 1);
        assert_eq!(h.filas[0].id, 20);

        // Y sin institución no se pregunta nada.
        let h = buscar(&pool, Ambito::Ninguno, &Criterio::default()).await.unwrap();
        assert!(h.filas.is_empty());
        assert_eq!(h.total, 0);
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
            ciudad_residencia: None,
            departamento_residencia: None,
            nacionalidad: None,
            pertenencia_etnica: None,
            comunidad_etnica: None,
            discapacidad: None,
            identidad_genero: None,
        }
    }

    /// Un paciente SIN código de municipio pero CON la ciudad en texto, para
    /// probar la derivación (#84). El resto igual que `nuevo`.
    fn nuevo_con_ciudad(numero: &str, ciudad: &str, depto: Option<&str>) -> PacienteNuevo {
        PacienteNuevo {
            cod_municipio: None,
            ciudad_residencia: Some(ciudad.into()),
            departamento_residencia: depto.map(Into::into),
            ..nuevo(numero, "SinCodigo")
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

    /// #84/#603: los cuatro campos demográficos del PatientRDA (nacionalidad,
    /// etnia+comunidad, discapacidad, identidad de género) se capturan por la API,
    /// se leen de vuelta, y su procedencia queda `confirmado` (la afirmó la
    /// captura). Su pareja: un paciente creado SIN ellos los deja `None` y NO los
    /// mete en la procedencia — lo ausente no se inventa (directiva 20).
    #[tokio::test]
    async fn la_demografia_del_rda_se_captura_y_su_procedencia_queda() {
        let pool = base().await;
        let mut n = nuevo("700", "Demo");
        n.nacionalidad = Some("170".into());
        n.pertenencia_etnica = Some("1".into());
        n.comunidad_etnica = Some("Emberá".into());
        n.discapacidad = Some("02".into());
        n.identidad_genero = Some("03".into());
        let id = crear(&pool, Ambito::institucion(1), &n).await.unwrap().unwrap();

        let p = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(p.nacionalidad.as_deref(), Some("170"));
        assert_eq!(p.pertenencia_etnica.as_deref(), Some("1"));
        assert_eq!(p.comunidad_etnica.as_deref(), Some("Emberá"));
        assert_eq!(p.discapacidad.as_deref(), Some("02"));
        assert_eq!(p.identidad_genero.as_deref(), Some("03"));
        for campo in ["nacionalidad", "pertenencia_etnica", "comunidad_etnica", "discapacidad", "identidad_genero"] {
            assert_eq!(
                procedencia_de(&p.procedencias, campo).as_deref(),
                Some("confirmado"),
                "la captura afirma {campo}"
            );
        }

        // Editar reemplaza: lo que la edición no trae se borra (no se hereda).
        let mut e = nuevo("700", "Demo");
        e.identidad_genero = Some("02".into());
        assert!(actualizar(&pool, Ambito::institucion(1), id, &e).await.unwrap());
        let p2 = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(p2.identidad_genero.as_deref(), Some("02"));
        assert_eq!(p2.nacionalidad, None, "lo que la edición no trae no se conserva");

        // Pareja: sin demografía, los campos quedan None y no ensucian la procedencia.
        let id2 = crear(&pool, Ambito::institucion(1), &nuevo("701", "Vacio")).await.unwrap().unwrap();
        let q = obtener(&pool, Ambito::institucion(1), id2).await.unwrap().unwrap();
        assert_eq!(q.discapacidad, None);
        assert_eq!(procedencia_de(&q.procedencias, "discapacidad"), None, "lo ausente no se inventa");
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

    /// El cuarto bloque es `ant_perinatales` —del NIÑO— y no `ant_obstetricos`
    /// —de la madre—, que es como se llamaba y era mentira (migración `0040`).
    ///
    /// La prueba guarda los ocho campos que faltaban frente al sistema vivo y
    /// los vuelve a leer. Son los indicadores de AIEPI y de crecimiento y
    /// desarrollo: si alguno se pierde por el camino, el informe epidemiológico
    /// nuevo no puede dar los números que el prestador viene entregando, y eso
    /// no se nota hasta el cierre del mes.
    #[tokio::test]
    async fn los_perinatales_guardan_los_indicadores_de_aiepi() {
        let pool = base().await;
        let bloque = json!({
            "embarazo_deseado": true,
            "hosp_ira": true,
            "hosp_eda": false,
            "hosp_otra": "bronquiolitis a los 8 meses",
            "lactancia_meses": 12,
            "inmunizacion_completa": true,
            "desarrollo_psicomotor_normal": false,
            "hermanos_muertos_menores_5": 0
        });
        let ant = Antecedentes {
            ant_perinatales: Some(bloque.clone()),
            ..Default::default()
        };
        assert!(actualizar_antecedentes(&pool, Ambito::institucion(1), 10, &ant).await.unwrap());

        let p = obtener(&pool, Ambito::institucion(1), 10).await.unwrap().unwrap();
        assert_eq!(p.ant_perinatales.0, bloque, "vuelve entero, campo por campo");
        // `0` y `false` tienen que sobrevivir: son respuestas, no ausencias.
        // Cero hermanos muertos menores de cinco años es un dato que el clínico
        // afirmó, y confundirlo con «no preguntado» es la directiva 20 al revés.
        assert_eq!(p.ant_perinatales.0["hermanos_muertos_menores_5"], json!(0));
        assert_eq!(p.ant_perinatales.0["desarrollo_psicomotor_normal"], json!(false));
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

    /// Una base con DIVIPOLA sembrada y una institución, para probar la
    /// derivación del municipio contra el catálogo real (directiva 28: cruza a
    /// la base, no se queda en la lógica pura).
    async fn base_con_divipola() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        crate::divipola::sembrar(&pool).await.unwrap();
        sqlx::query("INSERT INTO institucion (id, nombre, nit) VALUES (1,'Uno','900')")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    fn procedencia_de(json: &str, campo: &str) -> Option<String> {
        let m: serde_json::Value = serde_json::from_str(json).unwrap();
        m.get(campo).and_then(|v| v.as_str()).map(String::from)
    }

    /// #84 de extremo a extremo: crear un paciente con la ciudad en TEXTO y sin
    /// código deriva el `cod_municipio` y lo marca `derivado`. Y su pareja, la
    /// que descarta un derivador ciego: una ciudad AMBIGUA no deriva nada.
    #[tokio::test]
    async fn crear_deriva_el_municipio_del_texto_de_la_ciudad() {
        let pool = base_con_divipola().await;

        // VERDE: «Medellin» (sin tilde) es único → 05001, marcado derivado.
        let id = crear(&pool, Ambito::institucion(1), &nuevo_con_ciudad("501", "Medellin", None))
            .await
            .unwrap()
            .unwrap();
        let p = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(p.cod_municipio.as_deref(), Some("05001"), "derivó el municipio");
        assert_eq!(
            procedencia_de(&p.procedencias, "cod_municipio").as_deref(),
            Some("derivado"),
            "y lo marcó derivado, no confirmado"
        );

        // ROJO: «San Juan» sin departamento es ambiguo → NO deriva, queda sin
        // código, y no hay procedencia de municipio inventada.
        let id2 = crear(&pool, Ambito::institucion(1), &nuevo_con_ciudad("502", "San Juan", None))
            .await
            .unwrap()
            .unwrap();
        let p2 = obtener(&pool, Ambito::institucion(1), id2).await.unwrap().unwrap();
        assert_eq!(p2.cod_municipio, None, "un homónimo no se resuelve al primero");
        assert_eq!(
            procedencia_de(&p2.procedencias, "cod_municipio"),
            None,
            "sin derivar, no se marca nada"
        );
    }

    /// El código que TECLEÓ un humano manda: no se deriva encima, y queda
    /// `confirmado`, no `derivado`. Es la regla de no pisar lo confirmado.
    #[tokio::test]
    async fn el_codigo_tecleado_no_lo_pisa_la_derivacion() {
        let pool = base_con_divipola().await;
        // Trae código propio (08001) Y una ciudad que derivaría a otro (Medellín).
        let mut datos = nuevo_con_ciudad("503", "Medellin", None);
        datos.cod_municipio = Some("08001".into());
        let id = crear(&pool, Ambito::institucion(1), &datos).await.unwrap().unwrap();
        let p = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(p.cod_municipio.as_deref(), Some("08001"), "no se derivó encima del tecleado");
        assert_eq!(
            procedencia_de(&p.procedencias, "cod_municipio").as_deref(),
            Some("confirmado"),
            "lo tecleó un humano → confirmado"
        );
    }

    /// La derivación LLEVA EL ÁMBITO: pedirla con el ámbito de OTRA institución
    /// no lee ni escribe el paciente. Con el ámbito bueno, sí deriva. (Arreglo de
    /// la observación de la revisión 2026-08-21: era `pub` sin ámbito.)
    #[tokio::test]
    async fn derivar_municipio_respeta_el_ambito() {
        let pool = base_con_divipola().await;
        // Un paciente de la institución 1, sin código y con ciudad en texto,
        // insertado en crudo (sin pasar por `crear`, que ya derivaría). Procedencia
        // en BLANCO, que es como nace una fila del padrón legado (migración 0069).
        sqlx::query(
            "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, apellido1,
                                   fecha_nacimiento, sexo, ciudad_residencia, procedencias,
                                   institucion_id)
             VALUES (77,'CC','999','Ana','Ruiz','1990-01-01','F','Medellin','',1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        // ROJO: con el ámbito de la institución 2, no deriva ni toca la fila.
        assert_eq!(
            derivar_municipio(&pool, Ambito::institucion(2), 77).await.unwrap(),
            None,
            "un id ajeno no se deriva"
        );
        let p = obtener(&pool, Ambito::institucion(1), 77).await.unwrap().unwrap();
        assert_eq!(p.cod_municipio, None, "y no se escribió nada");

        // VERDE: con el ámbito bueno, deriva.
        assert_eq!(
            derivar_municipio(&pool, Ambito::institucion(1), 77).await.unwrap(),
            Some("05001".into()),
        );
        // Y la fila legada, tras derivar, queda marcada `derivado` — el blanco de
        // partida no la dejó fuera de la auditoría.
        let p = obtener(&pool, Ambito::institucion(1), 77).await.unwrap().unwrap();
        assert_eq!(procedencia_de(&p.procedencias, "cod_municipio").as_deref(), Some("derivado"));
    }

    /// #84, B2: `derivar_municipio` empieza de un mapa vacío cuando la procedencia
    /// está en BLANCO (padrón legado, migración 0069) — no revienta—. Su pareja:
    /// un JSON CORRUPTO sí se propaga como error, no se traga a un mapa vacío en
    /// silencio (directivas 20/23: blanco = «empezar de cero»; el resto, a error).
    #[tokio::test]
    async fn el_blanco_deriva_pero_el_json_corrupto_revienta() {
        let pool = base_con_divipola().await;
        sqlx::query(
            "INSERT INTO paciente (id, tipo_documento, numero_documento, nombre1, apellido1,
                                   fecha_nacimiento, sexo, ciudad_residencia, procedencias,
                                   institucion_id)
             VALUES (88,'CC','1','Beatriz','Lopez','1985-02-02','F','Medellin','{roto',1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        // ROJO: la procedencia corrupta revienta al derivar (no se traga en silencio).
        assert!(
            derivar_municipio(&pool, Ambito::institucion(1), 88).await.is_err(),
            "un JSON de procedencia ilegible se propaga como error"
        );
    }

    /// #84, la integridad del «duro sin aval»: reenviar el formulario con el
    /// municipio DERIVADO sin cambiarlo NO lo promueve a `confirmado` —seguiría
    /// derivado, para que el cierre lo siga marcando—. Su pareja: cambiar el
    /// código a otro SÍ lo confirma (un humano lo afirmó).
    #[tokio::test]
    async fn editar_no_promueve_un_municipio_derivado_a_confirmado() {
        let pool = base_con_divipola().await;
        let id = crear(&pool, Ambito::institucion(1), &nuevo_con_ciudad("601", "Medellin", None))
            .await
            .unwrap()
            .unwrap();
        // Nació derivado.
        let p0 = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(procedencia_de(&p0.procedencias, "cod_municipio").as_deref(), Some("derivado"));

        // Edición que REENVÍA el mismo código (como haría el formulario
        // pre-rellenado) y cambia otra cosa: NO se promueve.
        let mut editar = nuevo("601", "Ana");
        editar.cod_municipio = Some("05001".into()); // el derivado, sin cambio
        editar.celular = Some("3009999999".into());
        assert!(actualizar(&pool, Ambito::institucion(1), id, &editar).await.unwrap());
        let p1 = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(
            procedencia_de(&p1.procedencias, "cod_municipio").as_deref(),
            Some("derivado"),
            "reenviar el mismo código no es un aval humano"
        );

        // Pareja: cambiar el código a otro SÍ lo confirma.
        let mut cambiar = nuevo("601", "Ana");
        cambiar.cod_municipio = Some("08001".into()); // otro municipio, tecleado
        assert!(actualizar(&pool, Ambito::institucion(1), id, &cambiar).await.unwrap());
        let p2 = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(p2.cod_municipio.as_deref(), Some("08001"));
        assert_eq!(
            procedencia_de(&p2.procedencias, "cod_municipio").as_deref(),
            Some("confirmado"),
            "cambiar el código a mano sí es un aval humano"
        );
    }

    /// Caso rojo de la no-promoción (observación de la revisión 2026-08-22): si la
    /// `procedencias` de la fila está CORRUPTA, editar no la traga a un `None` que
    /// promovería el municipio a `confirmado` en silencio —el falso negativo de la
    /// directiva 23—: revienta, igual que `derivar_municipio`. (No ocurre por
    /// ninguna ruta del producto; solo por manipulación directa de la base.)
    #[tokio::test]
    async fn editar_con_procedencias_corrupta_revienta_no_promueve() {
        let pool = base_con_divipola().await;
        let id = crear(&pool, Ambito::institucion(1), &nuevo_con_ciudad("601", "Medellin", None))
            .await
            .unwrap()
            .unwrap();
        // Corrompe la columna a mano (lo que ningún camino del producto hace).
        sqlx::query("UPDATE paciente SET procedencias = '{roto' WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        let mut editar = nuevo("601", "Ana");
        editar.cod_municipio = Some("05001".into());
        editar.celular = Some("3009999999".into());
        assert!(
            actualizar(&pool, Ambito::institucion(1), id, &editar).await.is_err(),
            "una procedencia ilegible se propaga como error, no se promueve en silencio"
        );
    }

    /// El departamento desambigua a un homónimo TAMBIÉN por esta vía de base.
    /// «San Juan» solo es ambiguo sin departamento; en La Guajira hay uno solo
    /// que lo contiene —«San Juan del Cesar», 44650, que pese al nombre es de La
    /// Guajira—.
    #[tokio::test]
    async fn el_departamento_desambigua_en_la_base() {
        let pool = base_con_divipola().await;
        let id = crear(
            &pool,
            Ambito::institucion(1),
            &nuevo_con_ciudad("504", "San Juan", Some("La Guajira")),
        )
        .await
        .unwrap()
        .unwrap();
        let p = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(
            p.cod_municipio.as_deref(),
            Some("44650"),
            "con el departamento, el único San Juan de La Guajira"
        );
        assert_eq!(
            procedencia_de(&p.procedencias, "cod_municipio").as_deref(),
            Some("derivado"),
        );
    }
}
