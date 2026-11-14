// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! El catálogo de módulos del producto, y en qué estado está cada uno.
//!
//! Permite vender el MISMO binario a un consultorio, una IPS, una clínica y un
//! hospital: lo que cambia no es el código, es la licencia.
//!
//! # Tres estados, porque antes eran indistinguibles
//!
//! Todo lo no implementado respondía `200 []`, así que el frontend no podía
//! decir si un módulo estaba vacío, no comprado o sin construir. Son tres cosas
//! distintas y el cliente las vive distinto:
//!
//! | Estado | Qué significa | Qué hace el cliente |
//! |---|---|---|
//! | `Habilitado` | licenciado y funcionando | usarlo |
//! | `Pendiente` | licenciado pero aún sin construir | esperarlo, y saberlo |
//! | `NoIncluido` | existe en el producto, no en su plan | comprarlo si lo quiere |
//!
//! # Lo que este archivo NO decide
//!
//! Si un módulo está construido **no se escribe aquí a mano**. Se deriva de si
//! el binario sirve su ruta, y lo comprueba
//! `el_catalogo_no_miente_sobre_lo_que_el_binario_sirve` en la capa de API: una
//! lista mantenida a mano de «lo que ya está hecho» se desincroniza del router
//! al primer módulo nuevo, y entonces el producto promete algo que no existe.

use serde::{Deserialize, Serialize};

/// En qué estado está un módulo PARA UNA INSTITUCIÓN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoModulo {
    Habilitado,
    Pendiente,
    NoIncluido,
}

/// Un módulo del catálogo: su nombre, su grupo, y **la ruta por la que se
/// sabría si existe**.
///
/// `prefijo` es el dato que hace comprobable el estado: `Some("/api/nutricion")`
/// significa «este módulo está construido si y solo si el router sirve eso», y
/// `None`, «todavía no tiene ruta que mirar» — o sea, pendiente por definición.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modulo {
    pub nombre: &'static str,
    pub grupo: &'static str,
    pub prefijo: Option<&'static str>,
}

/// El catálogo, agrupado por área asistencial. El menú se construye con esto.
///
/// Los nombres son los mismos que usaba el backend Python (`ModuloSistema`),
/// para que una institución migrada conserve su licencia sin traducciones.
pub const CATALOGO: &[Modulo] = &[
    // --- Core ---------------------------------------------------------------
    m("CITAS", "Core", Some("/api/citas")),
    m("PACIENTES", "Core", Some("/api/pacientes")),
    m("HISTORIA", "Core", Some("/api/historias")),
    m("URGENCIAS", "Core", Some("/api/urgencias")),
    m("PORTAL_PACIENTE", "Core", None),
    // --- Hospitalario -------------------------------------------------------
    m("HOSPITALIZACION", "Hospitalario", Some("/api/hospitalizacion")),
    m("EPICRISIS", "Hospitalario", Some("/api/documentos")),
    m("INTERCONSULTAS", "Hospitalario", Some("/api/documentos")),
    m("MAR", "Hospitalario", Some("/api/enfermeria/administraciones")),
    m("KARDEX", "Hospitalario", Some("/api/enfermeria/kardex")),
    m("CENSO_DIARIO", "Hospitalario", Some("/api/censo")),
    m("PLAN_CUIDADO", "Hospitalario", Some("/api/enfermeria/planes")),
    m("CONTRARREFERENCIA", "Hospitalario", Some("/api/contrarreferencia")),
    // --- Quirúrgico ---------------------------------------------------------
    m("QUIROFANO", "Quirúrgico", Some("/api/quirofano")),
    m("NOTA_OPERATORIA", "Quirúrgico", Some("/api/documentos")),
    m("PREANESTESIA", "Quirúrgico", Some("/api/anestesia/valoraciones")),
    m("RECUPERACION", "Quirúrgico", Some("/api/anestesia/recuperaciones")),
    // --- Diagnóstico --------------------------------------------------------
    m("LABORATORIO", "Diagnóstico", Some("/api/laboratorio")),
    m("IMAGENES", "Diagnóstico", Some("/api/imagenes")),
    // --- Soporte clínico ----------------------------------------------------
    m("FARMACIA", "Soporte clínico", Some("/api/farmacia")),
    m("ENFERMERIA", "Soporte clínico", Some("/api/enfermeria/notas")),
    m("EVOLUCIONES", "Soporte clínico", Some("/api/documentos")),
    m("CONSENTIMIENTO", "Soporte clínico", Some("/api/consentimiento")),
    m("BANCO_SANGRE", "Soporte clínico", Some("/api/banco-sangre")),
    // --- Especialidades -----------------------------------------------------
    m("NUTRICION", "Especialidades", Some("/api/nutricion")),
    m("REHABILITACION", "Especialidades", Some("/api/rehabilitacion")),
    m("TRABAJO_SOCIAL", "Especialidades", Some("/api/trabajo-social/notas")),
    m("TELEMEDICINA", "Especialidades", Some("/api/teleconsultas")),
    // --- Promoción y prevención --------------------------------------------
    m("VACUNACION", "Promoción y prevención", Some("/api/pyp/vacunas")),
    m("PRENATAL", "Promoción y prevención", Some("/api/pyp/prenatal")),
    m("CRECIMIENTO_DESARROLLO", "Promoción y prevención", Some("/api/pyp/crecimiento")),
    m("SALUD_REPRODUCTIVA", "Promoción y prevención", Some("/api/pyp/reproductiva")),
    // --- Registros civiles --------------------------------------------------
    m("CERTIFICADO_DEFUNCION", "Registros civiles", Some("/api/defunciones")),
    m("REGISTRO_NACIMIENTO", "Registros civiles", Some("/api/recien-nacidos")),
    m("INCAPACIDADES", "Registros civiles", Some("/api/incapacidades")),
    m("REMISIONES", "Registros civiles", Some("/api/remisiones")),
    // --- Traslados ----------------------------------------------------------
    m("AMBULANCIAS", "Traslados", Some("/api/ambulancias")),
    // --- Administrativo -----------------------------------------------------
    m("FACTURACION", "Administrativo", Some("/api/facturacion")),
    m("RIPS", "Administrativo", Some("/api/rips")),
    m("AUDITORIA", "Administrativo", Some("/api/admin/auditoria")),
    m("ESTADISTICAS", "Administrativo", Some("/api/estadisticas")),
    m("TURNOS", "Administrativo", Some("/api/turnos")),
    m("COMITES", "Administrativo", Some("/api/habilitacion/comites")),
    m("EQUIPOS_MEDICOS", "Administrativo", Some("/api/habilitacion/equipos")),
    m("INDICADORES", "Administrativo", Some("/api/habilitacion/indicadores")),
    m("INFECCIONES", "Administrativo", Some("/api/habilitacion/infecciones")),
];

