//! Banco de sangre (#249): unidades y transfusiones.
//!
//! # La regla de compatibilidad NO está aquí
//!
//! Vive en `medico_dominio::sangre`, porque es una regla y se prueba sin base, en
//! microsegundos y con las 16 combinaciones escritas a mano. Aquí está lo que la
//! base sí puede garantizar: que una unidad no se transfunda dos veces, y que la
//! que no pasó el tamizaje no se transfunda nunca.
//!
//! # Tres puertas para la misma transfusión, y ninguna sobra
//!
//! 1. El `UNIQUE` de `transfusion.unidad_id` — la base impide la segunda.
//! 2. El `CHECK` de `unidad_sangre` — sin tamizaje conforme no hay estado
//!    disponible, así que un importador tampoco puede.
//! 3. `transfundir()` — comprueba la compatibilidad y **dice por qué** cuando no.
//!
//! Las dos primeras no pueden explicar nada; la tercera sí. Y la tercera sola se
//! puede rodear por otra puerta. No es redundancia: es que cada una hace algo que
//! las otras no.

use medico_dominio::sangre::{self, Hemocomponente, Veredicto};
use medico_dominio::Ambito;
use serde::{Deserialize, Serialize};

pub const ESTADOS: [&str; 5] = [
    "EN_CUARENTENA",
    "DISPONIBLE",
    "RESERVADA",
    "TRANSFUNDIDA",
    "DESCARTADA",
];

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Unidad {
    pub id: i64,
    pub codigo: String,
    pub hemocomponente: String,
    pub grupo: String,
    pub rh: String,
    pub fecha_obtencion: Option<String>,
    pub fecha_vencimiento: String,
    pub tamizaje_conforme: Option<bool>,
    pub estado: String,
    pub motivo_descarte: Option<String>,
    pub creado_en: String,
}

impl Unidad {
    /// ¿Está vencida a la fecha dada? Se compara, no se guarda.
    pub fn vencida(&self, hoy: &str) -> bool {
        self.fecha_vencimiento.as_str() < hoy
    }

