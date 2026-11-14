//! Las pruebas CRUZAN LA FRONTERA (directiva 28): levantan un servidor `axum`
//! real en un puerto efímero y le hablan por HTTP con el `reqwest` real, igual
//! que `medico-ihce`. Un `assert` de estructura a estructura mediría el mapeo y
//! no lo único que este crate aporta —el cuerpo COMPRIMIDO en el cable, el
//! `Bearer` puesto, el 400 que es un resultado y no una avería—.
//!
//! Portadas una a una de `backend/tests/test_muv_client.py`, que es el banco que
//! ya define esta semántica; donde el Python inyectaba un transporte falso, aquí
//! el mock es el servidor. Las que allí medían el transporte `urllib` (la
//! verificación de TLS) no se portan: aquí ese trozo es de `reqwest` y la
//! decisión vive en la feature `tls-local`, no en un objeto nuestro.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Router;

/// Lo que el mock devuelve y lo que va anotando de lo que RECIBE. Lo segundo es
/// la mitad que importa: sin ello se comprobaría lo que el cliente dice hacer.
struct Mock {
    status_login: u16,
    cuerpo_login: String,
    status_carga: u16,
    cuerpo_carga: String,
    visto_login: Mutex<Option<serde_json::Value>>,
    visto_carga_bytes: Mutex<Option<Vec<u8>>>,
    visto_encoding: Mutex<Option<String>>,
    visto_auth: Mutex<Option<String>>,
    visto_ruta: Mutex<Option<String>>,
    hits_carga: AtomicUsize,
}

impl Mock {
    fn nuevo() -> Self {
        Self {
            status_login: 200,
            cuerpo_login: r#"{"token":"tok-abc"}"#.into(),
            status_carga: 200,
            cuerpo_carga: r#"{"ResultState":true,"CodigoUnicoValidacion":"CUV-1"}"#.into(),
            visto_login: Mutex::new(None),
            visto_carga_bytes: Mutex::new(None),
            visto_encoding: Mutex::new(None),
            visto_auth: Mutex::new(None),
            visto_ruta: Mutex::new(None),
            hits_carga: AtomicUsize::new(0),
        }
    }
    fn login(mut self, status: u16, cuerpo: &str) -> Self {
        self.status_login = status;
        self.cuerpo_login = cuerpo.into();
        self
    }
    fn carga(mut self, status: u16, cuerpo: &str) -> Self {
        self.status_carga = status;
        self.cuerpo_carga = cuerpo.into();
        self
    }
}

async fn h_login(State(s): State<Arc<Mock>>, cuerpo: Bytes) -> impl IntoResponse {
    *s.visto_login.lock().unwrap() = serde_json::from_slice(&cuerpo).ok();
    respuesta(s.status_login, &s.cuerpo_login)
}

async fn h_carga(
    State(s): State<Arc<Mock>>,
    cabeceras: HeaderMap,
    cuerpo: Bytes,
) -> impl IntoResponse {
    s.hits_carga.fetch_add(1, Ordering::SeqCst);
    *s.visto_carga_bytes.lock().unwrap() = Some(cuerpo.to_vec());
    *s.visto_encoding.lock().unwrap() = cabeceras
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    *s.visto_auth.lock().unwrap() = cabeceras
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    respuesta(s.status_carga, &s.cuerpo_carga)
}

fn respuesta(status: u16, cuerpo: &str) -> axum::response::Response {
    (
        StatusCode::from_u16(status).unwrap(),
        [("content-type", "application/json")],
        cuerpo.to_string(),
    )
        .into_response()
}

