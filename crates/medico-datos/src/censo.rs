//! Censo diario de hospitalización (#249).
//!
//! # Tampoco tiene tabla, y por la misma razón que el kardex
//!
//! El censo de un día es una CUENTA sobre `hospitalizacion` y `cama`: quién
//! estaba, quién entró, quién salió y quién queda. Guardarlo sería una tercera
//! copia que envejece —un ingreso corregido a las 11 de la noche dejaría el censo
//! de ayer mintiendo para siempre— y además haría irreproducible el documento que
//! el hospital archiva. Se deriva, y por eso siempre cuadra con la realidad de la
//! base.
//!
//! Van tres módulos seguidos que no necesitaban tabla —`soportecobertura`,
//! `KARDEX` y éste—, y el patrón empieza a ser el aviso: cuando un «módulo que
//! falta» es un INFORME sobre datos que ya existen, la tabla nueva es la
//! respuesta equivocada.
//!
//! # La ecuación del censo, que es lo que lo hace un censo y no una lista
//!
//! ```text
//! existencia_inicial + ingresos − egresos = existencia_final
//! ```
//!
//! Un censo que no cuadra está mal, y punto. Aquí se calculan los cuatro
//! números POR SEPARADO —cada uno con su consulta— y luego se comprueba la
//! identidad. Derivar el último de los otros tres haría que cuadrara siempre,
//! incluso con los datos rotos: sería un adorno, no una comprobación
//! (directiva 30). Cuando no cuadra, el censo lo DICE en vez de imprimir un
//! número plausible.
//!
//! # El día es LOCAL y los instantes están en UTC
//!
//! El censo se lee por la mañana y se refiere al día de ayer del hospital, no al
//! día UTC. Los límites se convierten igual que en `estadisticas`, con el
//! extremo superior exclusivo.

use medico_dominio::{zona, Ambito};
use serde::Serialize;

/// Una cama con quién la ocupa, si la ocupa alguien.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CamaDelCenso {
    pub cama_id: i64,
    pub numero: String,
    pub piso: String,
    pub servicio: String,
    pub estado: String,
    pub hospitalizacion_id: Option<i64>,
    pub paciente_id: Option<i64>,
    pub paciente: Option<String>,
    pub identificacion: Option<String>,
    pub fecha_ingreso: Option<String>,
}

/// Los egresos del día, por tipo. Se enseñan separados porque una defunción y un
/// alta médica no son la misma noticia, y el censo es donde se ven las dos.
#[derive(Debug, Clone, Serialize)]
pub struct EgresosPorTipo {
    pub alta_medica: i64,
    pub alta_voluntaria: i64,
    pub remision: i64,
    pub defuncion: i64,
    /// Egresos cuyo estado no es ninguno de los cuatro. No se reparten entre los
    /// otros: un estado desconocido se cuenta aparte para que se vea, en vez de
    /// engordar el que más se le parezca (directiva 20).
    pub sin_clasificar: i64,
}

