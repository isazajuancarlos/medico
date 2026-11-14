//! El registro de ENFERMERÍA: los cuatro módulos del bloque hospitalario (#249).
//!
//! `ENFERMERIA` (la nota), `PLAN_CUIDADO`, `MAR` (administración de medicamentos)
//! y `KARDEX`. Van juntos porque son un solo flujo de turno, y porque tres de los
//! cuatro **no necesitaban tabla**: lo que faltaba era la puerta.
//!
//! # Lo que se midió antes de escribir
//!
//! | módulo | dónde vive | |
//! |---|---|---|
//! | `ENFERMERIA` | `documento_clinico`, tipo `NOTA_ENFERMERIA` | la espina genérica |
//! | `PLAN_CUIDADO` | `documento_clinico`, tipo `PLAN_CUIDADO` | igual |
//! | `KARDEX` | **derivado** | fórmulas activas + lo administrado de cada una |
//! | `MAR` | tabla `administracion_medicamento` | el único hecho nuevo |
//!
//! Es la lección de `terceros`, que resultó ser `entidad`, y de
//! `soportecobertura`, que resultó ser un reporte: buscar por el nombre del
//! módulo da por ausente lo que ya está con otro nombre.
//!
//! # El tipo del documento lo pone el SERVIDOR, no el cliente
//!
//! `POST /api/documentos` exige `EscribirClinico` —médico— y acepta el `tipo` que
//! le manden. Si la nota de enfermería entrara por ahí habría que abrir esa ruta
//! a enfermería, y con ella la EPICRISIS y la fórmula. Por eso el bloque tiene
//! rutas propias bajo `/api/enfermeria/` donde **el tipo es una constante del
//! endpoint**: no hay valor que el cliente pueda mandar para escribir otra cosa.
//! La puerta la cierra la forma, no una comprobación que se pueda olvidar.

use medico_dominio::Ambito;
use serde::{Deserialize, Serialize};

use crate::pertenencia::{opcional_es_del_paciente, Enlace};

/// El tipo con el que la nota de enfermería vive en `documento_clinico`.
pub const TIPO_NOTA: &str = "NOTA_ENFERMERIA";
/// El tipo del plan de cuidados.
pub const TIPO_PLAN: &str = "PLAN_CUIDADO";

/// El tipo de la nota de trabajo social. Vive aquí, junto a los otros dos de
/// disciplina, porque comparte la regla: el tipo lo pone el SERVIDOR y
/// `/api/documentos` lo rechaza (#249).
pub const TIPO_TRABAJO_SOCIAL: &str = "NOTA_TRABAJO_SOCIAL";

/// Los tipos que **solo** pueden entrar por las rutas de enfermería.
///
/// `POST /api/documentos` los rechaza: si un médico pudiera crear una nota de
/// enfermería desde la ruta general, el registro diría que enfermería escribió
/// algo que no escribió — y la autoría es lo que hace válida la historia.
pub const TIPOS_PROPIOS: [&str; 3] = [TIPO_NOTA, TIPO_PLAN, TIPO_TRABAJO_SOCIAL];

/// Los tres estados de una administración. No hay un cuarto implícito: una
/// casilla vacía no es un estado (directiva 20).
pub const ESTADOS: [&str; 3] = ["ADMINISTRADO", "OMITIDO", "APLAZADO"];

/// El estado que NO exige motivo. Los otros dos sí, y lo exige también el `CHECK`.
pub const ADMINISTRADO: &str = "ADMINISTRADO";

// ── MAR: la administración ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Administracion {
    pub id: i64,
    pub formula_id: i64,
    pub hospitalizacion_id: Option<i64>,
    pub paciente_id: i64,
    pub administrado_por: i64,
    pub programada_para: Option<String>,
    pub fecha: String,
    pub estado: String,
    pub motivo: Option<String>,
    pub dosis_administrada: Option<String>,
    pub via: Option<String>,
    pub observaciones: Option<String>,
    pub anulada: bool,
    pub motivo_anulacion: Option<String>,
    /// Unidos para la hoja del turno.
    pub medicamento: String,
    pub dosis_ordenada: String,
    pub quien: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NuevaAdministracion {
    pub formula_id: i64,
    #[serde(default)]
    pub hospitalizacion_id: Option<i64>,
    #[serde(default)]
    pub programada_para: Option<String>,
    pub estado: String,
    #[serde(default)]
    pub motivo: Option<String>,
    #[serde(default)]
    pub dosis_administrada: Option<String>,
    #[serde(default)]
    pub via: Option<String>,
    #[serde(default)]
    pub observaciones: Option<String>,
}