/// Levanta el mock y devuelve el cliente ya apuntado a él.
async fn montar(mock: Mock) -> (ClienteMuv, Arc<Mock>) {
    let estado = Arc::new(mock);
    let app = Router::new()
        .route(RUTA_LOGIN, post(h_login))
        .route(
            RUTA_CARGAR_SIN_FACTURA,
            post({
                let e = estado.clone();
                move |c: HeaderMap, b: Bytes| {
                    let e = e.clone();
                    async move {
                        *e.visto_ruta.lock().unwrap() = Some(RUTA_CARGAR_SIN_FACTURA.into());
                        h_carga(State(e), c, b).await.into_response()
                    }
                }
            }),
        )
        .route(
            RUTA_CARGAR_FEV,
            post({
                let e = estado.clone();
                move |c: HeaderMap, b: Bytes| {
                    let e = e.clone();
                    async move {
                        *e.visto_ruta.lock().unwrap() = Some(RUTA_CARGAR_FEV.into());
                        h_carga(State(e), c, b).await.into_response()
                    }
                }
            }),
        )
        .route(RUTA_CONSULTAR_CUV, post(h_login))
        .with_state(estado.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dir = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let cliente = ClienteMuv::nuevo(&format!("http://{dir}"), Duration::from_secs(5), Tls::DelSistema).unwrap();
    (cliente, estado)
}

fn credenciales() -> Credenciales {
    Credenciales {
        tipo_documento: "CC".into(),
        numero_documento: "1234567890".into(),
        clave: "la-clave-del-prestador".into(),
        nit: "900123456".into(),
        tipo_usuario: None,
    }
}

fn descomprimir(b: &[u8]) -> String {
    use std::io::Read as _;
    let mut s = String::new();
    flate2::read::GzDecoder::new(b).read_to_string(&mut s).unwrap();
    s
}

// ── login ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn el_login_arma_el_cuerpo_que_pide_el_manual() {
    let (c, m) = montar(Mock::nuevo()).await;
    c.login(&credenciales()).await.unwrap();
    let visto = m.visto_login.lock().unwrap().clone().unwrap();
    assert_eq!(visto["persona"]["identificacion"]["tipo"], "CC");
    assert_eq!(visto["persona"]["identificacion"]["numero"], "1234567890");
    assert_eq!(visto["clave"], "la-clave-del-prestador");
    assert_eq!(visto["nit"], "900123456");
}

#[tokio::test]
async fn una_ips_no_informa_tipo_usuario() {
    let (c, m) = montar(Mock::nuevo()).await;
    c.login(&credenciales()).await.unwrap();
    let visto = m.visto_login.lock().unwrap().clone().unwrap();
    assert!(
        visto.get("tipoUsuario").is_none(),
        "una IPS no informa tipoUsuario, y mandarlo vacío no es lo mismo que no mandarlo"
    );
}

#[tokio::test]
async fn el_independiente_si_lo_informa() {
    let (c, m) = montar(Mock::nuevo()).await;
    let mut cr = credenciales();
    cr.tipo_usuario = Some("PIN".into());
    c.login(&cr).await.unwrap();
    let visto = m.visto_login.lock().unwrap().clone().unwrap();
    assert_eq!(visto["tipoUsuario"], "PIN");
}

