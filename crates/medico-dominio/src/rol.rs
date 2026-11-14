//! Los roles del sistema, con los mismos valores de cadena que `models.py:52`.
//!
//! No se renombran ni se agrupan: esos valores están escritos en la base y en
//! los tokens ya emitidos. Un rol «mejor nombrado» invalidaría sesiones y
//! filas existentes a cambio de nada.

use serde::{Deserialize, Serialize};

use crate::Ambito;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    /// **Escribir el registro de ENFERMERÍA**: la nota de enfermería, el plan de
    /// cuidados y la administración de medicamentos (#249, bloque hospitalario).
    ///
    /// Nace como capacidad PROPIA, no como un hueco en `EscribirClinico`, y ese
    /// es todo el punto del módulo. `EscribirClinico` son `Superadmin | Admin |
    /// Medico`, así que **una enfermera no podía escribir una nota de
    /// enfermería** — y las notas de enfermería son parte integral de la
    /// historia clínica, no un anexo. Ensanchar `EscribirClinico` para dejarlas
    /// pasar habría abierto de paso la epicrisis y la fórmula a todo el que
    /// cuida, que es el fallo con el signo cambiado (directiva 21).
    ///
    /// Es la misma forma que `EscribirNutricion` y `EscribirRehabilitacion`: una
    /// disciplina que documenta SU cuidado, con su propia puerta. `Medico` entra
    /// por lo mismo que en aquéllas — el médico que escribe en el turno de noche
    /// no tiene que pedirle el teclado a nadie.
    EscribirEnfermeria,
    /// **Escribir la nota de TRABAJO SOCIAL** (#249).
    ///
    /// La cuarta disciplina con puerta propia, con la forma de
    /// `EscribirNutricion`, `EscribirRehabilitacion` y `EscribirEnfermeria`: una
    /// profesión que documenta SU intervención y que no escribe la nota médica.
    /// `Psicologo` entra porque en la mayoría de las IPS la valoración
    /// psicosocial la comparten los dos; `Medico` por lo mismo que en las otras.
    EscribirTrabajoSocial,
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
    /// Facturar: contratos, facturas, sus líneas y las glosas.
    ///
    /// Es capacidad ADMINISTRATIVA y no clínica, y por eso la lista es corta.
    /// No entra ningún rol clínico: el médico que atiende no radica ante la EPS,
    /// y darle la capacidad «porque total, ya ve al paciente» mezcla dos cosas
    /// que en una IPS hacen personas distintas y responden ante auditorías
    /// distintas.
    ///
    /// **No hay rol `Facturador` en el producto**, así que el defecto nacional
    /// es `Admin` (y `Superadmin`, que lo es todo). En el consultorio de un solo
    /// médico eso es el propio dueño con su cuenta de administrador, que es como
    /// funciona hoy en el sistema vivo. Vale el mismo límite que el resto de
    /// esta enumeración: es un DEFECTO, no la planta de una institución
    /// concreta.
    ///
    /// LEER no exige esta capacidad, igual que en farmacia: quien consulta el
    /// estado de una factura para responder al paciente en ventanilla no tiene
    /// por qué poder radicarla.
    GestionarFacturacion,
    /// Parque de ambulancias y despacho de traslados asistenciales.
    ///
    /// Une dos cosas que en un hospital grande hacen personas distintas —dar de
    /// alta un vehículo es administrativo; despachar es operativo— y aquí van
    /// juntas a propósito: en la IPS que tiene dos ambulancias las hace la misma
    /// persona, y partirlo en dos capacidades obligaría a configurar una
    /// distinción que ese cliente no usa. Cuando exista la asignación por
    /// institución, el hospital que sí las separa lo hará ahí.
    ///
    /// Quién: gestión, médico, enfermería y auxiliar — los que coordinan un
    /// traslado. Fuera laboratorio, imagenología, farmacia y terapias.
    GestionarAmbulancias,
    /// Definir la agenda de un profesional: sus meses abiertos, las franjas que
    /// atiende cada día y los ratos que bloquea (vacaciones, incapacidad).
    ///
    /// Quién: gestión y el propio médico. Declarar a qué horas atiende un
    /// profesional es una decisión de él y de la administración; que la tome
    /// cualquiera con cuenta significa que a un médico le pueden cerrar la
    /// consulta del jueves sin que se entere.
    ///
    /// **LEER la agenda NO exige esto, y es deliberado.** Quien agenda una cita
    /// —recepción, enfermería, el auxiliar que atiende el teléfono— tiene que
    /// ver los turnos libres para poder darlos, y cerrarle esa consulta dejaría
    /// el módulo de citas inservible justo en el mostrador. Se cierra escribir,
    /// no mirar.
    GestionarAgenda,
    /// Generar el RIPS de un periodo: el archivo con el que se factura.
    ///
    /// Son los dos roles que el Python ya exigía (`require_role(ADMIN,
    /// SUPERADMIN)` en `routers/rips.py`), y no se amplía el defecto al portar:
    /// el archivo lleva las atenciones de TODOS los pacientes de la institución
    /// y de él depende el pago.
    ///
    /// El prestador que es una sola persona —un «profesional independiente», que
    /// es el tipo de cuenta más común ante el MUV— se lo concede a `Medico` en
    /// su matriz. Por eso el defecto puede ser estrecho sin dejar a nadie fuera:
    /// hay camino, y lo abre quien conoce su planta.
    GenerarRips,
    /// Administrar la institución: su ficha, su configuración de operación, su
    /// política de seguridad, los campos dinámicos de la historia y los maestros
    /// que el prestador edita.
    ///
    /// **No se inventa**: es la matriz que el producto ya declara para `/admin`
    /// en `app.routes.ts:80` — `['admin','superadmin']`. Va como capacidad
    /// propia y no reutilizando `GestionarUsuarios` porque son dos permisos
    /// distintos con el mismo conjunto HOY: el día que una IPS quiera que su
    /// coordinador toque los horarios sin poder crear cuentas, la separación ya
    /// está hecha.
    AdministrarInstitucion,
    /// Crear y listar usuarios de la institución.
    GestionarUsuarios,
    /// Crear instituciones y fijarles licencia.
    GestionarInstituciones,
}

