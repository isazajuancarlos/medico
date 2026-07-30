//! Qué es un archivo DE VERDAD, mirando sus bytes.
//!
//! # Por qué existe
//!
//! `subir_imagen` tomaba el tipo del `Content-Type` que declara el cliente en su
//! parte del multipart y lo admitía si empezaba por `image/`. Dos consecuencias,
//! y la segunda es una vulnerabilidad:
//!
//! 1. `image/svg+xml` empieza por `image/`. Un SVG es un documento XML que puede
//!    llevar `<script>`.
//! 2. Ese mismo tipo, guardado tal cual, se devolvía como `Content-Type` con
//!    `Content-Disposition: inline`. La interfaz abre el adjunto como navegación
//!    de primer nivel, y la SPA se sirve DESDE EL MISMO ORIGEN que la API —a
//!    propósito, para que la sesión pueda ser una cookie `HttpOnly`—. Es decir:
//!    el script del SVG corría en el origen de la aplicación, con `SameSite=Strict`
//!    mandando la cookie en esa navegación. `HttpOnly` impide LEER la cookie, no
//!    USARLA: un `fetch('/api/...', {credentials:'include'})` habría actuado con
//!    los permisos de quien abrió el adjunto. Contra un gestor, eso alcanza
//!    `POST /api/usuarios`.
//!
//! Y el comentario de `subir_imagen` afirmaba que «el mime y el tamaño los mide el
//! servidor sobre los bytes recibidos». Del tamaño era cierto; del mime, no. Una
//! limitación declarada que no se cumple es peor que no declararla: el resto del
//! código se escribe confiando en ella.
//!
//! # La regla
//!
//! El tipo sale de los BYTES, nunca de lo que diga el cliente ni de lo que se
//! guardó en su día. Lo que no se reconoce no se admite y no se sirve como algo:
//! se descarga. La lista es corta a propósito — esto es un anexo de imagen
//! diagnóstica, no un buzón de archivos.

/// Los tipos que el sistema admite, por su firma en los primeros bytes.
///
/// Ninguno es interpretable por el navegador como documento con guion. SVG queda
/// fuera de forma explícita y por eso no aparece aquí: no es un olvido.
pub fn tipo_real(bytes: &[u8]) -> Option<&'static str> {
    let empieza = |firma: &[u8]| bytes.len() >= firma.len() && &bytes[..firma.len()] == firma;

    if empieza(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if empieza(b"\xFF\xD8\xFF") {
        return Some("image/jpeg");
    }
    if empieza(b"GIF87a") || empieza(b"GIF89a") {
        return Some("image/gif");
    }
    // WebP: «RIFF» …4 bytes de tamaño… «WEBP».
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if empieza(b"%PDF-") {
        return Some("application/pdf");
    }
    // TIFF, que sale de bastantes equipos de diagnóstico. Los dos órdenes de byte.
    if empieza(b"II\x2a\x00") || empieza(b"MM\x00\x2a") {
        return Some("image/tiff");
    }
    // DICOM: el preámbulo son 128 bytes cualesquiera y luego «DICM».
    if bytes.len() >= 132 && &bytes[128..132] == b"DICM" {
        return Some("application/dicom");
    }
    None
}

/// Si el navegador puede ABRIRLO sin peligro (`inline`) o hay que bajarlo
/// (`attachment`).
///
/// Se decide por el tipo real, no por el nombre. DICOM y TIFF se descargan: el
/// navegador no los pinta, y ofrecérselos «en línea» solo consigue que intente
/// adivinar qué son — que es exactamente lo que `nosniff` existe para impedir.
pub fn se_puede_mostrar(tipo: &str) -> bool {
    matches!(
        tipo,
        "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "application/pdf"
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn reconoce_los_formatos_que_admite() {
        assert_eq!(tipo_real(b"\x89PNG\r\n\x1a\n\x00\x00"), Some("image/png"));
        assert_eq!(tipo_real(b"\xFF\xD8\xFF\xE0 JFIF"), Some("image/jpeg"));
        assert_eq!(tipo_real(b"GIF89a....."), Some("image/gif"));
        assert_eq!(tipo_real(b"RIFF\x00\x00\x00\x00WEBPVP8 "), Some("image/webp"));
        assert_eq!(tipo_real(b"%PDF-1.7\n"), Some("application/pdf"));
        assert_eq!(tipo_real(b"II\x2a\x00rest"), Some("image/tiff"));

        let mut dicom = vec![0u8; 128];
        dicom.extend_from_slice(b"DICM");
        assert_eq!(tipo_real(&dicom), Some("application/dicom"));
    }

    #[test]
    fn un_svg_no_es_una_imagen_admitida_por_mucho_que_lo_diga_el_cliente() {
        // El corazón del asunto. Da igual lo que el cliente declare: estos bytes
        // no tienen firma reconocida, así que no entran. Antes bastaba con poner
        // `Content-Type: image/svg+xml` en la parte del multipart.
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
        assert_eq!(tipo_real(svg), None);

        // Y sus variantes: con declaración XML delante, o con espacios.
        assert_eq!(tipo_real(br#"<?xml version="1.0"?><svg><script/></svg>"#), None);
        assert_eq!(tipo_real(b"  \n<svg onload=alert(1)>"), None);
    }

    #[test]
    fn nada_de_lo_que_ejecuta_un_navegador_pasa() {
        for veneno in [
            &b"<html><script>alert(1)</script></html>"[..],
            &b"<!DOCTYPE html><body onload=alert(1)>"[..],
            &b"alert(document.cookie)"[..],
            &b"<?php system($_GET[0]); ?>"[..],
        ] {
            assert_eq!(tipo_real(veneno), None, "coló: {:?}", &veneno[..10.min(veneno.len())]);
        }
    }

    #[test]
    fn un_archivo_corto_no_desborda_ni_cuela() {
        for n in 0..12usize {
            let cortito = vec![0x89u8; n];
            assert_eq!(tipo_real(&cortito), None, "con {n} bytes");
        }
        // Un preámbulo de DICOM sin su marca tampoco vale.
        assert_eq!(tipo_real(&[0u8; 200]), None);
    }

    #[test]
    fn solo_se_muestra_en_linea_lo_que_el_navegador_pinta() {
        assert!(se_puede_mostrar("image/png"));
        assert!(se_puede_mostrar("application/pdf"));
        // Se reconocen y se guardan, pero se DESCARGAN: el navegador no los pinta.
        assert!(!se_puede_mostrar("application/dicom"));
        assert!(!se_puede_mostrar("image/tiff"));
        // Y lo que nunca debió entrar, tampoco se muestra si entrara por otra vía.
        assert!(!se_puede_mostrar("image/svg+xml"));
        assert!(!se_puede_mostrar("text/html"));
    }
}
