//! Procedencia de un dato clínico, la fricción que exige y la auditoría que eso
//! vuelve trivial (#84). Es el port a Rust de `backend/procedencia.py` +
//! `backend/auditoria_rips.py`, con una pieza NUEVA: la fricción graduada
//! anclada al validador de MinSalud.
//!
//! # El problema
//!
//! El clínico tiene doce minutos por paciente, así que el sistema predetermina
//! valores; la historia acaba llena de datos que nadie afirmó
//! (`cod_consulta_cups="890201"` en TODA consulta) e indistinguibles de los
//! reales. Quitar los defectos sin poner nada en su lugar solo hace la captura
//! más lenta, y lo lento se rodea. La salida no es exigir más: es **saber de
//! dónde vino cada cosa**, guardándolo CON el valor (`historia.procedencias`,
//! JSON `{campo: procedencia}`, migración 0005). Con eso la auditoría deja de
//! adivinar y solo consulta, y la pregunta se generaliza:
//!
//!   ¿qué campos tienen valor pero NO tienen procedencia registrada?
//!
//! Eso caza el defecto de CUALQUIER campo, no de los seis que alguien listó.
//!
//! # La fricción graduada (parte 4 de #84, anclada a evidencia)
//!
//! «Bloquear solo donde hay efecto jurídico o económico» no se opina: se mide
//! contra el validador FEV-RIPS de MinSalud. Un campo que el validador cruza
//! (reglas `RVC*`) mal puesto produce RECHAZO, y un RIPS rechazado no se paga.
//! De los resultados reales salieron `RVC019`/`RVC059` (CUPS × diagnóstico,
//! servicio, finalidad, sexo, edad), `RVC028` (CIE-10 × sexo/edad) y `RVC051`
//! (finalidad × sexo/edad); a eso se suman identidad, dinero y autorización.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Cómo llegó un valor a la historia. Los cuatro nombres son los de
/// `backend/procedencia.py` y los de la columna `procedencias` (migración 0005):
/// que coincidan lo ata la prueba `el_nombre_de_serde_y_el_de_la_base_coinciden`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Procedencia {
    /// Una persona lo escribió, o lo revisó y lo dio por bueno.
    Confirmado,
    /// Se calculó de otros datos que ya estaban. No cuesta tiempo al clínico y es
    /// reproducible, pero hereda los errores de aquellos. **No pasó por ojo humano.**
    Derivado,
    /// El sistema lo sugirió y alguien lo aceptó (con una tecla). Vale más que un
    /// defecto y menos que un confirmado; hay que medir cuántos se aceptan sin mirar.
    Propuesto,
    /// Se preguntó y no procede. Distinto de que FALTE: «no se midió el sangrado»
    /// y «no sangró» no son lo mismo. Es una afirmación humana, no un vacío.
    NoAplica,
}

impl Procedencia {
    /// Nombre estable para la base, serde y los logs.
    pub fn como_str(&self) -> &'static str {
        match self {
            Procedencia::Confirmado => "confirmado",
            Procedencia::Derivado => "derivado",
            Procedencia::Propuesto => "propuesto",
            Procedencia::NoAplica => "no_aplica",
        }
    }

    /// Desde el texto de la base. `None` si no es conocida (falla ruidoso, no cae
    /// a un defecto — directiva 20).
    pub fn desde_str(s: &str) -> Option<Self> {
        use serde::de::IntoDeserializer;
        let de: serde::de::value::StrDeserializer<serde::de::value::Error> = s.into_deserializer();
        <Self as serde::Deserialize>::deserialize(de).ok()
    }

    /// ¿Un humano puso o avaló este dato? `Confirmado` (lo escribió), `Propuesto`
    /// (lo aceptó) y `NoAplica` (dijo que no procede) SÍ; `Derivado` (lo calculó la
    /// máquina) NO. Es, en una función, la regla de que nada derivado entra como
    /// confirmado sin que alguien lo mire.
    pub fn avalado_por_humano(&self) -> bool {
        !matches!(self, Procedencia::Derivado)
    }
}

/// Cuánta fricción exige un campo al capturarlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Friccion {
    /// Efecto jurídico o económico: exige ojo humano. Un valor `Derivado` a secas
    /// no basta.
    Dura,
    /// Administrativo: se puede derivar o proponer y aceptar con una tecla.
    Suave,
}

