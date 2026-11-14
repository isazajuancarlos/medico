//! Registro de nacimiento (#249), y la fuente del arreglo `recienNacidos` de la
//! Resolución 948 (#67).
//!
//! # Por qué existe este módulo y no otro de los 22
//!
//! De los 22 módulos del catálogo que responden 501, éste es el único con motivo
//! EXTERNO: la 948 define siete arreglos y medico emite cinco. `recienNacidos` no
//! faltaba por pereza — faltaba porque no había dónde capturar el dato. Los otros
//! 21 son catálogo aspiracional (medido el 2026-08-15: ninguno tiene formulario en
//! el VFP vivo del prestador).
//!
//! # El ámbito se aplica por la MADRE
//!
//! `recien_nacido` no lleva `institucion_id`, igual que `historia`: la institución
//! la da el paciente. Aquí el paciente es la MADRE, y por ella se filtra. Ajeno ==
//! inexistente, como en el resto del dominio.
//!
//! # Lo que este módulo NO decide
//!
//! No decide si el recién nacido se convierte en paciente. Esa es una atención
//! distinta —con su propia historia y su propia identificación— y hoy la columna
//! `paciente_id` existe para cuando ocurra, siempre nula. Rellenarla sola sería
//! crear un paciente sin documento.

use medico_dominio::Ambito;
use serde::{Deserialize, Serialize};

use crate::pertenencia::{opcional_es_del_paciente, Enlace};

/// Los tres documentos que un recién nacido puede traer, del catálogo
/// `Rips.REFs_TipoDoc` del validador del MUV.
pub const TIPOS_DOCUMENTO: [&str; 3] = ["CN", "RC", "MS"];

/// Las ocho condiciones de destino al egreso, de `Rips.REFs_CondicionDestino`.
/// Van con su nombre porque la pantalla los enseña y el código los envía.
pub const CONDICIONES_DESTINO: [(&str, &str); 8] = [
    ("01", "Paciente con destino a su domicilio"),
    ("02", "Paciente muerto"),
    ("03", "Paciente derivado a otro servicio"),
    ("04", "Referido a otra institución"),
    ("05", "Contrarreferido a otra institución"),
    ("06", "Derivado o referido a hospitalización domiciliaria"),
    ("07", "Derivado a servicio social"),
    ("08", "Paciente continúa en el servicio (corte de facturación)"),
];

/// El código de `condicion_destino` que significa que el recién nacido murió.
///
/// Se nombra en vez de escribirse `"02"` por ahí suelto: de él depende que la
/// causa básica de muerte sea exigible, y una constante con nombre es lo que
/// permite encontrar todos sus usos.
pub const DESTINO_MUERTO: &str = "02";

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct RecienNacido {
    pub id: i64,
    pub madre_paciente_id: i64,
    pub hospitalizacion_id: Option<i64>,
    pub paciente_id: Option<i64>,
    pub tipo_documento: Option<String>,
    pub numero_documento: Option<String>,
    pub fecha_nacimiento: String,
    pub edad_gestacional: i64,
    pub num_consultas_prenatal: i64,
    pub sexo_biologico: String,
    pub peso_gramos: i64,
    pub diagnostico_principal: String,
    pub condicion_destino: Option<String>,
    pub diagnostico_causa_muerte: Option<String>,
    pub fecha_egreso: Option<String>,
    pub codigo_vida: Option<String>,
    pub creado_en: String,
    /// Unidos para la pantalla, como en `hospitalizacion`.
    pub madre_nombre: String,
    pub madre_identificacion: String,
}

/// Lo que la pantalla envía al registrar un nacimiento.
#[derive(Debug, Clone, Deserialize)]
pub struct Nuevo {
    pub madre_paciente_id: i64,
    pub hospitalizacion_id: Option<i64>,
    pub tipo_documento: Option<String>,
    pub numero_documento: Option<String>,
    pub fecha_nacimiento: String,
    pub edad_gestacional: i64,
    pub num_consultas_prenatal: i64,
    pub sexo_biologico: String,
    pub peso_gramos: i64,
    pub diagnostico_principal: String,
    pub condicion_destino: Option<String>,
    pub diagnostico_causa_muerte: Option<String>,
    pub fecha_egreso: Option<String>,
    pub codigo_vida: Option<String>,
}

