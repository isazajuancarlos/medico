// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! **El productor de PRODUCCIÓN del RDA de encuentro clínico** (#603 fase 4).
//!
//! El dominio (`medico_dominio::ihce::clinico::ensamblar`) ya sabe construir el
//! Bundle FHIR de un encuentro a partir de un `Insumo` —y está probado byte a byte
//! contra el ejemplo oficial—. Lo que faltaba, y vive aquí, es lo de en medio:
//! **leer la historia de la base, resolver los `display` contra los catálogos, y
//! armar ese `Insumo`**. Hasta ahora el `Insumo` solo se construía en las pruebas
//! del dominio; ningún camino de producción lo alimentaba desde la BD.
//!
//! # Devuelve un MEDIDOR, no siempre un documento
//!
//! El resultado ([`Preparacion`]) es `Listo` con el Bundle **o** `Faltan` con la
//! lista de lo que hay que capturar antes de poder emitirlo. Y hoy, para una
//! historia real, será casi siempre `Faltan`: la puerta del `nilknown` exige el
//! aval humano de las secciones vacías (que aún no tiene pantalla —F4.5—), y el
//! nombre partido del profesional aún no se captura. Eso es deliberado: un RDA a
//! medias que el servidor acepta es peor que uno que no sale (directiva 20), así
//! que el productor DICE qué falta en vez de rellenarlo.
//!
//! # NO envía nada
//!
//! Preparar es puro cómputo local: leer y ensamblar. Encolar el Bundle en
//! `cola_rda` para que el transporte lo suba al mecanismo IHCE es OTRA puerta
//! (F4.6), bloqueada en las credenciales de sandbox y sujeta a su propia revisión
//! de seguridad —encolar manda datos clínicos a la plataforma nacional—. Este
//! módulo se detiene en el Bundle.
//!
//! # De momento, solo CONSULTA EXTERNA
//!
//! Es el encuentro más liviano y el que fija el patrón (como el RDA de paciente
//! entre los cuatro documentos). Urgencias y hospitalización necesitan además
//! ampliar sus lectores (traen el diagnóstico de egreso y la causa en columnas
//! que hoy el struct de lectura no expone) y capturar el tipo de diagnóstico de
//! ingreso/egreso, que la Resolución 948 no pide para AU/AH; van en la fase
//! siguiente.

use medico_dominio::ihce::actores::{self, Codigo, Institucion, Nombre, Profesional};
use medico_dominio::ihce::clinico::{ensamblar, Encuentro};
use medico_dominio::ihce::paciente::{Diagnostico, Eps, Insumo, Servicio};
use medico_dominio::ihce::{Aval, Confirmacion};
use medico_dominio::ihce::terminologia::{
    self, SISTEMA_DISCAPACIDAD, SISTEMA_ETNIA, SISTEMA_IDENTIDAD_GENERO, SISTEMA_TIPO_DOCUMENTO,
};
use medico_dominio::ihce::sexo_rda;
use medico_dominio::Ambito;

/// El resultado de preparar un RDA: o está LISTO, o FALTAN datos por capturar.
#[derive(Debug)]
pub enum Preparacion {
    /// El Bundle FHIR, serializado, para (más adelante, F4.6) encolarse.
    Listo(serde_json::Value),
    /// Lo que impide emitir el RDA, en frases legibles: las faltas del
    /// ensamblador (dominio) más las de RESOLUCIÓN de catálogos que el productor
    /// no pudo cerrar. Todas juntas, para que quien capture lo vea de una vez y
    /// no en cuatro intentos.
    Faltan(Vec<String>),
}

/// Una sección vacía elegible para el aval del `nilknown`, con su estado, para la
/// pantalla de captura (#603 F4.5-C). El clínico la ve como una casilla.
#[derive(Debug, Clone)]
pub struct SeccionAvalable {
    /// El código LOINC de la sección (lo que se pasa a `avalar_secciones`).
    pub codigo: String,
    /// El título en español para mostrar.
    pub titulo: String,
    /// ¿Ya la avaló un clínico? (marca la casilla).
    pub avalada: bool,
}

/// El estado de preparación de un RDA para la PANTALLA (#603 F4.5-C): qué secciones
/// vacías puede avalar el clínico (con su estado), qué OTRAS faltas lo bloquean
/// —datos que se capturan en otra parte, no avalables aquí— y si ya está listo.
///
/// Es la versión estructurada de [`Preparacion`]: la pantalla no puede pintar
/// casillas con una lista de strings opacos, y `Falta` no lleva el código de la
/// sección. Aquí las secciones vienen separadas de las demás faltas.
#[derive(Debug)]
pub struct EstadoPreparacion {
    /// Las secciones vacías elegibles para aval, con su estado (casillas).
    pub secciones: Vec<SeccionAvalable>,
    /// Lo que bloquea el RDA y NO es un aval de sección (nombre del profesional,
    /// catálogos, entorno): se captura en otra pantalla, aquí solo se informa.
    pub otras_faltas: Vec<String>,
    /// `true` si no falta nada: todas las secciones avaladas y sin otras faltas.
    pub listo: bool,
}

/// El resultado interno del camino de lectura+ensamblado, del que se derivan tanto
/// [`Preparacion`] como [`EstadoPreparacion`] sin duplicar la lectura (directiva 9).
struct Interno {
    /// El Bundle serializado, `Some` solo si el ensamblado del dominio cerró.
    bundle: Option<serde_json::Value>,
    /// Faltas de RESOLUCIÓN del productor (catálogos, nombre del profesional…).
    faltan_catalogo: Vec<String>,
    /// Faltas del ENSAMBLADOR del dominio (incluye las de `emptyReason` de las
    /// secciones sin avalar, que se distinguen por `campo == "emptyReason"`).
    faltas_dominio: medico_dominio::ihce::Faltas,
    /// Las secciones vacías elegibles para aval (código, título).
    avalables: Vec<(String, String)>,
    /// Lo que ya está avalado, para cruzar contra `avalables`.
    avaladas: Vec<crate::rda_aval::SeccionAvalada>,
}

/// Un error de preparación que NO es «faltan datos clínicos»: la base falló, o la
/// fila que se pidió no existe en este ámbito. Se separa de [`Preparacion::Faltan`]
/// a propósito: «el clínico no ha capturado la causa externa» y «no existe esa
/// historia» son cosas distintas y quien llama las trata distinto.
///
/// Se implementa a mano (no con `thiserror`) para no añadir esa dependencia a
/// `medico-datos`, que hoy no la usa: sus lectores devuelven `sqlx::Error` a secas.
#[derive(Debug)]
pub enum ErrorRda {
    Bd(sqlx::Error),
    HistoriaNoExiste(i64),
    /// El encuentro (hospitalización/urgencias) no existe en este ámbito. La
    /// consulta externa usa `HistoriaNoExiste`; los otros dos, esta, con su tipo.
    EncuentroNoExiste { tipo: &'static str, id: i64 },
    /// El `tipo_encuentro` no es uno de los tres conocidos. Falla ruidosamente
    /// (directiva 20): la ruta valida el tipo, pero un valor mal escrito no debe
    /// caer en silencio a un productor por defecto.
    TipoDesconocido(String),
    PacienteNoExiste { historia: i64, paciente: i64 },
    ProfesionalNoExiste { historia: i64, profesional: i64 },
    InstitucionNoExiste(i64),
    ProfesionalDeOtraInstitucion { historia: i64, profesional: i64 },
    Serializar(serde_json::Error),
}

impl std::fmt::Display for ErrorRda {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorRda::Bd(e) => write!(f, "error de base preparando el RDA: {e}"),
            ErrorRda::HistoriaNoExiste(id) => {
                write!(f, "no existe la historia {id} en esta institución (o no es suya)")
            }
            ErrorRda::EncuentroNoExiste { tipo, id } => {
                write!(f, "no existe el encuentro {tipo} {id} en esta institución (o no es suyo)")
            }
            ErrorRda::TipoDesconocido(t) => write!(f, "tipo de encuentro desconocido: «{t}»"),
            ErrorRda::PacienteNoExiste { historia, paciente } => write!(
                f,
                "la historia {historia} apunta a un paciente ({paciente}) que no existe en esta institución"
            ),
            ErrorRda::ProfesionalNoExiste { historia, profesional } => {
                write!(f, "la historia {historia} apunta a un profesional ({profesional}) que no existe")
            }
            ErrorRda::InstitucionNoExiste(id) => write!(f, "no existe la institución {id}"),
            ErrorRda::ProfesionalDeOtraInstitucion { historia, profesional } => write!(
                f,
                "la historia {historia} registra un profesional ({profesional}) que no pertenece a \
                 esta institución: no se emite el RDA con un autor de otra sede"
            ),
            ErrorRda::Serializar(e) => write!(f, "el Bundle ensamblado no serializa: {e}"),
        }
    }
}

impl std::error::Error for ErrorRda {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ErrorRda::Bd(e) => Some(e),
            ErrorRda::Serializar(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for ErrorRda {
    fn from(e: sqlx::Error) -> Self {
        ErrorRda::Bd(e)
    }
}

impl From<serde_json::Error> for ErrorRda {
    fn from(e: serde_json::Error) -> Self {
        ErrorRda::Serializar(e)
    }
}

/// La fila del profesional que el RDA necesita, con su nombre PARTIDO (migración
/// 0071). Todo `Option`: el documento existe desde la 0037 y el nombre partido
/// desde la 0071, pero ninguna pantalla los captura aún, así que llegan `NULL` y
/// el ensamblador los marca como falta en vez de emitir un `Practitioner` a
/// medias.
#[derive(sqlx::FromRow)]
struct ProfesionalRda {
    tipo_documento: Option<String>,
    numero_documento: Option<String>,
    nombre1: Option<String>,
    nombre2: Option<String>,
    apellido1: Option<String>,
    apellido2: Option<String>,
}

async fn obtener_profesional(
    pool: &crate::Pool,
    id: i64,
) -> Result<Option<ProfesionalRda>, sqlx::Error> {
    sqlx::query_as(
        "SELECT tipo_documento, numero_documento, nombre1, nombre2, apellido1, apellido2 \
         FROM usuario WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Un `Codigo` cuyo `display` se DERIVA de un catálogo de `terminologia`, o sin
/// display si el código no está en él. Un código presente pero fuera de catálogo
/// no es una falta del RDA (el campo es opcional y demográfico); se emite desnudo,
/// que es lo que el propio dominio hace con el municipio sin nombre.
fn codigo_de<'a>(code: Option<&'a str>, sistema: &str) -> Option<Codigo<'a>> {
    let code = code.map(str::trim).filter(|s| !s.is_empty())?;
    Some(match terminologia::display(sistema, code) {
        Some(d) => Codigo::nuevo(code, d),
        None => Codigo::desnudo(code),
    })
}

/// El camino COMPARTIDO de lectura+ensamblado del RDA de consulta externa, del que
/// Arma el actor `Paciente` del RDA a partir de la fila del paciente y lo ya
/// resuelto contra catálogos, COMPARTIDO por los tres encuentros (directiva 9): el
/// bloque es idéntico salvo `fallecido`, que en consulta externa es `None` —nadie
/// sabe si murió DESPUÉS— y en egreso/urgencias lo declara el desenlace
/// (`actores::fallecido_de_egreso`/`_de_urgencias`). Todo lo prestado (`pac`,
/// `muni`) tiene que seguir vivo hasta después de ensamblar.
fn armar_paciente<'a>(
    pac: &'a crate::paciente::Paciente,
    tipo_doc_display: &'a str,
    genero_fhir: &'a str,
    sexo_colombiano: Option<Codigo<'a>>,
    muni: &'a Option<crate::divipola::Municipio>,
    fallecido: Option<bool>,
) -> actores::Paciente<'a> {
    actores::Paciente {
        tipo_doc: &pac.tipo_documento,
        numero_doc: &pac.numero_documento,
        tipo_doc_display,
        nombre: Nombre {
            nombre1: &pac.nombre1,
            nombre2: pac.nombre2.as_deref(),
            apellido1: &pac.apellido1,
            apellido2: pac.apellido2.as_deref(),
        },
        genero_fhir,
        sexo_colombiano,
        nacimiento: &pac.fecha_nacimiento,
        hora_nacimiento: None,
        // Municipio: nombre de divipola si lo hay, si no el código desnudo (el
        // dominio omite la ciudad y conserva el código DIVIPOLA).
        municipio: pac.cod_municipio.as_deref().filter(|s| !s.trim().is_empty()).map(|c| {
            match muni.as_ref().map(|m| m.nombre.as_str()) {
                Some(n) => Codigo::nuevo(c, n),
                None => Codigo::desnudo(c),
            }
        }),
        // País, nacionalidad y zona: ISO/DIVIPOLA numérico, sin catálogo de nombres
        // vendorizado — código desnudo, como ya hace el RDA de paciente.
        pais: pac.cod_pais.as_deref().filter(|s| !s.trim().is_empty()).map(Codigo::desnudo),
        zona: pac.zona.as_deref().filter(|s| !s.trim().is_empty()).map(Codigo::desnudo),
        fallecido,
        nacionalidad: pac
            .nacionalidad
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(Codigo::desnudo),
        etnia: codigo_de(pac.pertenencia_etnica.as_deref(), SISTEMA_ETNIA),
        discapacidad: codigo_de(pac.discapacidad.as_deref(), SISTEMA_DISCAPACIDAD),
        identidad_genero: codigo_de(pac.identidad_genero.as_deref(), SISTEMA_IDENTIDAD_GENERO),
    }
}

