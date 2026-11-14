// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Juan Carlos Isaza

//! El recolector de datos que faltan, para el RIPS (#65/#67).
//!
//! Es la primera pieza del port de `backend/rips_completo.py` a Rust, y va
//! primera porque es la que sostiene a todas las demás: cada campo de cada
//! documento pasa por aquí.
//!
//! # Por qué existe
//!
//! Antes cada hueco se rellenaba con una constante: el CUPS `"890201"`, el CUM
//! `"00000000"`, el diagnóstico `"R69"`, el valor del servicio en `0`. El
//! archivo salía siempre, pasaba la validación de estructura, y el problema
//! aparecía después — como glosa, como rechazo del MUV, o como un pago que
//! nunca llega porque el valor iba en cero.
//!
//! **Un RIPS que no sale se arregla en una tarde. Uno que sale con datos
//! inventados no se nota nunca.**
//!
//! # Falla al final, no en el primer hueco
//!
//! A propósito: quien cierra el mes necesita la lista COMPLETA para corregirla
//! de una vez. Reventar en la primera consulta de mil convierte el cierre en
//! mil intentos.
//!
//! # `0` no es ausente
//!
//! Un servicio puede valer cero de verdad. Confundir «no se registró» con «vale
//! cero» es el error que este módulo existe para no cometer, así que solo son
//! ausentes el `None` y la cadena en blanco.
//!
//! # Lo que el port añade sobre el original
//!
//! En Python, `dato()` devuelve el supuesto o `None` y **nada obliga al
//! llamante a mirar `faltantes`**: se puede generar un documento entero con
//! huecos y no llamar a `cerrar()`. Aquí el resultado es un [`Dato`], y el
//! único modo de sacar el valor es decidir explícitamente qué se hace con la
//! ausencia. La regla deja de depender de que alguien se acuerde
//! (ver [[afirmacion-necesita-su-prueba-en-el-tipo]]).

use crate::procedencia::{self, Falla, Respaldos};
use serde::Serialize;

/// Un dato obligatorio que no está —o que está y no vale—, con dónde buscarlo.
///
/// `motivo` separa los dos casos, y separarlos importa: «falta el CUM» se
/// corrige buscándolo; «el CUM 12345-6 no está en el catálogo del INVIMA» se
/// corrige revisando qué se digitó. Mandan a sitios distintos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Falta {
    /// `AC`, `AP`, `AM`, `AU`, `AH`.
    pub archivo: &'static str,
    /// El id del registro donde buscarlo: la historia, el procedimiento…
    pub registro_id: i64,
    /// El nombre del campo en la base, para quien va a corregirlo.
    pub campo: &'static str,
    /// Cómo se llama para un humano.
    pub etiqueta: &'static str,
    /// Qué se habría puesto si el llamante permitiera supuestos. Se registra
    /// aunque no se use: parte del valor del informe es saber qué se HABRÍA
    /// inventado.
    pub supuesto: Option<String>,
    /// `None` = el dato faltaba. `Some(_)` = estaba y no vale, y por qué.
    pub motivo: Option<String>,
}

impl std::fmt::Display for Falta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.motivo {
            None => write!(
                f,
                "{} #{}: falta {} ({})",
                self.archivo, self.registro_id, self.etiqueta, self.campo
            ),
            Some(m) => write!(
                f,
                "{} #{}: {} ({}) no vale — {m}",
                self.archivo, self.registro_id, self.etiqueta, self.campo
            ),
        }
    }
}

/// El resultado de pedir un dato obligatorio.
///
/// No es un `Option`: la diferencia está en que un `Option` se puede
/// `unwrap_or("890201")` sin que nadie se entere, y esto no. Para sacar el
/// valor hay que decir qué se hace con la ausencia, y las dos maneras de
/// hacerlo —[`Dato::o_nada`] y [`Dato::o_el_supuesto`]— dicen en su nombre
/// cuál es cuál.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dato {
    /// El valor estaba.
    Hay(String),
    /// No estaba. Ya quedó anotado en el recolector.
    Falta,
}

impl Dato {
    /// El valor, o `None` si faltaba. Es el camino normal: el JSON lleva `null`
    /// y el cierre fallará con la lista de lo que falta.
    pub fn o_nada(self) -> Option<String> {
        match self {
            Dato::Hay(v) => Some(v),
            Dato::Falta => None,
        }
    }