    /// ¿Se puede transfundir, dejando la compatibilidad aparte?
    ///
    /// Devuelve el motivo cuando no. Es lo que la pantalla enseña antes de
    /// ofrecer el botón, para que nadie descubra la regla por un rechazo.
    pub fn transfundible(&self, hoy: &str) -> Result<(), &'static str> {
        if self.tamizaje_conforme != Some(true) {
            return Err("la unidad no tiene tamizaje conforme");
        }
        if self.estado == "TRANSFUNDIDA" {
            return Err("la unidad ya se transfundió");
        }
        if self.estado == "DESCARTADA" {
            return Err("la unidad está descartada");
        }
        if self.vencida(hoy) {
            return Err("la unidad está vencida");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevaUnidad {
    pub codigo: String,
    pub hemocomponente: String,
    pub grupo: String,
    pub rh: String,
    #[serde(default)]
    pub fecha_obtencion: Option<String>,
    pub fecha_vencimiento: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevaTransfusion {
    pub unidad_id: i64,
    pub paciente_id: i64,
    /// Del receptor, confirmado en el momento. No se lee de ninguna columna
    /// guardada: ver la cabecera de `medico_dominio::sangre`.
    pub grupo_receptor: String,
    pub rh_receptor: String,
    #[serde(default)]
    pub observaciones: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Transfusion {
    pub id: i64,
    pub unidad_id: i64,
    pub paciente_id: i64,
    pub fecha: String,
    pub grupo_receptor: String,
    pub rh_receptor: String,
    pub reaccion_adversa: bool,
    pub descripcion_reaccion: Option<String>,
    pub observaciones: Option<String>,
    pub codigo_unidad: String,
    pub hemocomponente: String,
    pub grupo_unidad: String,
    pub rh_unidad: String,
    pub paciente: String,
    pub responsable: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Falta {
    UnidadInexistente,
    PacienteInexistente,
    CodigoRepetido,
    HemocomponenteDesconocido,
    GrupoORhDesconocido,
    /// La unidad no se puede transfundir por su estado, su tamizaje o su fecha.
    NoTransfundible(String),
    /// Incompatible, con el motivo que da el dominio.
    Incompatible(String),
    /// Falta un dato para poder decidir. **Nunca se resuelve a favor.**
    NoSePuedeSaber(String),
    SinMotivoDeDescarte,
    /// Una reacción adversa sin describir no sirve para investigarla.
    ReaccionSinDescripcion,
}

impl Falta {
    pub fn clave(&self) -> &'static str {
        match self {
            Falta::UnidadInexistente => "unidad_inexistente",
            Falta::PacienteInexistente => "paciente_inexistente",
            Falta::CodigoRepetido => "codigo_repetido",
            Falta::HemocomponenteDesconocido => "hemocomponente",
            Falta::GrupoORhDesconocido => "grupo_rh",
            Falta::NoTransfundible(_) => "no_transfundible",
            Falta::Incompatible(_) => "incompatible",
            Falta::NoSePuedeSaber(_) => "no_se_puede_saber",
            Falta::SinMotivoDeDescarte => "sin_motivo_descarte",
            Falta::ReaccionSinDescripcion => "reaccion_sin_descripcion",
        }
    }

    /// El mensaje. Los tres que llevan texto lo devuelven TAL CUAL: lo escribe el
    /// dominio y reescribirlo aquí daría dos versiones del mismo rechazo.
    pub fn mensaje(&self) -> String {
        match self {
            Falta::UnidadInexistente => "esa unidad no existe en esta institución".into(),
            Falta::PacienteInexistente => "ese paciente no existe en esta institución".into(),
            Falta::CodigoRepetido => {
                "ya hay una unidad con ese código: dos con el mismo número hacen imposible la \
                 trazabilidad"
                    .into()
            }
            Falta::HemocomponenteDesconocido => "ese hemocomponente no es del producto".into(),
            Falta::GrupoORhDesconocido => "el grupo va O, A, B o AB, y el RH + o −".into(),
            Falta::NoTransfundible(m) => m.clone(),
            Falta::Incompatible(m) => m.clone(),
            Falta::NoSePuedeSaber(m) => m.clone(),
            Falta::SinMotivoDeDescarte => "descartar una unidad exige decir por qué".into(),
            Falta::ReaccionSinDescripcion => {
                "una reacción adversa tiene que describirse: es lo que se investiga después".into()
            }
        }
    }
}

/// El texto que la PANTALLA enseña para una clave.
///
/// Las faltas que llevan motivo dentro —incompatible, no transfundible— no
/// pueden viajar por la URL: meter texto libre en un enlace es dejar que
/// cualquiera le haga decir a la pantalla lo que quiera. Viaja la clave, que es
/// un conjunto cerrado, y aquí se resuelve a un texto fijo. El detalle completo
/// —qué antígeno, qué fecha— lo da la API JSON, que es donde se integra.
pub fn mensaje_de_clave(clave: &str) -> Option<&'static str> {
    Some(match clave {
        "unidad_inexistente" => "esa unidad no existe en esta institución",
        "paciente_inexistente" => "ese paciente no existe en esta institución",
        "codigo_repetido" => "ya hay una unidad con ese código",
        "hemocomponente" => "ese hemocomponente no es del producto",
        "grupo_rh" => "el grupo va O, A, B o AB, y el RH + o −",
        "no_transfundible" => {
            "esa unidad no se puede transfundir: mírela en la tabla, ahí dice por qué"
        }
        "incompatible" => {
            "la unidad NO es compatible con el grupo del receptor. No se transfundió"
        }
        "no_se_puede_saber" => {
            "falta un dato para poder decidir la compatibilidad. No se transfundió: lo que no \
             se puede saber nunca se resuelve a favor"
        }
        "sin_motivo_descarte" => "descartar una unidad exige decir por qué",
        "reaccion_sin_descripcion" => "una reacción adversa tiene que describirse",
        _ => return None,
    })
}

fn institucion(ambito: Ambito) -> Option<Option<i64>> {
    match ambito {
        Ambito::Superadmin => Some(None),
        Ambito::Institucion(i) => Some(Some(i)),
        Ambito::Ninguno => None,
    }
}

const SELECT_UNIDAD: &str = "\
    SELECT id, codigo, hemocomponente, grupo, rh, fecha_obtencion, fecha_vencimiento, \
    tamizaje_conforme, estado, motivo_descarte, creado_en FROM unidad_sangre";

pub async fn listar_unidades(
    pool: &crate::Pool,
    ambito: Ambito,
) -> Result<Vec<Unidad>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = String::from(SELECT_UNIDAD);
    if inst.is_some() {
        sql.push_str(" WHERE institucion_id = ?");
    }
    sql.push_str(" ORDER BY fecha_vencimiento, id");
    let mut q = sqlx::query_as::<_, Unidad>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

pub async fn registrar_unidad(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevaUnidad,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    if Hemocomponente::de_codigo(&n.hemocomponente).is_none() {
        return Ok(Err(Falta::HemocomponenteDesconocido));
    }
    if !sangre::GRUPOS.contains(&n.grupo.as_str()) || !sangre::RH.contains(&n.rh.as_str()) {
        return Ok(Err(Falta::GrupoORhDesconocido));
    }
    let Some(Some(institucion_id)) = institucion(ambito) else {
        return Ok(Err(Falta::UnidadInexistente));
    };
    let repetido: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM unidad_sangre WHERE institucion_id = ? AND codigo = ?")
            .bind(institucion_id)
            .bind(n.codigo.trim())
            .fetch_optional(pool)
            .await?;
    if repetido.is_some() {
        return Ok(Err(Falta::CodigoRepetido));
    }
    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO unidad_sangre (institucion_id, codigo, hemocomponente, grupo, rh, \
         fecha_obtencion, fecha_vencimiento, creado_en) VALUES (?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(institucion_id)
    .bind(n.codigo.trim())
    .bind(&n.hemocomponente)
    .bind(&n.grupo)
    .bind(&n.rh)
    .bind(n.fecha_obtencion.as_deref())
    .bind(&n.fecha_vencimiento)
    .bind(&ahora)
    .fetch_one(pool)
    .await?;
    Ok(Ok(id))
}

/// Da por conforme o no conforme el tamizaje, y mueve el estado en consecuencia.
///
/// No conforme **descarta** la unidad con su motivo: dejarla en cuarentena
/// esperando una decisión es cómo una bolsa infectada acaba en una nevera junto a
/// las buenas.
pub async fn tamizar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    conforme: bool,
    motivo: Option<&str>,
) -> Result<Result<(), Falta>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::UnidadInexistente));
    };
    let mut sql = String::from("SELECT 1 FROM unidad_sangre WHERE id = ?");
    if inst.is_some() {
        sql.push_str(" AND institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, i64>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if q.fetch_optional(pool).await?.is_none() {
        return Ok(Err(Falta::UnidadInexistente));
    }
    if conforme {
        sqlx::query(
            "UPDATE unidad_sangre SET tamizaje_conforme = 1, estado = 'DISPONIBLE' WHERE id = ?",
        )
        .bind(id)
        .execute(pool)
        .await?;
    } else {
        let Some(m) = motivo.map(str::trim).filter(|m| !m.is_empty()) else {
            return Ok(Err(Falta::SinMotivoDeDescarte));
        };
        sqlx::query(
            "UPDATE unidad_sangre SET tamizaje_conforme = 0, estado = 'DESCARTADA', \
             motivo_descarte = ? WHERE id = ?",
        )
        .bind(m)
        .bind(id)
        .execute(pool)
        .await?;
    }
    Ok(Ok(()))
}