#[tokio::test]
async fn una_respuesta_sin_token_no_se_da_por_buena() {
    let (c, _) = montar(Mock::nuevo().login(200, r#"{"mensaje":"ok"}"#)).await;
    let e = c.login(&credenciales()).await.unwrap_err();
    assert!(matches!(e, ErrorMuv::NoDisponible(_)), "fue {e:?}");
}

#[tokio::test]
async fn un_token_vacio_tampoco_vale() {
    // La pareja de la anterior: la clave ESTÁ y no sirve. Sin este caso, un
    // `contains_key` pasaría por comprobación.
    let (c, _) = montar(Mock::nuevo().login(200, r#"{"token":""}"#)).await;
    assert!(matches!(
        c.login(&credenciales()).await.unwrap_err(),
        ErrorMuv::NoDisponible(_)
    ));
}

#[tokio::test]
async fn credenciales_rechazadas_no_se_confunden_con_una_caida() {
    let (c, _) = montar(Mock::nuevo().login(401, r#"{"error":"no"}"#)).await;
    let e = c.login(&credenciales()).await.unwrap_err();
    assert!(matches!(e, ErrorMuv::CredencialesInvalidas(401)), "fue {e:?}");
}

#[tokio::test]
async fn un_401_sigue_siendo_credenciales_aunque_traiga_cuerpo_de_resultado() {
    // El 400 con resultado es un rechazo del ARCHIVO; el 401 nunca lo es, por
    // más que el cuerpo se le parezca. El orden de las dos comprobaciones ES la
    // regla: al revés, un 401 con `ResultState` se leería como archivo malo y
    // mandaría a corregir un RIPS que está bien.
    let (c, _) = montar(
        Mock::nuevo().login(401, r#"{"ResultState":false,"ResultadosValidacion":[]}"#),
    )
    .await;
    assert!(matches!(
        c.login(&credenciales()).await.unwrap_err(),
        ErrorMuv::CredencialesInvalidas(401)
    ));
}

#[tokio::test]
async fn si_el_contenedor_no_esta_levantado_se_dice_claro() {
    // Puerto cerrado: no hay servidor. Es el caso más común en el consultorio.
    let c = ClienteMuv::nuevo("http://127.0.0.1:1", Duration::from_secs(2), Tls::DelSistema).unwrap();
    let e = c.login(&credenciales()).await.unwrap_err();
    assert!(matches!(e, ErrorMuv::NoDisponible(_)), "fue {e:?}");
}

#[tokio::test]
async fn sin_url_no_se_construye() {
    assert!(ClienteMuv::nuevo("   ", Duration::from_secs(5), Tls::DelSistema).is_err());
}

// ── el token no sale, y las credenciales tampoco ─────────────────────────────

#[tokio::test]
async fn las_credenciales_no_aparecen_al_imprimirlas() {
    let texto = format!("{:?}", credenciales());
    assert!(!texto.contains("la-clave-del-prestador"), "{texto}");
    assert!(!texto.contains("1234567890"), "{texto}");
    assert!(!texto.contains("900123456"), "{texto}");
    // Positivo de control: el Debug SÍ dice algo útil, no está vacío.
    assert!(texto.contains("CC"), "{texto}");
}

#[tokio::test]
async fn el_token_no_se_devuelve_pero_viaja_en_la_carga() {
    let (c, m) = montar(Mock::nuevo()).await;
    // `login` devuelve `()`: no hay por dónde sacar el token del cliente.
    let () = c.login(&credenciales()).await.unwrap();
    c.cargar(&serde_json::json!({"x":1}), Destino::SinFactura)
        .await
        .unwrap();
    assert_eq!(
        m.visto_auth.lock().unwrap().clone().unwrap(),
        "Bearer tok-abc"
    );
}

#[tokio::test]
async fn sin_login_no_se_carga() {
    let (c, m) = montar(Mock::nuevo()).await;
    let e = c
        .cargar(&serde_json::json!({"x": 1}), Destino::SinFactura)
        .await
        .unwrap_err();
    assert!(matches!(e, ErrorMuv::SinToken), "fue {e:?}");
    // La mitad que discrimina: no solo falla, es que NO LLEGÓ a salir a la red.
    assert_eq!(m.hits_carga.load(Ordering::SeqCst), 0);
}

// ── la carga ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn el_cuerpo_de_carga_va_comprimido() {
    let (c, m) = montar(Mock::nuevo()).await;
    c.login(&credenciales()).await.unwrap();
    c.cargar(&serde_json::json!({"rips": {"numDocumentoIdObligado": "900123456"}}), Destino::SinFactura)
        .await
        .unwrap();

    assert_eq!(m.visto_encoding.lock().unwrap().as_deref(), Some("gzip"));
    let bytes = m.visto_carga_bytes.lock().unwrap().clone().unwrap();
    assert_eq!(&bytes[..2], &[0x1f, 0x8b], "no empieza por la firma gzip");
    // Y descomprime al JSON que se le dio: comprimir basura también da bytes.
    let json: serde_json::Value = serde_json::from_str(&descomprimir(&bytes)).unwrap();
    assert_eq!(json["rips"]["numDocumentoIdObligado"], "900123456");
}

#[tokio::test]
async fn el_destino_elige_la_ruta_del_manual() {
    let (c, m) = montar(Mock::nuevo()).await;
    c.login(&credenciales()).await.unwrap();
    c.cargar(&serde_json::json!({}), Destino::Fev).await.unwrap();
    assert_eq!(
        m.visto_ruta.lock().unwrap().as_deref(),
        Some(RUTA_CARGAR_FEV)
    );
}

#[tokio::test]
async fn una_carga_aceptada_devuelve_el_cuv() {
    let (c, _) = montar(Mock::nuevo().carga(
        200,
        r#"{"ResultState":true,"CodigoUnicoValidacion":"CUV-OK","ProcesoId":42,
            "FechaRadicacion":"2026-08-10","Ambiente":"DockerStage","ResultadosValidacion":[]}"#,
    ))
    .await;
    c.login(&credenciales()).await.unwrap();
    let r = c.cargar(&serde_json::json!({}), Destino::SinFactura).await.unwrap();
    assert!(r.aceptado);
    assert_eq!(r.cuv.as_deref(), Some("CUV-OK"));
    assert_eq!(r.proceso_id, Some(42));
    assert_eq!(r.ambiente.as_deref(), Some("DockerStage"));
}

#[tokio::test]
async fn las_notificaciones_no_impiden_el_cuv() {
    let (c, _) = montar(Mock::nuevo().carga(
        200,
        r#"{"ResultState":true,"CodigoUnicoValidacion":"CUV-OK","ResultadosValidacion":[
            {"Clase":"NOTIFICACION","Codigo":"RVC021","Descripcion":"aviso","PathFuente":"usuarios[0]"}]}"#,
    ))
    .await;
    c.login(&credenciales()).await.unwrap();
    let r = c.cargar(&serde_json::json!({}), Destino::SinFactura).await.unwrap();
    assert!(r.aceptado);
    assert_eq!(r.notificaciones().len(), 1);
    assert!(r.bloqueantes().is_empty());
}

#[tokio::test]
async fn un_rechazo_no_devuelve_cuv_pero_si_los_hallazgos() {
    let (c, _) = montar(Mock::nuevo().carga(
        200,
        r#"{"ResultState":false,"CodigoUnicoValidacion":null,"ResultadosValidacion":[
            {"Clase":"RECHAZO","Codigo":"RVG05","Descripcion":"código inválido",
             "PathFuente":"usuarios[1].servicios.consultas[0].codConsulta"}]}"#,
    ))
    .await;
    c.login(&credenciales()).await.unwrap();
    let r = c.cargar(&serde_json::json!({}), Destino::SinFactura).await.unwrap();
    assert!(!r.aceptado);
    assert!(r.cuv.is_none());
    assert_eq!(r.bloqueantes().len(), 1);
    assert_eq!(
        r.bloqueantes()[0].path_fuente,
        "usuarios[1].servicios.consultas[0].codConsulta"
    );
}

#[tokio::test]
async fn un_rechazo_no_es_un_error() {
    // Es la decisión de diseño: un rechazo es un resultado normal del proceso.
    // Si fuera `Err`, cada llamante tendría que desenterrar los hallazgos de
    // dentro de un error para poder enseñarlos.
    let (c, _) = montar(Mock::nuevo().carga(200, r#"{"ResultState":false,"ResultadosValidacion":[]}"#)).await;
    c.login(&credenciales()).await.unwrap();
    assert!(c.cargar(&serde_json::json!({}), Destino::SinFactura).await.is_ok());
}

#[tokio::test]
async fn una_clase_desconocida_se_trata_como_bloqueante() {
    // Por exclusión y a propósito: equivocarse hacia el lado seguro.
    let (c, _) = montar(Mock::nuevo().carga(
        200,
        r#"{"ResultState":true,"CodigoUnicoValidacion":"CUV-X","ResultadosValidacion":[
            {"Clase":"ADVERTENCIA_NUEVA","Codigo":"RVX99","Descripcion":"?"}]}"#,
    ))
    .await;
    c.login(&credenciales()).await.unwrap();
    let r = c.cargar(&serde_json::json!({}), Destino::SinFactura).await.unwrap();
    assert_eq!(r.bloqueantes().len(), 1);
    assert!(!r.aceptado, "una clase que no conocemos no puede dar por bueno el archivo");
}

#[tokio::test]
async fn un_cuv_con_bloqueantes_no_se_da_por_aceptado() {
    let (c, _) = montar(Mock::nuevo().carga(
        200,
        r#"{"ResultState":true,"CodigoUnicoValidacion":"CUV-CONTRADICTORIO","ResultadosValidacion":[
            {"Clase":"RECHAZO","Codigo":"RVG05","Descripcion":"malo"}]}"#,
    ))
    .await;
    c.login(&credenciales()).await.unwrap();
    let r = c.cargar(&serde_json::json!({}), Destino::SinFactura).await.unwrap();
    assert!(!r.aceptado);
    assert!(r.cuv.is_none(), "no se entrega un CUV de un resultado contradictorio");
}