impl EgresosPorTipo {
    pub fn total(&self) -> i64 {
        self.alta_medica
            + self.alta_voluntaria
            + self.remision
            + self.defuncion
            + self.sin_clasificar
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Censo {
    /// El día LOCAL al que se refiere, `AAAA-MM-DD`.
    pub fecha: String,
    pub existencia_inicial: i64,
    pub ingresos: i64,
    pub egresos: EgresosPorTipo,
    pub existencia_final: i64,
    /// Camas del ámbito, con su ocupante actual.
    pub camas: Vec<CamaDelCenso>,
    pub camas_totales: i64,
    pub camas_ocupadas: i64,
    /// `None` cuando no hay camas: dividir entre cero daría `NaN`, y un informe
    /// con `NaN` en el porcentaje de ocupación se imprime igual.
    pub ocupacion: Option<f64>,
    /// **Vacío cuando el censo cuadra.** Si no cuadra, dice en qué.
    ///
    /// No se corrige solo ni se esconde: un censo descuadrado es un dato roto en
    /// `hospitalizacion` —un egreso anterior a su ingreso, una fecha ilegible— y
    /// quien lo lea tiene que saberlo antes de firmarlo.
    pub descuadres: Vec<String>,
}

/// El día local convertido a los dos extremos UTC que guarda la base.
/// El superior es exclusivo: así el día entra entero sin depender de los segundos.
fn limites_del_dia(fecha: &str) -> Option<(String, String)> {
    let d = chrono::NaiveDate::parse_from_str(fecha, "%Y-%m-%d").ok()?;
    Some((
        instante_utc(d)?,
        instante_utc(d + chrono::Duration::days(1))?,
    ))
}

fn instante_utc(dia: chrono::NaiveDate) -> Option<String> {
    let local = dia.and_hms_opt(0, 0, 0)?;
    // `a_utc` devuelve `None` en un salto de hora legal: la medianoche de ese día
    // no existió. No se inventa la más cercana — el censo diría que cubre un
    // instante que nadie vivió.
    Some(zona::a_utc(local)?.format("%Y-%m-%d %H:%M:%S").to_string())
}

/// Los instantes se comparan con la `T` sustituida, como en `estadisticas`: la
/// base guarda unas veces `2026-08-16T04:00:00Z` y otras `2026-08-16 04:00:00`.
fn instante(col: &str) -> String {
    format!("replace(replace({col},'T',' '),'Z','')")
}

fn filtro(ambito: Ambito) -> Option<Option<i64>> {
    match ambito {
        Ambito::Superadmin => Some(None),
        Ambito::Institucion(i) => Some(Some(i)),
        Ambito::Ninguno => None,
    }
}

async fn contar(
    pool: &crate::Pool,
    sql: &str,
    inst: Option<i64>,
    valores: &[&str],
) -> Result<i64, sqlx::Error> {
    // AQUÍ la institución va PRIMERO porque su `?` está en `base`, o sea antes
    // que los de las fechas en la cadena. Atarla al final hacía que el filtro de
    // institución recibiera una fecha y la primera fecha recibiera el id — y el
    // resultado no era un error, era un CERO. Lo cazaron a la vez la prueba de
    // la ecuación y la de aislamiento.
    //
    // **La regla no es «la institución primero»**: es el ORDEN DE LOS `?` EN LA
    // CADENA. Lo escribí como «la institución primero» al arreglar esto, y en
    // `indicadores` —donde el filtro se concatena al final— esa regla volvió a
    // dar ceros. Los dos módulos atan al revés y los dos están bien.
    let mut q = sqlx::query_scalar::<_, i64>(sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    for v in valores {
        q = q.bind(*v);
    }
    q.fetch_one(pool).await
}

/// El censo del día. `fecha` en `AAAA-MM-DD`, hora LOCAL.
pub async fn del_dia(
    pool: &crate::Pool,
    ambito: Ambito,
    fecha: &str,
) -> Result<Option<Censo>, sqlx::Error> {
    let Some(inst) = filtro(ambito) else {
        return Ok(None);
    };
    // Una fecha ilegible NO se sustituye por hoy: se responde que no se puede
    // (directiva 20). Un censo del día equivocado se firma igual.
    let Some((ini, fin)) = limites_del_dia(fecha) else {
        return Ok(None);
    };

    let dentro = if inst.is_some() {
        " AND p.institucion_id = ?"
    } else {
        ""
    };
    let i_ing = instante("h.fecha_ingreso");
    let i_egr = instante("h.fecha_egreso");

    // Los CUATRO números, cada uno con su consulta. Ninguno se deriva de otro:
    // ahí está la comprobación.
    let base = format!("FROM hospitalizacion h JOIN paciente p ON p.id = h.paciente_id WHERE 1=1{dentro}");

    let existencia_inicial = contar(
        pool,
        &format!("SELECT COUNT(*) {base} AND {i_ing} < ? AND ({i_egr} IS NULL OR {i_egr} >= ?)"),
        inst,
        &[&ini, &ini],
    )
    .await?;

    let ingresos = contar(
        pool,
        &format!("SELECT COUNT(*) {base} AND {i_ing} >= ? AND {i_ing} < ?"),
        inst,
        &[&ini, &fin],
    )
    .await?;

    let cuenta_egresos = |estado: Option<&'static str>| {
        let cond = match estado {
            Some(_) => " AND h.estado = ?",
            None => " AND (h.estado IS NULL OR h.estado NOT IN \
                     ('ALTA_MEDICA','ALTA_VOLUNTARIA','REMISION','DEFUNCION'))",
        };
        format!("SELECT COUNT(*) {base} AND {i_egr} >= ? AND {i_egr} < ?{cond}")
    };

    let mut por_tipo = [0_i64; 4];
    for (n, estado) in ["ALTA_MEDICA", "ALTA_VOLUNTARIA", "REMISION", "DEFUNCION"]
        .iter()
        .enumerate()
    {
        por_tipo[n] = contar(pool, &cuenta_egresos(Some(estado)), inst, &[&ini, &fin, estado]).await?;
    }
    let sin_clasificar = contar(pool, &cuenta_egresos(None), inst, &[&ini, &fin]).await?;

    let egresos = EgresosPorTipo {
        alta_medica: por_tipo[0],
        alta_voluntaria: por_tipo[1],
        remision: por_tipo[2],
        defuncion: por_tipo[3],
        sin_clasificar,
    };

    let existencia_final = contar(
        pool,
        &format!("SELECT COUNT(*) {base} AND {i_ing} < ? AND ({i_egr} IS NULL OR {i_egr} >= ?)"),
        inst,
        &[&fin, &fin],
    )
    .await?;

    // La identidad del censo. Se comprueba, no se impone.
    let mut descuadres = Vec::new();
    let esperada = existencia_inicial + ingresos - egresos.total();
    if esperada != existencia_final {
        descuadres.push(format!(
            "el censo no cuadra: {existencia_inicial} inicial + {ingresos} ingresos − {} egresos \
             = {esperada}, pero quedan {existencia_final}. Suele ser una hospitalización con el \
             egreso anterior al ingreso, o una fecha que no se puede leer.",
            egresos.total()
        ));
    }
    if egresos.sin_clasificar > 0 {
        descuadres.push(format!(
            "{} egreso(s) del día con un estado que no es ninguno de los cuatro del producto: \
             se cuentan aparte para que se vean, no se reparten.",
            egresos.sin_clasificar
        ));
    }

    // Las camas y su ocupante ACTUAL. Es la foto de ahora, no la del día pedido:
    // así se dice en el nombre del campo y en la pantalla, porque una cama no
    // guarda su historia — quien quiera la de ayer mira los cuatro números.
    let mut sql_camas = String::from(
        "SELECT c.id AS cama_id, c.numero, c.piso, c.servicio, c.estado, \
         h.id AS hospitalizacion_id, h.paciente_id, \
         (p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || \
          COALESCE(' '||p.apellido2,'')) AS paciente, \
         p.numero_documento AS identificacion, h.fecha_ingreso \
         FROM cama c \
         LEFT JOIN hospitalizacion h ON h.cama_id = c.id AND h.fecha_egreso IS NULL \
         LEFT JOIN paciente p ON p.id = h.paciente_id",
    );
    if inst.is_some() {
        sql_camas.push_str(" WHERE c.institucion_id = ?");
    }
    sql_camas.push_str(" ORDER BY c.piso, c.servicio, c.numero");
    let mut q = sqlx::query_as::<_, CamaDelCenso>(&sql_camas);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    let camas = q.fetch_all(pool).await?;

    let camas_totales = camas.len() as i64;
    let camas_ocupadas = camas.iter().filter(|c| c.hospitalizacion_id.is_some()).count() as i64;
    let ocupacion = (camas_totales > 0)
        .then(|| camas_ocupadas as f64 * 100.0 / camas_totales as f64);

    Ok(Some(Censo {
        fecha: fecha.to_string(),
        existencia_inicial,
        ingresos,
        egresos,
        existencia_final,
        camas,
        camas_totales,
        camas_ocupadas,
        ocupacion,
        descuadres,
    }))
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
                    (11,'CC','3','Caro','Diaz','1970-01-01','F',1), \
                    (12,'CC','4','Dario','Sol','1960-01-01','M',1), \
                    (20,'CC','2','Beto','Paz','1985-01-01','M',2)",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO cama (id,numero,piso,servicio,estado,institucion_id) \
             VALUES (1,'101','1','MEDICINA','OCUPADA',1), \
                    (2,'102','1','MEDICINA','DISPONIBLE',1), \
                    (3,'201','2','CIRUGIA','DISPONIBLE',1), \
                    (9,'A1','1','MEDICINA','DISPONIBLE',2)",
        )
        .execute(&pool).await.unwrap();
        pool
    }

