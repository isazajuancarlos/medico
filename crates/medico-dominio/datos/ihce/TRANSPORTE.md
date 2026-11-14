# IHCE / RDA — capa de transporte y autenticación (#603)

**Fuente**: *Manual de Operaciones de interoperabilidad IHCE*, MinSalud (OTIC),
**versión 01, octubre 2025** — `minsalud.gov.co/ihce/Manuales/`. Transcrito el
2026-08-09. Es material de referencia oficial; **no** son los perfiles FHIR (esos
viven en el paquete `minsalud.fhir.co.rda`, aún no traído — ver `[[ihce-rda-603-estado]]`).

> **Qué de esto es IG-INDEPENDIENTE (construíble YA, sin el paquete)** — todo lo de
> este archivo: el flujo OAuth2, los encabezados, las URLs de operación, el manejo
> de errores y las reglas de validación estructural. Lo único que necesita el IG es
> el **contenido** del `Bundle` (qué recursos y con qué perfiles). O sea: la mitad
> de transporte se puede escribir y probar contra un *mock* antes de tener el IG; el
> ensamblado del cuerpo, no.

> **Estado del binario Rust hoy (medido 2026-08-09)**: medico **no tiene cliente
> HTTP saliente** — cero `reqwest`/`hyper`/`ureq`/`oauth2` en los `Cargo.toml`. Los
> «POST a MinSalud» que hubo eran del backend Python, retirado en #598. El
> transporte IHCE es **greenfield** y añade la primera dependencia de HTTP saliente
> (directiva 35: reqwest+rustls es el estándar del ecosistema axum; el token OAuth2
> es un solo POST, no justifica el crate `oauth2` entero).

---

## 1. Autenticación — OAuth2 *client credentials* (Azure AD)

- **Token endpoint**: `POST https://login.microsoftonline.com/<tenantid>/oauth2/v2.0/token`
- **Content-Type**: `application/x-www-form-urlencoded`
- **Body**: `grant_type=client_credentials`, `client_id=<clientid>`,
  `client_secret=<clientsecret>`, `scope=<scope>`
- **Response 200**: `{ "access_token": "...", "token_type": "Bearer", "expires_in": 3599 }`
- **TLS 1.2+ obligatorio**.
- Cada prestador recibe **client_id + client_secret + subscription-key** por el
  portal, procedimiento del *Manual de Gestión de Credenciales* (documento aparte,
  `Manual_de_gestion_de_credenciales_Historia_Clinica_Interoperable_V1.pdf`).
- **Rotación de credenciales**: periódica, definida por MinSalud.

**Almacenamiento de credenciales (art. 6.2, Res. 1888/2025 — es requisito, no
consejo)**: Client ID/Secret en bóveda segura (Azure Key Vault, AWS Secrets Manager
o equivalente). **Prohibido** texto plano en código fuente, configuración sin
cifrar o repos públicos. → En medico esto cae en el `.env` del VPS (que no se
sobrescribe nunca desde local) y/o `guaca::reposo`/`guaca::custodia`; nunca en el
repo. Es la regla de secretos del CLAUDE.md general aplicada a esta integración.

## 2. Encabezados comunes de la API IHCE

Toda llamada a la API (no al token endpoint) lleva:

| Encabezado | Valor |
|---|---|
| `Authorization` | `Bearer <access_token>` |
| `Ocp-Apim-Subscription-Key` | `<subscription-key>` (la API está tras Azure API Management) |
| `Content-Type` | `application/fhir+json` |

- **URL base**: se publica **al asignar credenciales** (Manual de Gestión de
  Credenciales). No está en el manual de operaciones; es por-prestador/ambiente.
  → Va como configuración (columna de `configuracion_institucion` o `.env`), nunca
  fija en código.

## 3. Operaciones (5.5)

Los cuatro RDA se envían a operaciones `$enviar-rda-*` distintas; el resto son
consultas. Todas con los tres encabezados de §2.

| # | Método y URL | Uso | Cuerpo | Respuesta |
|---|---|---|---|---|
| 1 | `POST /Composition/$enviar-rda-paciente` | Enviar RDA Paciente | Bundle `document`, entry[0]=Composition, entry[n]=Patient/Encounter/Condition… | 200: Bundle con id del RDA · 400: OperationOutcome (estructura) · 409: OperationOutcome (Encounter repetido) |
| 2 | `POST /Composition/$enviar-rda-hospitalizacion` | RDA egreso | Bundle `document`; Encounter con hospitalization, diagnósticos, procedimientos | 200: OperationOutcome con id |
| — | `POST /Composition/$enviar-rda-consulta` · `…-urgencias` | Los otros dos RDA | (mismo patrón; el manual detalla paciente y hospitalización como ejemplares) | ídem |

> **CORREGIDO EL 2026-08-14, y el manual DICE OTRA COSA.** Esta fila decía
> `$enviar-rda-consulta-externa`, transcrito del Manual de Operaciones IHCE v01
> §5.5. Es lo que dice el manual y **no es lo que responde el servidor**: en la
> colección Postman del sandbox, la petición se LLAMA
> `enviar-rda-consulta-externa` y su URL es `$enviar-rda-consulta`. El nombre y
> la ruta no coinciden, y ahí estaba la trampa.
>
> Manda la colección, porque es el artefacto que habla con el servidor real. El
> extracto con las cinco operaciones está vendorizado al lado, en
> `OPERACIONES.txt`, para que esta afirmación no dependa de un archivo que vive
> fuera del repositorio (directiva 31). Las otras tres coinciden nombre y ruta.
>
> Escrito de memoria, esto habría dado 404 en uno de los cuatro envíos.
| 3 | `POST /Patient/$consultar-paciente-similar` | Búsqueda por similaridad (antes de crear) | Parameters: identifier `{type:CC, value}`, name `{family, given}`, gender | 200: lista con score |
| 4 | `POST /Composition/$consultar-rda-paciente` | Listar RDAs de un paciente | Parameters: identifier | 200: Bundle de Compositions, paginado (`Link.relation=next`, `Link.url`) |
| 5 | `GET /siguiente-pagina/{id}` | Página siguiente | — | 200: Bundle. **El `link.url` caduca**: vigencia limitada |
| 6 | `GET /Composition/{id}/$document` | Recuperar un RDA completo | — | 200: Bundle document con Composition + recursos de primer nivel |
| 7 | `POST /Practitioner/$consultar-profesional-salud` | Validar profesional en RETHUS | Parameters: identifier | 200: datos del profesional u OperationOutcome |
| 8 | `GET /Patient/{id-patient}/$consultar-resumen-longitudinal` | Resumen longitudinal del paciente | — | 200: Bundle consolidado |

