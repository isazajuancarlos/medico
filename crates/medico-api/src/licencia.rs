// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! La licencia por módulo, **aplicada en el servidor** (#534).
//!
//! Hasta el 2026-08-07 `modulos_habilitados` solo alimentaba `GET /api/modulos`,
//! que arma el menú. O sea que las rutas de los 23 módulos construidos
//! respondían a cualquier usuario autenticado de la institución **aunque no
//! hubiera licenciado el módulo**: bastaba escribir la URL. Y el frontal
//! AFIRMABA lo contrario — el docstring de `moduloGuard` dice «el backend ya
//! responde 403», que era falso.
//!
//! Lo señalaron por separado las dos revisiones independientes de la rama de
//! estadísticas (2026-08-06).
//!
//! # Por qué importa, y no es solo comercial
//!
//! La ficha de la tarea lo clasificó como deuda comercial —«los datos que
//! devuelve son los del propio prestador, no es un fallo de aislamiento»—, y esa
//! mitad es cierta. La otra no: es un **control de acceso declarado que el
//! servidor no ejerce**. El menú promete una puerta que no tiene cerradura, el
//! frontal documenta un 403 que nadie emite, y la única barrera contra
//! `/api/estadisticas/financiero` —la facturación del prestador— era que el
//! usuario no adivinara la URL. Un permiso que solo vive en el navegador no es
//! un permiso.
//!
//! Y sí tiene lado comercial, que es el que paga: el catálogo de módulos es la
//! pieza que permite vender el MISMO binario a un consultorio, una IPS y un
//! hospital cambiando solo la licencia.
//!
//! # Por qué una CAPA y no un extractor por manejador
//!
//! La ficha proponía `ModuloHabilitado<X>` al lado de `Autorizado<X>`. Se
//! descartó por dos razones medidas:
//!
//! 1. **Son 193 endpoints.** Un extractor hay que escribirlo en cada firma, y el
//!    que se escribe a mano es el que se olvida en la ruta siguiente — que es
//!    textualmente el argumento con el que nació `Autorizado`. Con una capa, una
//!    ruta nueva bajo `/api/nutricion/…` queda cubierta en el momento en que se
//!    registra, sin que nadie se acuerde.
//! 2. **El backend Python ya había tomado esta decisión**, y por lo mismo:
//!    `requiere_modulo(...)` se pone a nivel de ROUTER, en `dependencies=[…]`,
//!    «no endpoint por endpoint: así no se olvida en uno».
//!
//! La contrapartida honesta: lo que la capa protege no se lee en la firma del
//! manejador, que sí era la virtud de `Autorizado`. Lo compensa
//! [`pruebas::ninguna_ruta_de_modulo_se_queda_sin_licencia`], que dice ruta por
//! ruta cuál cae bajo qué módulo y obliga a declarar por escrito las que no.
//!
//! # De dónde sale «qué módulo es esta ruta»
//!
//! Del `prefijo` que el catálogo ya declara ([`medico_dominio::modulo`]), no de
//! una segunda lista escrita a mano. Ese dato ya existía y ya estaba vigilado
//! por `el_catalogo_no_miente_sobre_lo_que_el_binario_sirve`: reusarlo es la
//! directiva 1 —no se añade un mecanismo para sostener lo que hace otro— y
//! además hace que declarar un módulo nuevo lo licencie y lo anuncie de una vez.
//!
//! # Los tres estados siguen distinguiéndose, y esa es la prueba del diseño
//!
//! | estado | qué pasa | quién lo produce |
//! |---|---|---|
//! | `Habilitado` | la respuesta del manejador | la ruta existe y la licencia la cubre |
//! | `Pendiente` | **404** | licenciado pero sin construir: no hay ruta que servir |
//! | `NoIncluido` | **403** | esta capa |
//!
//! El 404 del pendiente sale solo, sin código: un módulo sin construir no tiene
//! ruta registrada. No hacía falta inventarle nada.

