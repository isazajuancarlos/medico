//! Historia clínica, con el modelo COMPLETO. Cuelga del paciente: quien no ve al
//! paciente no ve su historia (el aislamiento se sigue por esa cadena, como en el
//! sistema original — la historia no lleva institucion_id).

use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

/// Las columnas de `historia`, en el orden en que se leen y se escriben.
const COLUMNAS: &str = "id, fecha, codigo_historia, paciente_id, medico_id, cita_id, \
    acomp_nombre, acomp_parentesco, acomp_telefono, acomp_documento, \
    motivo_consulta, enfermedad_actual, \
    tension_arterial, pam, frecuencia_cardiaca, frecuencia_respiratoria, pulso, temperatura, \
    pabd, pc, peso, talla, imc, peso_ideal, estado_nutricional, \
    ef_cabeza, ef_ojos, ef_nariz, ef_oidos, ef_boca, ef_faringe, ef_cuello, ef_pulmones, \
    ef_corazon, ef_abdomen, ef_gu, ef_neurologico, ef_extremidad, ef_osteomuscular, ef_piel, \
    ef_hallazgos, ef_orl, ef_cardiopulmonar, \
    finalidad, causa_externa, tipo_diagnostico, cod_consulta_cups, \
    modalidad_grupo_servicio, grupo_servicios, cod_servicio, via_ingreso_servicio, causa_motivo, \
    valor_servicio, concepto_recaudo, valor_pago_moderador, num_fev_pago_moderador, \
    datos_dinamicos, \
    diagnostico_principal, diagnostico_relacionado1, diagnostico_relacionado2, \
    diagnostico_relacionado3, impresion_diagnostica, diagnostico_principal_cie11, \
    diagnostico_principal_uri, procedencias, genera_incapacidad, dias_incapacidad, plan_tratamiento";

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Historia {
    pub id: i64,
    pub fecha: String,
    pub codigo_historia: String,
    pub paciente_id: i64,
    pub medico_id: i64,
    pub cita_id: Option<i64>,
    pub acomp_nombre: Option<String>,
    pub acomp_parentesco: Option<String>,
    pub acomp_telefono: Option<String>,
    pub acomp_documento: Option<String>,
    pub motivo_consulta: String,
    pub enfermedad_actual: String,
    pub tension_arterial: Option<String>,
    pub pam: Option<String>,
    pub frecuencia_cardiaca: Option<i64>,
    pub frecuencia_respiratoria: Option<i64>,
    pub pulso: Option<i64>,
    pub temperatura: Option<f64>,
    pub pabd: Option<f64>,
    pub pc: Option<f64>,
    pub peso: Option<f64>,
    pub talla: Option<f64>,
    pub imc: Option<f64>,
    pub peso_ideal: Option<f64>,
    pub estado_nutricional: Option<String>,
    pub ef_cabeza: Option<String>,
    pub ef_ojos: Option<String>,
    pub ef_nariz: Option<String>,
    pub ef_oidos: Option<String>,
    pub ef_boca: Option<String>,
    pub ef_faringe: Option<String>,
    pub ef_cuello: Option<String>,
    pub ef_pulmones: Option<String>,
    pub ef_corazon: Option<String>,
    pub ef_abdomen: Option<String>,
    pub ef_gu: Option<String>,
    pub ef_neurologico: Option<String>,
    pub ef_extremidad: Option<String>,
    pub ef_osteomuscular: Option<String>,
    pub ef_piel: Option<String>,
    pub ef_hallazgos: Option<String>,
    pub ef_orl: Option<String>,
    pub ef_cardiopulmonar: Option<String>,
    pub finalidad: String,
    pub causa_externa: String,
    pub tipo_diagnostico: String,
    pub cod_consulta_cups: Option<String>,
    pub modalidad_grupo_servicio: Option<String>,
    pub grupo_servicios: Option<String>,
    pub cod_servicio: Option<i64>,
    pub via_ingreso_servicio: Option<String>,
    pub causa_motivo: Option<String>,
    pub valor_servicio: Option<i64>,
    pub concepto_recaudo: Option<String>,
    pub valor_pago_moderador: i64,
    pub num_fev_pago_moderador: Option<String>,
    pub datos_dinamicos: String,
    pub diagnostico_principal: String,
    pub diagnostico_relacionado1: Option<String>,
    pub diagnostico_relacionado2: Option<String>,
    pub diagnostico_relacionado3: Option<String>,
    pub impresion_diagnostica: Option<String>,
    pub diagnostico_principal_cie11: Option<String>,
    pub diagnostico_principal_uri: Option<String>,
    pub procedencias: Option<String>,
    pub genera_incapacidad: bool,
    pub dias_incapacidad: i64,
    pub plan_tratamiento: String,
}