/// Arma el actor `Profesional` del RDA, compartido por los tres encuentros
/// (directiva 9). `prof_tipo_doc` y `prof_td` (display) se resuelven fuera porque
/// producen faltas propias que el productor recoge.
fn armar_profesional<'a>(
    prof: &'a ProfesionalRda,
    prof_tipo_doc: &'a str,
    prof_td: Option<&'a str>,
) -> Profesional<'a> {
    Profesional {
        tipo_doc: prof_tipo_doc,
        numero_doc: prof.numero_documento.as_deref().unwrap_or(""),
        tipo_doc_display: prof_td.unwrap_or(""),
        nombre: Nombre {
            nombre1: prof.nombre1.as_deref().unwrap_or(""),
            nombre2: prof.nombre2.as_deref(),
            apellido1: prof.apellido1.as_deref().unwrap_or(""),
            apellido2: prof.apellido2.as_deref(),
        },
    }
}

/// [`preparar_consulta`] y [`estado_de_preparacion`] derivan sus resultados sin
/// duplicar la lectura (directiva 9).
///
/// Lee la historia/paciente/profesional/institución (con ámbito y aislamiento),
/// construye el `Insumo`, calcula las secciones vacías avalables y ensambla. El aval
/// del `nilknown` NO es parámetro (F4.5, requisito #1): se LEE de la bitácora
/// `aval_seccion_rda`. Sin filas, la puerta del `nilknown` niega el ensamblado, que
/// es lo correcto. La pantalla de captura escribe esas filas vía
/// [`crate::rda_aval::avalar_secciones`].
async fn preparar_interno(
    pool: &crate::Pool,
    institucion_id: i64,
    historia_id: i64,
    entorno: &str,
) -> Result<Interno, ErrorRda> {
    let ambito = Ambito::institucion(institucion_id);

    // ── 1. Leer todo, OWNED. El `Insumo` presta `&str`, así que estos valores
    // tienen que seguir vivos hasta después de ensamblar.
    let h = crate::historia::obtener(pool, ambito, historia_id)
        .await?
        .ok_or(ErrorRda::HistoriaNoExiste(historia_id))?;
    let pac = crate::paciente::obtener(pool, ambito, h.paciente_id)
        .await?
        .ok_or(ErrorRda::PacienteNoExiste { historia: historia_id, paciente: h.paciente_id })?;
    let prof = obtener_profesional(pool, h.medico_id)
        .await?
        .ok_or(ErrorRda::ProfesionalNoExiste { historia: historia_id, profesional: h.medico_id })?;
    // AISLAMIENTO DEL PROFESIONAL (refuerzo del concilio, 2026-09-03). El paciente
    // y la institución se leen CON el ámbito; el profesional se lee por `medico_id`
    // a secas, así que este productor confiaba en que ese id fuera del ámbito. Hoy
    // lo garantiza `historia::crear` —valida `es_del_ambito(medico_id)` antes de
    // insertar, la más grave de sus tres puertas— y ninguna vía de producción
    // escribe `historia.medico_id` sin pasar por ahí. Pero este es el ÚLTIMO punto
    // antes del envío al visor nacional, y dejar dos de los tres actores acotados y
    // el tercero no es el patrón «se olvidó uno de tres» que otras capas de este
    // repo ya reconocen como grave (directiva 23). Se cierra aquí también: si el
    // vínculo se forjara (un bug futuro en otro módulo), el RDA no saldría con el
    // documento y el nombre de un médico de otra sede. Un superadmin como
    // `medico_id` también cae aquí —`es_del_ambito` lo niega—, que es correcto: el
    // dueño del sistema no es el médico tratante.
    if !crate::usuario::es_del_ambito(pool, ambito, h.medico_id).await? {
        return Err(ErrorRda::ProfesionalDeOtraInstitucion {
            historia: historia_id,
            profesional: h.medico_id,
        });
    }
    let inst = crate::rips::institucion_que_reporta(pool, institucion_id)
        .await?
        .ok_or(ErrorRda::InstitucionNoExiste(institucion_id))?;

    // ── EL AVAL del `nilknown` (F4.5): lo que un clínico confirmó revisado para
    // esta historia, leído OWNED de la bitácora. Cada fila lleva su usuario y su
    // sello, así que el `Aval` que se construye no es una lista de códigos sino el
    // acto atribuido (requisito #1); sin filas, `Aval` queda vacío y la puerta del
    // `nilknown` niega el ensamblado.
    let avaladas = crate::rda_aval::secciones_avaladas(pool, OPERACION_CONSULTA_EXTERNA, historia_id).await?;
    let confirmaciones: Vec<Confirmacion> = avaladas
        .iter()
        .map(|a| Confirmacion { seccion: &a.seccion_codigo, usuario_id: a.usuario_id, sello: &a.sello })
        .collect();
    let aval = Aval::nuevo(&confirmaciones);

    // Resoluciones contra catálogo, cada una con su falta propia si no cierra.
    let mut faltan: Vec<String> = Vec::new();

    let cie10 = crate::clinico::obtener_cie10(pool, &h.diagnostico_principal).await?;
    if cie10.is_none() {
        faltan.push(format!(
            "el diagnóstico principal «{}» no está en el catálogo CIE-10",
            h.diagnostico_principal
        ));
    }

    let cups_display: Option<String> = match h.cod_consulta_cups.as_deref() {
        Some(c) if !c.trim().is_empty() => {
            let d = crate::clinico::obtener_cups(pool, c).await?;
            if d.is_none() {
                faltan.push(format!("el CUPS de la consulta «{c}» no está en el catálogo CUPS"));
            }
            d
        }
        _ => None, // el dominio marcará la falta de serviceType (obligatorio en consulta)
    };

    let muni = match pac.cod_municipio.as_deref() {
        Some(c) if !c.trim().is_empty() => crate::divipola::obtener(pool, c).await?,
        _ => None,
    };

    // Tipo de documento: el `display` sale del catálogo; el dominio NO lo
    // revalida, así que un tipo fuera de catálogo lo emitiría vacío — la falta la
    // pone aquí el productor.
    let pac_td = terminologia::display(SISTEMA_TIPO_DOCUMENTO, &pac.tipo_documento);
    if pac_td.is_none() {
        faltan.push(format!(
            "el tipo de documento del paciente «{}» no está en ColombianPersonIdentifier",
            pac.tipo_documento
        ));
    }
    let prof_tipo_doc = prof.tipo_documento.as_deref().unwrap_or("");
    let prof_td = terminologia::display(SISTEMA_TIPO_DOCUMENTO, prof_tipo_doc);
    if !prof_tipo_doc.is_empty() && prof_td.is_none() {
        faltan.push(format!(
            "el tipo de documento del profesional «{prof_tipo_doc}» no está en ColombianPersonIdentifier"
        ));
    }

    // Sexo → los dos vocabularios (FHIR + ColombianGenderGroup).
    let (genero_fhir, sexo_grupo) = match sexo_rda(&pac.sexo) {
        Some((fhir, code)) => (
            fhir,
            terminologia::display(medico_dominio::ihce::terminologia::SISTEMA_GENERO_GRUPO, code)
                .map(|d| Codigo::nuevo(code, d)),
        ),
        None => {
            faltan.push(format!(
                "el sexo del paciente «{}» no se traduce al RDA (medico captura M/F)",
                pac.sexo
            ));
            ("", None)
        }
    };

    // ── 2. Armar el Insumo, prestando de lo leído.
    let cie10_display = cie10.as_ref().map(|d| d.descripcion.as_str()).unwrap_or("");
    let servicio = match (h.cod_consulta_cups.as_deref(), cups_display.as_deref()) {
        (Some(c), Some(d)) if !c.trim().is_empty() => Some(Servicio { cups: c, display: d }),
        _ => None,
    };

    let insumo = Insumo {
        // Consulta externa: `fallecido = None` —nadie sabe si el paciente murió
        // DESPUÉS— a diferencia de egreso y urgencias (directiva 9: el actor se
        // arma en `armar_paciente`, la diferencia es este parámetro).
        paciente: armar_paciente(&pac, pac_td.unwrap_or(""), genero_fhir, sexo_grupo, &muni, None),
        profesional: armar_profesional(&prof, prof_tipo_doc, prof_td),
        institucion: Institucion {
            codigo_habilitacion: &inst.codigo_habilitacion,
            nit: &inst.nit,
        },
        fecha: &h.fecha,
        modalidad: h.modalidad_grupo_servicio.as_deref(),
        grupo_servicios: h.grupo_servicios.as_deref(),
        // Decisión de Juan (1a): la consulta guarda un INSTANTE, no un intervalo;
        // se declara `start = end = fecha` en vez de inventar una duración que
        // nadie midió. Un periodo de duración cero dice «se atendió en este
        // momento», que es lo único que medico sabe.
        periodo: Some((&h.fecha, &h.fecha)),
        aval,
        entorno: Some(entorno),
        causa_externa: Some(&h.causa_externa),
        diagnostico_ingreso: Some(Diagnostico {
            cie10: &h.diagnostico_principal,
            display: cie10_display,
            tipo: Some(&h.tipo_diagnostico),
        }),
        // Consulta externa: un diagnóstico (el principal); sin egreso, sin
        // hospitalización, sin pagador (el perfil no los exige aquí).
        diagnostico_egreso: None,
        via_ingreso: None,
        destino_egreso: None,
        eps: None,
        // El documento de soporte de la consulta es la propia historia; su
        // identificador natural es su código.
        epicrisis_id: Some(&h.codigo_historia),
        servicio,
    };

    // ── 3. Las secciones vacías avalables (para la pantalla), calculadas con la
    // MISMA lógica que el ensamblado (directiva 9): son las que el encuentro NO
    // puebla con este insumo.
    let avalables: Vec<(String, String)> =
        medico_dominio::ihce::clinico::secciones_avalables(Encuentro::ConsultaExterna, &insumo)
            .into_iter()
            .map(|(codigo, _display, titulo)| (codigo.to_string(), titulo.to_string()))
            .collect();

    // ── 4. Ensamblar. Se llama SIEMPRE, aunque el productor ya tenga faltas de
    // resolución, para recogerlas todas de una vez (directiva 9: una corrección, no
    // cuatro intentos). Se devuelve el intermedio; los dos llamantes públicos
    // derivan de él su resultado.
    let (bundle, faltas_dominio) = match ensamblar(Encuentro::ConsultaExterna, &insumo) {
        Ok(b) => (Some(serde_json::to_value(&b)?), medico_dominio::ihce::Faltas::default()),
        Err(fs) => (None, fs),
    };
    Ok(Interno { bundle, faltan_catalogo: faltan, faltas_dominio, avalables, avaladas })
}