use axum::extract::{MatchedPath, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use medico_dominio::modulo;
use medico_dominio::rol::Capacidad;
use medico_dominio::Ambito;
use serde::Serialize;

use crate::auth::Estado;

/// Rutas cuyo prefijo MIENTE sobre a qué módulo pertenecen.
///
/// Es una lista corta y tiene que seguir siéndolo: cada línea es un sitio donde
/// la convención no se cumple, y se declara con su motivo porque el fallo que
/// evita no se ve.
///
/// **El caso real, y es exactamente el sesgo a cerrar de la directiva 21**:
/// `/api/imagenes/{id}` y `/api/imagenes/{id}/archivo` NO son del módulo de
/// imágenes diagnósticas — son los **adjuntos de la historia clínica**, que se
/// suben desde `/api/historias/{id}/imagenes` y conviven con las rutas del
/// módulo porque los segmentos literales (`ordenes`, `paciente`) tienen
/// prioridad sobre el parámetro. Metiéndolos bajo `IMAGENES`, un consultorio que
/// no compró imagenología dejaría de poder ver los adjuntos de sus propias
/// historias: la licencia habría roto la historia clínica.
///
/// Se casa contra el patrón de ruta REGISTRADO (`MatchedPath`), no contra la URL
/// concreta: `/api/imagenes/{id}/archivo` es una cadena exacta que el guardián
/// de fuente puede cruzar con el router, mientras que `/api/imagenes/42/archivo`
/// habría que reconocerlo adivinando si `42` es un id o un segmento literal.
///
/// **Las tres últimas las encontró este mismo guardián en su primera corrida**,
/// y son la misma forma que el adjunto: se CREAN y se LISTAN bajo
/// `/api/historias/{id}/…` —que es del módulo `HISTORIA`— y se editan o se
/// borran por una ruta hermana PLANA, que ya no lleva el prefijo de la historia.
/// Sin decidirlas, la fórmula y la orden se habrían servido sin puerta mientras
/// su listado sí la tenía; decidiéndolas mal, un consultorio sin imagenología ni
/// laboratorio no podría corregir una fórmula de su propia historia clínica.
///
/// Que la lista creciera sola de dos a cinco en la primera ejecución es el
/// argumento de por qué esto es un guardián y no una lectura: las tres estaban
/// a la vista en el router y no las vio nadie.
///
/// # Lo que esta excepción concede de más, dicho a propósito
///
/// Lo midió la revisión de seguridad de este cambio, y no se tapa porque es una
/// decisión comercial, no un descuido:
///
/// - **`/api/imagenes/{id}/archivo` sirve también los BYTES de la imagen que
///   acompaña a un informe radiológico**, porque `img_subir` los guarda por el
///   mismo almacén y con la misma URL que el adjunto de la historia. O sea que
///   la licencia de `IMAGENES` protege el TEXTO del informe
///   (`/api/imagenes/ordenes/{id}/informe`) y no las placas. No es fuga entre
///   instituciones —`imagen::obtener` pasa por `imagen_visible`—: es que ese
///   módulo se vende con una puerta a medias.
/// - **`/api/ordenes/{id}/estado` deja marcar «Completada» una orden de
///   laboratorio sin tener `LABORATORIO`**, mientras
///   `/api/laboratorio/ordenes/{id}/estado` sí lo exige. La excepción hace falta
///   para que la pantalla de la historia funcione en un consultorio que no
///   compró laboratorio; el precio es que la ruta hermana plana concede lo que
///   la del módulo niega.
///
/// Separarlas de verdad exige mirar el RECURSO y no la ruta —de qué servicio es
/// la orden, de qué informe es la imagen—, que es un cambio de otro tamaño. Va
/// escrito aquí para que sea una decisión y no un hallazgo futuro.
const LA_RUTA_MIENTE_SOBRE_SU_MODULO: &[(&str, &str)] = &[
    ("/api/imagenes/{id}", "HISTORIA"),
    ("/api/imagenes/{id}/archivo", "HISTORIA"),
    ("/api/formulas/{id}", "HISTORIA"),
    ("/api/ordenes/{id}", "HISTORIA"),
    // El cambio de estado GENÉRICO, el que usa la pantalla de la historia.
    // Laboratorio e imagenología tienen el suyo propio bajo su prefijo
    // (`/api/laboratorio/ordenes/{id}/estado`), y ésos sí van con su módulo.
    ("/api/ordenes/{id}/estado", "HISTORIA"),
];

/// Rutas que el catálogo no reclama y que están FUERA del licenciamiento **por
/// decisión**, cada una con su motivo.
///
/// Existe para que el guardián de fuente pueda exigir que toda ruta registrada
/// esté decidida: o la reclama un módulo, o alguien escribió aquí por qué no.
/// Sin esta lista, «no la reclama nadie» y «se nos olvidó» se ven igual — que es
/// la directiva 30 aplicada a la propia salvaguarda.
///
/// Se declaran por PREFIJO de patrón registrado; el guardián comprueba que cada
/// entrada cubra al menos una ruta real, para que una línea que se quedó
/// obsoleta no siga tapando un hueco que ya no existe.
///
/// Va bajo `#[cfg(test)]` porque **en ejecución no decide nada**: la capa deja
/// pasar lo que ningún módulo reclama, y esta lista solo existe para que el
/// guardián pueda exigir que esa decisión esté ESCRITA. Compilarla en el binario
/// sería llevarse texto que nadie lee.
#[cfg(test)]
const FUERA_DEL_LICENCIAMIENTO: &[(&str, &str)] = &[
    // --- Sesión ------------------------------------------------------------
    ("/api/acceso", "entrar y verificar la cadena de accesos: sin sesión no hay institución a la que preguntarle la licencia"),
    ("/api/salir", "salir de la sesión propia"),
    ("/api/salir-todo", "cerrar las demás sesiones propias"),
    ("/api/mi-clave", "cambiar la contraseña propia; no administra a nadie"),
    ("/api/2fa/verificar", "el SEGUNDO PASO de entrar: quien lo llama todavía no tiene sesión, así que no hay institución a la que preguntarle la licencia — igual que `/api/acceso`. Lo que le da acceso es el token pendiente, no una capacidad"),
    ("/api/usuarios/{id}/2fa", "el rescate de quien perdió el teléfono Y sus códigos. Es administración, como el resto de `/api/usuarios`, y exige `GestionarUsuarios`. La seguridad de la cuenta no se vende por módulos: cobrarla dejaría a un cliente sin poder rescatar a su propio personal"),
    ("/api/yo", "quién soy: lo pide el frontal antes de pintar nada"),
    ("/api/demo", "las credenciales que publica una demostración; se responde SIN sesión"),
    ("/api/modulos", "EL MENÚ. Licenciarlo sería la puerta que se cierra con la llave dentro: el cliente no podría ni saber qué compró"),
    // --- Administración: no es un módulo, es la consola del producto -------
    ("/api/admin", "administrar la institución. Ya exige `AdministrarInstitucion`/`GestionarUsuarios`, y NO se vende por módulos: un cliente sin ella no podría dar de alta a nadie. La excepción es `/api/admin/auditoria`, que SÍ es el módulo AUDITORIA y el catálogo lo reclama"),
    ("/api/instituciones", "crear instituciones y fijarles licencia: es la ruta del VENDEDOR. Licenciarla haría que la licencia se exigiera a sí misma"),
    ("/api/usuarios", "dar de alta al personal y corregirle el documento; exige `GestionarUsuarios`"),
    ("/api/capacidades", "la matriz de quién puede qué; exige `GestionarUsuarios`"),
    ("/api/medicos", "el listado de profesionales, que alimenta los selectores de media aplicación"),
    // --- Agenda ------------------------------------------------------------
    ("/api/agenda", "NO hay módulo de agenda en el catálogo, y añadir uno cambia lo que se vende: es decisión comercial de Juan, no de este archivo. Mientras no exista, queda fuera y dicho"),
    // --- Catálogos de referencia ------------------------------------------
    ("/api/municipios", "DIVIPOLA: catálogo nacional, lo necesita cualquier formulario"),
    ("/api/especialidades", "catálogo de especialidades"),
    ("/api/medicamentos", "búsqueda en el CUM del INVIMA: catálogo nacional"),
    ("/api/cups", "búsqueda en la Clasificación Única de Procedimientos en Salud: catálogo nacional, igual que el CUM. Lo necesita la captura de la historia, que es del módulo HISTORIA"),
    ("/api/unidades-dispensacion", "maestro del MUV"),
    ("/api/finalidades", "tabla de la 948 que la historia ofrece para que el médico ELIJA"),
    ("/api/causas-motivo", "tabla de la 948, igual que la anterior"),
    ("/api/tipos-documento-profesional", "maestro del MUV para el documento de quien atiende"),
];

/// **Las pantallas del panel en `maud`, y de qué módulo es cada una.**
///
/// Una sola tabla con DOS usos, y ése es el punto (directiva 1):
///
/// 1. [`crate::vista::panel::menu`] construye el menú con ella;
/// 2. [`exige`] la usa para cerrar la puerta.
///
/// Con dos listas —una para pintar y otra para vigilar— pasaría exactamente lo
/// que #534 arregló: el menú diciendo una cosa y el servidor haciendo otra. Aquí
/// **no pueden divergir porque son la misma**.
///
/// `/panel/pacientes` es tan ruta de módulo como `/api/pacientes`, y el guardián
/// de fuente lo exige igual: una pantalla nueva sin entrada aquí no compila la
/// prueba en verde. Sin esto, migrar una pantalla a `maud` la habría sacado de
/// la licencia sin que nadie se enterara — el frontal nuevo estrenando el mismo
/// agujero que el viejo.
/// Una pantalla del panel: de qué módulo es, dónde vive, cómo se llama en el
/// menú y **qué capacidad exige además de la sesión**.
///
/// La cuarta columna llegó con `/panel/rips`, que es la primera pantalla que
/// pide algo más que estar dentro: `GET /api/rips` exige `GenerarRips`, así que
/// la pantalla exige lo mismo (ver [`crate::vista::EnSesionCon`]). Es un dato y
/// no un `if` en el manejador porque **el menú también lo necesita**: sin él se
/// ofrece una entrada que lleva a «esta sección no le corresponde», y el sidebar
/// de Angular que se está migrando sí la escondía.
pub struct Pantalla {
    pub modulo: &'static str,
    pub ruta: &'static str,
    pub etiqueta: &'static str,
    /// `None` = basta con la sesión, que es el caso de las pantallas de
    /// consulta. Se escribe la capacidad y no el rol: los roles que la tienen
    /// los decide la matriz de cada institución, y aquí una lista de roles sería
    /// la segunda lista que este archivo existe para no tener.
    pub exige: Option<Capacidad>,
}

pub const PANTALLAS: &[Pantalla] = &[
    Pantalla { modulo: "PACIENTES", ruta: "/panel/pacientes", etiqueta: "Pacientes", exige: None },
    // Una sola fila cubre las quince rutas de la historia: `exige` casa por
    // prefijo. Y va con `exige: None` a propósito —leer una historia solo pide
    // sesión, igual que `GET /api/historias/{id}`—; las escrituras declaran
    // `EscribirClinico`, `EscribirPaciente`, `FirmarClinico` o `BorrarClinico`
    // en su propio manejador, que son cuatro capacidades distintas dentro de la
    // misma pantalla y ninguna vale como puerta de entrada.
    Pantalla { modulo: "HISTORIA", ruta: "/panel/historias", etiqueta: "Historias clínicas", exige: None },
    Pantalla { modulo: "CITAS", ruta: "/panel/citas", etiqueta: "Citas", exige: None },
    Pantalla { modulo: "INCAPACIDADES", ruta: "/panel/incapacidades", etiqueta: "Incapacidades", exige: None },
    Pantalla { modulo: "REMISIONES", ruta: "/panel/remisiones", etiqueta: "Remisiones", exige: None },
    Pantalla { modulo: "CONTRARREFERENCIA", ruta: "/panel/contrarreferencia", etiqueta: "Contrarreferencia", exige: None },
    Pantalla { modulo: "CONSENTIMIENTO", ruta: "/panel/consentimiento", etiqueta: "Consentimientos", exige: None },
    Pantalla { modulo: "URGENCIAS", ruta: "/panel/urgencias", etiqueta: "Urgencias", exige: None },
    Pantalla { modulo: "EVOLUCIONES", ruta: "/panel/evoluciones", etiqueta: "Evoluciones", exige: None },
    Pantalla { modulo: "EPICRISIS", ruta: "/panel/epicrisis", etiqueta: "Epicrisis", exige: None },
    Pantalla { modulo: "INTERCONSULTAS", ruta: "/panel/interconsultas", etiqueta: "Interconsultas", exige: None },
    Pantalla { modulo: "NOTA_OPERATORIA", ruta: "/panel/notas-operatorias", etiqueta: "Notas operatorias", exige: None },
    Pantalla { modulo: "HOSPITALIZACION", ruta: "/panel/hospitalizacion", etiqueta: "Hospitalización", exige: None },
    // LEER la lista no exige capacidad, igual que la cola de urgencias: quien
    // cierra el mes tiene que poder comprobar qué va a salir en el RIPS.
    // REGISTRAR sí, y lo exige su propio manejador (`AtenderUrgencias`).
    Pantalla { modulo: "REGISTRO_NACIMIENTO", ruta: "/panel/nacimientos", etiqueta: "Registro de nacimiento", exige: None },
    // LEER la lista no exige capacidad; EXPEDIR sí, y es `FirmarClinico` — el
    // certificado lo firma un médico con su nombre, y alimenta el registro civil.
    Pantalla { modulo: "CERTIFICADO_DEFUNCION", ruta: "/panel/defunciones", etiqueta: "Certificado de defunción", exige: None },
    // LOS CUATRO comparten pantalla y **es a propósito**: son un solo momento de
    // trabajo (mirar lo ordenado, ponerlo, anotarlo). Se licencian por separado
    // —cuatro prefijos distintos en la API— y dentro de la pantalla cada sección
    // se dibuja solo si su módulo está en el plan. Cuatro filas con la misma
    // ruta es lo que hace que cualquiera de las cuatro licencias la abra, que es
    // la regla que `modulos_de_ruta` ya declara para `/api/documentos`.
    Pantalla { modulo: "ENFERMERIA", ruta: "/panel/enfermeria", etiqueta: "Enfermería", exige: None },
    Pantalla { modulo: "KARDEX", ruta: "/panel/enfermeria", etiqueta: "Enfermería", exige: None },
    Pantalla { modulo: "MAR", ruta: "/panel/enfermeria", etiqueta: "Enfermería", exige: None },
    Pantalla { modulo: "PLAN_CUIDADO", ruta: "/panel/enfermeria", etiqueta: "Enfermería", exige: None },
    Pantalla { modulo: "CENSO_DIARIO", ruta: "/panel/censo", etiqueta: "Censo diario", exige: None },
    // Los dos extremos del acto anestésico comparten pantalla, como el bloque de
    // enfermería: se licencian por separado y cada sección mira su módulo.
    Pantalla { modulo: "PREANESTESIA", ruta: "/panel/anestesia", etiqueta: "Anestesia", exige: None },
    Pantalla { modulo: "RECUPERACION", ruta: "/panel/anestesia", etiqueta: "Anestesia", exige: None },
    // Los cuatro de P&P comparten pantalla y el programa es un FILTRO: lo que
    // cambia entre ellos es la población y el esquema, no la forma de trabajar.
    // La pantalla solo ofrece los que estén en el plan (`cx.tiene`).
    Pantalla { modulo: "VACUNACION", ruta: "/panel/pyp", etiqueta: "Promoción y prevención", exige: None },
    Pantalla { modulo: "PRENATAL", ruta: "/panel/pyp", etiqueta: "Promoción y prevención", exige: None },
    Pantalla { modulo: "CRECIMIENTO_DESARROLLO", ruta: "/panel/pyp", etiqueta: "Promoción y prevención", exige: None },
    Pantalla { modulo: "SALUD_REPRODUCTIVA", ruta: "/panel/pyp", etiqueta: "Promoción y prevención", exige: None },
    // Los cuatro de habilitación comparten pantalla: quien la abre es quien
    // prepara la visita, y lo que necesita es ver lo que está mal de un vistazo.
    Pantalla { modulo: "EQUIPOS_MEDICOS", ruta: "/panel/habilitacion", etiqueta: "Habilitación", exige: None },
    Pantalla { modulo: "COMITES", ruta: "/panel/habilitacion", etiqueta: "Habilitación", exige: None },
    Pantalla { modulo: "INFECCIONES", ruta: "/panel/habilitacion", etiqueta: "Habilitación", exige: None },
    Pantalla { modulo: "INDICADORES", ruta: "/panel/habilitacion", etiqueta: "Habilitación", exige: None },
    Pantalla { modulo: "BANCO_SANGRE", ruta: "/panel/banco-sangre", etiqueta: "Banco de sangre", exige: None },
    Pantalla { modulo: "TURNOS", ruta: "/panel/turnos", etiqueta: "Turnos", exige: None },
    Pantalla { modulo: "TELEMEDICINA", ruta: "/panel/teleconsultas", etiqueta: "Telemedicina", exige: None },
    // TRABAJO_SOCIAL no tiene pantalla propia: su nota se escribe desde la
    // historia del paciente, como la de enfermería. Su ruta JSON sí existe.

    Pantalla { modulo: "REHABILITACION", ruta: "/panel/rehabilitacion", etiqueta: "Rehabilitación", exige: None },
    // La primera con capacidad propia: el archivo con el que se factura.
    Pantalla {
        modulo: "RIPS",
        ruta: "/panel/rips",
        etiqueta: "RIPS",
        exige: Some(Capacidad::GenerarRips),
    },
    // `exige: None` **a propósito, y no es un olvido**: LEER la bandeja de
    // imagenología no pide capacidad —igual que `GET /api/imagenes/ordenes`, que
    // es `Autenticado`—, porque el médico que pidió el examen tiene que poder
    // ver su resultado. Lo que exige `EscribirImagenologia` son las cuatro
    // escrituras de dentro, cada una en su manejador y con la misma capacidad
    // que su ruta JSON. Poner aquí la capacidad de escritura habría cerrado la
    // pantalla a quien la API deja entrar.
    Pantalla { modulo: "IMAGENES", ruta: "/panel/imagenes", etiqueta: "Imágenes", exige: None },
    // Igual que imagenología: mirar el parque solo pide sesión —saber si hay una
    // ambulancia disponible es lo primero que necesita quien atiende una
    // urgencia— y las seis escrituras exigen `GestionarAmbulancias`.
    Pantalla {
        modulo: "AMBULANCIAS",
        ruta: "/panel/ambulancias",
        etiqueta: "Ambulancias",
        exige: None,
    },
    // Siete paneles de SOLO LECTURA. Solo sesión, igual que las ocho rutas JSON:
    // mirar el resumen de la propia institución no es una acción de gestión, y
    // exigir una capacidad dejaría sin portada a los roles que atienden. Lo que
    // cada uno ve lo decide su ÁMBITO.
    Pantalla {
        modulo: "ESTADISTICAS",
        ruta: "/panel/estadisticas",
        etiqueta: "Estadísticas",
        exige: None,
    },
    // Como imagenología: leer el laboratorio no pide capacidad —el médico que
    // pidió el examen tiene que poder ver su resultado— y las cinco escrituras
    // exigen `EscribirLaboratorio`.
    Pantalla {
        modulo: "LABORATORIO",
        ruta: "/panel/laboratorio",
        etiqueta: "Laboratorio",
        exige: None,
    },
    // Igual que las otras dos de servicio: leer abierto, escribir con capacidad.
    Pantalla {
        modulo: "NUTRICION",
        ruta: "/panel/nutricion",
        etiqueta: "Nutrición",
        exige: None,
    },
    // Ver la agenda quirúrgica solo pide sesión. Programar exige
    // `EscribirClinico` y conducir la cirugía `AtenderUrgencias`: son DOS
    // capacidades distintas dentro de la misma pantalla, así que ninguna de las
    // dos puede ir en esta columna —cerraría la pantalla a quien la API deja
    // entrar— y cada manejador declara la suya.
    Pantalla {
        modulo: "QUIROFANO",
        ruta: "/panel/quirofano",
        etiqueta: "Quirófano",
        exige: None,
    },
    // Leer el inventario no pide capacidad: enfermería tiene que poder saber si
    // queda adrenalina antes de pedirla. Escribir exige `GestionarFarmacia`.
    Pantalla {
        modulo: "FARMACIA",
        ruta: "/panel/farmacia",
        etiqueta: "Farmacia",
        exige: None,
    },
    // Consultar el estado de una factura no pide capacidad: quien responde en
    // ventanilla tiene que poder mirarlo, y no por eso puede radicarla ni
    // anularla. Las ocho escrituras exigen `GestionarFacturacion`.
    Pantalla {
        modulo: "FACTURACION",
        ruta: "/panel/facturacion",
        etiqueta: "Facturación",
        exige: None,
    },
    // **La auditoría es la ÚNICA pestaña de administración que se vende.** El
    // resto de `/panel/admin` va exenta —un cliente sin ella no podría dar de
    // alta a nadie— y esta sí es el módulo `AUDITORIA` del catálogo, igual que
    // `/api/admin/auditoria`. Es la decisión comercial que #534 dejó tomada.
    Pantalla {
        modulo: "AUDITORIA",
        ruta: "/panel/admin/auditoria",
        etiqueta: "Auditoría",
        exige: Some(Capacidad::AdministrarInstitucion),
    },
];

/// Rutas del panel que NO son de ningún módulo: el armazón.
///
/// Mismo criterio que `FUERA_DEL_LICENCIAMIENTO` para `/api`, y por la misma
/// razón — que «no la reclama nadie» y «se nos olvidó» no se vean igual.
#[cfg(test)]
const PANEL_FUERA_DEL_LICENCIAMIENTO: &[(&str, &str)] = &[
    ("/panel/entrada", "la pantalla de entrar: quien la ve todavía no tiene institución a la que preguntarle la licencia"),
    ("/panel/salir", "cerrar la sesión propia"),
    ("/panel/entrada/codigo", "el segundo paso de entrar, en el panel: quien lo ve todavía no tiene sesión, igual que `/panel/entrada`"),
    ("/panel/segundo-factor/empezar", "empezar el enrolamiento; ver la entrada de `/panel/segundo-factor`"),
    ("/panel/segundo-factor/confirmar", "confirmar el enrolamiento; ver la entrada de `/panel/segundo-factor`"),
    ("/panel/segundo-factor/retirar", "retirar el propio factor; ver la entrada de `/panel/segundo-factor`"),
    ("/panel/segundo-factor", "activar o retirar el propio segundo factor. NO es un módulo del catálogo y no se vende: la seguridad de la cuenta propia no se cobra aparte, y licenciarla dejaría a un cliente sin poder protegerse. Exige sesión —cada manejador la relee— y retirar o cambiar de teléfono exige además la CONTRASEÑA"),
    ("/panel/buscar/paciente", "el fragmento que busca un paciente para los formularios de ambulancias, quirófano y nutrición: si colgara del módulo PACIENTES, un cliente que compró AMBULANCIAS y no PACIENTES se quedaría sin poder despachar. Exige sesión, va acotado al ámbito de quien pregunta y deja eslabón en la bitácora como cualquier listado que atraviesa titulares"),
    ("/panel/usuarios", "el personal de la institución: administración, no un módulo del catálogo — igual que `/api/usuarios`. NO queda sin puerta: exige `GestionarUsuarios`, la MISMA capacidad que su ruta JSON (ver `vista::EnSesionCon`)"),
    ("/panel/salir-todo", "cerrar las demás sesiones propias; actúa sobre el usuario autenticado y nunca sobre un id del cliente"),
    ("/panel/cambiar-clave", "cambiar la propia contraseña (#538): no es un módulo, y además es adonde el panel OBLIGA a ir cuando el administrador restableció la contraseña — licenciarla dejaría a un usuario sin poder cumplir el deber que se le exige. Exige sesión y actúa sobre la fila propia"),
    ("/panel/comportamientos/confirmar-doble.js", "un comportamiento de `boi`, vendorizado: hace falta para PINTAR, igual que htmx"),
    ("/panel/panel.css", "la hoja de estilos, que hace falta para PINTAR la entrada"),
    ("/panel/boi.css", "la mitad estética de boi (tokens+tema), vendorizada: hace falta para PINTAR, igual que panel.css"),
    ("/panel/htmx.min.js", "lo vendorizado, por lo mismo"),
    ("/panel/boi.js", "lo vendorizado, por lo mismo"),
    ("/panel/tablero", "las cifras del prestador y, para el superadmin, sus instituciones. NO es un módulo del catálogo: es la portada del panel, y licenciarla dejaría a un cliente sin saber cómo va lo suyo"),
    ("/panel", "el inicio del panel: es el MENÚ, y licenciarlo sería la puerta con la llave dentro — igual que `/api/modulos`"),
    // Superadministración. NO es de un módulo del catálogo y no puede serlo: es
    // quien DECIDE los módulos de los demás, así que licenciarla sería la puerta
    // con la llave dentro por segunda vez. Su puerta es la capacidad
    // `GestionarInstituciones`, que la matriz de una institución no puede
    // conceder — un administrador que se la diera se estaría regalando el
    // producto entero.
    ("/panel/superadmin", "la consola del sistema: decide los módulos de las demás instituciones, así que no puede depender de los módulos de ninguna. Exige `GestionarInstituciones`, que no es configurable por institución"),
    ("/panel/superadmin/{id}", "por lo mismo: la ficha de una IPS"),
    ("/panel/superadmin/{id}/licencia", "por lo mismo: fijarle el plan"),
    ("/panel/superadmin/{id}/modulos", "por lo mismo: decidir qué módulos compró"),
    ("/panel/superadmin/{id}/admin", "por lo mismo: crearle su primer administrador"),
    // La agenda médica. Hereda la decisión de `/api/agenda`: NO hay módulo de
    // agenda en el catálogo, y añadir uno cambia lo que se vende. Cada ruta se
    // declara aparte porque las del panel se casan EXACTAS y no por prefijo —
    // ver el comentario del guardián: con prefijo, una pantalla sacada de
    // `PANTALLAS` quedaría tapada en silencio.
    ("/panel/agenda", "los horarios mensuales: mismo motivo que `/api/agenda`, no hay módulo de agenda en el catálogo y añadir uno es decisión comercial"),
    ("/panel/agenda/bloqueos", "vacaciones e incapacidades del profesional; parte de la misma agenda sin módulo"),
    ("/panel/agenda/bloqueos/{id}/borrar", "quitar uno de esos bloqueos"),
    ("/panel/agenda/indicadores", "los indicadores de la Resolución 256 sobre las citas propias; misma agenda sin módulo"),
    ("/panel/agenda/{id}", "una agenda mensual y su estado"),
    ("/panel/agenda/{id}/borrar", "eliminar esa agenda"),
    // El RDA (Resumen Digital de Atención, #603). NO es un módulo del catálogo y no
    // puede serlo: emitir el RDA es OBLIGACIÓN LEGAL de todo prestador con REPS
    // activo desde el 2026-04-15 (Res. 1888 de 2025), así que licenciarlo dejaría a
    // un cliente incumpliendo por diseño. NO queda sin puerta: exige `EscribirClinico`
    // (es un acto clínico, `EnSesionCon<EscribirClinico>`), y la capa que escribe
    // (`rda_aval`) acota además la historia y el avalador al ámbito.
    ("/panel/rda/{tipo}/{id}", "preparar el RDA de un encuentro (consulta/hospitalización/urgencias): obligación legal (Res. 1888), no módulo; gateado por EscribirClinico"),
    ("/panel/rda/{tipo}/{id}/avalar", "avalar las secciones vacías del RDA; misma obligación y misma puerta clínica"),
    ("/panel/rda/{tipo}/{id}/emitir", "emitir (encolar) el RDA hacia el visor nacional (#603 F4.6); misma obligación legal y misma puerta clínica"),
    ("/panel/agenda/{id}/bloques", "añadirle un bloque de atención"),
    ("/panel/agenda/{id}/bloques/{bloque_id}/borrar", "quitarle uno"),
    // Administración. Mismo motivo que `/api/admin`: administrar la institución
    // NO se vende por módulos —un cliente sin ella no podría dar de alta a
    // nadie— y ya exige `AdministrarInstitucion` o `GestionarUsuarios` en cada
    // manejador. La excepción es `/panel/admin/auditoria`, que SÍ es el módulo
    // AUDITORIA y está en `PANTALLAS`.
    ("/panel/admin", "los datos del prestador: administrar no se vende por módulos"),
    ("/panel/admin/configuracion", "horarios, facturación y recordatorios de la propia institución"),
    ("/panel/admin/seguridad", "su política de contraseñas"),
    ("/panel/admin/campos", "los campos propios de sus formularios"),
    ("/panel/admin/campos/{id}/borrar", "quitar uno de esos campos"),
    ("/panel/admin/catalogos", "especialidades y entidades: catálogo NACIONAL, no de un módulo"),
    ("/panel/admin/especialidades", "añadir una especialidad al catálogo nacional"),
    ("/panel/admin/entidades", "añadir una entidad pagadora al catálogo nacional"),
    ("/panel/admin/usuarios", "el personal: exige `GestionarUsuarios`, igual que `/api/admin/usuarios`"),
    ("/panel/admin/usuarios/{id}/activo", "dar de baja o reactivar a alguien"),
    ("/panel/admin/usuarios/{id}/password", "restablecerle la contraseña"),
    ("/panel/admin/usuarios/{id}/nombre", "capturarle el nombre partido para el RDA (#603): administración de personal, exige `GestionarUsuarios`, no es un módulo del catálogo"),
    ("/panel/admin/capacidades", "la matriz de capacidades: exige `GestionarUsuarios` y no se vende"),
];

/// Lo que se responde cuando el módulo no está en el plan.
///
/// Lleva los nombres dentro a propósito: la pantalla puede decir «Nutrición no
/// está incluida en su plan» en vez de un 403 pelado, que es indistinguible de
/// «no tiene permiso» y manda al administrador a revisar la matriz de
/// capacidades, donde no hay nada que arreglar.
#[derive(Debug, Serialize)]
struct NoLicenciado {
    error: &'static str,
    /// Cualquiera de estos habría abierto la puerta.
    modulos: Vec<&'static str>,
}

/// ¿Cae `ruta` bajo `prefijo`, respetando la frontera de segmento?
///
/// Es la misma regla que `medico_dominio::modulo`, repetida aquí porque allí es
/// privada y exportarla solo para esto sería ensanchar la superficie del dominio
/// por comodidad de la API. Las dos tienen su caso rojo.
fn bajo_la_ruta(ruta: &str, prefijo: &str) -> bool {
    ruta.starts_with(prefijo) && ruta.as_bytes().get(prefijo.len()) == Some(&b'/')
}

/// Qué módulos exige esta ruta. `None` = ninguno la reclama.
fn exige(patron: &str) -> Option<Vec<&'static str>> {
    if let Some((_, duenno)) = LA_RUTA_MIENTE_SOBRE_SU_MODULO.iter().find(|(r, _)| *r == patron) {
        return Some(vec![*duenno]);
    }
    // Las pantallas del panel en `maud`. Van ANTES del catálogo porque sus rutas
    // viven bajo `/panel` y ningún prefijo del catálogo las reclamaría — pero son
    // exactamente el mismo módulo que su equivalente en `/api`.
    //
    // **TODOS los reclamantes, no el primero.** Cuatro filas comparten
    // `/panel/enfermeria`, cuatro `/panel/pyp`, cuatro `/panel/habilitacion` y dos
    // `/panel/anestesia`, y el comentario de `PANTALLAS` promete que «cualquiera
    // de las cuatro licencias la abra». Con `find` eso era falso en las dos
    // direcciones y sin dar un solo error: la capa exigía SIEMPRE el primero de la
    // lista, así que quien compró `MAR` y no `ENFERMERIA` recibía 403 en la
    // pantalla que pagó, y quien compró `ENFERMERIA` y no `MAR` escribía en la
    // hoja de medicamentos que no compró. La rama del catálogo, tres líneas más
    // abajo, siempre devolvió todos los reclamantes; ésta era la única que no.
    let reclamantes: Vec<&'static str> = PANTALLAS
        .iter()
        .filter(|p| p.ruta == patron || bajo_la_ruta(patron, p.ruta))
        .map(|p| p.modulo)
        .collect();
    if !reclamantes.is_empty() {
        return Some(reclamantes);
    }
    let ms = modulo::modulos_de_ruta(patron);
    if ms.is_empty() {
        None
    } else {
        Some(ms.iter().map(|m| m.nombre).collect())
    }
}

