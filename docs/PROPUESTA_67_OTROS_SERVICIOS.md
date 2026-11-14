# #67 · `otrosServicios` (FEV-RIPS) — modelo de datos

**Estado: IMPLEMENTADO el 2026-09-08** (Opción 2, decidida por Juan; D2/D3/D4
con las recomendaciones). Este documento se conserva como el diseño aprobado;
lo construido lo resume la sección final «Lo hecho».

Decidido por Juan: **camino A (construirlo)**, alcance **solo insumos/
dispositivos, sin SISPRO**, **Opción 2** (la dispensación de insumo cuelga de la
atención).

Fecha: 2026-09-08. Medido contra `main 0c7be095`.

---

## 1. Qué falta y por qué (verificado, no supuesto)

`recienNacidos` está HECHO end-to-end (mig. 0062, `nacimientos.rs`, emisión
`anexar_recien_nacidos`). El único arreglo de la 948 que medico no emite es
**`otrosServicios` (AT)**, 18 campos.

El bloqueo **no** es el dominio `tipoOS` (resuelto: `01 = Dispositivos médicos e
insumos`; la transición admite el código propio del facturador). El bloqueo es
que **medico no captura la facturación de una dispensación**. Verificado:

- `struct Dispensacion` (inventario.rs) y tabla `dispensacion` (mig. 0025) NO
  tienen valor, ni autorización, ni MIPRES, ni modalidad de pago.
- `facturacion.rs` (contratos + tarifas ISS/SOAT/CUPS) **no liga** con
  `dispensacion` (`grep` = 0 referencias).
- `item_inventario.costo_unitario` es **costo, no tarifa**; tiene
  `registro_invima`, **no IUM**.

## 2. El patrón que ya existe y hay que espejar (D9)

medico **ya modela la valuación FEV**, un arreglo a la vez, sobre la tabla-fuente
de cada uno:

| Arreglo | Tabla-fuente | Migración que añadió los `valor_*` |
|---|---|---|
| consultas/procedimientos | `historia` | 0005 |
| medicamentos (AM) | `formula` | 0041 |
| órdenes | `orden` | 0041 |
| **otrosServicios (AT)** | **`dispensacion`** ← falta | **propuesta (0073)** |

Las columnas del quinteto de facturación son idénticas en las tres:
`valor_unitario/valor_servicio/concepto_recaudo/valor_pago_moderador/num_fev_pago_moderador`.
El `Recolector` ya anota «falta» (D20) cuando son `NULL`: no se rellena con
supuestos salvo en modo permisivo, que emite `0` **solo donde la norma lo manda**.

`otrosServicios` (AT) es estructuralmente casi gemelo de `medicamentos` (AM): el
emisor `crates/medico-dominio/src/rips/aos.rs` se modela sobre `am.rs`.

## 3. Los 18 campos, mapeados