## 4. Convenciones de referencia (5.3) — para el ensamblado del Bundle

- `Bundle`: `resourceType: Bundle`, `type: "document"`. Composition es la primera
  entrada, **solo una** por Bundle.
- **IDs locales de recursos a crear**: patrón `#TipoRecurso-NumRecurso`
  (`#ServiceRequest-0`, `#Encounter-0`, `#Condition-0`, `#Condition-1`). El `id` del
  recurso es el mismo patrón sin `#`.
- **Referencias externas persistidas** (recurso ya existente en IHCE):
  - `Patient` → `#TipoIdentificacion-NumIdentificacion` (ej. `#CC-1234567890`)
  - `Practitioner` → `#TipoIdentificacion-NumIdentificacion`
  - `Organization` (Prestador) → `#NumeroDeHabilitacion` (ej. `#0987654321`)
  - `Location` (Sede) → `#CodigoSedePrestador-NumeroSede` (ej. `#0987654321-00`)
  - `Organization` (EAPBS) → `#CodigoEAPBS` (ej. `#EPS000`)
- Toda referencia `#Tipo-Num` debe tener su entrada correspondiente **dentro del
  mismo Bundle**, o el servidor rechaza por inconsistencia de referencias.

## 5. Reglas de validación obligatorias (5.4) — validar EN ORIGEN antes de enviar

1. `Bundle.type == document`; `Bundle.entry[0]` es `Composition`; una sola Composition.
2. **Todas** las secciones del Composition se envían. Sección vacía → `emptyReason`
   (coding `http://terminology.hl7.org/CodeSystem/list-empty-reason`, code `nilknown`,
   display "Nil Known") + `text.div` narrativo. Ejemplo del manual: alergias con LOINC
   `48765-2`. **→ Confirma la brecha SNOMED/LOINC**: el Composition usa LOINC.
3. **Paciente**: el `Patient` referenciado debe existir en el **Registro Nacional
   EVOL** y coincidir: tipo+número de identificación (exacto), primer apellido y
   primer nombre (case-insensitive, con `trim`), sexo biológico (exacto). Paciente
   extranjero o sin identificación: no se valida contra EVOL; se crea si no existe.
4. **RDA duplicado**: si `Encounter.subject` + `period` + `serviceProvider` +
   `participant` coinciden exactamente con un registro existente → **409 Conflict**.
   Si alguno difiere, se acepta.
5. **Attachments** (epicrisis PDF vía DocumentReference): `content.contentType`
   `application/pdf`, contenido Base64, límite de tamaño configurable (~5 MB). Formato
   o tamaño excedido → rechazo.
6. `Practitioner`, `Organization`, `Location` referenciados deben estar habilitados
   y activos en RETHUS y REPS.

## 6. Manejo de errores (5.6)

- Errores estructurados con `OperationOutcome` (HL7 FHIR): `issue[]` con `severity`
  (error/warning/information), `code` (structure/value/processing/invalid…),
  `diagnostics`, `location`.
- **Mapeo HTTP**: `400` → error de formato/estructura · `401/403` → auth/permiso ·
  `409` → duplicado (RDA existente).

## 7. Buenas prácticas orientadas al prestador (5.7)

- **Envío ASÍNCRONO** (encolamiento) recomendado: que el envío no bloquee la
  operación clínica. → En medico: una cola en SQLite + tarea de fondo, no un POST
  síncrono dentro del guardado de la historia.
- **Reintentos con backoff exponencial**, registrando cada intento y su respuesta.
- **Trazabilidad completa** de envíos, respuestas y errores del mecanismo.
- Visor: historial mínimo de **12 meses**, filtrable por fecha/tipo de RDA; la
  consulta solo en el momento de la atención (no dejar abierta por identificación).
- Paginación `link.url` caduca → preferir scroll infinito.

---

## Determinación para #603 (fases)

- **Fase 4 (transporte/auth)**: **especificada por completo y CONSTRUÍBLE YA** sin el
  IG — cliente OAuth2 client-credentials + cliente HTTP con los tres encabezados +
  parseo de `OperationOutcome` + cola asíncrona con backoff. Lo único que falta para
  *probarla contra el mecanismo real* es la **URL base + credenciales de sandbox**,
  que también las entrega MinSalud al onboarding (otro `[J]`). Contra *mock* se puede
  escribir y probar hoy.
- **Fases 0–3 (perfiles, ensamblado de los 4 Bundles, mapeo del modelo→FHIR)**:
  siguen **bloqueadas en el paquete `minsalud.fhir.co.rda`** (no traíble desde aquí).
- **Brecha de terminología confirmada**: el Composition exige LOINC (y el IG
  probablemente SNOMED CT); medico hoy tiene CIE-10/11, CUPS, CUM — **cero LOINC/SNOMED**.