const fn m(nombre: &'static str, grupo: &'static str, prefijo: Option<&'static str>) -> Modulo {
    Modulo { nombre, grupo, prefijo }
}

/// Los tres con los que arranca una institución nueva. Coincide con el `DEFAULT`
/// de la migración 0032, y `el_defecto_del_dominio_y_el_de_la_base_coinciden` lo
/// ata: si divergieran, una institución creada por SQL y otra creada por la API
/// tendrían licencias distintas sin que nadie lo decidiera.
pub const HABILITADOS_POR_DEFECTO: &[&str] = &["CITAS", "PACIENTES", "HISTORIA"];

/// ¿Existe este módulo en el catálogo? Se usa para rechazar un nombre inventado
/// al guardar la licencia, en vez de aceptarlo y que no signifique nada.
pub fn existe(nombre: &str) -> bool {
    CATALOGO.iter().any(|m| m.nombre == nombre)
}

pub fn por_nombre(nombre: &str) -> Option<&'static Modulo> {
    CATALOGO.iter().find(|m| m.nombre == nombre)
}

/// Un módulo del catálogo que sigue SIN construir, para quien necesite un
/// ejemplo de «licenciado y pendiente».
///
/// Existe porque tres pruebas escribían `KARDEX` a mano como ese ejemplo, y el
/// día que KARDEX se construyó (#249) dos se pusieron rojas y **una se quedó
/// verde midiendo nada** — pedía `/api/kardex`, una ruta que ya no reclamaba
/// nadie, así que seguía dando 404 por el motivo equivocado. Ése es el fallo de
/// la directiva 30: el negativo se ve igual venga de donde venga.
///
/// Devuelve `None` cuando no queda ninguno pendiente. Quien lo use tiene que
/// tratar ese caso a gritos: una prueba que se salta sola porque su ejemplo
/// desapareció es indistinguible de una que pasa.
pub fn alguno_pendiente() -> Option<&'static Modulo> {
    CATALOGO.iter().find(|m| m.prefijo.is_none())
}