impl Friccion {
    /// ¿La procedencia REGISTRADA de un dato satisface la fricción de su campo?
    /// - Dura: solo con aval humano (no `Derivado`).
    /// - Suave: cualquier procedencia registrada vale.
    ///
    /// La AUSENCIA de procedencia no se pregunta aquí (no hay `Procedencia` que
    /// pasar): la caza `auditar`, como «valor sin respaldo».
    pub fn satisfecha_por(&self, p: Procedencia) -> bool {
        match self {
            Friccion::Dura => p.avalado_por_humano(),
            Friccion::Suave => true,
        }
    }
}

/// Normaliza un nombre de campo para casar `cod_consulta` y `codConsulta`.
fn normaliza(campo: &str) -> String {
    campo.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// Campos PROVABLEMENTE administrativos: los únicos que fluyen con fricción suave.
/// Normalizados. La lista crece a mano: cada entrada afirma «este no arriesga ni
/// el pago ni la responsabilidad».
const SUAVES: &[&str] = &[
    "codpaisresidencia",
    "codpaisorigen",
    "codzonaterritorialresidencia",
    "consecutivo",
];

/// Clasifica un campo por su fricción.
///
/// **Dirección segura, deliberada (directiva 21):** se enumera lo provablemente
/// inofensivo (`SUAVES`); todo lo demás —lo que MinSalud valida, identidad,
/// dinero, autorización, Y lo aún no clasificado— es Dura. Un campo nuevo nace
/// PROTEGIDO. En la historia clínica un falso «fluye» esconde responsabilidad mal
/// atribuida; un falso «confirma» cuesta una tecla (asimetría de la directiva 20).
pub fn friccion_de(campo: &str) -> Friccion {
    if SUAVES.contains(&normaliza(campo).as_str()) {
        Friccion::Suave
    } else {
        Friccion::Dura
    }
}

// ============================================================================
// AUDITORÍA — la pregunta que se vuelve trivial al guardar la procedencia.
// Port de `auditoria_rips.py`: no infiere, CONSULTA el JSON `procedencias`.
// ============================================================================

/// Los `procedencias` de un registro, ya parseados del JSON `{campo: procedencia}`.
/// Un campo ausente del mapa es un campo SIN respaldo (nadie afirmó su valor).
#[derive(Debug, Clone, Default)]
pub struct Respaldos(BTreeMap<String, Procedencia>);

impl Respaldos {
    /// Parsea la columna `procedencias` (JSON). `None`/vacío → sin ningún respaldo.
    /// Un JSON ilegible NO se traga: devuelve error para que el llamante decida
    /// (directiva 20), en vez de fingir que no hay respaldos.
    pub fn desde_json(json: Option<&str>) -> Result<Self, serde_json::Error> {
        match json.map(str::trim).filter(|s| !s.is_empty()) {
            None => Ok(Respaldos::default()),
            Some(s) => Ok(Respaldos(serde_json::from_str(s)?)),
        }
    }

    /// La procedencia registrada de un campo, o `None` si nadie la afirmó.
    pub fn de(&self, campo: &str) -> Option<Procedencia> {
        let n = normaliza(campo);
        self.0.iter().find(|(k, _)| normaliza(k) == n).map(|(_, v)| *v)
    }
}

/// Por qué un campo de un registro sale marcado en la auditoría.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falla {
    /// Tiene valor pero NADIE afirmó de dónde salió: la firma de un valor por
    /// defecto. Antes era invisible porque la columna nunca estaba vacía.
    SinRespaldo,
    /// Es un campo de efecto jurídico/económico (fricción Dura) y su procedencia
    /// registrada NO tiene aval humano (está `Derivado`). El pago o la clasificación
    /// clínica dependen de él y lo puso la máquina sin que nadie lo mirara.
    DuroSinAval,
}

/// El resultado de auditar un campo de un registro: nada, o una falla.
///
/// `tiene_valor` lo decide el llamante (el dato puede ser texto, número o fecha).
/// Reglas, en orden:
///   1. sin valor → no es asunto de esta función (lo mira la completitud del RIPS).
///   2. valor con procedencia con aval humano y campo suave → limpio.
///   3. valor SIN procedencia → `SinRespaldo`.
///   4. valor con procedencia pero campo Dura sin aval → `DuroSinAval`.
pub fn auditar_campo(campo: &str, tiene_valor: bool, respaldos: &Respaldos) -> Option<Falla> {
    if !tiene_valor {
        return None;
    }
    match respaldos.de(campo) {
        None => Some(Falla::SinRespaldo),
        Some(p) => {
            if friccion_de(campo).satisfecha_por(p) {
                None
            } else {
                Some(Falla::DuroSinAval)
            }
        }
    }
}