#[tokio::test]
async fn los_hallazgos_se_agrupan_por_el_campo_que_los_produjo() {
    let (c, _) = montar(Mock::nuevo().carga(
        200,
        r#"{"ResultState":false,"ResultadosValidacion":[
            {"Clase":"RECHAZO","Codigo":"A","Descripcion":"1","PathFuente":"usuarios[0].tipoDocumentoIdentificacion"},
            {"Clase":"NOTIFICACION","Codigo":"B","Descripcion":"2","PathFuente":"usuarios[0].tipoDocumentoIdentificacion"},
            {"Clase":"RECHAZO","Codigo":"C","Descripcion":"3","PathFuente":"usuarios[1].codSexo"}]}"#,
    ))
    .await;
    c.login(&credenciales()).await.unwrap();
    let r = c.cargar(&serde_json::json!({}), Destino::SinFactura).await.unwrap();
    let campos = r.campos_a_corregir();
    assert_eq!(campos.len(), 2);
    assert_eq!(campos["usuarios[0].tipoDocumentoIdentificacion"].len(), 2);
    assert_eq!(campos["usuarios[1].codSexo"].len(), 1);
}

#[tokio::test]
async fn un_hallazgo_sin_path_no_rompe_el_agrupamiento() {
    let (c, _) = montar(Mock::nuevo().carga(
        200,
        r#"{"ResultState":false,"ResultadosValidacion":[
            {"Clase":"RECHAZO","Codigo":"A","Descripcion":"sin path","PathFuente":null},
            {"Clase":"RECHAZO","Codigo":"B","Descripcion":"con path","PathFuente":"usuarios[0]"}]}"#,
    ))
    .await;
    c.login(&credenciales()).await.unwrap();
    let r = c.cargar(&serde_json::json!({}), Destino::SinFactura).await.unwrap();
    assert_eq!(r.hallazgos.len(), 2, "el nulo no puede perder el hallazgo");
    assert_eq!(r.campos_a_corregir().len(), 1, "pero tampoco inventar un campo vacío");
}