/// Por qué se rechaza una administración.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falta {
    /// La fórmula no existe, o su paciente es de otra institución.
    FormulaInexistente,
    EstadoDesconocido,
    /// No se administró y no se dijo por qué. Es el dato que se busca al día
    /// siguiente, y el que no se puede reconstruir después.
    SinMotivoDeLaOmision,
    /// Se administró y no se dijo cuánto.
    SinDosisAdministrada,
    /// Anular sin decir por qué borra la prueba sin dejar otra.
    SinMotivoDeAnulacion,
    /// Ya estaba anulada: anular dos veces no es idempotente aquí, porque la
    /// segunda anulación sobrescribiría el motivo de la primera.
    YaAnulada,
    /// La atención que se enlaza **no es de este paciente** — o no existe, o es
    /// de otra institución. Las tres se responden igual: distinguirlas diría que
    /// existe.
    AtencionQueNoEsDelPaciente,
}

impl Falta {
    pub fn clave(self) -> &'static str {
        match self {
            Falta::FormulaInexistente => "formula_inexistente",
            Falta::EstadoDesconocido => "estado",
            Falta::SinMotivoDeLaOmision => "sin_motivo",
            Falta::SinDosisAdministrada => "sin_dosis",
            Falta::SinMotivoDeAnulacion => "sin_motivo_anulacion",
            Falta::YaAnulada => "ya_anulada",
            Falta::AtencionQueNoEsDelPaciente => "atencion_que_no_es_del_paciente",
        }
    }

    pub fn de_clave(clave: &str) -> Option<Falta> {
        TODAS.iter().copied().find(|f| f.clave() == clave)
    }

    pub fn mensaje(self) -> &'static str {
        match self {
            Falta::FormulaInexistente => "esa orden no existe en esta institución",
            Falta::EstadoDesconocido => {
                "el estado debe ser ADMINISTRADO, OMITIDO o APLAZADO"
            }
            Falta::SinMotivoDeLaOmision => {
                "una dosis que no se administró tiene que decir por qué: es lo primero que \
                 se pregunta en el turno siguiente, y después ya no se puede reconstruir"
            }
            Falta::SinDosisAdministrada => {
                "si se administró, hay que anotar qué dosis se puso"
            }
            Falta::SinMotivoDeAnulacion => "para anular un registro hay que decir por qué",
            Falta::YaAnulada => "ese registro ya estaba anulado",
            Falta::AtencionQueNoEsDelPaciente => {
                "la hospitalización que se enlaza no es de ese paciente"
            }
        }
    }
}

pub const TODAS: [Falta; 7] = [
    Falta::FormulaInexistente,
    Falta::EstadoDesconocido,
    Falta::SinMotivoDeLaOmision,
    Falta::SinDosisAdministrada,
    Falta::SinMotivoDeAnulacion,
    Falta::YaAnulada,
    Falta::AtencionQueNoEsDelPaciente,
];

/// Comprueba la administración antes de tocar la base.
pub fn revisar(n: &NuevaAdministracion) -> Result<(), Falta> {
    if !ESTADOS.contains(&n.estado.as_str()) {
        return Err(Falta::EstadoDesconocido);
    }
    let hay = |v: &Option<String>| v.as_deref().is_some_and(|s| !s.trim().is_empty());
    if n.estado == ADMINISTRADO {
        if !hay(&n.dosis_administrada) {
            return Err(Falta::SinDosisAdministrada);
        }
    } else if !hay(&n.motivo) {
        return Err(Falta::SinMotivoDeLaOmision);
    }
    Ok(())
}

const SELECT_MAR: &str = "\
    SELECT a.id, a.formula_id, a.hospitalizacion_id, a.paciente_id, a.administrado_por, \
    a.programada_para, a.fecha, a.estado, a.motivo, a.dosis_administrada, a.via, \
    a.observaciones, a.anulada, a.motivo_anulacion, \
    f.medicamento, f.dosis AS dosis_ordenada, COALESCE(u.nombre,'') AS quien \
    FROM administracion_medicamento a \
    JOIN formula f ON f.id = a.formula_id \
    JOIN paciente p ON p.id = a.paciente_id \
    LEFT JOIN usuario u ON u.id = a.administrado_por";