/// La capa. Va sobre el router entero con `route_layer`, así que **no toca el
/// `fallback`**: la interfaz embebida y sus archivos se sirven igual, que es lo
/// correcto — el menú se pinta en el navegador y quien no tenga un módulo verá
/// su pantalla vacía con el 403 dentro, no una aplicación que no arranca.
pub async fn exigir_modulo_licenciado(
    State(e): State<Estado>,
    peticion: Request,
    siguiente: Next,
) -> Response {
    // Sin patrón no hay ruta reconocida: se lo queda el `fallback`.
    let Some(patron) = peticion.extensions().get::<MatchedPath>().map(|p| p.as_str().to_owned())
    else {
        return siguiente.run(peticion).await;
    };

    let Some(exigidos) = exige(&patron) else {
        return siguiente.run(peticion).await;
    };

    // SIN SESIÓN SE DEJA PASAR, y es deliberado: el manejador responderá 401 con
    // su propio extractor. Contestar 403 aquí convertiría la capa en un oráculo
    // —cualquiera desde fuera sabría qué módulos tiene contratados esta
    // instalación— y además no hay institución a la que preguntarle la licencia.
    // El 401 que emite el manejador es la respuesta correcta y no dice nada.
    let Some(usuario) = crate::auth::usuario_de(&e, peticion.headers()).await else {
        return siguiente.run(peticion).await;
    };

    let institucion_id = match usuario.ambito() {
        // El superadmin es el dueño del sistema, no un cliente: no tiene
        // licencia que consultar. Es el mismo trato que le da `Autorizado`.
        Ambito::Superadmin => return siguiente.run(peticion).await,
        Ambito::Institucion(id) => id,
        // SIN INSTITUCIÓN SE DEJA PASAR, y la primera versión de esta capa hacía
        // lo contrario: devolvía 403 «sin_institucion», razonando que «no se
        // puede saber» se niega, como `SIN_DUENIO` en el aislamiento.
        //
        // Lo puso en rojo `un_paciente_no_ve_la_clinica`, y la prueba tenía
        // razón. Un `paciente` no pertenece a ninguna institución, y para lo que
        // no le corresponde este producto responde **404 y nunca 403** — que un
        // registro ajeno sea indistinguible de uno inexistente es una decisión
        // de seguridad tomada hace tiempo y escrita en `core/aislamiento.py`. La
        // capa de licencia se estaba colando DELANTE de esa respuesta y la
        // sustituía por una menos específica y más habladora.
        //
        // Pasar no abre nada: quien no tiene institución no ve una fila de
        // nadie, porque el ámbito ya lo niega en cada consulta. Y aquí no hay
        // licencia que consultar, así que no hay nada que esta capa pueda
        // decidir mejor que la ruta. Es la directiva 21 en su forma pura —el
        // sesgo es CERRAR, y la regla se prueba contra el cliente legítimo—,
        // cazada por una prueba que ya existía.
        Ambito::Ninguno => return siguiente.run(peticion).await,
    };

    let habilitados = match medico_datos::institucion::modulos_habilitados(&e.pool, institucion_id)
        .await
    {
        // 503 y no 403: si la licencia no se puede LEER, el usuario no tiene
        // nada que corregir. Caer al «sí» sería regalar el producto y caer al
        // «no» acusaría al cliente de no haberlo comprado (directiva 20).
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        Ok(None) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        Ok(Some(h)) => h,
    };

    if exigidos.iter().any(|n| habilitados.iter().any(|h| h == n)) {
        return siguiente.run(peticion).await;
    }

    // 403 y no 404. Dentro de la propia institución la EXISTENCIA del módulo no
    // es secreto —`GET /api/modulos` se lo enseña entero, con `no_incluido`
    // escrito—, así que ocultarla mandaría a buscar un fallo que no hay. El 404
    // sigue siendo para lo AJENO, que es donde sí se oculta.
    (StatusCode::FORBIDDEN, Json(NoLicenciado { error: "modulo_no_licenciado", modulos: exigidos }))
        .into_response()
}

