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

impl RolUsuario {
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
