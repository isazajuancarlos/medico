// SPDX-License-Identifier: LicenseRef-Propietario
// Copyright (c) 2026 Juan Carlos Isaza
//! Los **recursos clínicos** que cuelgan del RDA de encuentro (#603 fase 4):
//! `Condition`, el pagador `Organization` (EAPB) y el `DocumentReference` de la
//! epicrisis. El `Encounter` se arma en [`super::clinico`], que es quien conoce
//! las diferencias entre los tres encuentros.
//!
//! # La disciplina, la misma de todo el módulo
//!
//! Cada recurso se contrasta contra SU `StructureDefinition` (no contra el
//! ejemplo) en `tests/conformidad_rda.rs`. Emitir la referencia a un recurso NO
//! conforme es un verde falso: la `Composition` pasa «tiene encounter», el recurso
//! lo rechaza MinSalud (directiva 28/33). Por eso los valores FIJOS del perfil van
//! literales aquí —son contrato, no dato—, y los `display` de los códigos VARIABLES
//! se derivan del CodeSystem (`super::terminologia`): un dato inventado en un
//! `display` sobrevive porque el servidor lo acepta como venga (lección `tableta=1`).
//!
//! # Los identificadores internos del Bundle
//!
//! Con `#<id>`, como el ejemplo oficial. Fijos y distintos para que ninguna
//! referencia cuelgue ni se duplique (lo vigilan `referencias_colgantes` e
//! `ids_duplicados`).

use serde_json::{json, Value};

use super::paciente::{Diagnostico, Eps};
use super::terminologia::{
    display, SISTEMA_CAUSA_EXTERNA, SISTEMA_DESTINO_EGRESO, SISTEMA_ENTORNO, SISTEMA_TIPO_DIAGNOSTICO,
    SISTEMA_VIA_INGRESO,
};
use super::{Falta, Faltas, Meta, Ref};

/// El `id` interno del `Encounter` en el Bundle.
pub(super) const ID_ENCOUNTER: &str = "Encounter-0";
/// El `Condition` del diagnóstico de ingreso (o el principal, en consulta externa).
pub(super) const ID_COND_INGRESO: &str = "Condition-0";
/// El `Condition` del diagnóstico de egreso (urgencias y hospitalización).
pub(super) const ID_COND_EGRESO: &str = "Condition-1";
/// El `DocumentReference` de la epicrisis.
pub(super) const ID_DOCREF: &str = "DocumentReference-0";

/// El `id` del pagador EAPB, derivado de su código para no colisionar con el
/// custodio (cuyo `id` es el código de habilitación).
pub(super) fn id_eapb(codigo_eps: &str) -> String {
    format!("EAPB-{}", codigo_eps.trim())
}

/// Un `Coding` `{system, code, display}` con el `display` DERIVADO del catálogo, o
/// una FALTA si el código no está: un `display` inventado es justo lo que el
/// servidor acepta sin protestar (directiva 20).
pub(super) fn coding(
    sistema: &str,
    codigo: &str,
    recurso: &'static str,
    campo: &'static str,
    porque: &'static str,
    f: &mut Faltas,
) -> Option<Value> {
    match display(sistema, codigo) {
        Some(d) => Some(json!({ "system": sistema, "code": codigo, "display": d })),
        None => {
            f.0.push(Falta { recurso, campo, porque });
            None
        }
    }
}

/// El `Coding` de la causa externa (`Encounter.reasonCode`).
pub(super) fn causa_externa(codigo: &str, f: &mut Faltas) -> Option<Value> {
    coding(
        SISTEMA_CAUSA_EXTERNA,
        codigo,
        "Encounter",
        "reasonCode",
        "causa externa fuera del catálogo RIPSCausaExternaVersion2: emitirla con un \
         display inventado la haría pasar por válida",
        f,
    )
}

/// El `Coding` del entorno de atención (`Encounter.type:encounterEnvironment`).
pub(super) fn entorno(codigo: &str, f: &mut Faltas) -> Option<Value> {
    coding(
        SISTEMA_ENTORNO,
        codigo,
        "Encounter",
        "type:encounterEnvironment",
        "entorno de atención fuera del catálogo EntornoAtencion",
        f,
    )
}

/// El `Coding` de la vía de ingreso (`hospitalization.admitSource`).
pub(super) fn via_ingreso(codigo: &str, f: &mut Faltas) -> Option<Value> {
    coding(
        SISTEMA_VIA_INGRESO,
        codigo,
        "Encounter",
        "hospitalization.admitSource",
        "vía de ingreso fuera del catálogo ViaIngreso",
        f,
    )
}

