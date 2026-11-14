// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! ¿La fila que enlaza el cliente es de ESE paciente?
//!
//! # Por qué existe, y por qué en un solo sitio
//!
//! Un módulo clínico comprueba su `paciente_id` contra la institución —eso lo
//! hacen los once del bloque #249— y acto seguido ata en crudo la clave foránea
//! que le mandaron: la hospitalización, la atención de urgencias, la inscripción
//! del programa. Esa segunda mitad faltaba en **cinco** sitios, y la revisión de
//! seguridad de la rama `registro-nacimiento` los encontró en bloque.
//!
//! En cuatro de los cinco el daño era una fila de la institución A apuntando a
//! una de la B: integridad referencial cruzada, sin fuga, porque ninguna consulta
//! de lectura une esas tablas hoy. En el quinto —`defuncion::certificar`— el
//! valor además se LEÍA, y la diferencia entre `MurioAntesDeIngresar` y el resto
//! convertía la omisión en un oráculo con el que se reconstruía por bisección la
//! hora de ingreso de un paciente de otro prestador.
//!
//! O sea que era **un defecto mirado por dos lados**, y el lado callado es el
//! que nunca se queja. Por eso la comprobación no se copia cinco veces: la
//! sexta es la que va a salir mal (directiva 23 — el arreglo va en la regla, no
//! en el caso).
//!
//! # No hace falta pasarle el ámbito, y conviene entender por qué
//!
//! Los cinco llamantes comprueban antes su paciente contra la institución. Con
//! eso, «esta fila es de este paciente» implica «es de esta institución»: una
//! hospitalización de otro prestador no puede ser de un paciente de éste. Pedir
//! el ámbito además sería el segundo mecanismo que sostiene lo que ya sostiene
//! el primero (directiva 1).
//!
//! La contrapartida honesta: quien use esto **sin** haber acotado el paciente no
//! está acotando el ámbito. Por eso el parámetro se llama `paciente_id` y no
//! `id`, y por eso está escrito aquí.

/// Las tablas a las que un registro clínico puede colgarse por un id que llega
/// del cliente.
///
/// Es un enum y no un `&str` con el nombre de la tabla **a propósito**: el
/// nombre de una tabla no puede ir por `bind`, así que la alternativa era
/// interpolarlo. Aquí se elige la consulta ENTERA con un `match` y no queda
/// ninguna cadena que armar — no hay nada que inyectar, en vez de haber algo
/// que recordar validar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enlace {
    Hospitalizacion,
    Urgencias,
    InscripcionPyp,
}

impl Enlace {
    const fn consulta(self) -> &'static str {
        match self {
            Enlace::Hospitalizacion => {
                "SELECT 1 FROM hospitalizacion WHERE id = ? AND paciente_id = ?"
            }
            Enlace::Urgencias => {
                "SELECT 1 FROM atencion_urgencias WHERE id = ? AND paciente_id = ?"
            }
            Enlace::InscripcionPyp => {
                "SELECT 1 FROM inscripcion_pyp WHERE id = ? AND paciente_id = ?"
            }
        }
    }
}

/// ¿Esa fila es de ese paciente?
///
/// **Responde `false` igual para «no es suya», «no existe» y «es de otra
/// institución».** Distinguirlas sería el oráculo: quien no puede ver una fila
/// tampoco puede saber cuál de las tres razones es (`aislamiento.rs`).
pub async fn es_del_paciente(
    pool: &crate::Pool,
    enlace: Enlace,
    id: i64,
    paciente_id: i64,
) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query_scalar::<_, i64>(enlace.consulta())
        .bind(id)
        .bind(paciente_id)
        .fetch_optional(pool)
        .await?
        .is_some())
}

/// Lo mismo, pero para el enlace OPCIONAL, que es la forma en que llegan los
/// cinco: `None` no es un enlace ajeno, es la ausencia de enlace, y ausente es
/// legítimo en los cinco sitios.
///
/// Existe para que el llamante no tenga que escribir el `if let Some(..) && !..`
/// cada vez — que es exactamente donde se olvidó cinco veces.
pub async fn opcional_es_del_paciente(
    pool: &crate::Pool,
    enlace: Enlace,
    id: Option<i64>,
    paciente_id: i64,
) -> Result<bool, sqlx::Error> {
    match id {
        None => Ok(true),
        Some(id) => es_del_paciente(pool, enlace, id, paciente_id).await,
    }
}