/// ¿Esta ruta cae bajo ese prefijo, respetando los segmentos?
///
/// `starts_with` a secas NO sirve, y la trampa es real: `/api/citas` es prefijo
/// textual de `/api/citaseleccion`, que sería otro módulo. Se exige que la ruta
/// sea el prefijo exacto o siga con `/`.
fn bajo(ruta: &str, prefijo: &str) -> bool {
    ruta == prefijo || (ruta.starts_with(prefijo) && ruta.as_bytes().get(prefijo.len()) == Some(&b'/'))
}

/// Qué módulos reclaman esta ruta. **Vacío = no es ruta de módulo.**
///
/// # Por qué devuelve VARIOS y no uno
///
/// `/api/documentos` la sirven CUATRO módulos —`EPICRISIS`, `INTERCONSULTAS`,
/// `NOTA_OPERATORIA` y `EVOLUCIONES`—, que comparten una espina genérica. Con
/// «un módulo por ruta» habría que elegir uno, y entonces quien compró epicrisis
/// pero no evoluciones se quedaría sin ninguna de las dos o con las dos. La
/// regla es **cualquiera de los que la reclaman**: basta tener uno licenciado.
///
/// # Por qué gana el prefijo MÁS LARGO
///
/// `AUDITORIA` declara `/api/admin/auditoria`, que vive dentro del árbol de
/// administración. Sin la regla del más largo, una ruta podría quedar reclamada
/// por un módulo ancestro y por el suyo a la vez, y el «cualquiera de ellos»
/// abriría la puerta con la licencia equivocada.
///
/// # Lo que esta función NO decide
///
/// Si una ruta SIN reclamar debe estar fuera del licenciamiento. Eso es una
/// decisión por ruta y se declara en la capa de API, con su motivo escrito y su
/// guardián de fuente: aquí un vacío significa «el catálogo no la reclama», no
/// «es libre».
pub fn modulos_de_ruta(ruta: &str) -> Vec<&'static Modulo> {
    let mejor = CATALOGO
        .iter()
        .filter_map(|m| m.prefijo)
        .filter(|p| bajo(ruta, p))
        .map(str::len)
        .max();
    let Some(largo) = mejor else {
        return Vec::new();
    };
    CATALOGO
        .iter()
        .filter(|m| m.prefijo.is_some_and(|p| p.len() == largo && bajo(ruta, p)))
        .collect()
}

/// Los grupos, en el orden en que se declaran arriba. Es el orden del menú, y
/// va de lo clínico a lo administrativo a propósito.
pub fn grupos() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    for m in CATALOGO {
        if !v.contains(&m.grupo) {
            v.push(m.grupo);
        }
    }
    v
}

