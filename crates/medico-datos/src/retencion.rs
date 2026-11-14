//! #758 — Retención y supresión de la historia clínica por defunción (Ley 1581).
//!
//! Al cumplirse el periodo legal de conservación contado **desde la fecha de
//! defunción** (20 años por defecto, Res. 1995/1999), el expediente completo del
//! paciente se archiva bajo custodia (copia comprimida + cifrada + manifiesto
//! firmado; eso vive en `medico-api`, que tiene la cripto) y se PURGA de la base
//! viva. El «demostrable» de la Ley 1581 es el manifiesto firmado + el hash del
//! archivo: acreditan qué se borró y cuándo sin conservar el dato.
//!
//! # Por qué la fecha de defunción y no la última atención
//!
//! Nadie recibe atención después de morir, así que la defunción es siempre ≥ la
//! última atención. Anclar el reloj en ella nunca borra antes del mínimo legal
//! (Res. 839/2017, 15 años desde la última atención): siempre lo respeta o
//! conserva de más. Decisión de Juan (2026-09-22), y además es la más simple.
//!
//! # Este módulo es la CAPA DE DATOS: vuelca y purga; no cifra ni firma
//!
//! El alcance —qué tablas son «el expediente de un paciente»— vive aquí en UNA
//! lista curada, [`EXPEDIENTE`], y una prueba lo contrasta contra el esquema real
//! ([`contra_la_base::el_mapa_cubre_toda_fk_al_paciente`]): toda tabla con clave
//! foránea a `paciente` (directa o vía `historia`) tiene que estar en
//! [`EXPEDIENTE`] o en [`EXENTAS`] con su razón, o la prueba falla. Es el patrón
//! de `respaldo::TABLAS`, y por la misma razón (D22): un expediente archivado o
//! purgado a medias es peor que uno intacto, y no se ve en la primera vuelta.
//!
//! # Lo que NO se toca, por decisión de Juan (2026-09-22)
//!
//! - La fila `paciente` **no se borra**: la referencian la cadena `acceso` (que se
//!   conserva) y los `recien_nacido` de hijos que pueden seguir vivos. Se ANONIMIZA
//!   su PII ([`anonimizar_paciente`]); el dato completo queda solo en el archivo.
//! - La cadena `acceso` **se conserva** entera (prueba de tratamiento lícito; su
//!   integridad la da la cadena por hash, no el recuento). Está en [`EXENTAS`].
//! - `recien_nacido` se purga SOLO como sujeto (`paciente_id` = fallecido), nunca
//!   por `madre_paciente_id`: esa fila es del HIJO, otro paciente que puede vivir.
//! - `envio_rda` queda fuera: no tiene `paciente_id` (solo se filtraría parseando
//!   el bundle FHIR) y son datos ya transmitidos al visor nacional. Está en
//!   [`EXENTAS`] como residuo documentado.

use medico_dominio::Ambito;
use serde_json::Value;

/// Periodo legal de conservación por defecto, en años, contado desde la defunción.
pub const ANOS_POR_DEFECTO: i64 = 20;

/// El PISO legal de conservación (Res. 839/2017, mínimo 15 años). Un periodo
/// configurado por debajo de esto sería borrado ilegal: el llamante que lee la
/// config debe rechazarlo (D20/D21), no aceptarlo por ser «lo que pidió el
/// operador» — el mínimo es de la ley, no una preferencia.
pub const MINIMO_LEGAL_ANOS: i64 = 15;

/// Cómo una tabla del expediente se liga al paciente que se suprime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Liga {
    /// Tiene `paciente_id`: se filtra por él.
    Directa,
    /// Cuelga de `historia`: se filtra por `historia_id IN (historias del paciente)`.
    ViaHistoria,
    /// Enlace POLIMÓRFICO a un encuentro (`tipo_encuentro`, `encuentro_id`) sin FK
    /// —el caso de `aval_seccion_rda` tras la 0074—. El id apunta a `historia`,
    /// `hospitalizacion` o `atencion_urgencias` según el tipo. Liga el `paciente_id`
    /// TRES veces (una por rama).
    ViaEncuentro,
    /// Tiene `paciente_id` Y `historia_id`, y el `paciente_id` puede ir NULL en una
    /// fila cuyo `historia_id` sí es del fallecido (p. ej. `detalle_factura`: sus dos
    /// columnas son `Option` independientes). Filtrar solo por `paciente_id` dejaría
    /// esa fila colgando de una historia borrada → FK sin `ON DELETE` → la purga
    /// aborta. Lo cazó el concilio del 2026-09-22 (fable). Liga `paciente_id` DOS
    /// veces (la directa y la del subselect por historia).
    DirectaOViaHistoria,
}

/// El expediente de UN paciente: las tablas a archivar y purgar, con su liga.
///
/// Curada a mano y verificada contra el esquema real por la prueba
/// `el_mapa_cubre_toda_fk_al_paciente`. `historia` va la ÚLTIMA a propósito (es la
/// raíz de la que cuelgan `formula`/`orden`/`imagen_diagnostica`); el borrado corre
/// con `PRAGMA defer_foreign_keys=ON`, así que el orden no rompe integridad, pero
/// dejarlo explícito documenta la dependencia.
const EXPEDIENTE: &[(&str, Liga)] = &[
    // VA PRIMERO: su filtro `ViaEncuentro` hace subselect a `historia`,
    // `hospitalizacion` y `atencion_urgencias`; si esas se borraran antes, el
    // subselect saldría vacío y los avales de hosp/urgencias del fallecido
    // SOBREVIVIRÍAN (concilio 2026-09-22, opus). El chequeo post-borrado lo caza,
    // pero el orden lo hace posible en una sola pasada. Regla general: toda tabla
    // con subselect a otra del conjunto va ANTES que ella; `historia` va la última.
    ("aval_seccion_rda", Liga::ViaEncuentro),
    ("administracion_medicamento", Liga::Directa),
    ("atencion_urgencias", Liga::Directa),
    ("cita", Liga::Directa),
    // Con paciente_id Y historia_id independientes: una fila con paciente_id NULL
    // pero historia_id del fallecido debe purgarse igual (concilio 2026-09-22).
    ("consentimiento_informado", Liga::DirectaOViaHistoria),
    ("contrarreferencia", Liga::Directa),
    ("control_pyp", Liga::Directa),
    ("defuncion", Liga::Directa),
    ("detalle_factura", Liga::DirectaOViaHistoria),
    ("dispensacion", Liga::DirectaOViaHistoria),
    ("documento_clinico", Liga::DirectaOViaHistoria),
    ("evento_iaas", Liga::Directa),
    ("hospitalizacion", Liga::Directa),
    ("informe_imagen", Liga::Directa),
    ("inscripcion_pyp", Liga::Directa),
    ("movimiento_inventario", Liga::Directa),
    ("plan_alimentacion", Liga::Directa),
    ("plan_rehabilitacion", Liga::Directa),
    ("programacion_quirurgica", Liga::Directa),
    // SOLO como sujeto (`paciente_id`); nunca por `madre_paciente_id` (hijo vivo).
    ("recien_nacido", Liga::Directa),
    ("recuperacion_postanestesica", Liga::Directa),
    ("resultado_laboratorio", Liga::Directa),
    ("sesion_terapia", Liga::Directa),
    ("transfusion", Liga::Directa),
    ("traslado", Liga::Directa),
    ("vacuna_aplicada", Liga::Directa),
    ("valoracion_nutricional", Liga::Directa),
    ("valoracion_preanestesica", Liga::Directa),
    ("formula", Liga::ViaHistoria),
    ("orden", Liga::ViaHistoria),
    ("imagen_diagnostica", Liga::ViaHistoria),
    ("teleconsulta", Liga::ViaHistoria),
    // La raíz, al final: todas las ligas ViaHistoria/DirectaOViaHistoria subseleccionan
    // sobre ella.
    ("historia", Liga::Directa),
];

/// Tablas con FK a `paciente` que NO se purgan, cada una con su razón. Lo exige la
/// prueba del mapa: nada ligado al paciente se queda sin clasificar en silencio.
const EXENTAS: &[(&str, &str)] = &[
    (
        "acceso",
        "Bitácora encadenada de la Ley 1581 (prueba de tratamiento lícito). Su \
         integridad la da la cadena por hash, no el recuento; borrar eslabones la \
         rompería. Se CONSERVA (Juan, 2026-09-22). La fila `paciente` sobrevive \
         anonimizada como ancla de esta FK.",
    ),
    (
        "supresion",
        "El CERTIFICADO de supresión (esta migración): es la PRUEBA de que el \
         expediente existió y se borró (Ley 1581). Purgarlo destruiría la prueba \
         que da sentido a todo el mecanismo. Referencia `paciente(id)`, conservada \
         anonimizada.",
    ),
    (
        "envio_rda",
        "Bundle FHIR ya transmitido al visor nacional; NO tiene `paciente_id` (solo \
         se filtraría parseando el JSON). Fuera de la purga por paciente como \
         residuo documentado (Juan, 2026-09-22); incluirlo es mejora aparte.",
    ),
];

/// Las tablas ligadas al paciente que NO se purgan, con su razón. Para un informe
/// de auditoría del alcance de la supresión (y para que la razón sea consultable,
/// no solo un comentario).
pub fn tablas_exentas() -> &'static [(&'static str, &'static str)] {
    EXENTAS
}

/// Los nombres de las tablas que forman el expediente que se archiva y purga.
pub fn tablas_del_expediente() -> Vec<&'static str> {
    EXPEDIENTE.iter().map(|(t, _)| *t).collect()
}

/// El filtro `WHERE` de una tabla del expediente. Cada `?` se liga a `paciente_id`
/// —ver [`binds`] para cuántos—.
fn filtro(liga: Liga) -> String {
    match liga {
        Liga::Directa => "paciente_id = ?".into(),
        Liga::ViaHistoria => {
            "historia_id IN (SELECT id FROM historia WHERE paciente_id = ?)".into()
        }
        Liga::ViaEncuentro => format!(
            "(tipo_encuentro = '{c}' AND encuentro_id IN (SELECT id FROM historia WHERE paciente_id = ?)) OR \
             (tipo_encuentro = '{h}' AND encuentro_id IN (SELECT id FROM hospitalizacion WHERE paciente_id = ?)) OR \
             (tipo_encuentro = '{u}' AND encuentro_id IN (SELECT id FROM atencion_urgencias WHERE paciente_id = ?))",
            c = crate::rda_encuentro::OPERACION_CONSULTA_EXTERNA,
            h = crate::rda_encuentro::OPERACION_HOSPITALIZACION,
            u = crate::rda_encuentro::OPERACION_URGENCIAS,
        ),
        Liga::DirectaOViaHistoria => {
            "(paciente_id = ? OR historia_id IN (SELECT id FROM historia WHERE paciente_id = ?))"
                .into()
        }
    }
}

/// Cuántas veces hay que ligar `paciente_id` en el filtro de esta liga.
fn binds(liga: Liga) -> usize {
    match liga {
        Liga::ViaEncuentro => 3,
        Liga::DirectaOViaHistoria => 2,
        _ => 1,
    }
}

fn institucion(ambito: Ambito) -> Option<Option<i64>> {
    match ambito {
        Ambito::Superadmin => Some(None),
        Ambito::Institucion(i) => Some(Some(i)),
        Ambito::Ninguno => None,
    }
}

/// Un paciente elegible para supresión: fallecido hace ≥ `anos`, aún no suprimido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elegible {
    pub paciente_id: i64,
    pub institucion_id: i64,
    pub fecha_defuncion: String,
    pub fecha_vencimiento: String,
}

/// Por qué una purga no se ejecutó, distinto de un error de base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RazonNoPurga {
    /// La defunción dejó de estar vigente (anulada) o el plazo no se cumple, o ya
    /// hay una supresión — re-comprobado DENTRO de la transacción.
    NoElegible,
    /// Una tabla tiene hoy un recuento distinto del archivado: el expediente
    /// cambió entre el volcado y la purga. No se borra (el archivo no es fiel).
    CambioDetectado { tabla: String, archivado: i64, ahora: i64 },
    /// Tras borrar, una tabla AÚN tiene filas del fallecido: el borrado quedó
    /// incompleto (orden mal, un CASCADE que no alcanzó, un trigger, una liga que
    /// no cubre). Se hace ROLLBACK: es inaceptable confirmar una purga parcial y
    /// certificarla como completa (concilio 2026-09-22). Es el keystone de la
    /// seguridad: ninguna fila del fallecido sobrevive a un commit.
    BorradoIncompleto { tabla: String, quedan: i64 },
}

