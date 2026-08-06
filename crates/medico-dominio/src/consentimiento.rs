//! Cuándo un consentimiento informado vale, y por qué no.
//!
//! Port de `backend/consentimientos.py`. Vive aquí, sin base de datos, por lo
//! mismo que [`crate::facturacion`]: es una regla, se prueba en microsegundos y
//! la capa de datos no puede saltársela.
//!
//! # Firmar no es consentir
//!
//! Es la distinción que sostiene toda la figura. Lo que la Ley 23 de 1981
//! (art. 15) y la Resolución 1995 de 1999 exigen es que al paciente se le hayan
//! explicado el procedimiento, sus riesgos, sus beneficios, las alternativas y
//! qué pasa si no se hace, y que lo haya **comprendido**. La firma prueba eso;
//! no lo sustituye.
//!
//! Por eso `paciente_comprende` y `acepta_procedimiento` son dos campos y aquí
//! valen los dos. Un registro con la firma puesta y `comprende` en falso no es
//! un consentimiento a medias: **no es un consentimiento**, y quien opere
//! confiando en él no tiene con qué defenderse.
//!
//! # Por qué el juicio es un tipo y no un `bool`
//!
//! El Python devuelve un `Validez` con la lista de faltantes, y la pantalla la
//! pinta mientras se llena el formulario. Aquí lo mismo, con una diferencia que
//! importa: [`Faltante`] es un enum, así que la pantalla no compara cadenas en
//! español para decidir qué campo resaltar. Es la lección de `orden.estado`
//! —tres escrituras del mismo valor conviviendo hasta que una pantalla deja de
//! encontrar sus filas— aplicada antes de que ocurra.

use serde::{Deserialize, Serialize};

/// Lo que puede faltarle a un consentimiento para serlo.
///
/// El orden de las variantes es el de la evaluación, y la pantalla lo respeta:
/// primero lo que hay que haber explicado, después lo que el paciente afirmó,
/// al final las firmas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Faltante {
    /// En qué consiste el procedimiento.
    Descripcion,
    /// Los riesgos.
    Riesgos,
    /// Los beneficios esperados.
    Beneficios,
    /// Las alternativas.
    Alternativas,
    /// La constancia de que el paciente comprendió. El más importante y el más
    /// fácil de pasar por alto: sin esto hay una firma, no un consentimiento
    /// informado.
    Comprension,
    /// La aceptación del procedimiento.
    Aceptacion,
    /// La firma del paciente.
    FirmaDelPaciente,
    /// La firma del médico.
    FirmaDelMedico,
    /// El paciente lo revocó. No es una falta de captura: es una decisión suya,
    /// y por eso es la única que llega sola y sin las demás.
    Revocado,
}

impl Faltante {
    /// Cómo se lee en la pantalla. Lo sirve el servidor y no el navegador para
    /// que no haya dos redacciones de la misma regla (la del navegador es la
    /// que ve el médico, y diverge al primer cambio).
    pub fn etiqueta(self) -> &'static str {
        match self {
            Faltante::Descripcion => "en qué consiste el procedimiento",
            Faltante::Riesgos => "los riesgos",
            Faltante::Beneficios => "los beneficios esperados",
            Faltante::Alternativas => "las alternativas",
            Faltante::Comprension => "la constancia de que el paciente comprendió",
            Faltante::Aceptacion => "la aceptación del procedimiento",
            Faltante::FirmaDelPaciente => "la firma del paciente",
            Faltante::FirmaDelMedico => "la firma del médico",
            Faltante::Revocado => "la revocación del paciente",
        }
    }
}

/// Lo que hay que juzgar. Es una vista, no la fila: [`evaluar`] no necesita el
/// id, ni las fechas, ni el testigo, y pedirle la fila entera ataría la regla a
/// la forma de la tabla.
#[derive(Debug, Clone, Copy)]
pub struct AJuzgar<'a> {
    pub descripcion_procedimiento: &'a str,
    pub riesgos_explicados: &'a str,
    pub beneficios_esperados: &'a str,
    pub alternativas_explicadas: &'a str,
    pub paciente_comprende: bool,
    pub acepta_procedimiento: bool,
    pub firma_paciente_path: Option<&'a str>,
    pub firma_medico_path: Option<&'a str>,
    pub revocado: bool,
}