/// Transfunde una unidad. Es la operación con más puertas del producto.
pub async fn transfundir(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevaTransfusion,
    usuario_id: i64,
    hoy: &str,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::UnidadInexistente));
    };
    let mut sql = String::from(SELECT_UNIDAD);
    sql.push_str(" WHERE id = ?");
    if inst.is_some() {
        sql.push_str(" AND institucion_id = ?");
    }
    let mut q = sqlx::query_as::<_, Unidad>(&sql).bind(n.unidad_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    let Some(unidad) = q.fetch_optional(pool).await? else {
        return Ok(Err(Falta::UnidadInexistente));
    };

    let mut sql = String::from("SELECT 1 FROM paciente WHERE id = ?");
    if inst.is_some() {
        sql.push_str(" AND institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, i64>(&sql).bind(n.paciente_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if q.fetch_optional(pool).await?.is_none() {
        return Ok(Err(Falta::PacienteInexistente));
    }

    if let Err(m) = unidad.transfundible(hoy) {
        return Ok(Err(Falta::NoTransfundible(m.to_string())));
    }

    let Some(componente) = Hemocomponente::de_codigo(&unidad.hemocomponente) else {
        // La unidad tiene un hemocomponente que el dominio no conoce. NO se
        // transfunde: no se puede calcular compatibilidad de algo que no se sabe
        // qué es (directiva 20).
        return Ok(Err(Falta::NoSePuedeSaber(format!(
            "la unidad {} tiene un hemocomponente que este producto no reconoce ({})",
            unidad.codigo, unidad.hemocomponente
        ))));
    };
    match sangre::compatible(
        componente,
        &unidad.grupo,
        &unidad.rh,
        &n.grupo_receptor,
        &n.rh_receptor,
    ) {
        Veredicto::Compatible => {}
        Veredicto::Incompatible(m) => return Ok(Err(Falta::Incompatible(m))),
        Veredicto::NoSePuedeSaber(m) => return Ok(Err(Falta::NoSePuedeSaber(m))),
    }

    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO transfusion (unidad_id, paciente_id, responsable_id, fecha, grupo_receptor, \
         rh_receptor, observaciones, creado_en) VALUES (?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(n.unidad_id)
    .bind(n.paciente_id)
    .bind(usuario_id)
    .bind(&ahora)
    .bind(&n.grupo_receptor)
    .bind(&n.rh_receptor)
    .bind(n.observaciones.as_deref())
    .bind(&ahora)
    .fetch_one(pool)
    .await?;
    sqlx::query("UPDATE unidad_sangre SET estado = 'TRANSFUNDIDA' WHERE id = ?")
        .bind(n.unidad_id)
        .execute(pool)
        .await?;
    Ok(Ok(id))
}

const SELECT_TRANSFUSION: &str = "\
    SELECT t.id, t.unidad_id, t.paciente_id, t.fecha, t.grupo_receptor, t.rh_receptor, \
    t.reaccion_adversa, t.descripcion_reaccion, t.observaciones, \
    u.codigo AS codigo_unidad, u.hemocomponente, u.grupo AS grupo_unidad, u.rh AS rh_unidad, \
    (p.nombre1 || ' ' || p.apellido1) AS paciente, COALESCE(r.nombre,'') AS responsable \
    FROM transfusion t \
    JOIN unidad_sangre u ON u.id = t.unidad_id \
    JOIN paciente p ON p.id = t.paciente_id \
    LEFT JOIN usuario r ON r.id = t.responsable_id";

pub async fn listar_transfusiones(
    pool: &crate::Pool,
    ambito: Ambito,
) -> Result<Vec<Transfusion>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = String::from(SELECT_TRANSFUSION);
    if inst.is_some() {
        sql.push_str(" WHERE p.institucion_id = ?");
    }
    sql.push_str(" ORDER BY t.fecha DESC, t.id DESC");
    let mut q = sqlx::query_as::<_, Transfusion>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

/// Anota una reacción adversa sobre una transfusión ya hecha.
pub async fn anotar_reaccion(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    descripcion: &str,
) -> Result<Result<(), Falta>, sqlx::Error> {
    if descripcion.trim().is_empty() {
        return Ok(Err(Falta::ReaccionSinDescripcion));
    }
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::UnidadInexistente));
    };
    let mut sql = String::from(
        "SELECT 1 FROM transfusion t JOIN paciente p ON p.id = t.paciente_id WHERE t.id = ?",
    );
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, i64>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if q.fetch_optional(pool).await?.is_none() {
        return Ok(Err(Falta::UnidadInexistente));
    }
    sqlx::query(
        "UPDATE transfusion SET reaccion_adversa = 1, descripcion_reaccion = ? WHERE id = ?",
    )
    .bind(descripcion.trim())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(Ok(()))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO institucion (id,nombre,nit,codigo_habilitacion) \
             VALUES (1,'Uno','900123456','050010123401'),(2,'Dos','900999','050010999901')",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) \
             VALUES (6,'Dr. Paz','d@x.co','h','medico',1,1)",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
             fecha_nacimiento,sexo,institucion_id) \
             VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1), \
                    (20,'CC','2','Beto','Paz','1985-01-01','M',2)",
        )
        .execute(&pool).await.unwrap();
        pool
    }

    fn unidad(codigo: &str, grupo: &str, rh: &str) -> NuevaUnidad {
        NuevaUnidad {
            codigo: codigo.into(),
            hemocomponente: "GLOBULOS_ROJOS".into(),
            grupo: grupo.into(),
            rh: rh.into(),
            fecha_obtencion: Some("2026-08-01".into()),
            fecha_vencimiento: "2026-09-15".into(),
        }
    }

    fn transfusion(unidad_id: i64, grupo: &str, rh: &str) -> NuevaTransfusion {
        NuevaTransfusion {
            unidad_id,
            paciente_id: 10,
            grupo_receptor: grupo.into(),
            rh_receptor: rh.into(),
            observaciones: None,
        }
    }

    /// El ciclo bueno: se registra, se tamiza, se transfunde y la unidad queda
    /// marcada.
    /// Toda falta tiene texto para la pantalla. Sin esto, una clave nueva
    /// viajaría por la URL y no pintaría nada — el rechazo se vería como éxito.
    #[test]
    fn toda_falta_tiene_mensaje_de_pantalla() {
        let todas = [
            Falta::UnidadInexistente,
            Falta::PacienteInexistente,
            Falta::CodigoRepetido,
            Falta::HemocomponenteDesconocido,
            Falta::GrupoORhDesconocido,
            Falta::NoTransfundible(String::new()),
            Falta::Incompatible(String::new()),
            Falta::NoSePuedeSaber(String::new()),
            Falta::SinMotivoDeDescarte,
            Falta::ReaccionSinDescripcion,
        ];
        for f in &todas {
            assert!(
                mensaje_de_clave(f.clave()).is_some(),
                "la clave «{}» no tiene texto de pantalla",
                f.clave()
            );
        }
        assert!(mensaje_de_clave("inventada").is_none());
    }

    #[tokio::test]
    async fn el_ciclo_completo_de_una_unidad() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = registrar_unidad(&pool, a, &unidad("U-1", "O", "-")).await.unwrap().unwrap();
        // Recién registrada NO se puede transfundir: está en cuarentena.
        assert!(matches!(
            transfundir(&pool, a, &transfusion(id, "A", "+"), 6, "2026-08-16").await.unwrap(),
            Err(Falta::NoTransfundible(_))
        ));
        tamizar(&pool, a, id, true, None).await.unwrap().unwrap();
        let t = transfundir(&pool, a, &transfusion(id, "A", "+"), 6, "2026-08-16")
            .await.unwrap().unwrap();
        let lista = listar_transfusiones(&pool, a).await.unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].id, t);
        assert_eq!(lista[0].codigo_unidad, "U-1");
        assert_eq!(lista[0].paciente, "Ana Ruiz");
        // Y la unidad queda transfundida.
        let u = listar_unidades(&pool, a).await.unwrap();
        assert_eq!(u[0].estado, "TRANSFUNDIDA");
    }

    /// **Una unidad no se transfunde dos veces.** Lo impide el `UNIQUE` de la
    /// base, que es la garantía que ninguna comprobación en el código puede dar
    /// frente a dos peticiones a la vez.
    #[tokio::test]
    async fn una_unidad_no_se_transfunde_dos_veces() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = registrar_unidad(&pool, a, &unidad("U-1", "O", "-")).await.unwrap().unwrap();
        tamizar(&pool, a, id, true, None).await.unwrap().unwrap();
        transfundir(&pool, a, &transfusion(id, "A", "+"), 6, "2026-08-16").await.unwrap().unwrap();
        assert!(matches!(
            transfundir(&pool, a, &transfusion(id, "A", "+"), 6, "2026-08-16").await.unwrap(),
            Err(Falta::NoTransfundible(_))
        ));
    }

    /// La incompatibilidad se rechaza **con el motivo del dominio**.
    #[tokio::test]
    async fn la_transfusion_incompatible_se_rechaza_con_su_motivo() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = registrar_unidad(&pool, a, &unidad("U-A", "A", "+")).await.unwrap().unwrap();
        tamizar(&pool, a, id, true, None).await.unwrap().unwrap();
        let r = transfundir(&pool, a, &transfusion(id, "O", "+"), 6, "2026-08-16").await.unwrap();
        let Err(Falta::Incompatible(m)) = r else {
            panic!("A → O tenía que ser incompatible, salió {r:?}");
        };
        assert!(m.contains("ABO"), "{m}");
        // Y la unidad NO se marcó como transfundida.
        assert_eq!(listar_unidades(&pool, a).await.unwrap()[0].estado, "DISPONIBLE");
    }

    /// Un grupo de receptor que no se puede leer NO se resuelve a favor.
    #[tokio::test]
    async fn el_grupo_ilegible_del_receptor_no_transfunde() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = registrar_unidad(&pool, a, &unidad("U-1", "O", "-")).await.unwrap().unwrap();
        tamizar(&pool, a, id, true, None).await.unwrap().unwrap();
        assert!(matches!(
            transfundir(&pool, a, &transfusion(id, "", "+"), 6, "2026-08-16").await.unwrap(),
            Err(Falta::NoSePuedeSaber(_))
        ));
    }

    /// El tamizaje NO conforme descarta con motivo, y esa unidad ya no se
    /// transfunde nunca.
    #[tokio::test]
    async fn el_tamizaje_no_conforme_descarta_y_cierra_la_puerta() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = registrar_unidad(&pool, a, &unidad("U-1", "O", "-")).await.unwrap().unwrap();
        assert_eq!(
            tamizar(&pool, a, id, false, None).await.unwrap(),
            Err(Falta::SinMotivoDeDescarte)
        );
        tamizar(&pool, a, id, false, Some("reactivo para hepatitis B")).await.unwrap().unwrap();
        let u = listar_unidades(&pool, a).await.unwrap();
        assert_eq!(u[0].estado, "DESCARTADA");
        assert!(matches!(
            transfundir(&pool, a, &transfusion(id, "O", "-"), 6, "2026-08-16").await.unwrap(),
            Err(Falta::NoTransfundible(_))
        ));
    }

    /// La unidad vencida no se transfunde. Y su pareja: **antes de vencer, sí** —
    /// sin ella, la regla podría estar rechazándolo todo.
    #[tokio::test]
    async fn la_unidad_vencida_no_se_transfunde_pero_la_vigente_si() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = registrar_unidad(&pool, a, &unidad("U-1", "O", "-")).await.unwrap().unwrap();
        tamizar(&pool, a, id, true, None).await.unwrap().unwrap();
        assert!(matches!(
            transfundir(&pool, a, &transfusion(id, "O", "-"), 6, "2026-10-01").await.unwrap(),
            Err(Falta::NoTransfundible(_))
        ), "vence el 2026-09-15");
        assert!(transfundir(&pool, a, &transfusion(id, "O", "-"), 6, "2026-09-01")
            .await.unwrap().is_ok());
    }

    /// El `CHECK` de la base impide poner DISPONIBLE lo que no se tamizó, aunque
    /// se entre por la puerta de atrás. Con su positivo de control.
    #[tokio::test]
    async fn el_check_impide_disponible_sin_tamizaje() {
        let pool = base().await;
        let sql = "INSERT INTO unidad_sangre (institucion_id,codigo,hemocomponente,grupo,rh,\
                   fecha_vencimiento,tamizaje_conforme,estado,creado_en) \
                   VALUES (1,?,'GLOBULOS_ROJOS','O','-','2026-09-15',?,'DISPONIBLE','x')";
        sqlx::query(sql).bind("OK").bind(Some(1)).execute(&pool).await
            .expect("tamizada Y disponible entra");
        let sin: Option<i64> = None;
        assert!(sqlx::query(sql).bind("X1").bind(sin).execute(&pool).await.is_err(),
                "sin tamizar no puede estar disponible");
        assert!(sqlx::query(sql).bind("X2").bind(Some(0)).execute(&pool).await.is_err(),
                "tamizaje NO conforme tampoco");
    }

    #[tokio::test]
    async fn el_codigo_repetido_se_rechaza_y_el_ambito_aisla() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        registrar_unidad(&pool, a, &unidad("U-1", "O", "-")).await.unwrap().unwrap();
        assert_eq!(
            registrar_unidad(&pool, a, &unidad("U-1", "A", "+")).await.unwrap(),
            Err(Falta::CodigoRepetido)
        );
        // La otra institución sí puede usar ese número: es único POR institución.
        assert!(registrar_unidad(&pool, Ambito::Institucion(2), &unidad("U-1", "A", "+"))
            .await.unwrap().is_ok());
        assert_eq!(listar_unidades(&pool, a).await.unwrap().len(), 1);
        assert!(listar_unidades(&pool, Ambito::Ninguno).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn no_se_transfunde_al_paciente_de_otra_institucion() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = registrar_unidad(&pool, a, &unidad("U-1", "O", "-")).await.unwrap().unwrap();
        tamizar(&pool, a, id, true, None).await.unwrap().unwrap();
        let mut n = transfusion(id, "O", "-");
        n.paciente_id = 20;
        assert_eq!(
            transfundir(&pool, a, &n, 6, "2026-08-16").await.unwrap(),
            Err(Falta::PacienteInexistente)
        );
    }

    #[tokio::test]
    async fn la_reaccion_adversa_exige_descripcion() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        let id = registrar_unidad(&pool, a, &unidad("U-1", "O", "-")).await.unwrap().unwrap();
        tamizar(&pool, a, id, true, None).await.unwrap().unwrap();
        let t = transfundir(&pool, a, &transfusion(id, "O", "-"), 6, "2026-08-16")
            .await.unwrap().unwrap();
        assert_eq!(
            anotar_reaccion(&pool, a, t, "  ").await.unwrap(),
            Err(Falta::ReaccionSinDescripcion)
        );
        anotar_reaccion(&pool, a, t, "fiebre y escalofrío a los 10 minutos").await.unwrap().unwrap();
        let v = listar_transfusiones(&pool, a).await.unwrap();
        assert!(v[0].reaccion_adversa);
        assert!(v[0].descripcion_reaccion.is_some());
    }
}