/// Por qué se rechaza un registro de nacimiento.
///
/// Enumerado y no `String` por la misma razón que en `ihce::Faltas`: un error que
/// lleva el valor dentro acaba en un log, y aquí los valores son datos de salud.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Falta {
    /// La madre no existe, o es de otra institución. Las dos cosas se responden
    /// igual: quien no puede verla no puede saber cuál de las dos es.
    MadreInexistente,
    TipoDocumentoDesconocido,
    /// Documento a medias: tipo sin número o número sin tipo. Los dos juntos o
    /// ninguno — un número sin tipo no identifica a nadie.
    DocumentoIncompleto,
    SexoDesconocido,
    EdadGestacionalImposible,
    ConsultasPrenatalNegativas,
    PesoImposible,
    DiagnosticoPrincipalVacio,
    CondicionDestinoDesconocida,
    /// Murió y no se dijo de qué. La 948 exige la causa básica cuando la
    /// condición de destino es `02`, y NO se deduce del destino.
    MuertoSinCausa,
    /// Causa de muerte en alguien que no murió: o el destino está mal o la causa
    /// sobra, y adivinar cuál es inventar.
    CausaSinMuerte,
    /// La atención que se enlaza **no es de esta madre** — o no existe, o es
    /// de otra institución. Las tres se responden igual: distinguirlas diría que
    /// existe.
    AtencionQueNoEsDeLaMadre,
}

impl Falta {
    /// Una etiqueta estable para viajar por la URL de vuelta a la pantalla.
    ///
    /// El mensaje NO viaja: si el texto fuera el que va en el enlace, cualquiera
    /// podría hacerle decir a la pantalla lo que quisiera con una URL a mano, y
    /// además habría que codificarlo. La clave es un conjunto cerrado y el texto
    /// lo pone [`Falta::mensaje`], que es la única redacción que existe.
    pub fn clave(self) -> &'static str {
        match self {
            Falta::MadreInexistente => "madre_inexistente",
            Falta::TipoDocumentoDesconocido => "tipo_documento",
            Falta::DocumentoIncompleto => "documento_incompleto",
            Falta::SexoDesconocido => "sexo",
            Falta::EdadGestacionalImposible => "edad_gestacional",
            Falta::ConsultasPrenatalNegativas => "consultas_prenatal",
            Falta::PesoImposible => "peso",
            Falta::DiagnosticoPrincipalVacio => "diagnostico",
            Falta::CondicionDestinoDesconocida => "condicion_destino",
            Falta::MuertoSinCausa => "muerto_sin_causa",
            Falta::CausaSinMuerte => "causa_sin_muerte",
            Falta::AtencionQueNoEsDeLaMadre => "atencion_que_no_es_de_la_madre",
        }
    }

    /// La vuelta de [`Falta::clave`]. Devuelve `None` para lo que no reconoce —
    /// una clave inventada no pinta nada, en vez de pintar un texto cualquiera.
    pub fn de_clave(clave: &str) -> Option<Falta> {
        TODAS.iter().copied().find(|f| f.clave() == clave)
    }

    pub fn mensaje(self) -> &'static str {
        match self {
            Falta::MadreInexistente => "la madre no existe en esta institución",
            Falta::TipoDocumentoDesconocido => {
                "el tipo de documento debe ser CN (certificado de nacido vivo), RC (registro civil) o MS (menor sin identificar)"
            }
            Falta::DocumentoIncompleto => "el tipo y el número de documento van los dos o ninguno",
            Falta::SexoDesconocido => "el sexo biológico debe ser M, F o I",
            Falta::EdadGestacionalImposible => "la edad gestacional va en semanas, entre 20 y 45",
            Falta::ConsultasPrenatalNegativas => "el número de consultas de control prenatal no puede ser negativo",
            Falta::PesoImposible => "el peso del recién nacido va en GRAMOS, entre 200 y 8000",
            Falta::DiagnosticoPrincipalVacio => "el diagnóstico principal es obligatorio",
            Falta::CondicionDestinoDesconocida => "la condición de destino al egreso no está en la tabla del MUV",
            Falta::MuertoSinCausa => {
                "si la condición de destino es 02 (paciente muerto) hay que registrar la causa básica de muerte"
            }
            Falta::CausaSinMuerte => {
                "hay causa de muerte registrada pero la condición de destino no es 02 (paciente muerto)"
            }
            Falta::AtencionQueNoEsDeLaMadre => {
                "la hospitalización que se enlaza no es de esa madre"
            }
        }
    }
}

