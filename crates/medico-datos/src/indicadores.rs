//! Indicadores de calidad (#249). **El cuarto módulo sin tabla.**
//!
//! Van cuatro derivados —`soportecobertura`, `KARDEX`, `CENSO_DIARIO` y éste— y
//! el patrón ya no es una casualidad: cuando un «módulo que falta» es un INFORME
//! sobre datos que ya existen, la tabla nueva es la respuesta equivocada.
//!
//! # Lo que este módulo dice y lo que se NIEGA a decir
//!
//! Un tablero de indicadores es el sitio donde más barato sale inventar. Cada
//! número tiene una fórmula con un denominador, y si el denominador no está, sale
//! igual de bonito con el número equivocado — y nadie lo revisa, porque un
//! porcentaje se lee y se cree.
//!
//! Por eso cada indicador viene con su **numerador, su denominador y cuántos
//! registros quedaron FUERA por datos incompletos**, y los que no se pueden
//! calcular se devuelven en [`Indicadores::no_calculables`] diciendo qué falta.
//! Un tablero con cuatro números y una lista de tres cosas que no se pueden medir
//! es más útil que uno con siete números de los que tres son mentira.
//!
//! # La oportunidad de la consulta obligó a añadir una columna
//!
//! Los días entre que el paciente pide la cita y el día que lo atienden es de los
//! indicadores que más se reportan, y **no se podía calcular**: `cita` guardaba
//! la fecha de la CITA y no la de la SOLICITUD (directiva 32 — son dos datos). Se
//! añadió `cita.fecha_solicitud` en la 0067, nulable, y las citas anteriores se
//! quedan sin él a propósito: el indicador dice sobre cuántas pudo calcularse.
//!
//! Rellenarlas con la fecha de la cita habría dado **oportunidad cero para todo
//! el histórico** — el número más halagador y más falso posible.

use medico_dominio::Ambito;
use serde::Serialize;

/// Un indicador con lo que hace falta para poder discutirlo.
#[derive(Debug, Clone, Serialize)]
pub struct Indicador {
    pub nombre: String,
    /// La fórmula, escrita. Un indicador sin su fórmula a la vista es un número
    /// que cada quien interpreta como quiere.
    pub formula: String,
    pub numerador: i64,
    pub denominador: i64,
    /// `None` cuando el denominador es cero: no hay tasa, y un `0 %` diría que
    /// se midió y salió bien.
    pub valor: Option<f64>,
    /// Registros que existían pero no pudieron entrar por datos incompletos.
    /// Cero es lo normal; distinto de cero es lo que hay que mirar.
    pub excluidos: i64,
    pub unidad: String,
}

impl Indicador {
    fn tasa(nombre: &str, formula: &str, num: i64, den: i64, excluidos: i64) -> Self {
        Indicador {
            nombre: nombre.into(),
            formula: formula.into(),
            numerador: num,
            denominador: den,
            valor: (den > 0).then(|| num as f64 * 100.0 / den as f64),
            excluidos,
            unidad: "%".into(),
        }
    }
}