    async fn hosp(pool: &crate::Pool, id: i64, paciente: i64, cama: i64, ing: &str, egr: Option<&str>, estado: &str) {
        sqlx::query(
            "INSERT INTO hospitalizacion (id,paciente_id,cama_id,medico_tratante_id,\
             fecha_ingreso,fecha_egreso,estado) VALUES (?,?,?,6,?,?,?)",
        )
        .bind(id).bind(paciente).bind(cama).bind(ing).bind(egr).bind(estado)
        .execute(pool).await.unwrap();
    }

    /// El caso normal, y la ecuación en verde: uno venía de antes, uno entra y
    /// uno sale.
    #[tokio::test]
    async fn la_ecuacion_del_censo_cuadra() {
        let pool = base().await;
        // Venía de antes y sigue.
        hosp(&pool, 1, 10, 1, "2026-08-14T10:00:00Z", None, "ACTIVA").await;
        // Venía de antes y sale HOY.
        hosp(&pool, 2, 11, 2, "2026-08-14T10:00:00Z", Some("2026-08-16T15:00:00Z"), "ALTA_MEDICA").await;
        // Entra HOY y sigue.
        hosp(&pool, 3, 12, 3, "2026-08-16T09:00:00Z", None, "ACTIVA").await;

        let c = del_dia(&pool, Ambito::Institucion(1), "2026-08-16").await.unwrap().unwrap();
        assert_eq!(c.existencia_inicial, 2);
        assert_eq!(c.ingresos, 1);
        assert_eq!(c.egresos.alta_medica, 1);
        assert_eq!(c.egresos.total(), 1);
        assert_eq!(c.existencia_final, 2);
        assert!(c.descuadres.is_empty(), "tenía que cuadrar: {:?}", c.descuadres);
    }