/// Prepara el RDA de una CONSULTA EXTERNA a partir de su `historia`: el Bundle FHIR
/// [`Preparacion::Listo`], o [`Preparacion::Faltan`] con lo que falta capturar.
///
/// `institucion_id` es la sede que emite (de la sesión; el superadmin no emite RDA).
/// `entorno` es el código `EntornoAtencion` de la config del prestador.
pub async fn preparar_consulta(
    pool: &crate::Pool,
    institucion_id: i64,
    historia_id: i64,
    entorno: &str,
) -> Result<Preparacion, ErrorRda> {
    let i = preparar_interno(pool, institucion_id, historia_id, entorno).await?;
    if i.faltan_catalogo.is_empty() && i.faltas_dominio.0.is_empty() {
        // Sin faltas de catálogo NI de dominio → el ensamblado cerró y hay Bundle.
        Ok(Preparacion::Listo(i.bundle.expect("sin faltas, el ensamblado produjo Bundle")))
    } else {
        let mut faltan = i.faltan_catalogo;
        for x in i.faltas_dominio.0 {
            faltan.push(format!("{}.{}: {}", x.recurso, x.campo, x.porque));
        }
        Ok(Preparacion::Faltan(faltan))
    }
}

/// El nombre corto de la operación RDA de consulta externa, con el que la cola
/// (`cola_rda`) persiste la fila y por el que el transporte la vuelve a decodificar.
///
/// **DEBE ser igual a `medico_ihce::OperacionRda::ConsultaExterna.nombre()`.** Se
/// escribe aquí como literal —y no se importa `medico-ihce`— porque `medico-datos`
/// no depende del transporte a propósito (ver la cabecera de `cola_rda`): la cola
/// no sabe hablar HTTP. El amarre entre los dos literales lo fija un caso rojo en
/// `medico-api` (`la_operacion_encolada_la_decodifica_el_transporte`): si divergen,
/// esa prueba se pone roja. Sin ese amarre, una operación mal escrita no fallaría
/// aquí sino en el drenaje, como `Permanente` («operación desconocida»).
pub const OPERACION_CONSULTA_EXTERNA: &str = "consulta-externa";

/// El nombre corto de la operación RDA de HOSPITALIZACIÓN. **Igual a
/// `medico_ihce::OperacionRda::Hospitalizacion.nombre()`** (#603 pieza 2); mismo
/// amarre por caso rojo que [`OPERACION_CONSULTA_EXTERNA`].
pub const OPERACION_HOSPITALIZACION: &str = "hospitalizacion";

/// El nombre corto de la operación RDA de URGENCIAS. **Igual a
/// `medico_ihce::OperacionRda::Urgencias.nombre()`** (#603 pieza 3); mismo amarre
/// por caso rojo que [`OPERACION_CONSULTA_EXTERNA`].
pub const OPERACION_URGENCIAS: &str = "urgencias";

/// Como [`preparar_interno`] pero para una HOSPITALIZACIÓN (#603 pieza 2). Lee la
/// `hospitalizacion` (con ámbito), su paciente, su médico tratante y la institución
/// que reporta, y arma el `Insumo` de un encuentro de EGRESO. Reusa los mismos
/// helpers hoja y los constructores de actores (directiva 9); lo propio de la
/// internación es el diagnóstico de egreso, la vía de ingreso, la condición de
/// destino, el desenlace (`fallecido`) y el periodo ingreso→egreso.
///
/// **Es un MEDIDOR, como el de consulta**: la modalidad de atención la DERIVA el
/// dominio (internación → intramural), así que ya no falta; el tipo de diagnóstico
/// de ingreso/egreso YA se captura (columnas `tipo_diagnostico_ingreso`/`_egreso`,
/// migración 0075) y se pasa aquí. Lo que sigue faltando hasta que exista su captura
/// son los avales del `nilknown` — no se inventan (directiva 20).
async fn preparar_hosp_interno(
    pool: &crate::Pool,
    institucion_id: i64,
    hospitalizacion_id: i64,
    entorno: &str,
) -> Result<Interno, ErrorRda> {
    let ambito = Ambito::institucion(institucion_id);
    let h = crate::hospitalizacion::obtener(pool, ambito, hospitalizacion_id)
        .await?
        .ok_or(ErrorRda::EncuentroNoExiste { tipo: OPERACION_HOSPITALIZACION, id: hospitalizacion_id })?;
    let pac = crate::paciente::obtener(pool, ambito, h.paciente_id)
        .await?
        .ok_or(ErrorRda::PacienteNoExiste { historia: hospitalizacion_id, paciente: h.paciente_id })?;
    let prof = obtener_profesional(pool, h.medico_tratante_id)
        .await?
        .ok_or(ErrorRda::ProfesionalNoExiste { historia: hospitalizacion_id, profesional: h.medico_tratante_id })?;
    // AISLAMIENTO DEL PROFESIONAL, igual que consulta: el tratante tiene que ser
    // del ámbito, o el RDA saldría con el autor de otra sede (directiva 23).
    if !crate::usuario::es_del_ambito(pool, ambito, h.medico_tratante_id).await? {
        return Err(ErrorRda::ProfesionalDeOtraInstitucion {
            historia: hospitalizacion_id,
            profesional: h.medico_tratante_id,
        });
    }
    let inst = crate::rips::institucion_que_reporta(pool, institucion_id)
        .await?
        .ok_or(ErrorRda::InstitucionNoExiste(institucion_id))?;

    // El aval del `nilknown`, llaveado por la HOSPITALIZACIÓN.
    let avaladas = crate::rda_aval::secciones_avaladas(pool, OPERACION_HOSPITALIZACION, hospitalizacion_id).await?;
    let confirmaciones: Vec<Confirmacion> = avaladas
        .iter()
        .map(|a| Confirmacion { seccion: &a.seccion_codigo, usuario_id: a.usuario_id, sello: &a.sello })
        .collect();
    let aval = Aval::nuevo(&confirmaciones);

    let mut faltan: Vec<String> = Vec::new();

    // Diagnósticos de ingreso y egreso (CIE-10), cada uno con su falta si el
    // código no está en catálogo. El `tipo` de cada uno sale de las columnas
    // `tipo_diagnostico_ingreso`/`_egreso` (0075); si no está puesto, Falta (D20).
    let cie_ing = match h.diagnostico_ingreso.as_deref() {
        Some(c) if !c.trim().is_empty() => {
            let d = crate::clinico::obtener_cie10(pool, c).await?;
            if d.is_none() {
                faltan.push(format!("el diagnóstico de ingreso «{c}» no está en el catálogo CIE-10"));
            }
            d
        }
        _ => None,
    };
    let cie_egr = match h.diagnostico_egreso.as_deref() {
        Some(c) if !c.trim().is_empty() => {
            let d = crate::clinico::obtener_cie10(pool, c).await?;
            if d.is_none() {
                faltan.push(format!("el diagnóstico de egreso «{c}» no está en el catálogo CIE-10"));
            }
            d
        }
        _ => None,
    };

    let muni = match pac.cod_municipio.as_deref() {
        Some(c) if !c.trim().is_empty() => crate::divipola::obtener(pool, c).await?,
        _ => None,
    };

    let pac_td = terminologia::display(SISTEMA_TIPO_DOCUMENTO, &pac.tipo_documento);
    if pac_td.is_none() {
        faltan.push(format!(
            "el tipo de documento del paciente «{}» no está en ColombianPersonIdentifier",
            pac.tipo_documento
        ));
    }
    let prof_tipo_doc = prof.tipo_documento.as_deref().unwrap_or("");
    let prof_td = terminologia::display(SISTEMA_TIPO_DOCUMENTO, prof_tipo_doc);
    if !prof_tipo_doc.is_empty() && prof_td.is_none() {
        faltan.push(format!(
            "el tipo de documento del profesional «{prof_tipo_doc}» no está en ColombianPersonIdentifier"
        ));
    }

    let (genero_fhir, sexo_grupo) = match sexo_rda(&pac.sexo) {
        Some((fhir, code)) => (
            fhir,
            terminologia::display(medico_dominio::ihce::terminologia::SISTEMA_GENERO_GRUPO, code)
                .map(|d| Codigo::nuevo(code, d)),
        ),
        None => {
            faltan.push(format!(
                "el sexo del paciente «{}» no se traduce al RDA (medico captura M/F)",
                pac.sexo
            ));
            ("", None)
        }
    };

    // El pagador (EAPB): sale de la entidad del paciente.
    let entidad = match pac.entidad_id {
        Some(id) => crate::administracion::entidad_por_id(pool, id).await?,
        None => None,
    };
    let eps = entidad.as_ref().map(|e| Eps { codigo: e.codigo_eps.as_str(), nombre: e.nombre.as_str() });

    // Los mapeos a value-set FHIR YA existen y los usa el RIPS AH: se REUSAN
    // (directiva 9), no se reescriben. `PROGRAMADA`/`URGENCIAS`/`REMISION` →
    // ViaIngreso; el `estado` → CondicionyDestinoUsuarioEgreso; y el desenlace →
    // `fallecido`, todos por vocabulario y no por memoria.
    let via = medico_dominio::muv::via_ingreso_de_hospitalizacion(Some(h.via_ingreso.as_str()));
    let destino = medico_dominio::muv::condicion_destino_de_hospitalizacion(Some(h.estado.as_str()));
    let fallecido = medico_dominio::ihce::actores::fallecido_de_egreso(&h.estado);

    // El documento se fecha al EGRESO (cuando existe); si el episodio sigue
    // abierto, el dominio lo bloqueará por el periodo, así que la fecha es inocua.
    let fecha_doc = h.fecha_egreso.as_deref().unwrap_or(h.fecha_ingreso.as_str());
    let epicrisis = format!("HOSP-{}", h.id);
    let ing_display = cie_ing.as_ref().map(|d| d.descripcion.as_str()).unwrap_or("");
    let egr_display = cie_egr.as_ref().map(|d| d.descripcion.as_str()).unwrap_or("");

    let insumo = Insumo {
        // Egreso/urgencias declaran el desenlace: `fallecido` sale del estado.
        paciente: armar_paciente(&pac, pac_td.unwrap_or(""), genero_fhir, sexo_grupo, &muni, fallecido),
        profesional: armar_profesional(&prof, prof_tipo_doc, prof_td),
        institucion: Institucion { codigo_habilitacion: &inst.codigo_habilitacion, nit: &inst.nit },
        fecha: fecha_doc,
        // Modalidad: `None` aquí → el dominio la DERIVA (internación → intramural)
        // SOLO si el entorno configurado es `05` Institucional; con un entorno
        // extramural NO deriva y es Falta (no afirmar «Hogar + Intramural»). La cama
        // física fija la intramuralidad dentro de lo institucional (D20/D1).
        // `grupo_servicios` (Composition.event) sigue sin capturarse: el event no es
        // bloqueante hoy y su grupo también se derivaría del tipo — hueco aparte.
        modalidad: None,
        grupo_servicios: None,
        // Periodo del episodio ingreso→egreso. Sin egreso (ACTIVA) no hay `end`,
        // el dominio marca la falta y no se emite el RDA de egreso de quien no ha
        // egresado — coherente con `fallecido = None` de `ACTIVA`.
        periodo: h.fecha_egreso.as_deref().map(|fin| (h.fecha_ingreso.as_str(), fin)),
        aval,
        entorno: Some(entorno),
        // Causa externa (`Encounter.reasonCode`): sale de `causa_motivo`, que es la
        // misma `RIPSCausaExternaVersion2` que el RIPS AH emite como
        // `causaMotivoAtencion` (tabla MUV 21-49). El dominio la exige.
        causa_externa: h.causa_motivo.as_deref().filter(|s| !s.trim().is_empty()),
        diagnostico_ingreso: cie_ing.as_ref().map(|_| Diagnostico {
            cie10: h.diagnostico_ingreso.as_deref().unwrap_or(""),
            display: ing_display,
            // Tipo del diagnóstico (obligatorio en el perfil, `ExtensionDiagnosisType`
            // min=1). Vacío se trata como ausente en el dominio → Falta (D20).
            tipo: h.tipo_diagnostico_ingreso.as_deref(),
        }),
        diagnostico_egreso: cie_egr.as_ref().map(|_| Diagnostico {
            cie10: h.diagnostico_egreso.as_deref().unwrap_or(""),
            display: egr_display,
            tipo: h.tipo_diagnostico_egreso.as_deref(),
        }),
        via_ingreso: via,
        destino_egreso: destino,
        eps,
        epicrisis_id: Some(&epicrisis),
        // `serviceType` (CUPS) es OPCIONAL fuera de consulta externa.
        servicio: None,
    };

    let avalables: Vec<(String, String)> =
        medico_dominio::ihce::clinico::secciones_avalables(Encuentro::Hospitalizacion, &insumo)
            .into_iter()
            .map(|(codigo, _display, titulo)| (codigo.to_string(), titulo.to_string()))
            .collect();

    let (bundle, faltas_dominio) = match ensamblar(Encuentro::Hospitalizacion, &insumo) {
        Ok(b) => (Some(serde_json::to_value(&b)?), medico_dominio::ihce::Faltas::default()),
        Err(fs) => (None, fs),
    };
    Ok(Interno { bundle, faltan_catalogo: faltan, faltas_dominio, avalables, avaladas })
}