fn institucion(ambito: Ambito) -> Option<Option<i64>> {
    match ambito {
        Ambito::Superadmin => Some(None),
        Ambito::Institucion(i) => Some(Some(i)),
        Ambito::Ninguno => None,
    }
}

/// La hoja de administraciones de un paciente, de la más reciente a la más
/// antigua. Incluye las ANULADAS: una hoja que las escondiera diría que la dosis
/// nunca se registró, y lo que pasó es que se registró y se corrigió.
pub async fn hoja_del_paciente(
    pool: &crate::Pool,
    ambito: Ambito,
    paciente_id: i64,
) -> Result<Vec<Administracion>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = String::from(SELECT_MAR);
    sql.push_str(" WHERE a.paciente_id = ?");
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    sql.push_str(" ORDER BY a.fecha DESC, a.id DESC");
    let mut q = sqlx::query_as::<_, Administracion>(&sql).bind(paciente_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

/// Registra una administración (o su omisión). Devuelve el id.
pub async fn administrar(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &NuevaAdministracion,
    usuario_id: i64,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    if let Err(f) = revisar(n) {
        return Ok(Err(f));
    }
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::FormulaInexistente));
    };
    // El paciente se DERIVA de la fórmula, no se recibe: quien administra no
    // elige a quién se le apunta la dosis. Y la misma consulta comprueba el
    // ámbito, así que una fórmula ajena responde «no existe».
    let mut sql = String::from(
        "SELECT p.id FROM formula f JOIN historia h ON h.id = f.historia_id \
         JOIN paciente p ON p.id = h.paciente_id WHERE f.id = ?",
    );
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, i64>(&sql).bind(n.formula_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    let Some(paciente_id) = q.fetch_optional(pool).await? else {
        return Ok(Err(Falta::FormulaInexistente));
    };
    // El paciente se derivó de la fórmula y está acotado; el ingreso llega del
    // cliente y no lo estaba.
    if !opcional_es_del_paciente(pool, Enlace::Hospitalizacion, n.hospitalizacion_id, paciente_id)
        .await?
    {
        return Ok(Err(Falta::AtencionQueNoEsDelPaciente));
    }

    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO administracion_medicamento (formula_id, hospitalizacion_id, paciente_id, \
         administrado_por, programada_para, fecha, estado, motivo, dosis_administrada, via, \
         observaciones) VALUES (?,?,?,?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(n.formula_id)
    .bind(n.hospitalizacion_id)
    .bind(paciente_id)
    .bind(usuario_id)
    .bind(n.programada_para.as_deref())
    .bind(&ahora)
    .bind(&n.estado)
    .bind(n.motivo.as_deref())
    .bind(n.dosis_administrada.as_deref())
    .bind(n.via.as_deref())
    .bind(n.observaciones.as_deref())
    .fetch_one(pool)
    .await?;
    Ok(Ok(id))
}

/// Anula un registro. **No lo borra ni lo edita**: un MAR es prueba, y corregir
/// en su sitio destruye lo que decía antes. La corrección es una fila nueva.
pub async fn anular(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    motivo: &str,
    usuario_id: i64,
) -> Result<Result<(), Falta>, sqlx::Error> {
    if motivo.trim().is_empty() {
        return Ok(Err(Falta::SinMotivoDeAnulacion));
    }
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::FormulaInexistente));
    };
    let mut sql = String::from(
        "SELECT a.anulada FROM administracion_medicamento a \
         JOIN paciente p ON p.id = a.paciente_id WHERE a.id = ?",
    );
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, bool>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    match q.fetch_optional(pool).await? {
        None => return Ok(Err(Falta::FormulaInexistente)),
        Some(true) => return Ok(Err(Falta::YaAnulada)),
        Some(false) => {}
    }
    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    sqlx::query(
        "UPDATE administracion_medicamento SET anulada = 1, anulada_por = ?, anulada_en = ?, \
         motivo_anulacion = ? WHERE id = ?",
    )
    .bind(usuario_id)
    .bind(&ahora)
    .bind(motivo.trim())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(Ok(()))
}