/// El mismo 403 de la capa, para el manejador que ESCRIBE en un módulo distinto
/// del que abre su pantalla.
///
/// La capa ve la RUTA; el módulo de una escritura lo decide la TABLA, y eso solo
/// lo sabe el manejador. En una pantalla compartida las dos cosas no coinciden:
/// `/panel/enfermeria` la abre cualquiera de sus cuatro licencias —es un solo
/// momento de trabajo y para LEER es lo correcto—, pero su `POST` escribe en
/// `administracion_medicamento`, que es `MAR` y solo `MAR`. Sin esta segunda
/// puerta, ensanchar `exige()` a todos los reclamantes habría dejado que una
/// licencia pagara por las otras tres.
pub fn no_licenciado(modulo: &'static str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(NoLicenciado { error: "modulo_no_licenciado", modulos: vec![modulo] }),
    )
        .into_response()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Del `Router::new()` al `.fallback(`, igual que hace `modulos::sirve`.
    fn bloque_del_router() -> &'static str {
        const RUTAS: &str = include_str!("rutas.rs");
        let inicio = RUTAS.find("Router::new()").expect("rutas.rs sin `Router::new()`");
        let fin = RUTAS[inicio..]
            .find(".fallback(")
            .map(|x| inicio + x)
            .expect("el router no cierra con `.fallback(`");
        &RUTAS[inicio..fin]
    }

    /// El cuerpo ENTERO de `pub fn router(`, **sin comentarios**.
    ///
    /// Dos acotaciones, y cada una tapa un agujero distinto:
    ///
    /// 1. **La función entera, no el bloque hasta `.fallback(`.** axum acepta
    ///    registrar una ruta después del fallback, y esa ruta quedaría fuera de
    ///    la capa **y** fuera del bloque que leen los otros guardianes. Dos
    ///    cegueras que comparten causa (directiva 23).
    /// 2. **Sin comentarios**, porque el guardián de abajo hace aritmética de
    ///    POSICIONES y una mención en prosa la falsea. No es hipotético: se
    ///    puso rojo en su primera corrida contra el comentario que este mismo
    ///    cambio acababa de escribir, que nombra `.fallback(` unas líneas antes
    ///    de la llamada de verdad. `rutas.rs` ya tenía avisada esta trampa para
    ///    el guardián de escrituras — allí falla cerrado; aquí fallaría en la
    ///    dirección contraria el día que la prosa vaya después.
    fn cuerpo_del_router() -> String {
        const RUTAS: &str = include_str!("rutas.rs");
        let inicio = RUTAS.find("pub fn router(").expect("rutas.rs sin `pub fn router(`");
        let fin = RUTAS[inicio..]
            .find("\n}\n")
            .map(|x| inicio + x)
            .expect("no se encontró el final de `router`");
        RUTAS[inicio..fin]
            .lines()
            .map(|l| match l.find("//") {
                Some(i) => &l[..i],
                None => l,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// **La capa tiene que aplicarse DESPUÉS de la última ruta.**
    ///
    /// `route_layer` envuelve solo los endpoints que existen en el momento de
    /// aplicarla. Una `.route(` escrita debajo se sirve **sin licencia**, y
    /// `ninguna_ruta_de_modulo_se_queda_sin_licencia` la da por buena — porque
    /// pregunta si está DECIDIDA, no si la capa llega a ella. Son dos preguntas
    /// distintas y hacían falta las dos.
    ///
    /// Lo encontró la revisión de seguridad de este cambio, midiéndolo sobre el
    /// binario: ruta bajo la capa → **200** con la de al lado en 403, y las 17
    /// pruebas del guardián en verde. El comentario del router llegó a INVITAR a
    /// escribir ahí («va antes del `.fallback(` a propósito»).
    #[test]
    fn la_capa_va_despues_de_la_ultima_ruta() {
        let cuerpo = cuerpo_del_router();
        let cuerpo = cuerpo.as_str();
        // CONTROL: el despojado de comentarios no se llevó el código por delante.
        assert!(
            cuerpo.contains("/api/pacientes") && cuerpo.contains(".with_state("),
            "CONTROL EN ROJO: el cuerpo del router quedó vacío o recortado"
        );
        let capa = cuerpo
            .find(".route_layer(")
            .expect("el router perdió la capa de licencia: nada exige el módulo");
        let ultima = cuerpo
            .rfind(".route(")
            .expect("CONTROL EN ROJO: no se leyó ni una `.route(` del router");
        assert!(
            capa > ultima,
            "hay al menos una ruta registrada DESPUÉS del `.route_layer(`, y axum NO \
             la cubre: se sirve sin comprobar la licencia. Muévela arriba, antes de \
             la capa."
        );
        // Y que el `fallback` no se cuele entre medias sin que nadie lo note: la
        // capa va antes, para no envolver la interfaz embebida.
        let fallback = cuerpo.find(".fallback(").expect("el router no tiene `.fallback(`");
        assert!(
            capa < fallback,
            "la capa quedó DESPUÉS del `.fallback(`: envolvería la interfaz embebida y \
             un cliente al que le falte un módulo no podría ni cargar la aplicación"
        );
    }

    /// El fuente del router del panel, para el segundo barrido.
    const RUTAS_DEL_PANEL: &str = include_str!("vista/mod.rs");

    /// Los patrones de ruta REGISTRADOS, leídos del fuente de los DOS routers:
    /// el de `/api` (en `rutas.rs`) y el del panel (en `vista/mod.rs`).
    fn rutas_registradas() -> Vec<String> {
        let mut v = Vec::new();
        for (i, _) in bloque_del_router().match_indices(".route(") {
            let resto = &bloque_del_router()[i + ".route(".len()..];
            // El primer argumento es siempre un literal entre comillas.
            let Some(a) = resto.find('"') else { continue };
            let Some(b) = resto[a + 1..].find('"') else { continue };
            let ruta = &resto[a + 1..a + 1 + b];
            if ruta.starts_with("/api/") && !v.iter().any(|x: &String| x == ruta) {
                v.push(ruta.to_string());
            }
        }
        // Y las del panel en `maud`, que se registran en su propio `Router`
        // (`vista::rutas`). Sin este segundo barrido, migrar una pantalla de
        // Angular a `maud` la sacaría de la licencia **y** del guardián a la vez:
        // el frontal nuevo estrenando el agujero que #534 acaba de tapar.
        for (i, _) in RUTAS_DEL_PANEL.match_indices(".route(") {
            let resto = &RUTAS_DEL_PANEL[i + ".route(".len()..];
            let Some(a) = resto.find('"') else { continue };
            let Some(b) = resto[a + 1..].find('"') else { continue };
            let ruta = &resto[a + 1..a + 1 + b];
            if ruta.starts_with("/panel") && !v.iter().any(|x: &String| x == ruta) {
                v.push(ruta.to_string());
            }
        }
        v
    }

    /// **EL GUARDIÁN: ninguna ruta se queda sin decidir.**
    ///
    /// Toda ruta registrada tiene que caer en uno de tres sitios: la reclama un
    /// módulo del catálogo, su prefijo miente y se dice a qué módulo pertenece
    /// de verdad, o está declarada FUERA del licenciamiento con su motivo. Lo
    /// que no puede es no estar en ninguno — porque entonces envía sin puerta y
    /// nadie se entera, que es exactamente cómo llegó #534 hasta aquí con 760
    /// pruebas en verde.
    ///
    /// Es lo que sustituye a leer 193 firmas de manejador: la capa no se ve en
    /// la firma, y esta lista sí se lee de un vistazo.
    #[test]
    fn ninguna_ruta_de_modulo_se_queda_sin_licencia() {
        let rutas = rutas_registradas();
        assert!(
            rutas.len() >= 130,
            "se leyeron {} rutas del router: ¿cambió la forma de registrarlas y este \
             guardián dejó de mirar nada?",
            rutas.len()
        );
        // CONTROL POSITIVO: el lector encuentra rutas que sabemos que existen —
        // y **de los DOS routers**. Sin la ruta del panel en esta lista, el día
        // que el segundo barrido dejara de leer `vista/mod.rs` este guardián
        // seguiría verde sobre un panel entero sin vigilar, que es la forma
        // exacta del fallo del que viene todo esto.
        for esperada in [
            "/api/pacientes",
            "/api/nutricion/valoraciones",
            "/api/yo",
            "/panel/pacientes",
            "/panel/entrada",
        ] {
            assert!(rutas.iter().any(|r| r == esperada), "no se leyó «{esperada}»");
        }

        let sin_decidir: Vec<&String> = rutas
            .iter()
            .filter(|r| exige(r).is_none())
            .filter(|r| !FUERA_DEL_LICENCIAMIENTO.iter().any(|(p, _)| modulo_bajo(r, p)))
            // Las del panel se casan EXACTAS, no por prefijo, y la diferencia no
            // es de estilo: `/panel` es una ruta —el inicio—, no un árbol. Con la
            // regla de prefijo, esa entrada se tragaba `/panel/pacientes` y con
            // ella el panel entero, y el guardián se quedaba verde sobre un
            // frontal sin licencia.
            //
            // **Lo encontró el CASO ROJO, no la lectura**: al sacar `PACIENTES`
            // de `PANTALLAS` a propósito, esta prueba siguió pasando. Escrita la
            // pareja, salió a la primera. Es el mismo fallo que
            // `una_exencion_ancha_no_se_traga_el_modulo_que_vive_dentro` vigila
            // para `/api/admin`, cometido otra vez en el archivo que lo vigila.
            .filter(|r| !PANEL_FUERA_DEL_LICENCIAMIENTO.iter().any(|(p, _)| *p == r.as_str()))
            .collect();
        assert!(
            sin_decidir.is_empty(),
            "estas rutas no las reclama ningún módulo del catálogo y NADIE ha dicho por \
             qué quedan fuera de la licencia, así que se sirven sin puerta: {sin_decidir:?}.\n\
             O le declaras su `prefijo` al módulo en `medico_dominio::modulo::CATALOGO`, \
             o la añades a `FUERA_DEL_LICENCIAMIENTO` con el motivo escrito."
        );
    }

    /// Misma frontera de segmento que en el dominio: una entrada de la lista de
    /// exentas no puede tapar una ruta que solo comparte prefijo textual.
    fn modulo_bajo(ruta: &str, prefijo: &str) -> bool {
        ruta == prefijo
            || (ruta.starts_with(prefijo) && ruta.as_bytes().get(prefijo.len()) == Some(&b'/'))
    }

    /// **El grupo «Administración» del menú no puede convertirse en la segunda
    /// lista de #534.**
    ///
    /// `panel::SIN_MODULO` existe porque una pantalla exenta de licencia se
    /// quedaba también exenta de MENÚ: se servía y solo se llegaba a ella
    /// tecleando la URL. Pasó con `/panel/usuarios` y `/panel/agenda` iba a ser
    /// la segunda.
    ///
    /// Lo que la hace segura es que no compite con `PANTALLAS`: cada ruta está
    /// en una lista o en la otra, nunca en las dos, y **cada fila de aquí tiene
    /// que estar declarada exenta con su motivo**. Sin esto, una pantalla de
    /// módulo metida en este grupo se ofrecería sin mirar la licencia — que es
    /// exactamente el fallo que #534 cerró.
    #[test]
    fn las_pantallas_sin_modulo_estan_exentas_de_licencia() {
        // CONTROL: la lista tiene filas. Con cero, el bucle pasaría sin mirar.
        assert!(!crate::vista::panel::SIN_MODULO.is_empty());
        for (ruta, etiqueta, _) in crate::vista::panel::SIN_MODULO {
            assert!(
                PANEL_FUERA_DEL_LICENCIAMIENTO.iter().any(|(p, _)| p == ruta),
                "«{etiqueta}» ({ruta}) está en el menú de administración y NADIE ha dicho                  por qué queda fuera de la licencia"
            );
            assert!(
                !PANTALLAS.iter().any(|p| p.ruta == *ruta),
                "«{etiqueta}» ({ruta}) está en las DOS listas: el menú la enseñaría dos                  veces y no se sabría cuál manda"
            );
        }
        // Y la vuelta: ninguna pantalla de módulo se coló en el grupo de
        // administración. Sin esta mitad, meter `/panel/pacientes` aquí lo
        // ofrecería sin licencia.
        for p in PANTALLAS {
            assert!(
                !crate::vista::panel::SIN_MODULO.iter().any(|(r, _, _)| *r == p.ruta),
                "«{}» es de un módulo y está en el grupo sin módulo",
                p.etiqueta
            );
        }
    }

    /// **La pareja del guardián de arriba**: una lista de exenciones que cubre
    /// rutas que ya no existen es una lista que tapa huecos futuros sin decirlo.
    /// Cada entrada tiene que cubrir al menos una ruta real.
    #[test]
    fn ninguna_exencion_sobra() {
        let rutas = rutas_registradas();
        let muertas: Vec<&str> = FUERA_DEL_LICENCIAMIENTO
            .iter()
            .filter(|(p, _)| !rutas.iter().any(|r| modulo_bajo(r, p)))
            .map(|(p, _)| *p)
            .collect();
        assert!(
            muertas.is_empty(),
            "estas exenciones no cubren ninguna ruta registrada: {muertas:?}. \
             Una exención obsoleta seguirá dando por decidida la ruta que mañana \
             ocupe ese prefijo."
        );
    }

    /// Igual con la tabla de rutas que mienten: si la ruta se renombró, la
    /// entrada deja de aplicarse y el adjunto de la historia vuelve a quedar
    /// bajo la licencia de imagenología **sin que nada falle**.
    #[test]
    fn las_rutas_que_mienten_siguen_existiendo() {
        let rutas = rutas_registradas();
        for (patron, duenno) in LA_RUTA_MIENTE_SOBRE_SU_MODULO {
            assert!(
                rutas.iter().any(|r| r == patron),
                "«{patron}» ya no está registrada: la excepción no se aplica a nada y el \
                 adjunto que protegía volvió a caer bajo el módulo equivocado"
            );
            assert!(modulo::existe(duenno), "«{duenno}» no está en el catálogo");
        }
    }

    /// **El caso que la excepción existe para impedir.** El adjunto de la
    /// historia NO puede exigir la licencia de imagenología, y su pareja: la
    /// ruta de al lado, que sí es del módulo, SÍ la exige.
    #[test]
    fn el_adjunto_de_la_historia_no_es_del_modulo_de_imagenes() {
        assert_eq!(exige("/api/imagenes/{id}/archivo"), Some(vec!["HISTORIA"]));
        assert_eq!(exige("/api/imagenes/{id}"), Some(vec!["HISTORIA"]));
        // La pareja: sin la excepción, éstas caen donde tienen que caer.
        assert_eq!(exige("/api/imagenes/ordenes"), Some(vec!["IMAGENES"]));
        assert_eq!(exige("/api/imagenes/paciente/{id}/informes"), Some(vec!["IMAGENES"]));
    }

    /// **Una pantalla compartida la abren TODAS sus licencias, no la primera.**
    ///
    /// Es el caso rojo del defecto que encontró la revisión de seguridad de esta
    /// rama: `exige()` resolvía la rama de `PANTALLAS` con `.find()`, así que
    /// devolvía siempre el primer reclamante de la lista y la capa —que decide
    /// con `any()`— exigía ése y solo ése. Fallaba en las dos direcciones y sin
    /// dar un error: quien compró `MAR` y no `ENFERMERIA` recibía 403 en la
    /// pantalla que pagó, y quien compró `ENFERMERIA` y no `MAR` escribía en la
    /// hoja de medicamentos que no compró.
    ///
    /// Se comprueba el CONJUNTO, no la longitud: con `.find()` la aserción falla
    /// aunque alguien reordene `PANTALLAS`.
    #[test]
    fn una_pantalla_compartida_la_reclaman_todas_sus_licencias() {
        const COMPARTIDAS: &[(&str, &[&str])] = &[
            ("/panel/enfermeria", &["ENFERMERIA", "KARDEX", "MAR", "PLAN_CUIDADO"]),
            ("/panel/anestesia", &["PREANESTESIA", "RECUPERACION"]),
            (
                "/panel/pyp",
                &["VACUNACION", "PRENATAL", "CRECIMIENTO_DESARROLLO", "SALUD_REPRODUCTIVA"],
            ),
            (
                "/panel/habilitacion",
                &["EQUIPOS_MEDICOS", "COMITES", "INFECCIONES", "INDICADORES"],
            ),
        ];
        for (ruta, esperados) in COMPARTIDAS {
            for patron in [ruta.to_string(), format!("{ruta}/{{id}}/algo")] {
                let da: std::collections::BTreeSet<_> =
                    exige(&patron).unwrap_or_default().into_iter().collect();
                let toca: std::collections::BTreeSet<_> = esperados.iter().copied().collect();
                assert_eq!(
                    da, toca,
                    "«{patron}» tenía que dejar entrar a cualquiera de {esperados:?} y \
                     solo deja a {da:?}: la licencia de uno está pagando por las otras, \
                     o cerrando la pantalla a quien sí la compró"
                );
            }
        }
        // La pareja: una pantalla con UN solo reclamante sigue exigiendo uno.
        assert_eq!(exige("/panel/nacimientos"), Some(vec!["REGISTRO_NACIMIENTO"]));
        assert_eq!(exige("/panel/censo"), Some(vec!["CENSO_DIARIO"]));
    }

    /// **`bajo_la_ruta` respeta la frontera de SEGMENTO** — con fuente
    /// sintética, que es lo que le faltaba.
    ///
    /// Su docstring prometía «las dos tienen su caso rojo» y aquí no había
    /// ninguno: el del dominio existe, y `modulo_bajo`, más abajo, es una
    /// REIMPLEMENTACIÓN dentro de las pruebas, no una prueba de esta función.
    /// Lo señaló la revisión del arreglo. Hoy ninguna de las 34 rutas de
    /// `PANTALLAS` es prefijo textual de otra, así que un mutante que quitara la
    /// comprobación del `/` sobrevivía a todo el banco — el caso de un detector
    /// medido solo contra artefactos limpios (directiva 33).
    ///
    /// Por eso los casos son inventados y no salen del catálogo: lo que se mide
    /// es el VIGILANTE, no lo vigilado.
    #[test]
    fn el_prefijo_solo_cuenta_en_la_frontera_de_segmento() {
        // SÍ: la ruta cuelga del prefijo.
        for (ruta, prefijo) in [
            ("/panel/pyp/inscribir", "/panel/pyp"),
            ("/panel/pyp/{id}/egresar", "/panel/pyp"),
            ("/panel/a/b/c", "/panel/a"),
        ] {
            assert!(bajo_la_ruta(ruta, prefijo), "«{ruta}» sí cuelga de «{prefijo}»");
        }
        // NO: comparte prefijo TEXTUAL y es otra pantalla. Es la mitad que se
        // cae si alguien cambia el `starts_with` por uno a secas — y la que hoy
        // ningún dato real ejercita.
        for (ruta, prefijo) in [
            ("/panel/pypx", "/panel/pyp"),
            ("/panel/pyp-antiguo/x", "/panel/pyp"),
            ("/panel/py", "/panel/pyp"),
            // La igualdad NO es «bajo»: la resuelve la comparación exacta de
            // `exige`, y contarla aquí duplicaría el reclamante.
            ("/panel/pyp", "/panel/pyp"),
        ] {
            assert!(
                !bajo_la_ruta(ruta, prefijo),
                "«{ruta}» NO cuelga de «{prefijo}» y se dio por bueno: la licencia de una \
                 pantalla estaría abriendo otra"
            );
        }
    }

    /// **Toda escritura de una pantalla COMPARTIDA pide su licencia.**
    ///
    /// La revisión de este mismo arreglo midió el hueco que deja el banco de
    /// arriba: `exige_licencia` está desplegada en QUINCE manejadores y solo UNO
    /// —`POST /panel/enfermeria`— tiene prueba de extremo a extremo. Borrar
    /// cualquiera de los otros catorce dejaba todo en verde y reabría el defecto
    /// que este cambio cierra. Es la directiva 33 aplicada al arreglo: el
    /// mecanismo probado en un sitio y desplegado en quince.
    ///
    /// No se escribe la lista de los quince a mano —envejecería al primer
    /// manejador nuevo—: se lee el ROUTER y se cruza con `PANTALLAS`. Y lo que
    /// esta prueba no puede derivar sola lo exige a gritos: si aparece una
    /// quinta pantalla compartida, la primera mitad se pone roja pidiendo que se
    /// añada su archivo, en vez de dejarla fuera en silencio.
    #[test]
    fn toda_escritura_de_pantalla_compartida_pide_su_licencia() {
        const ROUTER: &str = include_str!("vista/mod.rs");
        // El archivo de cada pantalla compartida, con su ruta sacada del `const`
        // de verdad: el valor no puede desviarse, solo la pertenencia — y de eso
        // se ocupa la comprobación de abajo.
        let miradas: &[(&str, &str, &str)] = &[
            ("enfermeria", crate::vista::enfermeria::RUTA, include_str!("vista/enfermeria.rs")),
            ("anestesia", crate::vista::anestesia::RUTA, include_str!("vista/anestesia.rs")),
            ("pyp", crate::vista::pyp::RUTA, include_str!("vista/pyp.rs")),
            (
                "habilitacion",
                crate::vista::habilitacion::RUTA,
                include_str!("vista/habilitacion.rs"),
            ),
        ];

        // 1. ¿Son ésas TODAS las pantallas compartidas? Se deriva de `PANTALLAS`.
        let mut compartidas: Vec<&str> = PANTALLAS
            .iter()
            .filter(|p| PANTALLAS.iter().filter(|q| q.ruta == p.ruta).count() > 1)
            .map(|p| p.ruta)
            .collect();
        compartidas.sort_unstable();
        compartidas.dedup();
        let mut vistas: Vec<&str> = miradas.iter().map(|(_, r, _)| *r).collect();
        vistas.sort_unstable();
        assert_eq!(
            compartidas, vistas,
            "hay una pantalla compartida que esta prueba no mira. Añade su archivo a \
             `miradas` — si no, sus escrituras quedan sin vigilar y nada se pondrá rojo"
        );

        // 2. Cada manejador que el router registra con `post(` para esas cuatro
        //    pantallas tiene que pedir su licencia.
        let mut comprobados = 0;
        for (modulo, _, fuente) in miradas {
            let aguja = format!("post({modulo}::");
            let mut desde = 0;
            while let Some(i) = ROUTER[desde..].find(&aguja) {
                let ini = desde + i + aguja.len();
                let fin = ROUTER[ini..].find(')').expect("registro sin cerrar") + ini;
                let manejador = &ROUTER[ini..fin];
                desde = fin;

                let firma = format!("pub async fn {manejador}(");
                let j = fuente
                    .find(&firma)
                    .unwrap_or_else(|| panic!("`{modulo}::{manejador}` no está en su archivo"));
                // El cuerpo llega hasta la siguiente declaración de nivel
                // superior. Basta para no leer la licencia del manejador de al
                // lado, que es el único falso verde posible aquí.
                let cuerpo = &fuente[j..];
                let cuerpo = &cuerpo[..cuerpo[1..].find("\npub ").map_or(cuerpo.len(), |k| k + 1)];
                assert!(
                    cuerpo.contains("exige_licencia"),
                    "`{modulo}::{manejador}` escribe desde una pantalla que abren varias \
                     licencias y no pide la suya: la licencia de un módulo está pagando \
                     por la de otro"
                );
                comprobados += 1;
            }
        }
        // Positivo de control: sin esto, un router que dejara de registrar nada
        // —o una aguja mal escrita— pasaría la prueba con cero comprobaciones,
        // que es el negativo que se ve igual que el verde (directiva 30).
        assert_eq!(
            comprobados, 15,
            "se esperaban 15 escrituras de pantalla compartida y se miraron {comprobados}: \
             o el router cambió, o la aguja dejó de casar"
        );
    }

    /// **Ninguna ruta de `PANTALLAS` puede vivir dentro de otra.**
    ///
    /// Hoy no ocurre —44 filas, 34 rutas, ninguna prefijo de otra—, y por eso
    /// `exige()` puede quedarse con el `filter` simple. El día que alguien anide
    /// una pantalla dentro de otra, el conjunto se ensancharía en silencio: la
    /// licencia de la pantalla PADRE abriría la hija. Esta prueba obliga a
    /// decidirlo entonces —prefijo más largo manda— en vez de descubrirlo con un
    /// módulo servido de más.
    #[test]
    fn ninguna_pantalla_vive_dentro_de_otra() {
        for a in PANTALLAS {
            for b in PANTALLAS {
                assert!(
                    a.ruta == b.ruta || !bajo_la_ruta(b.ruta, a.ruta),
                    "«{}» vive dentro de «{}»: su licencia se ensancharía con la del padre. \
                     `exige()` tiene que pasar a resolver por el prefijo MÁS LARGO",
                    b.ruta,
                    a.ruta
                );
            }
        }
    }

    /// **La exención ancha no puede tragarse el módulo que vive dentro.**
    ///
    /// `/api/admin` está exenta —administrar no se vende por módulos— y
    /// `/api/admin/auditoria` SÍ es el módulo `AUDITORIA`. Hoy gana el módulo
    /// porque `exige()` consulta el catálogo primero y el prefijo más largo
    /// manda. Pero si alguien le quitara o renombrara el `prefijo` a
    /// `AUDITORIA`, esa ruta caería **en silencio** bajo la exención ancha y
    /// dejaría de licenciarse: `ninguna_exencion_sobra` seguiría verde, porque
    /// comprueba que una exención cubra algo, no que no cubra de más.
    ///
    /// Lo señaló la revisión de seguridad de este cambio como fragilidad
    /// emparejada, y es la forma general del problema: **toda exención por
    /// prefijo necesita su lista de lo que NO se lleva por delante.**
    #[test]
    fn una_exencion_ancha_no_se_traga_el_modulo_que_vive_dentro() {
        // Cada par: (ruta que vive dentro de una exención, módulo que la reclama).
        const DENTRO: &[(&str, &str)] = &[("/api/admin/auditoria", "AUDITORIA")];
        for (ruta, esperado) in DENTRO {
            let e = exige(ruta);
            assert_eq!(
                e.as_deref(),
                Some(&[*esperado][..]),
                "«{ruta}» tenía que exigir {esperado} y exige {e:?}: se la tragó la \
                 exención ancha y quedó fuera de la licencia sin que nada fallara"
            );
        }
        // La pareja: el resto del árbol exento sigue exento. Sin esto, la
        // aserción de arriba pasaría igual con TODO `/api/admin` licenciado.
        assert!(exige("/api/admin/usuarios").is_none());
        assert!(exige("/api/admin/politica-seguridad").is_none());

        // **Y el mismo fallo, cometido otra vez en este archivo.** `/panel` está
        // exento —es el menú—, y las pantallas cuelgan de él. Con la regla de
        // PREFIJO, esa exención se tragaba `/panel/pacientes` y con ella el panel
        // entero: el guardián se quedaba verde sobre un frontal sin licencia.
        //
        // No lo vio la lectura: lo vio el CASO ROJO. Al sacar `PACIENTES` de
        // `PANTALLAS` a propósito, `ninguna_ruta_de_modulo_se_queda_sin_licencia`
        // siguió pasando. Por eso las del panel se casan EXACTAS.
        assert_eq!(
            exige("/panel/pacientes").as_deref(),
            Some(&["PACIENTES"][..]),
            "la exención de `/panel` se tragó la pantalla que vive dentro"
        );
        assert!(
            exige("/panel").is_none(),
            "el inicio del panel ES el menú y no puede licenciarse: sería la \
             puerta con la llave dentro"
        );
    }

    /// El menú queda fuera, y esto es lo que impide la puerta con la llave
    /// dentro: sin `/api/modulos` abierto, un cliente al que le falte un módulo
    /// no podría ni averiguar cuáles tiene.
    #[test]
    fn el_menu_y_la_sesion_no_se_licencian() {
        for libre in ["/api/modulos", "/api/acceso", "/api/yo", "/api/demo", "/api/mi-clave"] {
            assert!(exige(libre).is_none(), "«{libre}» quedó bajo licencia");
        }
        // Y la pareja, para que esto no pase por decir que todo es libre.
        assert!(exige("/api/estadisticas/financiero").is_some());
        assert!(exige("/api/facturacion/facturas").is_some());
    }
}