/// Prepara el RDA de una HOSPITALIZACIÓN a partir de su id: el Bundle FHIR
/// [`Preparacion::Listo`], o [`Preparacion::Faltan`] con lo que falta capturar.
pub async fn preparar_hospitalizacion(
    pool: &crate::Pool,
    institucion_id: i64,
    hospitalizacion_id: i64,
    entorno: &str,
) -> Result<Preparacion, ErrorRda> {
    let i = preparar_hosp_interno(pool, institucion_id, hospitalizacion_id, entorno).await?;
    if i.faltan_catalogo.is_empty() && i.faltas_dominio.0.is_empty() {
        Ok(Preparacion::Listo(i.bundle.expect("sin faltas, el ensamblado produjo Bundle")))
    } else {
        let mut faltan = i.faltan_catalogo;
        for x in i.faltas_dominio.0 {
            faltan.push(format!("{}.{}: {}", x.recurso, x.campo, x.porque));
        }
        Ok(Preparacion::Faltan(faltan))
    }
}

/// Como [`preparar_interno`] pero para una ATENCIÓN DE URGENCIAS (#603 pieza 3).
/// Lee la `atencion_urgencias` (con ámbito), su paciente y la institución, arma el
/// `Insumo` de un encuentro de urgencias y ensambla. Reusa los helpers y los
/// constructores de actores (directiva 9).
///
/// Dos cosas propias de urgencias, dichas de frente (directiva 20):
/// - **El único clínico que la tabla captura es la ENFERMERA DE TRIAGE**
///   (`enfermera_triage_id`): medico no modela un médico tratante para urgencias
///   (el RIPS AU tampoco lleva profesional). Se usa como autor del RDA porque es
///   quien registró el encuentro; capturar el tratante es trabajo aparte.
/// - **No hay `via_ingreso`** (urgencias no la tiene) ni diagnóstico de EGRESO
///   (solo de ingreso); van `None` y el dominio marcará lo que exija.
///
/// Es MEDIDOR, como los otros dos: la modalidad la DERIVA el dominio (urgencias →
/// intramural) y el tipo de diagnóstico de ingreso ya se captura; sale `Faltan`
/// hasta que existan el aval del `nilknown` y el diagnóstico de egreso (write-only).
async fn preparar_urg_interno(
    pool: &crate::Pool,
    institucion_id: i64,
    urgencias_id: i64,
    entorno: &str,
) -> Result<Interno, ErrorRda> {
    let ambito = Ambito::institucion(institucion_id);
    let u = crate::urgencias::obtener(pool, ambito, urgencias_id)
        .await?
        .ok_or(ErrorRda::EncuentroNoExiste { tipo: OPERACION_URGENCIAS, id: urgencias_id })?;
    let pac = crate::paciente::obtener(pool, ambito, u.paciente_id)
        .await?
        .ok_or(ErrorRda::PacienteNoExiste { historia: urgencias_id, paciente: u.paciente_id })?;
    let prof = obtener_profesional(pool, u.enfermera_triage_id)
        .await?
        .ok_or(ErrorRda::ProfesionalNoExiste { historia: urgencias_id, profesional: u.enfermera_triage_id })?;
    if !crate::usuario::es_del_ambito(pool, ambito, u.enfermera_triage_id).await? {
        return Err(ErrorRda::ProfesionalDeOtraInstitucion {
            historia: urgencias_id,
            profesional: u.enfermera_triage_id,
        });
    }
    let inst = crate::rips::institucion_que_reporta(pool, institucion_id)
        .await?
        .ok_or(ErrorRda::InstitucionNoExiste(institucion_id))?;

    let avaladas = crate::rda_aval::secciones_avaladas(pool, OPERACION_URGENCIAS, urgencias_id).await?;
    let confirmaciones: Vec<Confirmacion> = avaladas
        .iter()
        .map(|a| Confirmacion { seccion: &a.seccion_codigo, usuario_id: a.usuario_id, sello: &a.sello })
        .collect();
    let aval = Aval::nuevo(&confirmaciones);

    let mut faltan: Vec<String> = Vec::new();

    let cie_ing = match u.diagnostico_ingreso.as_deref() {
        Some(c) if !c.trim().is_empty() => {
            let d = crate::clinico::obtener_cie10(pool, c).await?;
            if d.is_none() {
                faltan.push(format!("el diagnóstico de ingreso «{c}» no está en el catálogo CIE-10"));
            }
            d
        }
        _ => None,
    };

    let muni = match pac.cod_municipio.as_deref() {
        Some(c) if !c.trim().is_empty() => crate::divipola::obtener(pool, c).await?,
        _ => None,
    };

    let pac_td = terminologia::display(SISTEMA_TIPO_DOCUMENTO, &pac.tipo_documento);
    if pac_td.is_none() {
        faltan.push(format!(
            "el tipo de documento del paciente «{}» no está en ColombianPersonIdentifier",
            pac.tipo_documento
        ));
    }
    let prof_tipo_doc = prof.tipo_documento.as_deref().unwrap_or("");
    let prof_td = terminologia::display(SISTEMA_TIPO_DOCUMENTO, prof_tipo_doc);
    if !prof_tipo_doc.is_empty() && prof_td.is_none() {
        faltan.push(format!(
            "el tipo de documento del profesional «{prof_tipo_doc}» no está en ColombianPersonIdentifier"
        ));
    }

    let (genero_fhir, sexo_grupo) = match sexo_rda(&pac.sexo) {
        Some((fhir, code)) => (
            fhir,
            terminologia::display(medico_dominio::ihce::terminologia::SISTEMA_GENERO_GRUPO, code)
                .map(|d| Codigo::nuevo(code, d)),
        ),
        None => {
            faltan.push(format!(
                "el sexo del paciente «{}» no se traduce al RDA (medico captura M/F)",
                pac.sexo
            ));
            ("", None)
        }
    };

    let entidad = match pac.entidad_id {
        Some(id) => crate::administracion::entidad_por_id(pool, id).await?,
        None => None,
    };
    let eps = entidad.as_ref().map(|e| Eps { codigo: e.codigo_eps.as_str(), nombre: e.nombre.as_str() });

    // El destino y el desenlace salen de `destino_salida` por vocabulario (reusan
    // `muv`/`actores`, directiva 9). Urgencias NO tiene vía de ingreso.
    let destino = medico_dominio::muv::condicion_destino_de_urgencias(u.destino_salida.as_deref());
    let fallecido = u.destino_salida.as_deref().and_then(medico_dominio::ihce::actores::fallecido_de_urgencias);

    let fecha_doc = u.fecha_egreso.as_deref().unwrap_or(u.fecha_ingreso.as_str());
    let epicrisis = format!("URG-{}", u.id);
    let ing_display = cie_ing.as_ref().map(|d| d.descripcion.as_str()).unwrap_or("");

    let insumo = Insumo {
        paciente: armar_paciente(&pac, pac_td.unwrap_or(""), genero_fhir, sexo_grupo, &muni, fallecido),
        profesional: armar_profesional(&prof, prof_tipo_doc, prof_td),
        institucion: Institucion { codigo_habilitacion: &inst.codigo_habilitacion, nit: &inst.nit },
        fecha: fecha_doc,
        // Modalidad `None` → el dominio la deriva (urgencias → intramural) solo con
        // entorno `05` Institucional; extramural → Falta. El grupo (Composition.event)
        // sigue sin capturarse: hueco aparte, no bloqueante hoy.
        modalidad: None,
        grupo_servicios: None,
        // Periodo ingreso→egreso; sin egreso (aún en el servicio) el dominio lo marca.
        periodo: u.fecha_egreso.as_deref().map(|fin| (u.fecha_ingreso.as_str(), fin)),
        aval,
        entorno: Some(entorno),
        // Causa externa (`reasonCode`): de `causa_motivo` (misma RIPSCausaExternaVersion2).
        causa_externa: u.causa_motivo.as_deref().filter(|s| !s.trim().is_empty()),
        diagnostico_ingreso: cie_ing.as_ref().map(|_| Diagnostico {
            cie10: u.diagnostico_ingreso.as_deref().unwrap_or(""),
            display: ing_display,
            // Tipo del diagnóstico (obligatorio en el perfil AU, min=1). Lo captura el
            // médico al egresar; vacío = ausente → Falta (D20).
            tipo: u.tipo_diagnostico_ingreso.as_deref(),
        }),
        // El diagnóstico de EGRESO de urgencias es write-only: la tabla lo guarda
        // (0042) y `egreso()` lo escribe, pero `Atencion`/`COLS` no lo leen y aquí va
        // `None`. Cerrar ese hueco (leerlo + emitir `DischargeDiagnosis`, min=1 en el
        // perfil AU) es una pieza aparte, no la del tipo de diagnóstico.
        diagnostico_egreso: None,
        // Urgencias no tiene vía de ingreso.
        via_ingreso: None,
        destino_egreso: destino,
        eps,
        epicrisis_id: Some(&epicrisis),
        servicio: None,
    };

    let avalables: Vec<(String, String)> =
        medico_dominio::ihce::clinico::secciones_avalables(Encuentro::Urgencias, &insumo)
            .into_iter()
            .map(|(codigo, _display, titulo)| (codigo.to_string(), titulo.to_string()))
            .collect();

    let (bundle, faltas_dominio) = match ensamblar(Encuentro::Urgencias, &insumo) {
        Ok(b) => (Some(serde_json::to_value(&b)?), medico_dominio::ihce::Faltas::default()),
        Err(fs) => (None, fs),
    };
    Ok(Interno { bundle, faltan_catalogo: faltan, faltas_dominio, avalables, avaladas })
}

