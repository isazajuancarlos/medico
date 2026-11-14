// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! Administración de la institución: su ficha, su configuración, su política de
//! seguridad, los campos dinámicos de la historia, los maestros que edita el
//! prestador y **la gestión de usuarios**.
//!
//! # Por qué todo junto, y no un módulo por familia
//!
//! Porque es la mayor superficie de privilegio del producto y conviene que se
//! audite de una sentada. Aquí se crea gente, se le cambia el rol y se le
//! restablece la contraseña: cualquiera de las tres, mal hecha, vale más que
//! todas las fugas de datos que el port lleva cerradas.
//!
//! # Las tres reglas que no se pueden olvidar, y de dónde salen
//!
//! **1. Solo un superadmin fabrica otro superadmin.** El comentario del lado
//! Python lo cuenta entero: la primera versión cerró los dos `PUT` y dejó
//! abiertas las dos ALTAS, que son el camino fácil —quien crea la cuenta elige
//! además su contraseña—, y las pruebas salieron verdes porque recorrían solo los
//! `PUT`: su alcance era el de la mitigación, no el de la amenaza. Un ascenso es
//! una escalada que **cruza instituciones**, porque el ámbito del superadmin no
//! acota nada.
//!
//! **2. El cupo se consume sin crear a nadie.** `puerta_para_usuario` cuenta
//! usuarios ACTIVOS, así que reactivar a quien se fue ocupa un asiento igual que
//! un alta. Comprobarlo solo al crear deja crecer indefinidamente a una
//! institución con el tope lleno: desactivar, crear, reactivar.
//!
//! **3. Un id ajeno y uno inexistente se responden igual.** 404, nunca 403.
//!
//! # Lo que NO se porta: el bloqueo de cuenta
//!
//! El modelo Python tiene `cuenta_bloqueada` y su endpoint de desbloqueo. Este
//! binario ya decidió lo contrario, y está escrito en [`crate::freno`]: tras cinco
//! fallos el siguiente intento ESPERA y la espera se dobla, «un bloqueo que exige
//! a alguien desbloquear convierte cinco peticiones de un desconocido en una
//! historia clínica inaccesible a las tres de la mañana». Añadir la columna sería
//! un segundo mecanismo para lo mismo, y de los dos gana siempre el que bloquea.
//!
//! Así que `cuenta_bloqueada` se **deriva** del freno y «desbloquear» borra los
//! intentos de ese correo, que es exactamente lo que hace un acierto.

use medico_dominio::{Ambito, RolUsuario, Visibilidad};
use serde::{Deserialize, Serialize};

use crate::filtro_ambito;

// ─────────────────────────────────────────────────────────────────────────────
// La ficha de la institución
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Institucion {
    pub id: i64,
    pub nit: String,
    pub razon_social: Option<String>,
    pub nombre_comercial: Option<String>,
    pub codigo_habilitacion: Option<String>,
    pub direccion: Option<String>,
    pub ciudad: Option<String>,
    pub departamento: Option<String>,
    pub telefono: Option<String>,
    pub email: Option<String>,
    pub representante_legal: Option<String>,
    pub identificacion_representante: Option<String>,
    pub codigo_prestador: Option<String>,
    pub tipo_prestador: Option<String>,
    pub color_primario: Option<String>,
    pub color_secundario: Option<String>,
    pub activa: bool,
}

/// Los campos editables de la ficha. **`nit` y `activa` NO están**, y es
/// deliberado: el NIT identifica al prestador ante la DIAN y la Supersalud
/// —cambiarlo es dar de alta a otro—, y desactivar una institución es una
/// decisión del vendedor, no del administrador del cliente. Los dos se cambian
/// por la ruta de superadmin.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct InstitucionCambio {
    pub razon_social: Option<String>,
    pub nombre_comercial: Option<String>,
    pub codigo_habilitacion: Option<String>,
    pub direccion: Option<String>,
    pub ciudad: Option<String>,
    pub departamento: Option<String>,
    pub telefono: Option<String>,
    pub email: Option<String>,
    pub representante_legal: Option<String>,
    pub identificacion_representante: Option<String>,
    pub codigo_prestador: Option<String>,
    pub tipo_prestador: Option<String>,
    pub color_primario: Option<String>,
    pub color_secundario: Option<String>,
}

const COLS_INST: &str = "id, nit, razon_social, nombre_comercial, codigo_habilitacion, \
    direccion, ciudad, departamento, telefono, email, representante_legal, \
    identificacion_representante, codigo_prestador, tipo_prestador, color_primario, \
    color_secundario, activa";