    /// El valor, o el supuesto. **Solo cuando el llamante pidió
    /// `permitir_supuestos`**; el recolector ya lo comprobó al construir esto.
    pub fn o_el_supuesto(self, supuesto: Option<&str>) -> Option<String> {
        match self {
            Dato::Hay(v) => Some(v),
            Dato::Falta => supuesto.map(str::to_string),
        }
    }

    pub fn hay(&self) -> bool {
        matches!(self, Dato::Hay(_))
    }
}

/// Lo mismo que [`Dato`] para los campos que el RIPS emite como NÚMERO:
/// `vrServicio`, `codServicio`, `cantidadMedicamento`.
///
/// No es un adorno del tipo: el MUV rechaza `"7.0"` y `"0"` donde espera `7` y
/// `0` —«Existen errores en el formato JSON»—, así que la diferencia entre
/// número y cadena tiene que sobrevivir hasta el serializador. Pasarlos por
/// [`Dato`] obligaría a convertir a texto y volver, y el viaje de vuelta es
/// donde un `7` se convierte en `7.0`.
///
/// Y aquí la regla de «vacío» es distinta a propósito: **solo `None` es
/// ausente**. Un servicio puede valer cero de verdad, y confundir «no se
/// registró» con «vale cero» es exactamente lo que este módulo existe para no
/// hacer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatoNumero {
    Hay(i64),
    Falta,
}

impl DatoNumero {
    pub fn o_nada(self) -> Option<i64> {
        match self {
            DatoNumero::Hay(v) => Some(v),
            DatoNumero::Falta => None,
        }
    }

    /// El valor, o el supuesto declarado en el [`Donde`]. El supuesto se declara
    /// como texto —es el mismo campo para todos los tipos— y se convierte aquí;
    /// si no fuera un número, el campo queda vacío en vez de llevar basura.
    pub fn o_el_supuesto(self, supuesto: Option<&str>) -> Option<i64> {
        match self {
            DatoNumero::Hay(v) => Some(v),
            DatoNumero::Falta => supuesto.and_then(|s| s.parse().ok()),
        }
    }

    pub fn hay(&self) -> bool {
        matches!(self, DatoNumero::Hay(_))
    }
}

/// Dónde buscar un dato que falte: el archivo, el registro, el campo, cómo se
/// llama para un humano, y qué se habría supuesto.
///
/// Existe porque esos cinco **siempre viajan juntos** —son literalmente los
/// campos de [`Falta`]— y pasarlos sueltos convertía cada llamada en seis
/// posiciones que sólo el orden distingue. Con esto la llamada dice qué es cada
/// cosa, y el constructor por archivo (`Donde::ac`) impide escribir `"AP"` en
/// una consulta.
#[derive(Debug, Clone)]
pub struct Donde {
    archivo: &'static str,
    registro_id: i64,
    campo: &'static str,
    etiqueta: &'static str,
    supuesto: Option<&'static str>,
}

impl Donde {
    pub fn ac(registro_id: i64, campo: &'static str, etiqueta: &'static str) -> Self {
        Self::en("AC", registro_id, campo, etiqueta)
    }
    pub fn ap(registro_id: i64, campo: &'static str, etiqueta: &'static str) -> Self {
        Self::en("AP", registro_id, campo, etiqueta)
    }
    pub fn am(registro_id: i64, campo: &'static str, etiqueta: &'static str) -> Self {
        Self::en("AM", registro_id, campo, etiqueta)
    }
    pub fn au(registro_id: i64, campo: &'static str, etiqueta: &'static str) -> Self {
        Self::en("AU", registro_id, campo, etiqueta)
    }
    pub fn ah(registro_id: i64, campo: &'static str, etiqueta: &'static str) -> Self {
        Self::en("AH", registro_id, campo, etiqueta)
    }
    /// AT — `otrosServicios` (#67). El registro es la fila de `dispensacion` del
    /// insumo/dispositivo, que es donde hay que ir a corregir.
    pub fn aos(registro_id: i64, campo: &'static str, etiqueta: &'static str) -> Self {
        Self::en("AT", registro_id, campo, etiqueta)
    }

    /// El bloque de identificación del usuario, que no pertenece a ningún
    /// archivo: es el mismo en los cinco y se escribe una vez por paciente. Sin
    /// esto, la falta de un municipio aparecería como si fuera de la consulta.
    pub fn usuarios(registro_id: i64, campo: &'static str, etiqueta: &'static str) -> Self {
        Self::en("usuarios", registro_id, campo, etiqueta)
    }

