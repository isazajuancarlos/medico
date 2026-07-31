// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

//! Lo que una institución **aparta** del defecto de autorización.
//!
//! [`crate::rol::RolUsuario::puede`] responde por el defecto nacional: la planta
//! típica de una IPS colombiana. Este módulo guarda las excepciones de UNA
//! institución —«aquí el terapeuta también atiende urgencias», «aquí el médico
//! no toca nutrición porque hay nutricionista en planta»— y las superpone.
//!
//! **Solo se guarda lo que se aparta.** Una institución que no configura nada no
//! tiene ni una fila, y sigue el defecto entero; si mañana el defecto cambia,
//! esa institución se mueve con él. Guardar la matriz completa haría lo
//! contrario: congelaría el defecto del día de la instalación en cada cliente,
//! sin que nadie lo hubiera decidido.

use std::collections::HashMap;

use crate::rol::{Capacidad, RolUsuario};

/// Por qué se rechaza un cambio en la matriz.
///
/// Cada variante nombra una situación **imposible de reparar desde dentro**, no
/// una inusual (directiva 21). Que una capacidad se quede sin ningún rol es
/// perfectamente legítimo —una IPS sin ambulancias no quiere que nadie las
/// gestione— y por eso NO está aquí: siempre queda alguien que puede volver a
/// concederla. `GestionarUsuarios` es la excepción porque es la puerta desde la
/// que se arregla todo lo demás.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rechazo {
    /// La matriz de una institución no puede decir nada sobre este rol.
    RolNoConfigurable(RolUsuario),
    /// Ni sobre esta capacidad.
    CapacidadNoConfigurable(Capacidad),
    /// Dejaría a la institución sin NADIE que pueda gestionar usuarios: nadie
    /// podría volver a tocar esta matriz ni dar de alta a quien lo hiciera.
    SinQuienGestioneUsuarios,
    /// El administrador se estaría quitando a sí mismo la capacidad de
    /// administrar. Es el mismo callejón que el anterior, en pequeño, y se
    /// comete con más facilidad: basta desmarcar la casilla de la fila propia.
    SeQuitaSuPropiaAdministracion(RolUsuario),
}

impl std::fmt::Display for Rechazo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rechazo::RolNoConfigurable(r) => write!(
                f,
                "el rol «{}» no se configura por institución",
                r.como_str()
            ),
            Rechazo::CapacidadNoConfigurable(c) => write!(
                f,
                "la capacidad «{}» no se configura por institución",
                c.como_str()
            ),
            Rechazo::SinQuienGestioneUsuarios => write!(
                f,
                "dejaría a la institución sin ningún rol que pueda gestionar usuarios: \
                 nadie podría volver a cambiar esta matriz"
            ),
            Rechazo::SeQuitaSuPropiaAdministracion(r) => write!(
                f,
                "quitaría a «{}», que es su propio rol, la gestión de usuarios",
                r.como_str()
            ),
        }
    }
}

/// Las excepciones de una institución: `(capacidad, rol) -> permitido`.
///
/// Lo que no está en el mapa **no es un «no»**: es «sigue el defecto». Esa
/// distinción es todo el diseño, y por eso el mapa guarda un `bool` en vez de
/// ser un conjunto de lo permitido — un conjunto no sabría distinguir «se lo
/// quitaron» de «nunca lo tuvo».
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ajustes {
    apartes: HashMap<(Capacidad, RolUsuario), bool>,
}

impl Ajustes {
    /// Construye desde las filas de la base. Las filas que nombran un rol o una
    /// capacidad desconocidos —una base de una versión posterior, o tocada a
    /// mano— se **descartan**: se cae al defecto, que es la respuesta segura.
    /// Lo contrario sería adivinar.
    pub fn desde_filas<I>(filas: I) -> Self
    where
        I: IntoIterator<Item = (String, String, bool)>,
    {
        let mut apartes = HashMap::new();
        for (capacidad, rol, permitido) in filas {
            if let (Some(c), Some(r)) = (Capacidad::desde_str(&capacidad), RolUsuario::desde_str(&rol))
            {
                apartes.insert((c, r), permitido);
            }
        }
        Self { apartes }
    }

    /// Sin ninguna excepción: todo sigue el defecto.
    pub fn vacios() -> Self {
        Self::default()
    }

    /// Cuántas excepciones hay. Solo para diagnóstico y pruebas.
    pub fn cuantos(&self) -> usize {
        self.apartes.len()
    }

