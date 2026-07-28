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
}