// ── un 400 que ES un resultado, y uno que no ─────────────────────────────────

#[tokio::test]
async fn un_400_con_resultado_es_un_rechazo_no_una_averia() {
    let (c, _) = montar(Mock::nuevo().carga(
        400,
        r#"{"ResultState":false,"Ambiente":"DockerStage","ResultadosValidacion":[
            {"Clase":"RECHAZO","Codigo":"RVG05","Descripcion":"malo","PathFuente":"usuarios[0]"}]}"#,
    ))
    .await;
    c.login(&credenciales()).await.unwrap();
    let r = c
        .cargar(&serde_json::json!({}), Destino::SinFactura)
        .await
        .expect("un 400 con resultado dentro NO es una avería");
    assert!(!r.aceptado);
    assert_eq!(r.bloqueantes().len(), 1);
}

#[tokio::test]
async fn un_400_que_no_es_un_resultado_sigue_siendo_averia() {
    // La pareja de la anterior, y la que hace que la regla discrimine: el HTML
    // de un proxy delante del contenedor no es un veredicto sobre el archivo.
    let (c, _) = montar(Mock::nuevo().carga(400, "<html><body>Bad Gateway</body></html>")).await;
    c.login(&credenciales()).await.unwrap();
    let e = c
        .cargar(&serde_json::json!({}), Destino::SinFactura)
        .await
        .unwrap_err();
    assert!(matches!(e, ErrorMuv::NoDisponible(_)), "fue {e:?}");
}