// ── KARDEX: la vista, que no tiene tabla ─────────────────────────────────────

/// Una línea del kardex: la orden y lo que se ha hecho con ella.
///
/// **No hay tabla `kardex`, y es la decisión de diseño del módulo.** Lo que el
/// kardex enseña son las fórmulas del ingreso y sus administraciones, y los dos
/// datos ya existen. Una tabla propia sería una tercera copia que se
/// desincroniza del original al primer cambio de orden médica — y entonces el
/// kardex diría que hay que poner algo que el médico ya suspendió.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct LineaKardex {
    pub formula_id: i64,
    pub medicamento: String,
    pub codigo_cum: Option<String>,
    pub dosis: String,
    pub unidad: Option<String>,
    pub dias_tratamiento: Option<i64>,
    pub observaciones: Option<String>,
    pub fecha_formula: String,
    /// Cuántas veces se administró de verdad (sin contar anuladas ni omitidas).
    pub administradas: i64,
    /// Cuántas se omitieron. Va al lado del anterior a propósito: un kardex que
    /// solo contara lo puesto escondería justo lo que hay que mirar.
    pub omitidas: i64,
    pub ultima: Option<String>,
}

/// El kardex de un ingreso: las órdenes del paciente y su cumplimiento.
///
/// Se resuelve por PACIENTE y no por hospitalización, aunque la firme un ingreso:
/// las órdenes cuelgan de la historia, no de la cama, y un paciente que cambia de
/// cama no cambia de tratamiento.
pub async fn kardex(
    pool: &crate::Pool,
    ambito: Ambito,
    paciente_id: i64,
) -> Result<Vec<LineaKardex>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = String::from(
        "SELECT f.id AS formula_id, f.medicamento, f.codigo_cum, f.dosis, f.unidad, \
         f.dias_tratamiento, f.observaciones, f.fecha_formula, \
         COALESCE(SUM(CASE WHEN a.estado = 'ADMINISTRADO' AND a.anulada = 0 THEN 1 ELSE 0 END), 0) \
             AS administradas, \
         COALESCE(SUM(CASE WHEN a.estado <> 'ADMINISTRADO' AND a.anulada = 0 THEN 1 ELSE 0 END), 0) \
             AS omitidas, \
         MAX(CASE WHEN a.anulada = 0 THEN a.fecha END) AS ultima \
         FROM formula f \
         JOIN historia h ON h.id = f.historia_id \
         JOIN paciente p ON p.id = h.paciente_id \
         LEFT JOIN administracion_medicamento a ON a.formula_id = f.id \
         WHERE p.id = ?",
    );
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    sql.push_str(" GROUP BY f.id ORDER BY f.fecha_formula DESC, f.id DESC");
    let mut q = sqlx::query_as::<_, LineaKardex>(&sql).bind(paciente_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn puesta() -> NuevaAdministracion {
        NuevaAdministracion {
            formula_id: 1,
            hospitalizacion_id: None,
            programada_para: Some("2026-08-16T14:00:00Z".into()),
            estado: "ADMINISTRADO".into(),
            motivo: None,
            dosis_administrada: Some("500 mg".into()),
            via: Some("VO".into()),
            observaciones: None,
        }
    }

    #[test]
    fn la_administracion_normal_pasa() {
        assert_eq!(revisar(&puesta()), Ok(()));
    }

    /// La omisión SIN motivo se rechaza. Es la regla por la que existe el
    /// módulo: un hueco no distingue «no se puso» de «se puso y no se anotó».
    #[test]
    fn la_omision_sin_motivo_se_rechaza() {
        let mut n = puesta();
        n.estado = "OMITIDO".into();
        n.dosis_administrada = None;
        assert_eq!(revisar(&n), Err(Falta::SinMotivoDeLaOmision));
    }

    /// Su pareja: CON motivo pasa, y sin dosis — a una dosis que no se puso no
    /// se le anota cuánto se puso.
    #[test]
    fn la_omision_con_motivo_pasa_y_no_exige_dosis() {
        let mut n = puesta();
        n.estado = "OMITIDO".into();
        n.dosis_administrada = None;
        n.motivo = Some("paciente en ayunas para cirugía".into());
        assert_eq!(revisar(&n), Ok(()));
    }

    #[test]
    fn lo_administrado_sin_dosis_se_rechaza() {
        let mut n = puesta();
        n.dosis_administrada = None;
        assert_eq!(revisar(&n), Err(Falta::SinDosisAdministrada));
    }

    /// Un motivo de espacios en blanco es un motivo ausente disfrazado.
    #[test]
    fn el_motivo_en_blanco_no_cuenta_como_motivo() {
        let mut n = puesta();
        n.estado = "APLAZADO".into();
        n.dosis_administrada = None;
        n.motivo = Some("   ".into());
        assert_eq!(revisar(&n), Err(Falta::SinMotivoDeLaOmision));
    }

    #[test]
    fn el_estado_inventado_se_rechaza() {
        let mut n = puesta();
        n.estado = "PUESTA".into();
        assert_eq!(revisar(&n), Err(Falta::EstadoDesconocido));
    }

    #[test]
    fn los_tres_estados_del_producto_se_aceptan() {
        for e in ESTADOS {
            let mut n = puesta();
            n.estado = e.into();
            if e != ADMINISTRADO {
                n.dosis_administrada = None;
                n.motivo = Some("el paciente lo rechazó".into());
            }
            assert_eq!(revisar(&n), Ok(()), "«{e}» es un estado del producto");
        }
    }

    #[test]
    fn las_faltas_van_y_vuelven_por_su_clave() {
        let mut vistas = std::collections::BTreeSet::new();
        for f in TODAS {
            assert!(vistas.insert(f.clave()), "clave repetida: {}", f.clave());
            assert_eq!(Falta::de_clave(f.clave()), Some(f));
        }
        assert_eq!(Falta::de_clave("inventada"), None);
    }
}

