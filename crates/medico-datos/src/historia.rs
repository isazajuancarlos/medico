//! Historia clínica, con el modelo COMPLETO. Cuelga del paciente: quien no ve al
//! paciente no ve su historia (el aislamiento se sigue por esa cadena, como en el
//! sistema original — la historia no lleva institucion_id).

use chrono::{SecondsFormat, Utc};
use medico_dominio::{Ambito, Visibilidad};
use serde::{Deserialize, Serialize};

/// Las columnas de `historia`, en el orden en que se leen y se escriben.
const COLUMNAS: &str = "id, fecha, codigo_historia, paciente_id, medico_id, cita_id, \
    creado_por, firmado_por, firmado_en, \
    acomp_nombre, acomp_parentesco, acomp_telefono, acomp_documento, \
    motivo_consulta, enfermedad_actual, \
    tension_arterial, pam, frecuencia_cardiaca, frecuencia_respiratoria, pulso, temperatura, \
    pabd, pc, peso, talla, imc, peso_ideal, talla_ideal, estado_nutricional, \
    ef_cabeza, ef_ojos, ef_nariz, ef_oidos, ef_boca, ef_faringe, ef_cuello, ef_pulmones, \
    ef_corazon, ef_abdomen, ef_gu, ef_neurologico, ef_extremidad, ef_osteomuscular, ef_piel, \
    ef_hallazgos, ef_orl, ef_cardiopulmonar, \
    finalidad, causa_externa, tipo_diagnostico, cod_consulta_cups, \
    modalidad_grupo_servicio, grupo_servicios, cod_servicio, via_ingreso_servicio, causa_motivo, \
    valor_servicio, concepto_recaudo, valor_pago_moderador, num_fev_pago_moderador, \
    datos_dinamicos, \
    diagnostico_principal, diagnostico_relacionado1, diagnostico_relacionado2, \
    diagnostico_relacionado3, impresion_diagnostica, diagnostico_principal_cie11, \
    diagnostico_principal_uri, procedencias, genera_incapacidad, dias_incapacidad, plan_tratamiento, \
    firmada, firma";

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
    /// En METROS, como `talla`. La deriva el servidor buscando en la tabla
    /// `PESOTALLA1` del sistema original por edad y sexo (#104).
    pub talla_ideal: Option<f64>,
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
    /// Una historia firmada queda cerrada: no se edita ni se borra.
    /// Quién REGISTRÓ la historia. Lo pone el servidor desde la sesión; el
    /// cliente no lo manda. Distinto de `medico_id`, que es a quién se ATRIBUYE
    /// el acto: un auxiliar que transcribe la nota del médico es una operación
    /// real, y con un solo campo había que mentir en uno de los dos.
    pub creado_por: Option<i64>,
    pub firmada: bool,
    /// Firma criptográfica (post-cuántica) del contenido al firmarse. `None` si
    /// se cerró sin clave configurada. Verificarla detecta cualquier alteración.
    pub firma: Option<String>,
    /// Quién firmó, y cuándo. Van DENTRO de lo firmado (`firmable` serializa la
    /// historia entera y solo quita `firma` y `firmada`), así que cambiarlos
    /// rompe la firma. Una firma que no dice quién firmó no acredita a nadie.
    pub firmado_por: Option<i64>,
    pub firmado_en: Option<String>,
}

/// **Lo que le falta a esta historia para poder FIRMARSE**, con el nombre que
/// entiende quien la redacta. Vacío = se puede firmar.
///
/// # Por qué la puerta está en la FIRMA y no en el guardado
///
/// Es el hermano del informe radiológico ([`crate::diagnostico::InformeImagen::faltan_para_firmar`])
/// y la razón es la misma: un borrador es trabajo a medias —se abre la historia
/// con el motivo, entra el paciente, se explora, y el diagnóstico se escribe al
/// final—, así que bloquear el guardado **enseña a teclear un punto para poder
/// salvar**. Lo lento se rodea, y lo que se cuela es un dato falso. Lo que no
/// puede existir es una historia **FIRMADA** que no diga nada: la firma acredita
/// autoría (Ley 527/1999) y es el acto que la convierte en documento cerrado —
/// desde ahí no se edita, se emite otra.
///
/// No es hipotético: `POST /panel/historias/paciente/{id}` abre la historia con
/// `diagnostico_principal` y `plan_tratamiento` VACÍOS **a propósito** —abrirla
/// con un diagnóstico inventado sería el `890201` otra vez—, así que el camino
/// normal del panel produce exactamente ese borrador.
///
/// # De dónde sale la lista, que es lo que no se podía inventar
///
/// Decisión de Juan el 2026-08-08, tras medir que **no hay prior art que
/// copiar**: el VFP vivo declara `campos_obligatorios = txtcodigo, txtcedula`
/// (`Fuentes/forms/historia.SCT`), o sea nada clínico —y su bucle recorre las
/// comas pero siempre lee `GETWORDNUM(…, 1, ",")`, así que en la práctica solo
/// comprueba el código—; y `backend/routers/historias.py:187` firma sin mirar el
/// contenido. Los tres que quedan son los que toda atención tiene:
///
/// - **motivo de consulta** — no hay consulta sin motivo, ni siquiera un control;
/// - **diagnóstico principal** — la Resolución 948 lo exige en el AC
///   (`codDiagnosticoPrincipal`): sin él la atención no se reporta ni se cobra, y
///   el CIE-10 tiene código para el sano (Z00.1, control de salud de rutina);
/// - **plan de tratamiento** — la conducta es lo que el paciente se lleva. «Sigue
///   igual» es contenido; en blanco no lo es.
///
/// **`enfermedad_actual` queda FUERA a propósito**, aunque `HistoriaNueva` la
/// declare entre los clínicamente obligatorios: un control de crecimiento y
/// desarrollo o un control prenatal **no tienen enfermedad**, y exigirla enseña a
/// escribir «ninguna» — el dato falso que este repositorio persigue. Es la
/// directiva 21: la regla se prueba contra la operación normal del prestador, no
/// contra el descuido.
impl Historia {
    pub fn faltan_para_firmar(&self) -> Vec<&'static str> {
        [
            ("el motivo de consulta", &self.motivo_consulta),
            ("el diagnóstico principal", &self.diagnostico_principal),
            ("el plan de tratamiento", &self.plan_tratamiento),
        ]
        .into_iter()
        .filter(|(_, v)| v.trim().is_empty())
        .map(|(n, _)| n)
        .collect()
    }
}

/// Lo que llega para crear una historia. Los campos con defecto en la base son
/// `Option` aquí y se resuelven al insertar; los clínicamente obligatorios no.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// En METROS, como `talla`. La deriva el servidor buscando en la tabla
    /// `PESOTALLA1` del sistema original por edad y sexo (#104).
    pub talla_ideal: Option<f64>,
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
    /// La procedencia de los campos RIPS (#84), JSON `{campo: procedencia}`.
    ///
    /// **`skip_deserializing`: el cuerpo de `POST`/`PUT /api/historias` NO puede
    /// escribirla.** Lo llenan las pantallas del panel, que construyen esta
    /// estructura en Rust y le ponen lo que devuelve `fusionar_procedencias`.
    ///
    /// Sin esto era una cadena LIBRE del cliente enchufada directa al `.bind`, y
    /// eso abría dos cosas que ninguna pantalla puede producir: certificar en
    /// bloque como «confirmado» lo que nadie miró, y —peor— mandar un JSON
    /// ilegible, que `en_registro` se tragaba dejando la historia **exenta de la
    /// auditoría entera**, sin un mensaje y sin diferencia observable frente a
    /// una historia limpia. Lo cazó la revisión de la directiva 25 sobre el
    /// cambio de la fórmula: allí la puerta se había cerrado con un mapa tipado
    /// y aquí, en el portador ORIGINAL, llevaba abierta desde la migración 0005.
    ///
    /// Es el mismo defecto que arreglar en la REGLA y no en el caso (directiva
    /// 23): la procedencia la escribe el servidor o no la escribe nadie.
    #[serde(skip_deserializing)]
    pub procedencias: Option<String>,
    pub genera_incapacidad: Option<bool>,
    pub dias_incapacidad: Option<i64>,
    pub plan_tratamiento: String,
}

/// Lo que hay HOY escrito en la historia, en la forma que espera
/// [`actualizar`].
///
/// Existe porque `actualizar` **reemplaza las 61 columnas editables de una vez**
/// —es lo que hace el `PUT` desde que nació— y hay un consumidor que no edita la
/// historia entera: el panel la guarda por secciones (signos vitales,
/// diagnóstico, plan…). Sin esta función, cada sección tendría que reconstruir
/// a mano el resto de los campos, y **el que se olvidara se borraría en
/// silencio**: no da error, no rompe nada y solo se nota cuando alguien vuelve a
/// abrir la historia. En una historia clínica eso no se puede reconstruir.
///
/// Se escribe aquí, al lado de las dos estructuras, y no en la capa de
/// pantallas, para que una columna nueva no obligue a acordarse de un archivo
/// lejano — y para que el guardián de abajo pueda medir que no falta ninguna.
pub fn como_nueva(h: &Historia) -> HistoriaNueva {
    HistoriaNueva {
        codigo_historia: Some(h.codigo_historia.clone()),
        paciente_id: h.paciente_id,
        medico_id: h.medico_id,
        cita_id: h.cita_id,
        acomp_nombre: h.acomp_nombre.clone(),
        acomp_parentesco: h.acomp_parentesco.clone(),
        acomp_telefono: h.acomp_telefono.clone(),
        acomp_documento: h.acomp_documento.clone(),
        motivo_consulta: h.motivo_consulta.clone(),
        enfermedad_actual: h.enfermedad_actual.clone(),
        tension_arterial: h.tension_arterial.clone(),
        pam: h.pam.clone(),
        frecuencia_cardiaca: h.frecuencia_cardiaca,
        frecuencia_respiratoria: h.frecuencia_respiratoria,
        pulso: h.pulso,
        temperatura: h.temperatura,
        pabd: h.pabd,
        pc: h.pc,
        peso: h.peso,
        talla: h.talla,
        imc: h.imc,
        peso_ideal: h.peso_ideal,
        talla_ideal: h.talla_ideal,
        estado_nutricional: h.estado_nutricional.clone(),
        ef_cabeza: h.ef_cabeza.clone(),
        ef_ojos: h.ef_ojos.clone(),
        ef_nariz: h.ef_nariz.clone(),
        ef_oidos: h.ef_oidos.clone(),
        ef_boca: h.ef_boca.clone(),
        ef_faringe: h.ef_faringe.clone(),
        ef_cuello: h.ef_cuello.clone(),
        ef_pulmones: h.ef_pulmones.clone(),
        ef_corazon: h.ef_corazon.clone(),
        ef_abdomen: h.ef_abdomen.clone(),
        ef_gu: h.ef_gu.clone(),
        ef_neurologico: h.ef_neurologico.clone(),
        ef_extremidad: h.ef_extremidad.clone(),
        ef_osteomuscular: h.ef_osteomuscular.clone(),
        ef_piel: h.ef_piel.clone(),
        ef_hallazgos: h.ef_hallazgos.clone(),
        ef_orl: h.ef_orl.clone(),
        ef_cardiopulmonar: h.ef_cardiopulmonar.clone(),
        finalidad: Some(h.finalidad.clone()),
        causa_externa: Some(h.causa_externa.clone()),
        tipo_diagnostico: Some(h.tipo_diagnostico.clone()),
        cod_consulta_cups: h.cod_consulta_cups.clone(),
        modalidad_grupo_servicio: h.modalidad_grupo_servicio.clone(),
        grupo_servicios: h.grupo_servicios.clone(),
        cod_servicio: h.cod_servicio,
        via_ingreso_servicio: h.via_ingreso_servicio.clone(),
        causa_motivo: h.causa_motivo.clone(),
        valor_servicio: h.valor_servicio,
        concepto_recaudo: h.concepto_recaudo.clone(),
        valor_pago_moderador: Some(h.valor_pago_moderador),
        num_fev_pago_moderador: h.num_fev_pago_moderador.clone(),
        datos_dinamicos: Some(h.datos_dinamicos.clone()),
        diagnostico_principal: h.diagnostico_principal.clone(),
        diagnostico_relacionado1: h.diagnostico_relacionado1.clone(),
        diagnostico_relacionado2: h.diagnostico_relacionado2.clone(),
        diagnostico_relacionado3: h.diagnostico_relacionado3.clone(),
        impresion_diagnostica: h.impresion_diagnostica.clone(),
        diagnostico_principal_cie11: h.diagnostico_principal_cie11.clone(),
        diagnostico_principal_uri: h.diagnostico_principal_uri.clone(),
        procedencias: h.procedencias.clone(),
        genera_incapacidad: Some(h.genera_incapacidad),
        dias_incapacidad: Some(h.dias_incapacidad),
        plan_tratamiento: h.plan_tratamiento.clone(),
    }
}