/// El veredicto: si sirve para lo que tiene que servir, y qué le falta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Validez {
    pub valido: bool,
    pub faltantes: Vec<Faltante>,
    pub revocado: bool,
    /// La frase de una línea que pinta la pantalla.
    pub resumen: String,
}

/// Juzga un consentimiento. No lo modifica.
///
/// Se separa del guardado a propósito: la pantalla necesita saber qué falta
/// **mientras** se llena, y el informe necesita saberlo después. Si el juicio
/// viviera en la ruta que guarda, habría que duplicarlo.
///
/// Un texto en blanco cuenta como ausente —se recorta antes de mirarlo—, porque
/// un campo con un espacio dentro no es una explicación: es la forma que toma
/// «lo llené para que me dejara guardar».
pub fn evaluar(c: AJuzgar<'_>) -> Validez {
    if c.revocado {
        return Validez {
            valido: false,
            faltantes: vec![Faltante::Revocado],
            revocado: true,
            resumen: "Revocado por el paciente. No autoriza el procedimiento.".to_string(),
        };
    }

    let mut faltan = Vec::new();
    for (texto, cual) in [
        (c.descripcion_procedimiento, Faltante::Descripcion),
        (c.riesgos_explicados, Faltante::Riesgos),
        (c.beneficios_esperados, Faltante::Beneficios),
        (c.alternativas_explicadas, Faltante::Alternativas),
    ] {
        if texto.trim().is_empty() {
            faltan.push(cual);
        }
    }
    if !c.paciente_comprende {
        faltan.push(Faltante::Comprension);
    }
    if !c.acepta_procedimiento {
        faltan.push(Faltante::Aceptacion);
    }
    if c.firma_paciente_path.is_none_or(|f| f.trim().is_empty()) {
        faltan.push(Faltante::FirmaDelPaciente);
    }
    if c.firma_medico_path.is_none_or(|f| f.trim().is_empty()) {
        faltan.push(Faltante::FirmaDelMedico);
    }

    let resumen = if faltan.is_empty() {
        "Consentimiento informado completo.".to_string()
    } else {
        let lista: Vec<&str> = faltan.iter().map(|f| f.etiqueta()).collect();
        format!("No es un consentimiento válido: falta {}.", lista.join(", "))
    };
    Validez {
        valido: faltan.is_empty(),
        faltantes: faltan,
        revocado: false,
        resumen,
    }
}

/// Por qué no se pudo registrar una revocación. Hay **un solo** motivo posible,
/// y ese es el punto del tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoSePuedeRevocar;

impl NoSePuedeRevocar {
    pub fn mensaje(self) -> &'static str {
        "Escriba lo que el paciente manifestó al revocar. No hace falta que sea \
         un motivo válido —no tiene que darlo—, pero sí que quede constancia de \
         que revocó y de cuándo."
    }
}

impl std::fmt::Display for NoSePuedeRevocar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.mensaje())
    }
}

impl std::error::Error for NoSePuedeRevocar {}

