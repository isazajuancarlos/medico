// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza Arenas

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
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntidadNueva {
    pub nit: String,
    pub nombre: String,
    pub codigo_eps: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EntidadCambio {
    pub nombre: Option<String>,
    pub codigo_eps: Option<String>,
}

pub async fn entidades(pool: &crate::Pool) -> Result<Vec<Entidad>, sqlx::Error> {
    sqlx::query_as("SELECT id, nit, nombre, codigo_eps FROM entidad ORDER BY nombre")
        .fetch_all(pool)
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
    match sqlx::query("INSERT INTO entidad (nit, nombre, codigo_eps) VALUES (?,?,?)")
        .bind(n.nit.trim())
        .bind(n.nombre.trim())
        .bind(n.codigo_eps.trim())
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
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE entidad SET nombre = COALESCE(?, nombre), \
         codigo_eps = COALESCE(?, codigo_eps) WHERE id = ?",
    )
    .bind(c.nombre.as_deref().map(str::trim))
    .bind(c.codigo_eps.as_deref().map(str::trim))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
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
        "SELECT u.id, u.email, u.nombre, u.rol, u.activo, u.especialidad_id, \
         e.nombre AS especialidad_nombre, u.registro_medico, u.registro_rethus, \
         u.requiere_cambio_password, u.ultimo_cambio_password \
         FROM usuario u LEFT JOIN especialidad e ON e.id = u.especialidad_id \
         WHERE {filtro} AND u.rol <> 'superadmin'"
    );
    if f.q.is_some() {
        sql.push_str(" AND (u.nombre LIKE ? OR u.email LIKE ?)");
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
pub async fn correos_frenados(pool: &crate::Pool) -> Result<Vec<String>, sqlx::Error> {
    let corte = (chrono::Utc::now() - chrono::Duration::hours(crate::freno::RETENCION_HORAS))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    sqlx::query_scalar(
        "SELECT email FROM intento_acceso WHERE exito = 0 AND momento >= ? \
         GROUP BY email HAVING COUNT(*) >= ?",
    )
    .bind(corte)
    .bind(crate::freno::UMBRAL)
    .fetch_all(pool)
    .await
}

/// Borra los intentos fallidos de ese correo: es lo mismo que hace un acierto, y
/// quita la espera ahora. **No hay ninguna cuenta que «desbloquear»** — ver la
/// cabecera del módulo.
pub async fn quitar_freno(pool: &crate::Pool, email: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM intento_acceso WHERE email = ?")
        .bind(email)
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
pub fn ascenso_permitido(rol_destino: Option<&str>, quien: &crate::usuario::Usuario) -> bool {
    // Se mira el DESTINO y no el origen: degradar a un superadmin no crea
    // autoridad nueva, y prohibirlo dejaría al dueño del sistema sin poder
    // retirar la suya.
    rol_destino != Some("superadmin") || quien.rol() == Some(RolUsuario::Superadmin)
}

/// ¿Ese usuario es del ámbito de quien pregunta? `None` si no existe o es ajeno.
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
    if RolUsuario::desde_str(&n.rol).is_none() {
        return Ok(Err(FalloUsuario::RolDesconocido(n.rol.clone())));
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
         especialidad_id, registro_medico, registro_rethus, requiere_cambio_password) \
         VALUES (?,?,?,?,?,1,?,?,?,0)",
    )
    .bind(n.nombre_completo.trim())
    .bind(email)
    .bind(hash)
    .bind(&n.rol)
    .bind(institucion_id)
    .bind(n.especialidad_id)
    .bind(n.registro_medico.as_deref().map(str::trim))
    .bind(n.registro_rethus.as_deref().map(str::trim))
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
}

