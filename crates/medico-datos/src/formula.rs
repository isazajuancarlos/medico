//! Fórmulas (prescripciones) de una historia. Aislamiento por la cadena
//! historia → paciente → institución: quien no ve la historia no toca sus fórmulas.

use medico_dominio::procedencia::Procedencia;
use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Formula {
    pub id: i64,
    pub historia_id: i64,
    pub medicamento: String,
    pub codigo_cum: Option<String>,
    pub dosis: String,
    pub cantidad: f64,
    pub unidad: Option<String>,
    pub dias_tratamiento: Option<i64>,
    pub observaciones: Option<String>,
    pub fecha_formula: String,
    pub dispensada: bool,
    /// #84: `{campo: procedencia}` de los campos RIPS de ESTA fórmula, o `NULL`
    /// si la fila es anterior a la captura de procedencia (entonces el
    /// recolector la salta en vez de marcarla entera como sin respaldo).
    pub procedencias: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormulaNueva {
    pub medicamento: String,
    pub codigo_cum: Option<String>,
    pub dosis: String,
    /// La cantidad se valida **en la frontera de `serde`**, no en cada
    /// manejador: una cantidad imposible no llega a existir como `FormulaNueva`.
    ///
    /// Lo trajo la revisión de la directiva 25 sobre `1c570350`: el PANEL había
    /// ganado su guarda (`is_finite() && n > 0.0`) y las dos rutas JSON
    /// —`POST /api/historias/{id}/formulas` y `PUT /api/formulas/{id}`— no
    /// validaban NADA. `cantidad: -5` es entera, así que `cantidad_entera` la
    /// emitía al AM por el camino corto **con `rec.presente()`**: una cantidad
    /// negativa en lo que se le cobra a la EPS, auditada como si alguien la
    /// hubiera afirmado.
    ///
    /// Se arregla aquí y no en las dos rutas porque una guarda repetida en cada
    /// puerta es una puerta nueva esperando a que se olvide (directiva 1: quitar
    /// la causa, no contener el síntoma). `axum` responde 422 solo, con el campo
    /// nombrado, y una puerta futura nace protegida.
    #[serde(deserialize_with = "cantidad_dispensable")]
    pub cantidad: f64,
    pub unidad: Option<String>,
    /// Los días de tratamiento viajan al AM como `numDiasTratamiento`, así que
    /// se validan igual que la cantidad y en el mismo sitio, no en cada puerta.
    ///
    /// Lo trajo la segunda ronda de la directiva 25 sobre `b42033aa`: es el
    /// campo de al lado, en la misma estructura y en la misma
    /// `CAMPOS_RIPS_FORMULA`, y **ninguna de las tres puertas lo comprobaba**.
    /// Por el panel se grababa `-30` con la casilla marcada, y como
    /// `procedencias_del_formulario` solo mira `is_some()`, salía al archivo
    /// certificado como afirmado por un humano y sin una sola falta.
    ///
    /// **El cero SÍ es legítimo** —una dosis única no dura días— así que lo que
    /// se rechaza es el negativo, y nada más (directiva 21: se rechaza lo
    /// imposible, no lo inusual).
    #[serde(default, deserialize_with = "dias_de_tratamiento")]
    pub dias_tratamiento: Option<i64>,
    pub observaciones: Option<String>,
}

/// Días de tratamiento: ausente, o un número que no sea negativo.
fn dias_de_tratamiento<'de, D>(d: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let n = Option::<i64>::deserialize(d)?;
    if n.is_some_and(|v| v < 0) {
        return Err(D::Error::custom(
            "los días de tratamiento no pueden ser negativos",
        ));
    }
    Ok(n)
}