/// Prepara el RDA de una ATENCIÓN DE URGENCIAS a partir de su id: el Bundle FHIR
/// [`Preparacion::Listo`], o [`Preparacion::Faltan`] con lo que falta capturar.
pub async fn preparar_urgencias(
    pool: &crate::Pool,
    institucion_id: i64,
    urgencias_id: i64,
    entorno: &str,
) -> Result<Preparacion, ErrorRda> {
    let i = preparar_urg_interno(pool, institucion_id, urgencias_id, entorno).await?;
    if i.faltan_catalogo.is_empty() && i.faltas_dominio.0.is_empty() {
        Ok(Preparacion::Listo(i.bundle.expect("sin faltas, el ensamblado produjo Bundle")))
    } else {
        let mut faltan = i.faltan_catalogo;
        for x in i.faltas_dominio.0 {
            faltan.push(format!("{}.{}: {}", x.recurso, x.campo, x.porque));
        }
        Ok(Preparacion::Faltan(faltan))
    }
}

/// El desenlace de intentar EMITIR el RDA de una consulta: o quedó encolado para el
/// transporte (con el id de su fila en la cola), o faltan datos por capturar.
#[derive(Debug)]
pub enum Emision {
    /// El RDA estaba listo y quedó en la cola de envío (`envio_rda`). El `i64` es el
    /// id de la fila; la tarea de fondo (`correr_cola`) la drena con reintentos.
    Encolado(i64),
    /// Faltan datos: NO se encoló nada. La lista es la misma de
    /// [`Preparacion::Faltan`].
    Faltan(Vec<String>),
}

/// Prepara el RDA de la consulta y, **si está listo, lo ENCOLA** para el transporte
/// IHCE — no lo envía: eso lo hace la tarea de fondo `correr_cola` a partir de la
/// cola, con reintentos y solo cuando el mecanismo está configurado.
///
/// Si faltan datos, **no encola** y devuelve la lista: un RDA a medias que el visor
/// nacional acepta es peor que uno que no salió (directiva 20 — la misma razón por
/// la que [`preparar_consulta`] es un medidor). Encolar solo lo `Listo` es la
/// frontera entre «atestar» y «transmitir datos clínicos al Estado».
///
/// La PUERTA de «¿el transporte está configurado?» NO vive aquí sino en el llamante
/// (la ruta), que es donde vive la config del proceso: encolar un Bundle que nadie
/// va a drenar sería acumular datos clínicos que no llegan a ninguna parte, y la
/// ruta lo evita antes de llamar.
/// Despacha al productor interno del `tipo_encuentro` (directiva 9: los tres
/// encuentros comparten `Interno`, y desde aquí arriba el camino es uno solo). Un
/// tipo desconocido falla ruidosamente (directiva 20).
async fn interno_por_tipo(
    pool: &crate::Pool,
    institucion_id: i64,
    tipo_encuentro: &str,
    encuentro_id: i64,
    entorno: &str,
) -> Result<Interno, ErrorRda> {
    match tipo_encuentro {
        OPERACION_CONSULTA_EXTERNA => preparar_interno(pool, institucion_id, encuentro_id, entorno).await,
        OPERACION_HOSPITALIZACION => preparar_hosp_interno(pool, institucion_id, encuentro_id, entorno).await,
        OPERACION_URGENCIAS => preparar_urg_interno(pool, institucion_id, encuentro_id, entorno).await,
        otro => Err(ErrorRda::TipoDesconocido(otro.to_string())),
    }
}

/// Deriva [`Preparacion`] (Listo/Faltan) de un `Interno`, común a los tres tipos.
fn preparacion_de(i: Interno) -> Result<Preparacion, ErrorRda> {
    if i.faltan_catalogo.is_empty() && i.faltas_dominio.0.is_empty() {
        Ok(Preparacion::Listo(i.bundle.expect("sin faltas, el ensamblado produjo Bundle")))
    } else {
        let mut faltan = i.faltan_catalogo;
        for x in i.faltas_dominio.0 {
            faltan.push(format!("{}.{}: {}", x.recurso, x.campo, x.porque));
        }
        Ok(Preparacion::Faltan(faltan))
    }
}

/// Prepara Y ENCOLA (F4.6) el RDA de CUALQUIER encuentro por `(tipo, id)`. Solo
/// encola lo `Listo`; ante `Faltan` no toca la cola (directiva 20). El nombre de
/// operación con que se persiste es el propio `tipo_encuentro`, que es el mismo que
/// el transporte decodifica.
pub async fn preparar_y_encolar(
    pool: &crate::Pool,
    institucion_id: i64,
    tipo_encuentro: &str,
    encuentro_id: i64,
    entorno: &str,
    ahora: chrono::DateTime<chrono::Utc>,
) -> Result<Emision, ErrorRda> {
    let i = interno_por_tipo(pool, institucion_id, tipo_encuentro, encuentro_id, entorno).await?;
    match preparacion_de(i)? {
        Preparacion::Listo(bundle) => {
            let id = crate::cola_rda::encolar(pool, institucion_id, tipo_encuentro, &bundle, ahora)
                .await
                .map_err(ErrorRda::Bd)?;
            Ok(Emision::Encolado(id))
        }
        Preparacion::Faltan(faltan) => Ok(Emision::Faltan(faltan)),
    }
}