#[cfg(test)]
mod contra_la_base {
    use super::*;
    use medico_dominio::Ambito;

    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO institucion (id,nombre,nit,codigo_habilitacion) \
             VALUES (1,'Uno','900123456','050010123401'),(2,'Dos','900999','050010999901')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) \
             VALUES (5,'Enf. Ruiz','e@x.co','h','enfermeria',1,1), \
                    (6,'Dr. Paz','d@x.co','h','medico',1,1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
             fecha_nacimiento,sexo,institucion_id) \
             VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1), \
                    (20,'CC','2','Beto','Paz','1985-01-01','M',2)",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Una historia y una fórmula por paciente. La del 20 es de la otra
        // institución: existe para que el aislamiento tenga algo que NO deba salir.
        sqlx::query(
            "INSERT INTO historia (id,fecha,codigo_historia,paciente_id,medico_id,creado_por,\
             motivo_consulta,enfermedad_actual,plan_tratamiento,diagnostico_principal) \
             VALUES (100,'2026-08-16T08:00:00Z','H100',10,6,6,'dolor','desde ayer','reposo','A000'), \
                    (200,'2026-08-16T08:00:00Z','H200',20,6,6,'dolor','desde ayer','reposo','A000')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO formula (id,historia_id,medicamento,dosis,cantidad,unidad) \
             VALUES (1,100,'Acetaminofén 500 mg','500 mg cada 8 horas',30,'tableta'), \
                    (2,200,'Ibuprofeno 400 mg','400 mg cada 12 horas',20,'tableta')",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    /// **El ingreso que se enlaza tiene que ser de ese paciente** — caso rojo de una
    /// de las cuatro claves foráneas que se ataban en crudo.
    ///
    /// Las encontró la revisión de seguridad de esta rama, y las dejó por debajo
    /// del listón porque hoy ninguna consulta de lectura une estas tablas: el
    /// daño es una fila de la institución A apuntando a una de la B, no una
    /// fuga. Se cierran igual, y en la misma pasada que la quinta —la de
    /// `defuncion`, donde el mismo descuido SÍ era un oráculo—, porque son **un
    /// defecto mirado por dos lados** y el lado callado no se queja nunca
    /// (directiva 23).
    #[tokio::test]
    async fn el_ingreso_que_no_es_del_paciente_no_se_enlaza() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO cama (id,numero,piso,servicio,estado,institucion_id) \
             VALUES (1,'101','1','MEDICINA','OCUPADA',1),(2,'201','2','MEDICINA','OCUPADA',2)",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO hospitalizacion (id,paciente_id,cama_id,medico_tratante_id,fecha_ingreso,estado) \
             VALUES (77,10,1,6,'2026-08-15T10:00:00Z','ACTIVA'), \
                    (88,20,2,6,'2026-08-14T03:17:00Z','ACTIVA')",
        )
        .execute(&pool).await.unwrap();
        const ESPERADO: Result<i64, Falta> = Err(Falta::AtencionQueNoEsDelPaciente);
        let mut n = pruebas_puesta();
        n.hospitalizacion_id = Some(88);
        assert_eq!(administrar(&pool, Ambito::Institucion(1), &n, 5).await.unwrap(), ESPERADO);
        let mut x = pruebas_puesta();
        x.hospitalizacion_id = Some(9999);
        assert_eq!(administrar(&pool, Ambito::Institucion(1), &x, 5).await.unwrap(), ESPERADO);
        // POSITIVO DE CONTROL: el ingreso del paciente de la fórmula sí entra.
        let mut ok = pruebas_puesta();
        ok.hospitalizacion_id = Some(77);
        assert!(administrar(&pool, Ambito::Institucion(1), &ok, 5).await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn se_administra_y_sale_en_la_hoja() {
        let pool = base().await;
        let id = administrar(&pool, Ambito::Institucion(1), &super::pruebas_puesta(), 5)
            .await
            .unwrap()
            .unwrap();
        let hoja = hoja_del_paciente(&pool, Ambito::Institucion(1), 10).await.unwrap();
        assert_eq!(hoja.len(), 1);
        assert_eq!(hoja[0].id, id);
        // El paciente NO se recibió: se derivó de la fórmula.
        assert_eq!(hoja[0].paciente_id, 10);
        assert_eq!(hoja[0].quien, "Enf. Ruiz");
        assert_eq!(hoja[0].medicamento, "Acetaminofén 500 mg");
    }

    /// Administrar contra una fórmula de OTRA institución se rechaza. Es la
    /// puerta por la que se le apuntaría una dosis al paciente de otro.
    #[tokio::test]
    async fn no_se_administra_contra_la_formula_de_otra_institucion() {
        let pool = base().await;
        let mut n = super::pruebas_puesta();
        n.formula_id = 2;
        assert_eq!(
            administrar(&pool, Ambito::Institucion(1), &n, 5).await.unwrap(),
            Err(Falta::FormulaInexistente)
        );
        // Positivo de control: la 1 sí entra, así que el rechazo es del ámbito
        // y no de que `administrar` rechace todo.
        assert!(administrar(&pool, Ambito::Institucion(1), &super::pruebas_puesta(), 5)
            .await
            .unwrap()
            .is_ok());
    }

    #[tokio::test]
    async fn una_institucion_no_ve_la_hoja_de_la_otra() {
        let pool = base().await;
        administrar(&pool, Ambito::Institucion(1), &super::pruebas_puesta(), 5)
            .await
            .unwrap()
            .unwrap();
        assert!(hoja_del_paciente(&pool, Ambito::Institucion(2), 10).await.unwrap().is_empty());
        assert_eq!(hoja_del_paciente(&pool, Ambito::Institucion(1), 10).await.unwrap().len(), 1);
    }

    /// Anular no borra: la fila sigue en la hoja, marcada, con su motivo. Si
    /// desapareciera, la hoja diría que la dosis nunca se registró.
    #[tokio::test]
    async fn anular_deja_la_fila_marcada_y_no_la_borra() {
        let pool = base().await;
        let id = administrar(&pool, Ambito::Institucion(1), &super::pruebas_puesta(), 5)
            .await
            .unwrap()
            .unwrap();
        anular(&pool, Ambito::Institucion(1), id, "error de paciente", 5)
            .await
            .unwrap()
            .unwrap();
        let hoja = hoja_del_paciente(&pool, Ambito::Institucion(1), 10).await.unwrap();
        assert_eq!(hoja.len(), 1, "la fila sigue: anular no es borrar");
        assert!(hoja[0].anulada);
        assert_eq!(hoja[0].motivo_anulacion.as_deref(), Some("error de paciente"));
        // Y no se puede anular dos veces: la segunda pisaría el motivo de la primera.
        assert_eq!(
            anular(&pool, Ambito::Institucion(1), id, "otra vez", 5).await.unwrap(),
            Err(Falta::YaAnulada)
        );
    }

    #[tokio::test]
    async fn anular_sin_motivo_se_rechaza() {
        let pool = base().await;
        let id = administrar(&pool, Ambito::Institucion(1), &super::pruebas_puesta(), 5)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            anular(&pool, Ambito::Institucion(1), id, "   ", 5).await.unwrap(),
            Err(Falta::SinMotivoDeAnulacion)
        );
    }

    /// El kardex cuenta lo puesto y lo omitido por separado, y **no cuenta las
    /// anuladas**. Es la prueba de que la vista derivada dice la verdad sin tener
    /// tabla propia.
    #[tokio::test]
    async fn el_kardex_cuenta_puestas_omitidas_y_no_cuenta_anuladas() {
        let pool = base().await;
        let a = Ambito::Institucion(1);
        administrar(&pool, a, &super::pruebas_puesta(), 5).await.unwrap().unwrap();
        let mut omitida = super::pruebas_puesta();
        omitida.estado = "OMITIDO".into();
        omitida.dosis_administrada = None;
        omitida.motivo = Some("paciente en ayunas".into());
        administrar(&pool, a, &omitida, 5).await.unwrap().unwrap();
        let anulable = administrar(&pool, a, &super::pruebas_puesta(), 5).await.unwrap().unwrap();
        anular(&pool, a, anulable, "se registró dos veces", 5).await.unwrap().unwrap();

        let k = kardex(&pool, a, 10).await.unwrap();
        assert_eq!(k.len(), 1, "una orden");
        assert_eq!(k[0].administradas, 1, "dos puestas menos una anulada");
        assert_eq!(k[0].omitidas, 1);
        assert_eq!(k[0].medicamento, "Acetaminofén 500 mg");
    }

    /// Una orden SIN ninguna administración sale en el kardex con ceros, no
    /// desaparece: es justo la que hay que poner. Un `JOIN` en vez de un `LEFT
    /// JOIN` la escondería, y el kardex enseñaría solo lo ya hecho.
    #[tokio::test]
    async fn la_orden_sin_administrar_sale_en_el_kardex_con_cero() {
        let pool = base().await;
        let k = kardex(&pool, Ambito::Institucion(1), 10).await.unwrap();
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].administradas, 0);
        assert_eq!(k[0].omitidas, 0);
        assert!(k[0].ultima.is_none());
    }

    #[tokio::test]
    async fn el_kardex_respeta_el_ambito() {
        let pool = base().await;
        assert!(kardex(&pool, Ambito::Institucion(2), 10).await.unwrap().is_empty());
        assert_eq!(kardex(&pool, Ambito::Institucion(1), 10).await.unwrap().len(), 1);
        assert!(kardex(&pool, Ambito::Ninguno, 10).await.unwrap().is_empty());
    }

    /// El `CHECK` de la migración es la puerta de atrás: un importador que no
    /// pase por `revisar` tampoco puede meter una omisión muda.
    #[tokio::test]
    async fn el_check_rechaza_la_omision_muda_por_la_puerta_de_atras() {
        let pool = base().await;
        let sql = "INSERT INTO administracion_medicamento (formula_id,paciente_id,\
                   administrado_por,fecha,estado,motivo) VALUES (1,10,5,'2026-08-16T14:00:00Z',?,?)";
        // Positivo de control: con motivo sí entra.
        sqlx::query(sql).bind("OMITIDO").bind("en ayunas").execute(&pool).await
            .expect("la omisión CON motivo tiene que entrar");
        for (estado, motivo, que) in [
            ("OMITIDO", None, "omisión sin motivo"),
            ("APLAZADO", Some("  "), "motivo en blanco"),
            ("PUESTA", Some("x"), "estado fuera del vocabulario"),
        ] {
            let r = sqlx::query(sql).bind(estado).bind(motivo).execute(&pool).await;
            assert!(r.is_err(), "el CHECK tenía que rechazar: {que}");
        }
    }
}

/// La administración de prueba, compartida por los dos módulos de prueba.
#[cfg(test)]
fn pruebas_puesta() -> NuevaAdministracion {
    NuevaAdministracion {
        formula_id: 1,
        hospitalizacion_id: None,
        programada_para: Some("2026-08-16T14:00:00Z".into()),
        estado: "ADMINISTRADO".into(),
        motivo: None,
        dosis_administrada: Some("500 mg".into()),
        via: Some("VO".into()),
        observaciones: None,
    }
}