/// Un número finito y mayor que cero. Nada más.
///
/// `serde_json` ya rechaza `inf` y `NaN` en el JSON de entrada (no son literales
/// válidos), así que lo que esta guarda añade de verdad es el **cero y los
/// negativos** — y `is_finite()` de todos modos, porque esta estructura también
/// se deserializa desde otros formatos y una guarda que dependa de cuál sea el
/// códec no es una guarda.
fn cantidad_dispensable<'de, D>(d: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let n = f64::deserialize(d)?;
    if !n.is_finite() || n <= 0.0 {
        return Err(D::Error::custom(
            "la cantidad tiene que ser un número finito mayor que cero",
        ));
    }
    Ok(n)
}

const COLUMNAS: &str = "id, historia_id, medicamento, codigo_cum, dosis, cantidad, unidad, \
                        dias_tratamiento, observaciones, fecha_formula, dispensada, procedencias";

/// Serializa el mapa de procedencia (#84) para la columna. **El mapa vacío da
/// `{}`, NO `NULL`, y esa distinción es el mecanismo entero.**
///
/// La primera versión devolvía `None` con el mapa vacío, y con eso dejaba inerte
/// justo el caso que #84 existe para cazar. La cadena se cerraba sola: los cuatro
/// campos RIPS de la fórmula son de fricción DURA, así que sin el toque humano
/// `procedencias_de_captura` los omite TODOS → mapa vacío → `NULL` →
/// `Recolector::en_registro(None)` → la auditoría **salta el medicamento entero**.
/// O sea que una fórmula sin confirmar —el estado por defecto de una casilla— no
/// producía ni una anotación, mientras la migración 0060 prometía por escrito que
/// «el cierre del mes lo anota como sin respaldo». Lo cazó la revisión de la
/// directiva 25; mis pruebas no, porque afirmaban sobre la COLUMNA y no sobre la
/// alarma (directiva 33).
///
/// La causa de fondo es la directiva 32: `NULL` cargaba dos significados que no
/// son el mismo —«esta fila es anterior a la captura», que sí debe saltarse, y
/// «hubo captura y nadie afirmó nada», que es exactamente lo que hay que
/// marcar—. Con `{}` el recolector activa la auditoría y cada campo con valor
/// sale `SinRespaldo`; `NULL` queda solo para las filas anteriores a la 0060,
/// que es lo único que nadie reescribe.
///
/// **Es el ÚNICO sitio de este módulo que escribe esa columna, y por eso recibe un
/// mapa TIPADO y no una cadena.** Si `FormulaNueva` llevara el JSON, el cuerpo de
/// `POST /api/formulas` podría afirmar «confirmado» sobre cualquier campo sin que
/// nadie lo mirara: la procedencia dejaría de medir el toque humano y pasaría a
/// medir lo que el cliente dice de sí mismo. Con un `BTreeMap<String, Procedencia>`
/// serializado aquí, un JSON ilegible no puede entrar (directiva 20 al revés: el
/// dato malo no se rechaza tarde, no se puede construir).
fn json_de(procedencias: &BTreeMap<String, Procedencia>) -> Option<String> {
    // Siempre `Some`: escribir la columna es la afirmación «aquí hubo captura».
    serde_json::to_string(procedencias).ok()
}

/// ¿Es visible para el ámbito la historia `historia_id`? (por su paciente)
async fn historia_visible(pool: &crate::Pool, ambito: Ambito, historia_id: i64) -> Result<bool, sqlx::Error> {
    let fila: Option<(i64,)> = sqlx::query_as(
        "SELECT p.institucion_id FROM historia h JOIN paciente p ON p.id = h.paciente_id WHERE h.id = ?",
    )
    .bind(historia_id)
    .fetch_optional(pool)
    .await?;
    Ok(fila.is_some_and(|(i,)| ambito.puede_ver(Some(i)) == Visibilidad::Si))
}