impl Modulo {
    /// El estado de este módulo para una institución.
    ///
    /// `sirve_ruta` responde si el binario atiende su prefijo; se pide como
    /// parámetro en vez de consultarse aquí para que el dominio no tenga que
    /// saber nada del router — y para que la prueba pueda darle las dos
    /// respuestas.
    pub fn estado(&self, habilitados: &[String], sirve_ruta: impl Fn(&str) -> bool) -> EstadoModulo {
        if !habilitados.iter().any(|h| h == self.nombre) {
            return EstadoModulo::NoIncluido;
        }
        match self.prefijo {
            Some(p) if sirve_ruta(p) => EstadoModulo::Habilitado,
            // Licenciado pero sin construir. NO se dice «habilitado» por no dar
            // una mala noticia: el cliente que lo compró tiene derecho a saber
            // que todavía no está, y el que abre el menú, a no encontrarse una
            // pantalla vacía sin explicación.
            _ => EstadoModulo::Pendiente,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn habilitados(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn los_tres_estados_se_distinguen() {
        let nutricion = por_nombre("NUTRICION").unwrap();
        // Licenciado y con ruta servida.
        assert_eq!(
            nutricion.estado(&habilitados(&["NUTRICION"]), |_| true),
            EstadoModulo::Habilitado
        );
        // Licenciado, con ruta declarada, pero el binario no la sirve.
        assert_eq!(
            nutricion.estado(&habilitados(&["NUTRICION"]), |_| false),
            EstadoModulo::Pendiente
        );
        // No licenciado: da igual lo que sirva el binario.
        assert_eq!(
            nutricion.estado(&habilitados(&["CITAS"]), |_| true),
            EstadoModulo::NoIncluido
        );
    }

    /// Un módulo sin ruta declarada es PENDIENTE aunque esté licenciado, y no
    /// «habilitado por defecto». Es el caso que hacía indistinguible un módulo
    /// vacío de uno sin construir.
    ///
    /// El módulo de ejemplo **se pide al catálogo**, no se escribe: decía
    /// `KARDEX` y se puso rojo el día que KARDEX se construyó (#249). Es la
    /// cuarta prueba con el mismo anclaje — las otras tres estaban en la capa de
    /// API, y una de ellas se había quedado verde midiendo nada.
    #[test]
    fn sin_ruta_declarada_es_pendiente_y_no_habilitado() {
        let m = alguno_pendiente()
            .expect("no queda ningún módulo pendiente: este caso ya no mide nada y hay que retirarlo");
        assert!(m.prefijo.is_none(), "«{}» tenía que estar sin construir", m.nombre);
        assert_eq!(
            m.estado(&habilitados(&[m.nombre]), |_| true),
            EstadoModulo::Pendiente
        );
    }

    #[test]
    fn el_catalogo_no_tiene_nombres_repetidos() {
        let mut vistos = std::collections::HashSet::new();
        for m in CATALOGO {
            assert!(vistos.insert(m.nombre), "módulo repetido: {}", m.nombre);
        }
        assert!(CATALOGO.len() >= 45, "el catálogo encogió: {}", CATALOGO.len());
    }

    #[test]
    fn los_grupos_salen_en_el_orden_declarado() {
        let g = grupos();
        assert_eq!(g.first(), Some(&"Core"));
        assert_eq!(g.last(), Some(&"Administrativo"));
        assert_eq!(g.len(), 10);
    }

    #[test]
    fn un_modulo_inventado_no_existe() {
        assert!(existe("NUTRICION"));
        assert!(!existe("TELETRANSPORTE"));
        assert!(!existe("nutricion"), "los nombres son sensibles a mayúsculas");
    }

    #[test]
    fn los_de_por_defecto_estan_en_el_catalogo() {
        for n in HABILITADOS_POR_DEFECTO {
            assert!(existe(n), "{n} es el defecto y no está en el catálogo");
        }
    }

    fn nombres(ruta: &str) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = modulos_de_ruta(ruta).iter().map(|m| m.nombre).collect();
        v.sort_unstable();
        v
    }

    /// La pareja: la ruta que TIENE que reclamarse y la que TIENE que no.
    #[test]
    fn una_ruta_de_modulo_se_reclama_y_una_de_sistema_no() {
        assert_eq!(nombres("/api/nutricion/valoraciones"), ["NUTRICION"]);
        assert_eq!(nombres("/api/nutricion"), ["NUTRICION"]);
        // Sistema y catálogos: el catálogo no las reclama, y por eso el vacío
        // NO puede significar «pasa» por sí solo (lo decide la capa de API).
        assert!(nombres("/api/acceso").is_empty());
        assert!(nombres("/api/yo").is_empty());
        assert!(nombres("/api/modulos").is_empty());
        assert!(nombres("/api/municipios").is_empty());
        assert!(nombres("/api/agenda/turnos").is_empty());
    }

    /// Una espina genérica servida por cuatro módulos: se devuelven los cuatro.
    /// Si esto devolviera uno solo, tres clientes de cada cuatro perderían un
    /// módulo que compraron.
    #[test]
    fn los_documentos_clinicos_los_reclaman_sus_cuatro_modulos() {
        assert_eq!(
            nombres("/api/documentos/7/firmar"),
            ["EPICRISIS", "EVOLUCIONES", "INTERCONSULTAS", "NOTA_OPERATORIA"]
        );
    }

    /// El prefijo más largo gana. `/api/admin/auditoria` es de `AUDITORIA`, y el
    /// resto del árbol de administración no lo reclama nadie.
    #[test]
    fn gana_el_prefijo_mas_largo_y_no_el_ancestro() {
        assert_eq!(nombres("/api/admin/auditoria"), ["AUDITORIA"]);
        assert!(nombres("/api/admin/usuarios").is_empty());
    }

    /// **El caso rojo del prefijo textual.** Con `starts_with` a secas,
    /// `/api/citas-de-otro-modulo` habría quedado reclamado por `CITAS` y su
    /// licencia habría abierto una puerta ajena. Se exige frontera de segmento.
    #[test]
    fn el_prefijo_respeta_la_frontera_del_segmento() {
        assert_eq!(nombres("/api/citas"), ["CITAS"]);
        assert_eq!(nombres("/api/citas/9/estado"), ["CITAS"]);
        assert!(
            nombres("/api/citaseleccion").is_empty(),
            "un prefijo textual no es un prefijo de ruta"
        );
    }
}