/// Las once, para poder recorrerlas. Una lista escrita a mano se desincroniza
/// del enumerado, así que la prueba `las_once_faltas_van_y_vuelven` la ata.
pub const TODAS: [Falta; 12] = [
    Falta::MadreInexistente,
    Falta::TipoDocumentoDesconocido,
    Falta::DocumentoIncompleto,
    Falta::SexoDesconocido,
    Falta::EdadGestacionalImposible,
    Falta::ConsultasPrenatalNegativas,
    Falta::PesoImposible,
    Falta::DiagnosticoPrincipalVacio,
    Falta::CondicionDestinoDesconocida,
    Falta::MuertoSinCausa,
    Falta::CausaSinMuerte,
    Falta::AtencionQueNoEsDeLaMadre,
];

/// Comprueba el registro ANTES de tocar la base.
///
/// Va aparte de `crear` y es público a propósito: así la prueba puede recorrer
/// los once rechazos sin montar una base, y la pantalla puede pintar el mensaje
/// sin haber intentado escribir.
///
/// El `CHECK` de la migración cubre lo mismo por debajo. No es duplicación
/// ociosa: el de SQLite es la barandilla que impide que una fila mala ENTRE por
/// cualquier puerta —un importador, una migración de datos—, y éste es el que
/// puede decir POR QUÉ. Un `SQLITE_CONSTRAINT` no se le puede enseñar a nadie.
pub fn revisar(n: &Nuevo) -> Result<(), Falta> {
    match (&n.tipo_documento, &n.numero_documento) {
        (Some(t), Some(num)) => {
            if !TIPOS_DOCUMENTO.contains(&t.as_str()) {
                return Err(Falta::TipoDocumentoDesconocido);
            }
            if num.trim().is_empty() {
                return Err(Falta::DocumentoIncompleto);
            }
        }
        (None, None) => {}
        _ => return Err(Falta::DocumentoIncompleto),
    }
    if !["M", "F", "I"].contains(&n.sexo_biologico.as_str()) {
        return Err(Falta::SexoDesconocido);
    }
    if !(20..=45).contains(&n.edad_gestacional) {
        return Err(Falta::EdadGestacionalImposible);
    }
    if n.num_consultas_prenatal < 0 {
        return Err(Falta::ConsultasPrenatalNegativas);
    }
    if !(200..=8000).contains(&n.peso_gramos) {
        return Err(Falta::PesoImposible);
    }
    if n.diagnostico_principal.trim().is_empty() {
        return Err(Falta::DiagnosticoPrincipalVacio);
    }
    let murio = n.condicion_destino.as_deref() == Some(DESTINO_MUERTO);
    if let Some(c) = n.condicion_destino.as_deref()
        && !CONDICIONES_DESTINO.iter().any(|(cod, _)| *cod == c)
    {
        return Err(Falta::CondicionDestinoDesconocida);
    }
    let hay_causa = n
        .diagnostico_causa_muerte
        .as_deref()
        .is_some_and(|c| !c.trim().is_empty());
    if murio && !hay_causa {
        return Err(Falta::MuertoSinCausa);
    }
    if hay_causa && !murio {
        return Err(Falta::CausaSinMuerte);
    }
    Ok(())
}

const SELECT_ENRIQUECIDO: &str = "\
    SELECT rn.id, rn.madre_paciente_id, rn.hospitalizacion_id, rn.paciente_id, \
    rn.tipo_documento, rn.numero_documento, rn.fecha_nacimiento, rn.edad_gestacional, \
    rn.num_consultas_prenatal, rn.sexo_biologico, rn.peso_gramos, rn.diagnostico_principal, \
    rn.condicion_destino, rn.diagnostico_causa_muerte, rn.fecha_egreso, rn.codigo_vida, \
    rn.creado_en, \
    (p.nombre1 || COALESCE(' '||p.nombre2,'') || ' ' || p.apellido1 || COALESCE(' '||p.apellido2,'')) AS madre_nombre, \
    p.numero_documento AS madre_identificacion \
    FROM recien_nacido rn JOIN paciente p ON p.id = rn.madre_paciente_id";

