// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

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
    /// Se intenta CONCEDER una capacidad que el rol de quien concede no trae por
    /// defecto nacional. Ver el guardarraíl 3.
    NoPuedeConcederLoQueNoTiene(Capacidad),
}

impl std::fmt::Display for Rechazo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rechazo::RolNoConfigurable(r) => write!(
                f,
                "el rol «{}» no se configura por institución",
                r.como_str()
            ),
            Rechazo::NoPuedeConcederLoQueNoTiene(c) => write!(
                f,
                "no puede conceder «{}»: su rol no la tiene",
                c.como_str()
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

        // Guardarraíl 3: NADIE CONCEDE LO QUE SU PROPIO ROL NO TRAE.
        //
        // Vivía SOLO en el manejador de `PUT /api/capacidades`, y el 2026-08-10
        // la revisión de seguridad midió el precio: el manejador del PANEL —que
        // desde que Angular salió es la ÚNICA interfaz— llamaba a este `validar`
        // creyendo que la aplicaba, y su comentario lo afirmaba por escrito. No
        // la aplicaba. Un laboratorista al que el admin le hubiera concedido
        // solo `gestionar_usuarios` podía concederse `firmar_clinico` y firmar
        // historia clínica (Ley 527/1999), mientras la misma petición contra la
        // API daba 403.
        //
        // Por eso sube AQUÍ y se borra allí (directiva 1): una regla en dos
        // sitios es una regla que la tercera puerta hereda solo si alguien se
        // acuerda, y ya iban dos.
        //
        // Va al FINAL a propósito: así los rechazos más específicos —rol o
        // capacidad no configurables, y los dos guardarraíles de arriba— siguen
        // ganando, que es el orden exacto que tenía cuando la comprobación
        // estaba después de `validar` en la API. Cambiarlo alteraría qué motivo
        // se contesta cuando concurren dos.
        //
        // Se compara contra el DEFECTO NACIONAL (`quien.puede`) y no contra la
        // matriz efectiva, y esa elección es la regla: contra la efectiva, un
        // admin que se hubiera revocado `firmar_clinico` no podría devolvérselo
        // NUNCA (directiva 21). Contra el defecto, un `Admin` no pierde nada
        // —tiene las diecisiete configurables— y el laboratorista sigue sin
        // poder conceder lo que su rol no contempla.
        if let Some(&(capacidad, _, _)) = cambios
            .iter()
            .find(|&&(cap, _, permitido)| permitido && !quien.puede(cap))
        {
            return Err(Rechazo::NoPuedeConcederLoQueNoTiene(capacidad));
        }

        Ok(resultado)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    // ── Guardarraíl 3: nadie concede lo que su rol no trae ──────────────────
    //
    // La regla no tenía prueba en NINGUNA de las dos puertas hasta el
    // 2026-08-10, y por eso pudo vivir en una sola de ellas sin que nada se
    // pusiera rojo.

    #[test]
    fn un_laboratorista_con_gestionar_usuarios_no_se_concede_firmar() {
        // El caso EXACTO que la revisión de seguridad encontró explotable por el
        // panel. La precondición es legítima y la matriz la ofrece: el admin le
        // concedió `GestionarUsuarios` al laboratorista.
        let base = Ajustes::vacios();
        let con_gestion = base
            .validar(
                &[(Capacidad::GestionarUsuarios, RolUsuario::Laboratorista, true)],
                RolUsuario::Admin,
            )
            .expect("un admin SÍ puede delegar gestionar_usuarios");

        let r = con_gestion.validar(
            &[(Capacidad::FirmarClinico, RolUsuario::Laboratorista, true)],
            RolUsuario::Laboratorista,
        );
        assert!(
            matches!(r, Err(Rechazo::NoPuedeConcederLoQueNoTiene(Capacidad::FirmarClinico))),
            "un laboratorista no puede concederse firmar historia clínica: {r:?}"
        );
    }

    #[test]
    fn el_admin_si_concede_lo_que_su_rol_trae() {
        // La pareja, y la que impide que la regla se convierta en «no se concede
        // nada»: el admin trae las diecisiete configurables por defecto
        // nacional, así que su operación normal no se estorba (directiva 21).
        let a = Ajustes::vacios();
        assert!(
            a.validar(
                &[(Capacidad::FirmarClinico, RolUsuario::Laboratorista, true)],
                RolUsuario::Admin,
            )
            .is_ok(),
            "el admin sí puede: es exactamente lo que la matriz existe para hacer"
        );
    }

    #[test]
    fn quitar_no_es_conceder() {
        // La regla mira solo lo que se CONCEDE (`permitido`). Un laboratorista
        // con gestión de usuarios puede RETIRAR una capacidad que él no tiene:
        // recortar no amplía a nadie, y cerrarlo sería el sesgo a cerrar.
        let base = Ajustes::vacios();
        let r = base.validar(
            &[(Capacidad::FirmarClinico, RolUsuario::Medico, false)],
            RolUsuario::Laboratorista,
        );
        // El único motivo por el que esto puede fallar es el guardarraíl 2,
        // no el 3 — se comprueba que NO es «no puede conceder».
        assert!(
            !matches!(r, Err(Rechazo::NoPuedeConcederLoQueNoTiene(_))),
            "retirar no es conceder: {r:?}"
        );
    }

    #[test]
    fn se_juzga_contra_el_defecto_nacional_y_no_contra_la_matriz_efectiva() {
        // Un admin que se hubiera revocado `firmar_clinico` tiene que poder
        // devolvérselo. Contra la matriz EFECTIVA no podría nunca —quedaría
        // encerrado sin más salida que abrir la base a mano—, y por eso la regla
        // compara contra `quien.puede`, el defecto del rol.
        let sin_firma = Ajustes::vacios()
            .validar(
                &[(Capacidad::FirmarClinico, RolUsuario::Admin, false)],
                RolUsuario::Admin,
            )
            .expect("revocársela es legítimo");
        assert!(!sin_firma.puede(RolUsuario::Admin, Capacidad::FirmarClinico));
        assert!(
            sin_firma
                .validar(
                    &[(Capacidad::FirmarClinico, RolUsuario::Admin, true)],
                    RolUsuario::Admin,
                )
                .is_ok(),
            "y devolvérsela también, o se queda encerrado"
        );
    }

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
    ///
    /// El actor era `Medico` y pasó a `Superadmin` el 2026-08-10, al entrar el
    /// guardarraíl 3: un `Medico` no trae `GestionarUsuarios` por defecto
    /// nacional, así que ya no puede CONCEDERLA. Lo que esta prueba mide es el
    /// ORDEN de los cambios, no quién los hace, y con el actor viejo habría
    /// medido el guardarraíl equivocado. Que se pusiera roja fue el positivo de
    /// control de que la regla nueva muerde.
    #[test]
    fn el_relevo_vale_en_la_misma_peticion_y_en_cualquier_orden() {
        let quitar = (Capacidad::GestionarUsuarios, RolUsuario::Admin, false);
        let dar = (Capacidad::GestionarUsuarios, RolUsuario::Medico, true);
        assert!(Ajustes::vacios().validar(&[quitar, dar], RolUsuario::Superadmin).is_ok());
        assert!(Ajustes::vacios().validar(&[dar, quitar], RolUsuario::Superadmin).is_ok());
    }

    /// Y la CONSECUENCIA de esa regla, escrita como prueba para que no
    /// sorprenda: `GestionarUsuarios` **no se propaga**. Un rol que la tiene por
    /// la matriz de su institución —no por defecto nacional— no puede
    /// concedérsela a un tercero.
    ///
    /// Es deliberado y es la mitad de la defensa: si pudiera, el laboratorista
    /// al que el admin delegó la administración podría repartirla, y cada nuevo
    /// portador sería otro camino hacia el resto de la matriz. Quien quiera
    /// delegar la administración tiene que ser admin.
    #[test]
    fn gestionar_usuarios_no_se_propaga_entre_roles_delegados() {
        let delegado = Ajustes::vacios()
            .validar(
                &[(Capacidad::GestionarUsuarios, RolUsuario::Laboratorista, true)],
                RolUsuario::Admin,
            )
            .expect("el admin sí delega");
        let r = delegado.validar(
            &[(Capacidad::GestionarUsuarios, RolUsuario::Enfermeria, true)],
            RolUsuario::Laboratorista,
        );
        assert!(
            matches!(r, Err(Rechazo::NoPuedeConcederLoQueNoTiene(_))),
            "un delegado no reparte la administración: {r:?}"
        );
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