/// Lo que llega para crear una historia. Los campos con defecto en la base son
/// `Option` aquí y se resuelven al insertar; los clínicamente obligatorios no.
#[derive(Debug, Clone, Deserialize)]
pub struct HistoriaNueva {
    pub codigo_historia: Option<String>,
    pub paciente_id: i64,
    pub medico_id: i64,
    pub cita_id: Option<i64>,
    pub acomp_nombre: Option<String>,
    pub acomp_parentesco: Option<String>,
    pub acomp_telefono: Option<String>,
    pub acomp_documento: Option<String>,
    pub motivo_consulta: String,
    pub enfermedad_actual: String,
    pub tension_arterial: Option<String>,
    pub pam: Option<String>,
    pub frecuencia_cardiaca: Option<i64>,
    pub frecuencia_respiratoria: Option<i64>,
    pub pulso: Option<i64>,
    pub temperatura: Option<f64>,
    pub pabd: Option<f64>,
    pub pc: Option<f64>,
    pub peso: Option<f64>,
    pub talla: Option<f64>,
    pub imc: Option<f64>,
    pub peso_ideal: Option<f64>,
    pub estado_nutricional: Option<String>,
    pub ef_cabeza: Option<String>,
    pub ef_ojos: Option<String>,
    pub ef_nariz: Option<String>,
    pub ef_oidos: Option<String>,
    pub ef_boca: Option<String>,
    pub ef_faringe: Option<String>,
    pub ef_cuello: Option<String>,
    pub ef_pulmones: Option<String>,
    pub ef_corazon: Option<String>,
    pub ef_abdomen: Option<String>,
    pub ef_gu: Option<String>,
    pub ef_neurologico: Option<String>,
    pub ef_extremidad: Option<String>,
    pub ef_osteomuscular: Option<String>,
    pub ef_piel: Option<String>,
    pub ef_hallazgos: Option<String>,
    pub ef_orl: Option<String>,
    pub ef_cardiopulmonar: Option<String>,
    pub finalidad: Option<String>,
    pub causa_externa: Option<String>,
    pub tipo_diagnostico: Option<String>,
    pub cod_consulta_cups: Option<String>,
    pub modalidad_grupo_servicio: Option<String>,
    pub grupo_servicios: Option<String>,
    pub cod_servicio: Option<i64>,
    pub via_ingreso_servicio: Option<String>,
    pub causa_motivo: Option<String>,
    pub valor_servicio: Option<i64>,
    pub concepto_recaudo: Option<String>,
    pub valor_pago_moderador: Option<i64>,
    pub num_fev_pago_moderador: Option<String>,
    pub datos_dinamicos: Option<String>,
    pub diagnostico_principal: String,
    pub diagnostico_relacionado1: Option<String>,
    pub diagnostico_relacionado2: Option<String>,
    pub diagnostico_relacionado3: Option<String>,
    pub impresion_diagnostica: Option<String>,
    pub diagnostico_principal_cie11: Option<String>,
    pub diagnostico_principal_uri: Option<String>,
    pub procedencias: Option<String>,
    pub genera_incapacidad: Option<bool>,
    pub dias_incapacidad: Option<i64>,
    pub plan_tratamiento: String,
}