    fn en(
        archivo: &'static str,
        registro_id: i64,
        campo: &'static str,
        etiqueta: &'static str,
    ) -> Self {
        Self { archivo, registro_id, campo, etiqueta, supuesto: None }
    }

    /// Qué se habría puesto si el llamante permitiera supuestos. **Declararlo no
    /// lo usa**: se registra siempre, se emite solo con `permitir_supuestos`.
    pub fn supuesto(mut self, v: &'static str) -> Self {
        self.supuesto = Some(v);
        self
    }

    fn falta(&self, motivo: Option<String>) -> Falta {
        Falta {
            archivo: self.archivo,
            registro_id: self.registro_id,
            campo: self.campo,
            etiqueta: self.etiqueta,
            supuesto: self.supuesto.map(str::to_string),
            motivo,
        }
    }
}

/// Reúne TODO lo que falta antes de decidir si se puede generar.
///
/// Se crea fuera, se pasa a los generadores, y se inspecciona al terminar: quien
/// fuerza la generación con `permitir_supuestos` tiene que poder ver DESPUÉS qué
/// se supuso, para dejarlo en auditoría y mostrárselo a quien firmó.
///
/// `permitir_supuestos` es el camino permisivo que exige la directiva 20:
/// existe, **lo pide el llamante**, nunca se activa solo, y lo supuesto queda
/// registrado igual.
#[derive(Debug, Default)]
pub struct Recolector {
    permitir_supuestos: bool,
    faltantes: Vec<Falta>,
    /// La procedencia registrada del registro que se está emitiendo, o `None` si
    /// ese registro todavía no lleva procedencia (captura sin cablear). Se fija
    /// con [`Recolector::en_registro`] antes de emitir los campos de cada uno.
    respaldos: Option<Respaldos>,
    /// Campos que TIENEN valor pero cuya procedencia no respalda su uso: sin
    /// procedencia registrada, o de efecto jurídico/económico sin aval humano
    /// (#84). No bloquean la generación —son advertencia, como los supuestos—,
    /// pero salen en el cierre para que quien firma los vea.
    sin_respaldo: Vec<Falta>,
}

/// Lo que se devuelve cuando el cierre encuentra huecos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatosIncompletos(pub Vec<Falta>);

impl std::fmt::Display for DatosIncompletos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "el RIPS no se puede generar: faltan {} datos obligatorios",
            self.0.len()
        )?;
        for x in &self.0 {
            writeln!(f, "  · {x}")?;
        }
        Ok(())
    }
}

impl std::error::Error for DatosIncompletos {}

impl Recolector {
    pub fn estricto() -> Self {
        Self { permitir_supuestos: false, ..Default::default() }
    }

    /// El camino permisivo. Se llama así y no `nuevo(true)` para que en el
    /// sitio donde se usa se lea qué se está pidiendo.
    pub fn permitiendo_supuestos() -> Self {
        Self { permitir_supuestos: true, ..Default::default() }
    }

    pub fn permite_supuestos(&self) -> bool {
        self.permitir_supuestos
    }

    pub fn faltantes(&self) -> &[Falta] {
        &self.faltantes
    }

    /// Ausente es `None` o cadena en blanco. **`0` no lo es** — pero eso lo
    /// garantiza el tipo: aquí solo entran cadenas, y los numéricos se
    /// convierten ANTES por el llamante, que es quien sabe si su cero es un
    /// dato o un hueco.
    fn vacio(valor: Option<&str>) -> bool {
        match valor {
            None => true,
            Some(s) => s.trim().is_empty(),
        }
    }

    /// Pide un dato obligatorio. Si falta, lo anota y devuelve [`Dato::Falta`].
    pub fn dato(&mut self, valor: Option<&str>, donde: &Donde) -> Dato {
        if !Self::vacio(valor) {
            self.auditar_procedencia(donde);
            return Dato::Hay(valor.expect("no vacío implica Some").to_string());
        }
        self.faltantes.push(donde.falta(None));
        Dato::Falta
    }