/// Lo que [`Historia`] tiene y [`HistoriaNueva`] no, con el motivo. Es la lista
/// de lo que **NO se edita nunca por el cuerpo de una petición**, y por eso es
/// una tabla y no un comentario: el guardián de abajo la usa para decidir qué
/// ausencia es legítima.
pub const SOLO_DEL_SERVIDOR: &[(&str, &str)] = &[
    ("id", "lo pone la base"),
    ("fecha", "el instante de la atención, no un dato que se teclee"),
    ("creado_por", "sale de la sesión: quién teclea no lo dice el cuerpo"),
    ("firmada", "la mueve `firmar`, y una firmada ya no se edita"),
    ("firma", "la calcula `firmar` sobre el contenido"),
    ("firmado_por", "va DENTRO de lo firmado"),
    ("firmado_en", "va DENTRO de lo firmado"),
];

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
pub async fn crear(
    pool: &crate::Pool,
    ambito: Ambito,
    // Quién la registra: sale de la SESIÓN, nunca del cuerpo. Es lo que hace que
    // el registro diga lo que pasó cuando quien teclea no es a quien se atribuye.
    creado_por: i64,
    h: &HistoriaNueva,
) -> Result<Option<i64>, sqlx::Error> {
    if !paciente_es_del_ambito(pool, ambito, h.paciente_id).await? {
        return Ok(None);
    }
    // Y el MÉDICO. Es la más grave de las tres: una historia se cierra con
    // `firmar`, que pone una firma criptográfica sobre un contenido que nombra al
    // profesional. Sin esta comprobación, una institución podía dejar firmado un
    // acto clínico a nombre de un médico de OTRA — que no puede verlo ni negarlo.
    if !crate::usuario::es_del_ambito(pool, ambito, h.medico_id).await? {
        return Ok(None);
    }
    let codigo = match h.codigo_historia.as_deref() {
        Some(c) if !c.trim().is_empty() => c.to_string(),
        _ => format!("HC-{}-{}", h.paciente_id, chrono::Utc::now().timestamp()),
    };
    let sql = "INSERT INTO historia (\
        creado_por, \
        codigo_historia, paciente_id, medico_id, cita_id, \
        acomp_nombre, acomp_parentesco, acomp_telefono, acomp_documento, \
        motivo_consulta, enfermedad_actual, \
        tension_arterial, pam, frecuencia_cardiaca, frecuencia_respiratoria, pulso, temperatura, \
        pabd, pc, peso, talla, imc, peso_ideal, talla_ideal, estado_nutricional, \
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
        VALUES (?, ?,?,?,?, ?,?,?,?, ?,?, ?,?,?,?,?,?, ?,?,?,?,?,?,?,?, \
        ?,?,?,?,?,?,?,?, ?,?,?,?,?,?,?, ?,?,?, ?,?,?,?, ?,?,?,?,?, ?,?,?,?, ?, ?,?,?,?,?,?,?,?, ?,?, ?)";
    let r = sqlx::query(sql)
        .bind(creado_por)
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
        .bind(h.talla_ideal)
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
        // Los tres RIPS que iban con relleno. Ver `0038_finalidad_de_la_948.sql`:
        // «10», «13» y «01» no los eligió nadie —ninguna pantalla ofrece estos
        // campos— y los dos primeros ni siquiera existen en las tablas de la
        // 948. Vacío es lo que son, y el recolector del RIPS ya lee el blanco
        // como ausencia, así que salen en el informe de cierre en vez de
        // viajar al validador como si alguien los hubiera afirmado.
        .bind(h.finalidad.as_deref().unwrap_or(""))
        .bind(h.causa_externa.as_deref().unwrap_or(""))
        .bind(h.tipo_diagnostico.as_deref().unwrap_or(""))
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
        // NUNCA `NULL`, y esta línea es una salvaguarda, no un detalle de estilo.
        //
        // `NULL` significa UNA sola cosa: «fila anterior a la captura, no se
        // audita» (directiva 32: `{}` y `NULL` no son el mismo valor). Escribirlo
        // aquí EXIME el registro de toda la auditoría de #84, y `crear`/`actualizar`
        // son los dos únicos sitios del programa que tocan esta columna.
        //
        // Lo cazaron los TRES revisores de la directiva 25 sobre `9f8c147d`, por
        // separado y con confianza 8/9/9: `HistoriaNueva.procedencias` lleva
        // `#[serde(skip_deserializing)]`, así que en toda petición JSON llega
        // `None` — y un `PUT /api/historias/{id}` con el MISMO cuerpo que devuelve
        // el `GET` dejaba la columna en `NULL`, respondía 200, y sacaba la consulta
        // del cierre de mes sin cambiar un byte del RIPS ni dejar una anotación.
        // Es el defecto que esta misma rama arregló en `formula::json_de` —que
        // devuelve siempre `Some`— y que quedó abierto en el portador ORIGINAL.
        //
        // Escribir `{}` es lo correcto y no lo conservador: quien guarda por la API
        // NO puede afirmar nada, así que lo que toca pierde su aval y la auditoría
        // lo marca. Conservar el mapa anterior sería PEOR que el fallo — dejaría un
        // «confirmado» pegado a un valor nuevo que nadie miró.
        .bind(h.procedencias.as_deref().unwrap_or("{}"))
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

// Editar o borrar una historia firmada se niega distinto de «no existe»: la
// distinción vive en `crate::Resultado`, compartida con los documentos clínicos.
pub use crate::Resultado;

/// Lee el paciente y el estado de firma de una historia visible para el ámbito.
/// `None` si no existe o es ajena —indistinguibles—.
pub(crate) async fn paciente_y_firma(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<(i64, bool)>, sqlx::Error> {
    let fila: Option<(i64, bool)> =
        sqlx::query_as("SELECT paciente_id, firmada FROM historia WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    match fila {
        Some((pid, firmada)) if paciente_es_del_ambito(pool, ambito, pid).await? => {
            Ok(Some((pid, firmada)))
        }
        _ => Ok(None),
    }
}

/// ¿La historia admite que se le AÑADA o se le QUITE algo colgado?
///
/// `NoExiste` si no es del ámbito —o no existe, indistinguibles—; `Firmada` si
/// está cerrada; `Hecho` si se puede escribir.
///
/// Vive aquí y la usan `formula`, `orden` e `imagen` porque la regla es de la
/// HISTORIA, no de cada tabla hija: «una historia firmada no se edita **por
/// ninguna puerta**, tampoco añadiéndole un medicamento o una orden». Hasta el
/// 2026-08-10 esa regla estaba escrita en el CLAUDE.md y en los manejadores, y
/// la revisión de seguridad midió el resultado: de las seis puertas del panel la
/// cumplían tres, y en la API **ninguna** —ni siquiera al crear—. Doce `if`
/// repartidos son el control escrito a mano que se olvida en la puerta
/// siguiente, que es literalmente el argumento con el que nació `Autorizado`.
///
/// **Lo que NO se cierra, y es deliberado**: `orden::cambiar_estado`. Marcar una
/// orden como procesada es flujo operativo POSTERIOR —el laboratorio trabaja
/// sobre una historia ya firmada, que es lo normal—, no editar el documento.
/// Cerrarlo dejaría el laboratorio sin poder cerrar sus órdenes (directiva 21).
pub async fn admite_escritura(
    pool: &crate::Pool,
    ambito: Ambito,
    historia_id: i64,
) -> Result<Resultado, sqlx::Error> {
    match paciente_y_firma(pool, ambito, historia_id).await? {
        None => Ok(Resultado::NoExiste),
        Some((_, true)) => Ok(Resultado::Firmada),
        Some((_, false)) => Ok(Resultado::Hecho),
    }
}

/// Lo mismo, resolviendo la historia desde una fila COLGADA de ella.
///
/// `historia_id` se lee con la consulta que el llamante pasa ya resuelta: cada
/// tabla hija sabe cuál es su columna, y así no hay SQL con nombre de tabla
/// interpolado.
pub async fn admite_escritura_de(
    pool: &crate::Pool,
    ambito: Ambito,
    historia_id: Option<i64>,
) -> Result<Resultado, sqlx::Error> {
    match historia_id {
        None => Ok(Resultado::NoExiste),
        Some(h) => admite_escritura(pool, ambito, h).await,
    }
}

/// Actualiza los campos editables de una historia. No toca `paciente_id`,
/// `medico_id`, `codigo_historia` ni `fecha`: editar una historia no la mueve de
/// paciente ni la re-fecha. **Se niega sobre una historia firmada** (`Firmada`),
/// que es inmutable por diseño.
pub async fn actualizar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    h: &HistoriaNueva,
) -> Result<Resultado, sqlx::Error> {
    let Some((_, firmada)) = paciente_y_firma(pool, ambito, id).await? else {
        return Ok(Resultado::NoExiste);
    };
    if firmada {
        return Ok(Resultado::Firmada);
    }
    let sql = "UPDATE historia SET \
        cita_id = ?, acomp_nombre = ?, acomp_parentesco = ?, acomp_telefono = ?, acomp_documento = ?, \
        motivo_consulta = ?, enfermedad_actual = ?, \
        tension_arterial = ?, pam = ?, frecuencia_cardiaca = ?, frecuencia_respiratoria = ?, pulso = ?, temperatura = ?, \
        pabd = ?, pc = ?, peso = ?, talla = ?, imc = ?, peso_ideal = ?, talla_ideal = ?, estado_nutricional = ?, \
        ef_cabeza = ?, ef_ojos = ?, ef_nariz = ?, ef_oidos = ?, ef_boca = ?, ef_faringe = ?, ef_cuello = ?, ef_pulmones = ?, \
        ef_corazon = ?, ef_abdomen = ?, ef_gu = ?, ef_neurologico = ?, ef_extremidad = ?, ef_osteomuscular = ?, ef_piel = ?, \
        ef_hallazgos = ?, ef_orl = ?, ef_cardiopulmonar = ?, \
        finalidad = ?, causa_externa = ?, tipo_diagnostico = ?, cod_consulta_cups = ?, \
        modalidad_grupo_servicio = ?, grupo_servicios = ?, cod_servicio = ?, via_ingreso_servicio = ?, causa_motivo = ?, \
        valor_servicio = ?, concepto_recaudo = ?, valor_pago_moderador = ?, num_fev_pago_moderador = ?, \
        datos_dinamicos = ?, \
        diagnostico_principal = ?, diagnostico_relacionado1 = ?, diagnostico_relacionado2 = ?, \
        diagnostico_relacionado3 = ?, impresion_diagnostica = ?, diagnostico_principal_cie11 = ?, \
        diagnostico_principal_uri = ?, procedencias = ?, genera_incapacidad = ?, dias_incapacidad = ?, plan_tratamiento = ? \
        WHERE id = ?";
    sqlx::query(sql)
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
        .bind(h.talla_ideal)
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
        // Los tres RIPS que iban con relleno. Ver `0038_finalidad_de_la_948.sql`:
        // «10», «13» y «01» no los eligió nadie —ninguna pantalla ofrece estos
        // campos— y los dos primeros ni siquiera existen en las tablas de la
        // 948. Vacío es lo que son, y el recolector del RIPS ya lee el blanco
        // como ausencia, así que salen en el informe de cierre en vez de
        // viajar al validador como si alguien los hubiera afirmado.
        .bind(h.finalidad.as_deref().unwrap_or(""))
        .bind(h.causa_externa.as_deref().unwrap_or(""))
        .bind(h.tipo_diagnostico.as_deref().unwrap_or(""))
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
        // NUNCA `NULL`, y esta línea es una salvaguarda, no un detalle de estilo.
        //
        // `NULL` significa UNA sola cosa: «fila anterior a la captura, no se
        // audita» (directiva 32: `{}` y `NULL` no son el mismo valor). Escribirlo
        // aquí EXIME el registro de toda la auditoría de #84, y `crear`/`actualizar`
        // son los dos únicos sitios del programa que tocan esta columna.
        //
        // Lo cazaron los TRES revisores de la directiva 25 sobre `9f8c147d`, por
        // separado y con confianza 8/9/9: `HistoriaNueva.procedencias` lleva
        // `#[serde(skip_deserializing)]`, así que en toda petición JSON llega
        // `None` — y un `PUT /api/historias/{id}` con el MISMO cuerpo que devuelve
        // el `GET` dejaba la columna en `NULL`, respondía 200, y sacaba la consulta
        // del cierre de mes sin cambiar un byte del RIPS ni dejar una anotación.
        // Es el defecto que esta misma rama arregló en `formula::json_de` —que
        // devuelve siempre `Some`— y que quedó abierto en el portador ORIGINAL.
        //
        // Escribir `{}` es lo correcto y no lo conservador: quien guarda por la API
        // NO puede afirmar nada, así que lo que toca pierde su aval y la auditoría
        // lo marca. Conservar el mapa anterior sería PEOR que el fallo — dejaría un
        // «confirmado» pegado a un valor nuevo que nadie miró.
        .bind(h.procedencias.as_deref().unwrap_or("{}"))
        .bind(h.genera_incapacidad.unwrap_or(false))
        .bind(h.dias_incapacidad.unwrap_or(0))
        .bind(&h.plan_tratamiento)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(Resultado::Hecho)
}

/// El resumen de los ANEXOS de una historia: lo que el médico prescribió y
/// ordenó, y los archivos que adjuntó.
///
/// # Por qué esto entra en la firma
///
/// Hasta el 2026-08-10 `firmable` serializaba SOLO la fila `historia`, así que
/// la firma acreditaba que esos sesenta campos no habían cambiado y **no decía
/// nada de la prescripción**. Se podía borrar un medicamento de una historia
/// firmada y `verificar` seguía contestando `Valida`. Cerrar las doce puertas de
/// la aplicación (`admite_escritura`) impide hacerlo DESDE EL PRODUCTO; no
/// impide hacerlo contra la base ni sobre un respaldo, y la capa que responde de
/// eso ante un tercero —un juez, una auditoría— es la firma, no el manejador.
///
/// En términos de la Ley 527/1999: la firma tiene que acreditar que el documento
/// es el mismo que se firmó, y una historia clínica sin su fórmula no es el
/// documento que el médico firmó.
///
/// # Qué entra y qué no
///
/// Entran las TRES tablas que son contenido clínico de la historia: `formula`,
/// `orden` e `imagen_diagnostica`. **No** entra `documento_clinico` —tiene su
/// propia firma, y meterla aquí la firmaría dos veces con dos alcances
/// distintos— ni `consentimiento` ni `factura`, que cuelgan de la historia por
/// conveniencia y no son parte del acto clínico que se firma.
///
/// # Determinismo
///
/// `ORDER BY id` y columnas fijas: firmar y verificar tienen que producir los
/// mismos bytes o la firma sería inestable, que es peor que no tenerla —una
/// verificación que falla a veces enseña a ignorarla—.
/// **LAS COLUMNAS QUE ENTRAN EN LA FIRMA, y son la ÚNICA fuente de verdad.**
///
/// La consulta se CONSTRUYE con estas listas: no hay un `SELECT` escrito aparte
/// que pueda divergir de ellas, y no hay tupla cuyo orden haya que mantener
/// alineado a mano. Añadir una columna al resumen es añadir un renglón aquí.
///
/// # Por qué existen también las listas de EXCLUIDAS
///
/// Van tres olvidos en esta lista —`unidad`, `observaciones` y `procedencias`—
/// y la revisión de la directiva 25 sobre `1c570350` encontró el cuarto, que
/// eran TRECE columnas de DINERO de la migración 0041: `valor_unitario`,
/// `valor_servicio`, `valor_pago_moderador`… todas leídas EN VIVO al cerrar el
/// mes (`rips.rs`), todas fuera de la firma. No son cuatro defectos: es uno
/// —**nadie vigilaba esta lista**— mientras `como_nueva` sí tiene su guardián.
///
/// Por eso toda columna de las tres tablas tiene que estar en una de las dos
/// listas, con su motivo si está fuera, y lo exige
/// `toda_columna_de_los_anexos_esta_declarada` leyendo `PRAGMA table_info`. Una
/// columna nueva que nadie clasifique pone la prueba roja el día que nace, que
/// es cuando la decisión se puede tomar bien.
const FIRMADAS_FORMULA: &[&str] = &[
    "id",
    "medicamento",
    "codigo_cum",
    "dosis",
    "cantidad",
    "unidad",
    "dias_tratamiento",
    "observaciones",
    "fecha_formula",
    "tipo_medicamento",
    "valor_unitario",
    "valor_servicio",
    "concepto_recaudo",
    "valor_pago_moderador",
    "num_fev_pago_moderador",
    "procedencias",
];

/// Las de `formula` que quedan FUERA, cada una con su motivo. Sin motivo escrito
/// no es una decisión: es un olvido con otra ropa.
#[cfg(test)]
const FUERA_FORMULA: &[(&str, &str)] = &[
    (
        "historia_id",
        "es el WHERE de la consulta: moverla saca la fila de este resumen y la mete \
         en otro, así que rompe las dos firmas sin necesidad de estar dentro",
    ),
    (
        "dispensada",
        "la mueve FARMACIA después de firmar (inventario.rs). Sellarla haría que \
         dispensar un medicamento rompiera la firma de la consulta, que es \
         operación normal — y una verificación que falla por lo normal es ruido",
    ),
];

const FIRMADAS_ORDEN: &[&str] = &[
    "id",
    "tipo",
    "codigo_cups",
    "descripcion",
    "justificacion",
    "especialidad_destino_id",
    "institucion_destino_id",
    "fecha_orden",
    "via_ingreso_servicio",
    "grupo_servicios",
    "cod_servicio",
    "valor_servicio",
    "concepto_recaudo",
    "valor_pago_moderador",
    "num_fev_pago_moderador",
];

#[cfg(test)]
const FUERA_ORDEN: &[(&str, &str)] = &[
    ("historia_id", "el WHERE, igual que en `formula`"),
    (
        "estado",
        "lo mueve el flujo posterior (`orden::cambiar_estado`): marcar una orden \
         como procesada no es editar el documento firmado",
    ),
];

const FIRMADAS_IMAGEN: &[&str] = &["id", "nombre_original", "tipo_mime", "tamano_bytes", "descripcion"];

#[cfg(test)]
const FUERA_IMAGEN: &[(&str, &str)] = &[
    ("historia_id", "el WHERE, igual que en `formula`"),
    (
        "ruta_archivo",
        "los BYTES del adjunto no entran: sellarlos obligaría a leer y descifrar el \
         archivo en cada verificación, y lo que hay que detectar es que el anexo \
         cambió o desapareció — eso ya lo dicen nombre, tipo y tamaño",
    ),
    (
        "cifrada",
        "propiedad del almacenamiento, no del documento: recifrar un adjunto no \
         cambia lo que el médico firmó",
    ),
    (
        "fecha_subida",
        "la pone el servidor al recibir el archivo y nadie la edita; sellarla no \
         añade nada que el `id` y el contenido no digan ya",
    ),
    (
        "origen",
        "decide si la fila ENTRA en este resumen (`adjunto_historia`) o no \
         (`resultado_imagenologia`, migración 0056). Cambiarla mete o saca el \
         anexo de la lista, y eso rompe la firma por sí solo",
    ),
];

/// Lee las columnas declaradas de una tabla como filas de array JSON.
///
/// `json_array` lo calcula SQLite, así que los tipos se conservan —un entero
/// sale entero y un `NULL` sale `null`— sin una tupla de dieciséis campos cuyo
/// orden haya que vigilar. JSON1 está compilado en el SQLite vendorizado
/// (`vendor/libsqlite3-sys/build.rs`, `-DSQLITE_ENABLE_JSON1`) y ya se usa en
/// `administracion.rs`.
async fn filas_de_anexo(
    pool: &crate::Pool,
    tabla: &str,
    columnas: &[&str],
    filtro_extra: &str,
    historia_id: i64,
) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    let sql = format!(
        "SELECT json_array({}) FROM {tabla} WHERE historia_id = ?{filtro_extra} ORDER BY id",
        columnas.join(", ")
    );
    let filas: Vec<(String,)> = sqlx::query_as(&sql).bind(historia_id).fetch_all(pool).await?;
    // REVIENTA, no degrada. Un `unwrap_or(Value::Null)` aquí convertiría una fila
    // ilegible en `null` y la firma dejaría de cubrirla **en silencio** — que es
    // exactamente el silenciador que este mismo commit quita en `vista/historias.rs`
    // y que `en_registro` quitó antes en el recolector. Firmar sobre un resumen
    // incompleto es peor que no poder firmar (directiva 20).
    filas
        .into_iter()
        .map(|(j,)| {
            serde_json::from_str(&j).map_err(|e| {
                sqlx::Error::Decode(
                    format!("`{tabla}`: json_array devolvió algo ilegible ({e})").into(),
                )
            })
        })
        .collect()
}