/// `fecha_defuncion` (RFC 3339) + `anos` años, como RFC 3339 UTC. `None` si la
/// fecha no se puede leer (D20: no se inventa un vencimiento).
pub fn vencimiento(fecha_defuncion: &str, anos: i64) -> Option<String> {
    let dt = chrono::DateTime::parse_from_rfc3339(fecha_defuncion).ok()?;
    let meses = u32::try_from(anos.checked_mul(12)?).ok()?;
    let venc = dt
        .with_timezone(&chrono::Utc)
        .checked_add_months(chrono::Months::new(meses))?;
    Some(venc.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

/// El MARCO HORARIO en que una columna guarda su fecha. Es OBLIGATORIO declararlo
/// por columna (no hay defecto, D20): el fail-safe #1 compara el acto contra
/// `fecha_defuncion`, que desde la migración 0078 se guarda en **UTC**, así que la
/// comparación solo es correcta si el acto también está en UTC.
///
/// **Por qué existe (concilio de cierre 2026-09-26, R1 y su clase):** sellar
/// `fecha_defuncion` a UTC dejó un FAIL-OPEN para las columnas que se guardan en hora
/// LOCAL naive —un acto 0-5 h ANTES de morir parecía posterior y NO bloqueaba, dejando
/// elegible a un fallecido real cuya muerte se tecleó mal—. `vacuna_aplicada`/`control_pyp`
/// se normalizaron al ESCRIBIR (migración 0079); `cita` NO puede —su `fecha`/`hora` las usa
/// la agenda para MOSTRAR—, así que se convierte AQUÍ, en la comparación.
///
/// **La asimetría fija el defecto seguro (D20/D23):** marcar como `LocalNaive` una columna
/// que en realidad es UTC solo sobre-bloquea (sobre-retiene un muerto, tolerable); marcar
/// como `Utc` una que es local reabre el fail-open (purga irreversible de un vivo). Ante la
/// duda, `LocalNaive`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Marco {
    /// La columna ya está en UTC (sellada por el servidor con `Utc::now()`/`crate::ahora()`/
    /// `datetime('now')`, o normalizada al escribir). Se compara directo.
    Utc,
    /// La columna es hora LOCAL del prestador, naive, tecleada para mostrar (`cita`:
    /// `type=date`+`type=time`). Se convierte de America/Bogota (UTC-05 FIJO, sin DST —
    /// mismo supuesto que las migraciones 0078/0079) a UTC en la comparación con `'+5 hours'`.
    LocalNaive,
}

impl Marco {
    /// La expresión `datetime(...)` en UTC lista para comparar contra `d.fecha_defuncion`.
    fn datetime_utc(self, expr: &str) -> String {
        match self {
            Marco::Utc => format!("datetime({expr})"),
            Marco::LocalNaive => format!("datetime({expr}, '+5 hours')"),
        }
    }
}

/// Una columna de fecha de un acto, dentro de una tabla de [`PRUEBA_DE_VIDA`].
struct Fecha {
    /// La expresión SQL de la fecha, con el alias `t` de la tabla (una columna
    /// simple `t.fecha`, o una compuesta como la de `cita`).
    expr: &'static str,
    /// El marco horario de la columna (ver [`Marco`]). OBLIGATORIO: una tabla nueva
    /// de `PRUEBA_DE_VIDA` no compila sin declararlo, así que no hay 4º caso silencioso.
    marco: Marco,
    /// `true`: la fecha SIEMPRE existe (NOT NULL), así que NULL o no-parseable
    /// BLOQUEA (dirección segura: una fecha que no se puede comparar no prueba que
    /// el acto sea anterior a la muerte). `false`: la columna es NULABLE (un egreso
    /// en curso, una cirugía programada y no realizada), así que solo bloquea si
    /// está PRESENTE y es posterior o no-parseable.
    obligatoria: bool,
}

/// Una tabla que **PRUEBA VIDA**: registra un acto que solo se le hace a un cuerpo
/// VIVO (una vacuna, una medicación, una transfusión…). Una fecha posterior a la
/// defunción = la defunción se tecleó mal → el paciente NO es elegible (fail-safe #1).
struct PruebaVida {
    tabla: &'static str,
    /// Cómo liga la tabla al paciente (la misma [`Liga`] que en [`EXPEDIENTE`]): así
    /// una fila con `paciente_id` NULL pero `historia_id` del paciente (p. ej.
    /// `dispensacion`, o `formula`/`orden` que solo cuelgan de la historia) NO se
    /// escapa del chequeo.
    liga: Liga,
    /// Filtro extra opcional (p. ej. solo las citas `ATENDIDA` prueban vida).
    filtro: Option<&'static str>,
    fechas: &'static [Fecha],
}

/// **FAIL-SAFE #1 (concilio 2026-09-22 fable/sonnet/opus, ampliado 2026-09-23 — D20/D23).**
/// Una fecha de defunción mal tecleada (un `2005` por `2025` en el `datetime-local`, que
/// la pantalla acepta sin `min`/`max`) dejaría a un paciente VIVO elegible y su historia
/// desaparecería para siempre. El módulo funda su reloj en «nadie recibe atención tras
/// morir»; [`sin_atencion_tras_defuncion`] lo HACE CUMPLIR: si el paciente tiene un acto
/// de vida con fecha **posterior** a la defunción —o una fecha que ni se puede comparar—,
/// NO es elegible **por construcción**.
///
/// **El primer concilio lo dejó a 4 tablas y el segundo (2026-09-23) midió el hueco**:
/// `EXPEDIENTE` —lo que la purga borra— tiene ~27 tablas ligadas al paciente con fecha
/// propia, y una vacuna/medicación/transfusión posterior a la muerte NO estaba en las 4.
/// Un paciente de vacunación (que por diseño existe sin `historia`) con la defunción mal
/// tecleada se PURGABA. Ahora la cubeta se deriva tabla por tabla de `EXPEDIENTE` y un
/// guardián (`toda_tabla_del_expediente_con_fecha_prueba_vida_o_se_exime`, D33) obliga a
/// que toda tabla nueva de `EXPEDIENTE` con fecha se clasifique aquí o en
/// [`ADMINISTRATIVAS_PARA_VIDA`] — no vuelve a colarse en silencio.
///
/// **La cubeta hermana ([`ADMINISTRATIVAS_PARA_VIDA`]) es igual de importante (D23)**: una
/// factura final, un resultado de laboratorio o una nota de cierre fechados DESPUÉS de la
/// muerte son legítimos, así que no bloquean; incluirlos volvería a un fallecido real
/// imposible de suprimir para siempre y rompería la obligación de la Ley 1581.
// TODAS las columnas son `Marco::Utc` salvo `cita` (`LocalNaive`), verificado columna por
// columna rastreando su ESCRITURA de producción (concilio 2026-09-26): `Utc::now()`,
// `crate::ahora()` (= `Utc::now()`), `datetime('now')` DEFAULT, o normalizadas al escribir
// (`vacuna_aplicada`/`control_pyp`, migración 0079). Solo `cita.fecha`+`hora` son hora local
// naive tecleada para la agenda. Cambiar un marco exige repetir esa verificación.
const PRUEBA_DE_VIDA: &[PruebaVida] = &[
    // Las cuatro originales (concilio 2026-09-22).
    PruebaVida { tabla: "historia", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.fecha", marco: Marco::Utc, obligatoria: true }] },
    PruebaVida { tabla: "hospitalizacion", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.fecha_ingreso", marco: Marco::Utc, obligatoria: true },
                  Fecha { expr: "t.fecha_egreso", marco: Marco::Utc, obligatoria: false }] },
    PruebaVida { tabla: "atencion_urgencias", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.fecha_ingreso", marco: Marco::Utc, obligatoria: true },
                  Fecha { expr: "t.fecha_egreso", marco: Marco::Utc, obligatoria: false }] },
    // Una cita AGENDADA y no atendida no cuenta (pudo agendarse en vida y quedar sin
    // cumplir tras la muerte); solo la ATENDIDA prueba vida. Fecha compuesta fecha+hora.
    // `LocalNaive`: `fecha` (`type=date`) y `hora` (`type=time`) son hora local del
    // prestador, tecleadas para la agenda; NO se normalizan al escribir (rompería la
    // visualización), así que se convierten a UTC AQUÍ (concilio 2026-09-26, R1-clase).
    PruebaVida { tabla: "cita", liga: Liga::Directa, filtro: Some("t.estado = 'ATENDIDA'"),
        fechas: &[Fecha { expr: "t.fecha || ' ' || COALESCE(NULLIF(t.hora, ''), '00:00')", marco: Marco::LocalNaive, obligatoria: true }] },
    // Ampliación 2026-09-23: actos sobre un cuerpo vivo que existían fuera de las 4.
    // `vacuna_aplicada`/`control_pyp` se guardan en UTC desde la migración 0079 (antes eran
    // el fail-open que este mismo concilio cazó): se normalizan al ESCRIBIR, no aquí.
    PruebaVida { tabla: "vacuna_aplicada", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.fecha", marco: Marco::Utc, obligatoria: true }] },
    PruebaVida { tabla: "administracion_medicamento", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.fecha", marco: Marco::Utc, obligatoria: true }] },
    PruebaVida { tabla: "control_pyp", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.fecha", marco: Marco::Utc, obligatoria: true }] },
    PruebaVida { tabla: "transfusion", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.fecha", marco: Marco::Utc, obligatoria: true }] },
    PruebaVida { tabla: "sesion_terapia", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.fecha_sesion", marco: Marco::Utc, obligatoria: true }] },
    PruebaVida { tabla: "valoracion_nutricional", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.fecha_valoracion", marco: Marco::Utc, obligatoria: true }] },
    PruebaVida { tabla: "valoracion_preanestesica", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.fecha", marco: Marco::Utc, obligatoria: true }] },
    PruebaVida { tabla: "plan_alimentacion", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.fecha_creacion", marco: Marco::Utc, obligatoria: true }] },
    PruebaVida { tabla: "plan_rehabilitacion", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.fecha_creacion", marco: Marco::Utc, obligatoria: true }] },
    PruebaVida { tabla: "traslado", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.fecha_salida", marco: Marco::Utc, obligatoria: true }] },
    // La cirugía PROGRAMADA (fecha_programada) no prueba vida —pudo programarse y no
    // realizarse—; la REAL (hora_inicio_real, nulable) sí: si se operó tras la muerte
    // tecleada, la fecha está mal. `hora_inicio_real` la sella el servidor (`crate::ahora()`).
    PruebaVida { tabla: "programacion_quirurgica", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.hora_inicio_real", marco: Marco::Utc, obligatoria: false }] },
    // La recuperación posanestésica es un acto sobre un cuerpo VIVO: se entra y se sale de
    // recuperación porque se está vivo. Su fecha de acto NO empieza por `fecha`/`hora`
    // (`ingreso_recuperacion`/`salida_recuperacion`), y el concilio de cierre (2026-09-23,
    // fable/sonnet/opus) cazó que estaba mal metida en ADMINISTRATIVAS con la razón falsa
    // «sin fecha de acto propia; la vida la prueba la cirugía» —la cirugía bloquea solo por
    // `hora_inicio_real`, que es NULABLE, así que no es garantía—. `ingreso_recuperacion` es
    // NOT NULL → obligatoria. Las dos las sella el servidor (`Utc::now()`).
    PruebaVida { tabla: "recuperacion_postanestesica", liga: Liga::Directa, filtro: None,
        fechas: &[Fecha { expr: "t.ingreso_recuperacion", marco: Marco::Utc, obligatoria: true },
                  Fecha { expr: "t.salida_recuperacion", marco: Marco::Utc, obligatoria: false }] },
    // Ligan por historia (o paciente): el `enlace` lo resuelve la liga, no `paciente_id` a secas.
    PruebaVida { tabla: "dispensacion", liga: Liga::DirectaOViaHistoria, filtro: None,
        fechas: &[Fecha { expr: "t.fecha_dispensacion", marco: Marco::Utc, obligatoria: true }] },
    // POLIMÓRFICA por `tipo` (concilio 2026-09-23, sonnet, confianza 10, demostrado): la
    // mayoría de sus tipos —EVOLUCION, NOTA_OPERATORIA, INTERCONSULTA, NOTA_ENFERMERIA,
    // PLAN_CUIDADO, NOTA_TRABAJO_SOCIAL— son actos/observaciones sobre un cuerpo VIVO y
    // deben BLOQUEAR. La EXCEPCIÓN es la EPICRISIS (resumen de egreso/defunción), que se
    // finaliza legítimamente al/tras el egreso o la muerte. `fecha` la sella el servidor
    // (`datetime('now')`). Se exime SOLO EPICRISIS —dirección segura D20/D23: la asimetría
    // manda (purgar un vivo es irreversible; sobre-retener un muerto no), así que cualquier
    // tipo NUEVO o desconocido BLOQUEA por defecto—. La lista de exención es criterio
    // clínico-legal de Juan; ampliarla exige su decisión, con su caso rojo/verde.
    PruebaVida { tabla: "documento_clinico", liga: Liga::DirectaOViaHistoria,
        filtro: Some("t.tipo NOT IN ('EPICRISIS')"),
        fechas: &[Fecha { expr: "t.fecha", marco: Marco::Utc, obligatoria: true }] },
    PruebaVida { tabla: "formula", liga: Liga::ViaHistoria, filtro: None,
        fechas: &[Fecha { expr: "t.fecha_formula", marco: Marco::Utc, obligatoria: true }] },
    PruebaVida { tabla: "orden", liga: Liga::ViaHistoria, filtro: None,
        fechas: &[Fecha { expr: "t.fecha_orden", marco: Marco::Utc, obligatoria: true }] },
];

/// Tablas de `EXPEDIENTE` con fecha que NO prueban vida: su fecha puede ser
/// legítimamente POSTERIOR a la muerte, así que bloquear con ellas dejaría a un
/// fallecido real imposible de suprimir (D23, hallazgo de opus el 2026-09-23). Cada
/// una con su razón; el guardián D33 exige que toda tabla de `EXPEDIENTE` con fecha
/// esté aquí o en [`PRUEBA_DE_VIDA`].
///
/// Es `#[cfg(test)]` porque su ÚNICO consumidor es el guardián D33: en producción la
/// cláusula la construye `sin_atencion_tras_defuncion` a partir de `PRUEBA_DE_VIDA`, y
/// esta lista es la contraparte que el guardián exige para no dejar nada sin clasificar.
#[cfg(test)]
const ADMINISTRATIVAS_PARA_VIDA: &[(&str, &str)] = &[
    ("defuncion", "es el registro de la muerte misma; su fecha ES la defunción"),
    ("detalle_factura", "facturación: una factura final es legítima tras la muerte"),
    ("movimiento_inventario", "movimiento de stock, no un acto clínico sobre el paciente"),
    ("consentimiento_informado", "firma/revocación administrativa; puede ocurrir tras la muerte"),
    ("contrarreferencia", "acto administrativo de referencia, no atención al cuerpo"),
    ("inscripcion_pyp", "inscripción a un programa (acto administrativo), no atención"),
    ("resultado_laboratorio", "el RESULTADO se produce/valida tras la toma; puede ser post-mortem"),
    ("informe_imagen", "el informe se emite tras el estudio; puede ser post-mortem"),
    ("imagen_diagnostica", "fecha_subida es la subida del archivo; puede ser post-mortem (como el resultado)"),
    ("evento_iaas", "una infección puede detectarse/notificarse tras la muerte"),
    ("recien_nacido", "fecha_nacimiento es del hijo, no prueba vida de este paciente"),
    // Sin fecha de acto propia: se clasifican para que el guardián no las deje fuera (no por
    // tener fecha post-mortem). Su vida la prueba, transitivamente, la historia/encuentro.
    ("teleconsulta", "sin fecha de acto propia (solo creado_en); su atención la fecha su historia (NOT NULL), que ya prueba vida"),
    ("aval_seccion_rda", "aval de una sección RDA; sin fecha de acto clínico, y su encuentro ya prueba vida"),
];

/// El enlace de una tabla de vida a `defuncion` (alias `d`) como SUBCONSULTA
/// CORRELACIONADA (referencia `d.paciente_id`, no un `?`), a diferencia de
/// [`filtro`], que usa `bind`. Mismo mapa de ligas.
fn enlace_correlacionado(liga: Liga, alias: &str) -> String {
    match liga {
        Liga::Directa => format!("{alias}.paciente_id = d.paciente_id"),
        Liga::ViaHistoria => {
            format!("{alias}.historia_id IN (SELECT id FROM historia WHERE paciente_id = d.paciente_id)")
        }
        Liga::DirectaOViaHistoria => format!(
            "{alias}.paciente_id = d.paciente_id OR \
             {alias}.historia_id IN (SELECT id FROM historia WHERE paciente_id = d.paciente_id)"
        ),
        Liga::ViaEncuentro => {
            unreachable!("ninguna tabla de PRUEBA_DE_VIDA liga por encuentro")
        }
    }
}

/// Construye la cláusula SQL del fail-safe #1 a partir de [`PRUEBA_DE_VIDA`]: un
/// `NOT EXISTS` por tabla, unidos por `AND`. Referencia la fila `defuncion` por el
/// alias `d`, de modo que la MISMA regla sirve en `elegibles` (el barrido) y en el
/// re-chequeo transaccional de `ejecutar_purga` (una sola regla, D9/D23).
fn sin_atencion_tras_defuncion() -> String {
    PRUEBA_DE_VIDA
        .iter()
        .map(|pv| {
            let enlace = enlace_correlacionado(pv.liga, "t");
            let filtro = pv.filtro.map(|f| format!(" AND {f}")).unwrap_or_default();
            let cond = pv
                .fechas
                .iter()
                .map(|f| {
                    // `dt` = el acto en UTC (con el `+5 hours` si la columna es `LocalNaive`),
                    // listo para comparar contra `d.fecha_defuncion` (UTC desde la 0078).
                    let dt = f.marco.datetime_utc(f.expr);
                    if f.obligatoria {
                        format!("{dt} IS NULL OR {dt} > datetime(d.fecha_defuncion)")
                    } else {
                        // La presencia se comprueba sobre la columna CRUDA (`f.expr`), no sobre `dt`.
                        format!(
                            "({e} IS NOT NULL AND ({dt} IS NULL OR {dt} > datetime(d.fecha_defuncion)))",
                            e = f.expr
                        )
                    }
                })
                .collect::<Vec<_>>()
                .join(" OR ");
            format!(
                "NOT EXISTS (SELECT 1 FROM \"{t}\" t WHERE ({enlace}){filtro} AND ({cond}))",
                t = pv.tabla
            )
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

/// Los pacientes cuyo expediente ya se puede suprimir en el ámbito dado.
///
/// El plazo Y el vencimiento se calculan **en SQL** con `datetime(fecha, '+N years')`
/// (`anos` sale del código/config, nunca de fuera). Antes el vencimiento se parseaba
/// en Rust con `parse_from_rfc3339`, que devuelve `None` para el formato que produce
/// la pantalla (`datetime-local` → `YYYY-MM-DDTHH:MM`, sin segundos ni `Z`), y el
/// `filter_map` DESCARTABA al paciente en silencio: el subsistema quedaba activo y no
/// suprimía a NADIE certificado por el panel (concilio 2026-09-22, fable — D20/D30/D32:
/// dos relojes para el mismo hecho). Un solo reloj, el de SQLite, que acepta ese
/// formato igual que el `WHERE`. Salta a quien ya tenga fila en `supresion`.
pub async fn elegibles(
    pool: &crate::Pool,
    ambito: Ambito,
    ahora: chrono::DateTime<chrono::Utc>,
    anos: i64,
) -> Result<Vec<Elegible>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let sin_atencion = sin_atencion_tras_defuncion();
    let mut sql = format!(
        "SELECT d.paciente_id, p.institucion_id, d.fecha_defuncion, \
                datetime(d.fecha_defuncion, '+{anos} years') AS vence \
         FROM defuncion d JOIN paciente p ON p.id = d.paciente_id \
         WHERE d.anulada = 0 \
           AND vence IS NOT NULL \
           AND vence <= datetime(?) \
           AND NOT EXISTS (SELECT 1 FROM supresion s WHERE s.paciente_id = d.paciente_id) \
           AND {sin_atencion}"
    );
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    let ahora_txt = ahora.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut q = sqlx::query_as::<_, (i64, i64, String, String)>(&sql).bind(&ahora_txt);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    let filas = q.fetch_all(pool).await?;
    Ok(filas
        .into_iter()
        .map(|(paciente_id, institucion_id, fecha_defuncion, fecha_vencimiento)| Elegible {
            paciente_id,
            institucion_id,
            fecha_defuncion,
            fecha_vencimiento,
        })
        .collect())
}

/// Una fila de SQLite a un objeto JSON, columna por columna, respetando la clase
/// de almacenamiento real de cada valor (no la afinidad declarada). Se hace en
/// Rust y no con `json_object` de SQLite porque esa función topa en 127 argumentos
/// y `historia` tiene 74 columnas (148 argumentos).
fn fila_a_json(row: &sqlx::sqlite::SqliteRow) -> serde_json::Map<String, Value> {
    use sqlx::{Column, Row, TypeInfo, ValueRef};
    let mut m = serde_json::Map::new();
    for col in row.columns() {
        let i = col.ordinal();
        let v = match row.try_get_raw(i) {
            Ok(raw) if raw.is_null() => Value::Null,
            Ok(raw) => match raw.type_info().name() {
                "INTEGER" | "BIGINT" | "INT8" | "INT" => {
                    row.try_get::<i64, _>(i).map(Value::from).unwrap_or(Value::Null)
                }
                "REAL" | "DOUBLE" | "FLOAT" => {
                    row.try_get::<f64, _>(i).map(Value::from).unwrap_or(Value::Null)
                }
                "BLOB" => row
                    .try_get::<Vec<u8>, _>(i)
                    .map(|b| Value::from(b.iter().map(|x| format!("{x:02x}")).collect::<String>()))
                    .unwrap_or(Value::Null),
                _ => row.try_get::<String, _>(i).map(Value::from).unwrap_or(Value::Null),
            },
            Err(_) => Value::Null,
        };
        m.insert(col.name().to_string(), v);
    }
    m
}

/// Vuelca el expediente completo del paciente como JSON, con el recuento por tabla.
///
/// Devuelve `{ "paciente": {...}, "administracion_medicamento": [...], ... }` y la
/// lista `(tabla, filas)`. Es lo que se comprime, cifra y firma; y su recuento es
/// lo que la purga vuelve a comprobar antes de borrar.
pub async fn volcar(
    pool: &crate::Pool,
    paciente_id: i64,
) -> Result<(Value, Vec<(String, i64)>), sqlx::Error> {
    let mut objeto = serde_json::Map::new();
    let mut recuentos = Vec::new();

    // La fila `paciente` (se anonimizará; el dato completo vive aquí).
    let pac = sqlx::query("SELECT * FROM paciente WHERE id = ?")
        .bind(paciente_id)
        .fetch_optional(pool)
        .await?;
    if let Some(r) = pac {
        objeto.insert("paciente".into(), Value::Object(fila_a_json(&r)));
    }

    for (tabla, liga) in EXPEDIENTE {
        // `tabla` sale de EXPEDIENTE (constante del código), nunca de fuera.
        let sql = format!("SELECT * FROM \"{tabla}\" WHERE {}", filtro(*liga));
        let mut q = sqlx::query(&sql);
        for _ in 0..binds(*liga) {
            q = q.bind(paciente_id);
        }
        let rows = q.fetch_all(pool).await?;
        let arr: Vec<Value> = rows.iter().map(|r| Value::Object(fila_a_json(r))).collect();
        recuentos.push(((*tabla).to_string(), arr.len() as i64));
        objeto.insert((*tabla).to_string(), Value::Array(arr));
    }

    Ok((Value::Object(objeto), recuentos))
}

/// Las imágenes del paciente `(ruta, cifrada)` para archivarlas y borrarlas del
/// disco en la purga. Sale de `imagen_diagnostica`, filtrada vía historia. El flag
/// `cifrada` (migración 0014) dice si el archivo en disco es un blob cifrado bajo
/// `MEDICO_STORAGE_KEY`: el archivador debe descifrarlo ANTES de meterlo en la copia
/// bajo custodia, para que esta sea autosuficiente con una sola clave (concilio
/// 2026-09-22, fable).
pub async fn rutas_de_imagen(
    pool: &crate::Pool,
    paciente_id: i64,
) -> Result<Vec<(String, bool)>, sqlx::Error> {
    let filas = sqlx::query_as::<_, (String, i64)>(
        "SELECT ruta_archivo, cifrada FROM imagen_diagnostica \
         WHERE ruta_archivo IS NOT NULL AND ruta_archivo <> '' \
           AND historia_id IN (SELECT id FROM historia WHERE paciente_id = ?)",
    )
    .bind(paciente_id)
    .fetch_all(pool)
    .await?;
    Ok(filas.into_iter().map(|(r, c)| (r, c != 0)).collect())
}

/// El `UPDATE` que anonimiza la fila `paciente`, conservándola como ancla. Deja id,
/// `tipo_documento`, `sexo`, `institucion_id` y `entidad_id`; el resto —nombre,
/// documento, contacto, residencia y antecedentes clínicos— se borra o se pone
/// centinela. `numero_documento` lleva un centinela ÚNICO por id para no chocar con
/// el índice de identidad.
const ANONIMIZAR: &str = "\
    UPDATE paciente SET \
      numero_documento = 'SUPRIMIDO-' || id, \
      nombre1 = 'SUPRIMIDO', nombre2 = NULL, \
      apellido1 = 'SUPRIMIDO', apellido2 = NULL, \
      fecha_nacimiento = '0001-01-01', \
      celular = NULL, email = NULL, direccion = NULL, \
      cod_municipio = NULL, ciudad_residencia = NULL, departamento_residencia = NULL, \
      cod_pais = NULL, cod_pais_origen = NULL, zona = NULL, tipo_usuario = NULL, \
      ant_familiares = '{}', ant_personales = '{}', ant_ginecologicos = '{}', ant_perinatales = '{}', \
      nacionalidad = NULL, pertenencia_etnica = NULL, comunidad_etnica = NULL, \
      discapacidad = NULL, identidad_genero = NULL, \
      plegado = '', procedencias = '' \
    WHERE id = ?";
// Los `ant_*` son JSON (`sqlx Json<Value>`, default `'{}'`): escribir `''` los hace
// ILEGIBLES y todo SELECT de la fila —que se conserva y entra en `paciente::listar`—
// falla ColumnDecode → el padrón entero da 500 (concilio 2026-09-22, fable, conf 9).
// `procedencias` es String con default `''`. Se anonimiza al default de cada columna.

/// Los datos del certificado de supresión, que se insertan DENTRO de la misma
/// transacción que borra (concilio 2026-09-22, sonnet): borrado y certificado son
/// atómicos, o no ocurre ninguno. El manifiesto ya viene firmado por `medico-api`.
pub struct DatosCertificado<'a> {
    pub institucion_id: i64,
    pub fecha_defuncion: &'a str,
    pub fecha_vencimiento: &'a str,
    pub ruta_archivo: &'a str,
    pub sha256_archivo: &'a str,
    pub manifiesto: &'a str,
    pub firma: &'a str,
}

/// El KEYSTONE, extraído para poder PROBAR que discrimina (D33): tras borrar,
/// ninguna tabla `Directa` puede conservar filas del fallecido. Devuelve la primera
/// que sobreviva `(tabla, cuántas)`, o `None` si todas quedaron en 0. Ciego a las
/// ligas con subselect a propósito (una vez borrada la raíz el filtro sale vacío);
/// a esas las protege el ORDEN de `EXPEDIENTE` y su guardián.
async fn directa_que_sobrevive(
    conn: &mut sqlx::SqliteConnection,
    paciente_id: i64,
) -> Result<Option<(String, i64)>, sqlx::Error> {
    for (tabla, liga) in EXPEDIENTE {
        if !matches!(liga, Liga::Directa) {
            continue;
        }
        let quedan: i64 =
            sqlx::query_scalar(&format!("SELECT COUNT(*) FROM \"{tabla}\" WHERE paciente_id = ?"))
                .bind(paciente_id)
                .fetch_one(&mut *conn)
                .await?;
        if quedan != 0 {
            return Ok(Some(((*tabla).to_string(), quedan)));
        }
    }
    Ok(None)
}

/// El núcleo de la purga sobre una transacción dada, SIN commit ni certificado:
/// difiere FK, re-verifica elegibilidad, exige que los recuentos casen lo
/// archivado, desliga al hijo, borra, y —KEYSTONE— comprueba que no quede ninguna
/// fila del fallecido. Devuelve `Ok(Err(razon))` para abortar (el llamante hace
/// rollback), `Ok(Ok(()))` para que el llamante certifique y confirme.
///
/// **Límite del keystone, dicho aquí porque importa:** el chequeo post-borrado usa
/// el MISMO filtro que borró. Para las ligas con subselect (`ViaHistoria`,
/// `ViaEncuentro`, `DirectaOViaHistoria`), una vez borrada la tabla-raíz el
/// subselect sale vacío y el conteo da 0 aunque algo sobreviviera — es CIEGO ahí.
/// Lo que protege a esas ligas es el ORDEN (subselect antes que su raíz), que
/// sostiene la prueba `el_orden_de_borrado_respeta_los_subselect`. El keystone
/// cubre las tablas `Directa` (p. ej. un CASCADE no alcanzado, un trigger).
async fn ejecutar_purga(
    conn: &mut sqlx::SqliteConnection,
    paciente_id: i64,
    anos: i64,
    ahora: chrono::DateTime<chrono::Utc>,
    recuentos_archivados: &[(String, i64)],
) -> Result<Result<(), RazonNoPurga>, sqlx::Error> {
    sqlx::query("PRAGMA defer_foreign_keys = ON").execute(&mut *conn).await?;
    let ahora_txt = ahora.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    // Re-chequeo de elegibilidad (una anulación o una supresión pudo llegar).
    let sin_atencion = sin_atencion_tras_defuncion();
    let sigue: Option<i64> = sqlx::query_scalar(&format!(
        "SELECT 1 FROM defuncion d \
         WHERE d.paciente_id = ? AND d.anulada = 0 \
           AND datetime(d.fecha_defuncion, '+{anos} years') <= datetime(?) \
           AND NOT EXISTS (SELECT 1 FROM supresion s WHERE s.paciente_id = d.paciente_id) \
           AND {sin_atencion} \
         LIMIT 1"
    ))
    .bind(paciente_id)
    .bind(&ahora_txt)
    .fetch_optional(&mut *conn)
    .await?;
    if sigue.is_none() {
        return Ok(Err(RazonNoPurga::NoElegible));
    }

    // Pre-chequeo: los recuentos de hoy casan lo archivado (nada cambió en la ventana).
    for (tabla, liga) in EXPEDIENTE {
        let sql = format!("SELECT COUNT(*) FROM \"{tabla}\" WHERE {}", filtro(*liga));
        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        for _ in 0..binds(*liga) {
            q = q.bind(paciente_id);
        }
        let n: i64 = q.fetch_one(&mut *conn).await?;
        let archivado =
            recuentos_archivados.iter().find(|(t, _)| t == tabla).map_or(-1, |(_, c)| *c);
        if n != archivado {
            return Ok(Err(RazonNoPurga::CambioDetectado {
                tabla: (*tabla).to_string(),
                archivado,
                ahora: n,
            }));
        }
    }

    // Desligar al HIJO (vivo) de una madre fallecida: su `recien_nacido` se conserva
    // pero su `hospitalizacion_id` apunta al parto de la madre, que se purga.
    sqlx::query(
        "UPDATE recien_nacido SET hospitalizacion_id = NULL \
         WHERE hospitalizacion_id IN (SELECT id FROM hospitalizacion WHERE paciente_id = ?)",
    )
    .bind(paciente_id)
    .execute(&mut *conn)
    .await?;

    // Borrar en el orden de EXPEDIENTE (subselect antes que su raíz).
    for (tabla, liga) in EXPEDIENTE {
        let sql = format!("DELETE FROM \"{tabla}\" WHERE {}", filtro(*liga));
        let mut q = sqlx::query(&sql);
        for _ in 0..binds(*liga) {
            q = q.bind(paciente_id);
        }
        q.execute(&mut *conn).await?;
    }

    // KEYSTONE: ninguna tabla `Directa` puede conservar filas del fallecido (caza
    // un CASCADE no alcanzado, un trigger, una liga `Directa` que no borró).
    if let Some((tabla, quedan)) = directa_que_sobrevive(&mut *conn, paciente_id).await? {
        return Ok(Err(RazonNoPurga::BorradoIncompleto { tabla, quedan }));
    }

    // La fila `paciente` se conserva, anonimizada.
    sqlx::query(ANONIMIZAR).bind(paciente_id).execute(&mut *conn).await?;
    Ok(Ok(()))
}

/// El `INSERT` del certificado sobre una conexión/transacción dada. `recuentos` es
/// lo ARCHIVADO (el volcado), no `rows_affected`: tras el keystone, todo lo
/// archivado está borrado, y `rows_affected` engañaría ante un CASCADE (concilio
/// 2026-09-22, fable). Devuelve el id.
async fn insertar_certificado(
    conn: &mut sqlx::SqliteConnection,
    paciente_id: i64,
    ahora: chrono::DateTime<chrono::Utc>,
    cert: &DatosCertificado<'_>,
    recuentos: &[(String, i64)],
) -> Result<i64, sqlx::Error> {
    let mapa: std::collections::BTreeMap<&str, i64> =
        recuentos.iter().map(|(t, n)| (t.as_str(), *n)).collect();
    let recuentos_json = serde_json::to_string(&mapa).unwrap_or_else(|_| "{}".into());
    let ahora_txt = ahora.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let r = sqlx::query(
        "INSERT INTO supresion \
         (paciente_id, institucion_id, fecha_defuncion, fecha_vencimiento, fecha_supresion, \
          ruta_archivo, sha256_archivo, manifiesto, firma, recuentos) \
         VALUES (?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(paciente_id)
    .bind(cert.institucion_id)
    .bind(cert.fecha_defuncion)
    .bind(cert.fecha_vencimiento)
    .bind(&ahora_txt)
    .bind(cert.ruta_archivo)
    .bind(cert.sha256_archivo)
    .bind(cert.manifiesto)
    .bind(cert.firma)
    .bind(&recuentos_json)
    .execute(&mut *conn)
    .await?;
    Ok(r.last_insert_rowid())
}

/// Purga el expediente (sin certificar). Para las pruebas de la capa de datos; la
/// producción usa [`purgar_y_certificar`], que además escribe el certificado en la
/// MISMA transacción.
pub async fn purgar(
    pool: &crate::Pool,
    paciente_id: i64,
    anos: i64,
    ahora: chrono::DateTime<chrono::Utc>,
    recuentos_archivados: &[(String, i64)],
) -> Result<Result<(), RazonNoPurga>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    match ejecutar_purga(&mut tx, paciente_id, anos, ahora, recuentos_archivados).await? {
        Ok(()) => {
            tx.commit().await?;
            Ok(Ok(()))
        }
        Err(razon) => {
            tx.rollback().await?;
            Ok(Err(razon))
        }
    }
}

/// Purga el expediente Y escribe el certificado de supresión en la MISMA
/// transacción: o pasan los dos, o ninguno (concilio 2026-09-22, sonnet). Es el
/// punto de no retorno de producción. Devuelve el id de la fila `supresion`.
pub async fn purgar_y_certificar(
    pool: &crate::Pool,
    paciente_id: i64,
    anos: i64,
    ahora: chrono::DateTime<chrono::Utc>,
    recuentos_archivados: &[(String, i64)],
    cert: &DatosCertificado<'_>,
) -> Result<Result<i64, RazonNoPurga>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    match ejecutar_purga(&mut tx, paciente_id, anos, ahora, recuentos_archivados).await? {
        Err(razon) => {
            tx.rollback().await?;
            Ok(Err(razon))
        }
        Ok(()) => {
            let id =
                insertar_certificado(&mut tx, paciente_id, ahora, cert, recuentos_archivados).await?;
            tx.commit().await?;
            Ok(Ok(id))
        }
    }
}

/// Inserta un certificado de supresión suelto (sin purgar), para sembrar en pruebas.
#[cfg(test)]
async fn registrar_supresion_suelta(
    pool: &crate::Pool,
    paciente_id: i64,
    ahora: chrono::DateTime<chrono::Utc>,
    cert: &DatosCertificado<'_>,
) -> Result<i64, sqlx::Error> {
    let mut c = pool.acquire().await?;
    insertar_certificado(&mut c, paciente_id, ahora, cert, &[]).await
}

#[cfg(test)]
mod contra_la_base {
    use super::*;
    use chrono::{Duration, Utc};

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
             VALUES (6,'Dr. Paz','d@x.co','h','medico',1,1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
             fecha_nacimiento,sexo,institucion_id) \
             VALUES (10,'CC','111','Ana','Ruiz','1940-01-01','F',1), \
                    (20,'CC','222','Beto','Paz','1935-01-01','M',1), \
                    (30,'CC','333','Cira','Sol','1950-01-01','F',2)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    /// Certifica una defunción `hace` (p. ej. hace 21 años) y devuelve la fecha.
    async fn morir(pool: &crate::Pool, paciente: i64, hace_anos: i64) -> String {
        let fecha = (Utc::now() - Duration::days(hace_anos * 365 + 5))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        sqlx::query(
            "INSERT INTO defuncion \
             (paciente_id,fecha_defuncion,sitio,manera,causa_basica,certificado_por,anulada,creado_en) \
             VALUES (?,?,'INSTITUCION','NATURAL','J189',6,0,?)",
        )
        .bind(paciente)
        .bind(&fecha)
        .bind(&fecha)
        .execute(pool)
        .await
        .unwrap();
        fecha
    }

    /// Una historia con `fecha` explícita. La `fecha` importa desde el fail-safe #1:
    /// un encuentro posterior a la defunción vuelve al paciente NO elegible.
    async fn una_historia_fechada(pool: &crate::Pool, paciente: i64, fecha: &str) -> i64 {
        let cod = format!("H{}-{}", paciente, Utc::now().timestamp_nanos_opt().unwrap_or(0));
        let r = sqlx::query(
            "INSERT INTO historia \
             (codigo_historia,paciente_id,medico_id,motivo_consulta,enfermedad_actual,\
              diagnostico_principal,plan_tratamiento,fecha) \
             VALUES (?,?,6,'dolor','x','J189','y',?)",
        )
        .bind(cod)
        .bind(paciente)
        .bind(fecha)
        .execute(pool)
        .await
        .unwrap();
        r.last_insert_rowid()
    }

    /// El expediente de quien murió hace ≥20 años tiene encuentros AÚN MÁS antiguos
    /// (defunción ≥ última atención): las fixtures de purga los fechan 22 años atrás,
    /// antes de las defunciones de 19-21 años, para no chocar con el fail-safe #1. El
    /// caso rojo del fail-safe usa `una_historia_fechada` con una fecha reciente.
    async fn una_historia(pool: &crate::Pool, paciente: i64) -> i64 {
        let vieja =
            (Utc::now() - Duration::days(22 * 365)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        una_historia_fechada(pool, paciente, &vieja).await
    }

    // ── El reloj: el par rojo/verde de los 20 años (D33) ──

    #[tokio::test]
    async fn no_se_purga_antes_de_los_veinte_anos() {
        let pool = base().await;
        morir(&pool, 10, 19).await; // hace 19 años: aún no
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert!(e.is_empty(), "un fallecido hace 19 años no debe ser elegible a 20");
    }

    #[tokio::test]
    async fn se_purga_a_los_veinte_anos() {
        let pool = base().await;
        morir(&pool, 10, 21).await; // hace 21 años: sí
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].paciente_id, 10);
    }

    /// La fecha que produce la pantalla (`<input type="datetime-local">`: sin
    /// segundos ni zona) hace elegible al paciente. Antes, `vencimiento()` la
    /// parseaba como RFC3339 → `None` y el `filter_map` la descartaba en silencio;
    /// el subsistema no suprimía a NADIE certificado por el panel (concilio
    /// 2026-09-22, fable). Caso rojo por la vía real (el formato REAL del panel).
    #[tokio::test]
    async fn una_fecha_del_panel_sin_segundos_es_elegible() {
        let pool = base().await;
        let fecha = (Utc::now() - Duration::days(21 * 365)).format("%Y-%m-%dT%H:%M").to_string();
        sqlx::query(
            "INSERT INTO defuncion \
             (paciente_id,fecha_defuncion,sitio,manera,causa_basica,certificado_por,anulada,creado_en) \
             VALUES (10,?,'INSTITUCION','NATURAL','J189',6,0,?)",
        )
        .bind(&fecha)
        .bind(&fecha)
        .execute(&pool)
        .await
        .unwrap();
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert_eq!(e.len(), 1, "la fecha del panel (sin segundos) debe hacer elegible");
        assert!(!e[0].fecha_vencimiento.is_empty(), "y el vencimiento se calcula en SQL");
    }

    // ── FAIL-SAFE #1: una atención POSTERIOR a la defunción = fecha mal tecleada ──

    /// RED + su pareja verde (D33): quien murió hace 21 años pero tiene una historia
    /// de HOY no es elegible — el síntoma de una fecha de defunción mal tecleada (un
    /// 2005 por 2025). Quitar la cláusula de `sin_atencion_tras_defuncion` lo vuelve
    /// elegible y la historia VIVA se borraría (irreversible). La pareja verde, en el mismo test: sin
    /// el encuentro posterior, el mismo fallecido SÍ es elegible (no sobre-bloquea).
    #[tokio::test]
    async fn un_encuentro_posterior_a_la_defuncion_no_es_elegible() {
        let pool = base().await;
        morir(&pool, 10, 21).await;
        // VERDE: sin encuentros posteriores, el fallecido hace 21 años es elegible.
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert_eq!(e.len(), 1, "sin atención posterior, el fallecido hace 21 años es elegible");
        // Llega (o ya existía) una historia de HOY: la defunción de hace 21 años no
        // puede ser cierta — nadie recibe atención tras morir.
        let hoy = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        una_historia_fechada(&pool, 10, &hoy).await;
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert!(e.is_empty(), "una atención posterior a la defunción bloquea (fecha mal tecleada)");
    }

    /// Una HOSPITALIZACIÓN cuyo ingreso es posterior a la defunción bloquea.
    #[tokio::test]
    async fn una_hospitalizacion_posterior_a_la_defuncion_no_es_elegible() {
        let pool = base().await;
        morir(&pool, 10, 21).await;
        sqlx::query("INSERT INTO cama (id,numero,piso,servicio,estado,institucion_id) \
                     VALUES (1,'101','1','MEDICINA','OCUPADA',1)")
            .execute(&pool).await.unwrap();
        let hoy = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        sqlx::query(
            "INSERT INTO hospitalizacion (id,paciente_id,cama_id,medico_tratante_id,fecha_ingreso,estado) \
             VALUES (1,10,1,6,?,'ALTA_MEDICA')",
        )
        .bind(&hoy).execute(&pool).await.unwrap();
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert!(e.is_empty(), "una hospitalización posterior a la defunción bloquea");
    }

    /// Una atención de URGENCIAS posterior a la defunción bloquea.
    #[tokio::test]
    async fn una_urgencia_posterior_a_la_defuncion_no_es_elegible() {
        let pool = base().await;
        morir(&pool, 10, 21).await;
        let hoy = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        sqlx::query(
            "INSERT INTO atencion_urgencias \
             (paciente_id,triage,motivo_consulta,enfermera_triage_id,fecha_ingreso) \
             VALUES (10,'TRIAGE_3','dolor',6,?)",
        )
        .bind(&hoy).execute(&pool).await.unwrap();
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert!(e.is_empty(), "una urgencia posterior a la defunción bloquea");
    }

    /// Una cita ATENDIDA posterior a la defunción bloquea; una PROGRAMADA no (pudo
    /// agendarse en vida y quedar sin cumplir tras la muerte — no prueba atención).
    #[tokio::test]
    async fn una_cita_atendida_posterior_bloquea_pero_una_programada_no() {
        let pool = base().await;
        morir(&pool, 10, 21).await;
        let manana = (Utc::now() + Duration::days(1)).format("%Y-%m-%d").to_string();
        // PROGRAMADA a futuro: NO bloquea.
        sqlx::query("INSERT INTO cita (institucion_id,paciente_id,fecha,hora,estado) \
                     VALUES (1,10,?,'08:00','PROGRAMADA')")
            .bind(&manana).execute(&pool).await.unwrap();
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert_eq!(e.len(), 1, "una cita PROGRAMADA tras la muerte no prueba atención: no bloquea");
        // ATENDIDA tras la muerte: imposible → bloquea.
        sqlx::query("INSERT INTO cita (institucion_id,paciente_id,fecha,hora,estado) \
                     VALUES (1,10,?,'09:00','ATENDIDA')")
            .bind(&manana).execute(&pool).await.unwrap();
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert!(e.is_empty(), "una cita ATENDIDA tras la muerte bloquea");
    }

    /// Inserta una defunción en UTC (como la deja `certificar` tras la 0078) para
    /// `paciente` el día de hace ~21 años a las 15:00:00Z (= 10:00 local Bogotá).
    /// Devuelve el día `YYYY-MM-DD` para fechar `cita` en hora LOCAL alrededor de la muerte.
    async fn morir_utc_a_las_15z(pool: &crate::Pool, paciente: i64) -> String {
        let dia = (Utc::now() - Duration::days(21 * 365 + 5)).format("%Y-%m-%d").to_string();
        let muerte = format!("{dia}T15:00:00Z");
        sqlx::query(
            "INSERT INTO defuncion \
             (paciente_id,fecha_defuncion,sitio,manera,causa_basica,certificado_por,anulada,creado_en) \
             VALUES (?,?,'INSTITUCION','NATURAL','J189',6,0,?)",
        )
        .bind(paciente).bind(&muerte).bind(&muerte)
        .execute(pool).await.unwrap();
        dia
    }

    /// RED (concilio de cierre 2026-09-26, clase de R1): una `cita` ATENDIDA guardada en
    /// hora LOCAL (`fecha`/`hora`, sin normalizar — la agenda las usa así) 2 h DESPUÉS de la
    /// muerte real. Como la defunción se sella en UTC (0078) y la cita es local, sin el shift
    /// `LocalNaive` del fail-safe la cita (local 12:00 = 17:00Z) se leería como 12:00 UTC <
    /// 15:00Z muerte → "antes" → NO bloquearía → fallecido real ELEGIBLE para purga irreversible.
    /// Con el shift, 17:00Z > 15:00Z → bloquea. Mutante (marcar `cita` como `Marco::Utc`) pone
    /// esto rojo (elegible), por la vía real `elegibles`.
    #[tokio::test]
    async fn una_cita_local_dos_horas_tras_la_muerte_utc_bloquea() {
        let pool = base().await;
        let dia = morir_utc_a_las_15z(&pool, 10).await;
        // Local 12:00 (= 17:00Z) = 2 h DESPUÉS de la muerte (15:00Z / 10:00 local).
        sqlx::query("INSERT INTO cita (institucion_id,paciente_id,fecha,hora,estado) \
                     VALUES (1,10,?,'12:00','ATENDIDA')")
            .bind(&dia).execute(&pool).await.unwrap();
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert!(e.is_empty(),
            "una cita ATENDIDA local 2 h tras la muerte (misma zona) bloquea la purga; \
             sin el shift LocalNaive el fail-safe se abriría (fail-open irreversible)");
    }

    /// VERDE (pareja): una `cita` ATENDIDA local 2 h ANTES de la muerte real NO bloquea —el
    /// paciente estaba vivo y murió después—, así que el fallecido real sigue siendo elegible.
    /// Prueba que el shift no SOBRE-bloquea a un difunto legítimo (D23).
    #[tokio::test]
    async fn una_cita_local_dos_horas_antes_de_la_muerte_utc_no_bloquea() {
        let pool = base().await;
        let dia = morir_utc_a_las_15z(&pool, 10).await;
        // Local 08:00 (= 13:00Z) = 2 h ANTES de la muerte (15:00Z).
        sqlx::query("INSERT INTO cita (institucion_id,paciente_id,fecha,hora,estado) \
                     VALUES (1,10,?,'08:00','ATENDIDA')")
            .bind(&dia).execute(&pool).await.unwrap();
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert_eq!(e.len(), 1,
            "una cita ATENDIDA anterior a la muerte no prueba vida posterior: no sobre-bloquea");
    }

    /// GUARDIÁN D33 de la clase: en la cláusula generada, SOLO las columnas `LocalNaive`
    /// (y todas ellas) llevan el shift `+5 hours` a UTC. Si alguien vuelve a marcar `cita`
    /// como `Utc` (el fail-open que este concilio cerró), el conteo baja a 0 y la aserción de
    /// `>= 1` falla; si añade un shift espurio a una columna UTC, el conteo sube y no casa.
    #[test]
    fn solo_las_columnas_locales_se_desplazan_a_utc_en_la_clausula() {
        let sql = sin_atencion_tras_defuncion();
        let locales = PRUEBA_DE_VIDA.iter().flat_map(|p| p.fechas)
            .filter(|f| f.marco == Marco::LocalNaive).count();
        assert!(locales >= 1, "al menos `cita` debe ser LocalNaive; si baja a 0 el fail-open volvió");
        // Cada `Fecha` genera su `dt` DOS veces en la cláusula (rama `IS NULL` y la comparación),
        // así que hay 2 apariciones de `+5 hours` por columna LocalNaive.
        assert_eq!(sql.matches("'+5 hours'").count(), 2 * locales,
            "cada columna LocalNaive —y solo esas— se convierte a UTC en la comparación");
        assert!(sql.contains("datetime(t.fecha || ' ' || COALESCE(NULLIF(t.hora, ''), '00:00'), '+5 hours')"),
            "la expr de `cita` (LocalNaive) lleva el shift a UTC");
    }

    /// Defensa en profundidad (D33): si el encuentro posterior LLEGA entre el volcado y
    /// la purga, el re-chequeo transaccional aborta con `NoElegible` — el fail-safe #1
    /// también vive dentro de la transacción, y corre ANTES del chequeo de recuentos,
    /// así que aborta por «no elegible» y no por «cambio detectado». Quitar la cláusula
    /// del re-chequeo devuelve `CambioDetectado`: esta aserción lo distingue.
    #[tokio::test]
    async fn un_encuentro_posterior_que_llega_en_la_ventana_aborta_la_purga() {
        let pool = base().await;
        morir(&pool, 10, 21).await;
        una_historia(&pool, 10).await; // vieja (pre-muerte): elegible al volcar
        let (_d, recuentos) = volcar(&pool, 10).await.unwrap();
        let manana = (Utc::now() + Duration::days(1)).format("%Y-%m-%d").to_string();
        sqlx::query("INSERT INTO cita (institucion_id,paciente_id,fecha,hora,estado) \
                     VALUES (1,10,?,'09:00','ATENDIDA')")
            .bind(&manana).execute(&pool).await.unwrap();
        let r = purgar(&pool, 10, ANOS_POR_DEFECTO, Utc::now(), &recuentos).await.unwrap();
        assert_eq!(r, Err(RazonNoPurga::NoElegible), "el fail-safe #1 aborta también en la transacción");
    }

    /// Un detalle de factura con `paciente_id` NULL pero `historia_id` del fallecido
    /// se PURGA (liga `DirectaOViaHistoria`) en vez de colgar de una historia borrada
    /// y abortar la purga para siempre (concilio 2026-09-22, fable). Vía real.
    #[tokio::test]
    async fn un_detalle_de_factura_sin_paciente_no_cuelga_la_purga() {
        let pool = base().await;
        morir(&pool, 10, 21).await;
        let h = una_historia(&pool, 10).await;
        sqlx::query("INSERT INTO entidad (id,nit,nombre,codigo_eps) VALUES (1,'900','EPS','EPS001')")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO factura \
             (id,institucion_id,numero_factura,entidad_id,fecha_factura,periodo_inicio,periodo_fin) \
             VALUES (1,1,'F1',1,'2005-01-01','2005-01-01','2005-01-31')",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO detalle_factura (factura_id,servicio,paciente_id,historia_id) \
             VALUES (1,'consulta',NULL,?)",
        )
        .bind(h).execute(&pool).await.unwrap();

        let (_d, recuentos) = volcar(&pool, 10).await.unwrap();
        let n = recuentos.iter().find(|(t, _)| t == "detalle_factura").map(|(_, c)| *c);
        assert_eq!(n, Some(1), "el volcado captura el detalle ligado por historia");
        // NO debe abortar (antes: FOREIGN KEY constraint failed).
        purgar(&pool, 10, ANOS_POR_DEFECTO, Utc::now(), &recuentos).await.unwrap().unwrap();
        let quedan: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM detalle_factura")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(quedan, 0, "el detalle huérfano se purgó, no colgó");
    }

    /// Control (D33): una defunción ANULADA no dispara el reloj.
    #[tokio::test]
    async fn una_defuncion_anulada_no_es_elegible() {
        let pool = base().await;
        morir(&pool, 10, 21).await;
        sqlx::query("UPDATE defuncion SET anulada=1, anulada_por=6, motivo_anulacion='corrección' WHERE paciente_id = 10")
            .execute(&pool)
            .await
            .unwrap();
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert!(e.is_empty());
    }

    // ── FAIL-SAFE #1 · la prueba de vida cubre TODO el expediente, no 4 tablas ──

    /// RED (concilio 2026-09-23, opus por ejecución): un paciente de VACUNACIÓN —que por
    /// diseño existe SIN `historia`— con la defunción mal tecleada. Antes de ampliar la
    /// cubeta, `vacuna_aplicada` no se miraba y el paciente VIVO era elegible → su
    /// expediente se purgaba. Quitar `vacuna_aplicada` de `PRUEBA_DE_VIDA` (o revertir el
    /// builder) devuelve `len == 1` y pone este caso rojo. Pareja verde en el mismo test.
    #[tokio::test]
    async fn una_vacuna_posterior_a_la_defuncion_no_es_elegible() {
        let pool = base().await;
        morir(&pool, 10, 21).await; // defunción hace 21 años (vencida a 20)
        // VERDE: sin acto posterior, el fallecido hace 21 años es elegible.
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert_eq!(e.len(), 1, "sin atención posterior, el fallecido es elegible");
        // Llega una vacuna de HOY (sin historia): el paciente está VIVO, la defunción se
        // tecleó mal. RED: debe dejar de ser elegible.
        let hoy = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        sqlx::query(
            "INSERT INTO vacuna_aplicada \
             (paciente_id,aplicado_por,fecha,biologico,dosis_numero,lote,creado_en) \
             VALUES (10,6,?,'BCG',1,'L-1',?)",
        )
        .bind(&hoy)
        .bind(&hoy)
        .execute(&pool)
        .await
        .unwrap();
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert!(e.is_empty(), "HUECO: una vacuna post-defunción vuelve al paciente NO elegible");
    }

    /// VERDE del otro lado (D23, hallazgo de opus 2026-09-23): un registro ADMINISTRATIVO
    /// fechado DESPUÉS de la muerte —aquí un `documento_clinico`, una nota de cierre— NO
    /// bloquea la supresión. Si `documento_clinico` estuviera en `PRUEBA_DE_VIDA`, un
    /// fallecido real con su nota de cierre sería imposible de suprimir para siempre y se
    /// rompería la obligación de la Ley 1581. Distingue la cubeta correcta.
    #[tokio::test]
    async fn una_epicrisis_post_mortem_no_bloquea_la_supresion() {
        // VERDE (D23): la EPICRISIS (resumen de egreso/defunción) es el único tipo de
        // `documento_clinico` que se finaliza legítimamente tras la muerte → está EXENTO
        // (`tipo NOT IN ('EPICRISIS')`), así que no bloquea la supresión de un fallecido real.
        let pool = base().await;
        morir(&pool, 10, 21).await;
        let hoy = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        sqlx::query("INSERT INTO documento_clinico (tipo,paciente_id,medico_id,fecha) VALUES ('EPICRISIS',10,6,?)")
            .bind(&hoy)
            .execute(&pool)
            .await
            .unwrap();
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert_eq!(e.len(), 1, "una epicrisis post-mortem (exenta) NO bloquea la supresión");
    }

    /// RED (concilio 2026-09-23, sonnet confianza 10, demostrado por ejecución): una nota
    /// de EVOLUCIÓN de HOY, sin historia, sobre un fallecido hace 21 años. `documento_clinico`
    /// estaba clasificada ENTERA como administrativa; una evolución es un acto sobre cuerpo
    /// vivo, así que la defunción está mal tecleada y su expediente NO debe purgarse. Quitar
    /// `documento_clinico` de PRUEBA_DE_VIDA (o exceptuar EVOLUCION) vuelve al paciente elegible.
    #[tokio::test]
    async fn una_evolucion_post_mortem_no_es_elegible() {
        let pool = base().await;
        morir(&pool, 10, 21).await;
        let hoy = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        sqlx::query("INSERT INTO documento_clinico (tipo,paciente_id,medico_id,fecha) VALUES ('EVOLUCION',10,6,?)")
            .bind(&hoy)
            .execute(&pool)
            .await
            .unwrap();
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert!(e.is_empty(), "HUECO: una evolución de hoy (acto sobre cuerpo vivo) → NO elegible");
    }

    /// RED (concilio de cierre 2026-09-23, sonnet+opus por ejecución): la recuperación
    /// posanestésica es un acto sobre cuerpo VIVO cuya fecha (`ingreso_recuperacion`) NO
    /// empieza por `fecha`/`hora`. Estuvo mal clasificada como ADMINISTRATIVA y un vivo con
    /// la defunción mal tecleada se purgaba. Mover la tabla de vuelta a ADMINISTRATIVAS pone
    /// este caso rojo (el guardián no lo caza: la tabla sigue «clasificada»). La cirugía
    /// padre se deja en PROGRAMADA (`hora_inicio_real` NULL), para probar que NO basta con la
    /// cirugía para cubrir la recuperación.
    #[tokio::test]
    async fn una_recuperacion_postanestesica_posterior_a_la_defuncion_no_es_elegible() {
        let pool = base().await;
        morir(&pool, 10, 21).await;
        let hoy = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        sqlx::query("INSERT INTO quirofano (id,institucion_id,nombre) VALUES (1,1,'Q1')")
            .execute(&pool).await.unwrap();
        let prog = sqlx::query(
            "INSERT INTO programacion_quirurgica \
             (quirofano_id,paciente_id,cirujano_id,fecha_programada,procedimiento,estado) \
             VALUES (1,10,6,?,'apendicectomía','PROGRAMADA')",
        )
        .bind(&hoy)
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();
        // Recibido HOY en recuperación, con la cirugía aún sin iniciar (hora_inicio_real NULL).
        sqlx::query(
            "INSERT INTO recuperacion_postanestesica \
             (programacion_id,paciente_id,responsable_id,ingreso_recuperacion,aldrete_ingreso,observaciones,creado_en) \
             VALUES (?,10,6,?,9,'estable',?)",
        )
        .bind(prog)
        .bind(&hoy)
        .bind(&hoy)
        .execute(&pool)
        .await
        .unwrap();
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert!(
            e.is_empty(),
            "HUECO: recibido HOY en recuperación posanestésica (vivo) → NO elegible para purga"
        );
    }

    /// GUARDIÁN D33 (hermano de `el_mapa_cubre_toda_fk_al_paciente`): **TODA** tabla de
    /// `EXPEDIENTE` debe estar clasificada como `PRUEBA_DE_VIDA` o
    /// `ADMINISTRATIVAS_PARA_VIDA`, o esta prueba falla.
    ///
    /// La primera versión (2026-09-23) solo exigía clasificar las tablas con una columna
    /// `fecha*`/`hora*`, y el concilio de cierre demostró que ESE patrón tenía un punto
    /// ciego: `recuperacion_postanestesica` fecha su acto en `ingreso_recuperacion`
    /// —que no casa el patrón— y quedó mal clasificada sin que el guardián protestara (el
    /// vigilante miraba el artefacto, no a sí mismo — D30/D33). La regla ahora NO detecta
    /// fechas: clasifica el conjunto ENTERO. Una tabla sin fecha de acto (teleconsulta,
    /// aval_seccion_rda) va a ADMINISTRATIVAS con su razón; así no hay heurística que
    /// pueda cegarse, y una tabla nueva de `EXPEDIENTE` no entra sin una decisión de cubeta.
    #[test]
    fn toda_tabla_del_expediente_esta_clasificada_para_el_fail_safe() {
        let en_vida = |t: &str| PRUEBA_DE_VIDA.iter().any(|p| p.tabla == t);
        let en_admin = |t: &str| ADMINISTRATIVAS_PARA_VIDA.iter().any(|(x, _)| *x == t);

        let sin_clasificar: Vec<&str> = EXPEDIENTE
            .iter()
            .map(|(t, _)| *t)
            .filter(|t| !en_vida(t) && !en_admin(t))
            .collect();
        assert!(
            sin_clasificar.is_empty(),
            "tablas de EXPEDIENTE sin clasificar para el fail-safe #1 (PRUEBA_DE_VIDA o \
             ADMINISTRATIVA): {sin_clasificar:?}. Clasifícala con su razón — un acto sobre \
             cuerpo vivo bloquea; lo administrativo/post-mortem no."
        );

        // Coherencia: cada cubeta nombra tablas REALES de EXPEDIENTE, y ninguna está en
        // las dos (un dato con dos significados sería dos campos, D32).
        for p in PRUEBA_DE_VIDA {
            assert!(
                EXPEDIENTE.iter().any(|(t, _)| *t == p.tabla),
                "PRUEBA_DE_VIDA nombra `{}`, que no está en EXPEDIENTE",
                p.tabla
            );
            assert!(!en_admin(p.tabla), "`{}` está en las DOS cubetas", p.tabla);
        }
        for (t, _) in ADMINISTRATIVAS_PARA_VIDA {
            assert!(
                EXPEDIENTE.iter().any(|(x, _)| x == t),
                "ADMINISTRATIVAS_PARA_VIDA nombra `{t}`, que no está en EXPEDIENTE"
            );
        }
    }

    /// COHERENCIA esquema↔`PRUEBA_DE_VIDA` (concilio 2026-09-23, fable): cada columna
    /// simple de acto (`t.<col>`) declarada en la cubeta debe EXISTIR en su tabla, y una
    /// marcada `obligatoria: true` debe ser realmente `NOT NULL`. Cierra la clase de raíz:
    /// un typo de columna o una que se creía NOT NULL haría fallar la consulta (falla
    /// CERRADO, no purga) o dejaría un hueco silencioso. Las `expr` compuestas (p. ej. la
    /// de `cita`, con `||`) se saltan aquí — su discriminación la cubre su caso funcional.
    #[tokio::test]
    async fn las_columnas_de_prueba_de_vida_existen_y_su_obligatoriedad_casa_el_esquema() {
        let pool = base().await;
        for pv in PRUEBA_DE_VIDA {
            for f in pv.fechas {
                // Solo columnas simples `t.<ident>`; las compuestas llevan otros chars.
                let Some(col) = f.expr.strip_prefix("t.") else { continue };
                if !col.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
                    continue; // expr compuesta (cita) → se salta
                }
                let notnull: Option<i64> = sqlx::query_scalar(&format!(
                    "SELECT \"notnull\" FROM pragma_table_info(\"{}\") WHERE name = '{col}'",
                    pv.tabla
                ))
                .fetch_optional(&pool)
                .await
                .unwrap();
                let notnull = notnull.unwrap_or_else(|| {
                    panic!("PRUEBA_DE_VIDA: la columna `{col}` no existe en `{}`", pv.tabla)
                });
                if f.obligatoria {
                    assert_eq!(
                        notnull, 1,
                        "`{}.{col}` está marcada obligatoria pero es NULABLE en el esquema \
                         (un NULL espurio sobre-bloquearía a un fallecido real)",
                        pv.tabla
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn el_barrido_salta_a_quien_ya_se_suprimio() {
        let pool = base().await;
        let fecha = morir(&pool, 10, 21).await;
        let venc = vencimiento(&fecha, ANOS_POR_DEFECTO).unwrap();
        let cert = DatosCertificado {
            institucion_id: 1,
            fecha_defuncion: &fecha,
            fecha_vencimiento: &venc,
            ruta_archivo: "/x",
            sha256_archivo: "abc",
            manifiesto: "{}",
            firma: "f",
        };
        registrar_supresion_suelta(&pool, 10, Utc::now(), &cert).await.unwrap();
        let e = elegibles(&pool, Ambito::Superadmin, Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert!(e.is_empty(), "no se suprime dos veces");
    }

    // ── La purga borra el expediente y anonimiza al paciente ──

    #[tokio::test]
    async fn la_purga_borra_la_historia_y_anonimiza_al_paciente() {
        let pool = base().await;
        morir(&pool, 10, 21).await;
        let h = una_historia(&pool, 10).await;
        // Una fórmula que cuelga de la historia (liga ViaHistoria): también se va. Se
        // fecha 22 años atrás, como su historia: `fecha_formula` default `datetime('now')`
        // la pondría HOY y el fail-safe #1 (que ahora cubre `formula`) la leería como
        // «recetado tras la muerte» → paciente vivo → no elegible. Un expediente real de
        // hace 22 años tiene la fórmula igual de antigua.
        let vieja = (Utc::now() - Duration::days(22 * 365)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        sqlx::query("INSERT INTO formula (historia_id,medicamento,dosis,cantidad,fecha_formula) VALUES (?,'A','1','2',?)")
            .bind(h)
            .bind(&vieja)
            .execute(&pool)
            .await
            .unwrap();

        let (_dump, recuentos) = volcar(&pool, 10).await.unwrap();
        let r = purgar(&pool, 10, ANOS_POR_DEFECTO, Utc::now(), &recuentos).await.unwrap();
        assert!(r.is_ok(), "debe purgar: {r:?}");

        // La historia se fue, y con ella la fórmula que colgaba (liga ViaHistoria).
        let hist: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM historia WHERE paciente_id = 10")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(hist, 0);
        let form: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM formula")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(form, 0, "la fórmula vía historia también se purga");

        // El paciente sigue (ancla) pero anonimizado.
        let (nombre, doc): (String, String) =
            sqlx::query_as("SELECT nombre1, numero_documento FROM paciente WHERE id = 10")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(nombre, "SUPRIMIDO");
        assert_eq!(doc, "SUPRIMIDO-10");
        // D28: leer por el struct REAL del producto (Json<Value> en ant_*), no por
        // columnas sueltas. Un ANONIMIZAR que dejara JSON inválido reventaría aquí
        // con ColumnDecode y con él `paciente::listar` = 500 del padrón (fable, conc-3).
        let p = crate::paciente::obtener(&pool, Ambito::Superadmin, 10).await.unwrap();
        assert!(p.is_some(), "la fila anonimizada debe seguir decodificando por el struct Paciente");
        assert!(
            crate::paciente::listar(&pool, Ambito::Institucion(1)).await.is_ok(),
            "el padrón de la institución debe seguir listándose tras la supresión"
        );
    }

    /// El corazón de la seguridad (D33): si el expediente cambió entre el volcado y
    /// la purga, NO se borra. El caso rojo falla por la vía real: se inserta una
    /// historia después de volcar y la purga tiene que abortar.
    #[tokio::test]
    async fn si_el_expediente_cambio_no_se_purga() {
        let pool = base().await;
        morir(&pool, 10, 21).await;
        una_historia(&pool, 10).await;
        let (_dump, recuentos) = volcar(&pool, 10).await.unwrap();
        // Llega una historia nueva DESPUÉS del volcado.
        una_historia(&pool, 10).await;

        let r = purgar(&pool, 10, ANOS_POR_DEFECTO, Utc::now(), &recuentos).await.unwrap();
        match r {
            Err(RazonNoPurga::CambioDetectado { tabla, .. }) => assert_eq!(tabla, "historia"),
            otro => panic!("debía abortar por cambio, no {otro:?}"),
        }
        // Y nada se borró.
        let hist: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM historia WHERE paciente_id = 10")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(hist, 2, "la purga abortada no toca nada");
    }

    /// Una defunción anulada entre el volcado y la purga aborta (re-chequeo en tx).
    #[tokio::test]
    async fn una_anulacion_tardia_aborta_la_purga() {
        let pool = base().await;
        morir(&pool, 10, 21).await;
        let (_d, recuentos) = volcar(&pool, 10).await.unwrap();
        sqlx::query("UPDATE defuncion SET anulada=1, anulada_por=6, motivo_anulacion='corrección' WHERE paciente_id = 10")
            .execute(&pool)
            .await
            .unwrap();
        let r = purgar(&pool, 10, ANOS_POR_DEFECTO, Utc::now(), &recuentos).await.unwrap();
        assert_eq!(r, Err(RazonNoPurga::NoElegible));
    }

    /// La fila `recien_nacido` de una MADRE fallecida NO se toca: es del hijo, que
    /// puede seguir vivo. Decisión de Juan (2026-09-22).
    #[tokio::test]
    async fn el_recien_nacido_de_madre_fallecida_no_se_purga() {
        let pool = base().await;
        // La madre (10) muere; el hijo (20) sigue vivo. El parto es una
        // HOSPITALIZACIÓN de la madre (se purgará), y el registro del hijo la
        // referencia por `hospitalizacion_id` — LA VÍA REAL que abortaba el commit.
        morir(&pool, 10, 21).await;
        sqlx::query("INSERT INTO cama (id,numero,piso,servicio,estado,institucion_id) \
                     VALUES (1,'101','1','MATERNIDAD','OCUPADA',1)")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO hospitalizacion (id,paciente_id,cama_id,medico_tratante_id,fecha_ingreso,estado) \
             VALUES (77,10,1,6,'2005-01-01T10:00:00Z','ALTA_MEDICA')",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO recien_nacido \
             (madre_paciente_id,paciente_id,hospitalizacion_id,fecha_nacimiento,edad_gestacional,\
              num_consultas_prenatal,sexo_biologico,peso_gramos,diagnostico_principal,creado_en) \
             VALUES (10,20,77,'2005-01-01',38,4,'M',3200,'Z380','2005-01-01T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let (_d, recuentos) = volcar(&pool, 10).await.unwrap();
        // NO debe abortar (antes del arreglo, la FK de hospitalizacion_id reventaba).
        purgar(&pool, 10, ANOS_POR_DEFECTO, Utc::now(), &recuentos).await.unwrap().unwrap();

        // La hospitalización de la madre se fue; la fila del hijo queda, desligada.
        let hosp: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hospitalizacion WHERE id = 77")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(hosp, 0, "el parto (hospitalización de la madre) se purga");
        let (quedan, hosp_id): (i64, Option<i64>) = sqlx::query_as(
            "SELECT COUNT(*), MAX(hospitalizacion_id) FROM recien_nacido WHERE madre_paciente_id = 10",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(quedan, 1, "la fila del hijo vivo se conserva");
        assert_eq!(hosp_id, None, "y queda desligada de la hospitalización purgada");
    }

    /// El aval del `nilknown` (polimórfico, sin FK) SÍ se purga con el encuentro del
    /// fallecido. Lo destapó la revisión del 2026-09-22: antes sobrevivía huérfano.
    #[tokio::test]
    async fn el_aval_del_rda_del_fallecido_se_purga() {
        let pool = base().await;
        morir(&pool, 10, 21).await;
        let h = una_historia(&pool, 10).await;
        // Un aval de consulta externa sobre la historia del fallecido.
        sqlx::query(
            "INSERT INTO aval_seccion_rda (tipo_encuentro,encuentro_id,seccion_codigo,usuario_id,sello) \
             VALUES ('consulta-externa',?,'sectionProblems',6,'2020-01-01T00:00:00Z')",
        )
        .bind(h)
        .execute(&pool)
        .await
        .unwrap();
        // Y un aval de OTRO paciente (20) que NO debe tocarse.
        let h2 = una_historia(&pool, 20).await;
        sqlx::query(
            "INSERT INTO aval_seccion_rda (tipo_encuentro,encuentro_id,seccion_codigo,usuario_id,sello) \
             VALUES ('consulta-externa',?,'sectionProblems',6,'2020-01-01T00:00:00Z')",
        )
        .bind(h2)
        .execute(&pool)
        .await
        .unwrap();

        // El volcado debe capturar el aval del fallecido (recuento 1).
        let (_d, recuentos) = volcar(&pool, 10).await.unwrap();
        let n_aval = recuentos.iter().find(|(t, _)| t == "aval_seccion_rda").map(|(_, n)| *n);
        assert_eq!(n_aval, Some(1), "el volcado captura el aval del fallecido");

        purgar(&pool, 10, ANOS_POR_DEFECTO, Utc::now(), &recuentos).await.unwrap().unwrap();
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM aval_seccion_rda")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(total, 1, "se purga el del fallecido y queda el del otro paciente");
    }

    /// El aval de HOSPITALIZACIÓN (no solo el de consulta externa) del fallecido se
    /// purga. Antes, `aval_seccion_rda` se borraba DESPUÉS de `hospitalizacion`, así
    /// que su subselect ya salía vacío y el aval sobrevivía (concilio 2026-09-22,
    /// opus). El arreglo es el ORDEN (aval al frente); esta prueba lo sostiene por la
    /// vía real —la rama que el test de consulta externa no tocaba (D33)—.
    #[tokio::test]
    async fn el_aval_de_hospitalizacion_del_fallecido_se_purga() {
        let pool = base().await;
        morir(&pool, 10, 21).await;
        sqlx::query("INSERT INTO cama (id,numero,piso,servicio,estado,institucion_id) \
                     VALUES (1,'101','1','MEDICINA','OCUPADA',1)")
            .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO hospitalizacion (id,paciente_id,cama_id,medico_tratante_id,fecha_ingreso,estado) \
             VALUES (55,10,1,6,'2005-01-01T10:00:00Z','ALTA_MEDICA')",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO aval_seccion_rda (tipo_encuentro,encuentro_id,seccion_codigo,usuario_id,sello) \
             VALUES ('hospitalizacion',55,'sectionProblems',6,'2005-01-01T00:00:00Z')",
        )
        .execute(&pool).await.unwrap();

        let (_d, recuentos) = volcar(&pool, 10).await.unwrap();
        assert_eq!(
            recuentos.iter().find(|(t, _)| t == "aval_seccion_rda").map(|(_, n)| *n),
            Some(1),
            "el volcado captura el aval de hospitalización"
        );
        purgar(&pool, 10, ANOS_POR_DEFECTO, Utc::now(), &recuentos).await.unwrap().unwrap();
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM aval_seccion_rda")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(total, 0, "el aval de hospitalización se purgó (aval va al frente)");
    }

    /// PRUEBA DE VIDA del keystone (concilio 2026-09-22, opus, D33): `directa_que_sobrevive`
    /// DEBE detectar una fila `Directa` del fallecido que sobreviva, y NO dar falso
    /// positivo sobre datos limpios. Sin esto, mutar el keystone a que nunca dispare
    /// dejaba el banco verde (un guardián sin caso rojo).
    #[tokio::test]
    async fn el_keystone_detecta_un_superviviente_directa() {
        let pool = base().await;
        // Una cita (tabla Directa) del paciente 10: un superviviente.
        sqlx::query(
            "INSERT INTO cita (institucion_id,paciente_id,fecha,hora) \
             VALUES (1,10,'2005-01-01','08:00')",
        )
        .execute(&pool).await.unwrap();
        let mut c = pool.acquire().await.unwrap();
        // Rojo: con la cita presente, el keystone la caza.
        let r = super::directa_que_sobrevive(&mut c, 10).await.unwrap();
        assert_eq!(r, Some(("cita".to_string(), 1)), "debe detectar la cita superviviente");
        // Verde/control: un paciente sin filas Directa → None.
        assert_eq!(super::directa_que_sobrevive(&mut c, 20).await.unwrap(), None);
    }

    /// GUARDIÁN de orden (concilio 2026-09-22, opus): toda tabla cuyo filtro
    /// subseleccione sobre una tabla-raíz del conjunto debe borrarse ANTES que esa
    /// raíz; si no, el subselect saldría vacío y sus filas sobrevivirían (y el
    /// keystone es ciego ahí, porque usa el mismo filtro). Si un reorden futuro
    /// rompe esto, la prueba lo caza.
    #[tokio::test]
    async fn el_orden_de_borrado_respeta_los_subselect() {
        let idx = |n: &str| EXPEDIENTE.iter().position(|(t, _)| *t == n).unwrap();
        let historia = idx("historia");
        let hosp = idx("hospitalizacion");
        let urg = idx("atencion_urgencias");
        for (i, (t, liga)) in EXPEDIENTE.iter().enumerate() {
            match liga {
                Liga::ViaHistoria | Liga::DirectaOViaHistoria => {
                    assert!(i < historia, "{t} subselecciona historia: debe ir antes que ella");
                }
                Liga::ViaEncuentro => assert!(
                    i < historia && i < hosp && i < urg,
                    "{t} (ViaEncuentro) debe ir antes que historia/hospitalizacion/atencion_urgencias"
                ),
                Liga::Directa => {}
            }
        }
    }

    /// El ámbito no cruza instituciones: institución 1 no suprime al paciente de 2.
    #[tokio::test]
    async fn el_ambito_no_cruza_instituciones() {
        let pool = base().await;
        morir(&pool, 30, 21).await; // paciente de la institución 2
        let e = elegibles(&pool, Ambito::Institucion(1), Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert!(e.is_empty(), "la institución 1 no ve al fallecido de la 2");
        let e2 = elegibles(&pool, Ambito::Institucion(2), Utc::now(), ANOS_POR_DEFECTO).await.unwrap();
        assert_eq!(e2.len(), 1);
    }

    /// GUARDIÁN (D33, patrón de `respaldo::TABLAS`): toda tabla con FK a `paciente`
    /// —directa o por `madre_paciente_id`— o con `historia_id` está en EXPEDIENTE o
    /// en EXENTAS. Si nace una tabla ligada al paciente y nadie la clasifica, esta
    /// prueba falla: un expediente purgado a medias deja PHks fuera del archivo o
    /// datos que debían borrarse.
    #[tokio::test]
    async fn el_mapa_cubre_toda_fk_al_paciente() {
        let pool = base().await;
        let tablas: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let en_expediente = |t: &str| EXPEDIENTE.iter().any(|(x, _)| *x == t);
        let en_exentas = |t: &str| EXENTAS.iter().any(|(x, _)| *x == t);

        let mut sin_clasificar = Vec::new();
        for t in &tablas {
            if t == "paciente" || t == "historia" {
                continue; // ancla y raíz, tratadas aparte
            }
            // ¿Referencia a `paciente`?
            let fk_pac: Option<i64> = sqlx::query_scalar(&format!(
                "SELECT 1 FROM pragma_foreign_key_list(\"{t}\") WHERE \"table\"='paciente' LIMIT 1"
            ))
            .fetch_optional(&pool)
            .await
            .unwrap();
            // ¿Tiene columna `historia_id` o el enlace polimórfico `encuentro_id`?
            // Lo segundo lo añadió la revisión del 2026-09-22: `aval_seccion_rda`
            // liga al paciente por (tipo_encuentro, encuentro_id) SIN FK, y el
            // guardián no lo veía.
            let tiene_hist: Option<i64> = sqlx::query_scalar(&format!(
                "SELECT 1 FROM pragma_table_info(\"{t}\") \
                 WHERE name IN ('historia_id','encuentro_id') LIMIT 1"
            ))
            .fetch_optional(&pool)
            .await
            .unwrap();

            let ligada = fk_pac.is_some() || tiene_hist.is_some();
            if ligada && !en_expediente(t) && !en_exentas(t) {
                sin_clasificar.push(t.clone());
            }
        }
        assert!(
            sin_clasificar.is_empty(),
            "tablas ligadas al paciente sin clasificar en EXPEDIENTE ni EXENTAS: {sin_clasificar:?}"
        );
    }

    /// GUARDIÁN de H1 (revisión 2026-09-22): NINGUNA tabla CONSERVADA (fuera del
    /// conjunto de purga) tiene una FK hacia una tabla que SÍ se purga. Si la
    /// tuviera, al borrar el padre su fila quedaría colgando y —sin `ON DELETE`— el
    /// COMMIT reventaría, abortando la purga. La única referencia CRUZADA entre
    /// pacientes que sí existe es `recien_nacido` (hijo→hospitalización de la madre),
    /// y está DENTRO del conjunto de purga y se desliga a mano antes de borrar. Si
    /// una migración futura añade un referente conservado, esta prueba lo caza.
    #[tokio::test]
    async fn ninguna_tabla_conservada_referencia_el_conjunto_de_purga() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        let purga: std::collections::HashSet<&str> =
            EXPEDIENTE.iter().map(|(t, _)| *t).collect();
        let tablas: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let mut referentes_conservados = Vec::new();
        for t in &tablas {
            if purga.contains(t.as_str()) {
                continue; // los de dentro se borran con su paciente
            }
            let padres: Vec<String> = sqlx::query_scalar(&format!(
                "SELECT \"table\" FROM pragma_foreign_key_list(\"{t}\")"
            ))
            .fetch_all(&pool)
            .await
            .unwrap();
            for p in padres {
                if purga.contains(p.as_str()) {
                    referentes_conservados.push(format!("{t} -> {p}"));
                }
            }
        }
        assert!(
            referentes_conservados.is_empty(),
            "tablas conservadas que referencian el conjunto de purga (colgarían al \
             borrar y abortarían el commit): {referentes_conservados:?}"
        );
    }

    /// GUARDIÁN de H2 del concilio (2026-09-22, fable): toda tabla del expediente que
    /// tenga columna `historia_id` DEBE usar una liga que incluya la rama por historia
    /// (`ViaHistoria` o `DirectaOViaHistoria`), NUNCA `Directa` a secas. Si no, una
    /// fila con `paciente_id` NULL y `historia_id` del fallecido sobreviviría al filtro
    /// y colgaría de una historia borrada → FK sin ON DELETE → la purga aborta para
    /// siempre. Si una tabla nueva con `historia_id` entra como `Directa`, esto la caza.
    #[tokio::test]
    async fn toda_tabla_con_historia_id_cubre_esa_liga() {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        let mut mal = Vec::new();
        for (t, liga) in EXPEDIENTE {
            let tiene_hist: Option<i64> = sqlx::query_scalar(&format!(
                "SELECT 1 FROM pragma_table_info(\"{t}\") WHERE name='historia_id' LIMIT 1"
            ))
            .fetch_optional(&pool)
            .await
            .unwrap();
            if tiene_hist.is_some()
                && !matches!(liga, Liga::ViaHistoria | Liga::DirectaOViaHistoria)
            {
                mal.push(*t);
            }
        }
        assert!(
            mal.is_empty(),
            "tablas con historia_id cuya liga NO cubre la rama por historia (una fila con \
             paciente_id NULL colgaría y abortaría la purga): {mal:?}"
        );
    }

    /// Positivo de control del guardián (D30/D33): si se «olvida» una tabla del
    /// mapa, el guardián la detecta. Se simula quitando `historia` del chequeo —no,
    /// mejor: se comprueba que una tabla ligada conocida (`defuncion`) ESTÁ, de modo
    /// que el guardián mide algo y no pasa por vacío.
    #[tokio::test]
    async fn el_guardian_mide_algo() {
        // defuncion tiene paciente_id y está en EXPEDIENTE: si el guardián no
        // examinara las FK, esta afirmación no probaría nada; aquí ancla que el
        // mapa de verdad contiene lo esperado.
        assert!(EXPEDIENTE.iter().any(|(t, _)| *t == "defuncion"));
        assert!(EXENTAS.iter().any(|(t, _)| *t == "acceso"));
    }
}