    /// Pide un dato obligatorio que se emite como número. Ver [`DatoNumero`].
    ///
    /// Comparte el registro de faltas con [`Recolector::dato`] —una sola lista,
    /// un solo informe de cierre— y se separa solo en lo que de verdad difiere:
    /// aquí `0` es un valor y allá `"0"` también lo sería, pero allá la cadena
    /// en blanco es una ausencia y aquí no hay cadena en blanco que valga.
    pub fn numero(&mut self, valor: Option<i64>, donde: &Donde) -> DatoNumero {
        match valor {
            Some(v) => {
                self.auditar_procedencia(donde);
                DatoNumero::Hay(v)
            }
            None => {
                self.faltantes.push(donde.falta(None));
                DatoNumero::Falta
            }
        }
    }

    /// Como [`Recolector::dato`], pero además comprueba el valor contra un
    /// catálogo.
    ///
    /// Existe porque `dato` solo ve la AUSENCIA. Un código presente pero
    /// inexistente —un CUM mal digitado, un municipio que no está en DIVIPOLA—
    /// tiene la forma correcta, pasa cualquier validación de estructura y hace
    /// rechazar la factura entera en el MUV. Detectarlo aquí lo convierte en una
    /// línea del informe de cierre en vez de en un rechazo tres semanas después.
    ///
    /// Cuando el valor está pero NO vale, se devuelve **tal cual**, no el
    /// supuesto: sustituirlo escondería lo que el operador escribió, que es
    /// justo el dato que necesita ver para corregirlo.
    ///
    /// Y si el dato FALTABA, no se valida lo que se puso en su lugar: eso
    /// pondría dos líneas en el informe por un solo problema. Se cuenta la
    /// ausencia, que es la causa.
    pub fn codigo(
        &mut self,
        valor: Option<&str>,
        donde: &Donde,
        validar: impl Fn(&str) -> Result<(), String>,
    ) -> Dato {
        let d = self.dato(valor, donde);
        let Dato::Hay(v) = &d else { return d };
        if let Err(motivo) = validar(v) {
            self.no_vale(donde, motivo);
        }
        d
    }

    /// Anota un dato que **está y no vale**, cuando el motivo no sale de
    /// contrastarlo con un catálogo.
    ///
    /// Existe por la cantidad del AM: `2.5` está presente, es un número, y aun
    /// así el RIPS no lo admite. Pasarlo por [`Recolector::codigo`] obligaba a
    /// inventarse un valor de mentira para dárselo al validador —y un centinela
    /// en una lista de faltas reales es la clase de detalle que dentro de un año
    /// alguien lee como un dato.
    pub fn no_vale(&mut self, donde: &Donde, motivo: String) {
        self.faltantes.push(donde.falta(Some(motivo)));
    }