async fn resumen_de_anexos(
    pool: &crate::Pool,
    historia_id: i64,
) -> Result<serde_json::Value, sqlx::Error> {
    let formulas = filas_de_anexo(pool, "formula", FIRMADAS_FORMULA, "", historia_id).await?;
    let ordenes = filas_de_anexo(pool, "orden", FIRMADAS_ORDEN, "", historia_id).await?;
    // SOLO los adjuntos que el médico puso en la historia. La placa que el
    // servicio de imagenología sube DESPUÉS (`resultado_imagenologia`, migración
    // 0056) no es contenido del documento firmado, y sellarla haría que una
    // operación normal —subir la placa de una consulta ya firmada— rompiera la
    // firma. Una verificación que falla por lo normal es ruido, y el ruido
    // enseña a ignorarla.
    let adjuntos = filas_de_anexo(
        pool,
        "imagen_diagnostica",
        FIRMADAS_IMAGEN,
        " AND origen = 'adjunto_historia'",
        historia_id,
    )
    .await?;

    Ok(serde_json::json!({
        // La VERSIÓN del formato del resumen, dentro de lo firmado. Sin ella, el
        // próximo cambio de forma invalidaría en silencio todas las firmas que
        // existieran entonces y nadie sabría por qué.
        //
        // **SUBE A 2 el 2026-08-16**, y la lección es que no subió sola. El
        // commit anterior amplió las fórmulas de 8 a 9 elementos y dejó el `1`
        // puesto: dos formatos distintos afirmando ser la misma versión, que es
        // exactamente el fail-open que este campo existe para impedir —el día
        // que alguien escriba un verificador que ramifique por versión, elegiría
        // el layout viejo para una firma nueva y `procedencias` saldría de la
        // verificación sin que nada fallara—. Lo cazó la revisión de la
        // directiva 25 sobre `1c570350`.
        //
        // Consecuencia declarada: **toda firma emitida con el formato anterior
        // verifica `Alterada`**. Es correcto —no sellaba lo que decía sellar— y
        // hay que decirlo antes de desplegar, no después.
        "v": 2,
        "formulas": formulas,
        "ordenes": ordenes,
        "adjuntos": adjuntos,
    }))
}