pub async fn consumo_de(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    c: &UsuarioCambioAdmin,
) -> Result<Option<Consumo>, sqlx::Error> {
    let Some((_, _, era_activo)) = del_ambito(pool, ambito, id).await? else {
        return Ok(None);
    };
    let sera_activo = c.activo.unwrap_or(era_activo);
    // Se comprueba al ENTRAR, nunca al salir: desactivar libera asiento y
    // estorbarlo sería cerrar por cerrar.
    Ok(Some(Consumo { reactiva: sera_activo && !era_activo }))
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
    let r = sqlx::query(
        "UPDATE usuario SET \
         nombre = COALESCE(?, nombre), email = COALESCE(?, email), rol = COALESCE(?, rol), \
         activo = COALESCE(?, activo), especialidad_id = COALESCE(?, especialidad_id), \
         registro_medico = COALESCE(?, registro_medico), \
         registro_rethus = COALESCE(?, registro_rethus), \
         requiere_cambio_password = COALESCE(?, requiere_cambio_password) \
         WHERE id = ?",
    )
    .bind(c.nombre_completo.as_deref().map(str::trim))
    .bind(c.email.as_deref().map(str::trim))
    .bind(&c.rol)
    .bind(c.activo)
    .bind(c.especialidad_id)
    .bind(c.registro_medico.as_deref().map(str::trim))
    .bind(c.registro_rethus.as_deref().map(str::trim))
    .bind(c.requiere_cambio_password)
    .bind(id)
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
    sqlx::query("UPDATE usuario SET activo = 0, token_version = token_version + 1 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(true)
}

/// Restablece la contraseña y **obliga a cambiarla al entrar**: quien la fijó la
/// conoce, y una contraseña que conoce el administrador no identifica a nadie.
///
/// Revoca también las sesiones abiertas. Sin eso, restablecer la contraseña de
/// una cuenta comprometida no echa a quien está dentro.
pub async fn restablecer_password(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
    hash: &str,
) -> Result<bool, sqlx::Error> {
    let Some((email, _, _)) = del_ambito(pool, ambito, id).await? else {
        return Ok(false);
    };
    sqlx::query(
        "UPDATE usuario SET password_hash = ?, requiere_cambio_password = 1, \
         ultimo_cambio_password = ?, token_version = token_version + 1 WHERE id = ?",
    )
    .bind(hash)
    .bind(crate::ahora())
    .bind(id)
    .execute(pool)
    .await?;
    // Y se le quita el freno: si estaba esperando por intentos fallidos, la
    // contraseña nueva no le serviría hasta que pasara la espera.
    quitar_freno(pool, &email).await?;
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

    /// Se mira el DESTINO y no el origen: degradar a un superadmin no crea
    /// autoridad nueva, y prohibirlo dejaría al dueño sin poder retirar la suya.
    #[tokio::test]
    async fn degradar_a_un_superadmin_no_esta_prohibido() {
        let pool = base().await;
        let dueño = quien(&pool, 4).await;
        assert!(ascenso_permitido(Some("admin"), &dueño));
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
            Some(Consumo { reactiva: true })
        );

        // Y las tres formas de NO entrar al conjunto contado:
        let ya_activo = UsuarioCambioAdmin { activo: Some(true), ..Default::default() };
        assert_eq!(
            consumo_de(&pool, UNO, 2, &ya_activo).await.unwrap(),
            Some(Consumo { reactiva: false }),
            "el que ya estaba activo no ocupa un asiento nuevo"
        );
        let apagar = UsuarioCambioAdmin { activo: Some(false), ..Default::default() };
        assert_eq!(
            consumo_de(&pool, UNO, 2, &apagar).await.unwrap(),
            Some(Consumo { reactiva: false }),
            "desactivar LIBERA: estorbarlo sería cerrar por cerrar"
        );
        let solo_nombre = UsuarioCambioAdmin {
            nombre_completo: Some("Otro".into()),
            ..Default::default()
        };
        assert_eq!(
            consumo_de(&pool, UNO, 5, &solo_nombre).await.unwrap(),
            Some(Consumo { reactiva: false }),
            "guardar sin tocar `activo` no es crecer"
        );
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
    #[tokio::test]
    async fn el_comodin_de_la_busqueda_no_es_del_usuario() {
        let pool = base().await;
        let f = FiltroUsuarios { q: Some("%".into()), ..Default::default() };
        assert!(
            usuarios(&pool, UNO, &f, &[]).await.unwrap().is_empty(),
            "el `%` se trató como comodín: nadie se llama literalmente «%»"
        );
        // La pareja: una búsqueda de verdad sí encuentra.
        let f = FiltroUsuarios { q: Some("Medica".into()), ..Default::default() };
        assert_eq!(usuarios(&pool, UNO, &f, &[]).await.unwrap().len(), 1);
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

    /// Restablecer obliga a cambiarla, revoca las sesiones y quita el freno.
    #[tokio::test]
    async fn restablecer_obliga_a_cambiar_revoca_y_quita_el_freno() {
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
        let intentos: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM intento_acceso WHERE email = 'm1@x.co'")
                .fetch_one(&pool).await.unwrap();
        assert_eq!(intentos, 0, "la contraseña nueva no serviría hasta pasar la espera");
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
        assert_eq!(c.prefijo_factura, "FE");
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

    #[tokio::test]
    async fn no_se_borra_una_entidad_referenciada() {
        let pool = base().await;
        let id = crear_entidad(&pool, &EntidadNueva {
            nit: "800".into(), nombre: "EPS Uno".into(), codigo_eps: "E1".into(),
        }).await.unwrap().unwrap();
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
        let id = crear_entidad(&pool, &EntidadNueva {
            nit: "800".into(), nombre: "EPS".into(), codigo_eps: "E1".into(),
        }).await.unwrap().unwrap();
        actualizar_entidad(&pool, id, &EntidadCambio {
            nombre: Some("EPS Nueva".into()), codigo_eps: None,
        }).await.unwrap();
        let e = &entidades(&pool).await.unwrap()[0];
        assert_eq!(e.nombre, "EPS Nueva");
        assert_eq!(e.nit, "800", "el NIT identifica ante la DIAN: no se toca");
        assert_eq!(e.codigo_eps, "E1", "y lo que no vino, tampoco");
    }

    #[tokio::test]
    async fn una_entidad_repetida_se_rechaza_por_su_nit() {
        let pool = base().await;
        let n = EntidadNueva { nit: "800".into(), nombre: "A".into(), codigo_eps: "E1".into() };
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
}