/// La institución del ámbito, o `None` si el ámbito no puede ver nada.
fn institucion(ambito: Ambito) -> Option<Option<i64>> {
    match ambito {
        Ambito::Superadmin => Some(None),
        Ambito::Institucion(i) => Some(Some(i)),
        Ambito::Ninguno => None,
    }
}

/// Lista los nacimientos del ámbito, del más reciente al más antiguo.
pub async fn listar(
    pool: &crate::Pool,
    ambito: Ambito,
    desde: Option<&str>,
    hasta: Option<&str>,
) -> Result<Vec<RecienNacido>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(Vec::new());
    };
    let mut sql = String::from(SELECT_ENRIQUECIDO);
    sql.push_str(" WHERE 1=1");
    if desde.is_some() {
        sql.push_str(" AND date(rn.fecha_nacimiento) >= date(?)");
    }
    if hasta.is_some() {
        sql.push_str(" AND date(rn.fecha_nacimiento) <= date(?)");
    }
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    sql.push_str(" ORDER BY rn.fecha_nacimiento DESC, rn.id DESC");
    let mut q = sqlx::query_as::<_, RecienNacido>(&sql);
    if let Some(d) = desde {
        q = q.bind(d);
    }
    if let Some(h) = hasta {
        q = q.bind(h);
    }
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_all(pool).await
}

