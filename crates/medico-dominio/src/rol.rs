//! Los roles del sistema, con los mismos valores de cadena que `models.py:52`.
//!
//! No se renombran ni se agrupan: esos valores están escritos en la base y en
//! los tokens ya emitidos. Un rol «mejor nombrado» invalidaría sesiones y
//! filas existentes a cambio de nada.

use serde::{Deserialize, Serialize};

use crate::Ambito;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolUsuario {
    Superadmin,
    Admin,
    Medico,
    Paciente,
    Enfermeria,
    AuxiliarEnfermeria,
    RegenteFarmacia,
    Laboratorista,
    Radiologo,
    Anestesiologo,
    Terapeuta,
    Nutricionista,
    Psicologo,
    TrabajoSocial,
    Bacteriologo,
}

/// Lo que un rol puede HACER, aparte de a qué institución alcanza.
///
/// # Por qué hacen falta dos cosas y no una
///
/// [`Ambito`] responde «¿de qué institución son los datos que ve?». No responde
/// «¿puede firmar una historia clínica?». Y como [`RolUsuario::ambito`] manda a
/// todo rol no-superadmin con institución al mismo `Ambito::Institucion(id)`,
/// durante el port TODOS los roles clínicos quedaron con la misma autoridad: un
/// laboratorista podía firmar y borrar historias igual que el médico tratante. El
/// aislamiento entre instituciones estaba bien hecho; la autorización por rol no
/// existía.
///
/// # Lo que esta enumeración cubre, y lo que a propósito no
///
/// Están **solo** las capacidades cuya respuesta ya la da el producto: la matriz
/// de `frontend/src/app/app.routes.ts` (que hoy vive únicamente en el navegador,
/// o sea en ningún sitio) y las comprobaciones que las rutas ya hacían a mano.
///
/// **No** están todavía «leer» y «escribir» por módulo. No es un olvido: la
/// matriz del frontend cubre seis páginas, y siete de los quince roles
/// —laboratorista, radiólogo, anestesiólogo, terapeuta, nutricionista, trabajo
/// social, bacteriólogo— no aparecen en ninguna, mientras que sí existen módulos
/// suyos en la API. Inventar ahí una respuesta cerrada dejaría a una
/// nutricionista fuera del módulo de nutrición, que es el mismo fallo con el
/// signo cambiado. Esa parte se decide con el cliente y se añade aquí; mientras
/// tanto esta enumeración **no finge** cubrirla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capacidad {
    /// Firmar una historia o un documento clínico.
    ///
    /// Es capacidad PROPIA y no «escribir con otro nombre»: la firma afirma
    /// autoría, y bajo la Ley 527 de 1999 el artefacto resultante vale como tal.
    /// Quien firma queda registrado como quien lo hizo.
    FirmarClinico,
    /// Borrar una historia o un documento clínico no firmado.
    BorrarClinico,
    /// Crear y listar usuarios de la institución.
    GestionarUsuarios,
    /// Crear instituciones y fijarles licencia.
    GestionarInstituciones,
}

impl RolUsuario {
    /// ¿Este rol puede hacer esto?
    ///
    /// La lista es explícita y sin comodín: un rol nuevo no hereda nada por
    /// descuido, y quitar uno de aquí es un cambio que se ve en el diff.
    pub fn puede(&self, que: Capacidad) -> bool {
        use Capacidad::*;
        use RolUsuario::*;
        match que {
            // Los mismos tres que el producto ya deja entrar a /historias
            // (`app.routes.ts:42`). Un laboratorista o un trabajador social
            // firmando la nota de un médico no es un caso inusual: es uno que no
            // existe.
            FirmarClinico | BorrarClinico => matches!(self, Superadmin | Admin | Medico),
            // Ya se comprobaba a mano en las rutas; aquí queda dicho una vez.
            GestionarUsuarios => matches!(self, Superadmin | Admin),
            GestionarInstituciones => matches!(self, Superadmin),
        }
    }