/// Valida el motivo de una revocación y lo devuelve recortado.
///
/// **No recibe el consentimiento, y eso es el diseño.** No hay nada que
/// comprobar sobre su estado: el paciente puede retirar su consentimiento en
/// cualquier momento y sin dar razones, así que una función que mirara el estado
/// previo sería la puerta por donde entra la comprobación que un día impide
/// registrar una revocación. El motivo se pide únicamente para que quede
/// constancia de lo que dijo, y se acepta cualquier texto: no es una
/// justificación que la institución deba aprobar.
///
/// Que devuelva el motivo en vez de escribirlo es lo que hace que un
/// consentimiento ya revocado se pueda volver a revocar sin ruido, y que
/// revocar uno inválido, uno sin firmas o uno de hace tres años funcione igual.
pub fn motivo_de_revocacion(motivo: &str) -> Result<&str, NoSePuedeRevocar> {
    let limpio = motivo.trim();
    if limpio.is_empty() {
        return Err(NoSePuedeRevocar);
    }
    Ok(limpio)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El consentimiento que sí lo es. Es el control positivo de todo lo demás:
    /// sin él, un `evaluar` que dijera «inválido» a todo pasaría las pruebas de
    /// abajo enteras.
    fn completo() -> AJuzgar<'static> {
        AJuzgar {
            descripcion_procedimiento: "Colecistectomía laparoscópica",
            riesgos_explicados: "Sangrado, infección, lesión de vía biliar",
            beneficios_esperados: "Resolución del cuadro doloroso",
            alternativas_explicadas: "Manejo médico con dieta y analgesia",
            paciente_comprende: true,
            acepta_procedimiento: true,
            firma_paciente_path: Some("/firmas/p1.png"),
            firma_medico_path: Some("/firmas/m1.png"),
            revocado: false,
        }
    }

    #[test]
    fn el_completo_es_valido() {
        let v = evaluar(completo());
        assert!(v.valido, "{}", v.resumen);
        assert!(v.faltantes.is_empty());
        assert_eq!(v.resumen, "Consentimiento informado completo.");
    }

    /// LA REGLA QUE DA NOMBRE AL MÓDULO. Todo firmado, todo explicado, y el
    /// paciente no consta que haya comprendido: no es un consentimiento.
    #[test]
    fn con_las_dos_firmas_pero_sin_comprension_no_vale() {
        let mut c = completo();
        c.paciente_comprende = false;
        let v = evaluar(c);
        assert!(!v.valido);
        assert_eq!(v.faltantes, vec![Faltante::Comprension]);
    }

    #[test]
    fn sin_aceptacion_no_vale() {
        let mut c = completo();
        c.acepta_procedimiento = false;
        assert_eq!(evaluar(c).faltantes, vec![Faltante::Aceptacion]);
    }

    /// Barrido de los cuatro textos: cada uno, por separado, invalida.
    #[test]
    fn cada_explicacion_que_falta_se_nombra() {
        let casos: [(fn(&mut AJuzgar<'static>), Faltante); 4] = [
            (|c| c.descripcion_procedimiento = "", Faltante::Descripcion),
            (|c| c.riesgos_explicados = "", Faltante::Riesgos),
            (|c| c.beneficios_esperados = "", Faltante::Beneficios),
            (|c| c.alternativas_explicadas = "", Faltante::Alternativas),
        ];
        for (romper, esperado) in casos {
            let mut c = completo();
            romper(&mut c);
            let v = evaluar(c);
            assert!(!v.valido, "{esperado:?} vacío y salió válido");
            assert_eq!(v.faltantes, vec![esperado]);
        }
    }

    /// Un espacio no es una explicación. Es la forma que toma «lo llené para que
    /// me dejara guardar», y es indistinguible de haberlo explicado si no se
    /// recorta.
    #[test]
    fn el_texto_en_blanco_cuenta_como_ausente() {
        for relleno in ["   ", "\t", "\n", " \t\n "] {
            let mut c = completo();
            c.riesgos_explicados = relleno;
            assert_eq!(
                evaluar(c).faltantes,
                vec![Faltante::Riesgos],
                "{relleno:?} pasó por explicación"
            );
        }
    }

    /// Y su pareja: una explicación de verdad, aunque sea corta, SÍ cuenta. Sin
    /// esto, una regla que rechazara todo texto pasaría la prueba de arriba.
    #[test]
    fn una_explicacion_corta_de_verdad_si_cuenta() {
        let mut c = completo();
        c.riesgos_explicados = "Sangrado";
        assert!(evaluar(c).valido);
    }

    /// La ruta de la firma también se recorta: `Some("  ")` es una fila que
    /// PARECE traer firma. El fallo se ve en el informe, no en la pantalla.
    #[test]
    fn una_firma_en_blanco_no_es_una_firma() {
        let mut c = completo();
        c.firma_paciente_path = Some("  ");
        assert_eq!(evaluar(c).faltantes, vec![Faltante::FirmaDelPaciente]);

        let mut c = completo();
        c.firma_medico_path = None;
        assert_eq!(evaluar(c).faltantes, vec![Faltante::FirmaDelMedico]);
    }

    /// Un consentimiento vacío nombra las OCHO faltas, no la primera. La
    /// pantalla las pinta todas mientras se llena el formulario; una regla que
    /// cortara en la primera obligaría a guardar ocho veces para descubrirlas.
    #[test]
    fn el_vacio_nombra_las_ocho() {
        let v = evaluar(AJuzgar {
            descripcion_procedimiento: "",
            riesgos_explicados: "",
            beneficios_esperados: "",
            alternativas_explicadas: "",
            paciente_comprende: false,
            acepta_procedimiento: false,
            firma_paciente_path: None,
            firma_medico_path: None,
            revocado: false,
        });
        assert!(!v.valido);
        assert_eq!(
            v.faltantes,
            vec![
                Faltante::Descripcion,
                Faltante::Riesgos,
                Faltante::Beneficios,
                Faltante::Alternativas,
                Faltante::Comprension,
                Faltante::Aceptacion,
                Faltante::FirmaDelPaciente,
                Faltante::FirmaDelMedico,
            ]
        );
    }

    /// Revocado gana sobre todo lo demás, y llega SOLO. Enumerar además las
    /// faltas de captura de un consentimiento que el paciente ya retiró sería
    /// invitar a completarlas.
    #[test]
    fn revocado_invalida_aunque_este_completo() {
        let mut c = completo();
        c.revocado = true;
        let v = evaluar(c);
        assert!(!v.valido);
        assert!(v.revocado);
        assert_eq!(v.faltantes, vec![Faltante::Revocado]);
        assert_eq!(
            v.resumen,
            "Revocado por el paciente. No autoriza el procedimiento."
        );
    }

    #[test]
    fn el_motivo_de_revocacion_se_recorta() {
        assert_eq!(motivo_de_revocacion("  lo pensó mejor "), Ok("lo pensó mejor"));
    }

    #[test]
    fn sin_motivo_no_hay_constancia() {
        for vacio in ["", "   ", "\n\t"] {
            assert_eq!(motivo_de_revocacion(vacio), Err(NoSePuedeRevocar), "{vacio:?}");
        }
    }

    /// Cualquier texto sirve: la institución no aprueba el motivo. Un
    /// «porque sí» es una constancia igual de válida que un párrafo.
    #[test]
    fn cualquier_motivo_sirve() {
        for m in ["porque sí", "no", ".", "cambié de opinión"] {
            assert!(motivo_de_revocacion(m).is_ok(), "{m:?} fue rechazado");
        }
    }

    /// Las nueve etiquetas son distintas. Dos iguales dejarían a la pantalla sin
    /// forma de decir cuál de los dos campos hay que llenar.
    #[test]
    fn las_etiquetas_no_se_repiten() {
        let todas = [
            Faltante::Descripcion,
            Faltante::Riesgos,
            Faltante::Beneficios,
            Faltante::Alternativas,
            Faltante::Comprension,
            Faltante::Aceptacion,
            Faltante::FirmaDelPaciente,
            Faltante::FirmaDelMedico,
            Faltante::Revocado,
        ];
        let mut vistas: Vec<&str> = todas.iter().map(|f| f.etiqueta()).collect();
        vistas.sort_unstable();
        let antes = vistas.len();
        vistas.dedup();
        assert_eq!(vistas.len(), antes, "hay etiquetas repetidas");
    }
}