/// Un nacimiento por su id, dentro del ámbito.
pub async fn uno(
    pool: &crate::Pool,
    ambito: Ambito,
    id: i64,
) -> Result<Option<RecienNacido>, sqlx::Error> {
    let Some(inst) = institucion(ambito) else {
        return Ok(None);
    };
    let mut sql = String::from(SELECT_ENRIQUECIDO);
    sql.push_str(" WHERE rn.id = ?");
    if inst.is_some() {
        sql.push_str(" AND p.institucion_id = ?");
    }
    let mut q = sqlx::query_as::<_, RecienNacido>(&sql).bind(id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    q.fetch_optional(pool).await
}

/// Registra un nacimiento. Devuelve el id.
///
/// Comprueba la madre DENTRO del ámbito antes de escribir: sin eso, una
/// institución podría colgarle un recién nacido a la paciente de otra, y el
/// arreglo `recienNacidos` saldría en el FEV de quien no la atendió.
pub async fn crear(
    pool: &crate::Pool,
    ambito: Ambito,
    n: &Nuevo,
    usuario_id: Option<i64>,
) -> Result<Result<i64, Falta>, sqlx::Error> {
    if let Err(f) = revisar(n) {
        return Ok(Err(f));
    }
    let Some(inst) = institucion(ambito) else {
        return Ok(Err(Falta::MadreInexistente));
    };
    let mut comprobar = String::from("SELECT 1 FROM paciente WHERE id = ?");
    if inst.is_some() {
        comprobar.push_str(" AND institucion_id = ?");
    }
    let mut q = sqlx::query_scalar::<_, i64>(&comprobar).bind(n.madre_paciente_id);
    if let Some(i) = inst {
        q = q.bind(i);
    }
    if q.fetch_optional(pool).await?.is_none() {
        return Ok(Err(Falta::MadreInexistente));
    }
    // El ingreso que se enlaza tiene que ser de LA MADRE. Sin esto se ataba en
    // crudo el id que mandara el cliente, y una institución podía colgar su
    // registro de nacimiento de una hospitalización de otra.
    if !opcional_es_del_paciente(
        pool,
        Enlace::Hospitalizacion,
        n.hospitalizacion_id,
        n.madre_paciente_id,
    )
    .await?
    {
        return Ok(Err(Falta::AtencionQueNoEsDeLaMadre));
    }

    let ahora = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO recien_nacido (madre_paciente_id, hospitalizacion_id, tipo_documento, \
         numero_documento, fecha_nacimiento, edad_gestacional, num_consultas_prenatal, \
         sexo_biologico, peso_gramos, diagnostico_principal, condicion_destino, \
         diagnostico_causa_muerte, fecha_egreso, codigo_vida, creado_en, creado_por) \
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(n.madre_paciente_id)
    .bind(n.hospitalizacion_id)
    .bind(n.tipo_documento.as_deref())
    .bind(n.numero_documento.as_deref())
    .bind(&n.fecha_nacimiento)
    .bind(n.edad_gestacional)
    .bind(n.num_consultas_prenatal)
    .bind(&n.sexo_biologico)
    .bind(n.peso_gramos)
    .bind(&n.diagnostico_principal)
    .bind(n.condicion_destino.as_deref())
    .bind(n.diagnostico_causa_muerte.as_deref())
    .bind(n.fecha_egreso.as_deref())
    .bind(n.codigo_vida.as_deref())
    .bind(&ahora)
    .bind(usuario_id)
    .fetch_one(pool)
    .await?;
    Ok(Ok(id))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn valido() -> Nuevo {
        Nuevo {
            madre_paciente_id: 1,
            hospitalizacion_id: None,
            tipo_documento: Some("CN".into()),
            numero_documento: Some("9900123456".into()),
            fecha_nacimiento: "2026-08-16T03:20:00Z".into(),
            edad_gestacional: 39,
            num_consultas_prenatal: 7,
            sexo_biologico: "F".into(),
            peso_gramos: 3250,
            diagnostico_principal: "Z380".into(),
            condicion_destino: Some("01".into()),
            diagnostico_causa_muerte: None,
            fecha_egreso: Some("2026-08-18T10:00:00Z".into()),
            codigo_vida: Some("VIDA-2026-0001".into()),
        }
    }

    #[test]
    fn el_nacimiento_normal_pasa() {
        assert_eq!(revisar(&valido()), Ok(()));
    }

    /// La pareja del de arriba, y la razón de que este módulo tenga `revisar`
    /// aparte: cada rechazo con su caso, no uno genérico.
    #[test]
    fn cada_falta_tiene_su_caso_rojo() {
        type Romper = Box<dyn Fn(&mut Nuevo)>;
        let casos: Vec<(Falta, Romper)> = vec![
            (
                Falta::TipoDocumentoDesconocido,
                Box::new(|n: &mut Nuevo| n.tipo_documento = Some("CC".into())),
            ),
            (
                Falta::DocumentoIncompleto,
                Box::new(|n: &mut Nuevo| n.numero_documento = None),
            ),
            (
                Falta::SexoDesconocido,
                Box::new(|n: &mut Nuevo| n.sexo_biologico = "X".into()),
            ),
            (
                Falta::EdadGestacionalImposible,
                Box::new(|n: &mut Nuevo| n.edad_gestacional = 19),
            ),
            (
                Falta::ConsultasPrenatalNegativas,
                Box::new(|n: &mut Nuevo| n.num_consultas_prenatal = -1),
            ),
            (
                Falta::PesoImposible,
                Box::new(|n: &mut Nuevo| n.peso_gramos = 3),
            ),
            (
                Falta::DiagnosticoPrincipalVacio,
                Box::new(|n: &mut Nuevo| n.diagnostico_principal = "  ".into()),
            ),
            (
                Falta::CondicionDestinoDesconocida,
                Box::new(|n: &mut Nuevo| n.condicion_destino = Some("99".into())),
            ),
            (
                Falta::MuertoSinCausa,
                Box::new(|n: &mut Nuevo| n.condicion_destino = Some("02".into())),
            ),
            (
                Falta::CausaSinMuerte,
                Box::new(|n: &mut Nuevo| n.diagnostico_causa_muerte = Some("P219".into())),
            ),
        ];
        for (esperada, romper) in casos {
            let mut n = valido();
            romper(&mut n);
            assert_eq!(
                revisar(&n),
                Err(esperada),
                "este caso tenía que dar {esperada:?}"
            );
        }
    }

    /// El peso en KILOS es el error que la unidad en el nombre existe para
    /// impedir, y tiene que rebotar: 3,25 kg tecleados como `3` son 3 gramos.
    #[test]
    fn el_peso_en_kilos_rebota() {
        let mut n = valido();
        n.peso_gramos = 3;
        assert_eq!(revisar(&n), Err(Falta::PesoImposible));
    }

    /// Sin documento es LEGÍTIMO y tiene que pasar: un parto de madrugada no
    /// tiene certificado de nacido vivo hasta que alguien lo expide. Es la
    /// pareja de `DocumentoIncompleto` por el otro lado.
    #[test]
    fn sin_documento_todavia_es_legitimo() {
        let mut n = valido();
        n.tipo_documento = None;
        n.numero_documento = None;
        assert_eq!(revisar(&n), Ok(()));
    }

    /// Un muerto CON su causa pasa. Sin esta prueba, `MuertoSinCausa` podría
    /// estar rechazando toda defunción y nadie lo notaría.
    #[test]
    fn el_muerto_con_su_causa_pasa() {
        let mut n = valido();
        n.condicion_destino = Some("02".into());
        n.diagnostico_causa_muerte = Some("P219".into());
        assert_eq!(revisar(&n), Ok(()));
    }

    /// Toda falta va y vuelve por su clave, y ninguna clave se repite. Es lo
    /// que hace fiable el viaje por la URL: si alguien añade una variante y no
    /// le pone clave, o le pone una repetida, esto se pone rojo.
    #[test]
    fn las_doce_faltas_van_y_vuelven() {
        let mut vistas = std::collections::BTreeSet::new();
        for f in TODAS {
            assert!(vistas.insert(f.clave()), "clave repetida: {}", f.clave());
            assert_eq!(Falta::de_clave(f.clave()), Some(f));
        }
        assert_eq!(vistas.len(), 12, "TODAS se quedó corta respecto del enumerado");
        assert_eq!(Falta::de_clave("lo_que_sea"), None);
    }

    /// Los ocho códigos del MUV se aceptan. Si alguien recorta la lista, esto se
    /// pone rojo antes de que el paquete lo rechace el validador.
    #[test]
    fn las_ocho_condiciones_de_destino_del_muv_se_aceptan() {
        for (cod, _) in CONDICIONES_DESTINO {
            let mut n = valido();
            n.condicion_destino = Some(cod.into());
            if cod == DESTINO_MUERTO {
                n.diagnostico_causa_muerte = Some("P219".into());
            }
            assert_eq!(revisar(&n), Ok(()), "el código {cod} es del MUV y se rechazó");
        }
    }
}

/// Las pruebas contra BASE REAL: el `revisar` de arriba se mide sin base, pero
/// el aislamiento por institución y el `CHECK` de la migración solo se pueden
/// medir escribiendo.
#[cfg(test)]
mod contra_la_base {
    use super::*;
    use medico_dominio::Ambito;

    /// Dos instituciones con una madre cada una: la 2 existe para que el
    /// aislamiento tenga algo que NO deba salir.
    async fn base() -> crate::Pool {
        let pool = crate::abrir("sqlite::memory:").await.unwrap();
        sqlx::query(
            "INSERT INTO institucion (id,nombre,nit,codigo_habilitacion) \
             VALUES (1,'Uno','900123456','050010123401'),(2,'Dos','900999','050010999901')",
        )
        .execute(&pool)
        .await
        .unwrap();
        // El usuario que registra: `creado_por` es una FK de verdad, y sin esta
        // fila el INSERT revienta con `FOREIGN KEY constraint failed` — que es
        // justamente lo que se quiere que pase si alguien inventa un autor.
        sqlx::query(
            "INSERT INTO usuario (id,nombre,email,password_hash,rol,institucion_id,activo) \
             VALUES (5,'Dra','d@x.co','h','medico',1,1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO paciente (id,tipo_documento,numero_documento,nombre1,apellido1,\
             fecha_nacimiento,sexo,institucion_id) \
             VALUES (10,'CC','1','Ana','Ruiz','1990-01-01','F',1), \
                    (20,'CC','2','Bea','Paz','1991-01-01','F',2)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn de(madre: i64) -> Nuevo {
        Nuevo {
            madre_paciente_id: madre,
            hospitalizacion_id: None,
            tipo_documento: Some("CN".into()),
            numero_documento: Some("9900123456".into()),
            fecha_nacimiento: "2026-08-16T03:20:00Z".into(),
            edad_gestacional: 39,
            num_consultas_prenatal: 7,
            sexo_biologico: "F".into(),
            peso_gramos: 3250,
            diagnostico_principal: "Z380".into(),
            condicion_destino: Some("01".into()),
            diagnostico_causa_muerte: None,
            fecha_egreso: Some("2026-08-18T10:00:00Z".into()),
            codigo_vida: Some("VIDA-2026-0001".into()),
        }
    }

    /// **El ingreso que se enlaza tiene que ser de la madre** — caso rojo de una
    /// de las cuatro claves foráneas que se ataban en crudo.
    ///
    /// Las encontró la revisión de seguridad de esta rama, y las dejó por debajo
    /// del listón porque hoy ninguna consulta de lectura une estas tablas: el
    /// daño es una fila de la institución A apuntando a una de la B, no una
    /// fuga. Se cierran igual, y en la misma pasada que la quinta —la de
    /// `defuncion`, donde el mismo descuido SÍ era un oráculo—, porque son **un
    /// defecto mirado por dos lados** y el lado callado no se queja nunca
    /// (directiva 23).
    #[tokio::test]
    async fn el_ingreso_que_no_es_de_la_madre_no_se_enlaza() {
        let pool = base().await;
        sqlx::query(
            "INSERT INTO cama (id,numero,piso,servicio,estado,institucion_id) \
             VALUES (1,'101','1','MEDICINA','OCUPADA',1),(2,'201','2','MEDICINA','OCUPADA',2)",
        )
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO hospitalizacion (id,paciente_id,cama_id,medico_tratante_id,fecha_ingreso,estado) \
             VALUES (77,10,1,5,'2026-08-15T10:00:00Z','ACTIVA'), \
                    (88,20,2,5,'2026-08-14T03:17:00Z','ACTIVA')",
        )
        .execute(&pool).await.unwrap();
        const ESPERADO: Result<i64, Falta> = Err(Falta::AtencionQueNoEsDeLaMadre);
        let mut n = de(10);
        n.hospitalizacion_id = Some(88);
        assert_eq!(crear(&pool, Ambito::Institucion(1), &n, Some(5)).await.unwrap(), ESPERADO);
        // Y un id que no existe responde IGUAL: no se puede enumerar.
        let mut x = de(10);
        x.hospitalizacion_id = Some(9999);
        assert_eq!(crear(&pool, Ambito::Institucion(1), &x, Some(5)).await.unwrap(), ESPERADO);
        // POSITIVO DE CONTROL: el ingreso de la propia madre sí entra.
        let mut ok = de(10);
        ok.hospitalizacion_id = Some(77);
        assert!(crear(&pool, Ambito::Institucion(1), &ok, Some(5)).await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn se_registra_y_se_lista() {
        let pool = base().await;
        let id = crear(&pool, Ambito::Institucion(1), &de(10), Some(5))
            .await
            .unwrap()
            .unwrap();
        let filas = listar(&pool, Ambito::Institucion(1), None, None).await.unwrap();
        assert_eq!(filas.len(), 1);
        assert_eq!(filas[0].id, id);
        assert_eq!(filas[0].madre_nombre, "Ana Ruiz");
        assert_eq!(filas[0].peso_gramos, 3250);
    }

    /// La pareja del de arriba, y la que de verdad importa: la institución 2 no
    /// ve el nacimiento de la 1. Sin este caso, `listar` podría estar
    /// devolviéndolo todo y el de arriba pasaría igual.
    #[tokio::test]
    async fn una_institucion_no_ve_los_nacimientos_de_la_otra() {
        let pool = base().await;
        crear(&pool, Ambito::Institucion(1), &de(10), Some(5))
            .await
            .unwrap()
            .unwrap();
        let ajenas = listar(&pool, Ambito::Institucion(2), None, None).await.unwrap();
        assert!(ajenas.is_empty(), "la institución 2 no atendió ese parto");
        let propias = listar(&pool, Ambito::Institucion(1), None, None).await.unwrap();
        assert_eq!(propias.len(), 1, "y la 1 sí lo ve: el filtro no lo esconde todo");
    }

    /// Colgarle un recién nacido a la paciente de OTRA institución se rechaza.
    /// Es la puerta por la que el arreglo saldría en el FEV de quien no la
    /// atendió, y se responde igual que si la madre no existiera.
    #[tokio::test]
    async fn no_se_le_cuelga_un_nacimiento_a_la_madre_de_otra_institucion() {
        let pool = base().await;
        let r = crear(&pool, Ambito::Institucion(1), &de(20), Some(1))
            .await
            .unwrap();
        assert_eq!(r, Err(Falta::MadreInexistente));
    }

    #[tokio::test]
    async fn el_ambito_sin_institucion_no_ve_ni_escribe() {
        let pool = base().await;
        assert_eq!(
            crear(&pool, Ambito::Ninguno, &de(10), None).await.unwrap(),
            Err(Falta::MadreInexistente)
        );
        assert!(listar(&pool, Ambito::Ninguno, None, None).await.unwrap().is_empty());
    }

    /// El filtro de fechas recorta de verdad. Con un solo caso «dentro» esto
    /// pasaría aunque el filtro no se aplicara nunca.
    #[tokio::test]
    async fn el_rango_de_fechas_deja_fuera_lo_que_no_cae() {
        let pool = base().await;
        let mut julio = de(10);
        julio.fecha_nacimiento = "2026-07-04T01:00:00Z".into();
        crear(&pool, Ambito::Institucion(1), &julio, None).await.unwrap().unwrap();
        crear(&pool, Ambito::Institucion(1), &de(10), None).await.unwrap().unwrap();

        let agosto = listar(&pool, Ambito::Institucion(1), Some("2026-08-01"), Some("2026-08-31"))
            .await
            .unwrap();
        assert_eq!(agosto.len(), 1, "solo el de agosto");
        let todo = listar(&pool, Ambito::Institucion(1), None, None).await.unwrap();
        assert_eq!(todo.len(), 2, "sin filtro salen los dos: el filtro es el que recorta");
    }

    /// `uno` respeta el mismo ámbito que `listar`. Se prueba con su pareja
    /// porque un `uno` que devolviera siempre `None` pasaría el caso ajeno.
    #[tokio::test]
    async fn uno_respeta_el_ambito() {
        let pool = base().await;
        let id = crear(&pool, Ambito::Institucion(1), &de(10), None)
            .await
            .unwrap()
            .unwrap();
        assert!(uno(&pool, Ambito::Institucion(1), id).await.unwrap().is_some());
        assert!(uno(&pool, Ambito::Institucion(2), id).await.unwrap().is_none());
    }

    /// El `CHECK` de la migración es la barandilla de la puerta de atrás: un
    /// importador que no pase por `revisar` tampoco puede meter un peso
    /// imposible. Se escribe SQL directo a propósito — pasar por `crear` mediría
    /// `revisar`, que ya está medido arriba.
    #[tokio::test]
    async fn el_check_de_la_migracion_rechaza_lo_imposible_por_la_puerta_de_atras() {
        let pool = base().await;
        let sql = "INSERT INTO recien_nacido (madre_paciente_id,fecha_nacimiento,\
                   edad_gestacional,num_consultas_prenatal,sexo_biologico,peso_gramos,\
                   diagnostico_principal,creado_en) VALUES (10,'2026-08-16T03:20:00Z',?,?,?,?,'Z380','x')";
        // El positivo de control: los MISMOS valores, buenos, sí entran.
        sqlx::query(sql).bind(39).bind(7).bind("F").bind(3250)
            .execute(&pool).await.expect("el caso bueno tiene que entrar");
        for (gest, cons, sexo, peso, que) in [
            (19, 7, "F", 3250, "edad gestacional imposible"),
            (39, -1, "F", 3250, "consultas negativas"),
            (39, 7, "X", 3250, "sexo fuera del vocabulario"),
            (39, 7, "F", 3, "peso en kilos"),
        ] {
            let r = sqlx::query(sql).bind(gest).bind(cons).bind(sexo).bind(peso)
                .execute(&pool).await;
            assert!(r.is_err(), "el CHECK tenía que rechazar: {que}");
        }
    }
}
