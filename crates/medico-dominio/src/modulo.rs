// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

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
    m("MAR", "Hospitalario", None),
    m("KARDEX", "Hospitalario", None),
    m("CENSO_DIARIO", "Hospitalario", None),
    m("PLAN_CUIDADO", "Hospitalario", None),
    m("CONTRARREFERENCIA", "Hospitalario", Some("/api/contrarreferencia")),
    // --- Quirúrgico ---------------------------------------------------------
    m("QUIROFANO", "Quirúrgico", Some("/api/quirofano")),
    m("NOTA_OPERATORIA", "Quirúrgico", Some("/api/documentos")),
    m("PREANESTESIA", "Quirúrgico", None),
    m("RECUPERACION", "Quirúrgico", None),
    // --- Diagnóstico --------------------------------------------------------
    m("LABORATORIO", "Diagnóstico", Some("/api/laboratorio")),
    m("IMAGENES", "Diagnóstico", Some("/api/imagenes")),
    // --- Soporte clínico ----------------------------------------------------
    m("FARMACIA", "Soporte clínico", Some("/api/farmacia")),
    m("ENFERMERIA", "Soporte clínico", None),
    m("EVOLUCIONES", "Soporte clínico", Some("/api/documentos")),
    m("CONSENTIMIENTO", "Soporte clínico", Some("/api/consentimiento")),
    m("BANCO_SANGRE", "Soporte clínico", None),
    // --- Especialidades -----------------------------------------------------
    m("NUTRICION", "Especialidades", Some("/api/nutricion")),
    m("REHABILITACION", "Especialidades", Some("/api/rehabilitacion")),
    m("TRABAJO_SOCIAL", "Especialidades", None),
    m("TELEMEDICINA", "Especialidades", None),
    // --- Promoción y prevención --------------------------------------------
    m("VACUNACION", "Promoción y prevención", None),
    m("PRENATAL", "Promoción y prevención", None),
    m("CRECIMIENTO_DESARROLLO", "Promoción y prevención", None),
    m("SALUD_REPRODUCTIVA", "Promoción y prevención", None),
    // --- Registros civiles --------------------------------------------------
    m("CERTIFICADO_DEFUNCION", "Registros civiles", None),
    m("REGISTRO_NACIMIENTO", "Registros civiles", None),
    m("INCAPACIDADES", "Registros civiles", Some("/api/incapacidades")),
    m("REMISIONES", "Registros civiles", Some("/api/remisiones")),
    // --- Traslados ----------------------------------------------------------
    m("AMBULANCIAS", "Traslados", Some("/api/ambulancias")),
    // --- Administrativo -----------------------------------------------------
    m("FACTURACION", "Administrativo", Some("/api/facturacion")),
    m("RIPS", "Administrativo", Some("/api/rips")),
    m("AUDITORIA", "Administrativo", Some("/api/admin/auditoria")),
    m("ESTADISTICAS", "Administrativo", Some("/api/estadisticas")),
    m("TURNOS", "Administrativo", None),
    m("COMITES", "Administrativo", None),
    m("EQUIPOS_MEDICOS", "Administrativo", None),
    m("INDICADORES", "Administrativo", None),
    m("INFECCIONES", "Administrativo", None),
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
    #[test]
    fn sin_ruta_declarada_es_pendiente_y_no_habilitado() {
        let kardex = por_nombre("KARDEX").unwrap();
        assert!(kardex.prefijo.is_none());
        assert_eq!(
            kardex.estado(&habilitados(&["KARDEX"]), |_| true),
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
}