impl Capacidad {
    /// Todas las capacidades, para pintar la matriz y para recorrerlas al
    /// validar. Escrita a mano y no derivada: si mañana se añade una variante,
    /// `TODAS` se queda corta en silencio, y por eso existe
    /// `todas_las_capacidades_estan_en_la_lista`, que compara contra un `match`
    /// exhaustivo — el compilador obliga a tocarlo.
    pub const TODAS: &'static [Capacidad] = &[
        Capacidad::FirmarClinico,
        Capacidad::BorrarClinico,
        Capacidad::EscribirClinico,
        Capacidad::EscribirPaciente,
        Capacidad::AtenderUrgenciasHospitalizacion,
        Capacidad::EscribirNutricion,
        Capacidad::EscribirRehabilitacion,
        Capacidad::EscribirEnfermeria,
        Capacidad::EscribirTrabajoSocial,
        Capacidad::GestionarRemision,
        Capacidad::EscribirLaboratorio,
        Capacidad::EscribirImagenologia,
        Capacidad::GestionarFarmacia,
        Capacidad::GestionarFacturacion,
        Capacidad::GestionarAmbulancias,
        Capacidad::GestionarAgenda,
        Capacidad::GenerarRips,
        Capacidad::AdministrarInstitucion,
        Capacidad::GestionarUsuarios,
        Capacidad::GestionarInstituciones,
    ];

    /// De la cadena guardada en la base a la capacidad. Mismo `Deserialize`
    /// (snake_case) con el que se serializa: una sola lista de nombres, que no
    /// se puede desincronizar. `None` si no nombra ninguna conocida.
    pub fn desde_str(s: &str) -> Option<Self> {
        use serde::de::IntoDeserializer;
        let de: serde::de::value::StrDeserializer<serde::de::value::Error> = s.into_deserializer();
        <Self as serde::Deserialize>::deserialize(de).ok()
    }

    /// El nombre con el que viaja a la columna `capacidad` de la base.
    ///
    /// Es un `match` y no `serde_json::to_value` porque hace falta un
    /// `&'static str` y aquello devuelve un `String` propio. Eso deja DOS
    /// listas de nombres, que es justo lo que se quiere evitar, así que
    /// `el_nombre_de_la_base_y_el_de_serde_son_el_mismo` las ata: recorre
    /// `TODAS` y exige que coincidan una por una.
    pub fn como_str(&self) -> &'static str {
        match self {
            Capacidad::FirmarClinico => "firmar_clinico",
            Capacidad::BorrarClinico => "borrar_clinico",
            Capacidad::EscribirClinico => "escribir_clinico",
            Capacidad::EscribirPaciente => "escribir_paciente",
            Capacidad::AtenderUrgenciasHospitalizacion => "atender_urgencias_hospitalizacion",
            Capacidad::EscribirNutricion => "escribir_nutricion",
            Capacidad::EscribirRehabilitacion => "escribir_rehabilitacion",
            Capacidad::EscribirEnfermeria => "escribir_enfermeria",
            Capacidad::EscribirTrabajoSocial => "escribir_trabajo_social",
            Capacidad::GestionarRemision => "gestionar_remision",
            Capacidad::EscribirLaboratorio => "escribir_laboratorio",
            Capacidad::EscribirImagenologia => "escribir_imagenologia",
            Capacidad::GestionarFarmacia => "gestionar_farmacia",
            Capacidad::GestionarFacturacion => "gestionar_facturacion",
            Capacidad::GestionarAmbulancias => "gestionar_ambulancias",
            Capacidad::GestionarAgenda => "gestionar_agenda",
            Capacidad::GenerarRips => "generar_rips",
            Capacidad::AdministrarInstitucion => "administrar_institucion",
            Capacidad::GestionarUsuarios => "gestionar_usuarios",
            Capacidad::GestionarInstituciones => "gestionar_instituciones",
        }
    }

    /// ¿Puede un administrador de institución reconfigurar quién tiene esto?
    ///
    /// `GestionarInstituciones` NO: crea instituciones y les fija la licencia,
    /// o sea que vive por encima de la institución. Dejar que el administrador
    /// de una se lo concediera a un rol suyo sería una escalada, no una
    /// configuración — se estaría dando la llave del piso de arriba.
    ///
    /// Todo lo demás sí, incluida `GestionarUsuarios`: quién administra la
    /// planta es justo la clase de decisión que cambia entre una IPS de tres
    /// personas y un hospital. Lo que se le impide es dejarla SIN NADIE, que es
    /// lo irreparable; eso lo comprueba [`Ajustes::validar`].
    pub fn configurable_por_institucion(&self) -> bool {
        !matches!(self, Capacidad::GestionarInstituciones)
    }
}