    /// Quien entra y sale el MISMO día cuenta en ingresos y en egresos, y en
    /// ninguna existencia. La ecuación tiene que seguir cuadrando — es el caso
    /// que rompe los censos escritos a ojo.
    #[tokio::test]
    async fn el_que_entra_y_sale_el_mismo_dia_no_descuadra() {
        let pool = base().await;
        hosp(&pool, 1, 10, 1, "2026-08-16T08:00:00Z", Some("2026-08-16T20:00:00Z"), "ALTA_MEDICA").await;
        let c = del_dia(&pool, Ambito::Institucion(1), "2026-08-16").await.unwrap().unwrap();
        assert_eq!(c.existencia_inicial, 0);
        assert_eq!(c.ingresos, 1);
        assert_eq!(c.egresos.total(), 1);
        assert_eq!(c.existencia_final, 0);
        assert!(c.descuadres.is_empty());
    }

    /// **El caso rojo de la ecuación.** Un egreso ANTERIOR a su ingreso rompe la
    /// identidad, y el censo tiene que DECIRLO en vez de imprimir un número
    /// plausible. Sin este caso, `descuadres` podría estar siempre vacío y las
    /// pruebas de arriba pasarían igual.
    ///
    /// **El caso rojo costó encontrarlo, y eso dice algo bueno de la identidad.**
    /// El primero que escribí —egresar a las 08:00 habiendo ingresado a las
    /// 20:00 del MISMO día— no la rompe: los dos instantes caen dentro de la
    /// ventana, así que suma un ingreso y un egreso y vuelve a cuadrar. La
    /// ecuación solo se rompe cuando el egreso cae FUERA del día por abajo:
    /// ingresó hoy y consta que salió ayer, que es la errata de fecha real.
    #[tokio::test]
    async fn el_dato_roto_descuadra_y_se_dice() {
        let pool = base().await;
        // Ingresa HOY y consta que egresó AYER. En la base cabe.
        hosp(&pool, 1, 10, 1, "2026-08-16T09:00:00Z", Some("2026-08-15T09:00:00Z"), "ALTA_MEDICA").await;
        let c = del_dia(&pool, Ambito::Institucion(1), "2026-08-16").await.unwrap().unwrap();
        assert!(
            !c.descuadres.is_empty(),
            "un egreso anterior a su ingreso tiene que descuadrar el censo"
        );
        assert!(c.descuadres[0].contains("no cuadra"));
    }