    /// El supuesto declarado para un sitio, o `None`. Lo necesita el llamante
    /// para elegir entre [`Dato::o_nada`] y [`Dato::o_el_supuesto`] sin tener
    /// que repetir la constante en dos lugares — que es como se desincronizan.
    pub fn supuesto_de(donde: &Donde) -> Option<&'static str> {
        donde.supuesto
    }

    /// Fija la procedencia (JSON `{campo: procedencia}`) del registro que se va a
    /// emitir. `None`/vacío = ese registro AÚN no lleva procedencia (captura sin
    /// cablear): su auditoría se salta —en vez de marcar todo como sin respaldo—,
    /// que es lo que hace seguro el despliegue incremental. En cuanto la captura
    /// escriba la procedencia de un registro, la auditoría se activa para él y
    /// caza los campos que se cuelen.
    ///
    /// Un JSON que no parsea NO rompe la generación del RIPS, pero tampoco exime:
    /// se audita como si el mapa estuviera vacío, o sea marcando todo lo que tiene
    /// valor. **Este párrafo decía lo contrario y era falso por partida doble.**
    /// Decía que un JSON ilegible «solo llegaría por manipulación directa de la
    /// base» — y llegaba por `POST /api/historias`, porque `HistoriaNueva` lo
    /// deserializaba del cuerpo (cerrado ya con `skip_deserializing`); y decía que
    /// su efecto era «benigno» — y era el peor posible, porque dejaba el registro
    /// fuera de toda auditoría sin diferencia observable.
    pub fn en_registro(&mut self, procedencias: Option<&str>) {
        // Sin columna (`None` o en blanco): fila anterior a la captura, se salta —
        // el despliegue incremental. CON columna, se audita SIEMPRE, y una que no
        // parsee se trata como el mapa vacío: todos sus campos con valor salen
        // `SinRespaldo`.
        //
        // Antes era un `.ok()`, y un JSON malo dejaba `None` —es decir, EXIMÍA el
        // registro—. Eso convertía un dato corrupto en un permiso, que es la peor
        // dirección posible: el estado más sospechoso producía el informe más
        // limpio, y era indistinguible de uno sano. La dirección segura es la
        // contraria (directiva 21), y no hay caso legítimo que la pague: lo que
        // escribe esta columna serializa con serde y siempre produce JSON válido.
        self.respaldos = procedencias
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| Respaldos::desde_json(Some(s)).unwrap_or_default());
    }

    /// Audita la procedencia de un campo PRESENTE contra los respaldos del
    /// registro actual. Silencioso si el registro no lleva procedencia.
    fn auditar_procedencia(&mut self, donde: &Donde) {
        let Some(resp) = &self.respaldos else { return };
        // Un campo con PUNTO nombra la columna de OTRA tabla —
        // `historia.diagnostico_principal` en el AM, `medico.numero_documento` en
        // los tres—, y ese valor no lo afirma quien captura ESTE registro: no lo
        // ve, y no tiene dónde confirmarlo. Auditarlo aquí marca «sin respaldo»
        // un dato que o bien está respaldado donde vive (el diagnóstico, en la
        // historia) o bien pertenece a otra pantalla (el documento del
        // profesional, en usuarios). Es la alarma que resulta falsa dos veces y
        // a la tercera nadie mira (directiva 23), y ya estaba viva en el AC desde
        // que la captura de la consulta escribe procedencia: toda historia con un
        // diagnóstico confirmado producía DOS faltas por el documento del médico.
        if donde.campo.contains('.') {
            return;
        }
        if let Some(falla) = procedencia::auditar_campo(donde.campo, true, resp) {
            let motivo = match falla {
                Falla::SinRespaldo => "valor sin procedencia registrada (posible valor por defecto)",
                Falla::DuroSinAval => "efecto jurídico/económico sin aval humano (derivado)",
            };
            self.sin_respaldo.push(donde.falta(Some(motivo.to_string())));
        }
    }

    /// Declara que un campo SALE con valor, para que su procedencia se audite
    /// (#84), sin pasar por [`Recolector::dato`] ni [`Recolector::numero`].
    ///
    /// La necesita el emisor que decide él mismo si hay valor y por eso no llama a
    /// ninguno de los dos. Hoy es uno: `cantidad_entera` del AM, que ante una
    /// cantidad entera devuelve por el camino corto — y así **la cantidad del
    /// medicamento se quedaba fuera de la auditoría justo cuando estaba presente**,
    /// que es el único caso en que hay algo que respaldar. No tocaba las faltas
    /// (una cantidad presente no falta) y por eso no lo veía nadie.
    pub fn presente(&mut self, donde: &Donde) {
        self.auditar_procedencia(donde);
    }

    /// Los campos que TIENEN valor pero cuya procedencia no lo respalda (#84).
    /// No bloquean; se muestran a quien firma.
    pub fn sin_respaldo(&self) -> &[Falta] {
        &self.sin_respaldo
    }

    /// Cierra la recolección. `Err` con la lista completa si hay huecos y no se
    /// permitieron supuestos.
    pub fn cerrar(self) -> Result<Vec<Falta>, DatosIncompletos> {
        if !self.faltantes.is_empty() && !self.permitir_supuestos {
            return Err(DatosIncompletos(self.faltantes));
        }
        Ok(self.faltantes)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn r() -> Recolector {
        Recolector::estricto()
    }

    #[test]
    fn un_dato_presente_pasa_sin_anotarse() {
        let mut rec = r();
        let d = rec.dato(Some("890201"), &Donde::ac(1, "cod_consulta_cups", "CUPS"));
        assert_eq!(d, Dato::Hay("890201".into()));
        assert!(rec.faltantes().is_empty());
    }

    /// #84: el recolector audita la PROCEDENCIA de los campos presentes, en el
    /// mismo paso donde emite el RIPS (sin deriva). Su pareja: se activa con la
    /// procedencia declarada y se SALTA cuando el registro aún no la lleva.
    #[test]
    fn audita_procedencia_de_los_campos_presentes() {
        let mut rec = r();
        rec.en_registro(Some(
            r#"{"cod_consulta_cups":"confirmado","cod_diagnostico_principal":"derivado"}"#,
        ));
        // CUPS confirmado por un humano → limpio.
        rec.dato(Some("890201"), &Donde::ac(1, "cod_consulta_cups", "CUPS"));
        assert!(rec.sin_respaldo().is_empty(), "un confirmado no se marca");
        // Diagnóstico (DURO) derivado por la máquina → sin aval humano.
        rec.dato(Some("R69"), &Donde::ac(1, "cod_diagnostico_principal", "diagnóstico"));
        // Un CUPS de procedimiento presente pero que NADIE afirmó → sin respaldo.
        rec.dato(Some("890301"), &Donde::ap(2, "cod_procedimiento", "CUPS del procedimiento"));
        assert_eq!(rec.sin_respaldo().len(), 2, "derivado-duro y sin-respaldo se marcan");

        // Un registro SIN procedencia declarada (captura aún sin cablear): no se
        // audita, en vez de marcar TODO como sin respaldo. Es lo que hace seguro
        // el despliegue incremental.
        let mut rec2 = r();
        rec2.en_registro(None);
        rec2.dato(Some("890201"), &Donde::ac(9, "cod_consulta_cups", "CUPS"));
        assert!(rec2.sin_respaldo().is_empty(), "sin procedencia declarada no se audita");

        // Y la cadena EN BLANCO es lo mismo que `None`: «pre-captura», se salta.
        // Es el defecto del padrón legado (migración 0069, `DEFAULT ''`): sin este
        // salto, cada fila del VFP marcaría SinRespaldo toda su demografía en el
        // primer cierre. Su pareja es `'{}'`, que SÍ audita (abajo).
        let mut rec3 = r();
        rec3.en_registro(Some(""));
        rec3.dato(Some("890201"), &Donde::ac(9, "cod_consulta_cups", "CUPS"));
        assert!(rec3.sin_respaldo().is_empty(), "blanco = pre-captura, no se audita");

        // Pareja: `'{}'` es «capturado y nada avalado» → SÍ marca el campo presente.
        let mut rec4 = r();
        rec4.en_registro(Some("{}"));
        rec4.dato(Some("890201"), &Donde::ac(9, "cod_consulta_cups", "CUPS"));
        assert_eq!(rec4.sin_respaldo().len(), 1, "mapa vacío sí audita lo presente");
    }

    #[test]
    fn un_dato_ausente_se_anota_y_no_se_inventa() {
        let mut rec = r();
        let d = rec.dato(None, &Donde::ac(7, "cod_consulta_cups", "CUPS").supuesto("890201"));
        assert_eq!(d, Dato::Falta);
        assert_eq!(d.o_nada(), None, "el camino normal NO pone el supuesto");
        assert_eq!(rec.faltantes().len(), 1);
        let f = &rec.faltantes()[0];
        assert_eq!(f.registro_id, 7);
        assert_eq!(f.supuesto.as_deref(), Some("890201"), "se registra qué se HABRÍA puesto");
        assert!(f.motivo.is_none(), "faltaba, no es que no valiera");
    }

    /// La cadena en blanco es ausencia. Es el caso que más se cuela: un campo de
    /// formulario que el operador dejó con espacios.
    #[test]
    fn la_cadena_en_blanco_cuenta_como_ausente() {
        let mut rec = r();
        for v in ["", "   ", "\t\n"] {
            assert_eq!(rec.dato(Some(v), &Donde::ac(1, "c", "e")), Dato::Falta, "{v:?}");
        }
        assert_eq!(rec.faltantes().len(), 3);
    }

    /// **`"0"` NO es ausente.** Un servicio puede valer cero de verdad, y
    /// confundir «no se registró» con «vale cero» es el error que todo este
    /// módulo existe para no cometer.
    #[test]
    fn el_cero_es_un_dato_y_no_un_hueco() {
        let mut rec = r();
        assert_eq!(
            rec.dato(Some("0"), &Donde::ac(1, "valor_servicio", "valor").supuesto("0")),
            Dato::Hay("0".into())
        );
        assert!(rec.faltantes().is_empty(), "un cero legítimo no es una falta");
    }

    // === EL CAMINO PERMISIVO ==============================================

    #[test]
    fn con_supuestos_permitidos_el_valor_sale_pero_queda_anotado() {
        let mut rec = Recolector::permitiendo_supuestos();
        let d = rec.dato(None, &Donde::ac(3, "cod_consulta_cups", "CUPS").supuesto("890201"));
        assert_eq!(d.o_el_supuesto(Some("890201")), Some("890201".to_string()));
        assert_eq!(rec.faltantes().len(), 1, "supuesto NO significa silencioso");
        // Y cierra sin error: el llamante lo pidió y sabrá qué hacer con la lista.
        assert_eq!(rec.cerrar().unwrap().len(), 1);
    }

    #[test]
    fn en_estricto_el_cierre_falla_con_la_lista_entera() {
        let mut rec = r();
        rec.dato(None, &Donde::ac(1, "a", "A"));
        rec.dato(None, &Donde::ac(2, "b", "B"));
        rec.dato(Some("vale"), &Donde::ac(3, "c", "C"));
        let e = rec.cerrar().unwrap_err();
        assert_eq!(e.0.len(), 2, "solo las dos que faltaban");
        // Falla AL FINAL y con TODO: quien cierra el mes lo corrige de una vez.
        let texto = e.to_string();
        assert!(texto.contains("faltan 2"), "{texto}");
        assert!(texto.contains("AC #1"), "{texto}");
        assert!(texto.contains("AC #2"), "{texto}");
    }

    #[test]
    fn sin_faltas_el_cierre_pasa() {
        let mut rec = r();
        rec.dato(Some("x"), &Donde::ac(1, "a", "A"));
        assert!(rec.cerrar().unwrap().is_empty());
    }

    // === CÓDIGOS CONTRA CATÁLOGO ==========================================

    fn solo_cups(v: &str) -> Result<(), String> {
        if v == "890201" { Ok(()) } else { Err(format!("«{v}» no está en el CUPS")) }
    }

    #[test]
    fn un_codigo_valido_no_se_anota() {
        let mut rec = r();
        let d = rec.codigo(Some("890201"), &Donde::ac(1, "c", "CUPS"), solo_cups);
        assert_eq!(d, Dato::Hay("890201".into()));
        assert!(rec.faltantes().is_empty());
    }

    /// **Un código presente pero inexistente se devuelve TAL CUAL.** Sustituirlo
    /// escondería lo que el operador escribió, que es justo lo que necesita ver
    /// para corregirlo.
    #[test]
    fn un_codigo_que_no_existe_se_anota_con_su_motivo_y_se_devuelve_igual() {
        let mut rec = r();
        let d = rec.codigo(Some("999999"), &Donde::ac(4, "c", "CUPS").supuesto("890201"), solo_cups);
        assert_eq!(d, Dato::Hay("999999".into()), "se devuelve lo que se digitó");
        assert_eq!(rec.faltantes().len(), 1);
        let f = &rec.faltantes()[0];
        assert!(f.motivo.is_some(), "estaba y no valía: eso es un motivo");
        assert!(f.motivo.as_ref().unwrap().contains("999999"), "{:?}", f.motivo);
        // Y el mensaje manda a revisar lo digitado, no a buscarlo.
        assert!(f.to_string().contains("no vale"), "{f}");
    }

    /// Si el dato FALTABA, no se valida lo que se puso en su lugar: eso pondría
    /// DOS líneas en el informe de cierre por un solo problema.
    #[test]
    fn un_codigo_ausente_se_cuenta_una_vez_y_no_dos() {
        let mut rec = r();
        let d = rec.codigo(None, &Donde::ac(5, "c", "CUPS").supuesto("no-vale-tampoco"), solo_cups);
        assert_eq!(d, Dato::Falta);
        assert_eq!(rec.faltantes().len(), 1, "una falta, no dos");
        assert!(rec.faltantes()[0].motivo.is_none(), "la causa es la ausencia");
    }

    /// El tipo obliga a decidir. Esta prueba no comprueba comportamiento sino
    /// que la API no ofrezca el atajo: si `Dato` fuera un `Option`, un
    /// `.unwrap_or("890201")` metería el supuesto sin anotarlo y sin que nadie
    /// lo viera en la revisión.
    #[test]
    fn el_tipo_no_deja_sacar_el_valor_sin_decidir() {
        let d = Dato::Falta;
        assert_eq!(d.clone().o_nada(), None);
        assert_eq!(d.o_el_supuesto(Some("890201")), Some("890201".into()));
        // Y con supuesto ausente, tampoco se inventa nada.
        assert_eq!(Dato::Falta.o_el_supuesto(None), None);
    }
}