// ============================================================================
// CAPTURA — construir la procedencia a guardar, con fricción graduada (opción 3).
// ============================================================================

/// Construye el mapa `{campo: procedencia}` a guardar con la historia, a partir de
/// lo que la captura envió. Fricción GRADUADA (la opción que eligió Juan):
///   - señal explícita del formulario → esa procedencia (el humano confirmó,
///     aceptó una propuesta, o marcó «no aplica»);
///   - sin señal y campo SUAVE → `Confirmado` implícito: el humano dejó un valor en
///     un campo administrativo; no hace falta molestarlo;
///   - sin señal y campo DURO → **se OMITE**: un campo de efecto jurídico/económico
///     exige toque explícito, y omitirlo hace que la auditoría lo marque
///     `SinRespaldo` en vez de fingir que alguien lo afirmó (directiva 20).
///
/// `campos_con_valor` son los campos que traen valor (los vacíos no se registran).
/// `señal(campo)` da la procedencia explícita del formulario, o `None`. Puro.
pub fn procedencias_de_captura<'a, F>(
    campos_con_valor: impl IntoIterator<Item = &'a str>,
    señal: F,
) -> BTreeMap<String, Procedencia>
where
    F: Fn(&str) -> Option<Procedencia>,
{
    let mut m = BTreeMap::new();
    for campo in campos_con_valor {
        let p = match señal(campo) {
            Some(p) => Some(p),
            None => match friccion_de(campo) {
                Friccion::Suave => Some(Procedencia::Confirmado),
                Friccion::Dura => None,
            },
        };
        if let Some(p) = p {
            m.insert(campo.to_string(), p);
        }
    }
    m
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_nombre_de_serde_y_el_de_la_base_coinciden() {
        for p in [
            Procedencia::Confirmado,
            Procedencia::Derivado,
            Procedencia::Propuesto,
            Procedencia::NoAplica,
        ] {
            assert_eq!(serde_json::to_string(&p).unwrap(), format!("\"{}\"", p.como_str()));
            assert_eq!(Procedencia::desde_str(p.como_str()), Some(p));
        }
        // Los cuatro nombres son los del modelo Python y la migración 0005.
        assert_eq!(Procedencia::desde_str("no_aplica"), Some(Procedencia::NoAplica));
        assert_eq!(Procedencia::desde_str("propuesto"), Some(Procedencia::Propuesto));
        assert_eq!(Procedencia::desde_str("inventado"), None);
    }

    #[test]
    fn solo_lo_derivado_carece_de_aval_humano() {
        assert!(Procedencia::Confirmado.avalado_por_humano());
        assert!(Procedencia::Propuesto.avalado_por_humano());
        assert!(Procedencia::NoAplica.avalado_por_humano(), "«no procede» lo dijo un humano");
        assert!(!Procedencia::Derivado.avalado_por_humano(), "lo derivado lo calculó la máquina");
    }

    /// El corazón de la fricción y su pareja (directiva 33): lo que MinSalud
    /// valida + identidad/dinero es DURA; lo administrativo, SUAVE. El caso rojo
    /// (un suave no sale Dura, un duro no sale Suave) descarta un clasificador ciego.
    #[test]
    fn friccion_discrimina_lo_facturable_de_lo_administrativo() {
        for c in [
            "codConsulta", "cod_consulta", "cod_procedimiento", "codDiagnosticoPrincipal",
            "cod_diagnostico_principal", "codDiagnosticoRelacionado1", "finalidadTecnologiaSalud",
            "causaMotivoAtencion", "numDocumentoIdentificacion", "codSexo", "fechaNacimiento",
            "vrServicio", "numAutorizacion", "codPrestador", "fechaInicioAtencion",
        ] {
            assert_eq!(friccion_de(c), Friccion::Dura, "{c} → dura");
        }
        for c in ["codPaisResidencia", "cod_pais_origen", "codZonaTerritorialResidencia", "consecutivo"] {
            assert_eq!(friccion_de(c), Friccion::Suave, "{c} → suave");
        }
    }

    #[test]
    fn un_campo_sin_clasificar_nace_protegido() {
        assert_eq!(friccion_de("campoNuevoQueNadieClasifico"), Friccion::Dura);
    }

    #[test]
    fn la_compuerta_pide_ojo_humano_solo_donde_pesa() {
        assert!(!Friccion::Dura.satisfecha_por(Procedencia::Derivado));
        assert!(Friccion::Dura.satisfecha_por(Procedencia::Confirmado));
        assert!(Friccion::Dura.satisfecha_por(Procedencia::Propuesto));
        assert!(Friccion::Dura.satisfecha_por(Procedencia::NoAplica));
        // Un campo suave acepta hasta lo derivado.
        assert!(Friccion::Suave.satisfecha_por(Procedencia::Derivado));
    }

    #[test]
    fn respaldos_parsea_y_normaliza_las_dos_grafias() {
        let r = Respaldos::desde_json(Some(r#"{"cod_consulta":"confirmado","codSexo":"derivado"}"#)).unwrap();
        // Se consulta con cualquiera de las dos grafías.
        assert_eq!(r.de("codConsulta"), Some(Procedencia::Confirmado));
        assert_eq!(r.de("cod_sexo"), Some(Procedencia::Derivado));
        assert_eq!(r.de("finalidad"), None, "campo no afirmado → sin respaldo");
        // Vacío/ausente → sin respaldos, sin reventar.
        assert!(Respaldos::desde_json(None).unwrap().de("x").is_none());
        // JSON ilegible NO se traga (directiva 20).
        assert!(Respaldos::desde_json(Some("{roto")).is_err());
    }

    /// La fricción graduada (opción 3) en la construcción de la procedencia, y su
    /// pareja: el campo DURO sin toque se OMITE (→ luego sale como sin respaldo);
    /// el SUAVE fluye implícito; la señal explícita manda en los dos.
    #[test]
    fn la_captura_grada_la_friccion() {
        let con_valor = ["cod_diagnostico_principal", "cod_pais_residencia", "cod_consulta"];
        // Sin ninguna señal explícita:
        let m = procedencias_de_captura(con_valor, |_| None);
        assert_eq!(m.get("cod_pais_residencia"), Some(&Procedencia::Confirmado), "suave → implícito");
        assert!(!m.contains_key("cod_diagnostico_principal"), "duro sin toque → omitido");
        assert!(!m.contains_key("cod_consulta"), "duro sin toque → omitido");
        // Con toque explícito en los duros:
        let señales = |c: &str| match c {
            "cod_diagnostico_principal" => Some(Procedencia::Confirmado),
            "cod_consulta" => Some(Procedencia::Propuesto),
            _ => None,
        };
        let m2 = procedencias_de_captura(con_valor, señales);
        assert_eq!(m2.get("cod_diagnostico_principal"), Some(&Procedencia::Confirmado));
        assert_eq!(m2.get("cod_consulta"), Some(&Procedencia::Propuesto), "propuesta aceptada");
        assert_eq!(m2.get("cod_pais_residencia"), Some(&Procedencia::Confirmado));
    }

    /// La auditoría generalizada y su pareja. Es el port de `auditoria_rips.auditar`:
    /// caza el defecto de CUALQUIER campo, no de una lista de seis.
    #[test]
    fn la_auditoria_caza_el_valor_por_defecto_y_el_duro_sin_aval() {
        let r = Respaldos::desde_json(Some(
            r#"{"cod_consulta":"confirmado","cod_diagnostico_principal":"derivado","cod_pais_origen":"derivado"}"#,
        )).unwrap();

        // VERDE: campo duro con confirmado → limpio.
        assert_eq!(auditar_campo("cod_consulta", true, &r), None);
        // ROJO 1: CUPS con valor pero SIN respaldo → el defecto heredado, antes invisible.
        assert_eq!(auditar_campo("cod_procedimiento", true, &r), Some(Falla::SinRespaldo));
        // ROJO 2: diagnóstico (DURO) con procedencia DERIVADO → sin aval humano.
        assert_eq!(auditar_campo("cod_diagnostico_principal", true, &r), Some(Falla::DuroSinAval));
        // VERDE: un administrativo derivado fluye.
        assert_eq!(auditar_campo("cod_pais_origen", true, &r), None);
        // Sin valor: no es asunto de esta función.
        assert_eq!(auditar_campo("cod_consulta", false, &r), None);
    }
}