/// El estado de preparación ESTRUCTURADO para la pantalla del `nilknown` (#603
/// F4.5-C): las secciones vacías avalables (con su estado), las otras faltas que
/// bloquean el RDA (informativas), y si ya está listo. Reusa el mismo camino de
/// lectura que [`preparar_consulta`] (directiva 9).
pub async fn estado_de_preparacion(
    pool: &crate::Pool,
    institucion_id: i64,
    tipo_encuentro: &str,
    encuentro_id: i64,
    entorno: &str,
) -> Result<EstadoPreparacion, ErrorRda> {
    let i = interno_por_tipo(pool, institucion_id, tipo_encuentro, encuentro_id, entorno).await?;

    // Cruza avalables × avaladas: marca la casilla de las ya confirmadas.
    let avaladas: std::collections::HashSet<&str> =
        i.avaladas.iter().map(|a| a.seccion_codigo.as_str()).collect();
    let secciones: Vec<SeccionAvalable> = i
        .avalables
        .iter()
        .map(|(codigo, titulo)| SeccionAvalable {
            codigo: codigo.clone(),
            titulo: titulo.clone(),
            avalada: avaladas.contains(codigo.as_str()),
        })
        .collect();

    // Las OTRAS faltas: las de catálogo del productor + las del dominio que NO son de
    // sección (las de `emptyReason` son justo las casillas de arriba, no se repiten).
    let mut otras_faltas = i.faltan_catalogo;
    for x in i.faltas_dominio.0 {
        if x.campo != "emptyReason" {
            otras_faltas.push(format!("{}.{}: {}", x.recurso, x.campo, x.porque));
        }
    }

    let listo = otras_faltas.is_empty() && secciones.iter().all(|s| s.avalada);
    Ok(EstadoPreparacion { secciones, otras_faltas, listo })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Los NUEVE códigos de sección de la consulta externa (7 comunes + órdenes +
    /// soportes), con los que `avalar_consulta` siembra la bitácora del aval en el
    /// camino feliz. Se escriben a mano porque la lista del dominio es `#[cfg(test)]`
    /// interna a `clinico` (no puede ser un asa pública que desactive la puerta del
    /// `nilknown`); si el dominio cambia una, el camino feliz caería en `Faltan` y
    /// esto lo delataría.
    const SECCIONES_CONSULTA: [&str; 9] = [
        "48768-6", "74208-0", "105583-9", "11450-4", "48765-2", "75492-9", "10160-0",
        "61146-1", "55107-7",
    ];

    async fn base() -> crate::Pool {
        static N: AtomicU64 = AtomicU64::new(0);
        let id = N.fetch_add(1, Ordering::Relaxed);
        let url =
            format!("sqlite:file:rda_{}_{}?mode=memory&cache=shared", std::process::id(), id);
        let pool = crate::abrir(&url).await.unwrap();
        // Institución con su código de habilitación REPS y su NIT (el RDA los
        // exige; sin el código, `custodian`/`attester` cuelgan).
        sqlx::query(
            "INSERT INTO institucion (id, nombre, nit, codigo_habilitacion) \
             VALUES (1,'IPS Uno','900123456','0500100001'), (2,'IPS Dos','900999999','0500100002')",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Médico 5: con documento y NOMBRE PARTIDO (camino feliz). Médico 7: sin
        // nombre partido (el que aún no capturó su nombre → falta del profesional).
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo,\
                                  tipo_documento,numero_documento,nombre1,apellido1) \
             VALUES (5,'Carlos Gómez','c@x.co','h','medico',1,1,'CC','111111111','Carlos','Gómez')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo,\
                                  tipo_documento,numero_documento) \
             VALUES (7,'Sin Partir','s@x.co','h','medico',1,1,'CC','222222222')",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Paciente 10 de la institución 1, con municipio y país.
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
                                   fecha_nacimiento,sexo,cod_municipio,cod_pais,zona,institucion_id) \
             VALUES (10,'CC','1234567','Ana','Ruiz','1990-01-01','F','05001','170','01',1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Catálogos: un CIE-10 y un CUPS reales para que sus `display` resuelvan.
        sqlx::query("INSERT INTO cie10 (codigo,descripcion,categoria) VALUES ('J00','Rinofaringitis aguda [resfriado común]','J00')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO cups (codigo,nombre,seccion,ambito_rips,quirurgico) VALUES ('890201','CONSULTA DE PRIMERA VEZ POR MEDICINA GENERAL','SECCION 89','AC',0)")
            .execute(&pool).await.unwrap();
        // Municipio (con su departamento) para el display de la dirección.
        sqlx::query("INSERT INTO departamento (codigo,nombre) VALUES ('05','Antioquia')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO municipio (codigo,nombre,departamento,tipo,en_minsalud) VALUES ('05001','Medellín','05','Municipio',1)")
            .execute(&pool).await.unwrap();
        pool
    }

    /// Siembra una consulta externa del paciente 10 por el médico `medico_id`, con
    /// el diagnóstico `dx`, y devuelve su id.
    async fn sembrar_consulta(pool: &crate::Pool, medico_id: i64, dx: &str) -> i64 {
        sqlx::query(
            "INSERT INTO historia (codigo_historia, paciente_id, medico_id, fecha, \
                 motivo_consulta, enfermedad_actual, diagnostico_principal, plan_tratamiento, \
                 finalidad, causa_externa, tipo_diagnostico, cod_consulta_cups, \
                 modalidad_grupo_servicio, grupo_servicios) \
             VALUES ('HC-1', 10, ?, '2026-08-14T10:00:00-05:00', \
                 'Tos', 'Cuadro gripal', ?, 'Reposo', \
                 '01', '22', '02', '890201', '01', '01')",
        )
        .bind(medico_id)
        .bind(dx)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query_scalar("SELECT id FROM historia WHERE codigo_historia='HC-1'")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    /// Avala las nueve secciones vacías de la consulta externa para `hid`, como
    /// haría la pantalla de captura del clínico (F4.5): el productor lee ESTE aval
    /// de la bitácora, ya no lo recibe por parámetro. El avalador es el médico 5.
    async fn avalar_consulta(pool: &crate::Pool, hid: i64) {
        crate::rda_aval::avalar_secciones(
            pool,
            Ambito::institucion(1),
            5,
            OPERACION_CONSULTA_EXTERNA,
            hid,
            "2026-08-14T10:05:00-05:00",
            &SECCIONES_CONSULTA,
        )
        .await
        .unwrap();
    }

    fn faltan(p: &Preparacion) -> &[String] {
        match p {
            Preparacion::Faltan(v) => v,
            Preparacion::Listo(_) => panic!("se esperaba Faltan, salió Listo"),
        }
    }

    /// **EL CAMINO FELIZ, DE EXTREMO A EXTREMO**: con todo capturado y el aval de
    /// las secciones, el productor lee la BD, resuelve los catálogos y emite un
    /// Bundle FHIR de consulta externa. Es la directiva 28: se cruza la frontera
    /// (BD real sembrada → Bundle), no de estructura a estructura.
    #[tokio::test]
    async fn una_consulta_completa_produce_un_bundle() {
        let pool = base().await;
        let hid = sembrar_consulta(&pool, 5, "J00").await;
        avalar_consulta(&pool, hid).await;
        let p = preparar_consulta(&pool, 1, hid, "05").await.unwrap();
        let bundle = match p {
            Preparacion::Listo(v) => v,
            Preparacion::Faltan(f) => panic!("debería estar listo; faltó: {f:?}"),
        };
        assert_eq!(bundle["resourceType"], "Bundle");
        assert_eq!(bundle["type"], "document");
        // La primera entrada es la Composition, con el perfil de consulta externa.
        let comp = &bundle["entry"][0]["resource"];
        assert_eq!(comp["resourceType"], "Composition");
        assert!(
            comp["meta"]["profile"][0].as_str().unwrap().ends_with("CompositionAmbulatoryRDA"),
            "perfil: {}", comp["meta"]["profile"][0]
        );
        // Y el paciente viaja con su documento y su sexo traducido, señal de que
        // el mapeo de la BD llegó al Bundle.
        let pac = bundle["entry"].as_array().unwrap().iter()
            .map(|e| &e["resource"])
            .find(|r| r["resourceType"] == "Patient")
            .expect("hay Patient");
        assert_eq!(pac["gender"], "female", "el sexo F se tradujo a FHIR");
        assert_eq!(pac["identifier"][0]["value"], "1234567");
    }

    /// **PAREJA 1 — la puerta del `nilknown`**: sin el aval de las secciones vacías,
    /// no se emite. Es lo que hará casi toda consulta real hoy (no hay pantalla de
    /// aval, F4.5). Sin este caso, el camino feliz no probaría que la puerta existe.
    #[tokio::test]
    async fn sin_aval_de_las_secciones_no_se_emite() {
        let pool = base().await;
        let hid = sembrar_consulta(&pool, 5, "J00").await;
        // Sin avalar ninguna sección: la bitácora está vacía → el productor
        // construye un `Aval` vacío y la puerta del `nilknown` niega el ensamblado.
        let p = preparar_consulta(&pool, 1, hid, "05").await.unwrap();
        let f = faltan(&p);
        assert!(
            f.iter().any(|m| m.contains("emptyReason")),
            "debería faltar el aval de las secciones vacías: {f:?}"
        );
    }

    /// **PAREJA 2 — el nombre partido del profesional** (el hallazgo que motivó la
    /// migración 0071). El médico 7 tiene documento pero no nombre partido: el RDA
    /// no puede emitir su `Practitioner` y lo DICE, en vez de inventarlo.
    #[tokio::test]
    async fn el_profesional_sin_nombre_partido_es_una_falta() {
        let pool = base().await;
        let hid = sembrar_consulta(&pool, 7, "J00").await;
        avalar_consulta(&pool, hid).await;
        let p = preparar_consulta(&pool, 1, hid, "05").await.unwrap();
        let f = faltan(&p);
        assert!(
            f.iter().any(|m| m.contains("Practitioner") && m.contains("name")),
            "debería faltar el nombre del profesional: {f:?}"
        );
    }

    /// **PAREJA 3 — un catálogo que no resuelve**: un diagnóstico que no está en la
    /// CIE-10 se reporta como falta de RESOLUCIÓN del productor (que el dominio no
    /// revalida), no se emite con un `display` vacío.
    #[tokio::test]
    async fn un_diagnostico_fuera_de_catalogo_se_reporta() {
        let pool = base().await;
        let hid = sembrar_consulta(&pool, 5, "Z999").await; // no sembrado en cie10
        avalar_consulta(&pool, hid).await;
        let p = preparar_consulta(&pool, 1, hid, "05").await.unwrap();
        let f = faltan(&p);
        assert!(
            f.iter().any(|m| m.contains("CIE-10") && m.contains("Z999")),
            "debería reportar el diagnóstico fuera de catálogo: {f:?}"
        );
    }

    /// Un entorno inválido lo rechaza el ensamblador como falta (el productor lo
    /// pasa tal cual; la validación de config es la primera barrera, ésta la
    /// segunda). Positivo de control de que el `entorno` viaja de verdad al RDA.
    #[tokio::test]
    async fn un_entorno_fuera_de_catalogo_es_falta_del_encuentro() {
        let pool = base().await;
        let hid = sembrar_consulta(&pool, 5, "J00").await;
        avalar_consulta(&pool, hid).await;
        let p = preparar_consulta(&pool, 1, hid, "99").await.unwrap();
        let f = faltan(&p);
        assert!(
            f.iter().any(|m| m.contains("encounterEnvironment")),
            "un entorno fuera de catálogo debería ser falta: {f:?}"
        );
    }

    /// Una historia que no existe es un ERROR, no una falta de captura: son cosas
    /// distintas y quien llama las trata distinto.
    #[tokio::test]
    async fn una_historia_inexistente_es_error() {
        let pool = base().await;
        let e = preparar_consulta(&pool, 1, 9999, "05").await.unwrap_err();
        assert!(matches!(e, ErrorRda::HistoriaNoExiste(9999)), "{e}");
    }

    /// **AISLAMIENTO**: otra institución no ve la historia — el productor lee con
    /// el ámbito, así que pedirla desde la institución 2 es `HistoriaNoExiste`, no
    /// un RDA de un paciente ajeno.
    #[tokio::test]
    async fn otra_institucion_no_ve_la_historia() {
        let pool = base().await;
        let hid = sembrar_consulta(&pool, 5, "J00").await;
        avalar_consulta(&pool, hid).await;
        let e = preparar_consulta(&pool, 2, hid, "05").await.unwrap_err();
        assert!(matches!(e, ErrorRda::HistoriaNoExiste(_)), "{e}");
        // Pareja: desde su propia institución (1) SÍ la lee (sale Listo, con todo
        // sembrado y avalado) — el aislamiento niega la ajena, no la propia.
        let propia = preparar_consulta(&pool, 1, hid, "05").await.unwrap();
        assert!(matches!(propia, Preparacion::Listo(_)), "la propia sí se prepara");
    }

    /// **LA GUARDA PROPIA DEL PRODUCTOR, AISLADA** (refuerzo D33 tras el concilio).
    ///
    /// El tipo de documento del PACIENTE fuera de catálogo es la ÚNICA cosa que el
    /// productor —y solo el productor— defiende: el dominio únicamente exige que no
    /// esté vacío, no que esté en `ColombianPersonIdentifier`, y emitiría
    /// `identifier.type.code` con un código fuera de catálogo y `display:""` sin
    /// protestar. Con TODO lo demás completo y avalado, la única falta es la suya,
    /// así que este caso se pone rojo si se quita su `push` (Mut A del concilio) o
    /// si el gate final ignora la lista `faltan` (Mut B) — las dos mutaciones que
    /// dejaban el banco anterior en verde. Sin este caso, esa guarda no la medía
    /// nadie (el resto de los `Faltan` disparaban por la vía del dominio).
    #[tokio::test]
    async fn el_tipo_de_documento_del_paciente_fuera_de_catalogo_lo_caza_el_productor() {
        let pool = base().await;
        // «ZZ» no está en ColombianPersonIdentifier, pero NO está vacío: el dominio
        // no lo marca. Todo lo demás queda completo y avalado.
        sqlx::query("UPDATE paciente SET tipo_documento='ZZ' WHERE id=10")
            .execute(&pool).await.unwrap();
        let hid = sembrar_consulta(&pool, 5, "J00").await;
        avalar_consulta(&pool, hid).await;
        let p = preparar_consulta(&pool, 1, hid, "05").await.unwrap();
        let f = faltan(&p);
        assert!(
            f.iter().any(|m| m.contains("ColombianPersonIdentifier") && m.contains("paciente")),
            "la guarda del productor debía cazar el tipo de documento fuera de catálogo: {f:?}"
        );
    }

    /// Y el del PROFESIONAL, su gemela: misma guarda propia, otro actor. El dominio
    /// tampoco revalida el tipo de documento del profesional contra el catálogo.
    #[tokio::test]
    async fn el_tipo_de_documento_del_profesional_fuera_de_catalogo_lo_caza_el_productor() {
        let pool = base().await;
        sqlx::query("UPDATE usuario SET tipo_documento='ZZ' WHERE id=5")
            .execute(&pool).await.unwrap();
        let hid = sembrar_consulta(&pool, 5, "J00").await;
        avalar_consulta(&pool, hid).await;
        let p = preparar_consulta(&pool, 1, hid, "05").await.unwrap();
        let f = faltan(&p);
        assert!(
            f.iter().any(|m| m.contains("ColombianPersonIdentifier") && m.contains("profesional")),
            "la guarda del productor debía cazar el tipo de documento del profesional: {f:?}"
        );
    }

    /// **LA GUARDA DE AISLAMIENTO DEL PROFESIONAL** (refuerzo del concilio): una
    /// historia cuyo `medico_id` es un usuario de OTRA institución no emite el RDA,
    /// aunque el paciente sí sea de la institución que pide. Reproduce el ataque que
    /// el concilio encontró (forjar el vínculo por SQL directo, saltándose
    /// `historia::crear`) y comprueba que el productor lo cierra en su propia capa.
    #[tokio::test]
    async fn un_profesional_de_otra_institucion_no_emite() {
        let pool = base().await;
        // Un médico de la institución 2, con nombre partido para que la única
        // barrera sea la de ámbito y no la del nombre.
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo,\
                                  tipo_documento,numero_documento,nombre1,apellido1) \
             VALUES (99,'Ajeno','a@x.co','h','medico',2,1,'CC','333333333','Ajeno','Foraneo')",
        )
        .execute(&pool).await.unwrap();
        // Historia del paciente 10 (institución 1) pero con el médico de la 2:
        // sembrar_consulta inserta por SQL directo, saltándose la puerta de
        // `historia::crear` (que exige es_del_ambito del medico_id).
        let hid = sembrar_consulta(&pool, 99, "J00").await;
        let e = preparar_consulta(&pool, 1, hid, "05").await.unwrap_err();
        assert!(
            matches!(e, ErrorRda::ProfesionalDeOtraInstitucion { profesional: 99, .. }),
            "el profesional de otra institución debía cerrar el RDA: {e}"
        );
    }

    /// **EL ESTADO ESTRUCTURADO refleja el aval** (F4.5-C, D28 a nivel datos): antes
    /// de avalar hay secciones por confirmar, ninguna marcada y NO está listo; tras
    /// avalar TODAS las avalables (como hará la pantalla), todas quedan marcadas y,
    /// sin otras faltas (medico 5 tiene nombre partido y los catálogos resuelven), el
    /// RDA queda listo. Es la pareja del camino de la pantalla, medida contra la BD.
    #[tokio::test]
    async fn el_estado_de_preparacion_refleja_lo_avalado() {
        let pool = base().await;
        let hid = sembrar_consulta(&pool, 5, "J00").await;

        let e0 = estado_de_preparacion(&pool, 1, OPERACION_CONSULTA_EXTERNA, hid, "05").await.unwrap();
        assert!(!e0.secciones.is_empty(), "una consulta deja secciones vacías por avalar");
        assert!(e0.secciones.iter().all(|s| !s.avalada), "nada avalado aún");
        assert!(!e0.listo, "sin aval no está listo");
        assert!(
            e0.otras_faltas.is_empty(),
            "con medico 5 (nombre partido) y catálogos sembrados no debe haber otras faltas: {:?}",
            e0.otras_faltas
        );

        // Avalar TODAS las avalables, como haría la pantalla con el usuario de sesión.
        let codigos: Vec<String> = e0.secciones.iter().map(|s| s.codigo.clone()).collect();
        let refs: Vec<&str> = codigos.iter().map(String::as_str).collect();
        crate::rda_aval::avalar_secciones(
            &pool, Ambito::institucion(1), 5, OPERACION_CONSULTA_EXTERNA, hid, "2026-09-03T10:00:00-05:00", &refs,
        )
        .await
        .unwrap();

        let e1 = estado_de_preparacion(&pool, 1, OPERACION_CONSULTA_EXTERNA, hid, "05").await.unwrap();
        assert!(e1.secciones.iter().all(|s| s.avalada), "todas avaladas tras el aval");
        assert!(e1.listo, "con todo avalado y sin otras faltas, está listo");
        // Y el productor coincide: ahora emite el Bundle.
        assert!(
            matches!(preparar_consulta(&pool, 1, hid, "05").await.unwrap(), Preparacion::Listo(_)),
            "listo en el estado ⇒ Listo en el productor"
        );
    }

    /// **El caso PARCIAL, la pareja de los dos extremos** (F4.5-C): avalar SOLO
    /// algunas secciones marca `avalada` en EXACTAMENTE esas —ni de más ni de
    /// menos— y, con una sección todavía sin avalar, el RDA NO está listo. Sin
    /// esto, los dos tests de extremos (nada avalado / todo avalado) pasarían
    /// igual con un cruce por-sección roto que marcara todo-o-nada, o con un
    /// `listo` que ignorara las secciones sin confirmar.
    #[tokio::test]
    async fn el_estado_parcial_marca_solo_lo_avalado_y_no_esta_listo() {
        let pool = base().await;
        let hid = sembrar_consulta(&pool, 5, "J00").await;

        let e0 = estado_de_preparacion(&pool, 1, OPERACION_CONSULTA_EXTERNA, hid, "05").await.unwrap();
        assert!(
            e0.secciones.len() >= 2,
            "un caso parcial necesita ≥2 secciones avalables; hay {}",
            e0.secciones.len()
        );

        // Avalar SOLO la primera sección, con el usuario de sesión (como la pantalla).
        let primera = e0.secciones[0].codigo.clone();
        crate::rda_aval::avalar_secciones(
            &pool,
            Ambito::institucion(1),
            5,
            OPERACION_CONSULTA_EXTERNA,
            hid,
            "2026-09-03T10:00:00-05:00",
            &[primera.as_str()],
        )
        .await
        .unwrap();

        let e1 = estado_de_preparacion(&pool, 1, OPERACION_CONSULTA_EXTERNA, hid, "05").await.unwrap();
        // EXACTAMENTE la avalada queda marcada; ninguna otra (cruce por-sección).
        for s in &e1.secciones {
            assert_eq!(
                s.avalada,
                s.codigo == primera,
                "la sección {} quedó marcada={} y no era lo esperado",
                s.codigo,
                s.avalada
            );
        }
        assert_eq!(
            e1.secciones.iter().filter(|s| s.avalada).count(),
            1,
            "solo una sección debía quedar avalada"
        );
        // La PAREJA ROJA del `listo = todas avaladas`: con una sección sin avalar,
        // no puede estar listo.
        assert!(!e1.listo, "con una sección aún sin avalar, el RDA no puede estar listo");
    }

    /// **EMITIR UN RDA LISTO LO ENCOLA** (#603 F4.6): con todo capturado y las
    /// secciones avaladas, `preparar_y_encolar` deja exactamente una fila
    /// `pendiente` en `envio_rda`, con la operación de consulta externa y el Bundle
    /// entero guardado. Cruza la frontera (BD sembrada → fila en la cola), no de
    /// estructura a estructura (directiva 28).
    #[tokio::test]
    async fn emitir_una_consulta_lista_la_encola() {
        let pool = base().await;
        let hid = sembrar_consulta(&pool, 5, "J00").await;
        avalar_consulta(&pool, hid).await;

        // Control: la cola está vacía antes.
        let antes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM envio_rda")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(antes, 0, "la cola de envío no estaba vacía antes de emitir");

        let em = preparar_y_encolar(&pool, 1, OPERACION_CONSULTA_EXTERNA, hid, "05", chrono::Utc::now()).await.unwrap();
        let id = match em {
            Emision::Encolado(id) => id,
            Emision::Faltan(f) => panic!("debería haber encolado; faltó: {f:?}"),
        };
        assert!(id > 0, "el id de la fila encolada no es válido");

        // El DESENLACE, en la base: una fila pendiente, con la operación y el Bundle.
        let (n, operacion, estado, bundle): (i64, String, String, String) = {
            let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM envio_rda")
                .fetch_one(&pool)
                .await
                .unwrap();
            let fila: (String, String, String) = sqlx::query_as(
                "SELECT operacion, estado, bundle FROM envio_rda WHERE id = ?",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
            (n, fila.0, fila.1, fila.2)
        };
        assert_eq!(n, 1, "emitir dejó un número de filas distinto de una");
        assert_eq!(operacion, OPERACION_CONSULTA_EXTERNA, "la operación encolada no es la de consulta externa");
        assert_eq!(estado, "pendiente", "la fila no nació pendiente de envío");
        // El Bundle se guardó entero (no un vacío ni un placeholder): es lo que se
        // enviará tal cual (por eso la cola lo persiste completo).
        let v: serde_json::Value = serde_json::from_str(&bundle).expect("el bundle guardado no es JSON");
        assert_eq!(v["resourceType"], "Bundle", "no se guardó un Bundle FHIR en la cola");
    }

    /// **LA PAREJA ROJA: un RDA INCOMPLETO no se encola.** El médico 7 no tiene el
    /// nombre partido → `preparar_consulta` devuelve `Faltan`, así que
    /// `preparar_y_encolar` NO debe dejar nada en la cola. Sin esta mitad, un
    /// `preparar_y_encolar` que encolara SIEMPRE (ignorando `Faltan`) pasaría la
    /// prueba de arriba igual (directiva 33): aquí el número tiene que quedar en
    /// cero, y mutar el brazo `Faltan` para que encole lo pone rojo.
    #[tokio::test]
    async fn emitir_una_consulta_incompleta_no_encola() {
        let pool = base().await;
        // Médico 7 (sin nombre partido) y sin avalar: faltan datos.
        let hid = sembrar_consulta(&pool, 7, "J00").await;

        let em = preparar_y_encolar(&pool, 1, OPERACION_CONSULTA_EXTERNA, hid, "05", chrono::Utc::now()).await.unwrap();
        let faltan = match em {
            Emision::Faltan(f) => f,
            Emision::Encolado(id) => panic!("no debía encolar un RDA incompleto (fila {id})"),
        };
        assert!(!faltan.is_empty(), "un RDA incompleto debe reportar qué falta");

        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM envio_rda")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(n, 0, "un RDA incompleto dejó una fila en la cola: se emitió a medias");
    }

    // ── #603 pieza 2: RDA de HOSPITALIZACIÓN ─────────────────────────────────

    /// Siembra una cama de la institución 1 y una hospitalización del paciente 10
    /// con el médico `medico_id` y el `estado` dado (ACTIVA = sin egreso; un egreso
    /// como ALTA_MEDICA/DEFUNCION trae `fecha_egreso` y diagnóstico de egreso).
    async fn sembrar_hospitalizacion(pool: &crate::Pool, medico_id: i64, estado: &str) -> i64 {
        static C: AtomicU64 = AtomicU64::new(0);
        let n = C.fetch_add(1, Ordering::Relaxed);
        let cama: i64 = sqlx::query_scalar(
            "INSERT INTO cama (institucion_id, numero, piso, servicio, estado) \
             VALUES (1, ?, '1', 'GENERAL', 'OCUPADA') RETURNING id",
        )
        .bind(format!("H{n}"))
        .fetch_one(pool)
        .await
        .unwrap();
        let con_egreso = estado != "ACTIVA";
        let fecha_egreso: Option<&str> = con_egreso.then_some("2026-09-05T15:00:00-05:00");
        let dx_egreso: Option<&str> = con_egreso.then_some("J00");
        sqlx::query_scalar(
            "INSERT INTO hospitalizacion \
             (paciente_id, cama_id, medico_tratante_id, fecha_ingreso, fecha_egreso, \
              via_ingreso, estado, diagnostico_ingreso, diagnostico_egreso, causa_motivo) \
             VALUES (10, ?, ?, '2026-09-03T08:00:00-05:00', ?, 'PROGRAMADA', ?, 'J00', ?, '38') RETURNING id",
        )
        .bind(cama)
        .bind(medico_id)
        .bind(fecha_egreso)
        .bind(estado)
        .bind(dx_egreso)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    /// El productor de hospitalización RESUELVE lo que puede (paciente, profesional,
    /// diagnósticos CIE-10, vía/destino por `muv`, pagador) y la MODALIDAD ya no es
    /// hueco: el dominio la DERIVA (internación → intramural). Medidor honesto (D20):
    /// lo que queda es el tipo de diagnóstico (no sembrado aquí) y los avales del
    /// `nilknown`. Pareja D33: el control positivo es que NO aparece ninguna falta
    /// «no está en el catálogo» —si la vía, el destino o el CIE no se hubieran
    /// resuelto, la lista lo diría— NI la de modalidad, que ahora se deriva.
    #[tokio::test]
    async fn la_hospitalizacion_resuelve_lo_que_puede_y_deriva_la_modalidad() {
        let pool = base().await;
        // Pagador: una EAPB y el paciente afiliado a ella (ejercita el `eps`).
        sqlx::query("INSERT INTO entidad (id, nit, nombre, codigo_eps) VALUES (1,'8001','SALUD TOTAL EPS S.A.','EPS016')")
            .execute(&pool).await.unwrap();
        sqlx::query("UPDATE paciente SET entidad_id = 1 WHERE id = 10").execute(&pool).await.unwrap();

        let hid = sembrar_hospitalizacion(&pool, 5, "ALTA_MEDICA").await;
        let p = preparar_hospitalizacion(&pool, 1, hid, "05").await.unwrap();
        let fs = faltan(&p);

        // La modalidad se deriva: ya NO falta (antes de derivarla, sí faltaba).
        assert!(
            !fs.iter().any(|f| f.to_lowercase().contains("modal")),
            "la modalidad debe derivarse (intramural), no faltar: {fs:?}"
        );
        // Control positivo (D33): nada de lo que el productor SÍ resuelve quedó sin
        // resolver. Una vía/destino/CIE/documento sin resolver saldría como «no está
        // en el catálogo» o «no se traduce»; que no aparezcan prueba que se resolvieron.
        assert!(
            !fs.iter().any(|f| f.contains("no está en el catálogo") || f.contains("no se traduce")),
            "algo que el productor debía resolver quedó sin resolver: {fs:?}"
        );
        // Y control de que el pipeline SÍ corrió (no salió vacío por otra causa): el
        // tipo de diagnóstico no sembrado y los avales del nilknown siguen faltando.
        assert!(
            fs.iter().any(|f| f.contains("ExtensionDiagnosisType")) && fs.iter().any(|f| f.contains("Nil Known")),
            "deben quedar el tipo-dx (sin sembrar) y los avales del nilknown: {fs:?}"
        );
    }

    /// AISLAMIENTO del encuentro (D33): la institución 2 NO prepara la
    /// hospitalización de la 1 (su cama es de la 1), y la 1 SÍ.
    #[tokio::test]
    async fn otra_institucion_no_prepara_la_hospitalizacion() {
        let pool = base().await;
        let hid = sembrar_hospitalizacion(&pool, 5, "ALTA_MEDICA").await;
        // Rojo, por la vía real: la 2 no ve el encuentro de la 1.
        let e = preparar_hospitalizacion(&pool, 2, hid, "05").await.unwrap_err();
        assert!(
            matches!(e, ErrorRda::EncuentroNoExiste { tipo: OPERACION_HOSPITALIZACION, id } if id == hid),
            "aislamiento del encuentro: {e}"
        );
        // Verde: la institución dueña sí lo prepara (aunque salga Faltan por el tipo/avales).
        preparar_hospitalizacion(&pool, 1, hid, "05").await.unwrap();
    }

    /// AISLAMIENTO del profesional (refuerzo del concilio, D23): un tratante de otra
    /// institución NO emite, aunque la cama sea de la propia. Cierra el flanco «dos
    /// de tres» que el productor de consulta ya cerró.
    #[tokio::test]
    async fn un_tratante_de_otra_institucion_no_emite_la_hospitalizacion() {
        let pool = base().await;
        // Médico 9, de la institución 2, como tratante de una cama de la 1.
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo,\
                                  tipo_documento,numero_documento,nombre1,apellido1) \
             VALUES (9,'Ajeno','a@x.co','h','medico',2,1,'CC','999','Ajeno','Ajeno')",
        )
        .execute(&pool).await.unwrap();
        let hid = sembrar_hospitalizacion(&pool, 9, "ALTA_MEDICA").await;
        let e = preparar_hospitalizacion(&pool, 1, hid, "05").await.unwrap_err();
        assert!(
            matches!(e, ErrorRda::ProfesionalDeOtraInstitucion { profesional: 9, .. }),
            "el tratante de otra sede no debe emitir: {e}"
        );
    }

    /// #603 (tipo de diagnóstico): el tipo capturado LLEGA al RDA y BORRA la Falta
    /// `ExtensionDiagnosisType`, que el perfil AH exige (min=1) para el diagnóstico de
    /// ingreso y de egreso. Pareja D33 dentro del mismo test: una hospitalización SIN
    /// tipo deja 2 Faltas de ese campo (ingreso + egreso), y otra idéntica CON los dos
    /// tipos deja 0. Mutar el productor a `tipo: None` iguala los dos conteos → rojo.
    /// Y el control positivo (D30): las Faltas del `nilknown` (emptyReason) siguen en
    /// AMBAS, prueba de que el `Encounter`/las secciones se construyeron y solo cambió
    /// lo tocado. (La modalidad ya NO sirve de control: se deriva y no falta.)
    #[tokio::test]
    async fn el_tipo_de_diagnostico_capturado_llega_al_rda_de_hospitalizacion() {
        let pool = base().await;
        let cuenta_ext = |fs: &[String]| fs.iter().filter(|f| f.contains("ExtensionDiagnosisType")).count();

        // Sin tipo: dos Faltas del campo (ingreso + egreso).
        let sin = sembrar_hospitalizacion(&pool, 5, "ALTA_MEDICA").await;
        let p_sin = preparar_hospitalizacion(&pool, 1, sin, "05").await.unwrap();
        let fs_sin = faltan(&p_sin);
        assert_eq!(cuenta_ext(fs_sin), 2, "sin tipo deben faltar los dos ExtensionDiagnosisType: {fs_sin:?}");
        assert!(fs_sin.iter().any(|f| f.contains("Nil Known")), "control: el nilknown debe faltar (pipeline corrió)");

        // Con los dos tipos capturados: cero Faltas del campo.
        let con = sembrar_hospitalizacion(&pool, 5, "ALTA_MEDICA").await;
        sqlx::query("UPDATE hospitalizacion SET tipo_diagnostico_ingreso='02', tipo_diagnostico_egreso='03' WHERE id=?")
            .bind(con).execute(&pool).await.unwrap();
        let p_con = preparar_hospitalizacion(&pool, 1, con, "05").await.unwrap();
        let fs_con = faltan(&p_con);
        assert_eq!(cuenta_ext(fs_con), 0, "con los dos tipos no debe faltar ningún ExtensionDiagnosisType: {fs_con:?}");
        assert!(fs_con.iter().any(|f| f.contains("Nil Known")), "control: el nilknown sigue faltando (pipeline corrió)");
    }

    // ── #603 pieza 3: RDA de URGENCIAS ───────────────────────────────────────

    /// Siembra una atención de urgencias del paciente 10 con la clínica
    /// `clinico_id` como enfermera de triage. `con_egreso` fija `destino_salida` +
    /// `fecha_egreso`. Devuelve el id de la atención.
    async fn sembrar_urgencias(pool: &crate::Pool, clinico_id: i64, con_egreso: bool) -> i64 {
        let destino: Option<&str> = con_egreso.then_some("DOMICILIO");
        let fecha_egreso: Option<&str> = con_egreso.then_some("2026-09-03T12:00:00-05:00");
        sqlx::query_scalar(
            "INSERT INTO atencion_urgencias \
             (paciente_id, triage, motivo_consulta, enfermera_triage_id, fecha_ingreso, \
              diagnostico_ingreso, destino_salida, fecha_egreso, causa_motivo) \
             VALUES (10, 'TRIAGE_2', 'dolor torácico', ?, '2026-09-03T09:00:00-05:00', 'J00', ?, ?, '38') RETURNING id",
        )
        .bind(clinico_id)
        .bind(destino)
        .bind(fecha_egreso)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    /// El productor de urgencias RESUELVE lo que puede (paciente, enfermera de
    /// triage como autor, CIE de ingreso, destino por `muv`, pagador, causa) y la
    /// MODALIDAD ya se DERIVA (urgencias → intramural). Quedan como gaps el tipo-dx
    /// (ingreso, no sembrado; y egreso write-only) y los avales. Pareja D33: el
    /// control positivo es que NO hay faltas «no está en el catálogo» NI de modalidad.
    #[tokio::test]
    async fn la_urgencia_resuelve_lo_que_puede_y_deriva_la_modalidad() {
        let pool = base().await;
        sqlx::query("INSERT INTO entidad (id, nit, nombre, codigo_eps) VALUES (1,'8001','SALUD TOTAL EPS S.A.','EPS016')")
            .execute(&pool).await.unwrap();
        sqlx::query("UPDATE paciente SET entidad_id = 1 WHERE id = 10").execute(&pool).await.unwrap();

        let uid = sembrar_urgencias(&pool, 5, true).await;
        let p = preparar_urgencias(&pool, 1, uid, "05").await.unwrap();
        let fs = faltan(&p);

        assert!(
            !fs.iter().any(|f| f.to_lowercase().contains("modal")),
            "la modalidad debe derivarse (intramural), no faltar: {fs:?}"
        );
        assert!(
            !fs.iter().any(|f| f.contains("no está en el catálogo") || f.contains("no se traduce")),
            "algo que el productor debía resolver quedó sin resolver: {fs:?}"
        );
    }

    /// AISLAMIENTO del encuentro (D33): la institución 2 NO prepara la urgencia de
    /// la 1, y la 1 SÍ.
    #[tokio::test]
    async fn otra_institucion_no_prepara_la_urgencia() {
        let pool = base().await;
        let uid = sembrar_urgencias(&pool, 5, true).await;
        let e = preparar_urgencias(&pool, 2, uid, "05").await.unwrap_err();
        assert!(
            matches!(e, ErrorRda::EncuentroNoExiste { tipo: OPERACION_URGENCIAS, id } if id == uid),
            "aislamiento del encuentro: {e}"
        );
        preparar_urgencias(&pool, 1, uid, "05").await.unwrap();
    }

    /// AISLAMIENTO del profesional (D23): una enfermera de otra institución NO
    /// emite, aunque el paciente sea de la propia.
    #[tokio::test]
    async fn una_enfermera_de_otra_institucion_no_emite_la_urgencia() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo,\
                                  tipo_documento,numero_documento,nombre1,apellido1) \
             VALUES (9,'Ajena','a@x.co','h','enfermera',2,1,'CC','999','Ajena','Ajena')",
        )
        .execute(&pool).await.unwrap();
        let uid = sembrar_urgencias(&pool, 9, true).await;
        let e = preparar_urgencias(&pool, 1, uid, "05").await.unwrap_err();
        assert!(
            matches!(e, ErrorRda::ProfesionalDeOtraInstitucion { profesional: 9, .. }),
            "la enfermera de otra sede no debe emitir: {e}"
        );
    }

    /// #603 (tipo de diagnóstico): el tipo del diagnóstico de INGRESO de urgencias
    /// llega al RDA y borra SU Falta `ExtensionDiagnosisType`. Pareja D33: sin tipo
    /// hay 2 Faltas de ese campo (ingreso + egreso), con el tipo de ingreso queda 1
    /// —la del EGRESO, que urgencias no cablea aún (hueco aparte)—; el conteo baja de
    /// 2 a 1. Mutar el productor a `tipo: None` lo devuelve a 2 → rojo. No se afirma
    /// «0» a propósito: sería fingir que el egreso ya está resuelto (D20).
    #[tokio::test]
    async fn el_tipo_de_diagnostico_capturado_llega_al_rda_de_urgencias() {
        let pool = base().await;
        let cuenta_ext = |fs: &[String]| fs.iter().filter(|f| f.contains("ExtensionDiagnosisType")).count();

        let sin = sembrar_urgencias(&pool, 5, true).await;
        let p_sin = preparar_urgencias(&pool, 1, sin, "05").await.unwrap();
        assert_eq!(cuenta_ext(faltan(&p_sin)), 2, "sin tipo: ingreso + egreso sin ExtensionDiagnosisType");

        let con = sembrar_urgencias(&pool, 5, true).await;
        sqlx::query("UPDATE atencion_urgencias SET tipo_diagnostico_ingreso='02' WHERE id=?")
            .bind(con).execute(&pool).await.unwrap();
        let p_con = preparar_urgencias(&pool, 1, con, "05").await.unwrap();
        let fs_con = faltan(&p_con);
        assert_eq!(cuenta_ext(fs_con), 1, "con el tipo de ingreso solo debe quedar la del egreso (write-only): {fs_con:?}");
        assert!(fs_con.iter().any(|f| f.contains("Nil Known")), "control: el nilknown sigue faltando (pipeline corrió)");
    }
}