    /// Traduce `(rol, institucion_id)` de la base al ámbito de visibilidad.
    ///
    /// Es la única conversión del sistema donde un descuido concede todo o
    /// cierra todo, así que ocurre en un solo sitio y tiene nombre. En Python
    /// estaba repartida: cada router comprobaba por su cuenta si
    /// `institucion_id` era `None` antes de filtrar, y bastaba olvidarlo una
    /// vez.
    pub fn ambito(&self, institucion_id: Option<i64>) -> Ambito {
        match (self, institucion_id) {
            (RolUsuario::Superadmin, _) => Ambito::Superadmin,
            // El PACIENTE no es personal de la institución, y darle
            // `Institucion(id)` le daba la historia clínica de todo el mundo: el
            // listado de pacientes, las notas, los bytes de las imágenes. No es
            // un permiso de más, es el peor que hay en este sistema.
            //
            // Su ámbito propio —ver lo SUYO— es lo que corresponde, y no está
            // escrito porque el portal del paciente no existe: `auth.service.ts`
            // manda el rol `paciente` a `/portal-citas` y esa página no está en
            // `app.routes.ts`. Así que hoy una cuenta de paciente no tiene
            // ninguna pantalla legítima, y `Ninguno` no le quita nada que
            // funcione.
            //
            // CUANDO SE CONSTRUYA EL PORTAL: no basta con quitar esta línea —
            // hay que añadir un `Ambito::Paciente(paciente_id)` y que el
            // compilador obligue a cada consulta a decir qué hace con él.
            // Devolverlo a `Institucion` reabre esto entero.
            (RolUsuario::Paciente, _) => Ambito::Ninguno,
            (_, Some(id)) => Ambito::Institucion(id),
            (_, None) => Ambito::Ninguno,
        }
    }