/// El `Coding` del destino al egreso (`hospitalization.dischargeDisposition`).
pub(super) fn destino_egreso(codigo: &str, f: &mut Faltas) -> Option<Value> {
    coding(
        SISTEMA_DESTINO_EGRESO,
        codigo,
        "Encounter",
        "hospitalization.dischargeDisposition",
        "condición/destino de egreso fuera del catálogo CondicionyDestinoUsuarioEgreso",
        f,
    )
}

/// El `Coding` del tipo de diagnóstico, para `ExtensionDiagnosisType`.
///
/// **`None` en el tipo es una FALTA**, no un valor por defecto: afirmar «impresión
/// diagnóstica» o «confirmado» sin que nadie lo eligiera sería inventar un dato
/// clínico (directiva 20). Los TRES encuentros capturan el tipo (consulta en
/// `historia.tipo_diagnostico`; hospitalización y urgencias desde la migración 0075);
/// aunque la Resolución 948 no lo pida para AU/AH, el PERFIL RDA lo exige
/// (`ExtensionDiagnosisType`, min=1). `None` aquí = una fila sin capturarlo aún.
pub(super) fn tipo_diagnostico(tipo: Option<&str>, f: &mut Faltas) -> Option<Value> {
    let Some(t) = tipo.map(str::trim).filter(|s| !s.is_empty()) else {
        f.0.push(Falta {
            recurso: "Encounter",
            campo: "diagnosis.ExtensionDiagnosisType",
            porque: "el tipo de diagnóstico (impresión/confirmado) no está capturado y no se \
                     deduce del rol: afirmarlo sería inventar un dato clínico",
        });
        return None;
    };
    coding(
        SISTEMA_TIPO_DIAGNOSTICO,
        t,
        "Encounter",
        "diagnosis.ExtensionDiagnosisType",
        "tipo de diagnóstico fuera del catálogo RIPSTipoDiagnosticoPrincipalVersion2",
        f,
    )
}

/// Un `Condition` (`ConditionRDA`) desde un diagnóstico de medico.
///
/// El **CIE-10 es obligatorio** (`code.coding:ICD10`, con `code` y `display`); el
/// CIE-11 se DERIVA con [`crate::cie11::par`] y solo viaja si la tabla de la OMS lo
/// resuelve (es opcional en el perfil). Los estados clínicos van FIJOS por el
/// perfil (`active`/`confirmed`): el RDA solo transmite diagnósticos activos y
/// confirmados de un encuentro.
pub(super) fn condicion(id: &str, diag: &Diagnostico, paciente_id: &str, f: &mut Faltas) -> Value {
    if diag.cie10.trim().is_empty() {
        f.0.push(Falta {
            recurso: "Condition",
            campo: "code.coding:ICD10.code",
            porque: "sin código CIE-10 no hay diagnóstico que transmitir",
        });
    }
    if diag.display.trim().is_empty() {
        f.0.push(Falta {
            recurso: "Condition",
            campo: "code.coding:ICD10.display",
            porque: "el perfil exige el nombre del diagnóstico CIE-10, que el RIPS no emite \
                     (sale de la tabla cie10 de medico-datos)",
        });
    }
    let mut coding = vec![json!({
        "system": "http://hl7.org/fhir/sid/icd-10",
        "code": diag.cie10,
        "display": diag.display,
    })];
    // El CIE-11, DERIVADO del CIE-10 con la misma regla y tabla que el RIPS. Si no
    // deriva, no se emite —es opcional en el perfil— en vez de inventarlo.
    let (c11, n11) = crate::cie11::par(Some(diag.cie10));
    if !c11.is_empty() {
        coding.push(json!({
            "system": "http://hl7.org/fhir/sid/icd-11",
            "code": c11,
            "display": n11,
        }));
    }
    json!({
        "resourceType": "Condition",
        "id": id,
        "meta": Meta::perfil("ConditionRDA"),
        "clinicalStatus": { "coding": [{
            "system": "http://terminology.hl7.org/CodeSystem/condition-clinical",
            "code": "active", "display": "Active" }] },
        "verificationStatus": { "coding": [{
            "system": "http://terminology.hl7.org/CodeSystem/condition-ver-status",
            "code": "confirmed", "display": "Confirmed" }] },
        "category": [{ "coding": [{
            "system": "http://terminology.hl7.org/CodeSystem/condition-category",
            "code": "encounter-diagnosis", "display": "Encounter Diagnosis" }] }],
        "code": { "coding": coding },
        "subject": Ref::a(paciente_id),
    })
}