/// ¿Es de la institución del ámbito el paciente `paciente_id`?
async fn paciente_es_del_ambito(pool: &crate::Pool, ambito: Ambito, paciente_id: i64) -> Result<bool, sqlx::Error> {
    let fila: Option<(i64,)> = sqlx::query_as("SELECT institucion_id FROM paciente WHERE id = ?")
        .bind(paciente_id)
        .fetch_optional(pool)
        .await?;
    Ok(fila.is_some_and(|(i,)| ambito.puede_ver(Some(i)) == Visibilidad::Si))
}

/// Crea una historia. El paciente debe pertenecer a la institución del ámbito
/// (una historia no cruza fronteras). `None` si no le pertenece o no existe.
pub async fn crear(pool: &crate::Pool, ambito: Ambito, h: &HistoriaNueva) -> Result<Option<i64>, sqlx::Error> {
    if !paciente_es_del_ambito(pool, ambito, h.paciente_id).await? {
        return Ok(None);
    }
    let codigo = match h.codigo_historia.as_deref() {
        Some(c) if !c.trim().is_empty() => c.to_string(),
        _ => format!("HC-{}-{}", h.paciente_id, chrono::Utc::now().timestamp()),
    };
    let sql = "INSERT INTO historia (\
        codigo_historia, paciente_id, medico_id, cita_id, \
        acomp_nombre, acomp_parentesco, acomp_telefono, acomp_documento, \
        motivo_consulta, enfermedad_actual, \
        tension_arterial, pam, frecuencia_cardiaca, frecuencia_respiratoria, pulso, temperatura, \
        pabd, pc, peso, talla, imc, peso_ideal, estado_nutricional, \
        ef_cabeza, ef_ojos, ef_nariz, ef_oidos, ef_boca, ef_faringe, ef_cuello, ef_pulmones, \
        ef_corazon, ef_abdomen, ef_gu, ef_neurologico, ef_extremidad, ef_osteomuscular, ef_piel, \
        ef_hallazgos, ef_orl, ef_cardiopulmonar, \
        finalidad, causa_externa, tipo_diagnostico, cod_consulta_cups, \
        modalidad_grupo_servicio, grupo_servicios, cod_servicio, via_ingreso_servicio, causa_motivo, \
        valor_servicio, concepto_recaudo, valor_pago_moderador, num_fev_pago_moderador, \
        datos_dinamicos, \
        diagnostico_principal, diagnostico_relacionado1, diagnostico_relacionado2, \
        diagnostico_relacionado3, impresion_diagnostica, diagnostico_principal_cie11, \
        diagnostico_principal_uri, procedencias, genera_incapacidad, dias_incapacidad, plan_tratamiento) \
        VALUES (?,?,?,?, ?,?,?,?, ?,?, ?,?,?,?,?,?, ?,?,?,?,?,?,?, \
        ?,?,?,?,?,?,?,?, ?,?,?,?,?,?,?, ?,?,?, ?,?,?,?, ?,?,?,?,?, ?,?,?,?, ?, ?,?,?,?,?,?,?,?, ?,?, ?)";
    let r = sqlx::query(sql)
        .bind(codigo)
        .bind(h.paciente_id)
        .bind(h.medico_id)
        .bind(h.cita_id)
        .bind(&h.acomp_nombre)
        .bind(&h.acomp_parentesco)
        .bind(&h.acomp_telefono)
        .bind(&h.acomp_documento)
        .bind(&h.motivo_consulta)
        .bind(&h.enfermedad_actual)
        .bind(&h.tension_arterial)
        .bind(&h.pam)
        .bind(h.frecuencia_cardiaca)
        .bind(h.frecuencia_respiratoria)
        .bind(h.pulso)
        .bind(h.temperatura)
        .bind(h.pabd)
        .bind(h.pc)
        .bind(h.peso)
        .bind(h.talla)
        .bind(h.imc)
        .bind(h.peso_ideal)
        .bind(&h.estado_nutricional)
        .bind(&h.ef_cabeza)
        .bind(&h.ef_ojos)
        .bind(&h.ef_nariz)
        .bind(&h.ef_oidos)
        .bind(&h.ef_boca)
        .bind(&h.ef_faringe)
        .bind(&h.ef_cuello)
        .bind(&h.ef_pulmones)
        .bind(&h.ef_corazon)
        .bind(&h.ef_abdomen)
        .bind(&h.ef_gu)
        .bind(&h.ef_neurologico)
        .bind(&h.ef_extremidad)
        .bind(&h.ef_osteomuscular)
        .bind(&h.ef_piel)
        .bind(&h.ef_hallazgos)
        .bind(&h.ef_orl)
        .bind(&h.ef_cardiopulmonar)
        .bind(h.finalidad.as_deref().unwrap_or("10"))
        .bind(h.causa_externa.as_deref().unwrap_or("13"))
        .bind(h.tipo_diagnostico.as_deref().unwrap_or("01"))
        .bind(&h.cod_consulta_cups)
        .bind(&h.modalidad_grupo_servicio)
        .bind(&h.grupo_servicios)
        .bind(h.cod_servicio)
        .bind(&h.via_ingreso_servicio)
        .bind(&h.causa_motivo)
        .bind(h.valor_servicio)
        .bind(&h.concepto_recaudo)
        .bind(h.valor_pago_moderador.unwrap_or(0))
        .bind(&h.num_fev_pago_moderador)
        .bind(h.datos_dinamicos.as_deref().unwrap_or("{}"))
        .bind(&h.diagnostico_principal)
        .bind(&h.diagnostico_relacionado1)
        .bind(&h.diagnostico_relacionado2)
        .bind(&h.diagnostico_relacionado3)
        .bind(&h.impresion_diagnostica)
        .bind(&h.diagnostico_principal_cie11)
        .bind(&h.diagnostico_principal_uri)
        .bind(&h.procedencias)
        .bind(h.genera_incapacidad.unwrap_or(false))
        .bind(h.dias_incapacidad.unwrap_or(0))
        .bind(&h.plan_tratamiento)
        .execute(pool)
        .await?;
    Ok(Some(r.last_insert_rowid()))
}