/// ¿Es visible para el ámbito la fórmula `id`? (por su historia → paciente)
async fn formula_visible(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<bool, sqlx::Error> {
    let fila: Option<(i64,)> = sqlx::query_as(
        "SELECT p.institucion_id FROM formula f JOIN historia h ON h.id = f.historia_id \
         JOIN paciente p ON p.id = h.paciente_id WHERE f.id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(fila.is_some_and(|(i,)| ambito.puede_ver(Some(i)) == Visibilidad::Si))
}

pub async fn listar_por_historia(pool: &crate::Pool, ambito: Ambito, historia_id: i64) -> Result<Vec<Formula>, sqlx::Error> {
    if !historia_visible(pool, ambito, historia_id).await? {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, Formula>(&format!("SELECT {COLUMNAS} FROM formula WHERE historia_id = ? ORDER BY id"))
        .bind(historia_id)
        .fetch_all(pool)
        .await
}

/// Crea una fórmula en una historia visible. `None` si la historia no es del ámbito.
/// Cuelga una fórmula de una historia. **Se niega sobre una historia FIRMADA**:
/// añadirle un medicamento es editarla por al lado (ver
/// `historia::admite_escritura`).
///
/// `procedencias` (#84) es el mapa `{campo: procedencia}` que construyó la captura
/// con `procedencias_de_captura`. Vacío = nadie afirmó nada, y la columna queda
/// `{}` — que es lo contrario de lo que aquí decía: el recolector SÍ audita la fila
/// y marca su CUM como sin respaldo. `NULL` queda solo para lo anterior a la 0060.
pub async fn crear(
    pool: &crate::Pool,
    ambito: Ambito,
    historia_id: i64,
    f: &FormulaNueva,
    procedencias: &BTreeMap<String, Procedencia>,
) -> Result<crate::Creacion, sqlx::Error> {
    match crate::historia::admite_escritura(pool, ambito, historia_id).await? {
        crate::Resultado::Hecho => {}
        crate::Resultado::NoExiste => return Ok(crate::Creacion::NoExiste),
        crate::Resultado::Firmada => return Ok(crate::Creacion::Firmada),
    }
    let r = sqlx::query(
        "INSERT INTO formula (historia_id, medicamento, codigo_cum, dosis, cantidad, unidad, \
         dias_tratamiento, observaciones, procedencias) VALUES (?,?,?,?,?,?,?,?,?)",
    )
    .bind(historia_id)
    .bind(&f.medicamento)
    .bind(&f.codigo_cum)
    .bind(&f.dosis)
    .bind(f.cantidad)
    .bind(&f.unidad)
    .bind(f.dias_tratamiento)
    .bind(&f.observaciones)
    .bind(json_de(procedencias))
    .execute(pool)
    .await?;
    Ok(crate::Creacion::Creada(r.last_insert_rowid()))
}

/// Actualiza una fórmula visible. **Se niega sobre una historia FIRMADA.**
///
/// Era la puerta 13, y se quedó fuera cuando se cerraron las doce: `crear` y
/// `eliminar` pasaron por `admite_escritura` y ésta no. La encontró la revisión
/// de seguridad del cierre, y era la peor de las tres — sobre una prescripción,
/// MODIFICAR no cambia si hay medicamento: cambia cuál y cuánto.
///
/// # La procedencia se REESCRIBE entera, y un llamante que no la maneja la BORRA
///
/// No es un descuido: es la única semántica honesta (#84). Cambiar el CUM de una
/// prescripción invalida el aval que alguien dio sobre el CUM anterior — el visto
/// bueno era de aquel valor, no de este campo para siempre. Un llamante que edita
/// los valores y no afirma nada (el `PUT /api/formulas/{id}` de JSON, que no tiene
/// dónde poner el toque) pasa un mapa vacío, la columna queda **`{}`** y el cierre
/// del mes anota el CUM como sin respaldo. Es ruidoso y es correcto: la alternativa
/// —conservar el aval anterior sobre un valor nuevo— es exactamente el dato con la
/// forma correcta que nadie afirmó (directiva 20), pero peor, porque además lleva
/// firma prestada.
///
/// **Decía `NULL` y era falso**: con `NULL` el recolector no anotaba nada, saltaba
/// la fórmula entera. Así, un solo `PUT` con el mismo cuerpo sobre una fórmula ya
/// confirmada la sacaba de la auditoría para siempre, respondiendo `204` y sin
/// cambiar un byte del archivo del RIPS. Lo cazó la revisión de la directiva 25.
/// Ver [`json_de`].
pub async fn actualizar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    f: &FormulaNueva,
    procedencias: &BTreeMap<String, Procedencia>,
) -> Result<crate::Resultado, sqlx::Error> {
    if !formula_visible(pool, ambito, id).await? {
        return Ok(crate::Resultado::NoExiste);
    }
    let padre: Option<i64> = sqlx::query_scalar("SELECT historia_id FROM formula WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    if crate::historia::admite_escritura_de(pool, ambito, padre).await?
        == crate::Resultado::Firmada
    {
        return Ok(crate::Resultado::Firmada);
    }
    let r = sqlx::query(
        "UPDATE formula SET medicamento=?, codigo_cum=?, dosis=?, cantidad=?, unidad=?, \
         dias_tratamiento=?, observaciones=?, procedencias=? WHERE id=?",
    )
    .bind(&f.medicamento)
    .bind(&f.codigo_cum)
    .bind(&f.dosis)
    .bind(f.cantidad)
    .bind(&f.unidad)
    .bind(f.dias_tratamiento)
    .bind(&f.observaciones)
    .bind(json_de(procedencias))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(if r.rows_affected() > 0 { crate::Resultado::Hecho } else { crate::Resultado::NoExiste })
}

/// Qué pasó al borrar una fórmula.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultadoBorrado {
    Hecho,
    /// No existe o no es del ámbito. Los dos casos dan lo mismo a propósito.
    NoExiste,
    /// **Ya se entregó al paciente.** Borrar la prescripción dejaría en el
    /// sistema una entrega de medicamento sin nada que la respalde: la farmacia
    /// habría dispensado «sin fórmula». La constancia de la entrega y su
    /// prescripción se sostienen o caen juntas, y ninguna de las dos se corrige
    /// borrando — se corrige con la fórmula nueva que el médico firme.
    YaDispensada { entregas: i64 },
    /// **La historia ya está firmada.** Un documento firmado no se edita por
    /// ninguna puerta, y quitarle o ponerle una prescripción es editarlo.
    ///
    /// Esta puerta IMPIDE hacerlo desde el producto; que la firma lo DETECTE
    /// cuando se hace contra la base es la otra capa, y también existe desde el
    /// 2026-08-10: `firmable` incluye el resumen de las fórmulas, órdenes y
    /// adjuntos. Antes sellaba solo la fila `historia` y `verificar-firma`
    /// seguía diciendo `Valida` después de borrar una prescripción.
    HistoriaFirmada,
}

/// Elimina una fórmula visible que no se haya dispensado. Ver [`ResultadoBorrado`].
pub async fn eliminar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<ResultadoBorrado, sqlx::Error> {
    if !formula_visible(pool, ambito, id).await? {
        return Ok(ResultadoBorrado::NoExiste);
    }
    // La historia padre manda: si está firmada, esta fórmula es parte de un
    // documento cerrado.
    let padre: Option<i64> = sqlx::query_scalar("SELECT historia_id FROM formula WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    if crate::historia::admite_escritura_de(pool, ambito, padre).await?
        == crate::Resultado::Firmada
    {
        return Ok(ResultadoBorrado::HistoriaFirmada);
    }
    let entregas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM dispensacion WHERE formula_id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    if entregas > 0 {
        return Ok(ResultadoBorrado::YaDispensada { entregas });
    }
    let r = sqlx::query("DELETE FROM formula WHERE id = ?").bind(id).execute(pool).await?;
    Ok(if r.rows_affected() > 0 { ResultadoBorrado::Hecho } else { ResultadoBorrado::NoExiste })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **LOS DÍAS NEGATIVOS TAMPOCO**, y el CERO SÍ, que es la mitad que
    /// importa.
    ///
    /// Lo trajo la segunda ronda de la directiva 25 sobre `b42033aa`: es el
    /// campo de al lado de `cantidad`, en la misma estructura y en la misma
    /// `CAMPOS_RIPS_FORMULA`, viaja al AM como `numDiasTratamiento`, se firma —y
    /// **ninguna de las tres puertas lo comprobaba**.
    ///
    /// El cero es legítimo: una dosis única no dura días. Rechazarlo sería el
    /// sesgo a cerrar (directiva 21) — se rechaza lo imposible, no lo inusual.
    #[test]
    fn los_dias_de_tratamiento_negativos_no_se_deserializan() {
        let cuerpo = |d: &str| {
            format!(
                r#"{{"medicamento":"X","dosis":"1","cantidad":1,"dias_tratamiento":{d}}}"#
            )
        };
        for mala in ["-1", "-30"] {
            assert!(
                serde_json::from_str::<FormulaNueva>(&cuerpo(mala)).is_err(),
                "`dias_tratamiento: {mala}` no puede convertirse en una fórmula"
            );
        }
        for buena in ["0", "1", "30", "null"] {
            let f: FormulaNueva = serde_json::from_str(&cuerpo(buena))
                .unwrap_or_else(|e| panic!("`dias_tratamiento: {buena}` es legítimo: {e}"));
            assert!(f.dias_tratamiento.is_none_or(|d| d >= 0));
        }
        // Y ausente del todo: el campo es opcional.
        let f: FormulaNueva =
            serde_json::from_str(r#"{"medicamento":"X","dosis":"1","cantidad":1}"#).unwrap();
        assert_eq!(f.dias_tratamiento, None);
    }

    /// **LA CANTIDAD IMPOSIBLE NO LLEGA A EXISTIR**, con sus dos listas.
    ///
    /// El caso rojo va por la vía REAL —`serde_json::from_str::<FormulaNueva>`,
    /// que es exactamente lo que hace `axum` con el cuerpo de
    /// `POST /api/historias/{id}/formulas` y `PUT /api/formulas/{id}`— y no por
    /// una función auxiliar inventada para la prueba.
    ///
    /// El `-5` no es un caso de laboratorio: es entero, así que `cantidad_entera`
    /// lo emitía al AM por el camino corto **con `rec.presente()`**, o sea una
    /// cantidad negativa en lo que se le cobra a la EPS y sin una sola anotación
    /// en el informe de cierre.
    #[test]
    fn una_cantidad_que_no_se_puede_dispensar_no_se_deserializa() {
        let cuerpo = |c: &str| {
            format!(r#"{{"medicamento":"Metotrexato","dosis":"1 c/8h","cantidad":{c}}}"#)
        };

        // ROJAS: cero, negativa, y el subnormal que no es dispensable de nada.
        for mala in ["0", "-5", "-0.0", "0.0"] {
            assert!(
                serde_json::from_str::<FormulaNueva>(&cuerpo(mala)).is_err(),
                "`cantidad: {mala}` no puede convertirse en una fórmula"
            );
        }

        // VERDES, la pareja: sin ella pasaría igual una guarda que dijera que no
        // a todo, y con eso no se podría formular nada.
        for buena in ["1", "30", "0.5", "2.5"] {
            let f: FormulaNueva = serde_json::from_str(&cuerpo(buena))
                .unwrap_or_else(|e| panic!("`cantidad: {buena}` es legítima y falló: {e}"));
            assert!(f.cantidad > 0.0);
        }
    }

    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES (5,'Dra','d@x.co','h','medico',1,1)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1),(20,'CC','2','Beto','Paz','1985-01-01','M',2)")
            .execute(&pool).await.unwrap();
        // Una historia en cada institución (id 100 de la 1, id 200 de la 2).
        sqlx::query("INSERT INTO historia (id,codigo_historia,paciente_id,medico_id,motivo_consulta,enfermedad_actual,diagnostico_principal,plan_tratamiento) VALUES (100,'HC1',10,5,'m','e','J00','plan'),(200,'HC2',20,5,'m','e','J00','plan')")
            .execute(&pool).await.unwrap();
        pool
    }

    fn nueva() -> FormulaNueva {
        FormulaNueva {
            medicamento: "Acetaminofén 500mg".into(),
            codigo_cum: Some("19900001-1".into()),
            dosis: "1 cada 8h".into(),
            cantidad: 30.0,
            unidad: Some("tableta".into()),
            dias_tratamiento: Some(10),
            observaciones: None,
        }
    }

    /// La procedencia (#84) viaja a la columna y vuelve, **y su pareja**: sin
    /// mapa la columna queda `{}` —no `NULL`—, que es lo que mantiene la fórmula
    /// DENTRO de la auditoría cuando nadie afirmó nada.
    ///
    /// Las dos mitades hacen falta y esta prueba llegó a tener la segunda al
    /// revés: afirmaba `None` como resultado correcto, que es justo el estado que
    /// deja el registro exento. Medía la columna, no la alarma.
    #[tokio::test]
    async fn la_procedencia_se_guarda_y_el_mapa_vacio_deja_llaves_vacias() {
        let pool = base().await;
        let mut m = BTreeMap::new();
        m.insert("codigo_cum".to_string(), Procedencia::Confirmado);

        let crate::Creacion::Creada(con) =
            crear(&pool, Ambito::institucion(1), 100, &nueva(), &m).await.unwrap()
        else {
            panic!()
        };
        let crate::Creacion::Creada(sin) =
            crear(&pool, Ambito::institucion(1), 100, &nueva(), &Default::default())
                .await
                .unwrap()
        else {
            panic!()
        };

        let filas = listar_por_historia(&pool, Ambito::institucion(1), 100).await.unwrap();
        let de = |id: i64| filas.iter().find(|f| f.id == id).unwrap().procedencias.clone();
        assert_eq!(de(con).as_deref(), Some(r#"{"codigo_cum":"confirmado"}"#));
        // Vacío da `{}`, NO `NULL`, y esta línea es la que antes decía lo
        // contrario: con `NULL` el recolector se salta la fórmula entera y el
        // caso «nadie afirmó nada» —el estado por defecto de la casilla— no
        // producía ni una anotación. `NULL` queda solo para las filas anteriores
        // a la migración 0060, que es lo único que nadie reescribe.
        assert_eq!(de(sin).as_deref(), Some("{}"), "hubo captura y no se afirmó nada: eso se ANOTA, no se calla");
    }

    /// Editar los valores sin afirmar nada BORRA el aval anterior. Es la
    /// semántica del `PUT` de JSON, que no tiene dónde poner el toque: el visto
    /// bueno era del CUM viejo, no del campo para siempre.
    #[tokio::test]
    async fn actualizar_sin_afirmar_retira_el_aval_del_valor_anterior() {
        let pool = base().await;
        let mut m = BTreeMap::new();
        m.insert("codigo_cum".to_string(), Procedencia::Confirmado);
        let crate::Creacion::Creada(id) =
            crear(&pool, Ambito::institucion(1), 100, &nueva(), &m).await.unwrap()
        else {
            panic!()
        };

        let mut otro = nueva();
        otro.codigo_cum = Some("20000002-2".into());
        actualizar(&pool, Ambito::institucion(1), id, &otro, &Default::default()).await.unwrap();

        let f = listar_por_historia(&pool, Ambito::institucion(1), 100).await.unwrap();
        let f = f.iter().find(|x| x.id == id).unwrap();
        assert_eq!(f.codigo_cum.as_deref(), Some("20000002-2"));
        assert_eq!(
            f.procedencias.as_deref(),
            Some("{}"),
            "el aval no se hereda al valor nuevo, y el registro SIGUE auditándose: \
             con `NULL` este `PUT` lo sacaba de la auditoría para siempre"
        );

        // Pareja: si el llamante SÍ afirma, se guarda lo que afirmó.
        let mut m2 = BTreeMap::new();
        m2.insert("codigo_cum".to_string(), Procedencia::Propuesto);
        actualizar(&pool, Ambito::institucion(1), id, &otro, &m2).await.unwrap();
        let f = listar_por_historia(&pool, Ambito::institucion(1), 100).await.unwrap();
        let f = f.iter().find(|x| x.id == id).unwrap();
        assert_eq!(f.procedencias.as_deref(), Some(r#"{"codigo_cum":"propuesto"}"#));
    }

    #[tokio::test]
    async fn crear_listar_en_historia_propia() {
        let pool = base().await;
        let crate::Creacion::Creada(id) = crear(&pool, Ambito::institucion(1), 100, &nueva(), &Default::default()).await.unwrap()
        else {
            panic!("la historia 100 es del ámbito y está abierta")
        };
        let fs = listar_por_historia(&pool, Ambito::institucion(1), 100).await.unwrap();
        assert_eq!(fs.len(), 1);
        assert_eq!(fs[0].id, id);
        assert_eq!(fs[0].cantidad, 30.0);
        assert!(!fs[0].dispensada);
    }

    /// No se crea ni se lista una fórmula sobre la historia de otra institución.
    #[tokio::test]
    async fn la_historia_ajena_no_deja_formular() {
        let pool = base().await;
        assert_eq!(
            crear(&pool, Ambito::institucion(1), 200, &nueva(), &Default::default()).await.unwrap(),
            crate::Creacion::NoExiste,
            "la historia 200 es de otra institución"
        );
        assert!(listar_por_historia(&pool, Ambito::institucion(1), 200).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn actualizar_y_eliminar_respetan_el_ambito() {
        let pool = base().await;
        let crate::Creacion::Creada(id) = crear(&pool, Ambito::institucion(1), 100, &nueva(), &Default::default()).await.unwrap()
        else {
            panic!("la historia 100 es del ámbito y está abierta")
        };
        // La 2 no puede tocarla.
        assert_eq!(
            actualizar(&pool, Ambito::institucion(2), id, &nueva(), &Default::default()).await.unwrap(),
            crate::Resultado::NoExiste
        );
        assert_eq!(eliminar(&pool, Ambito::institucion(2), id).await.unwrap(), ResultadoBorrado::NoExiste);
        // La 1 sí.
        let mut cambio = nueva();
        cambio.dosis = "1 cada 12h".into();
        assert_eq!(
            actualizar(&pool, Ambito::institucion(1), id, &cambio, &Default::default()).await.unwrap(),
            crate::Resultado::Hecho
        );
        assert_eq!(eliminar(&pool, Ambito::institucion(1), id).await.unwrap(), ResultadoBorrado::Hecho);
        assert!(listar_por_historia(&pool, Ambito::institucion(1), 100).await.unwrap().is_empty());
    }

    /// Una fórmula ya entregada no se borra: dejaría una dispensación sin
    /// prescripción que la respalde.
    #[tokio::test]
    async fn una_formula_ya_dispensada_no_se_borra() {
        let pool = base().await;
        let crate::Creacion::Creada(id) = crear(&pool, Ambito::institucion(1), 100, &nueva(), &Default::default()).await.unwrap()
        else {
            panic!("la historia 100 es del ámbito y está abierta")
        };
        sqlx::query(
            "INSERT INTO item_inventario (id, institucion_id, nombre, lote, cantidad_actual) \
             VALUES (7, 1, 'Acetaminofén 500mg', 'L-1', 100)",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO dispensacion \
             (formula_id, item_inventario_id, paciente_id, regente_farmacia_id, \
              cantidad_dispensada, lote_dispensado) VALUES (?, 7, 10, 5, 30, 'L-1')",
        ).bind(id).execute(&pool).await.unwrap();

        assert_eq!(
            eliminar(&pool, Ambito::institucion(1), id).await.unwrap(),
            ResultadoBorrado::YaDispensada { entregas: 1 }
        );
        assert_eq!(listar_por_historia(&pool, Ambito::institucion(1), 100).await.unwrap().len(), 1);
    }
}