/// Bytes canónicos que se firman: el contenido de la historia como JSON con
/// claves ORDENADAS (BTreeMap), EXCLUYENDO los campos de firma, **más el
/// resumen de sus anexos** (ver [`resumen_de_anexos`]). Determinista: firmar y
/// verificar producen los mismos bytes, y cambiar cualquier campo clínico —o
/// cualquier fórmula, orden o adjunto— los cambia y rompe la firma.
fn firmable(h: &Historia, anexos: &serde_json::Value) -> Vec<u8> {
    let v = serde_json::to_value(h).expect("Historia serializa");
    let mut m: std::collections::BTreeMap<String, serde_json::Value> = match v {
        serde_json::Value::Object(o) => o.into_iter().collect(),
        _ => std::collections::BTreeMap::new(),
    };
    m.remove("firma");
    m.remove("firmada");
    // La clave va con un nombre que NINGUNA columna de `historia` usa, para que
    // no pueda colisionar con un campo y sustituirlo en silencio.
    m.insert("__anexos".to_string(), anexos.clone());
    guaca::firma::canonico(&m)
}

/// Firma una historia visible y sin firmar, y la cierra. Con `sk_hex` guarda una
/// firma criptográfica del contenido; sin clave (`None`) la cierra sin firma —la
/// ausencia de firma nunca impide cerrar—. `false` si no existe, es ajena o ya
/// estaba firmada.
pub async fn firmar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    // Quién firma: de la SESIÓN, nunca del cuerpo.
    firmante_id: i64,
    sk_hex: Option<&str>,
) -> Result<ResultadoFirma, sqlx::Error> {
    let Some(mut h) = obtener(pool, ambito, id).await? else {
        return Ok(ResultadoFirma::NoSePudo);
    };
    if h.firmada {
        return Ok(ResultadoFirma::NoSePudo);
    }

    // La puerta que faltaba. Va ANTES de tocar nada —no hay firmante que poner ni
    // clave que leer— y antes de la clave a propósito: a quien le falta el
    // diagnóstico hay que decirle eso, no «la clave del despliegue está rota».
    let faltan = h.faltan_para_firmar();
    if !faltan.is_empty() {
        return Ok(ResultadoFirma::Incompleto { faltan });
    }

    // El firmante y el instante se ponen ANTES de calcular la firma, porque
    // `firmable` serializa la historia entera: así quedan DENTRO de lo firmado y
    // cambiarlos después la rompe. Puestos después, la firma no diría nada sobre
    // quién firmó — que es justo lo que se está arreglando.
    h.firmado_por = Some(firmante_id);
    h.firmado_en = Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));

    // Los anexos se leen JUSTO antes de firmar, y quedan dentro de lo firmado.
    let anexos = resumen_de_anexos(pool, id).await?;

    // Dos ausencias que NO son la misma:
    //
    //   sin clave configurada  -> cerrar sin firma criptográfica es una postura
    //                             válida y documentada: la falta de firma nunca
    //                             impide cerrar una historia.
    //   clave configurada pero ILEGIBLE -> antes daba `firma = NULL` y aun así
    //                             marcaba `firmada = 1`. Una historia marcada como
    //                             firmada sin firma es peor que una sin firmar:
    //                             afirma algo que no ocurrió. Ahora se para.
    let firma = match sk_hex {
        None => None,
        Some(sk) => match guaca::firma::firmar(&firmable(&h, &anexos), sk) {
            Some(f) => Some(f),
            None => return Ok(ResultadoFirma::ClaveInservible),
        },
    };

    let r = sqlx::query(
        "UPDATE historia SET firmada = 1, firma = ?, firmado_por = ?, firmado_en = ? \
         WHERE id = ? AND firmada = 0",
    )
    .bind(&firma)
    .bind(firmante_id)
    .bind(&h.firmado_en)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(if r.rows_affected() > 0 {
        ResultadoFirma::Firmada
    } else {
        ResultadoFirma::NoSePudo
    })
}

/// Qué pasó al firmar. Un `bool` no bastaba: «no se pudo porque es ajena» y «no
/// se firmó porque la clave del despliegue está rota» piden respuestas distintas
/// —404 y 500—, y con un solo `false` la segunda se leía como la primera.
///
/// Ya no es `Copy`: `Incompleto` lleva QUÉ falta, y «no se pudo firmar» sin decir
/// cuál manda a ir campo por campo adivinando.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultadoFirma {
    Firmada,
    /// No existe, es ajena, o ya estaba firmada.
    NoSePudo,
    /// Hay clave configurada y no sirve. NO se marcó nada.
    ClaveInservible,
    /// Le falta contenido clínico, dicho por su nombre. Ver
    /// [`Historia::faltan_para_firmar`]. NO se marcó nada.
    Incompleto { faltan: Vec<&'static str> },
}

/// Verifica la firma de una historia contra su contenido actual. `None` si no
/// existe, es ajena o no tiene firma criptográfica; si no, distingue
/// `Valida` / `Alterada` (cambió tras firmarse) / `Invalida`.
pub async fn verificar(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    vk_hex: &str,
) -> Result<Option<guaca::firma::Verificacion>, sqlx::Error> {
    let Some(h) = obtener(pool, ambito, id).await? else {
        return Ok(None);
    };
    let Some(firma) = h.firma.clone() else {
        return Ok(None);
    };
    let anexos = resumen_de_anexos(pool, id).await?;
    Ok(Some(guaca::firma::verificar(&firmable(&h, &anexos), &firma, vk_hex)))
}

