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
/// # Estas listas son DEFECTOS, y el destino es que las asigne un administrador
///
/// Juan, 2026-07-30: «creo que lo mejor es que un usuario administrador sea quien
/// asigne esos perfiles». De acuerdo, y por una razón que va más allá de la
/// comodidad: la planta de una IPS rural de tres personas y la de un hospital de
/// tercer nivel no se parecen, y cualquier lista fija que se escriba aquí estará
/// mal en una de las dos.
///
/// Pero un sistema de permisos configurable NECESITA defectos igualmente. Si una
/// instalación nueva arranca sin nada asignado, nadie puede hacer nada, y lo
/// primero que hace entonces todo cliente es darle todo a todos — que es peor que
/// donde estábamos. Así que lo de aquí es la base sobre la que se configura, no
/// una alternativa a configurar.
///
/// Lo que falta —y está en la tarea que acompaña a este cambio— es la tabla de
/// asignación por institución que se superpone a esto: un administrador añade o
/// quita roles de una capacidad PARA SU institución, y lo que no toque sigue el
/// defecto. Ningún cliente configura lo que no usa; ninguno se queda sin camino.
///
/// **Lo que sigue sin cubrir**: SUBIR una imagen clínica (`POST
/// /api/historias/{id}/imagenes`). Borrarla sí exige `BorrarClinico`, pero subir
/// la hace el radiólogo, que no está en esa lista, y `/imagenes` no declara roles.
/// Cerrarlo a ojo dejaría al radiólogo sin poder hacer su trabajo.
///
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
    /// **Escribir** en la historia clínica: crearla, modificarla, añadirle
    /// documentos, fórmulas u órdenes.
    ///
    /// Firmar ya estaba cerrado, pero escribir no, y eso dejaba el agujero por el
    /// lado ancho: cualquier usuario autenticado de la institución
    /// —laboratorista, trabajo social, terapeuta— podía **crear una nota clínica
    /// y RECETAR MEDICAMENTOS** (`POST /api/historias/{id}/formulas`). Cerrar solo
    /// la firma protegía la autoría del documento y dejaba abierto su contenido.
    ///
    /// Los tres roles son los que el propio producto ya declara para `/historias`
    /// en `app.routes.ts:42`. No se inventa nada: se aplica en el servidor la
    /// respuesta que hasta ahora solo vivía en el navegador, o sea en ningún sitio.
    EscribirClinico,
    /// Crear o modificar el registro de un paciente y sus antecedentes.
    ///
    /// Conjunto más amplio que el de la historia, y a propósito: enfermería,
    /// auxiliar y psicología SÍ registran y actualizan pacientes aunque no
    /// escriban la nota médica. Son los seis roles que el producto declara para
    /// `/pacientes` en `app.routes.ts:31`.
    EscribirPaciente,
    /// Atender en urgencias y en hospitalización: triage, egreso, ingreso.
    ///
    /// Respuesta de Juan (2026-07-30): «por lo general lo hace el médico general,
    /// el especialista, los enfermeros, todos los profesionales que interactúan
    /// con el paciente». Se toma su enumeración —médico, enfermería, auxiliar— y
    /// se añade anestesiología, que atiende en urgencias y quirófano. NO entran
    /// laboratorio ni bacteriología: interactúan con la MUESTRA, no con el
    /// paciente en la camilla.
    ///
    /// El límite de esta lista es que es un DEFECTO nacional, no la planta de una
    /// institución concreta. Ver la nota sobre asignación por administrador en la
    /// cabecera de [`Capacidad`].
    AtenderUrgenciasHospitalizacion,
    /// Valoraciones y planes de nutrición.
    ///
    /// «El nutriólogo o nutricionista» (Juan). El rol `Medico` entra además, y no
    /// por ampliar por ampliar: en una IPS pequeña sin nutricionista en planta, la
    /// valoración la registra el médico tratante, y cerrarle la puerta dejaría el
    /// módulo inservible justo donde más se nota. Un administrador puede quitarlo
    /// cuando exista la asignación por institución.
    EscribirNutricion,
    /// Planes y sesiones de rehabilitación.
    ///
    /// «El fisioterapeuta» (Juan). En la enumeración de roles del sistema eso es
    /// `Terapeuta`; no hay un `Fisioterapeuta` aparte, y no se inventa uno aquí
    /// —cambiar la lista de roles toca la base y los tokens ya emitidos—. Se añade
    /// `Medico` por lo mismo que en nutrición.
    EscribirRehabilitacion,
    /// Remisiones y contrarreferencia.
    ///
    /// «Puede ser enfermeros o personal administrativo» (Juan). Personal
    /// administrativo es `Admin` en esta enumeración; enfermería y auxiliar
    /// entran, y el médico también porque la remisión sale de su decisión
    /// clínica.
    GestionarRemision,
    /// Registrar y validar resultados de laboratorio, y administrar los perfiles
    /// de examen.
    ///
    /// Los roles del propio producto para `/laboratorio` (`sidebar.component.ts`)
    /// son laboratorista, bacteriólogo, administración y superadmin — y el
    /// médico. Se conserva el médico por el mismo motivo que en nutrición: la IPS
    /// pequeña que manda las muestras a un laboratorio externo y transcribe el
    /// resultado no tiene bacteriólogo en planta, y cerrarle la puerta dejaría el
    /// módulo inservible donde más se usa. Cuando exista la asignación por
    /// institución, un hospital con laboratorio propio se lo quita.
    EscribirLaboratorio,
    /// Redactar y firmar informes de imágenes diagnósticas.
    ///
    /// Radiología, médico y gestión. El informe radiológico es un acto médico
    /// interpretativo: fuera enfermería, laboratorio y terapias, que no lo
    /// emiten.
    EscribirImagenologia,
    /// Farmacia: dar de alta items, registrar entradas de stock y **dispensar**.
    ///
    /// Los tres roles son los que el producto ya declara para `/farmacia`
    /// (`app.routes.ts:51` y el menú de `sidebar.component.ts`): regente de
    /// farmacia, administración y superadmin. No se añade `Medico` —a diferencia
    /// de nutrición y rehabilitación, donde sí se añadió porque en una IPS
    /// pequeña no hay nutricionista en planta—: aquí la separación entre quien
    /// PRESCRIBE y quien DISPENSA no es un detalle de plantilla, es la que
    /// permite que la entrega sea un control sobre la prescripción y no un acto
    /// del mismo que la firmó. Un consultorio sin regente no dispensa: remite a
    /// una farmacia.
    ///
    /// Leer el inventario es cosa aparte y NO exige esta capacidad: enfermería
    /// necesita saber si queda adrenalina antes de pedirla.
    GestionarFarmacia,
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
            // Los mismos tres, y por el mismo `app.routes.ts:42`. Firmar sin
            // poder escribir no tendría sentido, y escribir sin poder firmar es
            // exactamente lo que había: la autoría cerrada y el contenido
            // abierto.
            EscribirClinico => matches!(self, Superadmin | Admin | Medico),
            // Seis, no tres (`app.routes.ts:31`). Enfermería, auxiliar y
            // psicología registran pacientes y actualizan antecedentes sin
            // escribir la nota médica; cerrarles esto sería el fallo con el signo
            // cambiado.
            EscribirPaciente => matches!(
                self,
                Superadmin | Admin | Medico | Enfermeria | AuxiliarEnfermeria | Psicologo
            ),
            // Quien atiende al paciente en la camilla. Fuera laboratorio,
            // radiología y bacteriología: interactúan con la muestra o la imagen.
            AtenderUrgenciasHospitalizacion => matches!(
                self,
                Superadmin | Admin | Medico | Enfermeria | AuxiliarEnfermeria | Anestesiologo
            ),
            EscribirNutricion => matches!(self, Superadmin | Admin | Medico | Nutricionista),
            EscribirRehabilitacion => matches!(self, Superadmin | Admin | Medico | Terapeuta),
            GestionarRemision => matches!(
                self,
                Superadmin | Admin | Medico | Enfermeria | AuxiliarEnfermeria
            ),
            EscribirLaboratorio => matches!(
                self,
                Superadmin | Admin | Medico | Laboratorista | Bacteriologo
            ),
            EscribirImagenologia => matches!(self, Superadmin | Admin | Medico | Radiologo),
            GestionarFarmacia => matches!(self, Superadmin | Admin | RegenteFarmacia),
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