    /// Las excepciones, ordenadas, para devolverlas por la API.
    pub fn listar(&self) -> Vec<(Capacidad, RolUsuario, bool)> {
        let mut v: Vec<_> = self
            .apartes
            .iter()
            .map(|((c, r), p)| (*c, *r, *p))
            .collect();
        v.sort_by_key(|(c, r, _)| (*c, r.como_str()));
        v
    }

    /// Fija una excepción sin validar. Para construir en pruebas y para aplicar
    /// un cambio YA validado con [`Ajustes::validar`].
    pub fn fijar(&mut self, capacidad: Capacidad, rol: RolUsuario, permitido: bool) {
        self.apartes.insert((capacidad, rol), permitido);
    }

    /// **La pregunta que responde todo el módulo**: ¿este rol puede esto, en
    /// esta institución?
    ///
    /// Defecto, salvo que la institución lo haya apartado. Un rol no
    /// configurable —el superadmin— nunca mira el mapa: su respuesta es siempre
    /// la del defecto, aunque alguien hubiera colado una fila en la base.
    pub fn puede(&self, rol: RolUsuario, capacidad: Capacidad) -> bool {
        if !rol.configurable_por_institucion() || !capacidad.configurable_por_institucion() {
            return rol.puede(capacidad);
        }
        match self.apartes.get(&(capacidad, rol)) {
            Some(&permitido) => permitido,
            None => rol.puede(capacidad),
        }
    }

    /// Qué roles pueden una capacidad en esta institución, según la matriz ya
    /// resuelta. Es lo que necesita el guardarraíl y lo que pinta la pantalla.
    pub fn roles_que_pueden(&self, capacidad: Capacidad) -> Vec<RolUsuario> {
        RolUsuario::TODOS
            .iter()
            .copied()
            .filter(|r| self.puede(*r, capacidad))
            .collect()
    }