/// Las historias de un paciente, si el paciente es visible para el ámbito.
/// Vacío si no le pertenece (indistinguible de un paciente sin historias).
pub async fn listar_por_paciente(pool: &crate::Pool, ambito: Ambito, paciente_id: i64) -> Result<Vec<Historia>, sqlx::Error> {
    if !paciente_es_del_ambito(pool, ambito, paciente_id).await? {
        return Ok(Vec::new());
    }
    sqlx::query_as::<_, Historia>(&format!(
        "SELECT {COLUMNAS} FROM historia WHERE paciente_id = ? ORDER BY fecha DESC"
    ))
    .bind(paciente_id)
    .fetch_all(pool)
    .await
}

/// Una historia por id, si su paciente es visible para el ámbito.
pub async fn obtener(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<Option<Historia>, sqlx::Error> {
    let h = sqlx::query_as::<_, Historia>(&format!("SELECT {COLUMNAS} FROM historia WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    match h {
        Some(h) if paciente_es_del_ambito(pool, ambito, h.paciente_id).await? => Ok(Some(h)),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES (5,'Dra','d@x.co','h','medico',1,1)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1),(20,'CC','2','Beto','Paz','1985-01-01','M',2)")
            .execute(&pool).await.unwrap();
        pool
    }

    fn nueva(paciente_id: i64) -> HistoriaNueva {
        HistoriaNueva {
            codigo_historia: None,
            paciente_id,
            medico_id: 5,
            cita_id: None,
            acomp_nombre: None, acomp_parentesco: None, acomp_telefono: None, acomp_documento: None,
            motivo_consulta: "Dolor".into(),
            enfermedad_actual: "Desde ayer".into(),
            tension_arterial: Some("120/80".into()), pam: None,
            frecuencia_cardiaca: Some(72), frecuencia_respiratoria: None, pulso: None,
            temperatura: Some(36.8), pabd: None, pc: None, peso: Some(70.0), talla: Some(170.0),
            imc: None, peso_ideal: None, estado_nutricional: None,
            ef_cabeza: Some("Normal".into()), ef_ojos: None, ef_nariz: None, ef_oidos: None,
            ef_boca: None, ef_faringe: None, ef_cuello: None, ef_pulmones: None, ef_corazon: None,
            ef_abdomen: None, ef_gu: None, ef_neurologico: None, ef_extremidad: None,
            ef_osteomuscular: None, ef_piel: None, ef_hallazgos: None, ef_orl: None, ef_cardiopulmonar: None,
            finalidad: None, causa_externa: None, tipo_diagnostico: None, cod_consulta_cups: None,
            modalidad_grupo_servicio: None, grupo_servicios: None, cod_servicio: None,
            via_ingreso_servicio: None, causa_motivo: None, valor_servicio: None, concepto_recaudo: None,
            valor_pago_moderador: None, num_fev_pago_moderador: None,
            datos_dinamicos: None,
            diagnostico_principal: "J00".into(),
            diagnostico_relacionado1: None, diagnostico_relacionado2: None, diagnostico_relacionado3: None,
            impresion_diagnostica: None, diagnostico_principal_cie11: None, diagnostico_principal_uri: None,
            procedencias: None, genera_incapacidad: None, dias_incapacidad: None,
            plan_tratamiento: "Reposo".into(),
        }
    }

    #[tokio::test]
    async fn crear_y_obtener_conserva_los_campos_y_aplica_defectos() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), &nueva(10)).await.unwrap().unwrap();
        let h = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(h.motivo_consulta, "Dolor");
        assert_eq!(h.diagnostico_principal, "J00");
        assert_eq!(h.tension_arterial.as_deref(), Some("120/80"));
        assert_eq!(h.frecuencia_cardiaca, Some(72));
        assert_eq!(h.peso, Some(70.0));
        // Defectos aplicados.
        assert_eq!(h.finalidad, "10");
        assert_eq!(h.causa_externa, "13");
        assert_eq!(h.tipo_diagnostico, "01");
        assert_eq!(h.valor_pago_moderador, 0);
        assert_eq!(h.datos_dinamicos, "{}");
        assert!(!h.genera_incapacidad);
        assert!(h.codigo_historia.starts_with("HC-10-"));
    }

    #[tokio::test]
    async fn listar_por_paciente_devuelve_las_suyas() {
        let pool = base().await;
        crear(&pool, Ambito::institucion(1), &nueva(10)).await.unwrap();
        assert_eq!(listar_por_paciente(&pool, Ambito::institucion(1), 10).await.unwrap().len(), 1);
    }

    /// No se crea ni se ve la historia de un paciente de otra institución.
    #[tokio::test]
    async fn el_paciente_ajeno_no_deja_crear_ni_ver() {
        let pool = base().await;
        // La 1 no puede crear una historia para el paciente 20 (de la 2).
        assert!(crear(&pool, Ambito::institucion(1), &nueva(20)).await.unwrap().is_none());
        // Creada por la 2, la 1 no la ve.
        let id = crear(&pool, Ambito::institucion(2), &nueva(20)).await.unwrap().unwrap();
        assert!(obtener(&pool, Ambito::institucion(1), id).await.unwrap().is_none());
        assert!(listar_por_paciente(&pool, Ambito::institucion(1), 20).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn sin_ambito_no_ve_ni_crea() {
        let pool = base().await;
        assert!(crear(&pool, Ambito::Ninguno, &nueva(10)).await.unwrap().is_none());
        assert!(listar_por_paciente(&pool, Ambito::Ninguno, 10).await.unwrap().is_empty());
    }
}