pub async fn institucion(
    pool: &crate::Pool,
    id: i64,
) -> Result<Option<Institucion>, sqlx::Error> {
    sqlx::query_as(&format!("SELECT {COLS_INST} FROM institucion WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn actualizar_institucion(
    pool: &crate::Pool,
    id: i64,
    c: &InstitucionCambio,
) -> Result<bool, sqlx::Error> {
    // `COALESCE(?, columna)`: lo que no viene no se toca. Un `PUT` que mandara
    // solo el teléfono no puede borrar el representante legal.
    let r = sqlx::query(
        "UPDATE institucion SET \
         razon_social = COALESCE(?, razon_social), \
         nombre_comercial = COALESCE(?, nombre_comercial), \
         codigo_habilitacion = COALESCE(?, codigo_habilitacion), \
         direccion = COALESCE(?, direccion), \
         ciudad = COALESCE(?, ciudad), \
         departamento = COALESCE(?, departamento), \
         telefono = COALESCE(?, telefono), \
         email = COALESCE(?, email), \
         representante_legal = COALESCE(?, representante_legal), \
         identificacion_representante = COALESCE(?, identificacion_representante), \
         codigo_prestador = COALESCE(?, codigo_prestador), \
         tipo_prestador = COALESCE(?, tipo_prestador), \
         color_primario = COALESCE(?, color_primario), \
         color_secundario = COALESCE(?, color_secundario) \
         WHERE id = ?",
    )
    .bind(&c.razon_social)
    .bind(&c.nombre_comercial)
    .bind(&c.codigo_habilitacion)
    .bind(&c.direccion)
    .bind(&c.ciudad)
    .bind(&c.departamento)
    .bind(&c.telefono)
    .bind(&c.email)
    .bind(&c.representante_legal)
    .bind(&c.identificacion_representante)
    .bind(&c.codigo_prestador)
    .bind(&c.tipo_prestador)
    .bind(&c.color_primario)
    .bind(&c.color_secundario)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Configuración y política: dos tablas con la misma forma
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Configuracion {
    #[sqlx(rename = "institucion_id")]
    pub id: i64,
    pub horario_apertura: String,
    pub horario_cierre: String,
    #[sqlx(skip)]
    pub dias_atencion: Vec<String>,
    pub duracion_cita_default: i64,
    pub tiempo_buffer: i64,
    pub enviar_recordatorios: bool,
    pub horas_anticipacion: i64,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i64>,
    pub smtp_usuario: Option<String>,
    pub prefijo_factura: String,
    pub resolucion_facturacion: Option<String>,
    pub rango_desde: Option<i64>,
    pub rango_hasta: Option<i64>,
    pub permitir_citas_online: bool,
    pub requiere_autorizacion: bool,
    pub dias_maximos_agendamiento: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConfiguracionCambio {
    pub horario_apertura: Option<String>,
    pub horario_cierre: Option<String>,
    pub dias_atencion: Option<Vec<String>>,
    pub duracion_cita_default: Option<i64>,
    pub tiempo_buffer: Option<i64>,
    pub enviar_recordatorios: Option<bool>,
    pub horas_anticipacion: Option<i64>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i64>,
    pub smtp_usuario: Option<String>,
    pub prefijo_factura: Option<String>,
    pub resolucion_facturacion: Option<String>,
    pub rango_desde: Option<i64>,
    pub rango_hasta: Option<i64>,
    pub permitir_citas_online: Option<bool>,
    pub requiere_autorizacion: Option<bool>,
    pub dias_maximos_agendamiento: Option<i64>,
}

/// La configuración de una institución, creándola con sus defectos si no existe.
///
/// **Se crea al LEER y no al dar de alta la institución**, y es lo que evita la
/// rama «no hay fila» en cada uno de los diecisiete campos: la pantalla siempre
/// recibe una configuración completa, y los defectos están en el `DEFAULT` de la
/// migración, en un solo sitio.
pub async fn configuracion(
    pool: &crate::Pool,
    institucion_id: i64,
) -> Result<Option<Configuracion>, sqlx::Error> {
    if institucion(pool, institucion_id).await?.is_none() {
        return Ok(None);
    }
    sqlx::query("INSERT INTO configuracion_institucion (institucion_id) VALUES (?) \
                 ON CONFLICT(institucion_id) DO NOTHING")
        .bind(institucion_id)
        .execute(pool)
        .await?;
    let mut c: Configuracion = sqlx::query_as(
        "SELECT institucion_id, horario_apertura, horario_cierre, duracion_cita_default, \
         tiempo_buffer, enviar_recordatorios, horas_anticipacion, smtp_host, smtp_port, \
         smtp_usuario, prefijo_factura, resolucion_facturacion, rango_desde, rango_hasta, \
         permitir_citas_online, requiere_autorizacion, dias_maximos_agendamiento \
         FROM configuracion_institucion WHERE institucion_id = ?",
    )
    .bind(institucion_id)
    .fetch_one(pool)
    .await?;
    let dias: String = sqlx::query_scalar(
        "SELECT dias_atencion FROM configuracion_institucion WHERE institucion_id = ?",
    )
    .bind(institucion_id)
    .fetch_one(pool)
    .await?;
    c.dias_atencion = serde_json::from_str(&dias).unwrap_or_default();
    Ok(Some(c))
}

/// Qué se rechazó al guardar, y por qué. Cada variante lleva su mensaje: «datos
/// inválidos» obliga a adivinar cuál de diecisiete campos está mal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalido {
    Campo(String),
}

impl Invalido {
    pub fn mensaje(&self) -> String {
        let Invalido::Campo(m) = self;
        m.clone()
    }
}

pub async fn actualizar_configuracion(
    pool: &crate::Pool,
    institucion_id: i64,
    c: &ConfiguracionCambio,
) -> Result<Result<(), Invalido>, sqlx::Error> {
    if let Some(h) = &c.horario_apertura
        && !hora_valida(h)
    {
        return Ok(Err(Invalido::Campo(format!(
            "«{h}» no es una hora: horario_apertura se escribe HH:MM"
        ))));
    }
    if let Some(h) = &c.horario_cierre
        && !hora_valida(h)
    {
        return Ok(Err(Invalido::Campo(format!(
            "«{h}» no es una hora: horario_cierre se escribe HH:MM"
        ))));
    }
    if configuracion(pool, institucion_id).await?.is_none() {
        return Ok(Err(Invalido::Campo("la institución no existe".into())));
    }
    let dias = c
        .dias_atencion
        .as_ref()
        .map(|v| serde_json::to_string(v).expect("una lista de cadenas siempre serializa"));
    // Los `CHECK` de la migración son los que rechazan un rango al revés, un
    // puerto fuera de rango o una duración de cero. No se repiten aquí: dos
    // escrituras de la misma regla es una que se arregla en un sitio y se olvida
    // en el otro. El error de SQLite sube como tal.
    sqlx::query(
        "UPDATE configuracion_institucion SET \
         horario_apertura = COALESCE(?, horario_apertura), \
         horario_cierre = COALESCE(?, horario_cierre), \
         dias_atencion = COALESCE(?, dias_atencion), \
         duracion_cita_default = COALESCE(?, duracion_cita_default), \
         tiempo_buffer = COALESCE(?, tiempo_buffer), \
         enviar_recordatorios = COALESCE(?, enviar_recordatorios), \
         horas_anticipacion = COALESCE(?, horas_anticipacion), \
         smtp_host = COALESCE(?, smtp_host), \
         smtp_port = COALESCE(?, smtp_port), \
         smtp_usuario = COALESCE(?, smtp_usuario), \
         prefijo_factura = COALESCE(?, prefijo_factura), \
         resolucion_facturacion = COALESCE(?, resolucion_facturacion), \
         rango_desde = COALESCE(?, rango_desde), \
         rango_hasta = COALESCE(?, rango_hasta), \
         permitir_citas_online = COALESCE(?, permitir_citas_online), \
         requiere_autorizacion = COALESCE(?, requiere_autorizacion), \
         dias_maximos_agendamiento = COALESCE(?, dias_maximos_agendamiento) \
         WHERE institucion_id = ?",
    )
    .bind(&c.horario_apertura)
    .bind(&c.horario_cierre)
    .bind(&dias)
    .bind(c.duracion_cita_default)
    .bind(c.tiempo_buffer)
    .bind(c.enviar_recordatorios)
    .bind(c.horas_anticipacion)
    .bind(&c.smtp_host)
    .bind(c.smtp_port)
    .bind(&c.smtp_usuario)
    .bind(&c.prefijo_factura)
    .bind(&c.resolucion_facturacion)
    .bind(c.rango_desde)
    .bind(c.rango_hasta)
    .bind(c.permitir_citas_online)
    .bind(c.requiere_autorizacion)
    .bind(c.dias_maximos_agendamiento)
    .bind(institucion_id)
    .execute(pool)
    .await?;
    Ok(Ok(()))
}

fn hora_valida(h: &str) -> bool {
    chrono::NaiveTime::parse_from_str(h, "%H:%M").is_ok()
}

/// La postura de seguridad que configura cada institución.
///
/// # QUÉ SE APLICA HOY, Y QUÉ NO
///
/// Escrito porque hasta el 2026-08-06 estas veintiuna columnas eran de **solo
/// escritura**: cero lectores en todo el binario. El administrador movía
/// interruptores en una pantalla y no ocurría nada — que es la forma más cara
/// del defecto de la directiva 20, porque el sistema *parece* funcionar y nadie
/// lo descubre hasta que hace falta.
///
/// | columna | ¿se aplica? |
/// |---|---|
/// | `longitud_minima_password` | **sí** — [`password_admisible`], en las tres rutas que fijan una contraseña |
/// | `requiere_mayuscula` · `requiere_minuscula` · `requiere_numero` · `requiere_caracter_especial` | **sí**, la misma función |
/// | `max_intentos_login` · `tiempo_bloqueo_minutos` | **no, y no se van a aplicar**: este binario decidió que el freno fuera una ESPERA que se dobla y no un bloqueo, y la regla vive en `guaca::freno`, compartida con informes. Dos mecanismos para lo mismo es exactamente lo que la migración `0048` explica que no se porta |
/// | `timeout_sesion_minutos` | no — hoy manda `SESSION_TIMEOUT_MINUTES`, global |
/// | `metodo_2fa` | **sí, por construcción**: desde el 2026-08-12 el binario hace TOTP (`guaca::dosfactores`), que es el único valor que esta columna puede tomar. No se lee porque no hay nada que elegir |
/// | `requiere_2fa` · `requiere_2fa_admin` | **no todavía, y ahora por otra razón**: el segundo factor EXISTE (#212) pero el enrolamiento es voluntario. Estas dos columnas dicen «obligatorio para todos» / «obligatorio para administradores», y para aplicarlas hace falta decidir qué le pasa a quien entra sin haberlo activado —empujarlo a la pantalla de enrolamiento y no dejarle salir de ahí— que es una pantalla y una decisión aparte |
/// | `dias_expiracion_password` · `historial_passwords` | no — hacen falta la caducidad y el historial de hashes, que no existen |
/// | `sesiones_simultaneas_permitidas` · `cerrar_otras_sesiones_al_login` | no — la sesión no guarda estado en el servidor a propósito (la cookie firmada lo es todo) |
/// | `retener_logs_dias` | no — la bitácora está encadenada por hash: borrar un eslabón **rompe la cadena**, así que retener menos no es una opción de configuración sino un cambio de diseño |
/// | `notificar_login_nuevo_dispositivo` · `notificar_cambio_password` · `notificar_intentos_fallidos` | no — no hay salida de correo en el binario Rust |
///
/// Las que dicen «no» se guardan igual: son paridad con el Python y el dato del
/// cliente no se tira. Lo que no se puede es dejar que la pantalla las presente
/// como si mandaran; eso va en su tarea, con esta tabla como especificación.
/// **La tabla de arriba, convertida en DATO** para que la pantalla no la repita
/// en prosa (2026-08-08).
///
/// Cada columna con `None` si el binario la aplica, o con el motivo por el que
/// no. Existía como doc-comment y decía literalmente que la tarea pendiente era
/// «que la pantalla deje de presentarlas como si mandaran, con esta tabla como
/// especificación»: una especificación en prosa no la puede leer una pantalla.
///
/// Lo vigila `toda_columna_de_la_politica_dice_si_se_aplica`: una columna nueva
/// sin declarar rompe la prueba, que es justo lo que impide que vuelva a haber
/// interruptores que no hacen nada.
/// **Qué columnas de la configuración APLICA este binario**, y por qué no las
/// demás. `None` = se aplica; `Some(motivo)` = se guarda y hoy no la lee nadie.
///
/// Es la gemela de [`POLITICA_APLICADA`] y nació de la misma medición, hecha el
/// 2026-08-08 columna por columna con `grep` sobre `crates/` y no de memoria:
/// **de las 17, ninguna tenía lector** fuera de esta capa y de la propia
/// pantalla. Diecisiete interruptores que el administrador movía y que no
/// cambiaban nada, que es la directiva 20 al revés — un sistema que *parece*
/// funcionar.
///
/// Con Angular retirado ya no hay siquiera un «lo lee el frontal para pintarlo»:
/// el consumidor único era esa pantalla, y ya no existe.
///
/// **Dos se enchufaron en la misma pasada** —`duracion_cita_default` y
/// `prefijo_factura`—, porque eran discardes puros: el binario tenía el valor
/// escrito a mano al lado de la columna que lo dice. Las demás llevan su motivo,
/// y **el motivo distingue dos cosas que no son la misma**: falta la máquina
/// (no hay salida de correo) o falta una decisión (aplicar el máximo de días de
/// agendamiento RECHAZARÍA un control a un año, que es legítimo — directiva 21).
///
/// Lo vigila `toda_columna_de_la_configuracion_dice_si_se_aplica`: una columna
/// nueva sin declarar rompe la prueba.
pub const CONFIGURACION_APLICADA: &[(&str, Option<&str>)] = &[
    // ── Se aplican ──────────────────────────────────────────────────────────
    (
        "duracion_cita_default",
        None, // `agenda::crear` la usa cuando el bloque no trae la suya.
    ),
    (
        "prefijo_factura",
        None, // `facturacion::siguiente_numero` construye con él.
    ),
    // ── Falta la MÁQUINA ────────────────────────────────────────────────────
    ("smtp_host", Some("este binario no tiene salida de correo")),
    ("smtp_port", Some("no hay salida de correo")),
    ("smtp_usuario", Some("no hay salida de correo")),
    (
        "enviar_recordatorios",
        Some("un recordatorio se manda por correo o por mensaje, y no hay ninguno de los dos"),
    ),
    ("horas_anticipacion", Some("por lo mismo que el anterior: no hay qué anticipar")),
    (
        "permitir_citas_online",
        Some("no existe portal del paciente: no hay nadie fuera que pueda agendar"),
    ),
    // ── Falta una DECISIÓN, no una máquina ──────────────────────────────────
    (
        "dias_maximos_agendamiento",
        Some("aplicarlo RECHAZARÍA un control a un año, que es una cita legítima; el techo que               tiene sentido lo decide el prestador y hoy nadie lo ha decidido (directiva 21)"),
    ),
    (
        "horario_apertura",
        Some("aplicarlo cerraría los turnos de noche y las urgencias, que atienden fuera del               horario de consulta"),
    ),
    ("horario_cierre", Some("por lo mismo que el anterior")),
    (
        "dias_atencion",
        Some("mismo caso: un festivo o un sábado de jornada especial son atenciones reales"),
    ),
    (
        "requiere_autorizacion",
        Some("la autorización de la EPS existe en el dominio, pero exigirla para agendar es una               regla de negocio que nadie ha pedido"),
    ),
    (
        "tiempo_buffer",
        Some("el hueco entre turnos lo decide hoy `duracion_slot_min` de cada bloque; meter un               segundo mecanismo para lo mismo es lo que la directiva 1 dice que no se hace"),
    ),
    // ── La resolución de facturación: aplicada desde el 2026-08-09 (#597) ────
    (
        "resolucion_facturacion",
        // `facturacion::siguiente_numero` la EXIGE cuando hay tope de rango: un
        // rango autorizado sin su número de resolución no se factura.
        None,
    ),
    (
        "rango_desde",
        // `facturacion::siguiente_numero` arranca la serie en él.
        None,
    ),
    (
        "rango_hasta",
        // `facturacion::siguiente_numero` rechaza el número que lo supere: una
        // factura fuera del rango autorizado por la DIAN es inválida.
        None,
    ),
];

pub const POLITICA_APLICADA: &[(&str, Option<&str>)] = &[
    ("longitud_minima_password", None),
    ("requiere_mayuscula", None),
    ("requiere_minuscula", None),
    ("requiere_numero", None),
    ("requiere_caracter_especial", None),
    (
        "max_intentos_login",
        Some("este binario frena con una ESPERA que se dobla, no con un bloqueo: dos mecanismos \
              para lo mismo es lo que la migración 0048 explica que no se porta"),
    ),
    ("tiempo_bloqueo_minutos", Some("por lo mismo que el anterior")),
    ("timeout_sesion_minutos", Some("hoy manda SESSION_TIMEOUT_MINUTES, que es global")),
    // Desde el 2026-08-12 el segundo factor EXISTE (#212). Lo que estas dos
    // columnas piden es IMPONERLO, que es otra cosa: hace falta decidir qué le
    // pasa a quien entra sin haberlo activado. Se mantienen aquí con el motivo
    // corregido en vez de darlas por aplicadas — un inventario que dice «sí»
    // sobre algo que no se aplica es peor que uno que dice «no».
    (
        "requiere_2fa",
        Some("el segundo factor ya existe, pero el enrolamiento es voluntario: falta la \
              pantalla que empuje a activarlo a quien entre sin él"),
    ),
    ("requiere_2fa_admin", Some("por lo mismo que el anterior")),
    (
        "metodo_2fa",
        Some("TOTP es lo único que el binario hace, así que no hay nada que elegir"),
    ),
    (
        "dias_expiracion_password",
        Some("harían falta la caducidad y el historial de hashes, que no existen"),
    ),
    ("historial_passwords", Some("no existe el historial de hashes")),
    (
        "sesiones_simultaneas_permitidas",
        Some("la sesión no guarda estado en el servidor a propósito: la cookie firmada lo es todo"),
    ),
    ("cerrar_otras_sesiones_al_login", Some("por lo mismo que el anterior")),
    (
        "retener_logs_dias",
        Some("la bitácora está encadenada por hash: borrar un eslabón ROMPE la cadena, así que \
              retener menos no es una opción de configuración sino un cambio de diseño"),
    ),
    ("notificar_login_nuevo_dispositivo", Some("no hay salida de correo en el binario Rust")),
    ("notificar_cambio_password", Some("no hay salida de correo")),
    ("notificar_intentos_fallidos", Some("no hay salida de correo")),
    // **Ésta no estaba en la tabla del doc-comment**, y la encontró el guardián
    // al convertirla en dato: la bitácora de la Ley 1581 registra SIEMPRE, no
    // según un interruptor, así que la columna no manda nada. Es exactamente el
    // tipo de hueco que una especificación en prosa deja pasar.
    (
        "registrar_todos_los_accesos",
        Some("la bitácora de la Ley 1581 registra siempre: no es opcional, y un interruptor \
              que la apagara sería incumplir por configuración"),
    ),
];

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Politica {
    #[sqlx(rename = "institucion_id")]
    pub id: i64,
    pub longitud_minima_password: i64,
    pub requiere_mayuscula: bool,
    pub requiere_minuscula: bool,
    pub requiere_numero: bool,
    pub requiere_caracter_especial: bool,
    pub dias_expiracion_password: i64,
    pub historial_passwords: i64,
    pub timeout_sesion_minutos: i64,
    pub sesiones_simultaneas_permitidas: i64,
    pub cerrar_otras_sesiones_al_login: bool,
    pub max_intentos_login: i64,
    pub tiempo_bloqueo_minutos: i64,
    pub requiere_2fa: bool,
    pub requiere_2fa_admin: bool,
    pub metodo_2fa: String,
    pub registrar_todos_los_accesos: bool,
    pub retener_logs_dias: i64,
    pub notificar_login_nuevo_dispositivo: bool,
    pub notificar_cambio_password: bool,
    pub notificar_intentos_fallidos: bool,
}

/// El suelo de una contraseña, y **no es un defecto inventado** (directiva 20):
/// es el mismo número que el esquema le impone a
/// `politica_seguridad.longitud_minima_password` con su
/// `CHECK (… BETWEEN 8 AND 128)`. Una institución puede exigir más; ninguna
/// puede bajar de aquí, y una cuenta sin institución —que no tiene política que
/// leer— se queda exactamente en él.
pub const MINIMO_PASSWORD: usize = 8;

/// ¿Admite esta contraseña la política de seguridad de su institución?
///
/// **Existe porque el mínimo no existía.** Hasta el 2026-08-06 las dos rutas que
/// fijan una contraseña —el alta de usuario y el restablecimiento por un
/// administrador— tenían escrito `else { 400, "la contraseña no cumple el
/// mínimo exigido" }`, y esa rama estaba MUERTA: colgaba de que
/// `claves::hashear` devolviera `None`, y Argon2 hashea la cadena vacía sin
/// queja. Medido por HTTP: `{"nueva_password":""}` respondía **204** y después
/// se entraba con `""`. El admin de la IPS pulsaba «restablecer» con el campo en
/// blanco y creaba una puerta que abre cualquiera que conozca el correo — a una
/// cuenta que escribe y **firma** historia clínica (Ley 527/1999). El sistema
/// Python vivo sí lo comprobaba (`admin.py:400`, `len < 6`); se perdió al
/// portar.
///
/// Y por eso la regla se escribe **aquí y contra la política**, no como un `if`
/// en cada ruta: las veinte columnas de `politica_seguridad` eran de solo
/// escritura —el administrador configuraba una postura que el binario no
/// aplicaba en ninguna parte—, que es la otra mitad del mismo defecto
/// (directiva 1: se quita la causa, no se tapa el síntoma).
///
/// `politica` va como `Option` y no con un valor de relleno: `None` significa
/// «esta cuenta no pertenece a ninguna institución de la que leer política», y
/// entonces manda [`MINIMO_PASSWORD`] a secas. Las cuatro exigencias de
/// composición no se aplican sin política porque **nadie las pidió**; la
/// longitud sí, porque es el suelo del esquema.
pub fn password_admisible(clave: &str, politica: Option<&Politica>) -> Result<(), String> {
    // Se cuentan CARACTERES, no bytes: `len()` daría por larga una contraseña de
    // cuatro letras acentuadas.
    let largo = clave.chars().count();
    let minimo = politica
        .map(|p| p.longitud_minima_password.max(0) as usize)
        .unwrap_or(0)
        .max(MINIMO_PASSWORD);
    if largo < minimo {
        return Err(format!(
            "la contraseña debe tener al menos {minimo} caracteres"
        ));
    }
    let Some(p) = politica else {
        return Ok(());
    };
    // «Especial» es «ni letra ni dígito» —el espacio cuenta—: enumerar los
    // signos permitidos deja fuera los que no se le ocurrieron a quien escribió
    // la lista, y rechazar por eso es rechazar una contraseña más fuerte.
    for (exigido, falta, mensaje) in [
        (
            p.requiere_mayuscula,
            !clave.chars().any(char::is_uppercase),
            "una letra mayúscula",
        ),
        (
            p.requiere_minuscula,
            !clave.chars().any(char::is_lowercase),
            "una letra minúscula",
        ),
        (
            p.requiere_numero,
            !clave.chars().any(|c| c.is_numeric()),
            "un dígito",
        ),
        (
            p.requiere_caracter_especial,
            !clave.chars().any(|c| !c.is_alphanumeric()),
            "un carácter que no sea letra ni dígito",
        ),
    ] {
        if exigido && falta {
            return Err(format!(
                "la política de seguridad de la institución exige {mensaje}"
            ));
        }
    }
    Ok(())
}

/// La política que hay que aplicarle a la contraseña de OTRO usuario, con la
/// puerta del ámbito ya cruzada.
///
/// Devuelve dos niveles de `Option` y **no se pueden colapsar**:
///
/// | valor | significa |
/// |---|---|
/// | `Ok(None)` | no existe, o no es de este ámbito, o es un superadmin → el llamante responde **404** |
/// | `Ok(Some(None))` | sí se administra, pero no pertenece a ninguna institución → no hay política, manda el suelo |
/// | `Ok(Some(Some(p)))` | su política |
///
/// **El orden es la razón de que exista**, y no es cosmético: comprobar la
/// contraseña ANTES del ámbito responde «la política de la institución exige una
/// mayúscula» sobre un id que debía dar 404, y con eso se enumera la plantilla
/// del vecino probando números. Es la misma regla que ya rige aquí —404, nunca
/// 403— aplicada al mensaje de validación.
pub async fn politica_para_administrar(
    pool: &crate::Pool,
    ambito: Ambito,
    usuario_id: i64,
) -> Result<Option<Option<Politica>>, sqlx::Error> {
    if del_ambito(pool, ambito, usuario_id).await?.is_none() {
        return Ok(None);
    }
    let inst: Option<i64> =
        sqlx::query_scalar("SELECT institucion_id FROM usuario WHERE id = ?")
            .bind(usuario_id)
            .fetch_one(pool)
            .await?;
    match inst {
        Some(id) => Ok(Some(politica(pool, id).await?)),
        None => Ok(Some(None)),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PoliticaCambio {
    pub longitud_minima_password: Option<i64>,
    pub requiere_mayuscula: Option<bool>,
    pub requiere_minuscula: Option<bool>,
    pub requiere_numero: Option<bool>,
    pub requiere_caracter_especial: Option<bool>,
    pub dias_expiracion_password: Option<i64>,
    pub historial_passwords: Option<i64>,
    pub timeout_sesion_minutos: Option<i64>,
    pub sesiones_simultaneas_permitidas: Option<i64>,
    pub cerrar_otras_sesiones_al_login: Option<bool>,
    pub max_intentos_login: Option<i64>,
    pub tiempo_bloqueo_minutos: Option<i64>,
    pub requiere_2fa: Option<bool>,
    pub requiere_2fa_admin: Option<bool>,
    pub metodo_2fa: Option<String>,
    pub registrar_todos_los_accesos: Option<bool>,
    pub retener_logs_dias: Option<i64>,
    pub notificar_login_nuevo_dispositivo: Option<bool>,
    pub notificar_cambio_password: Option<bool>,
    pub notificar_intentos_fallidos: Option<bool>,
}

const COLS_POL: &str = "institucion_id, longitud_minima_password, requiere_mayuscula, \
    requiere_minuscula, requiere_numero, requiere_caracter_especial, \
    dias_expiracion_password, historial_passwords, timeout_sesion_minutos, \
    sesiones_simultaneas_permitidas, cerrar_otras_sesiones_al_login, max_intentos_login, \
    tiempo_bloqueo_minutos, requiere_2fa, requiere_2fa_admin, metodo_2fa, \
    registrar_todos_los_accesos, retener_logs_dias, notificar_login_nuevo_dispositivo, \
    notificar_cambio_password, notificar_intentos_fallidos";

pub async fn politica(
    pool: &crate::Pool,
    institucion_id: i64,
) -> Result<Option<Politica>, sqlx::Error> {
    if institucion(pool, institucion_id).await?.is_none() {
        return Ok(None);
    }
    sqlx::query("INSERT INTO politica_seguridad (institucion_id) VALUES (?) \
                 ON CONFLICT(institucion_id) DO NOTHING")
        .bind(institucion_id)
        .execute(pool)
        .await?;
    Ok(Some(
        sqlx::query_as(&format!(
            "SELECT {COLS_POL} FROM politica_seguridad WHERE institucion_id = ?"
        ))
        .bind(institucion_id)
        .fetch_one(pool)
        .await?,
    ))
}

pub async fn actualizar_politica(
    pool: &crate::Pool,
    institucion_id: i64,
    c: &PoliticaCambio,
) -> Result<Result<(), Invalido>, sqlx::Error> {
    if politica(pool, institucion_id).await?.is_none() {
        return Ok(Err(Invalido::Campo("la institución no existe".into())));
    }
    sqlx::query(
        "UPDATE politica_seguridad SET \
         longitud_minima_password = COALESCE(?, longitud_minima_password), \
         requiere_mayuscula = COALESCE(?, requiere_mayuscula), \
         requiere_minuscula = COALESCE(?, requiere_minuscula), \
         requiere_numero = COALESCE(?, requiere_numero), \
         requiere_caracter_especial = COALESCE(?, requiere_caracter_especial), \
         dias_expiracion_password = COALESCE(?, dias_expiracion_password), \
         historial_passwords = COALESCE(?, historial_passwords), \
         timeout_sesion_minutos = COALESCE(?, timeout_sesion_minutos), \
         sesiones_simultaneas_permitidas = COALESCE(?, sesiones_simultaneas_permitidas), \
         cerrar_otras_sesiones_al_login = COALESCE(?, cerrar_otras_sesiones_al_login), \
         max_intentos_login = COALESCE(?, max_intentos_login), \
         tiempo_bloqueo_minutos = COALESCE(?, tiempo_bloqueo_minutos), \
         requiere_2fa = COALESCE(?, requiere_2fa), \
         requiere_2fa_admin = COALESCE(?, requiere_2fa_admin), \
         metodo_2fa = COALESCE(?, metodo_2fa), \
         registrar_todos_los_accesos = COALESCE(?, registrar_todos_los_accesos), \
         retener_logs_dias = COALESCE(?, retener_logs_dias), \
         notificar_login_nuevo_dispositivo = COALESCE(?, notificar_login_nuevo_dispositivo), \
         notificar_cambio_password = COALESCE(?, notificar_cambio_password), \
         notificar_intentos_fallidos = COALESCE(?, notificar_intentos_fallidos) \
         WHERE institucion_id = ?",
    )
    .bind(c.longitud_minima_password)
    .bind(c.requiere_mayuscula)
    .bind(c.requiere_minuscula)
    .bind(c.requiere_numero)
    .bind(c.requiere_caracter_especial)
    .bind(c.dias_expiracion_password)
    .bind(c.historial_passwords)
    .bind(c.timeout_sesion_minutos)
    .bind(c.sesiones_simultaneas_permitidas)
    .bind(c.cerrar_otras_sesiones_al_login)
    .bind(c.max_intentos_login)
    .bind(c.tiempo_bloqueo_minutos)
    .bind(c.requiere_2fa)
    .bind(c.requiere_2fa_admin)
    .bind(&c.metodo_2fa)
    .bind(c.registrar_todos_los_accesos)
    .bind(c.retener_logs_dias)
    .bind(c.notificar_login_nuevo_dispositivo)
    .bind(c.notificar_cambio_password)
    .bind(c.notificar_intentos_fallidos)
    .bind(institucion_id)
    .execute(pool)
    .await?;
    Ok(Ok(()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Campos dinámicos de la historia clínica
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Campo {
    pub id: i64,
    pub etiqueta: String,
    pub tipo: String,
    pub opciones: Option<String>,
    pub activo: bool,
    pub orden: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CampoNuevo {
    pub etiqueta: String,
    pub tipo: String,
    pub opciones: Option<String>,
    #[serde(default)]
    pub orden: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CampoCambio {
    pub etiqueta: Option<String>,
    pub tipo: Option<String>,
    pub opciones: Option<String>,
    pub activo: Option<bool>,
    pub orden: Option<i64>,
}

pub async fn campos(pool: &crate::Pool, ambito: Ambito) -> Result<Vec<Campo>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT id, etiqueta, tipo, opciones, activo, orden FROM config_campo \
         WHERE {filtro} ORDER BY orden, id"
    );
    let mut q = sqlx::query_as::<_, Campo>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

pub async fn crear_campo(
    pool: &crate::Pool,
    institucion_id: i64,
    n: &CampoNuevo,
) -> Result<Result<i64, Invalido>, sqlx::Error> {
    let r = sqlx::query(
        "INSERT INTO config_campo (institucion_id, etiqueta, tipo, opciones, orden) \
         VALUES (?,?,?,?,?)",
    )
    .bind(institucion_id)
    .bind(n.etiqueta.trim())
    .bind(&n.tipo)
    .bind(&n.opciones)
    .bind(n.orden)
    .execute(pool)
    .await;
    match r {
        Ok(r) => Ok(Ok(r.last_insert_rowid())),
        // Los `CHECK` y el `UNIQUE` de la migración son los que deciden; aquí se
        // traduce el error a algo que el usuario pueda corregir.
        Err(e) if es_restriccion(&e) => Ok(Err(Invalido::Campo(explicar(&n.tipo, &e)))),
        Err(e) => Err(e),
    }
}

pub async fn actualizar_campo(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    c: &CampoCambio,
) -> Result<Result<bool, Invalido>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(Ok(false));
    };
    let sql = format!(
        "UPDATE config_campo SET \
         etiqueta = COALESCE(?, etiqueta), tipo = COALESCE(?, tipo), \
         opciones = COALESCE(?, opciones), activo = COALESCE(?, activo), \
         orden = COALESCE(?, orden) WHERE id = ? AND {filtro}"
    );
    let mut q = sqlx::query(&sql)
        .bind(c.etiqueta.as_deref().map(str::trim))
        .bind(&c.tipo)
        .bind(&c.opciones)
        .bind(c.activo)
        .bind(c.orden)
        .bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    match q.execute(pool).await {
        Ok(r) => Ok(Ok(r.rows_affected() > 0)),
        Err(e) if es_restriccion(&e) => {
            Ok(Err(Invalido::Campo(explicar(c.tipo.as_deref().unwrap_or(""), &e))))
        }
        Err(e) => Err(e),
    }
}

pub async fn borrar_campo(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<bool, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "institucion_id") else {
        return Ok(false);
    };
    let sql = format!("DELETE FROM config_campo WHERE id = ? AND {filtro}");
    let mut q = sqlx::query(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    Ok(q.execute(pool).await?.rows_affected() > 0)
}

fn es_restriccion(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(d) if d.message().contains("CHECK")
        || d.message().contains("UNIQUE"))
}

fn explicar(tipo: &str, e: &sqlx::Error) -> String {
    let sqlx::Error::Database(d) = e else {
        return "no se pudo guardar el campo".into();
    };
    let m = d.message();
    if m.contains("UNIQUE") {
        "ya existe un campo con esa etiqueta en esta institución".into()
    } else if tipo == "SELECCION" {
        "un campo de selección necesita sus opciones, separadas por coma".into()
    } else {
        format!(
            "«{tipo}» no es un tipo de campo. Los válidos son TEXTO, NUMERO, FECHA, \
             BOOLEANO, SELECCION y AREA_TEXTO"
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Maestros que edita el prestador
// ─────────────────────────────────────────────────────────────────────────────
//
// `especialidad` y `entidad` NO llevan institución: una EPS y la cardiología son
// las mismas para todos los prestadores, y así estaban ya en el esquema. Lo que
// sí se acota es quién puede TOCARLAS, y eso vive en la capa de rutas.

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Especialidad {
    pub id: i64,
    pub nombre: String,
}

pub async fn especialidades(pool: &crate::Pool) -> Result<Vec<Especialidad>, sqlx::Error> {
    sqlx::query_as("SELECT id, nombre FROM especialidad ORDER BY nombre")
        .fetch_all(pool)
        .await
}

pub async fn crear_especialidad(
    pool: &crate::Pool,
    nombre: &str,
) -> Result<Result<i64, Invalido>, sqlx::Error> {
    let n = nombre.trim();
    if n.is_empty() {
        return Ok(Err(Invalido::Campo("la especialidad necesita un nombre".into())));
    }
    match sqlx::query("INSERT INTO especialidad (nombre) VALUES (?)")
        .bind(n)
        .execute(pool)
        .await
    {
        Ok(r) => Ok(Ok(r.last_insert_rowid())),
        Err(e) if es_restriccion(&e) => {
            Ok(Err(Invalido::Campo(format!("la especialidad «{n}» ya existe"))))
        }
        Err(e) => Err(e),
    }
}

/// Borra una especialidad **solo si no la usa nadie**.
///
/// Con `ON DELETE` por defecto, SQLite dejaría `usuario.especialidad_id`
/// apuntando a una fila que ya no está y esa referencia rota no la ve ningún
/// filtro. Se comprueba antes y se dice cuántos la usan, que es el dato con el
/// que el administrador decide.
pub async fn borrar_especialidad(
    pool: &crate::Pool,
    id: i64,
) -> Result<Result<bool, Invalido>, sqlx::Error> {
    let usos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usuario WHERE especialidad_id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    if usos > 0 {
        return Ok(Err(Invalido::Campo(format!(
            "no se puede borrar: {usos} profesional(es) la tienen asignada"
        ))));
    }
    let r = sqlx::query("DELETE FROM especialidad WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(Ok(r.rows_affected() > 0))
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Entidad {
    pub id: i64,
    pub nit: String,
    pub nombre: String,
    pub codigo_eps: String,
    /// El código con el que el prestador la tiene numerada en su sistema de
    /// siempre (el `CODIGO` de seis dígitos del VFP). No identifica a nadie ante
    /// el Estado —para eso están el NIT y el `codigo_eps`—: sirve para que quien
    /// lleva veinte años tecleando `000611` siga encontrándola así.
    pub codigo_interno: Option<String>,
    pub direccion: Option<String>,
    pub telefono: Option<String>,
    pub fax: Option<String>,
    pub contacto: Option<String>,
    pub email: Option<String>,
    /// Municipio DIVIPOLA de cinco dígitos. **Con su cero a la izquierda**: 148
    /// municipios empiezan por cero, Medellín entre ellos.
    pub cod_municipio: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntidadNueva {
    pub nit: String,
    pub nombre: String,
    pub codigo_eps: String,
    /// Los siete de contacto son OPCIONALES y van con `#[serde(default)]`: el
    /// formulario los manda vacíos y una entidad sin teléfono es una entidad
    /// legítima, no un alta incompleta.
    #[serde(default)]
    pub codigo_interno: Option<String>,
    #[serde(default)]
    pub direccion: Option<String>,
    #[serde(default)]
    pub telefono: Option<String>,
    #[serde(default)]
    pub fax: Option<String>,
    #[serde(default)]
    pub contacto: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub cod_municipio: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EntidadCambio {
    pub nombre: Option<String>,
    pub codigo_eps: Option<String>,
    pub codigo_interno: Option<String>,
    pub direccion: Option<String>,
    pub telefono: Option<String>,
    pub fax: Option<String>,
    pub contacto: Option<String>,
    pub email: Option<String>,
    pub cod_municipio: Option<String>,
}

/// El municipio, si se declara, tiene que EXISTIR en DIVIPOLA.
///
/// Va en la capa de datos y no en el manejador porque son DOS puertas —el panel
/// y `POST /api/admin/entidades`— y una regla escrita dos veces es una regla que
/// se rompe por el lado que nadie miró (directiva 1). `rutas::municipio_valido`
/// hace lo mismo para el paciente; aquí no se puede llamar (está en la capa de
/// arriba), así que lo que se comparte es la consulta, `divipola::existe`.
///
/// Vacío o ausente **no es un error**: el municipio es opcional. Lo que se
/// rechaza es un código que no existe — y el mensaje dice lo del cero a la
/// izquierda, porque 148 municipios empiezan por cero y ese es el error real que
/// comete quien copia de una hoja de cálculo.
async fn municipio_desconocido(
    pool: &crate::Pool,
    cod: &Option<String>,
) -> Result<Option<Invalido>, sqlx::Error> {
    let Some(cod) = vacio_a_nulo(cod) else { return Ok(None) };
    if crate::divipola::existe(pool, cod).await? {
        return Ok(None);
    }
    Ok(Some(Invalido::Campo(format!(
        "el código de municipio «{cod}» no está en DIVIPOLA \
         (van cinco dígitos, con el cero de la izquierda si lo lleva)"
    ))))
}

/// Un campo de contacto en blanco es `NULL`, no `""`.
///
/// Son dos cosas distintas —«no se registró» y «se registró vacío»— y con la
/// cadena vacía no hay forma de distinguirlas después (directiva 32). Se decide
/// en UN sitio para que el alta y la edición no puedan divergir.
fn vacio_a_nulo(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

pub async fn entidades(pool: &crate::Pool) -> Result<Vec<Entidad>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, nit, nombre, codigo_eps, codigo_interno, direccion, telefono, fax, \
         contacto, email, cod_municipio FROM entidad ORDER BY nombre",
    )
        .fetch_all(pool)
        .await
}

/// Una entidad (EAPB) por id, para el pagador del RDA de encuentro (#603). `None`
/// si no existe. El catálogo de entidades es NACIONAL, no por institución, así que
/// no se filtra por ámbito —la EAPB de un paciente es la misma la mire quien la
/// mire—.
pub async fn entidad_por_id(pool: &crate::Pool, id: i64) -> Result<Option<Entidad>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, nit, nombre, codigo_eps, codigo_interno, direccion, telefono, fax, \
         contacto, email, cod_municipio FROM entidad WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn crear_entidad(
    pool: &crate::Pool,
    n: &EntidadNueva,
) -> Result<Result<i64, Invalido>, sqlx::Error> {
    for (campo, valor) in [("nit", &n.nit), ("nombre", &n.nombre), ("codigo_eps", &n.codigo_eps)] {
        if valor.trim().is_empty() {
            return Ok(Err(Invalido::Campo(format!("la entidad necesita su {campo}"))));
        }
    }
    if let Some(mal) = municipio_desconocido(pool, &n.cod_municipio).await? {
        return Ok(Err(mal));
    }
    // Un campo de contacto en blanco se guarda como `NULL`, no como `''`: son dos
    // cosas distintas —«no se registró» y «se registró vacío»— y con `''` no hay
    // forma de distinguirlas después (directiva 32). `vacio_a_nulo` recorta y
    // decide en un solo sitio, para que las dos consultas no puedan divergir.
    match sqlx::query(
        "INSERT INTO entidad (nit, nombre, codigo_eps, codigo_interno, direccion, telefono, \
         fax, contacto, email, cod_municipio) VALUES (?,?,?,?,?,?,?,?,?,?)",
    )
        .bind(n.nit.trim())
        .bind(n.nombre.trim())
        .bind(n.codigo_eps.trim())
        .bind(vacio_a_nulo(&n.codigo_interno))
        .bind(vacio_a_nulo(&n.direccion))
        .bind(vacio_a_nulo(&n.telefono))
        .bind(vacio_a_nulo(&n.fax))
        .bind(vacio_a_nulo(&n.contacto))
        .bind(vacio_a_nulo(&n.email))
        .bind(vacio_a_nulo(&n.cod_municipio))
        .execute(pool)
        .await
    {
        Ok(r) => Ok(Ok(r.last_insert_rowid())),
        Err(e) if es_restriccion(&e) => Ok(Err(Invalido::Campo(format!(
            "ya hay una entidad con el NIT {}",
            n.nit.trim()
        )))),
        Err(e) => Err(e),
    }
}

/// **El NIT no se edita.** Identifica a la aseguradora ante la DIAN; cambiarlo
/// convierte las facturas ya radicadas contra esa entidad en facturas contra
/// otra, sin que nada lo delate. Se da de alta la correcta y se deja de usar la
/// vieja.
pub async fn actualizar_entidad(
    pool: &crate::Pool,
    id: i64,
    c: &EntidadCambio,
) -> Result<Result<bool, Invalido>, sqlx::Error> {
    // Lo obligatorio no se puede vaciar POR EDICIÓN, igual que no se puede en el
    // alta. `None` sigue significando «no lo toques».
    for (campo, valor) in [("nombre", &c.nombre), ("codigo_eps", &c.codigo_eps)] {
        if valor.as_deref().is_some_and(|v| v.trim().is_empty()) {
            return Ok(Err(Invalido::Campo(format!(
                "la entidad necesita su {campo}: para dejarlo sin tocar, no lo mandes"
            ))));
        }
    }
    if let Some(mal) = municipio_desconocido(pool, &c.cod_municipio).await? {
        return Ok(Err(mal));
    }
    // TRES estados por campo, no dos, y `COALESCE` solo distingue dos (directiva
    // 32). Para los contactos hacen falta los tres:
    //
    //   `None`        → no se tocó         → se queda como está
    //   `Some("")`    → se BORRÓ el campo  → pasa a `NULL`
    //   `Some(valor)` → se escribió        → se guarda recortado
    //
    // Con `COALESCE(?, campo)` el segundo caso es inalcanzable: `Some("")` se
    // guardaría como cadena vacía, y entonces «no se registró» y «se registró
    // vacío» se confunden para siempre. Por eso va `CASE WHEN ? THEN ? ELSE
    // campo END`, con una bandera de PRESENCIA aparte del valor.
    //
    // `nombre` y `codigo_eps` NO llevan ese trato porque son OBLIGATORIOS. Pero
    // `COALESCE` solo no basta, y este comentario afirmaba que sí: `COALESCE('',
    // codigo_eps)` devuelve `''`, porque la cadena vacía **no es `NULL`**. O sea
    // que un `PUT` con `{"codigo_eps":"  "}` vaciaba el código que viaja al RIPS
    // de TODOS los prestadores, y respondía 204.
    //
    // Lo cazó la revisión de la directiva 25 sobre `16ed8b32`. El alta sí lo
    // rechaza; la edición no lo hacía, así que la invariante se rompía por el
    // otro lado. Se comprueba arriba, antes de tocar nada.
    macro_rules! opcional {
        ($q:expr, $c:expr) => {
            $q.bind($c.is_some()).bind($c.as_deref().map(str::trim).filter(|v| !v.is_empty()))
        };
    }
    let q = sqlx::query(
        "UPDATE entidad SET nombre = COALESCE(?, nombre), \
         codigo_eps = COALESCE(?, codigo_eps), \
         codigo_interno = CASE WHEN ? THEN ? ELSE codigo_interno END, \
         direccion     = CASE WHEN ? THEN ? ELSE direccion     END, \
         telefono      = CASE WHEN ? THEN ? ELSE telefono      END, \
         fax           = CASE WHEN ? THEN ? ELSE fax           END, \
         contacto      = CASE WHEN ? THEN ? ELSE contacto      END, \
         email         = CASE WHEN ? THEN ? ELSE email         END, \
         cod_municipio = CASE WHEN ? THEN ? ELSE cod_municipio END \
         WHERE id = ?",
    )
    .bind(c.nombre.as_deref().map(str::trim))
    .bind(c.codigo_eps.as_deref().map(str::trim));
    let q = opcional!(q, c.codigo_interno);
    let q = opcional!(q, c.direccion);
    let q = opcional!(q, c.telefono);
    let q = opcional!(q, c.fax);
    let q = opcional!(q, c.contacto);
    let q = opcional!(q, c.email);
    let q = opcional!(q, c.cod_municipio);
    let r = q.bind(id).execute(pool).await?;
    Ok(Ok(r.rows_affected() > 0))
}

/// Igual que la especialidad: no se borra lo que alguien está usando. Aquí son
/// tres tablas —pacientes, contratos y facturas— y las tres importan: una
/// factura que apunte a una entidad borrada no se puede volver a radicar.
pub async fn borrar_entidad(
    pool: &crate::Pool,
    id: i64,
) -> Result<Result<bool, Invalido>, sqlx::Error> {
    for (tabla, que) in [
        ("paciente", "paciente(s)"),
        ("contrato", "contrato(s)"),
        ("factura", "factura(s)"),
    ] {
        let usos: i64 =
            sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {tabla} WHERE entidad_id = ?"))
                .bind(id)
                .fetch_one(pool)
                .await?;
        if usos > 0 {
            return Ok(Err(Invalido::Campo(format!(
                "no se puede borrar: {usos} {que} la referencian"
            ))));
        }
    }
    let r = sqlx::query("DELETE FROM entidad WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(Ok(r.rows_affected() > 0))
}

// ─────────────────────────────────────────────────────────────────────────────
// Usuarios: la parte con privilegio
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct UsuarioAdmin {
    pub id: i64,
    /// El identificador con el que se inicia sesión. **En este binario es el
    /// CORREO**: no existe una columna `username` aparte, porque un segundo
    /// identificador que no autentica nada es un campo decorativo. Viaja con el
    /// nombre que la pantalla usa para no obligarla a cambiar.
    pub username: String,
    pub nombre_completo: String,
    /// El nombre partido del profesional para el RDA (#603): puede estar ausente
    /// (todos NULL hasta que la pantalla de personal lo capture).
    pub nombre1: Option<String>,
    pub nombre2: Option<String>,
    pub apellido1: Option<String>,
    pub apellido2: Option<String>,
    pub email: Option<String>,
    pub rol: String,
    pub activo: bool,
    /// **Derivado del freno**, no de una columna. Ver la cabecera del módulo:
    /// aquí no se bloquean cuentas, se retrasa el siguiente intento.
    pub cuenta_bloqueada: bool,
    pub especialidad_id: Option<i64>,
    pub especialidad_nombre: Option<String>,
    pub registro_medico: Option<String>,
    pub registro_rethus: Option<String>,
    pub requiere_cambio_password: bool,
    pub ultimo_cambio_password: Option<String>,
}

#[derive(sqlx::FromRow)]
struct FilaUsuario {
    id: i64,
    email: String,
    nombre: String,
    nombre1: Option<String>,
    nombre2: Option<String>,
    apellido1: Option<String>,
    apellido2: Option<String>,
    rol: String,
    activo: bool,
    especialidad_id: Option<i64>,
    especialidad_nombre: Option<String>,
    registro_medico: Option<String>,
    registro_rethus: Option<String>,
    requiere_cambio_password: bool,
    ultimo_cambio_password: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FiltroUsuarios {
    pub q: Option<String>,
    pub rol: Option<String>,
    pub activo: Option<bool>,
}

/// Los usuarios del ámbito. **El superadmin nunca sale del listado**, igual que
/// en el Python: no es personal del prestador y enseñarlo daría al administrador
/// de un cliente el correo del dueño del sistema.
pub async fn usuarios(
    pool: &crate::Pool,
    ambito: Ambito,
    f: &FiltroUsuarios,
    frenados: &[String],
) -> Result<Vec<UsuarioAdmin>, sqlx::Error> {
    let Some((filtro, inst)) = filtro_ambito(ambito, "u.institucion_id") else {
        return Ok(Vec::new());
    };
    let mut sql = format!(
        "SELECT u.id, u.email, u.nombre, u.nombre1, u.nombre2, u.apellido1, u.apellido2, \
         u.rol, u.activo, u.especialidad_id, \
         e.nombre AS especialidad_nombre, u.registro_medico, u.registro_rethus, \
         u.requiere_cambio_password, u.ultimo_cambio_password \
         FROM usuario u LEFT JOIN especialidad e ON e.id = u.especialidad_id \
         WHERE {filtro} AND u.rol <> 'superadmin'"
    );
    if f.q.is_some() {
        // `ESCAPE '\\'` NO ES DECORACIÓN: sin la cláusula, SQLite no reconoce
        // ningún carácter de escape y el escapado de abajo es inerte. No había
        // riesgo —el patrón va enlazado, así que el `%` del cliente nunca amplía
        // el conjunto— pero buscar un `%` o un `_` literal no encontraba nunca
        // nada, y la prueba que lo vigilaba salía verde por el motivo
        // equivocado. Lo cazó la revisión de esta rama.
        sql.push_str(" AND (u.nombre LIKE ? ESCAPE '\\' OR u.email LIKE ? ESCAPE '\\')");
    }
    if f.rol.is_some() {
        sql.push_str(" AND u.rol = ?");
    }
    if f.activo.is_some() {
        sql.push_str(" AND u.activo = ?");
    }
    sql.push_str(" ORDER BY u.nombre");

    let mut q = sqlx::query_as::<_, FilaUsuario>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if let Some(t) = &f.q {
        // El patrón se ENLAZA, no se interpola: `%` y `_` del usuario son datos.
        let patron = format!("%{}%", t.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        q = q.bind(patron.clone()).bind(patron);
    }
    if let Some(r) = &f.rol {
        q = q.bind(r.clone());
    }
    if let Some(a) = f.activo {
        q = q.bind(a);
    }
    Ok(q
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|f| UsuarioAdmin {
            cuenta_bloqueada: frenados.iter().any(|c| c == &f.email),
            username: f.email.clone(),
            email: Some(f.email),
            nombre_completo: f.nombre,
            nombre1: f.nombre1,
            nombre2: f.nombre2,
            apellido1: f.apellido1,
            apellido2: f.apellido2,
            id: f.id,
            rol: f.rol,
            activo: f.activo,
            especialidad_id: f.especialidad_id,
            especialidad_nombre: f.especialidad_nombre,
            registro_medico: f.registro_medico,
            registro_rethus: f.registro_rethus,
            requiere_cambio_password: f.requiere_cambio_password,
            ultimo_cambio_password: f.ultimo_cambio_password,
        })
        .collect())
}

/// Los correos con el freno activo ahora mismo, para derivar `cuenta_bloqueada`.
///
/// # Usa la REGLA, no una consulta que se le parece
///
/// La primera versión contaba `exito = 0` de las últimas 24 h agrupando por
/// correo, y eso NO es el freno. La regla —`guaca::freno::evaluar`, que es la
/// que decide de verdad en el inicio de sesión— difiere en tres cosas, y las
/// tres decían «bloqueada» de más:
///
/// | | la regla | la consulta que había |
/// |---|---|---|
/// | clave | **(correo, origen)** | solo correo |
/// | ventana | fallos **posteriores al último acierto** | todos los de 24 h |
/// | vigencia | la espera caduca (15 s … 5 min) | la marca duraba 24 h |
///
/// La segunda es la demoledora: `anotar` no borra los fallos al acertar, así que
/// **quien se equivocaba cinco veces y a la sexta entraba bien aparecía
/// bloqueado 24 horas**. No hace falta atacante: es el uso normal. Y esa falsa
/// alarma es lo que empuja a pulsar «desbloquear», que es el botón que borra el
/// rastro (ver [`quitar_freno`]). Directiva 23 aplicada a la propia salvaguarda.
///
/// Lo cazó la revisión de esta rama. Ahora se traen las filas y decide la misma
/// función que decide en el login, por cada par (correo, origen).
pub async fn correos_frenados(
    pool: &crate::Pool,
    ahora: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<String>, sqlx::Error> {
    let filas: Vec<(String, String, String, i64)> =
        sqlx::query_as("SELECT email, origen, momento, exito FROM intento_acceso")
            .fetch_all(pool)
            .await?;

    let mut por_par: std::collections::BTreeMap<(String, String), Vec<(String, bool)>> =
        std::collections::BTreeMap::new();
    for (email, origen, momento, exito) in filas {
        por_par
            .entry((email, origen))
            .or_default()
            .push((momento, exito != 0));
    }

    // Las reposiciones administrativas (#539), por correo: cada una entra en la
    // regla como un acierto sintético para el par que se evalúa, igual que en el
    // inicio de sesión. Sin esto, la lista seguiría marcando frenado a quien un
    // admin acaba de reponer.
    let reposiciones: std::collections::BTreeMap<String, String> =
        sqlx::query_as("SELECT email, momento FROM reposicion_freno")
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();

    let mut frenados: Vec<String> = por_par
        .into_iter()
        .filter(|((email, _), intentos)| {
            // Atada a una variable para que la cadena viva tanto como `v`, que la
            // toma prestada.
            let reposicion = reposiciones.get(email).cloned();
            let mut v: Vec<guaca::freno::Intento<'_>> = intentos
                .iter()
                .map(|(momento, exito)| guaca::freno::Intento {
                    momento: momento.as_str(),
                    exito: *exito,
                })
                .collect();
            v.extend(crate::freno::como_acierto(&reposicion));
            // Frenado = la regla impone espera AHORA. Un par que ya cumplió su
            // castigo deja de contar solo, sin que nadie pulse nada.
            guaca::freno::evaluar(&v, ahora).frenado()
        })
        .map(|((email, _), _)| email)
        .collect();
    frenados.sort();
    frenados.dedup();
    Ok(frenados)
}

/// Repone el freno de ese correo SIN borrar la evidencia del ensayo (#539).
///
/// **No hay ninguna cuenta que «desbloquear»** — ver la cabecera del módulo: el
/// freno es una espera que se dobla, no un bloqueo.
///
/// # Qué cambió, y por qué borraba lo que no debía
///
/// Antes hacía `DELETE FROM intento_acceso WHERE email=? AND exito=0`, y esa
/// tabla es el ÚNICO registro que existe de un ensayo de contraseñas: la bitácora
/// `acceso` anota lo que OCURRIÓ, no lo que se INTENTÓ. Así el botón que un admin
/// pulsa para desatascar a un médico borraba de paso la única prueba de que a esa
/// cuenta la estuvieron probando. La 0022 guardaba el acierto precisamente para
/// no tener que borrar esa serie (ver su cabecera); el DELETE la contradecía.
///
/// Ahora se anota una **reposición** en su instante: `guaca::freno::evaluar` la
/// trata como un acierto sintético (ver [`crate::freno`]), así que mueve hacia
/// adelante el punto desde el que se cuentan los fallos —limpia la espera— sin
/// tocar una sola fila de fallos. Los fallos que lleguen DESPUÉS vuelven a
/// contar: reponer no es un pase permanente.
///
/// # Lo que este endpoint cuesta, dicho en voz alta
///
/// La espera se agota sola en cinco minutos como mucho, así que lo que compra es
/// poco. Y el freno es por (correo, ORIGEN): la reposición es por correo, así que
/// alcanza a todos los orígenes —incluido el de un atacante que estuviera
/// ensayando contra ese correo, al que le mueve el punto de conteo—. Es la misma
/// amplitud que tenía el DELETE; lo que cambia es que la evidencia se queda. El
/// arreglo de [`correos_frenados`] quita, además, casi todas las razones para
/// pulsarlo.
pub async fn quitar_freno(pool: &crate::Pool, email: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO reposicion_freno (email, momento) VALUES (?, ?)
         ON CONFLICT(email) DO UPDATE SET momento = excluded.momento",
    )
    .bind(email)
    .bind(crate::ahora())
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
pub struct UsuarioAltaAdmin {
    /// El identificador de inicio de sesión. La pantalla lo llama `username`; si
    /// además viene `email`, gana `email` — son el mismo dato y tener dos formas
    /// de escribirlo es de la compatibilidad, no del modelo.
    pub username: Option<String>,
    pub email: Option<String>,
    pub password: String,
    pub nombre_completo: String,
    pub rol: String,
    pub especialidad_id: Option<i64>,
    pub registro_medico: Option<String>,
    pub registro_rethus: Option<String>,
    /// El nombre PARTIDO que el RDA de encuentro (#603) exige del profesional:
    /// `given` (nombres) y `family` (apellidos) por separado. Opcional y aparte
    /// del `nombre_completo` —que sigue siendo el de login/listados/firma—,
    /// porque no todo el personal es clínico y el productor del RDA anota FALTA
    /// cuando el profesional no lo tiene (migración 0071, directiva 20: no se
    /// parte un nombre completo adivinando dónde acaba el nombre).
    pub nombre1: Option<String>,
    pub nombre2: Option<String>,
    pub apellido1: Option<String>,
    pub apellido2: Option<String>,
}

impl UsuarioAltaAdmin {
    pub fn identificador(&self) -> Option<&str> {
        self.email
            .as_deref()
            .or(self.username.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UsuarioCambioAdmin {
    pub nombre_completo: Option<String>,
    pub email: Option<String>,
    pub rol: Option<String>,
    pub activo: Option<bool>,
    pub especialidad_id: Option<i64>,
    pub registro_medico: Option<String>,
    pub registro_rethus: Option<String>,
    pub requiere_cambio_password: Option<bool>,
}

/// Por qué no se pudo. Cada variante es una decisión distinta de quien llama:
/// las de privilegio son 403, las de datos 400 y las de existencia 404.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FalloUsuario {
    NoExiste,
    /// Solo un superadmin fabrica otro superadmin.
    AscensoProhibido,
    /// El rol no está en el catálogo.
    RolDesconocido(String),
    /// La licencia no deja crecer: sin licencia, vencida o con el cupo lleno.
    Licencia(String),
    Invalido(String),
}

/// Comprueba el ascenso ANTES que nada. Es la regla 1 de la cabecera.
///
/// **El rol se PARSEA antes de compararlo**, y no es prolijidad. La primera
/// versión comparaba la cadena cruda (`rol_destino != Some("superadmin")`)
/// mientras el escritor la parseaba con `RolUsuario::desde_str`: dos
/// representaciones del mismo dato, y la guardia mirando la que no manda.
///
/// Hoy `"SUPERADMIN"` pasaría la guardia de cadena y luego moriría en
/// `RolDesconocido` —falla cerrado, comprobado en
/// `el_ascenso_no_se_cuela_con_otra_caja`—, pero esa seguridad depende de que
/// `desde_str` siga siendo sensible a la caja, que es una propiedad de OTRO
/// archivo. Comparando el enum, la guardia no depende de nada.
///
/// Un rol ILEGIBLE devuelve `true`: no es un ascenso, es un dato inválido, y
/// rechazarlo aquí daría 403 donde corresponde 400. Quien escribe lo vuelve a
/// parsear y lo rechaza con su nombre.
/// # NADIE CREA UN ROL QUE PUEDA MÁS QUE ÉL
///
/// Hasta el 2026-08-10 esta función impedía **una sola cosa**: subir a
/// `Superadmin`. Cualquier otro rol pasaba para cualquier titular de
/// `GestionarUsuarios`, y eso convertía en decorativo el guardarraíl 3 de
/// `Ajustes::validar` —«nadie concede lo que su rol no trae»— que se había
/// añadido ese mismo día: el laboratorista al que el admin delegó la gestión no
/// necesitaba tocar la matriz, le bastaba **crearse un admin** con una
/// contraseña suya y volver a entrar. Con eso obtenía `FirmarClinico` y firmaba
/// historia clínica (Ley 527/1999). Lo encontró la revisión de seguridad.
///
/// La regla nueva es la MISMA que la del guardarraíl 3, aplicada al alta en vez
/// de a la matriz, y con el mismo criterio: se compara contra el **defecto
/// nacional** de cada rol (`RolUsuario::puede`), no contra la matriz efectiva de
/// la institución. Contra la efectiva, un admin al que su propia IPS le hubiera
/// revocado una capacidad dejaría de poder crear al rol que la trae, y se
/// quedaría sin poder reconstruir su propia plantilla (directiva 21).
///
/// **Medido antes de escribirla, que es lo que la hace segura**: de los quince
/// roles, el ÚNICO que tiene algo que `Admin` no tiene es `Superadmin`
/// (`gestionar_instituciones`). O sea que el administrador legítimo sigue
/// pudiendo crear a todo su personal, que es la operación normal y la que había
/// que no romper. Un laboratorista con la gestión delegada puede crear
/// `paciente`, `laboratorista`, `trabajo_social` y `bacteriologo` —los que no le
/// superan— y ninguno más.
pub fn ascenso_permitido(rol_destino: Option<&str>, quien: &crate::usuario::Usuario) -> bool {
    // Un rol ILEGIBLE devuelve `true`: no es un ascenso, es un dato inválido, y
    // rechazarlo aquí daría 403 donde corresponde 400. Quien escribe lo vuelve a
    // parsear y lo rechaza con su nombre.
    let Some(destino) = rol_destino.and_then(RolUsuario::desde_str) else {
        return true;
    };
    // Un rol propio ilegible no puede conceder nada: falla cerrado.
    let Some(mio) = quien.rol() else {
        return false;
    };
    if mio == RolUsuario::Superadmin {
        return true;
    }
    // Se mira el DESTINO y no el origen: degradar a un superadmin no crea
    // autoridad nueva, y prohibirlo dejaría al dueño del sistema sin poder
    // retirar la suya.
    if destino == RolUsuario::Superadmin {
        return false;
    }
    medico_dominio::rol::Capacidad::TODAS
        .iter()
        .all(|c| !destino.puede(*c) || mio.puede(*c))
}

/// ¿Ese usuario es del ámbito de quien pregunta? `None` si no existe, es ajeno
/// **o es un superadmin**.
///
/// # El rol se mira, y la primera versión no lo miraba
///
/// Decidía solo con `puede_ver(institucion_id)`, o sea con la COLUMNA, mientras
/// el listado excluía al superadmin por su ROL. Dos definiciones distintas de
/// «superadmin» en el mismo archivo, y la de escritura era la débil.
///
/// Lo encontró la revisión de esta rama, y el camino que describe es real: nada
/// impide que exista un superadmin CON institución —la columna no tiene `CHECK`—
/// y esta misma rama lo hacía creable, porque `adm_crear_usuario` exige una
/// institución. Esa fila quedaba invisible en el listado y **escribible** por el
/// admin de esa institución: cambiarle el correo, restablecerle la contraseña y
/// entrar como dueño del sistema.
///
/// La cola `WHERE` que acota una escritura sobre `usuario` al ámbito de quien
/// administra, **dentro de la misma sentencia**.
///
/// `None` significa que ese ámbito no administra a nadie y no hay `UPDATE` que
/// hacer.
///
/// # Por qué existe si `del_ambito` ya comprobó
///
/// Porque comprobar y actuar en dos sentencias deja una ventana entre el «es
/// mío» y el «lo cambio», y este repositorio ya escribió la regla en
/// `usuario::fijar_documento`: *el filtro del ámbito va DENTRO del UPDATE, no en
/// una consulta previa*. Las tres escrituras de administración —actualizar,
/// desactivar y restablecer la contraseña— hacían lo contrario: `del_ambito` y
/// después `UPDATE … WHERE id = ?` a secas. No era explotable —ninguna ruta
/// cambia `institucion_id`, así que la fila no puede mudarse de dueño entre las
/// dos— y por eso esto es una **segunda vuelta de llave**, no un parche: el día
/// que exista un traslado entre instituciones, la ventana se abre sola y en
/// silencio.
///
/// La exclusión del superadmin va **por ROL y no por institución**, igual que en
/// `del_ambito`: un superadmin con `institucion_id` puesto es una fila creable
/// —la columna no tiene `CHECK`— y sin esta mitad el `UPDATE` lo alcanzaría
/// aunque la comprobación previa lo hubiera descartado.
fn cola_del_ambito(ambito: Ambito) -> Option<(&'static str, Option<i64>)> {
    match ambito {
        Ambito::Superadmin => Some((" WHERE id = ? AND rol <> 'superadmin'", None)),
        Ambito::Institucion(inst) => Some((
            " WHERE id = ? AND institucion_id = ? AND rol <> 'superadmin'",
            Some(inst),
        )),
        Ambito::Ninguno => None,
    }
}

/// Es la razón de ser de `Ambito` (`aislamiento.rs`): que «sin institución» no
/// se confunda con «superadmin». Aquí se volvía a confundir leyendo la columna
/// en lugar del enum.
async fn del_ambito(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<(String, String, bool)>, sqlx::Error> {
    let fila: Option<(Option<i64>, String, String, bool)> =
        sqlx::query_as("SELECT institucion_id, email, rol, activo FROM usuario WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(fila.and_then(|(inst, email, rol, activo)| {
        // El superadmin no se administra desde aquí, lo pida quien lo pida.
        // Tampoco un superadmin a otro: para eso está la ruta de superadmin, que
        // es donde esa decisión tiene su sitio y su rastro.
        if RolUsuario::desde_str(&rol) == Some(RolUsuario::Superadmin) {
            return None;
        }
        (ambito.puede_ver(inst) == Visibilidad::Si).then_some((email, rol, activo))
    }))
}

pub async fn crear_usuario(
    pool: &crate::Pool,
    institucion_id: i64,
    n: &UsuarioAltaAdmin,
    hash: &str,
) -> Result<Result<i64, FalloUsuario>, sqlx::Error> {
    let Some(email) = n.identificador() else {
        return Ok(Err(FalloUsuario::Invalido(
            "el usuario necesita un correo de inicio de sesión".into(),
        )));
    };
    match RolUsuario::desde_str(&n.rol) {
        None => return Ok(Err(FalloUsuario::RolDesconocido(n.rol.clone()))),
        // **Esta ruta NO fabrica superadmins, ni siquiera para un superadmin.**
        //
        // No es una restricción de más: `crear_usuario` recibe una institución
        // obligatoria, y un superadmin CON institución es exactamente la fila
        // que rompe el aislamiento —invisible en el listado y escribible por el
        // admin de esa institución—. Dar de alta al dueño del sistema es otra
        // cosa y tiene su propia ruta.
        Some(RolUsuario::Superadmin) => {
            return Ok(Err(FalloUsuario::AscensoProhibido));
        }
        Some(_) => {}
    }
    if n.nombre_completo.trim().is_empty() {
        return Ok(Err(FalloUsuario::Invalido("el usuario necesita un nombre".into())));
    }
    if let Some(esp) = n.especialidad_id {
        let existe: Option<(i64,)> = sqlx::query_as("SELECT id FROM especialidad WHERE id = ?")
            .bind(esp)
            .fetch_optional(pool)
            .await?;
        if existe.is_none() {
            return Ok(Err(FalloUsuario::Invalido("esa especialidad no existe".into())));
        }
    }
    let r = sqlx::query(
        "INSERT INTO usuario (nombre, email, password_hash, rol, institucion_id, activo, \
         especialidad_id, registro_medico, registro_rethus, requiere_cambio_password, \
         nombre1, nombre2, apellido1, apellido2) \
         VALUES (?,?,?,?,?,1,?,?,?,0,?,?,?,?)",
    )
    .bind(n.nombre_completo.trim())
    .bind(email)
    .bind(hash)
    .bind(&n.rol)
    .bind(institucion_id)
    .bind(n.especialidad_id)
    .bind(n.registro_medico.as_deref().map(str::trim))
    .bind(n.registro_rethus.as_deref().map(str::trim))
    // Vacío → NULL, nunca cadena vacía: el productor del RDA distingue «ausente»
    // (anota FALTA) de un dato presente, y un `""` sería un presente falso (D20).
    .bind(sin_vacio(&n.nombre1))
    .bind(sin_vacio(&n.nombre2))
    .bind(sin_vacio(&n.apellido1))
    .bind(sin_vacio(&n.apellido2))
    .execute(pool)
    .await;
    match r {
        Ok(r) => Ok(Ok(r.last_insert_rowid())),
        Err(e) if es_restriccion(&e) => Ok(Err(FalloUsuario::Invalido(format!(
            "ya hay un usuario con el correo {email}"
        )))),
        Err(e) => Err(e),
    }
}

/// Qué hace falta comprobar antes de guardar un cambio de usuario.
///
/// Se devuelve en vez de comprobarse aquí porque el cupo lo decide
/// [`crate::licencia`] y la fecha la pone la capa de rutas: esta función dice
/// **si el cambio ENTRA al conjunto contado**, que es la parte que se olvida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consumo {
    /// Pasa de inactivo a activo: ocupa un asiento que estaba libre.
    pub reactiva: bool,
    /// **Queda como médico sin serlo antes**, o entra al conjunto de médicos
    /// activos por la puerta de la reactivación.
    ///
    /// Es la otra mitad del mismo olvido, y la que el Python sí cobraba
    /// (`exigir_cupo_al_modificar`): ascender a alguien que ya existe consume un
    /// asiento de médico exactamente igual que darlo de alta, y sin esto una IPS
    /// con el cupo de médicos lleno lo rodea creando auxiliares y ascendiéndolos
    /// después. `reactiva` no lo cubre —el ascenso no cambia `activo`— así que
    /// son dos banderas y no una (directiva 32).
    pub entra_al_cupo_de_medicos: bool,
}

pub async fn consumo_de(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    c: &UsuarioCambioAdmin,
) -> Result<Option<Consumo>, sqlx::Error> {
    let Some((_, era_rol, era_activo)) = del_ambito(pool, ambito, id).await? else {
        return Ok(None);
    };
    let sera_activo = c.activo.unwrap_or(era_activo);
    let sera_rol = c.rol.as_deref().unwrap_or(&era_rol);
    let era_medico = RolUsuario::desde_str(&era_rol) == Some(RolUsuario::Medico);
    let sera_medico = RolUsuario::desde_str(sera_rol) == Some(RolUsuario::Medico);
    // Se comprueba al ENTRAR, nunca al salir: desactivar libera asiento y
    // estorbarlo sería cerrar por cerrar. Igual con el rol: bajar de médico a
    // auxiliar no se frena.
    //
    // Al cupo de médicos se entra por DOS puertas y hay que mirar las dos: por
    // el rol (no era médico y va a serlo) y por la reactivación (ya constaba
    // como médico, pero inactivo, y por tanto no contaba).
    Ok(Some(Consumo {
        reactiva: sera_activo && !era_activo,
        entra_al_cupo_de_medicos: sera_activo && sera_medico && (!era_medico || !era_activo),
    }))
}

impl Consumo {
    /// Los asientos que hay que pedirle a la licencia, o `None` si el cambio no
    /// ocupa ninguno y por tanto **ni siquiera se le pregunta**.
    ///
    /// Preguntar de más no es inofensivo: una licencia vencida bloquearía
    /// entonces un cambio de nombre o de correo, y la licencia limita la
    /// EXPANSIÓN y nada más.
    pub fn asiento(&self) -> Option<crate::licencia::Asiento> {
        (self.reactiva || self.entra_al_cupo_de_medicos).then_some(crate::licencia::Asiento {
            de_usuario: self.reactiva,
            de_medico: self.entra_al_cupo_de_medicos,
        })
    }
}

pub async fn actualizar_usuario(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    c: &UsuarioCambioAdmin,
) -> Result<Result<(), FalloUsuario>, sqlx::Error> {
    if del_ambito(pool, ambito, id).await?.is_none() {
        return Ok(Err(FalloUsuario::NoExiste));
    }
    if let Some(r) = &c.rol
        && RolUsuario::desde_str(r).is_none()
    {
        return Ok(Err(FalloUsuario::RolDesconocido(r.clone())));
    }
    let Some((cola, inst)) = cola_del_ambito(ambito) else {
        return Ok(Err(FalloUsuario::NoExiste));
    };
    let sql = format!(
        "UPDATE usuario SET \
         nombre = COALESCE(?, nombre), email = COALESCE(?, email), rol = COALESCE(?, rol), \
         activo = COALESCE(?, activo), especialidad_id = COALESCE(?, especialidad_id), \
         registro_medico = COALESCE(?, registro_medico), \
         registro_rethus = COALESCE(?, registro_rethus), \
         requiere_cambio_password = COALESCE(?, requiere_cambio_password){cola}"
    );
    let r = sqlx::query(&sql)
    .bind(c.nombre_completo.as_deref().map(str::trim))
    .bind(c.email.as_deref().map(str::trim))
    .bind(&c.rol)
    .bind(c.activo)
    .bind(c.especialidad_id)
    .bind(c.registro_medico.as_deref().map(str::trim))
    .bind(c.registro_rethus.as_deref().map(str::trim))
    .bind(c.requiere_cambio_password)
    .bind(id);
    let r = match inst {
        Some(i) => r.bind(i),
        None => r,
    }
    .execute(pool)
    .await;
    match r {
        Ok(_) => Ok(Ok(())),
        Err(e) if es_restriccion(&e) => Ok(Err(FalloUsuario::Invalido(
            "ya hay un usuario con ese correo".into(),
        ))),
        Err(e) => Err(e),
    }
}

/// La contraseña PROPIA. Devuelve el `token_version` resultante, o `None` si el
/// usuario no existe.
///
/// **No lleva ámbito, y eso es el diseño**: el id no viene del cliente sino de
/// la cookie ya verificada, así que no hay fila ajena que alcanzar. Si algún día
/// aceptara un id por parámetro, esta función tendría que crecer una puerta —y
/// entonces sería `restablecer_password`, que ya existe.
///
/// **Apaga `requiere_cambio_password`**, que es lo que convierte esa columna en
/// una bandera con ciclo cerrado: la enciende el restablecimiento del admin y la
/// apaga el cambio propio. Hasta hoy solo tenía la mitad que la enciende.
///
/// Sube `token_version` **en la misma sentencia** que escribe el hash: en dos,
/// habría un instante con la contraseña nueva puesta y las sesiones viejas
/// todavía vivas. El incremento se calcula DENTRO del SQL por la razón de
/// siempre —hacerlo en Rust pierde incrementos bajo concurrencia— y se devuelve
/// con `RETURNING` para que quien llama pueda reemitir la cookie sin releer.
pub async fn cambiar_password_propia(
    pool: &crate::Pool,
    usuario_id: i64,
    hash: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE usuario SET password_hash = ?, requiere_cambio_password = 0, \
         ultimo_cambio_password = ?, token_version = token_version + 1 \
         WHERE id = ? RETURNING token_version",
    )
    .bind(hash)
    .bind(crate::ahora())
    .bind(usuario_id)
    .fetch_optional(pool)
    .await
}

/// Baja lógica: `activo = 0`. **No se borra la fila**, porque su id aparece en
/// historias firmadas y en la bitácora de accesos; borrarlo dejaría un rastro que
/// no se puede leer.
///
/// Y revoca sus sesiones: si no, quien acaba de ser dado de baja sigue trabajando
/// con la cookie que ya tenía hasta que caduque.
pub async fn desactivar_usuario(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<bool, sqlx::Error> {
    if del_ambito(pool, ambito, id).await?.is_none() {
        return Ok(false);
    }
    let Some((cola, inst)) = cola_del_ambito(ambito) else {
        return Ok(false);
    };
    let sql = format!("UPDATE usuario SET activo = 0, token_version = token_version + 1{cola}");
    let q = sqlx::query(&sql).bind(id);
    match inst {
        Some(i) => q.bind(i),
        None => q,
    }
    .execute(pool)
    .await?;
    Ok(true)
}

/// Restablece la contraseña, revoca las sesiones abiertas y **marca** que el
/// usuario debería cambiarla.
///
/// # `requiere_cambio_password` se escribe y HOY NO LA LEE NADIE
///
/// La primera versión de este doc decía que «obliga a cambiarla al entrar», y es
/// falso: la columna no aparece en el camino de autenticación, y **no existe
/// ningún endpoint para que el usuario cambie su propia contraseña** — así que
/// la bandera no se podría satisfacer aunque se comprobara. El propio admin
/// puede además apagarla por `PUT /api/admin/usuarios/{id}`. Lo cazó la revisión
/// de esta rama.
///
/// Es paridad con el Python, que hace exactamente lo mismo, no una regresión.
/// Pero la consecuencia hay que decirla: la contraseña que fijó el administrador
/// sigue valiendo indefinidamente sobre cuentas cuyos actos clínicos van
/// firmados y atribuidos. Se escribe la marca porque es el dato que hará falta
/// el día que entren el cambio propio y su comprobación, y va en su tarea.
///
/// Lo que sí ocurre hoy y sí protege: **se revocan las sesiones**. Sin eso,
/// restablecer la contraseña de una cuenta comprometida no echa a quien está
/// dentro.
pub async fn restablecer_password(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    hash: &str,
) -> Result<bool, sqlx::Error> {
    let Some((email, _, _)) = del_ambito(pool, ambito, id).await? else {
        return Ok(false);
    };
    let Some((cola, inst)) = cola_del_ambito(ambito) else {
        return Ok(false);
    };
    let sql = format!(
        "UPDATE usuario SET password_hash = ?, requiere_cambio_password = 1, \
         ultimo_cambio_password = ?, token_version = token_version + 1{cola}"
    );
    let q = sqlx::query(&sql)
    .bind(hash)
    .bind(crate::ahora())
    .bind(id);
    match inst {
        Some(i) => q.bind(i),
        None => q,
    }
    .execute(pool)
    .await?;
    // Y se le quita el freno: si estaba esperando por intentos fallidos, la
    // contraseña nueva no le serviría hasta que pasara la espera.
    quitar_freno(pool, &email).await?;
    Ok(true)
}

/// Recorta un texto opcional y convierte el vacío en `None`, para que una
/// casilla en blanco se persista como NULL y no como cadena vacía.
fn sin_vacio(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|t| !t.is_empty())
}

/// Fija (o borra) el nombre PARTIDO del profesional —los cuatro campos que el
/// RDA de encuentro exige (#603): `given`/`family` por separado—, respetando el
/// ámbito igual que el resto de la administración de personal (`del_ambito` +
/// `cola_del_ambito` excluyen al superadmin y a quien cae fuera de la
/// institución). Cada casilla vacía se guarda como NULL (directiva 20): el
/// productor del RDA distingue «ausente» —anota FALTA— de un dato presente, y un
/// `""` sería un presente falso. Deja intacto el `nombre` completo, que sigue
/// siendo el de login/listados/firma. Devuelve `false` si la persona no existe o
/// cae fuera del ámbito.
pub async fn fijar_nombre_partido(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    nombre1: Option<&str>,
    nombre2: Option<&str>,
    apellido1: Option<&str>,
    apellido2: Option<&str>,
) -> Result<bool, sqlx::Error> {
    if del_ambito(pool, ambito, id).await?.is_none() {
        return Ok(false);
    }
    let Some((cola, inst)) = cola_del_ambito(ambito) else {
        return Ok(false);
    };
    let sql = format!("UPDATE usuario SET nombre1 = ?, nombre2 = ?, apellido1 = ?, apellido2 = ?{cola}");
    let q = sqlx::query(&sql)
        .bind(nombre1.map(str::trim).filter(|t| !t.is_empty()))
        .bind(nombre2.map(str::trim).filter(|t| !t.is_empty()))
        .bind(apellido1.map(str::trim).filter(|t| !t.is_empty()))
        .bind(apellido2.map(str::trim).filter(|t| !t.is_empty()))
        .bind(id);
    match inst {
        Some(i) => q.bind(i),
        None => q,
    }
    .execute(pool)
    .await?;
    Ok(true)
}

// ─────────────────────────────────────────────────────────────────────────────
// Auditoría
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct EventoAuditoria {
    pub id: i64,
    pub fecha_hora: String,
    pub usuario_id: i64,
    pub usuario_nombre: String,
    pub accion: String,
    pub tabla_afectada: String,
    pub registro_id: i64,
    pub paciente_id: Option<i64>,
    /// **Siempre nulo**: la bitácora de este binario no guarda la dirección de
    /// origen. Ausente y no inventada.
    pub ip_address: Option<String>,
    /// **Siempre cierto**, y es un hecho sobre el registro, no un defecto: aquí
    /// solo se anota lo que OCURRIÓ. Un intento fallido de inicio de sesión vive
    /// en `intento_acceso`, que es otra tabla y otro propósito.
    pub exitoso: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaginaAuditoria {
    pub total: i64,
    pub items: Vec<EventoAuditoria>,
}

/// El rastro de accesos del ámbito, paginado.
///
/// El aislamiento va por `institucion_id`, que viaja DENTRO del contenido
/// firmado de cada evento: leerlo con `json_extract` no es un atajo, es la única
/// fuente que existe — la tabla no tiene columna de institución a propósito,
/// para que el encadenado cubra también ese dato.
pub async fn auditoria(
    pool: &crate::Pool,
    ambito: Ambito,
    limite: i64,
    desplazamiento: i64,
) -> Result<PaginaAuditoria, sqlx::Error> {
    let Some((filtro, inst)) =
        filtro_ambito(ambito, "json_extract(a.contenido, '$.institucion_id')")
    else {
        return Ok(PaginaAuditoria { total: 0, items: Vec::new() });
    };
    let total: i64 = {
        let sql = format!("SELECT COUNT(*) FROM acceso a WHERE {filtro}");
        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        if let Some(i) = inst {
            q = q.bind(i);
        }
        q.fetch_one(pool).await?
    };

    let sql = format!(
        "SELECT a.secuencia, \
         json_extract(a.contenido, '$.ts'), \
         json_extract(a.contenido, '$.usuario_id'), \
         COALESCE(u.nombre, ''), \
         json_extract(a.contenido, '$.accion'), \
         json_extract(a.contenido, '$.recurso'), \
         COALESCE(json_extract(a.contenido, '$.objeto.recurso_id'), \
                  json_extract(a.contenido, '$.objeto.paciente_id'), 0), \
         json_extract(a.contenido, '$.objeto.paciente_id') \
         FROM acceso a \
         LEFT JOIN usuario u ON u.id = json_extract(a.contenido, '$.usuario_id') \
         WHERE {filtro} ORDER BY a.secuencia DESC LIMIT ? OFFSET ?"
    );
    let mut q = sqlx::query_as::<_, (i64, String, i64, String, String, String, i64, Option<i64>)>(&sql);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    let filas = q
        .bind(limite.clamp(1, 500))
        .bind(desplazamiento.max(0))
        .fetch_all(pool)
        .await?;

    Ok(PaginaAuditoria {
        total,
        items: filas
            .into_iter()
            .map(
                |(id, ts, usuario_id, nombre, accion, recurso, registro_id, paciente_id)| {
                    EventoAuditoria {
                        id,
                        fecha_hora: ts,
                        usuario_id,
                        usuario_nombre: nombre,
                        accion,
                        tabla_afectada: recurso,
                        registro_id,
                        paciente_id,
                        ip_address: None,
                        exitoso: true,
                    }
                },
            )
            .collect(),
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// **Toda columna de la política dice si se APLICA.**
    ///
    /// Hasta el 2026-08-06 las veintiuna eran de solo escritura: el
    /// administrador movía interruptores y no ocurría nada. Se arregló cinco, se
    /// escribió la tabla de las demás en un doc-comment… y una especificación en
    /// prosa no la puede leer una pantalla. Ahora es un dato, y esta prueba
    /// impide que una columna nueva entre sin declarar qué hace.
    ///
    /// El conjunto de campos sale del propio tipo —serializándolo— y no de una
    /// lista escrita a mano: una lista a mano nace completa y caduca en el
    /// commit siguiente.
    #[test]
    fn toda_columna_de_la_politica_dice_si_se_aplica() {
        let v = serde_json::to_value(Politica {
            id: 0,
            longitud_minima_password: 0,
            requiere_mayuscula: false,
            requiere_minuscula: false,
            requiere_numero: false,
            requiere_caracter_especial: false,
            dias_expiracion_password: 0,
            historial_passwords: 0,
            timeout_sesion_minutos: 0,
            sesiones_simultaneas_permitidas: 0,
            cerrar_otras_sesiones_al_login: false,
            max_intentos_login: 0,
            tiempo_bloqueo_minutos: 0,
            requiere_2fa: false,
            requiere_2fa_admin: false,
            metodo_2fa: String::new(),
            registrar_todos_los_accesos: false,
            retener_logs_dias: 0,
            notificar_login_nuevo_dispositivo: false,
            notificar_cambio_password: false,
            notificar_intentos_fallidos: false,
        })
        .unwrap();
        let del_tipo: std::collections::BTreeSet<String> = v
            .as_object()
            .unwrap()
            .keys()
            .filter(|k| *k != "id")
            .cloned()
            .collect();
        let declaradas: std::collections::BTreeSet<String> =
            POLITICA_APLICADA.iter().map(|(c, _)| (*c).to_string()).collect();
        assert_eq!(
            del_tipo, declaradas,
            "hay columnas de la política sin decir si se aplican, o declaradas y ya inexistentes"
        );
        // CONTROL: alguna se aplica y alguna no. Con todas iguales, la pantalla
        // no tendría nada que distinguir y esta prueba no mediría la diferencia.
        assert!(POLITICA_APLICADA.iter().any(|(_, m)| m.is_none()));
        assert!(POLITICA_APLICADA.iter().any(|(_, m)| m.is_some()));
    }

    /// **El defecto declarado es el que dice la migración, y se puede cambiar.**
    ///
    /// Mide solo el viaje de ida y vuelta por esta capa; que esos dos valores
    /// LLEGUEN a la agenda y a la factura lo miden `agenda::pruebas` y
    /// `facturacion::pruebas`, cada una donde se usa.
    ///
    /// El `prefijo_factura` vale `FAC` y no `FE` desde la migración 0050: ver
    /// ahí por qué cambiar el defecto era parte de enchufar la columna.
    #[tokio::test]
    async fn el_defecto_de_la_configuracion_es_el_declarado_y_se_puede_cambiar() {
        let pool = base().await;

        // El defecto declarado: 20 minutos, prefijo FAC.
        let c = configuracion(&pool, 1).await.unwrap().unwrap();
        assert_eq!(c.duracion_cita_default, 20);
        assert_eq!(c.prefijo_factura, "FAC");

        // Y ahora se cambian los dos, por la MISMA función que usa la pantalla.
        actualizar_configuracion(
            &pool,
            1,
            &ConfiguracionCambio {
                duracion_cita_default: Some(45),
                prefijo_factura: Some("SETP".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .unwrap();
        let c = configuracion(&pool, 1).await.unwrap().unwrap();
        assert_eq!(c.duracion_cita_default, 45);
        assert_eq!(c.prefijo_factura, "SETP");
    }

    /// **La gemela para la configuración.** Sin ella, `CONFIGURACION_APLICADA`
    /// sería prosa: una columna nueva entraría en la tabla, en la pantalla y en
    /// el formulario sin que nadie tuviera que decir si sirve para algo — que
    /// es exactamente como llegaron a existir diecisiete que no servían.
    ///
    /// Se compara contra el TIPO SERIALIZADO y no contra una lista escrita a
    /// mano: la lista se queda atrás, el tipo no.
    #[test]
    fn toda_columna_de_la_configuracion_dice_si_se_aplica() {
        let v = serde_json::to_value(Configuracion {
            id: 0,
            horario_apertura: String::new(),
            horario_cierre: String::new(),
            dias_atencion: Vec::new(),
            duracion_cita_default: 0,
            tiempo_buffer: 0,
            enviar_recordatorios: false,
            horas_anticipacion: 0,
            smtp_host: None,
            smtp_port: None,
            smtp_usuario: None,
            prefijo_factura: String::new(),
            resolucion_facturacion: None,
            rango_desde: None,
            rango_hasta: None,
            permitir_citas_online: false,
            requiere_autorizacion: false,
            dias_maximos_agendamiento: 0,
        })
        .unwrap();
        let del_tipo: std::collections::BTreeSet<String> = v
            .as_object()
            .unwrap()
            .keys()
            .filter(|k| *k != "id")
            .cloned()
            .collect();
        let declaradas: std::collections::BTreeSet<String> =
            CONFIGURACION_APLICADA.iter().map(|(c, _)| (*c).to_string()).collect();
        assert_eq!(
            del_tipo, declaradas,
            "hay columnas de la configuración sin decir si se aplican, o declaradas y ya inexistentes"
        );
        // CONTROL, las dos mitades: alguna se aplica y alguna no.
        assert!(
            CONFIGURACION_APLICADA.iter().any(|(_, m)| m.is_none()),
            "ninguna se aplica: o la medición está mal o la pantalla no debería editar nada"
        );
        assert!(CONFIGURACION_APLICADA.iter().any(|(_, m)| m.is_some()));
    }

    const UNO: Ambito = Ambito::Institucion(1);
    const DOS: Ambito = Ambito::Institucion(2);

    /// Dos instituciones con su admin, y un superadmin que no es de ninguna.
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO institucion (id,nombre,nit) VALUES (1,'Uno','900'),(2,'Dos','901')",
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo)
             VALUES (1,'Admin Uno','a1@x.co','h','admin',1,1),
                    (2,'Medica Uno','m1@x.co','h','medico',1,1),
                    (3,'Admin Dos','a2@x.co','h','admin',2,1),
                    (4,'Dueño','root@x.co','h','superadmin',NULL,1),
                    (5,'Retirada Uno','r1@x.co','h','medico',1,0)",
        ).execute(&pool).await.unwrap();
        pool
    }

    async fn quien(pool: &crate::Pool, id: i64) -> crate::usuario::Usuario {
        crate::usuario::por_id(pool, id).await.unwrap().unwrap()
    }

    // ── La contraseña que un administrador le fija a otro ────────────────────

    /// Una `Politica` con los cuatro requisitos de composición APAGADOS, para
    /// que cada prueba encienda solo el que mide. Se construye a mano y no con
    /// `Default` porque `Politica` no lo tiene: sus veinte columnas salen de la
    /// base, y un `Default` inventaría una postura de seguridad que nadie fijó.
    fn politica_de(longitud: i64) -> Politica {
        Politica {
            id: 1,
            longitud_minima_password: longitud,
            requiere_mayuscula: false,
            requiere_minuscula: false,
            requiere_numero: false,
            requiere_caracter_especial: false,
            dias_expiracion_password: 0,
            historial_passwords: 0,
            timeout_sesion_minutos: 30,
            sesiones_simultaneas_permitidas: 1,
            cerrar_otras_sesiones_al_login: false,
            max_intentos_login: 5,
            tiempo_bloqueo_minutos: 5,
            requiere_2fa: false,
            requiere_2fa_admin: false,
            metodo_2fa: "totp".into(),
            registrar_todos_los_accesos: false,
            retener_logs_dias: 365,
            notificar_login_nuevo_dispositivo: false,
            notificar_cambio_password: false,
            notificar_intentos_fallidos: false,
        }
    }

    /// **La pareja del hallazgo.** El caso rojo es la contraseña VACÍA, que
    /// hasta el 2026-08-06 el producto aceptaba con un 204: la rama que decía
    /// «no cumple el mínimo exigido» colgaba de que Argon2 fallara, y Argon2
    /// hashea `""` sin queja.
    ///
    /// El verde de al lado es lo que impide que esta prueba pase con una regla
    /// que diga que no a todo.
    #[test]
    fn la_contrasena_vacia_no_se_admite_y_una_de_ocho_si() {
        assert!(password_admisible("", None).is_err());
        assert!(password_admisible("a", None).is_err());
        assert!(password_admisible("abcdefg", None).is_err(), "siete < 8");
        assert!(password_admisible("abcdefgh", None).is_ok(), "ocho es el suelo");
    }

    /// El suelo vale también **sin institución de la que leer política**, que es
    /// el caso del `admin` que un superadmin crea con `institucion_id = NULL`.
    /// Y no se rellena con la postura de nadie más: las cuatro exigencias de
    /// composición no se aplican, porque nadie las pidió.
    #[test]
    fn sin_politica_manda_el_suelo_y_solo_el_suelo() {
        assert_eq!(MINIMO_PASSWORD, 8);
        assert!(password_admisible("abcdefgh", None).is_ok());
        // Sin mayúscula, sin dígito y sin signo: pasa igual, porque no hay
        // política que los exija.
        assert!(password_admisible("abcdefghij", None).is_ok());
    }

    /// La institución puede exigir MÁS que el suelo, y esa es la mitad que
    /// convierte `politica_seguridad` en algo que el binario aplica en vez de
    /// una tabla de solo escritura.
    #[test]
    fn la_politica_sube_el_minimo_pero_no_lo_baja() {
        let estricta = politica_de(12);
        assert!(password_admisible("abcdefghijk", Some(&estricta)).is_err(), "once < 12");
        assert!(password_admisible("abcdefghijkl", Some(&estricta)).is_ok());

        // Y al revés: una política que pidiera menos NO puede rebajar el suelo.
        // El `CHECK` del esquema ya lo impide (BETWEEN 8 AND 128), pero la regla
        // no depende de que la base lo sujete.
        let floja = politica_de(3);
        assert!(password_admisible("abc", Some(&floja)).is_err());
        assert!(password_admisible("abcdefgh", Some(&floja)).is_ok());
    }

    /// Los cuatro requisitos de composición, cada uno con su pareja: la
    /// contraseña que le falta ESE carácter y la misma con él.
    #[test]
    fn cada_requisito_de_composicion_rechaza_solo_lo_suyo() {
        let exigiendo = |encender: fn(&mut Politica)| {
            let mut p = politica_de(8);
            encender(&mut p);
            p
        };
        let casos: [(Politica, &str, &str); 4] = [
            (
                exigiendo(|p| p.requiere_mayuscula = true),
                "abcdefgh1!",
                "Abcdefgh1!",
            ),
            (
                exigiendo(|p| p.requiere_minuscula = true),
                "ABCDEFGH1!",
                "ABCDEFGh1!",
            ),
            (
                exigiendo(|p| p.requiere_numero = true),
                "Abcdefgh!",
                "Abcdefgh1!",
            ),
            (
                exigiendo(|p| p.requiere_caracter_especial = true),
                "Abcdefgh1",
                "Abcdefgh1!",
            ),
        ];
        for (p, sin, con) in casos {
            assert!(password_admisible(sin, Some(&p)).is_err(), "«{sin}» debía caer");
            assert!(password_admisible(con, Some(&p)).is_ok(), "«{con}» debía pasar");
            // Y el control: con la política por defecto —que no exige nada— la
            // que caía pasa. Si no, la prueba estaría midiendo la longitud.
            assert!(password_admisible(sin, Some(&politica_de(8))).is_ok());
        }
    }

    /// Se cuentan CARACTERES, no bytes. Siete letras acentuadas son catorce
    /// bytes: con `len()` esta contraseña habría pasado por larga.
    #[test]
    fn el_minimo_cuenta_caracteres_y_no_bytes() {
        let siete = "áéíóúáé";
        assert_eq!(siete.len(), 14, "el caso rojo depende de que ocupe 14 bytes");
        assert_eq!(siete.chars().count(), 7);
        assert!(password_admisible(siete, None).is_err());
        assert!(password_admisible("áéíóúáéí", None).is_ok(), "ocho caracteres");
    }

    /// **El orden: el ámbito antes que la contraseña.** Si se comprobara al
    /// revés, el mensaje «la política de la institución exige…» se respondería
    /// sobre un id que debía dar 404, y con eso se enumera la plantilla del
    /// vecino probando números.
    #[tokio::test]
    async fn la_politica_ajena_no_se_lee_ni_para_validar_una_contrasena() {
        let pool = base().await;
        // La suya: se administra, y trae política.
        assert!(
            politica_para_administrar(&pool, UNO, 2).await.unwrap().is_some(),
            "la médica 2 es de la institución 1"
        );
        // La del vecino: NADA, ni siquiera la política.
        assert!(
            politica_para_administrar(&pool, UNO, 3).await.unwrap().is_none(),
            "el admin 3 es de la institución 2"
        );
        // El superadmin no se administra desde aquí, lo pida quien lo pida.
        assert!(
            politica_para_administrar(&pool, Ambito::Superadmin, 4).await.unwrap().is_none()
        );
        // Y el que no existe se ve igual que el ajeno: 404 en los dos casos.
        assert!(politica_para_administrar(&pool, UNO, 999).await.unwrap().is_none());
    }

    /// Un usuario sin institución sí se administra —lo hace el superadmin— pero
    /// no tiene política que leer. Son los dos niveles de `Option`, y colapsarlos
    /// convertiría «no hay política» en «no existe».
    #[tokio::test]
    async fn el_usuario_sin_institucion_se_administra_y_no_trae_politica() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo)
             VALUES (6,'Suelto','s@x.co','h','admin',NULL,1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let v = politica_para_administrar(&pool, Ambito::Superadmin, 6)
            .await
            .unwrap();
        assert!(v.is_some(), "existe y el superadmin lo administra");
        assert!(v.unwrap().is_none(), "pero no hay política que leer");
    }

    // ── Privilegio: el ascenso ───────────────────────────────────────────────

    /// **Solo un superadmin fabrica otro superadmin.**
    ///
    /// Es la regla que el lado Python tuvo abierta por la mitad: cerró los dos
    /// `PUT` y dejó las dos ALTAS, que son el camino fácil —quien crea la cuenta
    /// elige además la contraseña—, y las pruebas salieron verdes porque
    /// recorrían solo los `PUT`. Aquí la función es UNA y se prueba con sus dos
    /// llamantes; las pruebas de las rutas cubren los caminos HTTP.
    #[tokio::test]
    async fn solo_un_superadmin_fabrica_otro_superadmin() {
        let pool = base().await;
        let admin = quien(&pool, 1).await;
        let dueño = quien(&pool, 4).await;

        assert!(!ascenso_permitido(Some("superadmin"), &admin), "el admin se ascendía");
        assert!(ascenso_permitido(Some("superadmin"), &dueño));

        // Y NO se prohíbe cambiar de rol en general: un admin sube a alguien de
        // auxiliar a enfermería a diario. Cerrar eso sería cerrar lo inusual en
        // vez de lo imposible.
        for rol in ["medico", "enfermeria", "auxiliar_enfermeria", "admin"] {
            assert!(ascenso_permitido(Some(rol), &admin), "el admin no pudo poner rol {rol}");
        }
        // Sin cambio de rol, tampoco estorba.
        assert!(ascenso_permitido(None, &admin));
    }

    /// **El ascenso no se cuela escribiendo el rol con otra caja.**
    ///
    /// La guardia parsea antes de comparar, así que `SUPERADMIN`, `Superadmin` y
    /// ` superadmin ` no son un rol distinto que se escape: no parsean, así que
    /// no son un ascenso — y el escritor los rechaza después con su nombre. La
    /// forma canónica sí se caza.
    #[tokio::test]
    async fn el_ascenso_no_se_cuela_con_otra_caja() {
        let pool = base().await;
        let admin = quien(&pool, 1).await;
        assert!(!ascenso_permitido(Some("superadmin"), &admin), "la forma canónica");
        for variante in ["SUPERADMIN", "Superadmin", "SuperAdmin", " superadmin"] {
            // No parsean, así que no son un ascenso: la guardia los deja pasar y
            // el ALTA los rechaza con 400. Se comprueba que ese segundo filtro
            // existe de verdad, no que la guardia haga su trabajo.
            assert!(RolUsuario::desde_str(variante).is_none(), "«{variante}» parseó");
        }
        // Y la pareja end-to-end: el alta con la variante NO crea nada.
        let n = UsuarioAltaAdmin {
            username: Some("x@x.co".into()),
            email: None,
            password: "x".into(),
            nombre_completo: "X".into(),
            rol: "SUPERADMIN".into(),
            especialidad_id: None,
            registro_medico: None,
            registro_rethus: None,
            nombre1: None,
            nombre2: None,
            apellido1: None,
            apellido2: None,
        };
        assert!(matches!(
            crear_usuario(&pool, 1, &n, "h").await.unwrap(),
            Err(FalloUsuario::RolDesconocido(_))
        ));
        let n_super: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM usuario WHERE rol LIKE '%uperadmin%'")
                .fetch_one(&pool).await.unwrap();
        assert_eq!(n_super, 1, "se creó un superadmin con la caja cambiada");
    }

    /// Se mira el DESTINO y no el origen: degradar a un superadmin no crea
    /// autoridad nueva, y prohibirlo dejaría al dueño sin poder retirar la suya.
    #[tokio::test]
    async fn degradar_a_un_superadmin_no_esta_prohibido() {
        let pool = base().await;
        let dueño = quien(&pool, 4).await;
        assert!(ascenso_permitido(Some("admin"), &dueño));
    }

    /// EL RODEO QUE LA REVISIÓN ENCONTRÓ: sin esta regla, el guardarraíl 3 de
    /// `Ajustes::validar` era decorativo — el delegado no tenía que tocar la
    /// matriz, le bastaba crearse un admin y volver a entrar.
    #[tokio::test]
    async fn un_delegado_no_crea_un_rol_que_pueda_mas_que_el() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) \
             VALUES (77,'Lab','lab77@x.co','h','laboratorista',1,1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let lab = quien(&pool, 77).await;
        assert!(!ascenso_permitido(Some("admin"), &lab), "se fabricaba un admin");
        assert!(!ascenso_permitido(Some("medico"), &lab), "y un médico, que firma");
        assert!(!ascenso_permitido(Some("enfermeria"), &lab));
    }

    #[tokio::test]
    async fn pero_si_crea_a_los_que_no_le_superan() {
        // LA PAREJA, y la que impide que esto se convierta en «nadie crea a
        // nadie»: la gestión delegada tiene que servir para algo.
        let pool = base().await;
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) \
             VALUES (77,'Lab','lab77@x.co','h','laboratorista',1,1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let lab = quien(&pool, 77).await;
        for rol in ["laboratorista", "paciente", "trabajo_social", "bacteriologo"] {
            assert!(ascenso_permitido(Some(rol), &lab), "no pudo crear {rol}");
        }
    }

    #[tokio::test]
    async fn el_admin_sigue_creando_a_todo_su_personal() {
        // El caso legítimo que NO se puede romper, y el que se midió antes de
        // escribir la regla: de los quince roles, el único que tiene algo que
        // `Admin` no tiene es `Superadmin`. Si esto se pone rojo, la regla dejó
        // al administrador sin poder montar su plantilla.
        let pool = base().await;
        let admin = quien(&pool, 1).await;
        for rol in RolUsuario::TODOS {
            let esperado = *rol != RolUsuario::Superadmin;
            assert_eq!(
                ascenso_permitido(Some(rol.como_str()), &admin),
                esperado,
                "el admin con el rol {}",
                rol.como_str()
            );
        }
    }

    // ── Privilegio: el cupo ──────────────────────────────────────────────────

    /// **Reactivar consume un asiento**, y comprobarlo solo al crear deja crecer
    /// sin límite: desactivar, crear, reactivar.
    #[tokio::test]
    async fn reactivar_a_alguien_consume_cupo() {
        let pool = base().await;
        // 5 está inactivo.
        let c = UsuarioCambioAdmin { activo: Some(true), ..Default::default() };
        assert_eq!(
            consumo_de(&pool, UNO, 5, &c).await.unwrap(),
            // El 5 es médico, así que reactivarlo entra en LOS DOS cupos.
            Some(Consumo { reactiva: true, entra_al_cupo_de_medicos: true })
        );

        // Y las tres formas de NO entrar al conjunto contado:
        let ya_activo = UsuarioCambioAdmin { activo: Some(true), ..Default::default() };
        assert_eq!(
            consumo_de(&pool, UNO, 2, &ya_activo).await.unwrap(),
            Some(Consumo { reactiva: false, entra_al_cupo_de_medicos: false }),
            "el que ya estaba activo no ocupa un asiento nuevo"
        );
        let apagar = UsuarioCambioAdmin { activo: Some(false), ..Default::default() };
        assert_eq!(
            consumo_de(&pool, UNO, 2, &apagar).await.unwrap(),
            Some(Consumo { reactiva: false, entra_al_cupo_de_medicos: false }),
            "desactivar LIBERA: estorbarlo sería cerrar por cerrar"
        );
        let solo_nombre = UsuarioCambioAdmin {
            nombre_completo: Some("Otro".into()),
            ..Default::default()
        };
        assert_eq!(
            consumo_de(&pool, UNO, 5, &solo_nombre).await.unwrap(),
            Some(Consumo { reactiva: false, entra_al_cupo_de_medicos: false }),
            "guardar sin tocar `activo` no es crecer"
        );
    }

    /// **El ASCENSO consume asiento de médico y NO de usuario**, que es la
    /// distinción entera. El 2 está activo y es médico; el 6 está activo y es
    /// auxiliar.
    ///
    /// Sin la segunda mitad —`de_usuario: false`— una IPS con el cupo de
    /// usuarios justo no podría ascender a nadie, que es rechazar la operación
    /// normal de un cliente legítimo (directiva 21).
    #[tokio::test]
    async fn ascender_a_medico_ocupa_asiento_de_medico_y_no_de_usuario() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo)
             VALUES (6,'Auxiliar','aux@x.co','h','auxiliar_enfermeria',1,1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let ascender = UsuarioCambioAdmin { rol: Some("medico".into()), ..Default::default() };
        let c = consumo_de(&pool, UNO, 6, &ascender).await.unwrap().unwrap();
        assert_eq!(
            c,
            Consumo { reactiva: false, entra_al_cupo_de_medicos: true },
            "ascender a médico no entraba en ningún cupo: era el rodeo del tope"
        );
        assert_eq!(
            c.asiento(),
            Some(crate::licencia::Asiento { de_usuario: false, de_medico: true })
        );

        // LA PAREJA 1 — bajar de médico a auxiliar LIBERA, no se frena.
        let bajar = UsuarioCambioAdmin { rol: Some("auxiliar_enfermeria".into()), ..Default::default() };
        let b = consumo_de(&pool, UNO, 2, &bajar).await.unwrap().unwrap();
        assert!(!b.entra_al_cupo_de_medicos);
        assert_eq!(b.asiento(), None, "ni siquiera se le pregunta a la licencia");

        // LA PAREJA 2 — quien YA era médico y sigue activo no vuelve a pagar.
        let sigue = UsuarioCambioAdmin { rol: Some("medico".into()), ..Default::default() };
        let s = consumo_de(&pool, UNO, 2, &sigue).await.unwrap().unwrap();
        assert!(!s.entra_al_cupo_de_medicos, "cobró dos veces el mismo asiento");

        // Y un cambio que no toca ni rol ni actividad no consulta la licencia:
        // si lo hiciera, una licencia vencida bloquearía corregir un dedazo.
        let dedazo = UsuarioCambioAdmin { email: Some("bien@x.co".into()), ..Default::default() };
        assert_eq!(consumo_de(&pool, UNO, 6, &dedazo).await.unwrap().unwrap().asiento(), None);
    }

    /// **La cola del ámbito, medida DIRECTAMENTE, y hay que decir por qué.**
    ///
    /// La prueba de integración de esta salvaguarda —`el_update_de_personal_no_
    /// alcanza_la_fila_ajena_por_si_solo`, en `rutas.rs`— **no discrimina**, y
    /// se comprobó por mutación el 2026-08-06: quitándole el filtro a esta cola,
    /// aquella sigue verde. No es un defecto suyo, es que el camino no llega:
    /// `del_ambito` corre ANTES y devuelve `None` para la fila ajena, así que el
    /// `UPDATE` ni se ejecuta. Mientras la hebilla funcione, el cinturón es
    /// inalcanzable — que es literalmente lo que significa «segunda vuelta de
    /// llave», y por eso aquella mide `del_ambito` y no esto.
    ///
    /// Lo que sí se puede medir es la REGLA, aquí, con sus dos mitades: que la
    /// cola del ámbito de institución acote por institución, y que la del
    /// superadmin —que sí ve a todos— siga excluyendo al superadmin por ROL.
    #[test]
    fn la_cola_del_ambito_acota_por_institucion_y_excluye_al_superadmin() {
        let (cola, inst) = cola_del_ambito(Ambito::Institucion(7)).expect("debe administrar");
        assert!(cola.contains("institucion_id = ?"), "no acota por institución: {cola}");
        assert!(cola.contains("rol <> 'superadmin'"), "alcanza al superadmin: {cola}");
        assert_eq!(inst, Some(7), "el id de la institución no se enlaza");

        // El superadmin ve a todos: NO acota por institución —hacerlo lo dejaría
        // sin administrar a nadie— pero sigue sin tocar a otro superadmin.
        let (cola, inst) = cola_del_ambito(Ambito::Superadmin).expect("debe administrar");
        assert!(!cola.contains("institucion_id"), "acotó al superadmin: {cola}");
        assert!(cola.contains("rol <> 'superadmin'"), "un superadmin administra a otro: {cola}");
        assert_eq!(inst, None);

        // Y quien no pertenece a ninguna no administra a nadie: `None`, que el
        // llamante traduce a 404 SIN ejecutar el UPDATE.
        assert!(cola_del_ambito(Ambito::Ninguno).is_none());
    }

    #[tokio::test]
    async fn el_cupo_de_un_usuario_ajeno_no_se_calcula() {
        let pool = base().await;
        let c = UsuarioCambioAdmin { activo: Some(true), ..Default::default() };
        assert_eq!(consumo_de(&pool, DOS, 5, &c).await.unwrap(), None);
        assert_eq!(consumo_de(&pool, Ambito::Ninguno, 5, &c).await.unwrap(), None);
    }

    // ── Aislamiento ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn el_listado_de_usuarios_se_acota_y_esconde_al_superadmin() {
        let pool = base().await;
        let f = FiltroUsuarios::default();

        let u1 = usuarios(&pool, UNO, &f, &[]).await.unwrap();
        let nombres: Vec<&str> = u1.iter().map(|u| u.nombre_completo.as_str()).collect();
        assert!(nombres.contains(&"Admin Uno") && nombres.contains(&"Medica Uno"));
        assert!(!nombres.contains(&"Admin Dos"), "vio al admin de la otra institución");
        assert!(
            !nombres.contains(&"Dueño"),
            "EL SUPERADMIN NO SALE: no es personal del prestador, y enseñarlo le da al \
             administrador de un cliente el correo del dueño del sistema"
        );
        assert_eq!(usuarios(&pool, DOS, &f, &[]).await.unwrap().len(), 1);
        assert!(usuarios(&pool, Ambito::Ninguno, &f, &[]).await.unwrap().is_empty());
        // Y el superadmin ve a los de las dos, pero tampoco a sí mismo.
        let todos = usuarios(&pool, Ambito::Superadmin, &f, &[]).await.unwrap();
        assert_eq!(todos.len(), 4);
        assert!(!todos.iter().any(|u| u.nombre_completo == "Dueño"));
    }

    /// El `%` de una búsqueda es un DATO, no un comodín del atacante: el patrón
    /// se enlaza y los metacaracteres se escapan.
    /// El `%` de una búsqueda es un DATO, y **buscarlo encuentra a quien lo
    /// lleva en el nombre**.
    ///
    /// La versión anterior solo comprobaba que `%` no devolviera a todo el
    /// mundo, y eso salía verde con el escape borrado —el patrón va enlazado, así
    /// que el comodín nunca amplía el conjunto—. Sin `ESCAPE '\'` el escapado
    /// era inerte y buscar un literal no encontraba nada: es esa mitad la que
    /// discrimina.
    #[tokio::test]
    async fn el_comodin_de_la_busqueda_es_un_dato_y_se_puede_buscar() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo)
             VALUES (7,'Cien %','pct@x.co','h','medico',1,1),
                    (8,'Guion_bajo','gb@x.co','h','medico',1,1)",
        ).execute(&pool).await.unwrap();

        let buscar = |q: &str| {
            let f = FiltroUsuarios { q: Some(q.into()), ..Default::default() };
            let pool = pool.clone();
            async move { usuarios(&pool, UNO, &f, &[]).await.unwrap() }
        };
        // Buscar el literal encuentra SOLO a quien lo lleva.
        let r = buscar("%").await;
        assert_eq!(r.len(), 1, "buscar «%» tiene que encontrar a «Cien %»: {:?}",
                   r.iter().map(|u| &u.nombre_completo).collect::<Vec<_>>());
        assert_eq!(r[0].nombre_completo, "Cien %");
        let r = buscar("_").await;
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].nombre_completo, "Guion_bajo");

        // Y la pareja: una búsqueda normal sigue funcionando.
        assert_eq!(buscar("Medica").await.len(), 1);
        assert_eq!(buscar("nadie").await.len(), 0);
    }

    #[tokio::test]
    async fn los_filtros_de_rol_y_estado_discriminan() {
        let pool = base().await;
        let solo_medicos = FiltroUsuarios { rol: Some("medico".into()), ..Default::default() };
        assert_eq!(usuarios(&pool, UNO, &solo_medicos, &[]).await.unwrap().len(), 2);
        let inactivos = FiltroUsuarios { activo: Some(false), ..Default::default() };
        let v = usuarios(&pool, UNO, &inactivos, &[]).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].nombre_completo, "Retirada Uno");
    }

    /// `cuenta_bloqueada` se DERIVA del freno, no de una columna. Aquí no se
    /// bloquean cuentas: se retrasa el siguiente intento.
    #[tokio::test]
    async fn la_cuenta_bloqueada_sale_del_freno() {
        let pool = base().await;
        let f = FiltroUsuarios::default();
        let sin = usuarios(&pool, UNO, &f, &[]).await.unwrap();
        assert!(sin.iter().all(|u| !u.cuenta_bloqueada));

        let con = usuarios(&pool, UNO, &f, &["m1@x.co".to_string()]).await.unwrap();
        let medica = con.iter().find(|u| u.username == "m1@x.co").unwrap();
        assert!(medica.cuenta_bloqueada);
        assert!(con.iter().filter(|u| u.cuenta_bloqueada).count() == 1, "solo esa");
    }

    /// **La derivación usa la REGLA, no una consulta parecida.**
    ///
    /// Los tres casos que la versión anterior fallaba, y el que sí tiene que
    /// salir. Con la consulta vieja —`COUNT(*) >= 5` de las últimas 24 h por
    /// correo— los tres primeros salían «bloqueados».
    #[tokio::test]
    async fn el_freno_derivado_es_el_freno_de_verdad() {
        let pool = base().await;
        let ahora = chrono::Utc::now();
        let hace = |m: i64| (ahora - chrono::Duration::minutes(m))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let anotar = |email: &'static str, origen: &'static str, cuando: String, exito: i64| {
            let pool = pool.clone();
            async move {
                sqlx::query(
                    "INSERT INTO intento_acceso (email,origen,momento,exito) VALUES (?,?,?,?)",
                ).bind(email).bind(origen).bind(cuando).bind(exito)
                .execute(&pool).await.unwrap();
            }
        };

        // 1. Se equivocó cinco veces y a la sexta ENTRÓ. No está frenado, y es
        //    el caso normal: `anotar` no borra los fallos al acertar.
        for m in [50, 49, 48, 47, 46] { anotar("a1@x.co", "1.1.1.1", hace(m), 0).await; }
        anotar("a1@x.co", "1.1.1.1", hace(45), 1).await;

        // 2. Cinco fallos, pero de hace rato: la espera ya caducó.
        for m in [400, 399, 398, 397, 396] { anotar("m1@x.co", "2.2.2.2", hace(m), 0).await; }

        // 3. Cinco fallos repartidos entre CINCO orígenes: la regla es por par,
        //    así que ninguno llega al umbral.
        for (i, o) in ["3.3.3.1", "3.3.3.2", "3.3.3.3", "3.3.3.4", "3.3.3.5"].iter().enumerate() {
            sqlx::query("INSERT INTO intento_acceso (email,origen,momento,exito) VALUES (?,?,?,0)")
                .bind("r1@x.co").bind(*o).bind(hace(i as i64 + 1))
                .execute(&pool).await.unwrap();
        }

        let frenados = correos_frenados(&pool, ahora).await.unwrap();
        assert!(
            frenados.is_empty(),
            "ninguno de los tres está frenado de verdad, y salieron: {frenados:?}"
        );

        // 4. LA PAREJA: cinco fallos seguidos, mismo origen, ahora mismo. Ese sí.
        for s in 1..=5 {
            sqlx::query("INSERT INTO intento_acceso (email,origen,momento,exito) VALUES (?,?,?,0)")
                .bind("a2@x.co").bind("4.4.4.4")
                .bind((ahora - chrono::Duration::seconds(s)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
                .execute(&pool).await.unwrap();
        }
        assert_eq!(correos_frenados(&pool, ahora).await.unwrap(), vec!["a2@x.co".to_string()]);
    }

    /// **Reponer el freno LIMPIA la espera SIN borrar la evidencia (#539).**
    ///
    /// Antes, «desbloquear» hacía `DELETE ... exito = 0` y se llevaba la única
    /// prueba de que a la cuenta la estuvieron probando (`intento_acceso` es ese
    /// único registro). Ahora anota una reposición: la espera se limpia y las
    /// filas de fallos se quedan.
    ///
    /// La pareja es la segunda mitad —sin ella pasaría igual un `quitar_freno`
    /// que abriera la puerta para siempre—: los fallos POSTERIORES a la
    /// reposición vuelven a frenar. Reponer no es un pase permanente.
    #[tokio::test]
    async fn reponer_el_freno_limpia_la_espera_sin_borrar_la_evidencia() {
        let pool = base().await;
        let t0 = chrono::Utc::now();

        // Cinco fallos recientes (la espera base son 15 s desde el último): sin
        // reposición, esta cuenta está frenada en t0.
        for i in 0..5 {
            crate::freno::anotar(
                &pool, "m1@x.co", "1.1.1.1", false,
                t0 - chrono::Duration::seconds(10) + chrono::Duration::seconds(i),
            ).await.unwrap();
        }
        assert!(
            crate::freno::estado(&pool, "m1@x.co", "1.1.1.1", t0).await.unwrap().frenado(),
            "cinco fallos recientes tenían que frenar antes de reponer"
        );

        quitar_freno(&pool, "m1@x.co").await.unwrap();

        // 1) La evidencia sigue entera: los cinco fallos NO se borraron.
        let fallos: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM intento_acceso WHERE email = 'm1@x.co' AND exito = 0",
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(fallos, 5, "la reposición borró la evidencia del ensayo");

        // 2) Y la espera quedó limpia, por las DOS vías que consultan la regla:
        // el inicio de sesión (`estado`) y la lista del admin (`correos_frenados`).
        assert!(
            !crate::freno::estado(&pool, "m1@x.co", "1.1.1.1", t0 + chrono::Duration::seconds(1))
                .await.unwrap().frenado(),
            "tras reponer, la cuenta seguía frenada en el inicio de sesión"
        );
        assert!(
            correos_frenados(&pool, t0 + chrono::Duration::seconds(1)).await.unwrap().is_empty(),
            "tras reponer, la cuenta seguía en la lista de frenados del admin"
        );

        // LA PAREJA: fallos posteriores a la reposición SÍ vuelven a frenar.
        for i in 0..5 {
            crate::freno::anotar(
                &pool, "m1@x.co", "1.1.1.1", false,
                t0 + chrono::Duration::seconds(6) + chrono::Duration::seconds(i),
            ).await.unwrap();
        }
        let luego = t0 + chrono::Duration::seconds(11);
        assert!(
            crate::freno::estado(&pool, "m1@x.co", "1.1.1.1", luego).await.unwrap().frenado(),
            "reponer se volvió un pase permanente: los fallos nuevos no frenaron"
        );
        assert_eq!(
            correos_frenados(&pool, luego).await.unwrap(),
            vec!["m1@x.co".to_string()],
            "los fallos nuevos no volvieron a aparecer en la lista del admin"
        );
    }

    // ── El superadmin no se administra desde aquí ────────────────────────────

    /// **UN SUPERADMIN CON INSTITUCIÓN NO ES ADMINISTRABLE POR ESE ADMIN.**
    ///
    /// Nada en el esquema impide esa fila —`institucion_id` no tiene `CHECK`— y
    /// la primera versión de este módulo decidía con la COLUMNA mientras el
    /// listado lo excluía por el ROL: quedaba invisible en la lista y escribible
    /// por el admin de su institución, que podía cambiarle el correo,
    /// restablecerle la contraseña y entrar como dueño del sistema. Lo cazó la
    /// revisión de esta rama.
    #[tokio::test]
    async fn un_superadmin_con_institucion_no_se_administra() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo)
             VALUES (10,'Infiltrado','inf@x.co','h','superadmin',1,1)",
        ).execute(&pool).await.unwrap();

        // No sale en el listado (ya lo hacía) …
        let f = FiltroUsuarios::default();
        assert!(!usuarios(&pool, UNO, &f, &[]).await.unwrap().iter().any(|u| u.id == 10));

        // … y AHORA tampoco se escribe.
        let c = UsuarioCambioAdmin { email: Some("mio@atacante.co".into()), ..Default::default() };
        assert_eq!(
            actualizar_usuario(&pool, UNO, 10, &c).await.unwrap(),
            Err(FalloUsuario::NoExiste)
        );
        assert!(!restablecer_password(&pool, UNO, 10, "hash-atacante").await.unwrap());
        assert!(!desactivar_usuario(&pool, UNO, 10).await.unwrap());
        assert_eq!(consumo_de(&pool, UNO, 10, &UsuarioCambioAdmin::default()).await.unwrap(), None);

        let (email, hash): (String, String) =
            sqlx::query_as("SELECT email, password_hash FROM usuario WHERE id = 10")
                .fetch_one(&pool).await.unwrap();
        assert_eq!(email, "inf@x.co", "le cambiaron el correo al superadmin");
        assert_eq!(hash, "h", "le cambiaron la contraseña al superadmin");

        // Y ni siquiera OTRO superadmin lo administra por aquí: esa decisión
        // tiene su propia ruta.
        assert!(!restablecer_password(&pool, Ambito::Superadmin, 10, "x").await.unwrap());

        // LA PAREJA: un usuario normal de la misma institución SÍ se administra.
        assert!(restablecer_password(&pool, UNO, 2, "hash-nuevo").await.unwrap());
    }

    /// El alta por la ruta de administración **no fabrica superadmins**, ni para
    /// un superadmin: recibe una institución obligatoria, y un superadmin CON
    /// institución es exactamente la fila que rompe el aislamiento.
    #[tokio::test]
    async fn la_ruta_de_admin_no_da_de_alta_superadmins() {
        let pool = base().await;
        let n = UsuarioAltaAdmin {
            username: Some("nuevo@x.co".into()),
            email: None,
            password: "x".into(),
            nombre_completo: "Nuevo".into(),
            rol: "superadmin".into(),
            especialidad_id: None,
            registro_medico: None,
            registro_rethus: None,
            nombre1: None,
            nombre2: None,
            apellido1: None,
            apellido2: None,
        };
        assert_eq!(
            crear_usuario(&pool, 1, &n, "h").await.unwrap(),
            Err(FalloUsuario::AscensoProhibido)
        );
        let n_super: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM usuario WHERE rol = 'superadmin'")
                .fetch_one(&pool).await.unwrap();
        assert_eq!(n_super, 1, "solo el que ya había");
    }

    // ── Las dos escrituras que echan a alguien ───────────────────────────────

    /// Dar de baja **revoca las sesiones**. Sin eso, quien acaba de ser dado de
    /// baja sigue trabajando con la cookie que ya tenía.
    #[tokio::test]
    async fn dar_de_baja_revoca_las_sesiones() {
        let pool = base().await;
        let antes: i64 = sqlx::query_scalar("SELECT token_version FROM usuario WHERE id = 2")
            .fetch_one(&pool).await.unwrap();
        assert!(desactivar_usuario(&pool, UNO, 2).await.unwrap());
        let (activo, despues): (bool, i64) =
            sqlx::query_as("SELECT activo, token_version FROM usuario WHERE id = 2")
                .fetch_one(&pool).await.unwrap();
        assert!(!activo);
        assert!(despues > antes, "la cookie vieja sigue valiendo");
        // Y la fila NO se borra: su id está en historias firmadas y en la bitácora.
        let existe: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM usuario WHERE id = 2")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(existe, 1);
    }

    #[tokio::test]
    async fn no_se_da_de_baja_a_un_usuario_ajeno() {
        let pool = base().await;
        assert!(!desactivar_usuario(&pool, DOS, 2).await.unwrap());
        assert!(!desactivar_usuario(&pool, Ambito::Ninguno, 2).await.unwrap());
        let activo: bool = sqlx::query_scalar("SELECT activo FROM usuario WHERE id = 2")
            .fetch_one(&pool).await.unwrap();
        assert!(activo, "se desactivó igual");
    }

    /// Restablecer obliga a cambiarla, revoca las sesiones y REPONE el freno
    /// —sin borrar la evidencia (#539): la contraseña nueva no serviría hasta
    /// pasar la espera, y reponer la limpia dejando el rastro del ensayo—.
    #[tokio::test]
    async fn restablecer_obliga_a_cambiar_revoca_y_repone_el_freno() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO intento_acceso (email,origen,momento,exito)
             VALUES ('m1@x.co','1.1.1.1','2026-08-06T10:00:00Z',0)",
        ).execute(&pool).await.unwrap();
        let antes: i64 = sqlx::query_scalar("SELECT token_version FROM usuario WHERE id = 2")
            .fetch_one(&pool).await.unwrap();

        assert!(restablecer_password(&pool, UNO, 2, "hash-nuevo").await.unwrap());

        let (hash, exige, ver): (String, bool, i64) = sqlx::query_as(
            "SELECT password_hash, requiere_cambio_password, token_version FROM usuario WHERE id = 2",
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(hash, "hash-nuevo");
        assert!(exige, "quien la fijó la conoce: el usuario tiene que cambiarla");
        assert!(ver > antes, "no se echó a quien estaba dentro");
        // La evidencia del ensayo se queda...
        let intentos: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM intento_acceso WHERE email = 'm1@x.co'")
                .fetch_one(&pool).await.unwrap();
        assert_eq!(intentos, 1, "restablecer borró la evidencia del ensayo");
        // ...y la espera se limpió con una reposición, que es lo que deja entrar
        // con la contraseña nueva.
        let repuesto: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM reposicion_freno WHERE email = 'm1@x.co'")
                .fetch_one(&pool).await.unwrap();
        assert_eq!(repuesto, 1, "restablecer no repuso el freno");
    }

    #[tokio::test]
    async fn no_se_restablece_la_contrasena_de_un_ajeno() {
        let pool = base().await;
        assert!(!restablecer_password(&pool, DOS, 2, "mio").await.unwrap());
        let hash: String = sqlx::query_scalar("SELECT password_hash FROM usuario WHERE id = 2")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(hash, "h", "le cambiaron la contraseña al usuario del vecino");
    }

    // ── Alta ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn el_alta_valida_lo_suyo() {
        let pool = base().await;
        let base_alta = |rol: &str| UsuarioAltaAdmin {
            username: Some("nueva@x.co".into()),
            email: None,
            password: "x".into(),
            nombre_completo: "Nueva".into(),
            rol: rol.into(),
            especialidad_id: None,
            registro_medico: None,
            registro_rethus: None,
            nombre1: None,
            nombre2: None,
            apellido1: None,
            apellido2: None,
        };
        assert!(matches!(
            crear_usuario(&pool, 1, &base_alta("teletransportista"), "h").await.unwrap(),
            Err(FalloUsuario::RolDesconocido(_))
        ));

        let mut n = base_alta("medico");
        n.username = None;
        assert!(matches!(
            crear_usuario(&pool, 1, &n, "h").await.unwrap(),
            Err(FalloUsuario::Invalido(_))
        ));

        let mut n = base_alta("medico");
        n.especialidad_id = Some(9999);
        assert!(matches!(
            crear_usuario(&pool, 1, &n, "h").await.unwrap(),
            Err(FalloUsuario::Invalido(_))
        ));

        // Correo repetido.
        let mut n = base_alta("medico");
        n.username = Some("m1@x.co".into());
        assert!(matches!(
            crear_usuario(&pool, 1, &n, "h").await.unwrap(),
            Err(FalloUsuario::Invalido(_))
        ));

        // Y la pareja: la buena entra, activa y sin exigir cambio de contraseña.
        let id = crear_usuario(&pool, 1, &base_alta("medico"), "h").await.unwrap().unwrap();
        let (activo, exige, inst): (bool, bool, i64) = sqlx::query_as(
            "SELECT activo, requiere_cambio_password, institucion_id FROM usuario WHERE id = ?",
        ).bind(id).fetch_one(&pool).await.unwrap();
        assert!(activo && !exige);
        assert_eq!(inst, 1, "la institución sale del ADMIN, nunca del cuerpo");
    }

    /// `email` gana sobre `username`: son el mismo dato con dos nombres, y tener
    /// dos formas de escribirlo es de la compatibilidad, no del modelo.
    #[test]
    fn el_identificador_es_uno_solo() {
        let alta = |u: Option<&str>, e: Option<&str>| UsuarioAltaAdmin {
            username: u.map(str::to_string),
            email: e.map(str::to_string),
            password: "x".into(),
            nombre_completo: "N".into(),
            rol: "medico".into(),
            especialidad_id: None,
            registro_medico: None,
            registro_rethus: None,
            nombre1: None,
            nombre2: None,
            apellido1: None,
            apellido2: None,
        };
        assert_eq!(alta(Some("u@x.co"), None).identificador(), Some("u@x.co"));
        assert_eq!(alta(None, Some("e@x.co")).identificador(), Some("e@x.co"));
        assert_eq!(alta(Some("u@x.co"), Some("e@x.co")).identificador(), Some("e@x.co"));
        assert_eq!(alta(Some("   "), None).identificador(), None, "en blanco no vale");
        assert_eq!(alta(None, None).identificador(), None);
    }

    // ── Configuración y política ─────────────────────────────────────────────

    /// Se crean con sus defectos al LEER, así que la pantalla nunca recibe una
    /// configuración a medias.
    #[tokio::test]
    async fn la_configuracion_nace_con_sus_defectos() {
        let pool = base().await;
        let c = configuracion(&pool, 1).await.unwrap().unwrap();
        assert_eq!(c.horario_apertura, "07:00");
        assert_eq!(c.dias_atencion.len(), 5, "de lunes a viernes");
        assert_eq!(c.duracion_cita_default, 20);
        assert!(configuracion(&pool, 999).await.unwrap().is_none(), "institución inexistente");

        let p = politica(&pool, 1).await.unwrap().unwrap();
        assert_eq!(p.longitud_minima_password, 8);
        assert!(p.requiere_mayuscula && p.requiere_minuscula && p.requiere_numero);
        assert!(
            p.registrar_todos_los_accesos,
            "el rastro de accesos viene ENCENDIDO: es lo que la Ley 1581 espera"
        );
        assert_eq!(p.metodo_2fa, "NINGUNO");
    }

    #[tokio::test]
    async fn guardar_un_campo_no_borra_los_demas() {
        let pool = base().await;
        configuracion(&pool, 1).await.unwrap();
        let solo_horario = ConfiguracionCambio {
            horario_apertura: Some("06:30".into()),
            ..Default::default()
        };
        assert!(actualizar_configuracion(&pool, 1, &solo_horario).await.unwrap().is_ok());
        let c = configuracion(&pool, 1).await.unwrap().unwrap();
        assert_eq!(c.horario_apertura, "06:30");
        assert_eq!(c.horario_cierre, "18:00", "lo que no vino se borró");
        // `FAC` y no `FE` desde la migración 0050: el prefijo empezó a
        // aplicarse de verdad, y su defecto pasó a decir lo que este producto
        // ha escrito siempre en una factura. Ver la migración.
        assert_eq!(c.prefijo_factura, "FAC");
        assert_eq!(c.dias_atencion.len(), 5);
    }

    #[tokio::test]
    async fn una_hora_mal_escrita_se_rechaza() {
        let pool = base().await;
        for mala in ["25:00", "las siete", "7", ""] {
            let c = ConfiguracionCambio {
                horario_apertura: Some(mala.into()),
                ..Default::default()
            };
            assert!(
                actualizar_configuracion(&pool, 1, &c).await.unwrap().is_err(),
                "«{mala}» se aceptó como hora"
            );
        }
        // La pareja: las buenas pasan.
        for buena in ["07:00", "23:59", "00:00"] {
            let c = ConfiguracionCambio {
                horario_apertura: Some(buena.into()),
                ..Default::default()
            };
            assert!(actualizar_configuracion(&pool, 1, &c).await.unwrap().is_ok(), "{buena}");
        }
    }

    /// El rango de la resolución de facturación no puede ir al revés. Lo rechaza
    /// el `CHECK` de la migración, que es donde la regla vive una sola vez.
    #[tokio::test]
    async fn un_rango_de_facturacion_al_reves_no_se_guarda() {
        let pool = base().await;
        configuracion(&pool, 1).await.unwrap();
        let c = ConfiguracionCambio {
            rango_desde: Some(500),
            rango_hasta: Some(100),
            ..Default::default()
        };
        assert!(actualizar_configuracion(&pool, 1, &c).await.is_err());
        // La pareja: en orden, sí.
        let c = ConfiguracionCambio {
            rango_desde: Some(100),
            rango_hasta: Some(500),
            ..Default::default()
        };
        assert!(actualizar_configuracion(&pool, 1, &c).await.unwrap().is_ok());
    }

    // ── Maestros ─────────────────────────────────────────────────────────────

    /// No se borra lo que alguien está usando, y el mensaje dice cuántos.
    #[tokio::test]
    async fn no_se_borra_una_especialidad_en_uso() {
        let pool = base().await;
        let id = crear_especialidad(&pool, "Cirugía de mano").await.unwrap().unwrap();
        sqlx::query("UPDATE usuario SET especialidad_id = ? WHERE id = 2")
            .bind(id).execute(&pool).await.unwrap();

        let e = borrar_especialidad(&pool, id).await.unwrap().unwrap_err();
        assert!(e.mensaje().contains('1'), "dice cuántos la usan: {}", e.mensaje());

        // La pareja: liberada, sí se borra.
        sqlx::query("UPDATE usuario SET especialidad_id = NULL WHERE id = 2")
            .execute(&pool).await.unwrap();
        assert!(borrar_especialidad(&pool, id).await.unwrap().unwrap());
    }

    /// Lo mínimo que hace falta para dar de alta una entidad. Los siete de
    /// contacto van vacíos a propósito: son opcionales, y una prueba que los
    /// llenara no mediría el caso normal.
    fn entidad_minima(nit: &str, nombre: &str, codigo_eps: &str) -> EntidadNueva {
        EntidadNueva {
            nit: nit.into(),
            nombre: nombre.into(),
            codigo_eps: codigo_eps.into(),
            codigo_interno: None,
            direccion: None,
            telefono: None,
            fax: None,
            contacto: None,
            email: None,
            cod_municipio: None,
        }
    }

    #[tokio::test]
    async fn no_se_borra_una_entidad_referenciada() {
        let pool = base().await;
        let id = crear_entidad(&pool, &entidad_minima("800", "EPS Uno", "E1"))
            .await.unwrap().unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,
                                   fecha_nacimiento,sexo,institucion_id,entidad_id)
             VALUES (10,'CC','1','A','B','1990-01-01','F',1,?)",
        ).bind(id).execute(&pool).await.unwrap();

        assert!(borrar_entidad(&pool, id).await.unwrap().is_err());
        sqlx::query("UPDATE paciente SET entidad_id = NULL").execute(&pool).await.unwrap();
        assert!(borrar_entidad(&pool, id).await.unwrap().unwrap());
    }

    #[tokio::test]
    async fn el_nit_de_una_entidad_no_se_edita() {
        let pool = base().await;
        let id = crear_entidad(&pool, &entidad_minima("800", "EPS", "E1"))
            .await.unwrap().unwrap();
        actualizar_entidad(&pool, id, &EntidadCambio {
            nombre: Some("EPS Nueva".into()),
            ..Default::default()
        }).await.unwrap().unwrap();
        let e = &entidades(&pool).await.unwrap()[0];
        assert_eq!(e.nombre, "EPS Nueva");
        assert_eq!(e.nit, "800", "el NIT identifica ante la DIAN: no se toca");
        assert_eq!(e.codigo_eps, "E1", "y lo que no vino, tampoco");
    }

    /// **EL MUNICIPIO SE COMPRUEBA CONTRA DIVIPOLA**, con sus dos listas.
    ///
    /// Es la mitad del port de `terceros` que no es «una columna más»: el VFP
    /// valida su `CODCIUDAD` contra la tabla `ciudades` y avisa «el código de la
    /// ciudad no existe». Portar el campo sin portar esa comprobación habría
    /// dejado guardar un municipio inventado, que después viaja al RIPS.
    ///
    /// Va en la capa de DATOS y no en el manejador porque son dos puertas —el
    /// panel y `POST /api/admin/entidades`—, y la prueba entra por la misma
    /// función que usan las dos.
    #[tokio::test]
    async fn un_municipio_que_no_esta_en_divipola_no_se_guarda() {
        let pool = base().await;
        // El artefacto, no un supuesto: se siembra un municipio REAL con su cero
        // a la izquierda, que es el caso que más se rompe.
        sqlx::query(
            "INSERT INTO departamento (codigo, nombre) VALUES ('05','Antioquia')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO municipio (codigo, nombre, departamento, tipo, en_minsalud) \
             VALUES ('05001','Medellín','05','Municipio',1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        // ROJO: un código con la forma correcta y que no existe.
        let malo = crear_entidad(&pool, &EntidadNueva {
            cod_municipio: Some("99999".into()),
            ..entidad_minima("800", "EPS Uno", "E1")
        })
        .await
        .unwrap()
        .unwrap_err();
        assert!(malo.mensaje().contains("99999"), "{}", malo.mensaje());
        assert!(malo.mensaje().contains("DIVIPOLA"), "{}", malo.mensaje());

        // VERDE 1: el real entra.
        crear_entidad(&pool, &EntidadNueva {
            cod_municipio: Some("05001".into()),
            ..entidad_minima("801", "EPS Dos", "E2")
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(entidades(&pool).await.unwrap()[0].cod_municipio.as_deref(), Some("05001"));

        // VERDE 2, y es la que impide el sesgo a cerrar: el municipio es
        // OPCIONAL. Sin él la entidad se da de alta igual.
        crear_entidad(&pool, &entidad_minima("802", "EPS Tres", "E3"))
            .await
            .unwrap()
            .unwrap();
        let todas = entidades(&pool).await.unwrap();
        assert_eq!(todas.len(), 2, "dos altas buenas de tres intentos");
        assert!(
            !todas.iter().any(|x| x.nit == "800"),
            "y la del municipio inventado NO se guardó a medias"
        );
    }

    /// **LOS TRES ESTADOS DE UN CAMPO DE CONTACTO**, y son tres, no dos
    /// (directiva 32): no se tocó · se borró · se escribió.
    ///
    /// Con `COALESCE(?, campo)` —que es la forma obvia y la que tienen `nombre` y
    /// `codigo_eps`— el segundo estado es INALCANZABLE: mandar la cadena vacía
    /// guardaría `''`, y entonces «no se registró» y «se registró vacío» se
    /// confunden para siempre en la columna. Por eso el `UPDATE` lleva bandera de
    /// presencia aparte del valor.
    ///
    /// Estos siete campos son la parte de `terceros` que el VFP capturaba y
    /// medico no: el prestador tiene el teléfono de sus EPS desde hace años, y
    /// una migración que los pierda es pérdida de datos, no una simplificación.
    #[tokio::test]
    async fn un_contacto_se_escribe_se_conserva_y_se_borra() {
        let pool = base().await;
        let id = crear_entidad(&pool, &EntidadNueva {
            telefono: Some("  6045551234  ".into()),
            direccion: Some("Cra 13 # 32-76".into()),
            // El de contacto llega vacío desde el formulario: tiene que quedar
            // `NULL`, no cadena vacía.
            contacto: Some("   ".into()),
            ..entidad_minima("800", "EPS Uno", "E1")
        }).await.unwrap().unwrap();

        let e = &entidades(&pool).await.unwrap()[0];
        assert_eq!(e.telefono.as_deref(), Some("6045551234"), "se recorta al guardar");
        assert_eq!(e.contacto, None, "un campo en blanco es NULL, no cadena vacía");

        // NO SE TOCÓ: se edita el nombre y el teléfono sigue ahí.
        actualizar_entidad(&pool, id, &EntidadCambio {
            nombre: Some("EPS Uno S.A.".into()),
            ..Default::default()
        }).await.unwrap().unwrap();
        let e = &entidades(&pool).await.unwrap()[0];
        assert_eq!(e.nombre, "EPS Uno S.A.");
        assert_eq!(e.telefono.as_deref(), Some("6045551234"), "lo que no vino no se toca");
        assert_eq!(e.direccion.as_deref(), Some("Cra 13 # 32-76"));

        // SE ESCRIBIÓ: cambia.
        actualizar_entidad(&pool, id, &EntidadCambio {
            telefono: Some("6045559999".into()),
            ..Default::default()
        }).await.unwrap().unwrap();
        assert_eq!(entidades(&pool).await.unwrap()[0].telefono.as_deref(), Some("6045559999"));

        // SE BORRÓ: la cadena vacía sí pisa, y deja `NULL`.
        actualizar_entidad(&pool, id, &EntidadCambio {
            telefono: Some("".into()),
            ..Default::default()
        }).await.unwrap().unwrap();
        let e = &entidades(&pool).await.unwrap()[0];
        assert_eq!(e.telefono, None, "la cadena vacía BORRA el contacto");
        assert_eq!(
            e.direccion.as_deref(),
            Some("Cra 13 # 32-76"),
            "y no arrastra a los demás"
        );
    }

    #[tokio::test]
    async fn una_entidad_repetida_se_rechaza_por_su_nit() {
        let pool = base().await;
        let n = entidad_minima("800", "A", "E1");
        assert!(crear_entidad(&pool, &n).await.unwrap().is_ok());
        let e = crear_entidad(&pool, &n).await.unwrap().unwrap_err();
        assert!(e.mensaje().contains("800"), "{}", e.mensaje());
    }

    // ── Campos dinámicos ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn los_campos_dinamicos_se_acotan_y_validan() {
        let pool = base().await;
        let id = crear_campo(&pool, 1, &CampoNuevo {
            etiqueta: "Alergia al látex".into(), tipo: "BOOLEANO".into(),
            opciones: None, orden: 1,
        }).await.unwrap().unwrap();

        assert_eq!(campos(&pool, UNO).await.unwrap().len(), 1);
        assert!(campos(&pool, DOS).await.unwrap().is_empty(), "los ve la otra institución");
        assert!(campos(&pool, Ambito::Ninguno).await.unwrap().is_empty());

        // Un campo ajeno no se edita ni se borra.
        assert!(!actualizar_campo(&pool, DOS, id, &CampoCambio {
            etiqueta: Some("Pirata".into()), ..Default::default()
        }).await.unwrap().unwrap());
        assert!(!borrar_campo(&pool, DOS, id).await.unwrap());
        assert_eq!(campos(&pool, UNO).await.unwrap()[0].etiqueta, "Alergia al látex");

        // El suyo sí.
        assert!(borrar_campo(&pool, UNO, id).await.unwrap());
    }

    /// Un desplegable SIN opciones es un campo que nadie puede rellenar. Lo
    /// rechaza el `CHECK` de la migración y el mensaje lo explica.
    #[tokio::test]
    async fn un_desplegable_necesita_sus_opciones() {
        let pool = base().await;
        let e = crear_campo(&pool, 1, &CampoNuevo {
            etiqueta: "Lateralidad".into(), tipo: "SELECCION".into(),
            opciones: None, orden: 0,
        }).await.unwrap().unwrap_err();
        assert!(e.mensaje().contains("opciones"), "{}", e.mensaje());

        // La pareja: con opciones, entra.
        assert!(crear_campo(&pool, 1, &CampoNuevo {
            etiqueta: "Lateralidad".into(), tipo: "SELECCION".into(),
            opciones: Some("diestro,zurdo".into()), orden: 0,
        }).await.unwrap().is_ok());

        // Y un tipo inventado se rechaza diciendo cuáles valen.
        let e = crear_campo(&pool, 1, &CampoNuevo {
            etiqueta: "Otro".into(), tipo: "HOLOGRAMA".into(), opciones: None, orden: 0,
        }).await.unwrap().unwrap_err();
        assert!(e.mensaje().contains("TEXTO"), "{}", e.mensaje());
    }

    #[tokio::test]
    async fn dos_campos_no_comparten_etiqueta_en_la_misma_institucion() {
        let pool = base().await;
        let n = CampoNuevo {
            etiqueta: "Peso al nacer".into(), tipo: "NUMERO".into(), opciones: None, orden: 0,
        };
        assert!(crear_campo(&pool, 1, &n).await.unwrap().is_ok());
        let e = crear_campo(&pool, 1, &n).await.unwrap().unwrap_err();
        assert!(e.mensaje().contains("ya existe"), "{}", e.mensaje());
        // Pero la OTRA institución sí puede tener el suyo con el mismo nombre.
        assert!(crear_campo(&pool, 2, &n).await.unwrap().is_ok());
    }

    // ── La ficha ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn la_ficha_se_actualiza_sin_borrar_lo_que_no_viene() {
        let pool = base().await;
        assert!(actualizar_institucion(&pool, 1, &InstitucionCambio {
            razon_social: Some("IPS Uno S.A.S.".into()),
            telefono: Some("6041234567".into()),
            ..Default::default()
        }).await.unwrap());
        assert!(actualizar_institucion(&pool, 1, &InstitucionCambio {
            ciudad: Some("Medellín".into()), ..Default::default()
        }).await.unwrap());

        let i = institucion(&pool, 1).await.unwrap().unwrap();
        assert_eq!(i.razon_social.as_deref(), Some("IPS Uno S.A.S."));
        assert_eq!(i.telefono.as_deref(), Some("6041234567"), "se borró al guardar la ciudad");
        assert_eq!(i.ciudad.as_deref(), Some("Medellín"));
        assert_eq!(i.nit, "900", "el NIT no está entre los editables");
        assert!(i.activa, "ni `activa`");
        assert!(!actualizar_institucion(&pool, 999, &InstitucionCambio::default()).await.unwrap());
    }

    // ── Auditoría ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn la_auditoria_se_acota_por_institucion_y_pagina() {
        let pool = base().await;
        for (inst, usuario, n) in [(1i64, 1i64, 3), (2, 3, 2)] {
            for i in 0..n {
                let contenido = serde_json::json!({
                    "usuario_id": usuario, "rol": "admin", "institucion_id": inst,
                    "accion": "ver", "recurso": "paciente",
                    "objeto": {"paciente_id": 10 + i, "recurso_id": 100 + i},
                    "ts": format!("2026-08-06T1{i}:00:00Z"),
                }).to_string();
                sqlx::query(
                    "INSERT INTO acceso (hash_anterior, contenido, hash, firma) VALUES ('',?,'','')",
                ).bind(&contenido).execute(&pool).await.unwrap();
            }
        }
        let p1 = auditoria(&pool, UNO, 50, 0).await.unwrap();
        assert_eq!(p1.total, 3);
        assert_eq!(p1.items.len(), 3);
        assert!(p1.items.iter().all(|e| e.usuario_id == 1));
        assert_eq!(p1.items[0].usuario_nombre, "Admin Uno");
        assert_eq!(p1.items[0].tabla_afectada, "paciente");
        assert!(p1.items[0].exitoso);
        assert_eq!(p1.items[0].ip_address, None, "ausente, no inventada");

        assert_eq!(auditoria(&pool, DOS, 50, 0).await.unwrap().total, 2);
        assert_eq!(auditoria(&pool, Ambito::Superadmin, 50, 0).await.unwrap().total, 5);
        assert_eq!(auditoria(&pool, Ambito::Ninguno, 50, 0).await.unwrap().total, 0);

        // Paginado: el total NO cambia con la página.
        let p = auditoria(&pool, UNO, 2, 0).await.unwrap();
        assert_eq!((p.total, p.items.len()), (3, 2));
        let p = auditoria(&pool, UNO, 2, 2).await.unwrap();
        assert_eq!((p.total, p.items.len()), (3, 1));
    }

    // ── El nombre partido del profesional para el RDA (#603) ─────────────────

    /// Lee crudo los cuatro campos, para distinguir NULL de cadena vacía.
    async fn partido(pool: &crate::Pool, id: i64) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
        sqlx::query_as("SELECT nombre1, nombre2, apellido1, apellido2 FROM usuario WHERE id = ?")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn fijar_nombre_partido_escribe_y_la_lista_lo_refleja() {
        let pool = base().await;
        // Nacen todos NULL (la migración 0071 no los rellena).
        assert_eq!(partido(&pool, 2).await, (None, None, None, None));

        let ok = fijar_nombre_partido(&pool, UNO, 2, Some("Ana"), Some("María"), Some("Pérez"), Some("Gómez"))
            .await
            .unwrap();
        assert!(ok, "la médica es de la institución UNO, debe dejar fijar");
        assert_eq!(
            partido(&pool, 2).await,
            (Some("Ana".into()), Some("María".into()), Some("Pérez".into()), Some("Gómez".into()))
        );

        // La MISMA función que lee la pantalla lo refleja, y el `nombre` completo
        // sigue intacto (es el de login/listados/firma).
        let f = FiltroUsuarios::default();
        let lista = usuarios(&pool, UNO, &f, &[]).await.unwrap();
        let m = lista.iter().find(|p| p.id == 2).unwrap();
        assert_eq!(m.nombre1.as_deref(), Some("Ana"));
        assert_eq!(m.apellido1.as_deref(), Some("Pérez"));
        assert_eq!(m.nombre_completo, "Medica Uno");
    }

    /// D33: una casilla en blanco queda NULL, no `""`. El control positivo
    /// (`nombre1`) prueba que SÍ escribe cuando hay dato; el caso rojo (`nombre2`
    /// con espacios) prueba que NO fabrica un presente falso —una implementación
    /// que hiciera `bind(&str)` a secas guardaría `""` y el productor del RDA lo
    /// tomaría por un segundo nombre presente (directiva 20)—.
    #[tokio::test]
    async fn una_casilla_en_blanco_queda_null_no_cadena_vacia() {
        let pool = base().await;
        fijar_nombre_partido(&pool, UNO, 2, Some("Ana"), Some("   "), Some(""), None)
            .await
            .unwrap();
        let (n1, n2, a1, a2) = partido(&pool, 2).await;
        assert_eq!(n1.as_deref(), Some("Ana"), "control: el dato real SÍ se guarda");
        assert_eq!(n2, None, "espacios en blanco → NULL, no \"   \"");
        assert_eq!(a1, None, "cadena vacía → NULL, no \"\"");
        assert_eq!(a2, None);
    }

    /// D33: la guarda de ámbito. El control (misma institución) fija; el caso
    /// rojo (otra institución) NO toca el dato, y lo comprobamos leyéndolo crudo
    /// —un `Ok(false)` que igual hubiera escrito sería el fallo silencioso—.
    #[tokio::test]
    async fn no_toca_usuario_de_otra_institucion_ni_al_superadmin() {
        let pool = base().await;
        // Usuario 3 = Admin Dos, institución 2. UNO no lo alcanza.
        let r = fijar_nombre_partido(&pool, UNO, 3, Some("Intruso"), None, None, None)
            .await
            .unwrap();
        assert!(!r, "UNO no debe poder tocar a alguien de la institución 2");
        assert_eq!(partido(&pool, 3).await, (None, None, None, None), "y no lo escribió");

        // Control positivo: en su propia institución sí.
        assert!(fijar_nombre_partido(&pool, UNO, 2, Some("Ana"), None, None, None).await.unwrap());

        // El superadmin (id 4) no se administra desde aquí, lo pida quien lo pida.
        let s = fijar_nombre_partido(&pool, Ambito::Superadmin, 4, Some("X"), None, None, None)
            .await
            .unwrap();
        assert!(!s, "el superadmin no se toca por esta vía");
        assert_eq!(partido(&pool, 4).await, (None, None, None, None));
    }

    /// El alta captura el nombre partido cuando viene, y lo deja ausente cuando
    /// no —sin fabricar cadenas vacías (control + caso rojo, misma función).
    #[tokio::test]
    async fn el_alta_captura_el_nombre_partido() {
        let pool = base().await;
        let con = UsuarioAltaAdmin {
            username: None,
            email: Some("nueva.med@x.co".into()),
            password: "x".into(),
            nombre_completo: "Nueva Médica".into(),
            rol: "medico".into(),
            especialidad_id: None,
            registro_medico: None,
            registro_rethus: None,
            nombre1: Some("Luz".into()),
            nombre2: Some("  ".into()),
            apellido1: Some("Ruiz".into()),
            apellido2: None,
        };
        let id = crear_usuario(&pool, 1, &con, "h").await.unwrap().unwrap();
        assert_eq!(
            partido(&pool, id).await,
            (Some("Luz".into()), None, Some("Ruiz".into()), None),
            "el dato real se guarda; los espacios en blanco quedan NULL"
        );
    }
}