    /// Comprueba un cambio ANTES de guardarlo.
    ///
    /// `quien` es el rol de quien hace el cambio: hace falta para el segundo
    /// guardarraíl, y se pide explícitamente en vez de deducirlo para que quede
    /// en la firma quién responde por la operación.
    ///
    /// Se valida contra el estado RESULTANTE, no contra el cambio suelto:
    /// quitar `GestionarUsuarios` a `Admin` es inocuo si `Medico` lo tiene, y
    /// fatal si no. Un cambio no se puede juzgar sin el resto de la matriz.
    pub fn validar(
        &self,
        cambios: &[(Capacidad, RolUsuario, bool)],
        quien: RolUsuario,
    ) -> Result<Ajustes, Rechazo> {
        let mut resultado = self.clone();
        for &(capacidad, rol, permitido) in cambios {
            if !rol.configurable_por_institucion() {
                return Err(Rechazo::RolNoConfigurable(rol));
            }
            if !capacidad.configurable_por_institucion() {
                return Err(Rechazo::CapacidadNoConfigurable(capacidad));
            }
            resultado.fijar(capacidad, rol, permitido);
        }

        // Guardarraíl 1: la puerta desde la que se arregla todo lo demás no se
        // puede tapiar. Ojo: se mira SOLO entre los roles configurables, porque
        // el superadmin no pertenece a esta institución y no sirve de red.
        let quedan = resultado
            .roles_que_pueden(Capacidad::GestionarUsuarios)
            .into_iter()
            .filter(|r| r.configurable_por_institucion())
            .count();
        if quedan == 0 {
            return Err(Rechazo::SinQuienGestioneUsuarios);
        }

        // Guardarraíl 2: y no la puede cerrar sobre sí mismo. Al superadmin no
        // le aplica —no está en la matriz—, así que no se le estorba.
        if quien.configurable_por_institucion()
            && !resultado.puede(quien, Capacidad::GestionarUsuarios)
        {
            return Err(Rechazo::SeQuitaSuPropiaAdministracion(quien));
        }

        Ok(resultado)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn sin_ajustes_manda_el_defecto() {
        let a = Ajustes::vacios();
        // Lo que el defecto concede.
        assert!(a.puede(RolUsuario::Medico, Capacidad::EscribirClinico));
        // Y lo que no.
        assert!(!a.puede(RolUsuario::Laboratorista, Capacidad::EscribirClinico));
    }

    #[test]
    fn una_institucion_puede_ampliar_y_recortar() {
        let mut a = Ajustes::vacios();
        // Un hospital con terapeutas en urgencias.
        a.fijar(
            Capacidad::AtenderUrgenciasHospitalizacion,
            RolUsuario::Terapeuta,
            true,
        );
        // Y con nutricionista en planta, así que el médico no registra nutrición.
        a.fijar(Capacidad::EscribirNutricion, RolUsuario::Medico, false);

        assert!(a.puede(
            RolUsuario::Terapeuta,
            Capacidad::AtenderUrgenciasHospitalizacion
        ));
        assert!(!a.puede(RolUsuario::Medico, Capacidad::EscribirNutricion));
        // Y lo que no se tocó sigue el defecto, en los dos sentidos.
        assert!(a.puede(RolUsuario::Medico, Capacidad::EscribirClinico));
        assert!(!a.puede(RolUsuario::Terapeuta, Capacidad::EscribirClinico));
    }

    /// La distinción que justifica guardar un `bool` y no un conjunto: «se lo
    /// quitaron» y «nunca lo tuvo» dan la misma respuesta pero NO son el mismo
    /// dato, y solo el primero sobrevive a un cambio del defecto.
    #[test]
    fn quitado_y_nunca_concedido_se_distinguen() {
        let mut a = Ajustes::vacios();
        a.fijar(Capacidad::EscribirClinico, RolUsuario::Medico, false);
        assert!(!a.puede(RolUsuario::Medico, Capacidad::EscribirClinico));
        assert!(!a.puede(RolUsuario::Laboratorista, Capacidad::EscribirClinico));
        // Uno está guardado, el otro no.
        assert_eq!(a.cuantos(), 1);
        assert_eq!(
            a.listar(),
            vec![(Capacidad::EscribirClinico, RolUsuario::Medico, false)]
        );
    }

    #[test]
    fn al_superadmin_no_le_alcanza_la_matriz() {
        let mut a = Ajustes::vacios();
        // Aunque alguien cuele la fila directamente en la base.
        a.fijar(Capacidad::EscribirClinico, RolUsuario::Superadmin, false);
        assert!(
            a.puede(RolUsuario::Superadmin, Capacidad::EscribirClinico),
            "el superadmin no pertenece a la institución: su fila no se aplica"
        );
    }

    #[test]
    fn crear_instituciones_no_se_puede_conceder_desde_una_institucion() {
        let mut a = Ajustes::vacios();
        a.fijar(Capacidad::GestionarInstituciones, RolUsuario::Admin, true);
        assert!(
            !a.puede(RolUsuario::Admin, Capacidad::GestionarInstituciones),
            "un admin no se concede la llave del piso de arriba"
        );
        // Y al validarlo se dice, en vez de aceptarlo y no aplicarlo.
        assert_eq!(
            Ajustes::vacios().validar(
                &[(Capacidad::GestionarInstituciones, RolUsuario::Admin, true)],
                RolUsuario::Admin
            ),
            Err(Rechazo::CapacidadNoConfigurable(
                Capacidad::GestionarInstituciones
            ))
        );
    }

    #[test]
    fn una_fila_para_el_superadmin_se_rechaza_al_escribir() {
        assert_eq!(
            Ajustes::vacios().validar(
                &[(Capacidad::EscribirClinico, RolUsuario::Superadmin, false)],
                RolUsuario::Admin
            ),
            Err(Rechazo::RolNoConfigurable(RolUsuario::Superadmin))
        );
    }

    // === GUARDARRAÍLES ====================================================

    #[test]
    fn no_se_puede_dejar_la_institucion_sin_quien_gestione_usuarios() {
        // Por defecto solo Admin (y Superadmin, que no cuenta) la tienen.
        let r = Ajustes::vacios().validar(
            &[(Capacidad::GestionarUsuarios, RolUsuario::Admin, false)],
            RolUsuario::Superadmin,
        );
        assert_eq!(r, Err(Rechazo::SinQuienGestioneUsuarios));
    }

    #[test]
    fn pero_si_hay_relevo_el_cambio_pasa() {
        // Primero se le da a Medico, luego se le quita a Admin: legítimo.
        let a = Ajustes::vacios()
            .validar(
                &[(Capacidad::GestionarUsuarios, RolUsuario::Medico, true)],
                RolUsuario::Superadmin,
            )
            .expect("ampliar siempre es válido");
        let b = a.validar(
            &[(Capacidad::GestionarUsuarios, RolUsuario::Admin, false)],
            RolUsuario::Superadmin,
        );
        assert!(b.is_ok(), "queda Medico como relevo: {b:?}");
    }

    /// Los dos cambios en la MISMA petición también valen: se valida contra el
    /// estado resultante, no cambio a cambio. Si se validara suelto, el orden
    /// de las casillas decidiría si la petición pasa — y el administrador no
    /// controla ese orden.
    #[test]
    fn el_relevo_vale_en_la_misma_peticion_y_en_cualquier_orden() {
        let quitar = (Capacidad::GestionarUsuarios, RolUsuario::Admin, false);
        let dar = (Capacidad::GestionarUsuarios, RolUsuario::Medico, true);
        assert!(Ajustes::vacios().validar(&[quitar, dar], RolUsuario::Medico).is_ok());
        assert!(Ajustes::vacios().validar(&[dar, quitar], RolUsuario::Medico).is_ok());
    }

    #[test]
    fn un_admin_no_se_quita_a_si_mismo_la_administracion() {
        // Hay relevo (Medico), así que el guardarraíl 1 no salta; salta el 2.
        let a = Ajustes::vacios()
            .validar(
                &[(Capacidad::GestionarUsuarios, RolUsuario::Medico, true)],
                RolUsuario::Admin,
            )
            .unwrap();
        assert_eq!(
            a.validar(
                &[(Capacidad::GestionarUsuarios, RolUsuario::Admin, false)],
                RolUsuario::Admin
            ),
            Err(Rechazo::SeQuitaSuPropiaAdministracion(RolUsuario::Admin))
        );
    }

    /// Y el mismo cambio SÍ lo puede hacer otro: el guardarraíl protege contra
    /// el despiste propio, no convierte al rol en intocable.
    #[test]
    fn pero_otro_si_puede_hacerselo() {
        let a = Ajustes::vacios()
            .validar(
                &[(Capacidad::GestionarUsuarios, RolUsuario::Medico, true)],
                RolUsuario::Superadmin,
            )
            .unwrap();
        assert!(a
            .validar(
                &[(Capacidad::GestionarUsuarios, RolUsuario::Admin, false)],
                RolUsuario::Medico
            )
            .is_ok());
    }

    /// **Que una capacidad se quede sin nadie es legítimo** — el caso que la
    /// ficha original bloqueaba y que la directiva 21 rescata: una IPS sin
    /// ambulancias no quiere que nadie las gestione, y es recuperable porque
    /// queda quien pueda volver a concederlo.
    #[test]
    fn una_ips_sin_ambulancias_puede_dejarlas_sin_nadie() {
        let mut ajustes = Ajustes::vacios();
        let cambios: Vec<_> = ajustes
            .roles_que_pueden(Capacidad::GestionarAmbulancias)
            .into_iter()
            .filter(|r| r.configurable_por_institucion())
            .map(|r| (Capacidad::GestionarAmbulancias, r, false))
            .collect();
        assert!(!cambios.is_empty(), "el defecto concede a alguien");
        ajustes = ajustes
            .validar(&cambios, RolUsuario::Admin)
            .expect("dejar ambulancias sin nadie es una configuración válida");
        assert!(ajustes
            .roles_que_pueden(Capacidad::GestionarAmbulancias)
            .into_iter()
            .all(|r| !r.configurable_por_institucion()));
        // Y sigue habiendo quien lo repare.
        assert!(ajustes.puede(RolUsuario::Admin, Capacidad::GestionarUsuarios));
    }

    // === DESDE LA BASE ====================================================

    #[test]
    fn las_filas_de_la_base_se_leen_y_las_desconocidas_se_descartan() {
        let a = Ajustes::desde_filas([
            ("escribir_clinico".to_string(), "laboratorista".to_string(), true),
            // De una versión posterior, o tocada a mano: se cae al defecto.
            ("volar".to_string(), "medico".to_string(), true),
            ("escribir_clinico".to_string(), "arcangel".to_string(), true),
        ]);
        assert_eq!(a.cuantos(), 1, "solo sobrevive la fila que nombra cosas reales");
        assert!(a.puede(RolUsuario::Laboratorista, Capacidad::EscribirClinico));
        // Y la única que se guardó es esa, no otra: una fila con un rol
        // desconocido no puede acabar concediéndole nada a un rol conocido.
        assert_eq!(
            a.listar(),
            vec![(
                Capacidad::EscribirClinico,
                RolUsuario::Laboratorista,
                true
            )]
        );
    }

    /// Que descartar la fila basura **caiga al defecto** y no a «no», que es la
    /// otra manera de equivocarse: un rol legítimo no puede perder lo que el
    /// defecto le da porque alguien escribiera mal otra fila.
    #[test]
    fn una_fila_ilegible_no_le_quita_nada_a_quien_no_nombra() {
        let a = Ajustes::desde_filas([
            ("volar".to_string(), "medico".to_string(), false),
            ("escribir_clinico".to_string(), "arcangel".to_string(), false),
        ]);
        assert_eq!(a.cuantos(), 0);
        assert!(a.puede(RolUsuario::Medico, Capacidad::EscribirClinico));
    }
}