    /// Del valor de cadena guardado en la base al rol. Usa el MISMO
    /// `Deserialize` (snake_case) con el que se serializa, así que no hay una
    /// segunda lista de nombres que se pueda desincronizar. `None` si la cadena
    /// no nombra un rol conocido — que arriba se traduce a «no ve nada», nunca a
    /// «lo ve todo».
    pub fn desde_str(s: &str) -> Option<Self> {
        use serde::de::IntoDeserializer;
        let de: serde::de::value::StrDeserializer<serde::de::value::Error> = s.into_deserializer();
        <Self as serde::Deserialize>::deserialize(de).ok()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::Visibilidad;

    /// El superadmin tiene `institucion_id = NULL` y aun así lo ve todo.
    #[test]
    fn el_superadmin_sin_institucion_no_cae_en_ninguno() {
        assert_eq!(RolUsuario::Superadmin.ambito(None), Ambito::Superadmin);
    }

    /// Y al revés: cualquier otro rol sin institución NO hereda el privilegio.
    /// Es el fallo que se busca hacer imposible — los dos casos son
    /// `institucion_id = None` en la base.
    #[test]
    fn otro_rol_sin_institucion_no_ve_nada() {
        for rol in [
            RolUsuario::Admin,
            RolUsuario::Medico,
            RolUsuario::Enfermeria,
            RolUsuario::Paciente,
            RolUsuario::Bacteriologo,
        ] {
            assert_eq!(rol.ambito(None), Ambito::Ninguno, "{rol:?}");
            assert_eq!(
                rol.ambito(None).puede_ver(Some(1)),
                Visibilidad::NoExiste,
                "{rol:?} vio datos sin tener institución"
            );
        }
    }

    /// El paciente NO ve la institución, aunque su fila tenga `institucion_id`.
    #[test]
    fn el_paciente_no_hereda_la_institucion() {
        assert_eq!(RolUsuario::Paciente.ambito(Some(3)), Ambito::Ninguno);
        assert_eq!(
            RolUsuario::Paciente.ambito(Some(3)).puede_ver(Some(3)),
            Visibilidad::NoExiste,
            "un paciente vio la historia clínica de su institución"
        );
    }

    /// Firmar y borrar son de los tres que el producto deja entrar a /historias.
    /// El resto del personal clínico NO firma: la firma afirma autoría.
    #[test]
    fn solo_medico_y_gestores_firman_y_borran() {
        for rol in [RolUsuario::Superadmin, RolUsuario::Admin, RolUsuario::Medico] {
            assert!(rol.puede(Capacidad::FirmarClinico), "{rol:?} no podía firmar");
            assert!(rol.puede(Capacidad::BorrarClinico), "{rol:?} no podía borrar");
        }
        // La prueba que DISCRIMINA: sin ella, un `puede` que devolviera siempre
        // `true` pasaría la de arriba.
        for rol in [
            RolUsuario::Enfermeria,
            RolUsuario::AuxiliarEnfermeria,
            RolUsuario::RegenteFarmacia,
            RolUsuario::Laboratorista,
            RolUsuario::Radiologo,
            RolUsuario::Anestesiologo,
            RolUsuario::Terapeuta,
            RolUsuario::Nutricionista,
            RolUsuario::Psicologo,
            RolUsuario::TrabajoSocial,
            RolUsuario::Bacteriologo,
            RolUsuario::Paciente,
        ] {
            assert!(!rol.puede(Capacidad::FirmarClinico), "{rol:?} podía FIRMAR una historia");
            assert!(!rol.puede(Capacidad::BorrarClinico), "{rol:?} podía BORRAR una historia");
        }
    }

    #[test]
    fn gestionar_usuarios_e_instituciones_no_es_de_cualquiera() {
        assert!(RolUsuario::Superadmin.puede(Capacidad::GestionarUsuarios));
        assert!(RolUsuario::Admin.puede(Capacidad::GestionarUsuarios));
        assert!(!RolUsuario::Medico.puede(Capacidad::GestionarUsuarios));

        assert!(RolUsuario::Superadmin.puede(Capacidad::GestionarInstituciones));
        // Un admin manda en SU institución, no crea otras ni se fija licencia.
        assert!(!RolUsuario::Admin.puede(Capacidad::GestionarInstituciones));
        assert!(!RolUsuario::Medico.puede(Capacidad::GestionarInstituciones));
    }

    #[test]
    fn un_rol_con_institucion_queda_acotado_a_ella() {
        assert_eq!(
            RolUsuario::Medico.ambito(Some(3)),
            Ambito::Institucion(3)
        );
    }

    /// Los valores de cadena son parte del contrato con la base y los tokens.
    #[test]
    fn los_valores_serializados_no_cambian() {
        let casos = [
            (RolUsuario::Superadmin, "\"superadmin\""),
            (RolUsuario::Admin, "\"admin\""),
            (RolUsuario::Medico, "\"medico\""),
            (RolUsuario::AuxiliarEnfermeria, "\"auxiliar_enfermeria\""),
            (RolUsuario::RegenteFarmacia, "\"regente_farmacia\""),
            (RolUsuario::TrabajoSocial, "\"trabajo_social\""),
        ];
        for (rol, esperado) in casos {
            assert_eq!(serde_json::to_string(&rol).unwrap(), esperado);
        }
    }

    /// `desde_str` invierte la serialización, y lo desconocido da `None` —no un
    /// rol cualquiera—. Se prueba sobre el valor serializado real para que, si
    /// alguna vez cambia el nombre de cadena, esto caiga con el test de arriba.
    #[test]
    fn desde_str_invierte_la_serializacion_y_rechaza_lo_desconocido() {
        for rol in [
            RolUsuario::Superadmin,
            RolUsuario::Admin,
            RolUsuario::Medico,
            RolUsuario::AuxiliarEnfermeria,
            RolUsuario::RegenteFarmacia,
            RolUsuario::TrabajoSocial,
            RolUsuario::Bacteriologo,
        ] {
            let s = serde_json::to_string(&rol).unwrap();
            let s = s.trim_matches('"');
            assert_eq!(RolUsuario::desde_str(s), Some(rol), "{rol:?}");
        }
        assert_eq!(RolUsuario::desde_str("dios"), None);
        assert_eq!(RolUsuario::desde_str(""), None);
        assert_eq!(RolUsuario::desde_str("MEDICO"), None, "no es snake_case");
    }
}