/// Borra una historia (y en cascada sus fórmulas, órdenes e imágenes). **Se niega
/// sobre una firmada** (`Firmada`): un documento legal cerrado no se elimina.
pub async fn eliminar(pool: &crate::Pool, ambito: Ambito, id: i64) -> Result<Resultado, sqlx::Error> {
    let Some((_, firmada)) = paciente_y_firma(pool, ambito, id).await? else {
        return Ok(Resultado::NoExiste);
    };
    if firmada {
        return Ok(Resultado::Firmada);
    }
    sqlx::query("DELETE FROM historia WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(Resultado::Hecho)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **LA FORMA DEL RESUMEN FIRMADO, CONGELADA — y `"v"` con ella.**
    ///
    /// Es la pareja del guardián de abajo, y hace falta porque aquél NO cubre lo
    /// que este commit vino a arreglar. Lo cazó la segunda ronda de la directiva
    /// 25, sobre `b42033aa`, con confianza 8:
    ///
    /// `toda_columna_de_los_anexos_esta_declarada` comprueba que la partición
    /// `FIRMADAS ∪ FUERA` sea **total y disjunta**. Eso cierra «nace una columna
    /// y nadie la clasifica». **No cierra** dos cosas:
    ///
    /// 1. Que alguien MUEVA una columna de `FIRMADAS` a `FUERA` con un motivo de
    ///    31 caracteres. La partición sigue siendo total y disjunta, el guardián
    ///    sigue verde, y la cobertura de la firma se reduce en silencio.
    /// 2. Que la forma cambie **sin subir `"v"`** — que es LITERALMENTE el
    ///    defecto que este commit corrige, reintroducible con el guardián puesto.
    ///
    /// De las 36 columnas firmadas, solo tres tenían prueba que las fijara
    /// (`unidad`, `procedencias`, `valor_unitario`). Esta las fija todas de una
    /// vez, junto con el orden, la anidación y la versión.
    ///
    /// **Si esta prueba se pone roja y el cambio era intencional**: sube `"v"`,
    /// actualiza el literal de aquí, y DECIDE qué pasa con las firmas ya
    /// emitidas — midiendo cuántas hay, no suponiéndolo. El 2026-08-16 se midió
    /// en el VPS (`sqlite3 -readonly 'file:…?immutable=1'`, con control positivo:
    /// la base tiene 1 institución, 1 usuario y 1 paciente del sembrado) y había
    /// **0 historias y 0 firmas**. Ese cero es lo que hizo barato el salto de
    /// `v1` a `v2`; el comentario anterior afirmaba lo mismo del 2026-08-10 y
    /// nadie lo volvió a comprobar antes de cambiar la forma.
    #[tokio::test]
    async fn la_forma_del_resumen_firmado_esta_congelada() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        // Valores EXPLÍCITOS en todas las columnas con `DEFAULT datetime('now')`:
        // una fecha que ponga el reloj haría la prueba inestable, y una prueba
        // inestable se acaba marcando como `#[ignore]`.
        sqlx::query(
            "INSERT INTO formula (id, historia_id, medicamento, codigo_cum, dosis, cantidad, \
             unidad, dias_tratamiento, observaciones, fecha_formula, dispensada, \
             tipo_medicamento, valor_unitario, valor_servicio, concepto_recaudo, \
             valor_pago_moderador, num_fev_pago_moderador, procedencias) \
             VALUES (1, ?, 'Metotrexato', '20012345-1', '1 c/8h', 10.5, 'mg', 7, 'con comida', \
             '2026-03-01 08:00:00', 1, '01', 4800, 50400, '05', 0, 'FEV-1', '{}')",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO orden (id, historia_id, fecha_orden, estado, tipo, codigo_cups, \
             descripcion, justificacion, especialidad_destino_id, institucion_destino_id, \
             via_ingreso_servicio, grupo_servicios, cod_servicio, valor_servicio, \
             concepto_recaudo, valor_pago_moderador, num_fev_pago_moderador) \
             VALUES (1, ?, '2026-03-01 09:00:00', 'Pendiente', 'Laboratorio', '902210', \
             'Hemograma', 'control', NULL, NULL, '01', '01', 328, 12000, '05', 0, 'FEV-2')",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO imagen_diagnostica (id, historia_id, ruta_archivo, nombre_original, \
             tipo_mime, tamano_bytes, descripcion, fecha_subida, cifrada, origen) \
             VALUES (1, ?, '/x/1.enc', 'placa.jpg', 'image/jpeg', 4096, 'tórax PA', \
             '2026-03-01 10:00:00', 1, 'adjunto_historia')",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
        // Y una fila que NO debe entrar: la placa que sube imagenología después.
        // Sin ella, la prueba no distinguiría entre «el filtro de origen está» y
        // «no hay filtro pero tampoco hay resultados».
        sqlx::query(
            "INSERT INTO imagen_diagnostica (id, historia_id, ruta_archivo, nombre_original, \
             tipo_mime, tamano_bytes, descripcion, fecha_subida, cifrada, origen) \
             VALUES (2, ?, '/x/2.enc', 'resultado.pdf', 'application/pdf', 8192, 'informe', \
             '2026-03-02 10:00:00', 1, 'resultado_imagenologia')",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

        let r = resumen_de_anexos(&pool, id).await.unwrap();
        let bytes = serde_json::to_string(&r).unwrap();
        assert_eq!(
            bytes,
            concat!(
                r#"{"adjuntos":[[1,"placa.jpg","image/jpeg",4096,"tórax PA"]],"#,
                r#""formulas":[[1,"Metotrexato","20012345-1","1 c/8h",10.5,"mg",7,"con comida","#,
                r#""2026-03-01 08:00:00","01",4800,50400,"05",0,"FEV-1","{}"]],"#,
                r#""ordenes":[[1,"Laboratorio","902210","Hemograma","control",null,null,"#,
                r#""2026-03-01 09:00:00","01","01",328,12000,"05",0,"FEV-2"]],"v":2}"#,
            ),
            "LA FORMA DE LO FIRMADO CAMBIÓ. Si es intencional: sube `\"v\"`, \
             actualiza este literal, y decide qué pasa con las firmas ya emitidas \
             MIDIENDO cuántas hay. Si no lo es, acabas de reducir en silencio lo \
             que la firma cubre."
        );
    }

    /// **TODA COLUMNA DE LOS ANEXOS ESTÁ DECLARADA: o se firma, o se excluye con
    /// su motivo.**
    ///
    /// Es el guardián que faltaba, y su ausencia es la causa raíz de CUATRO
    /// olvidos en la misma lista: `unidad` y `observaciones` (revisión del
    /// cierre), `procedencias` (directiva 25 sobre `9f8c147d`) y las TRECE
    /// columnas de dinero de la migración 0041 (directiva 25 sobre `1c570350`).
    /// No eran cuatro defectos sino uno: nadie vigilaba la lista, mientras
    /// `como_nueva` sí tiene el suyo desde el principio.
    ///
    /// El modo de fallo que cierra es el que no da error: una columna nueva
    /// entra por una migración, nadie se acuerda de esta función, y un `UPDATE`
    /// contra la base la cambia sin romper la firma. Aquí se pone roja el día
    /// que la columna nace, que es cuando la decisión se puede tomar bien.
    ///
    /// La aguja sale del ARTEFACTO —`PRAGMA table_info` sobre la base ya
    /// migrada—, no de una lista copiada que envejecería con el mismo silencio.
    #[tokio::test]
    async fn toda_columna_de_los_anexos_esta_declarada() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        for (tabla, firmadas, fuera) in [
            ("formula", FIRMADAS_FORMULA, FUERA_FORMULA),
            ("orden", FIRMADAS_ORDEN, FUERA_ORDEN),
            ("imagen_diagnostica", FIRMADAS_IMAGEN, FUERA_IMAGEN),
        ] {
            let reales: Vec<String> = sqlx::query_scalar(&format!(
                "SELECT name FROM pragma_table_info('{tabla}')"
            ))
            .fetch_all(&pool)
            .await
            .unwrap();

            // POSITIVO DE CONTROL: si el `PRAGMA` devolviera vacío —tabla
            // renombrada, migración que no corrió— el bucle de abajo no
            // compararía nada y la prueba saldría verde sin haber mirado.
            assert!(
                reales.len() >= 4,
                "`{tabla}` devolvió {} columnas: el PRAGMA no llegó a leer la tabla",
                reales.len()
            );

            for c in &reales {
                let firmada = firmadas.contains(&c.as_str());
                let excluida = fuera.iter().any(|(n, _)| n == c);
                assert!(
                    firmada || excluida,
                    "`{tabla}.{c}` no está ni en la lista FIRMADA ni en la de EXCLUIDAS. \
                     Decide cuál de las dos y escríbelo: si es contenido del acto clínico \
                     va a la firma; si no, va a las excluidas CON SU MOTIVO. Dejarla sin \
                     clasificar es la cuarta vez que pasa (`unidad`, `observaciones`, \
                     `procedencias`, y las trece de la 0041)."
                );
                assert!(
                    !(firmada && excluida),
                    "`{tabla}.{c}` está en las DOS listas: una de las dos miente"
                );
            }

            // Y la dirección contraria: una lista que nombre una columna que ya
            // no existe haría que el `SELECT` reventara en producción al firmar,
            // no aquí.
            for c in firmadas.iter().chain(fuera.iter().map(|(n, _)| n)) {
                assert!(
                    reales.iter().any(|r| r == c),
                    "`{tabla}.{c}` está declarada y NO existe en el esquema"
                );
            }

            // Un motivo en blanco no es un motivo (directiva 20).
            for (n, motivo) in fuera {
                assert!(
                    motivo.len() > 30,
                    "`{tabla}.{n}` se excluye sin motivo escrito: «{motivo}»"
                );
            }
        }
    }

    // ── Una historia firmada no se edita POR NINGUNA PUERTA ─────────────────
    //
    // La regla estaba escrita en el CLAUDE.md y aplicada a medias: de las seis
    // puertas del panel la cumplían tres, y en la API ninguna. Lo midió la
    // revisión de seguridad del 2026-08-10. Estas pruebas van sobre la capa de
    // datos, que es por donde pasan obligatoriamente las DOCE.

    /// Base con una historia abierta del paciente 10 (institución 1).
    async fn base_con_historia() -> (crate::Pool, i64) {
        let pool = base().await;
        let id = crear(&pool, Ambito::Institucion(1), 5, &nueva(10))
            .await
            .unwrap()
            .expect("la historia se crea");
        (pool, id)
    }

    /// La firma, por la vía real: el mismo `firmar` del producto.
    async fn firmar_la(pool: &crate::Pool, id: i64) {
        sqlx::query("UPDATE historia SET firmada = 1 WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_una_historia_firmada_no_se_le_cuelga_nada() {
        let (pool, id) = base_con_historia().await;
        firmar_la(&pool, id).await;
        assert_eq!(
            admite_escritura(&pool, Ambito::Institucion(1), id).await.unwrap(),
            Resultado::Firmada
        );
    }

    #[tokio::test]
    async fn a_una_historia_abierta_si() {
        // La pareja. Sin ella, una regla que devolviera `Firmada` siempre
        // pasaría igual y cerraría el trabajo clínico normal.
        let (pool, id) = base_con_historia().await;
        assert_eq!(
            admite_escritura(&pool, Ambito::Institucion(1), id).await.unwrap(),
            Resultado::Hecho
        );
    }

    #[tokio::test]
    async fn una_historia_ajena_sigue_dando_no_existe_y_no_firmada() {
        // El orden importa: si `Firmada` ganara a `NoExiste`, saber que una
        // historia está firmada sería un oráculo de que EXISTE, y con eso se
        // enumera. 404 y nunca otra cosa para lo ajeno.
        let (pool, id) = base_con_historia().await;
        firmar_la(&pool, id).await;
        assert_eq!(
            admite_escritura(&pool, Ambito::Institucion(2), id).await.unwrap(),
            Resultado::NoExiste
        );
    }

    /// **`como_nueva` no puede dejarse una columna, y por eso no se comprueba
    /// leyéndola.**
    ///
    /// Es el guardián de un fallo que no da error: si mañana entra
    /// `ef_genitales` en `Historia` y en `HistoriaNueva` y nadie la añade a
    /// `como_nueva`, el panel guardará la sección del diagnóstico y **borrará el
    /// examen genital** de esa historia, porque `actualizar` reemplaza las 61
    /// columnas de una vez. No falla nada; el dato deja de estar.
    ///
    /// Se mide sobre las estructuras SERIALIZADAS, que es la única forma de
    /// preguntarle a Rust por los nombres de los campos, y la diferencia se
    /// juzga contra [`SOLO_DEL_SERVIDOR`], donde cada ausencia legítima está
    /// escrita con su motivo.
    #[tokio::test]
    async fn como_nueva_lleva_todas_las_columnas_editables() {
        let h = historia_de_muestra().await;
        let de_historia = claves(serde_json::to_value(&h).unwrap());
        let de_nueva = claves(serde_json::to_value(como_nueva(&h)).unwrap());

        let solo_del_servidor: std::collections::BTreeSet<String> =
            SOLO_DEL_SERVIDOR.iter().map(|(c, _)| (*c).to_string()).collect();
        let faltan: Vec<_> = de_historia.difference(&de_nueva).cloned().collect();
        let faltan_sin_motivo: Vec<_> =
            faltan.iter().filter(|c| !solo_del_servidor.contains(*c)).collect();
        assert!(
            faltan_sin_motivo.is_empty(),
            "estas columnas se perderían al guardar una sección: {faltan_sin_motivo:?}"
        );
        // La pareja, por el otro lado: una exención que ya no corresponde a
        // ninguna columna ausente sobra, y una lista de exenciones que crece
        // sola es la que acaba tapando la que sí importa (directiva 23).
        let sobran: Vec<_> = solo_del_servidor.iter().filter(|c| !faltan.contains(c)).collect();
        assert!(sobran.is_empty(), "exenciones que ya no hacen falta: {sobran:?}");
        // CONTROL: las dos listas tienen contenido. Con dos conjuntos vacíos
        // todo lo de arriba pasa igual y no se mide nada.
        assert!(de_historia.len() > 60, "{}", de_historia.len());
        assert_eq!(faltan.len(), SOLO_DEL_SERVIDOR.len());
    }

    /// Y su otra mitad: los VALORES viajan, no solo los nombres. Un
    /// `como_nueva` que pusiera `None` en todo pasaría el guardián de arriba.
    #[tokio::test]
    async fn como_nueva_conserva_lo_que_habia_escrito() {
        let h = historia_de_muestra().await;
        let n = como_nueva(&h);
        assert_eq!(n.motivo_consulta, h.motivo_consulta);
        assert_eq!(n.ef_cabeza, h.ef_cabeza);
        assert_eq!(n.peso, h.peso);
        assert_eq!(n.tension_arterial, h.tension_arterial);
        assert_eq!(n.genera_incapacidad, Some(h.genera_incapacidad));
        assert_eq!(n.valor_pago_moderador, Some(h.valor_pago_moderador));
        assert_eq!(n.finalidad, Some(h.finalidad.clone()));
        assert_eq!(n.diagnostico_principal, h.diagnostico_principal);
        // Y el código de la historia viaja: sin él, `actualizar` lo dejaría
        // igual —no está en su `UPDATE`—, pero `crear` lo inventaría de nuevo.
        assert_eq!(n.codigo_historia, Some(h.codigo_historia));
    }

    /// Una historia REAL, creada e insertada: medir sobre una estructura hecha
    /// a mano mediría el arnés, no la fila que el producto escribe.
    async fn historia_de_muestra() -> Historia {
        let pool = base().await;
        let id = crear(&pool, Ambito::Institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        obtener(&pool, Ambito::Institucion(1), id).await.unwrap().unwrap()
    }

    fn claves(v: serde_json::Value) -> std::collections::BTreeSet<String> {
        v.as_object().expect("un objeto").keys().cloned().collect()
    }


    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query("INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) VALUES (5,'Dra','d@x.co','h','medico',1,1),(6,'Dr Dos','d2@x.co','h','medico',2,1),(7,'Retirada','r@x.co','h','medico',1,0)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,fecha_nacimiento,sexo,institucion_id) VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1),(20,'CC','2','Beto','Paz','1985-01-01','M',2)")
            .execute(&pool).await.unwrap();
        pool
    }

    fn nueva(paciente_id: i64) -> HistoriaNueva {
        nueva_con(paciente_id, 5)
    }

    fn nueva_con(paciente_id: i64, medico_id: i64) -> HistoriaNueva {
        HistoriaNueva {
            codigo_historia: None,
            paciente_id,
            medico_id,
            cita_id: None,
            acomp_nombre: None, acomp_parentesco: None, acomp_telefono: None, acomp_documento: None,
            motivo_consulta: "Dolor".into(),
            enfermedad_actual: "Desde ayer".into(),
            tension_arterial: Some("120/80".into()), pam: None,
            frecuencia_cardiaca: Some(72), frecuencia_respiratoria: None, pulso: None,
            temperatura: Some(36.8), pabd: None, pc: None, peso: Some(70.0), talla: Some(170.0),
            imc: None, peso_ideal: None, talla_ideal: None, estado_nutricional: None,
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

    /// El relleno de los tres campos del RIPS no puede volver (#250, tarea de
    /// la finalidad).
    ///
    /// `finalidad`, `causa_externa` y `tipo_diagnostico` se escribían con «10»,
    /// «13» y «01» cuando el llamante no mandaba nada — y no los manda nunca,
    /// porque ninguna pantalla ofrece esos campos. Dos de esos tres valores ni
    /// siquiera existen en las tablas de la Resolución 948 (la de finalidad va
    /// del 11 al 44; la de causa/motivo, del 21 al 49), así que el archivo salía
    /// con un código que el validador rechaza sin que nada fallara aquí.
    ///
    /// Se guarda el hueco. El `DEFAULT '10'` del DDL sigue en la tabla —quitarlo
    /// exige reconstruir ochenta columnas— pero no se alcanza mientras estos
    /// `bind` escriban la columna siempre. Esta prueba es lo que lo ata.
    #[tokio::test]
    async fn sin_finalidad_se_guarda_el_hueco_y_no_el_relleno() {
        let pool = base().await;
        let mut h = nueva(10);
        h.finalidad = None;
        h.causa_externa = None;
        h.tipo_diagnostico = None;
        let id = crear(&pool, Ambito::institucion(1), 5, &h).await.unwrap().unwrap();

        let leida = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(leida.finalidad, "", "«10» no está en la tabla del MUV");
        assert_eq!(leida.causa_externa, "", "«13» tampoco");
        assert_eq!(leida.tipo_diagnostico, "");
    }

    /// Y lo que sí viene, se guarda tal cual: el hueco es para la ausencia, no
    /// una manera de ignorar lo que el médico eligió.
    #[tokio::test]
    async fn la_finalidad_elegida_se_guarda_tal_cual() {
        let pool = base().await;
        let mut h = nueva(10);
        h.finalidad = Some("11".into());
        h.causa_externa = Some("38".into());
        let id = crear(&pool, Ambito::institucion(1), 5, &h).await.unwrap().unwrap();

        let leida = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(leida.finalidad, "11");
        assert_eq!(leida.causa_externa, "38");
    }

    #[tokio::test]
    async fn crear_y_obtener_conserva_los_campos() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        let h = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(h.motivo_consulta, "Dolor");
        assert_eq!(h.diagnostico_principal, "J00");
        assert_eq!(h.tension_arterial.as_deref(), Some("120/80"));
        assert_eq!(h.frecuencia_cardiaca, Some(72));
        assert_eq!(h.peso, Some(70.0));
        // Lo que NO se rellena: los tres campos del RIPS que ninguna pantalla
        // ofrece. Ver `sin_finalidad_se_guarda_el_hueco_y_no_el_relleno`.
        assert_eq!(h.finalidad, "");
        assert_eq!(h.causa_externa, "");
        assert_eq!(h.tipo_diagnostico, "");
        // Este sí es un defecto de verdad: la mayoría de las atenciones no
        // llevan copago, y cero aquí es un dato, no un hueco.
        assert_eq!(h.valor_pago_moderador, 0);
        assert_eq!(h.datos_dinamicos, "{}");
        assert!(!h.genera_incapacidad);
        assert!(h.codigo_historia.starts_with("HC-10-"));
    }

    #[tokio::test]
    async fn listar_por_paciente_devuelve_las_suyas() {
        let pool = base().await;
        crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap();
        assert_eq!(listar_por_paciente(&pool, Ambito::institucion(1), 10).await.unwrap().len(), 1);
    }

    /// **El hallazgo de la security-review del 2026-07-30.** El paciente se
    /// validaba contra el ámbito y el MÉDICO no: `medico_id` llegaba en el cuerpo
    /// y se ataba al INSERT con la sola clave foránea a `usuario(id)`, que abarca
    /// todas las instituciones del despliegue.
    ///
    /// Lo grave no era leer: era que la historia se cierra con `firmar`, que pone
    /// una firma criptográfica sobre un contenido que NOMBRA al profesional. Una
    /// institución podía dejar firmado un acto clínico a nombre de un médico de
    /// otra — que no puede verlo ni negarlo.
    /// Quién REGISTRA sale de la sesión y no del cuerpo, y es un dato distinto de
    /// a quién se ATRIBUYE el acto. Con un solo campo había que mentir en uno de
    /// los dos escenarios: el médico que escribe su nota, y el auxiliar que la
    /// transcribe por él.
    #[tokio::test]
    async fn se_guarda_quien_registro_aunque_no_sea_a_quien_se_atribuye() {
        let pool = base().await;
        // La 5 atiende; la registra otro usuario de la misma institución.
        let id = crear(&pool, Ambito::institucion(1), 7, &nueva_con(10, 5))
            .await
            .unwrap()
            .unwrap();
        let h = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(h.medico_id, 5, "se atribuye a quien atendió");
        assert_eq!(h.creado_por, Some(7), "y consta quién la escribió");
    }

    /// **El segundo hallazgo.** `firmar` no registraba al firmante: una firma que
    /// no dice quién firmó no acredita a nadie. Y no basta con guardarlo en una
    /// columna: tiene que ir DENTRO de lo firmado, o cambiarlo después no rompería
    /// nada y la firma seguiría sin atar el documento a su autor.
    #[tokio::test]
    async fn la_firma_dice_quien_firmo_y_alterarlo_la_rompe() {
        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();

        assert_eq!(
            firmar(&pool, Ambito::institucion(1), id, 5, Some(&sk)).await.unwrap(),
            ResultadoFirma::Firmada
        );
        let h = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(h.firmado_por, Some(5), "la firma no dice quién firmó");
        assert!(h.firmado_en.is_some(), "ni cuándo");

        // Con el firmante intacto, verifica.
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Valida)
        );

        // Y si alguien cambia el firmante en la base, la firma se delata. Esto es
        // lo que separa «guardar quién firmó» de «que la firma lo acredite».
        sqlx::query("UPDATE historia SET firmado_por = 6 WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Alterada),
            "cambiar el firmante no rompió la firma: entonces no lo acreditaba"
        );
    }

    /// LA FIRMA CUBRE LOS ANEXOS, no solo la fila.
    ///
    /// Hasta el 2026-08-10 no lo hacía: se podía borrar un medicamento de una
    /// historia firmada y `verificar` seguía diciendo `Valida`. Cerrar las doce
    /// puertas de la aplicación impide hacerlo desde el producto; esto es lo que
    /// lo DETECTA cuando se hace contra la base o sobre un respaldo — que es la
    /// vía por la que llegaría una alteración de verdad, y la capa que responde
    /// ante un tercero (Ley 527/1999).
    ///
    /// El caso rojo va por SQL directo A PROPÓSITO: por la aplicación ya no se
    /// puede, así que probarlo por ahí mediría la puerta y no la firma.
    #[tokio::test]
    async fn borrar_una_formula_de_una_historia_firmada_rompe_la_firma() {
        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        sqlx::query(
            "INSERT INTO formula (id, historia_id, medicamento, dosis, cantidad) \
             VALUES (1, ?, 'Ibuprofeno', '1 c/8h', 10)",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            firmar(&pool, Ambito::institucion(1), id, 5, Some(&sk)).await.unwrap(),
            ResultadoFirma::Firmada
        );
        // Control positivo: recién firmada, verifica. Sin esto, una firma que
        // fallara SIEMPRE pasaría la mitad de abajo.
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Valida)
        );

        sqlx::query("DELETE FROM formula WHERE id = 1").execute(&pool).await.unwrap();
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Alterada),
            "se borró la prescripción y la firma no se enteró"
        );
    }

    #[tokio::test]
    async fn cambiar_la_unidad_de_una_formula_firmada_rompe_la_firma() {
        // El caso silencioso que encontró la revisión del cierre: `unidad` y
        // `observaciones` no estaban en el sello, así que un UPDATE que cambia
        // «mg» por «g» multiplicaba la dosis por mil y `verificar-firma` seguía
        // diciendo Valida. Cambiar SOLO esa columna es lo que discrimina.
        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        sqlx::query(
            "INSERT INTO formula (id, historia_id, medicamento, dosis, cantidad, unidad) \
             VALUES (1, ?, 'Metotrexato', '1 c/8h', 10, 'mg')",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
        firmar(&pool, Ambito::institucion(1), id, 5, Some(&sk)).await.unwrap();
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Valida)
        );

        sqlx::query("UPDATE formula SET unidad = 'g' WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Alterada),
            "la dosis quedó multiplicada por mil y la firma no se enteró"
        );
    }

    /// **CAMBIAR LA PROCEDENCIA DE UNA FÓRMULA FIRMADA ROMPE LA FIRMA.**
    ///
    /// La gemela de la de arriba, para la columna que añadió la 0060. Lo cazó la
    /// revisión de la directiva 25 sobre `9f8c147d`: `resumen_de_anexos`
    /// seleccionaba las ocho columnas de siempre y `procedencias` no estaba.
    ///
    /// La asimetría era lo que lo hacía hallazgo y no gusto: `historia.procedencias`
    /// SÍ quedaba firmada —va dentro de la `Historia` que serializa `firmable`— y la
    /// del medicamento no. El mismo dato, con el mismo valor probatorio, sellado en
    /// el AC y suelto en el AM: un `UPDATE` contra la base convertía un medicamento
    /// sin respaldo en uno plenamente afirmado, el cierre dejaba de marcarlo, y
    /// `verificar-firma` seguía contestando `Valida`.
    ///
    /// El modelo de amenaza es el que esta misma función declara por escrito:
    /// cerrar las puertas de la aplicación impide hacerlo DESDE EL PRODUCTO, no
    /// contra la base ni sobre un respaldo — y quien responde de eso ante un juez
    /// es la firma, no el manejador.
    #[tokio::test]
    async fn cambiar_la_procedencia_de_una_formula_firmada_rompe_la_firma() {
        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        sqlx::query(
            "INSERT INTO formula (id, historia_id, medicamento, dosis, cantidad, unidad, procedencias) \
             VALUES (1, ?, 'Metotrexato', '1 c/8h', 10, 'mg', '{}')",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
        firmar(&pool, Ambito::institucion(1), id, 5, Some(&sk)).await.unwrap();
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Valida)
        );

        // Fabricar el aval que nadie dio: es el sentido útil del ataque —el
        // medicamento pasa de «nadie lo afirmó» a «un humano respondió por él».
        sqlx::query(
            "UPDATE formula SET procedencias = '{\"codigo_cum\":\"confirmado\"}' WHERE id = 1",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Alterada),
            "se fabricó un aval humano sobre un medicamento firmado y la firma no se enteró"
        );
    }

    /// **CAMBIAR EL DINERO DE UNA FÓRMULA FIRMADA ROMPE LA FIRMA**, y su pareja:
    /// dispensarla no.
    ///
    /// La tercera gemela, por las TRECE columnas de la migración 0041 que la
    /// revisión de la directiva 25 sobre `1c570350` encontró fuera de la firma.
    /// `valor_unitario` es la que más duele —es lo que se le cobra a la EPS, y
    /// `rips.rs` la lee EN VIVO al cerrar el mes, sin copia congelada—, así que
    /// un `UPDATE` sobre un respaldo cambiaba la cuenta y `verificar-firma`
    /// seguía diciendo `Valida`.
    ///
    /// La pareja es lo que impide cerrar de más: `dispensada` la mueve farmacia
    /// DESPUÉS de firmar, y si entrara en la firma, dispensar un medicamento
    /// invalidaría la consulta — la operación más normal que hay.
    #[tokio::test]
    async fn cambiar_el_valor_de_una_formula_firmada_rompe_la_firma_y_dispensarla_no() {
        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        sqlx::query(
            "INSERT INTO formula (id, historia_id, medicamento, dosis, cantidad, valor_unitario) \
             VALUES (1, ?, 'Metotrexato', '1 c/8h', 10, 4800)",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
        firmar(&pool, Ambito::institucion(1), id, 5, Some(&sk)).await.unwrap();
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Valida)
        );

        // VERDE: dispensar es flujo posterior y NO puede romper nada.
        sqlx::query("UPDATE formula SET dispensada = 1 WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Valida),
            "dispensar un medicamento no puede invalidar la consulta que lo formuló"
        );

        // ROJO: mil veces el precio, sobre una historia cerrada.
        sqlx::query("UPDATE formula SET valor_unitario = 4800000 WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Alterada),
            "se multiplicó por mil lo que se le cobra a la EPS y la firma no se enteró"
        );
    }

    #[tokio::test]
    async fn la_placa_del_radiologo_no_rompe_la_firma_de_la_consulta() {
        // La pareja del origen (migración 0056): el resultado de imagenología
        // llega DESPUÉS y es lo normal. Si rompiera la firma, subir una placa
        // invalidaría una historia firmada legítimamente — ruido que enseña a
        // ignorar la verificación.
        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        firmar(&pool, Ambito::institucion(1), id, 5, Some(&sk)).await.unwrap();

        sqlx::query(
            "INSERT INTO imagen_diagnostica (historia_id, ruta_archivo, nombre_original, \
             tipo_mime, tamano_bytes, origen) \
             VALUES (?, '1.dat', 'placa.jpg', 'image/jpeg', 1000, 'resultado_imagenologia')",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Valida),
            "la placa posterior no puede invalidar la consulta"
        );

        // Y su contrario: un ADJUNTO de la historia sí la rompe.
        sqlx::query(
            "INSERT INTO imagen_diagnostica (historia_id, ruta_archivo, nombre_original, \
             tipo_mime, tamano_bytes, origen) \
             VALUES (?, '2.dat', 'metida.jpg', 'image/jpeg', 1000, 'adjunto_historia')",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Alterada)
        );
    }

    #[tokio::test]
    async fn anadir_una_orden_a_una_historia_firmada_tambien_la_rompe() {
        // La otra mitad: quitar y PONER. Una firma que solo detectara borrados
        // dejaría colar una orden que el médico nunca escribió.
        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        firmar(&pool, Ambito::institucion(1), id, 5, Some(&sk)).await.unwrap();
        sqlx::query(
            "INSERT INTO orden (id, historia_id, tipo, descripcion, justificacion) \
             VALUES (1, ?, 'Laboratorio', 'Hemograma', 'control')",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Alterada)
        );
    }

    #[tokio::test]
    async fn lo_que_no_es_contenido_de_la_historia_no_rompe_la_firma() {
        // LA PAREJA que hace que esto discrimine. Sin ella, un resumen que
        // metiera medio esquema pasaría igual y convertiría la verificación en
        // ruido: una factura emitida después, o un consentimiento firmado
        // aparte, no alteran el acto clínico y no pueden invalidarlo.
        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        firmar(&pool, Ambito::institucion(1), id, 5, Some(&sk)).await.unwrap();

        // Una fórmula de OTRA historia tampoco: el resumen va acotado a la suya.
        let otra = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        sqlx::query(
            "INSERT INTO formula (id, historia_id, medicamento, dosis, cantidad) \
             VALUES (99, ?, 'Ajena', '1', 1)",
        )
        .bind(otra)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Valida),
            "algo de otra historia invalidó esta firma"
        );
    }

    /// Una clave configurada e ilegible NO deja la historia marcada como firmada.
    /// Antes daba `firma = NULL` con `firmada = 1`: un registro que afirmaba estar
    /// firmado sin estarlo, que es peor que uno sin firmar.
    #[tokio::test]
    async fn una_clave_rota_no_marca_la_historia_como_firmada() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();

        assert_eq!(
            firmar(&pool, Ambito::institucion(1), id, 5, Some("no-es-una-clave")).await.unwrap(),
            ResultadoFirma::ClaveInservible
        );
        let h = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert!(!h.firmada, "quedó marcada como firmada sin firma");
        assert!(h.firma.is_none());

        // Y sin clave configurada sí se cierra: la ausencia de firma nunca impide
        // cerrar una historia. Son dos ausencias distintas y se distinguen.
        assert_eq!(
            firmar(&pool, Ambito::institucion(1), id, 5, None).await.unwrap(),
            ResultadoFirma::Firmada
        );
    }

    #[tokio::test]
    async fn no_se_atribuye_una_historia_a_un_medico_ajeno() {
        let pool = base().await;
        // Paciente propio (10, de la 1) y médico de la OTRA institución (6).
        assert!(
            crear(&pool, Ambito::institucion(1), 5, &nueva_con(10, 6)).await.unwrap().is_none(),
            "se creó una historia a nombre de un médico de otra institución"
        );
        // Y con el suyo, sí.
        assert!(crear(&pool, Ambito::institucion(1), 5, &nueva_con(10, 5)).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn no_se_atribuye_a_un_medico_dado_de_baja_ni_a_uno_inexistente() {
        let pool = base().await;
        // El 7 es de la institución 1 pero está inactivo: crear un registro nuevo
        // a nombre de quien ya no ejerce no es un caso legítimo de nadie.
        assert!(crear(&pool, Ambito::institucion(1), 5, &nueva_con(10, 7)).await.unwrap().is_none());
        // Y un id que no existe da lo MISMO que uno ajeno, no un error distinto:
        // si se distinguieran, esto sería un oráculo de qué ids existen.
        assert!(crear(&pool, Ambito::institucion(1), 5, &nueva_con(10, 9999)).await.unwrap().is_none());
    }

    /// No se crea ni se ve la historia de un paciente de otra institución.
    #[tokio::test]
    async fn el_paciente_ajeno_no_deja_crear_ni_ver() {
        let pool = base().await;
        // La 1 no puede crear una historia para el paciente 20 (de la 2).
        assert!(crear(&pool, Ambito::institucion(1), 5, &nueva(20)).await.unwrap().is_none());
        // Creada por la 2, la 1 no la ve.
        let id = crear(&pool, Ambito::institucion(2), 6, &nueva_con(20, 6)).await.unwrap().unwrap();
        assert!(obtener(&pool, Ambito::institucion(1), id).await.unwrap().is_none());
        assert!(listar_por_paciente(&pool, Ambito::institucion(1), 20).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn sin_ambito_no_ve_ni_crea() {
        let pool = base().await;
        assert!(crear(&pool, Ambito::Ninguno, 5, &nueva(10)).await.unwrap().is_none());
        assert!(listar_por_paciente(&pool, Ambito::Ninguno, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn una_historia_nueva_no_esta_firmada() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        let h = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert!(!h.firmada);
    }

    #[tokio::test]
    async fn actualizar_cambia_los_campos_editables() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        let mut edit = nueva(10);
        edit.motivo_consulta = "Control".into();
        edit.diagnostico_principal = "I10".into();
        assert_eq!(actualizar(&pool, Ambito::institucion(1), id, &edit).await.unwrap(), Resultado::Hecho);
        let h = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(h.motivo_consulta, "Control");
        assert_eq!(h.diagnostico_principal, "I10");
    }

    #[tokio::test]
    async fn la_historia_ajena_no_se_edita_ni_se_borra() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(2), 6, &nueva_con(20, 6)).await.unwrap().unwrap();
        assert_eq!(actualizar(&pool, Ambito::institucion(1), id, &nueva(20)).await.unwrap(), Resultado::NoExiste);
        assert_eq!(eliminar(&pool, Ambito::institucion(1), id).await.unwrap(), Resultado::NoExiste);
        // Y sigue existiendo para su dueña.
        assert!(obtener(&pool, Ambito::institucion(2), id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn una_historia_firmada_es_inmutable() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        assert_eq!(firmar(&pool, Ambito::institucion(1), id, 5, None).await.unwrap(), ResultadoFirma::Firmada);
        // Firmar de nuevo no cambia nada.
        assert_eq!(firmar(&pool, Ambito::institucion(1), id, 5, None).await.unwrap(), ResultadoFirma::NoSePudo);
        // Ni se edita ni se borra: se responde Firmada, no NoExiste.
        assert_eq!(actualizar(&pool, Ambito::institucion(1), id, &nueva(10)).await.unwrap(), Resultado::Firmada);
        assert_eq!(eliminar(&pool, Ambito::institucion(1), id).await.unwrap(), Resultado::Firmada);
        let h = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert!(h.firmada);
    }

    /// **Una historia en blanco se GUARDA y no se FIRMA**, y las dos mitades son
    /// la decisión (hermana de `un_informe_en_blanco_se_guarda_pero_no_se_firma`).
    ///
    /// El borrador es trabajo a medias —se abre la historia con el motivo, entra
    /// el paciente, se explora, y el diagnóstico se escribe al final—, así que
    /// bloquear el guardado enseñaría a teclear un punto para poder salvar. Lo
    /// que no puede existir es una historia FIRMADA que no diga nada.
    #[tokio::test]
    async fn una_historia_en_blanco_se_guarda_pero_no_se_firma() {
        let pool = base().await;
        let mut n = nueva(10);
        n.motivo_consulta = String::new();
        n.diagnostico_principal = String::new();
        n.plan_tratamiento = String::new();
        // Se guarda: es un borrador, y es LO QUE HACE el panel al abrirla.
        let id = crear(&pool, Ambito::institucion(1), 5, &n).await.unwrap().unwrap();

        let r = firmar(&pool, Ambito::institucion(1), id, 5, None).await.unwrap();
        assert_eq!(
            r,
            ResultadoFirma::Incompleto {
                faltan: vec!["el motivo de consulta", "el diagnóstico principal", "el plan de tratamiento"]
            },
            "se firmó una historia en blanco, o no se dijo qué falta"
        );
        // Y NO quedó marcada: un rechazo que igual marca es peor que no rechazar.
        let h = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert!(!h.firmada && h.firma.is_none(), "quedó firmada a pesar del rechazo");

        // **EL VACIADO DELIBERADO**, que es el caso por el que la puerta va en la
        // firma y no en el guardado: se escribe completo y luego se borra un
        // campo, y tiene que quedar vacío —`actualizar` no usa `COALESCE`—.
        assert_eq!(actualizar(&pool, Ambito::institucion(1), id, &nueva(10)).await.unwrap(), Resultado::Hecho);
        let mut sin_plan = nueva(10);
        sin_plan.plan_tratamiento = String::new();
        assert_eq!(actualizar(&pool, Ambito::institucion(1), id, &sin_plan).await.unwrap(), Resultado::Hecho);
        let h = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(h.plan_tratamiento, "", "el vaciado deliberado no se guardó");
        assert_eq!(
            firmar(&pool, Ambito::institucion(1), id, 5, None).await.unwrap(),
            ResultadoFirma::Incompleto { faltan: vec!["el plan de tratamiento"] },
            "se firmó sin plan, o se nombró un campo que sí estaba"
        );

        // LA PAREJA: completa, se firma. Sin esto, un servidor que rechazara
        // siempre pasaría todo lo de arriba.
        assert_eq!(actualizar(&pool, Ambito::institucion(1), id, &nueva(10)).await.unwrap(), Resultado::Hecho);
        assert_eq!(
            firmar(&pool, Ambito::institucion(1), id, 5, None).await.unwrap(),
            ResultadoFirma::Firmada,
            "no se firmó una historia completa"
        );
    }

    /// **`enfermedad_actual` NO impide firmar**, y esta prueba es la que fija la
    /// elección de la lista (directiva 21).
    ///
    /// `HistoriaNueva` la declara entre los clínicamente obligatorios, pero un
    /// control de crecimiento y desarrollo o un control prenatal **no tienen
    /// enfermedad**: exigirla enseña a escribir «ninguna», que es el dato falso
    /// que este repositorio persigue. Su mutante —añadirla a la lista— muere
    /// aquí, y solo aquí.
    #[tokio::test]
    async fn el_control_del_sano_se_firma_sin_enfermedad_actual() {
        let pool = base().await;
        let mut n = nueva(10);
        n.enfermedad_actual = String::new();
        n.motivo_consulta = "Control de crecimiento y desarrollo".into();
        n.diagnostico_principal = "Z001".into(); // control de salud de rutina del niño
        n.plan_tratamiento = "Próximo control en 6 meses".into();
        let id = crear(&pool, Ambito::institucion(1), 5, &n).await.unwrap().unwrap();
        assert_eq!(
            firmar(&pool, Ambito::institucion(1), id, 5, None).await.unwrap(),
            ResultadoFirma::Firmada,
            "no se pudo cerrar el control de un niño sano"
        );
    }

    /// **Los espacios no son contenido.** El camino real es `/api`, que escribe
    /// la columna tal cual —el panel recorta antes—, así que un `"   "` llega a
    /// la base y `faltan_para_firmar` es lo único que lo juzga.
    #[tokio::test]
    async fn un_espacio_no_pasa_por_diagnostico() {
        let pool = base().await;
        let mut n = nueva(10);
        n.diagnostico_principal = "   ".into();
        let id = crear(&pool, Ambito::institucion(1), 5, &n).await.unwrap().unwrap();
        let h = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert_eq!(h.diagnostico_principal, "   ", "la columna no guardó lo que se mandó");
        assert_eq!(h.faltan_para_firmar(), vec!["el diagnóstico principal"]);
        // La pareja: un diagnóstico de verdad no falta.
        assert!(nueva_historia_completa(&pool).await.faltan_para_firmar().is_empty());
    }

    async fn nueva_historia_completa(pool: &crate::Pool) -> Historia {
        let id = crear(pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        obtener(pool, Ambito::institucion(1), id).await.unwrap().unwrap()
    }

    #[tokio::test]
    async fn no_se_firma_ni_se_borra_la_ajena() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(2), 6, &nueva_con(20, 6)).await.unwrap().unwrap();
        assert_eq!(firmar(&pool, Ambito::institucion(1), id, 5, None).await.unwrap(), ResultadoFirma::NoSePudo);
        assert!(obtener(&pool, Ambito::institucion(2), id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn eliminar_una_de_borrador_la_quita() {
        let pool = base().await;
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        assert_eq!(eliminar(&pool, Ambito::institucion(1), id).await.unwrap(), Resultado::Hecho);
        assert!(obtener(&pool, Ambito::institucion(1), id).await.unwrap().is_none());
    }

    /// La firma criptográfica: firmar con clave, verificar Válida, y detectar una
    /// alteración hecha por DEBAJO de la app (UPDATE directo, simulando a quien
    /// escribe en la base) como Alterada.
    #[tokio::test]
    async fn la_firma_detecta_una_alteracion_directa_en_la_base() {
        let pool = base().await;
        let (vk, sk) = guaca::firma::generar_claves();
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        assert_eq!(firmar(&pool, Ambito::institucion(1), id, 5, Some(&sk)).await.unwrap(), ResultadoFirma::Firmada);

        // Recién firmada: verifica.
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Valida)
        );

        // Un atacante con acceso a la base cambia el diagnóstico saltándose la app.
        sqlx::query("UPDATE historia SET diagnostico_principal = 'ALTERADO' WHERE id = ?")
            .bind(id).execute(&pool).await.unwrap();
        assert_eq!(
            verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(),
            Some(guaca::firma::Verificacion::Alterada),
            "la firma no detectó la manipulación directa en la base"
        );
    }

    #[tokio::test]
    async fn sin_clave_se_cierra_pero_no_hay_firma_que_verificar() {
        let pool = base().await;
        let (vk, _) = guaca::firma::generar_claves();
        let id = crear(&pool, Ambito::institucion(1), 5, &nueva(10)).await.unwrap().unwrap();
        assert_eq!(firmar(&pool, Ambito::institucion(1), id, 5, None).await.unwrap(), ResultadoFirma::Firmada);
        let h = obtener(&pool, Ambito::institucion(1), id).await.unwrap().unwrap();
        assert!(h.firmada && h.firma.is_none());
        // Sin firma criptográfica, no hay nada que verificar.
        assert_eq!(verificar(&pool, Ambito::institucion(1), id, &vk).await.unwrap(), None);
    }
}