    /// Un egreso con estado desconocido se cuenta APARTE y se avisa. Repartirlo
    /// entre los cuatro conocidos escondería el dato roto.
    /// Un egreso cuyo estado no dice a qué egreso corresponde se cuenta APARTE.
    ///
    /// El estado tiene `CHECK` con los cinco valores del producto, así que
    /// «SE_FUE» ni siquiera entra —lo comprobé intentándolo—. El caso que SÍ
    /// ocurre es el contradictorio: una fila con fecha de egreso y estado
    /// `ACTIVA`, que la base admite porque las dos columnas no se vigilan entre
    /// sí. Repartirlo entre los cuatro conocidos escondería el dato roto.
    #[tokio::test]
    async fn el_egreso_con_estado_desconocido_se_cuenta_aparte() {
        let pool = base().await;
        hosp(&pool, 1, 10, 1, "2026-08-15T10:00:00Z", Some("2026-08-16T10:00:00Z"), "ACTIVA").await;
        let c = del_dia(&pool, Ambito::Institucion(1), "2026-08-16").await.unwrap().unwrap();
        assert_eq!(c.egresos.sin_clasificar, 1);
        assert_eq!(c.egresos.alta_medica, 0, "no se reparte al que más se le parece");
        assert_eq!(c.egresos.total(), 1, "pero SÍ cuenta en el total, o la ecuación mentiría");
        assert!(c.descuadres.iter().any(|d| d.contains("estado")));
    }

    #[tokio::test]
    async fn una_institucion_no_ve_el_censo_de_la_otra() {
        let pool = base().await;
        hosp(&pool, 1, 10, 1, "2026-08-16T09:00:00Z", None, "ACTIVA").await;
        hosp(&pool, 2, 20, 9, "2026-08-16T09:00:00Z", None, "ACTIVA").await;
        let uno = del_dia(&pool, Ambito::Institucion(1), "2026-08-16").await.unwrap().unwrap();
        let dos = del_dia(&pool, Ambito::Institucion(2), "2026-08-16").await.unwrap().unwrap();
        assert_eq!(uno.ingresos, 1);
        assert_eq!(dos.ingresos, 1, "cada una ve el suyo");
        assert_eq!(uno.camas_totales, 3, "y solo sus camas");
        assert_eq!(dos.camas_totales, 1);
        assert!(del_dia(&pool, Ambito::Ninguno, "2026-08-16").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn la_ocupacion_sale_de_las_camas_y_no_divide_entre_cero() {
        let pool = base().await;
        hosp(&pool, 1, 10, 1, "2026-08-16T09:00:00Z", None, "ACTIVA").await;
        let c = del_dia(&pool, Ambito::Institucion(1), "2026-08-16").await.unwrap().unwrap();
        assert_eq!(c.camas_totales, 3);
        assert_eq!(c.camas_ocupadas, 1);
        assert!((c.ocupacion.unwrap() - 33.333).abs() < 0.01);
        // La cama ocupada trae a su paciente; las libres no traen a nadie.
        let ocupada = c.camas.iter().find(|x| x.cama_id == 1).unwrap();
        assert_eq!(ocupada.paciente.as_deref(), Some("Ana Ruiz"));
        let libre = c.camas.iter().find(|x| x.cama_id == 2).unwrap();
        assert!(libre.paciente.is_none());
    }

    /// Sin camas no hay porcentaje, y no hay `NaN`: un informe con `NaN` en la
    /// ocupación se imprime igual de bien que uno correcto.
    #[tokio::test]
    async fn sin_camas_la_ocupacion_es_none_y_no_nan() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO institucion (id,nombre,nit,codigo_habilitacion) \
             VALUES (1,'Uno','900123456','050010123401')",
        )
        .execute(&pool).await.unwrap();
        let c = del_dia(&pool, Ambito::Institucion(1), "2026-08-16").await.unwrap().unwrap();
        assert_eq!(c.camas_totales, 0);
        assert!(c.ocupacion.is_none());
    }

    /// Una fecha ilegible responde que NO SE PUEDE, en vez de dar el censo de
    /// hoy. Un censo del día equivocado se firma igual que el bueno.
    #[tokio::test]
    async fn la_fecha_ilegible_no_se_sustituye_por_hoy() {
        let pool = base().await;
        assert!(del_dia(&pool, Ambito::Institucion(1), "el martes").await.unwrap().is_none());
        // Positivo de control: la buena sí responde.
        assert!(del_dia(&pool, Ambito::Institucion(1), "2026-08-16").await.unwrap().is_some());
    }
}