impl RolUsuario {
    /// Todos los roles, para pintar la matriz.
    pub const TODOS: &'static [RolUsuario] = &[
        RolUsuario::Superadmin,
        RolUsuario::Admin,
        RolUsuario::Medico,
        RolUsuario::Paciente,
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
    ];

    /// ¿Puede la matriz de una institución decir algo sobre este rol?
    ///
    /// **`Superadmin` no**: no pertenece a ninguna institución
    /// (`institucion_id = NULL`), así que una fila suya en una matriz
    /// per-institución no se aplicaría nunca. Aceptarla y no usarla sería
    /// exactamente lo que la directiva 21 prohíbe —guardar algo que dice una
    /// cosa y significa otra—, y además dejaría al administrador creyendo que
    /// ha restringido al superadmin. Se rechaza al escribir.
    ///
    /// **`Paciente` tampoco**: su ámbito es `Ninguno` mientras no exista el
    /// portal, de modo que concederle una capacidad no le abriría nada y
    /// prometería lo contrario. Cuando el portal exista, esto se revisa junto
    /// con `Ambito::Paciente`.
    pub fn configurable_por_institucion(&self) -> bool {
        !matches!(self, RolUsuario::Superadmin | RolUsuario::Paciente)
    }
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
            // Quien cuida en la cama. Los mismos que `AtenderUrgenciasHospitalizacion`
            // **menos anestesiología**, que atiende en quirófano y no lleva el
            // registro de enfermería de la planta.
            EscribirEnfermeria => matches!(
                self,
                Superadmin | Admin | Medico | Enfermeria | AuxiliarEnfermeria
            ),
            EscribirTrabajoSocial => matches!(self, Superadmin | Admin | Medico | Psicologo),
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
            GestionarFacturacion => matches!(self, Superadmin | Admin),
            GestionarAmbulancias => matches!(
                self,
                Superadmin | Admin | Medico | Enfermeria | AuxiliarEnfermeria
            ),
            // Gestión y el propio profesional. Enfermería y auxiliar quedan
            // fuera del DEFECTO —no del sistema—: la IPS que quiera que su
            // auxiliar arme las agendas se lo concede en su matriz.
            GestionarAgenda => matches!(self, Superadmin | Admin | Medico),
            // Ya se comprobaba a mano en las rutas; aquí queda dicho una vez.
            GenerarRips => matches!(self, Superadmin | Admin),
            AdministrarInstitucion => matches!(self, Superadmin | Admin),
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

    /// El nombre con el que el rol viaja a la base y a la pantalla.
    ///
    /// Mismo motivo que en [`Capacidad::como_str`]: hace falta un `&'static
    /// str` y el `Serialize` da un `String` propio. Que las dos formas digan lo
    /// mismo lo ata `el_nombre_de_la_base_y_el_de_serde_son_el_mismo`.
    pub fn como_str(&self) -> &'static str {
        match self {
            RolUsuario::Superadmin => "superadmin",
            RolUsuario::Admin => "admin",
            RolUsuario::Medico => "medico",
            RolUsuario::Paciente => "paciente",
            RolUsuario::Enfermeria => "enfermeria",
            RolUsuario::AuxiliarEnfermeria => "auxiliar_enfermeria",
            RolUsuario::RegenteFarmacia => "regente_farmacia",
            RolUsuario::Laboratorista => "laboratorista",
            RolUsuario::Radiologo => "radiologo",
            RolUsuario::Anestesiologo => "anestesiologo",
            RolUsuario::Terapeuta => "terapeuta",
            RolUsuario::Nutricionista => "nutricionista",
            RolUsuario::Psicologo => "psicologo",
            RolUsuario::TrabajoSocial => "trabajo_social",
            RolUsuario::Bacteriologo => "bacteriologo",
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Las listas escritas a mano —`TODAS`, `TODOS` y los dos `como_str`— son
    /// justo el sitio donde una variante nueva se olvida en silencio. Estas
    /// cuatro pruebas las atan al `match` exhaustivo y al `serde`, que son lo
    /// que el compilador sí vigila.
    #[test]
    fn todas_las_capacidades_estan_en_la_lista() {
        for c in Capacidad::TODAS {
            // Un `match` exhaustivo: añadir una variante rompe la compilación
            // aquí, y entonces hay que tocar TODAS.
            let _: () = match c {
                Capacidad::FirmarClinico
                | Capacidad::BorrarClinico
                | Capacidad::EscribirClinico
                | Capacidad::EscribirPaciente
                | Capacidad::AtenderUrgenciasHospitalizacion
                | Capacidad::EscribirNutricion
                | Capacidad::EscribirRehabilitacion
                | Capacidad::EscribirEnfermeria
                | Capacidad::EscribirTrabajoSocial
                | Capacidad::GestionarRemision
                | Capacidad::EscribirLaboratorio
                | Capacidad::EscribirImagenologia
                | Capacidad::GestionarFarmacia
                | Capacidad::GestionarFacturacion
                | Capacidad::GestionarAmbulancias
                | Capacidad::GestionarAgenda
                | Capacidad::GenerarRips
                | Capacidad::AdministrarInstitucion
                | Capacidad::GestionarUsuarios
                | Capacidad::GestionarInstituciones => (),
            };
        }
        assert_eq!(Capacidad::TODAS.len(), 20, "¿se añadió una capacidad?");
        let unicas: std::collections::HashSet<_> = Capacidad::TODAS.iter().collect();
        assert_eq!(unicas.len(), Capacidad::TODAS.len(), "hay una repetida");
    }

    #[test]
    fn todos_los_roles_estan_en_la_lista() {
        for r in RolUsuario::TODOS {
            let _: () = match r {
                RolUsuario::Superadmin
                | RolUsuario::Admin
                | RolUsuario::Medico
                | RolUsuario::Paciente
                | RolUsuario::Enfermeria
                | RolUsuario::AuxiliarEnfermeria
                | RolUsuario::RegenteFarmacia
                | RolUsuario::Laboratorista
                | RolUsuario::Radiologo
                | RolUsuario::Anestesiologo
                | RolUsuario::Terapeuta
                | RolUsuario::Nutricionista
                | RolUsuario::Psicologo
                | RolUsuario::TrabajoSocial
                | RolUsuario::Bacteriologo => (),
            };
        }
        assert_eq!(RolUsuario::TODOS.len(), 15, "¿se añadió un rol?");
        let unicas: std::collections::HashSet<_> = RolUsuario::TODOS.iter().collect();
        assert_eq!(unicas.len(), RolUsuario::TODOS.len(), "hay uno repetido");
    }

    /// Que el nombre de la base y el de serde sean el MISMO, en los dos
    /// sentidos. Si divergieran, una matriz guardada dejaría de leerse y todo
    /// caería al defecto sin ruido: la peor forma de fallar aquí.
    #[test]
    fn el_nombre_de_la_base_y_el_de_serde_son_el_mismo() {
        for c in Capacidad::TODAS {
            let por_serde = serde_json::to_string(c).unwrap();
            assert_eq!(por_serde, format!("\"{}\"", c.como_str()), "capacidad {c:?}");
            assert_eq!(Capacidad::desde_str(c.como_str()), Some(*c));
        }
        for r in RolUsuario::TODOS {
            let por_serde = serde_json::to_string(r).unwrap();
            assert_eq!(por_serde, format!("\"{}\"", r.como_str()), "rol {r:?}");
            assert_eq!(RolUsuario::desde_str(r.como_str()), Some(*r));
        }
    }
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
        // Administrar la institución: la matriz que el producto ya declara para
        // `/admin` (`app.routes.ts:80`). Los dos mismos, y NADIE más — un médico
        // cambiando la política de contraseñas de la clínica no es un caso
        // inusual, es uno que no existe.
        assert!(RolUsuario::Superadmin.puede(Capacidad::AdministrarInstitucion));
        assert!(RolUsuario::Admin.puede(Capacidad::AdministrarInstitucion));
        for r in [
            RolUsuario::Medico,
            RolUsuario::Enfermeria,
            RolUsuario::Laboratorista,
            RolUsuario::Anestesiologo,
            RolUsuario::TrabajoSocial,
        ] {
            assert!(!r.puede(Capacidad::AdministrarInstitucion), "{r:?} administraba la IPS");
        }

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