#[tokio::test]
async fn un_200_ininteligible_es_averia() {
    let (c, _) = montar(Mock::nuevo().carga(200, "no soy json")).await;
    c.login(&credenciales()).await.unwrap();
    assert!(matches!(
        c.cargar(&serde_json::json!({}), Destino::SinFactura).await.unwrap_err(),
        ErrorMuv::NoDisponible(_)
    ));
}

#[tokio::test]
async fn un_mensaje_con_tildes_no_revienta_al_recortarlo() {
    // `&texto[..300]` entra en pánico si el corte cae dentro de un carácter
    // multibyte, y los mensajes del MUV vienen en castellano. El cuerpo mide
    // más de 300 caracteres a propósito, para que el recorte ocurra.
    //
    // La `x` de delante NO es adorno y la descubrió el banco de mutación: con
    // «ñ» a secas —2 bytes— el byte 300 cae en frontera exacta de carácter, así
    // que el mutante que recorta por BYTES sobrevivía. Con un ASCII delante, el
    // corte cae DENTRO de una «ñ» y el caso rojo falla por la vía real
    // (directiva 33).
    let cuerpo = format!("x{}", "ñ".repeat(400));
    let (c, _) = montar(Mock::nuevo().carga(500, &cuerpo)).await;
    c.login(&credenciales()).await.unwrap();
    let e = c.cargar(&serde_json::json!({}), Destino::SinFactura).await.unwrap_err();
    assert!(matches!(e, ErrorMuv::NoDisponible(_)));
}

// ── las posturas flojas solo valen DENTRO de la máquina ─────────────────────
//
// Lo pidió la revisión de seguridad del 2026-08-10: la premisa que las hace
// defendibles —«la conexión no sale del equipo»— vivía en un comentario y no la
// comprobaba nadie.

/// Devuelve solo el ERROR, si lo hubo. `ClienteMuv` no implementa `Debug` a
/// propósito —lleva el token de sesión dentro—, así que `unwrap_err()` no se
/// puede usar aquí: el arnés se adapta al tipo, no al revés.
fn fallo_de(url: &str, tls: Tls) -> Option<ErrorMuv> {
    ClienteMuv::nuevo(url, Duration::from_secs(5), tls).err()
}

fn construye(url: &str, tls: Tls) -> bool {
    ClienteMuv::nuevo(url, Duration::from_secs(5), tls).is_ok()
}

#[tokio::test]
async fn sin_tls_contra_otra_maquina_no_se_construye() {
    let e = fallo_de("http://validador.ajeno.com:9443", Tls::DelSistema).expect("debe rechazar");
    assert!(matches!(e, ErrorMuv::ConfiguracionInsegura(_)), "fue {e:?}");
}

#[tokio::test]
async fn sin_verificar_contra_otra_maquina_no_se_construye() {
    let e = fallo_de("https://validador.ajeno.com:9443", Tls::SinVerificar).expect("debe rechazar");
    assert!(matches!(e, ErrorMuv::ConfiguracionInsegura(_)), "fue {e:?}");
}