/// El pagador `Organization` (`HealthBenefitPlanAdminOrganizationRDA`): la EAPB.
///
/// El `identifier` lleva DOS `type.coding` que el perfil fija (el internacional
/// `NIIP` y el colombiano `EAPB`), el `system` de EAPB fijo, y como `value` el
/// `codigo_eps`. Todo lo fijo es contrato; lo variable es el código y el nombre.
pub(super) fn organizacion_eapb(eps: &Eps, f: &mut Faltas) -> Value {
    if eps.codigo.trim().is_empty() {
        f.0.push(Falta {
            recurso: "Organization",
            campo: "identifier:EAPBIdentifier.value",
            porque: "sin código de EAPB no hay pagador que declarar",
        });
    }
    if eps.nombre.trim().is_empty() {
        f.0.push(Falta {
            recurso: "Organization",
            campo: "name",
            porque: "el perfil de la EAPB exige el nombre de la entidad",
        });
    }
    json!({
        "resourceType": "Organization",
        "id": id_eapb(eps.codigo),
        "meta": Meta::perfil("HealthBenefitPlanAdminOrganizationRDA"),
        "identifier": [{
            "id": "EAPBIdentifier",
            "use": "official",
            "type": { "coding": [
                { "system": "http://terminology.hl7.org/CodeSystem/v2-0203",
                  "code": "NIIP", "display": "National Insurance Payor Identifier" },
                { "system": "https://fhir.minsalud.gov.co/rda/CodeSystem/ColombianOrganizationIdentifiers",
                  "code": "EAPB", "display": "Entidad Administradora de Planes de Beneficios" }
            ] },
            "system": "https://fhir.minsalud.gov.co/rda/NamingSystem/EAPB",
            "value": eps.codigo,
        }],
        "active": true,
        "name": eps.nombre,
    })
}

/// El `DocumentReference` de la epicrisis (`DocumentReferenceEPIRDA`).
///
/// Casi todo es FIJO por el perfil —es SIEMPRE la epicrisis del encuentro—: el
/// tipo (LOINC + `EPI`), la categoría, la descripción, el `custodian`
/// (`Organization/MinSalud`), la etiqueta de seguridad (`restricted`) y el formato
/// (`application/pdf`). Lo variable son el `masterIdentifier`, el paciente, la
/// fecha, el autor (la institución) y el encuentro al que pertenece.
///
/// **El PDF no se embebe** (`content.attachment.data`): el perfil no lo exige y
/// medico no materializa el binario aquí. Se declara el `contentType` —es un
/// PDF— sin inventar su contenido.
pub(super) fn documento_epi(
    epicrisis_id: &str,
    paciente_id: &str,
    institucion_id: &str,
    fecha: &str,
    f: &mut Faltas,
) -> Value {
    if epicrisis_id.trim().is_empty() {
        f.0.push(Falta {
            recurso: "DocumentReference",
            campo: "masterIdentifier.value",
            porque: "sin identificador de la epicrisis no hay documento de soporte que referenciar",
        });
    }
    json!({
        "resourceType": "DocumentReference",
        "id": ID_DOCREF,
        "meta": Meta::perfil("DocumentReferenceEPIRDA"),
        "masterIdentifier": {
            "system": "https://fhir.minsalud.gov.co/rda/NamingSystem/EPI",
            "value": epicrisis_id,
        },
        "status": "current",
        "type": { "coding": [
            { "system": "http://loinc.org", "code": "18842-5", "display": "Discharge summary" },
            { "system": "https://fhir.minsalud.gov.co/rda/CodeSystem/ColombianDocumentTypes",
              "code": "EPI", "display": "Epicrisis" }
        ] },
        "category": [{ "coding": [{
            "system": "http://loinc.org", "code": "55108-5",
            "display": "Clinical presentation Document" }] }],
        "subject": Ref::a(paciente_id),
        "date": fecha,
        "author": [Ref::a(institucion_id)],
        "custodian": { "reference": "Organization/MinSalud" },
        "description": "Epicrisis del encuentro de atención en salud - RDA",
        "securityLabel": [{ "coding": [{
            "system": "http://terminology.hl7.org/CodeSystem/v3-Confidentiality",
            "code": "R", "display": "restricted" }] }],
        "content": [{
            "attachment": { "contentType": "application/pdf" },
            "format": {
                "system": "urn:ietf:bcp:13", "code": "application/pdf", "display": "PDF" },
        }],
        "context": { "encounter": [Ref::a(ID_ENCOUNTER)] },
    })
}