/// Lo que NO se puede calcular, con la razón. Es la mitad del valor del módulo.
#[derive(Debug, Clone, Serialize)]
pub struct NoCalculable {
    pub nombre: String,
    /// Qué falta, nombrado: la columna, la tabla o el módulo. Un «no disponible»
    /// sin decir qué falta no se puede arreglar.
    pub falta: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Indicadores {
    pub desde: String,
    pub hasta: String,
    pub indicadores: Vec<Indicador>,
    pub no_calculables: Vec<NoCalculable>,
}

fn institucion(ambito: Ambito) -> Option<Option<i64>> {
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
    // LOS VALORES PRIMERO Y LA INSTITUCIÓN AL FINAL, porque ése es el orden en
    // que aparecen los `?` en estas consultas: el filtro de institución se
    // concatena DESPUÉS del `BETWEEN`.
    //
    // La regla NO es «la institución primero», que es como la escribí al
    // arreglar el censo (0064) y es falsa: allí el filtro iba pegado al `WHERE`
    // y aquí va al final. La regla es **el orden de los `?` en la cadena**, y
    // por eso los dos casos se atan al revés. Equivocarse no da error: da CERO,
    // que en un tablero de indicadores es un hospital sin muertes.
    let mut q = sqlx::query_scalar::<_, i64>(sql);
    for v in valores {
        q = q.bind(*v);
    }
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_one(pool).await
}

/// Los indicadores del periodo, con lo que no se puede medir.
pub async fn del_periodo(
    pool: &crate::Pool,
    ambito: Ambito,
    desde: &str,
    hasta: &str,
) -> Result<Option<Indicadores>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(None);
    };
    let mut indicadores = Vec::new();
    let mut no_calculables = Vec::new();

    // ── Mortalidad hospitalaria ──────────────────────────────────────────────
    let filtro_h = if inst.is_some() {
        " AND p.institucion_id = ?"
    } else {
        ""
    };
    let egresos = contar(
        pool,
        &format!(
            "SELECT COUNT(*) FROM hospitalizacion h JOIN paciente p ON p.id = h.paciente_id \
             WHERE h.fecha_egreso IS NOT NULL AND date(h.fecha_egreso) BETWEEN date(?) AND date(?){filtro_h}"
        ),
        inst,
        &[desde, hasta],
    )
    .await?;
    let muertes = contar(
        pool,
        &format!(
            "SELECT COUNT(*) FROM hospitalizacion h JOIN paciente p ON p.id = h.paciente_id \
             WHERE h.fecha_egreso IS NOT NULL AND date(h.fecha_egreso) BETWEEN date(?) AND date(?) \
             AND h.estado = 'DEFUNCION'{filtro_h}"
        ),
        inst,
        &[desde, hasta],
    )
    .await?;
    indicadores.push(Indicador::tasa(
        "Mortalidad hospitalaria",
        "defunciones ÷ egresos del periodo × 100",
        muertes,
        egresos,
        0,
    ));

    // ── Infección asociada a la atención ─────────────────────────────────────
    let iaas = contar(
        pool,
        &format!(
            "SELECT COUNT(*) FROM evento_iaas v JOIN paciente p ON p.id = v.paciente_id \
             WHERE date(v.fecha_deteccion) BETWEEN date(?) AND date(?){filtro_h}"
        ),
        inst,
        &[desde, hasta],
    )
    .await?;
    indicadores.push(Indicador::tasa(
        "Infección asociada a la atención (IAAS)",
        "eventos IAAS ÷ egresos del periodo × 100",
        iaas,
        egresos,
        0,
    ));

    // ── Cancelación de cirugía programada ────────────────────────────────────
    let filtro_q = if inst.is_some() {
        " AND p.institucion_id = ?"
    } else {
        ""
    };
    let programadas = contar(
        pool,
        &format!(
            "SELECT COUNT(*) FROM programacion_quirurgica g JOIN paciente p ON p.id = g.paciente_id \
             WHERE date(g.fecha_programada) BETWEEN date(?) AND date(?){filtro_q}"
        ),
        inst,
        &[desde, hasta],
    )
    .await?;
    let canceladas = contar(
        pool,
        &format!(
            "SELECT COUNT(*) FROM programacion_quirurgica g JOIN paciente p ON p.id = g.paciente_id \
             WHERE date(g.fecha_programada) BETWEEN date(?) AND date(?) \
             AND g.estado = 'CANCELADA'{filtro_q}"
        ),
        inst,
        &[desde, hasta],
    )
    .await?;
    indicadores.push(Indicador::tasa(
        "Cancelación de cirugía programada",
        "canceladas ÷ programadas del periodo × 100",
        canceladas,
        programadas,
        0,
    ));

    // ── Oportunidad de la consulta ───────────────────────────────────────────
    //
    // Aquí está el ejemplo de por qué `excluidos` existe: las citas anteriores a
    // la 0067 no tienen fecha de solicitud, y entran en el conteo de EXCLUIDAS,
    // no en el promedio.
    let filtro_c = if inst.is_some() {
        " AND c.institucion_id = ?"
    } else {
        ""
    };
    let con_solicitud = contar(
        pool,
        &format!(
            "SELECT COUNT(*) FROM cita c WHERE date(c.fecha) BETWEEN date(?) AND date(?) \
             AND c.fecha_solicitud IS NOT NULL{filtro_c}"
        ),
        inst,
        &[desde, hasta],
    )
    .await?;
    let sin_solicitud = contar(
        pool,
        &format!(
            "SELECT COUNT(*) FROM cita c WHERE date(c.fecha) BETWEEN date(?) AND date(?) \
             AND c.fecha_solicitud IS NULL{filtro_c}"
        ),
        inst,
        &[desde, hasta],
    )
    .await?;
    let dias: i64 = {
        let sql = format!(
            // `0.0` y no `0`: con `0` el COALESCE devuelve INTEGER cuando no hay
            // filas y sqlx rechaza el decode a f64. Un detalle que solo aparece
            // en el caso vacío, o sea justo el que menos se prueba.
            "SELECT COALESCE(SUM(julianday(date(c.fecha)) - julianday(date(c.fecha_solicitud))), 0.0) \
             FROM cita c WHERE date(c.fecha) BETWEEN date(?) AND date(?) \
             AND c.fecha_solicitud IS NOT NULL{filtro_c}"
        );
        // Mismo orden que en `contar`: los dos `?` de las fechas y luego el de
        // la institución, que va al final de la cadena.
        let mut q = sqlx::query_scalar::<_, f64>(&sql).bind(desde).bind(hasta);
        if let Some(i) = inst {
            q = q.bind(i);
        }
        q.fetch_one(pool).await? as i64
    };
    indicadores.push(Indicador {
        nombre: "Oportunidad de la consulta".into(),
        formula: "suma de días entre solicitud y cita ÷ citas con fecha de solicitud".into(),
        numerador: dias,
        denominador: con_solicitud,
        valor: (con_solicitud > 0).then(|| dias as f64 / con_solicitud as f64),
        // Las citas sin fecha de solicitud NO entran en el promedio y se cuentan
        // aquí. Meterlas con cero días daría oportunidad inmediata para todo el
        // histórico, que es el número más halagador y más falso posible.
        excluidos: sin_solicitud,
        unidad: "días".into(),
    });

    // ── Y lo que este producto todavía no puede medir ────────────────────────
    no_calculables.push(NoCalculable {
        nombre: "Satisfacción del usuario".into(),
        falta: "no hay encuesta de satisfacción en el producto: no existe tabla ni captura"
            .into(),
    });
    no_calculables.push(NoCalculable {
        nombre: "Reingreso antes de 15 días".into(),
        falta: "se puede derivar de `hospitalizacion`, pero exige decidir qué cuenta como \
                reingreso POR LA MISMA CAUSA — compararlo por diagnóstico de egreso daría un \
                número que nadie ha acordado. Es decisión de producto, no de código"
            .into(),
    });
    no_calculables.push(NoCalculable {
        nombre: "Oportunidad en cirugía programada".into(),
        falta: "`programacion_quirurgica` guarda la fecha PROGRAMADA y no la de la solicitud \
                quirúrgica; es el mismo hueco que tenía `cita` antes de la 0067"
            .into(),
    });

    Ok(Some(Indicadores {
        desde: desde.to_string(),
        hasta: hasta.to_string(),
        indicadores,
        no_calculables,
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
                    (20,'CC','2','Beto','Paz','1985-01-01','M',2)",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO cama (id,numero,piso,servicio,estado,institucion_id) \
             VALUES (1,'101','1','MEDICINA','DISPONIBLE',1)",
        )
        .execute(&pool).await.unwrap();
        pool
    }

    async fn egreso(pool: &crate::Pool, id: i64, paciente: i64, estado: &str) {
        sqlx::query(
            "INSERT INTO hospitalizacion (id,paciente_id,cama_id,medico_tratante_id,\
             fecha_ingreso,fecha_egreso,estado) \
             VALUES (?,?,1,6,'2026-08-01T10:00:00Z','2026-08-10T10:00:00Z',?)",
        )
        .bind(id).bind(paciente).bind(estado)
        .execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn la_mortalidad_sale_de_los_egresos_del_periodo() {
        let pool = base().await;
        egreso(&pool, 1, 10, "ALTA_MEDICA").await;
        egreso(&pool, 2, 10, "DEFUNCION").await;
        egreso(&pool, 3, 10, "ALTA_MEDICA").await;
        egreso(&pool, 4, 10, "REMISION").await;
        let r = del_periodo(&pool, Ambito::Institucion(1), "2026-08-01", "2026-08-31")
            .await.unwrap().unwrap();
        let m = r.indicadores.iter().find(|i| i.nombre.starts_with("Mortalidad")).unwrap();
        assert_eq!(m.numerador, 1);
        assert_eq!(m.denominador, 4);
        assert_eq!(m.valor, Some(25.0));
    }

    /// Sin egresos NO hay tasa, y no sale `0 %`: un cero se lee como «se midió y
    /// salió perfecto», que es lo contrario de «no había nada que medir».
    #[tokio::test]
    async fn sin_denominador_no_hay_tasa_y_no_es_cero() {
        let pool = base().await;
        let r = del_periodo(&pool, Ambito::Institucion(1), "2026-08-01", "2026-08-31")
            .await.unwrap().unwrap();
        let m = r.indicadores.iter().find(|i| i.nombre.starts_with("Mortalidad")).unwrap();
        assert_eq!(m.denominador, 0);
        assert!(m.valor.is_none(), "sin denominador no hay porcentaje");
    }

    /// La oportunidad se calcula SOLO sobre las citas que tienen fecha de
    /// solicitud, y **dice cuántas quedaron fuera**. Es el caso que justifica el
    /// campo `excluidos` del módulo entero.
    #[tokio::test]
    async fn la_oportunidad_excluye_las_citas_sin_fecha_de_solicitud_y_lo_dice() {
        let pool = base().await;
        // Dos con solicitud: 3 y 5 días de espera.
        sqlx::query(
            "INSERT INTO cita (institucion_id,paciente_id,fecha,hora,fecha_solicitud) \
             VALUES (1,10,'2026-08-10','08:00','2026-08-07'), \
                    (1,10,'2026-08-15','08:00','2026-08-10')",
        )
        .execute(&pool).await.unwrap();
        // Una SIN solicitud: es del histórico anterior a la 0067.
        sqlx::query(
            "INSERT INTO cita (institucion_id,paciente_id,fecha,hora) \
             VALUES (1,10,'2026-08-20','08:00')",
        )
        .execute(&pool).await.unwrap();

        let r = del_periodo(&pool, Ambito::Institucion(1), "2026-08-01", "2026-08-31")
            .await.unwrap().unwrap();
        let o = r.indicadores.iter().find(|i| i.nombre.starts_with("Oportunidad")).unwrap();
        assert_eq!(o.denominador, 2, "solo las que tienen con qué calcularse");
        assert_eq!(o.numerador, 8, "3 + 5 días");
        assert_eq!(o.valor, Some(4.0));
        assert_eq!(o.excluidos, 1, "y la tercera se DICE, no se esconde ni se rellena");
    }

    /// El aislamiento. Con el `bind` del censo mal atado esto daba cero sin
    /// error, así que va con su positivo de control.
    #[tokio::test]
    async fn cada_institucion_ve_solo_lo_suyo() {
        let pool = base().await;
        egreso(&pool, 1, 10, "DEFUNCION").await;
        egreso(&pool, 2, 20, "DEFUNCION").await;
        let uno = del_periodo(&pool, Ambito::Institucion(1), "2026-08-01", "2026-08-31")
            .await.unwrap().unwrap();
        let dos = del_periodo(&pool, Ambito::Institucion(2), "2026-08-01", "2026-08-31")
            .await.unwrap().unwrap();
        assert_eq!(uno.indicadores[0].denominador, 1);
        assert_eq!(dos.indicadores[0].denominador, 1, "cada una la suya, y no las dos");
        assert!(del_periodo(&pool, Ambito::Ninguno, "2026-08-01", "2026-08-31").await.unwrap().is_none());
    }

    /// Lo que NO se puede medir viene con su razón NOMBRADA. Un «no disponible»
    /// sin decir qué falta no se puede arreglar.
    #[tokio::test]
    async fn lo_que_no_se_puede_medir_dice_que_le_falta() {
        let pool = base().await;
        let r = del_periodo(&pool, Ambito::Institucion(1), "2026-08-01", "2026-08-31")
            .await.unwrap().unwrap();
        assert!(!r.no_calculables.is_empty());
        for n in &r.no_calculables {
            assert!(!n.falta.trim().is_empty(), "«{}» no dice qué le falta", n.nombre);
            assert!(n.falta.len() > 20, "«{}» lo dice demasiado corto para servir", n.nombre);
        }
    }

    /// Todo indicador lleva su fórmula escrita: un número sin fórmula lo
    /// interpreta cada quien como quiere.
    #[tokio::test]
    async fn todo_indicador_lleva_su_formula() {
        let pool = base().await;
        let r = del_periodo(&pool, Ambito::Institucion(1), "2026-08-01", "2026-08-31")
            .await.unwrap().unwrap();
        assert_eq!(r.indicadores.len(), 4);
        for i in &r.indicadores {
            assert!(!i.formula.trim().is_empty(), "«{}» sin fórmula", i.nombre);
        }
    }
}