#[tokio::test]
async fn contra_esta_maquina_las_dos_posturas_valen() {
    // La pareja, y sin ella la regla podría estar rechazándolo TODO: es el caso
    // real del prestador —el contenedor en su propio equipo— y tiene que pasar.
    for url in ["http://127.0.0.1:9443", "http://localhost:9443", "http://[::1]:9443"] {
        assert!(construye(url, Tls::SinVerificar), "{url} es esta máquina");
        assert!(construye(url, Tls::DelSistema), "{url} es esta máquina");
    }
}

#[tokio::test]
async fn https_verificando_contra_otra_maquina_si_vale() {
    // La otra pareja: lo que se cierra es la postura FLOJA fuera de la máquina,
    // no hablar con otra máquina. Un validador remoto con TLS verificado es
    // legítimo, y rechazarlo sería el sesgo a cerrar.
    assert!(construye("https://validador.ajeno.com:9443", Tls::DelSistema));
}

#[tokio::test]
async fn el_usuario_en_la_url_no_disfraza_de_local_a_un_host_ajeno() {
    // `http://localhost@evil.com/` tiene host `evil.com`: el `localhost` es el
    // USUARIO. Un `starts_with("http://localhost")` lo habría dado por local, y
    // por eso la comprobación va sobre la URL parseada y no sobre la cadena.
    let e = fallo_de("http://localhost@validador.ajeno.com/", Tls::SinVerificar).expect("debe rechazar");
    assert!(matches!(e, ErrorMuv::ConfiguracionInsegura(_)), "fue {e:?}");
}

#[tokio::test]
async fn un_nombre_que_solo_empieza_por_localhost_tampoco_cuela() {
    let e = fallo_de("http://localhost.evil.com:9443", Tls::DelSistema).expect("debe rechazar");
    assert!(matches!(e, ErrorMuv::ConfiguracionInsegura(_)), "fue {e:?}");
}

#[tokio::test]
async fn una_url_ilegible_no_se_da_por_buena() {
    assert!(fallo_de("no soy una url", Tls::DelSistema).is_some());
}

// ── la forma del hallazgo tiene DOS autores ──────────────────────────────────

#[tokio::test]
async fn un_hallazgo_sobrevive_a_la_ida_y_la_vuelta() {
    // Desde que el consumidor GUARDA los hallazgos, esta forma la escriben dos
    // sitios —el validador y nuestra base— y releerlos tiene que dar exactamente
    // el mismo tipo. Sin esta prueba, un `rename` que se le olvide a `Serialize`
    // produce una fila que se relee con todos los campos en blanco, sin error.
    let original = Hallazgo {
        clase: "RECHAZO".into(),
        codigo: "RVG05".into(),
        descripcion: "código inválido".into(),
        observaciones: "el valor 99 no está en la tabla".into(),
        path_fuente: "usuarios[1].servicios.consultas[0].codConsulta".into(),
        fuente: "AC".into(),
    };
    let json = serde_json::to_string(&original).unwrap();
    // Con los MISMOS nombres que usa el validador, no con los de Rust: la fila
    // guardada y la respuesta original tienen que ser la misma cosa.
    assert!(json.contains("\"PathFuente\""), "{json}");
    assert!(json.contains("\"Clase\""), "{json}");
    assert_eq!(serde_json::from_str::<Hallazgo>(&json).unwrap(), original);
}

// ── el CUV se consulta sin token ─────────────────────────────────────────────

#[tokio::test]
async fn consultar_el_cuv_no_exige_token() {
    let (c, _) = montar(Mock::nuevo()).await;
    let r = c
        .consultar_cuv(&serde_json::json!({"numDocumentoIdObligado": "900123456"}))
        .await
        .expect("la consulta no requiere autenticación");
    assert_eq!(r["token"], "tok-abc");
}