| Campo 948 | Origen | ¿Nuevo? |
|---|---|---|
| `codPrestador` | institución (como AM) | reusa |
| `numAutorizacion` | **captura** | 🆕 |
| `idMIPRES` | **captura** (o `""`/null si no aplica) | 🆕 |
| `fechaSuministroTecnologia` | `dispensacion.fecha_dispensacion` | reusa |
| `tipoOS` | `"01"` (decidido) | constante |
| `codTecnologiaSalud` | código del insumo → **decisión D2** | 🆕 |
| `nomTecnologiaSalud` | `item_inventario.nombre` | reusa |
| `cantidadOS` | `dispensacion.cantidad_dispensada` | reusa |
| `tipoDocumentoIdentificacion` / `numDocumentoIdentificacion` | paciente (como AM) | reusa |
| `vrUnitOS` | **captura** `valor_unitario` | 🆕 |
| `vrDispensacion` | **captura** `valor_dispensacion` | 🆕 |
| `vrServicio` | **captura** `valor_servicio` | 🆕 |
| `conceptoRecaudo` | **captura** (catálogo; defecto `"05"` como AM) | 🆕 |
| `valorPagoModerador` | **captura** | 🆕 |
| `numFEVPagoModerador` | **captura** | 🆕 |
| `codigoVIDA` | fuera a propósito (amarre #603/IHCE) | — |
| `consecutivo` | lo pone el emisor | reusa |

**Regla dura (confirmar D3):** MEDICAMENTO ∉ `otrosServicios`. La fuente es
`dispensacion ⋈ item_inventario WHERE tipo IN ('INSUMO','DISPOSITIVO','REACTIVO')`,
excluyendo `MEDICAMENTO` (que va en el arreglo `medicamentos`). Un insumo contado
dos veces = doble cobro a la EPS.

## 4. LA decisión estructural (D1/D3) — dónde cuelga un insumo dispensado

Hoy `dispensacion.formula_id` es **NOT NULL**, y `formula` es una prescripción de
**medicamento** (`medicamento TEXT NOT NULL`, `codigo_cum`). Es decir: **toda
dispensación cuelga hoy de una fórmula de medicamento**. Un insumo/dispositivo
entregado en una atención (un material en un procedimiento, un dispositivo) no es
una fórmula de medicamento, así que **no hay hoy camino de captura para él**.

Dos opciones:

- **Opción 1 — mínima (espeja 0041, no toca la forma):** añadir el quinteto +
  `num_autorizacion` + `id_mipres` a `dispensacion`. Cubre **solo** insumos que ya
  se dispensen contra una fórmula. **Problema:** hoy no se prescriben insumos en
  `formula` (es medicamento-only), así que en la práctica **no capturaría nada**.
  No la recomiendo: dejaría el arreglo vacío por construcción (D30).

- **Opción 2 — RECOMENDADA:** el insumo se dispensa **contra la atención**, no
  contra una fórmula de medicamento. Concretamente:
  1. `dispensacion.formula_id` → **NULLABLE**.
  2. añadir `dispensacion.historia_id` (nullable, FK a `historia`).
  3. `CHECK`: exactamente uno de `{formula_id, historia_id}` no nulo — un
     medicamento cuelga de la fórmula (como hoy), un insumo/dispositivo de la
     atención.
  4. añadir el quinteto de facturación + `num_autorizacion` + `id_mipres` a
     `dispensacion`.
  5. una pantalla de captura: «suministrar insumo/dispositivo en esta atención»,
     con tarifa/autorización/MIPRES/moderador, que escribe la dispensación
     valorada. Nace con su E2E de navegador (D28).

  Es más trabajo, pero es la única que produce un `otrosServicios` real y no rompe
  el flujo de farmacia existente (medicamento sigue por `formula_id`).

## 5. Decisiones que necesito de Juan antes de escribir BD

- **D1 · estructura:** ¿Opción 2 (recomendada)? Si no, ¿cuál?
- **D2 · `codTecnologiaSalud`:** para `tipoOS=01`, la 948 pide IUM; la transición
  admite el código propio del facturador. medico tiene `registro_invima`, no IUM.
  ¿Uso `registro_invima` como código propio durante la transición, o añado una
  columna `ium` a `item_inventario` (nullable, se llena cuando se tenga)?
  Recomiendo: **`registro_invima` ahora + columna `ium` opcional para después**.
- **D3 · regla dura:** confirmar MEDICAMENTO ∉ otrosServicios (excluir por
  `item_inventario.tipo`).
- **D4 · tarifa:** ¿`valor_unitario` se captura a mano por dispensación (mínimo
  viable), o se resuelve desde un contrato/tarifario (`facturacion.rs`, más
  grande)? Recomiendo: **captura a mano ahora**; ligar a tarifario es una ficha
  aparte.

## 6. Plan de implementación (tras el OK), en orden

1. mig. **0073**: `dispensacion` nullable `formula_id` + `historia_id` + `CHECK` +
   quinteto + `num_autorizacion` + `id_mipres`; (opcional D2) `item_inventario.ium`.
2. dominio: `FuenteOtroServicio` + `crates/medico-dominio/src/rips/aos.rs`
   (`documento_aos`, modelado sobre `am.rs`), con pareja roja/verde por campo (D33).
3. lectura: `crates/medico-datos/src/rips.rs` — query de la fuente AT
   (`dispensacion ⋈ item_inventario` no-medicamento) y su `Fuente*`.
4. emisión: enchufar AT en el ensamblador FEV (hoy cableado a 0, como el Python).
5. captura: pantalla `medico-api/src/vista/` + ruta + E2E de navegador (D28).
6. cierre: `revisar-seguridad`/concilio (D25) — toca dinero facturado a EPS.

**Riesgo transversal:** un `otrosServicios` mal declarado es un cobro a la EPS sin
sustento. Por eso los `vr*` salen de captura explícita y el `Recolector` anota
falta en vez de suponer (D20); nada de códigos a mano (lección `tableta = 1`).

---

## Lo hecho (2026-09-08)

Cadena completa de `otrosServicios` (AT), Opción 2, con sus pruebas.

| Capa | Archivo | Qué |
|---|---|---|
| Migración **0073** | `medico-datos/migrations/0073_*.sql` | rebuild de `dispensacion`: `formula_id` nullable + `historia_id` + `CHECK` exactamente-uno + quinteto de facturación + `num_autorizacion` + `id_mipres`; `item_inventario.ium` |
| Dominio | `medico-dominio/src/rips/aos.rs` | `FuenteOtroServicio`, `OtroServicio` (18 campos 948), `documento_aos`; `tipoOS="01"`; `codTecnologiaSalud`=IUM o `registro_invima` |
| Emisor | `documentos.rs` (`Servicios.otros_servicios`), `fev.rs` (`consolidar`), `recolector.rs` (`Donde::aos`) | AT fundido por-usuario, renumerado, `num*`→null |
| Lectura | `medico-datos/src/rips.rs` (`otros_servicios_del_periodo`) | `dispensacion ⋈ item_inventario` NO-medicamento, colgado de historia, periodo por fecha de suministro |
| Ensamblador | `medico-api/src/rips.rs` (`Archivos.aos`, docs array, conteos AT) | AT en el FEV y en los conteos |
| Escritura | `medico-datos/src/inventario.rs` (`suministrar_insumo`, `SuministroNuevo`, `ResultadoSuministro`) | crea la dispensación colgada de la atención, baja stock, en transacción; rechaza medicamento y historia firmada |
| Pantalla | `medico-api/src/vista/farmacia.rs` (`suministros`/`suministrar`), ruta en `vista/mod.rs` | pestaña «Suministros (otros servicios)» con su formulario |

**Pruebas (D33/D8/D28):**
- dominio: `aos::pruebas` (verde completo; `sin_valor_unitario_es_falta_no_cero` = rojo discriminante D20; agrupa por paciente) + banco `campos_de_la_948.rs` (`el_at_emite_los_campos_de_la_948`, 8/8).
- lectura: `otros_servicios_lee_insumos_y_excluye_medicamentos` (discriminante), `el_ium_gana_al_registro_invima_en_el_codigo`.
- escritura: `el_insumo_se_suministra…`, `el_medicamento_no_se_suministra…` (discriminante por tipo), `no_se_suministra_a_una_historia_firmada`, `no_se_suministra_a_una_atencion_ajena`, `cien_suministros_concurrentes_no_dejan_stock_negativo` (D8).
- E2E HTTP (D28): `suministrar_un_insumo_cuelga_de_la_atencion_y_entra_en_otros_servicios`.

**Lo que queda como acción aparte (no bloquea la emisión):**
- **codigoVIDA** sigue fuera a propósito (amarre #603/IHCE), sale `null`.
- La captura se llega hoy desde farmacia con el `historia_id` a mano; integrarla desde la vista de la historia (id prefijado) es mejora de UX, no de datos.
- No liga `valor_unitario` a un tarifario (`facturacion.rs`): captura a mano (D4). Ligar a contrato es ficha aparte.
- **D25/concilio antes de mergear a `main` o desplegar** (repo sensible + toca dinero facturado a EPS).
