# La Guía de Implementación del RDA, vendorizada

**Qué hay aquí**: 206 de los 207 artefactos FHIR de la *Guía de Implementación —
Resumen Digital de Atención en Salud (RDA) v1.0.0* del MinSalud, tal como los
publica el sitio, **verbatim**. 75 `StructureDefinition` (los perfiles), 65
`ValueSet`, los `CodeSystem`, `CapabilityStatement`, `NamingSystem` y los
ejemplos.

## Por qué vendorizados y no consultados por red

Por la misma razón que `backend/datos/`: un consultorio en zona rural no puede
depender de que `vulcano.ihcecol.gov.co` esté arriba el día que emite un RDA. Y
por una segunda que aquí pesa más: **estos archivos son el contrato de
validación**. Si el banco de pruebas los bajara al correr, un cambio en el
servidor cambiaría el veredicto de las pruebas sin que nadie tocara el código —
y un verde que depende de una red ajena no es un verde.

## Por qué archivo a archivo y no `package.tgz`

**El paquete npm del IG no está publicado.** Medido el 2026-08-16: las rutas
`package.tgz`, `definitions.json.zip` y `full-ig.zip` devuelven **HTTP 200 con
el `index.html` byte a byte** — un 404 blando, que es la peor forma de fallar
porque un `%{http_code}` lo lee como éxito. Se detectó con `file`, y el control
positivo es que `artifacts.html` sí devuelve 183 249 B distintos.

La otra ruta canónica, `https://fhir.minsalud.gov.co/rda/`, **no resuelve por
DNS**: es la URL canónica que declara la guía, no una dirección publicada.

Así que la única fuente hoy son los artefactos sueltos. Los 207 se bajaron uno a
uno y los 207 abren como JSON con `resourceType` — cero fallos.

## Cómo se comprueba que no han cambiado

`MANIFIESTO.txt` lleva el `sha256` de cada archivo con su nombre. La fuente de
cada uno es `https://vulcano.ihcecol.gov.co/<nombre>`.

**Actualizarlos es un paso deliberado**, como regenerar los maestros del MUV:
**bajar, comparar contra el manifiesto, y leer el diff** — nunca *bajar y
sobrescribir*. Un perfil que gana un `min=1` es trabajo nuevo, no una
actualización de rutina.

Y hay que verlo de frente: este directorio abre un camino de una fuente EXTERNA
al árbol del producto. El contenido de hoy no tiene un solo elemento activo
—medido: cero `<script>`, cero `javascript:`, cero manejadores `on*`, cero
iframes, cero caracteres invisibles o de anulación bidi, y los 211 `text.div`
parsean como XML bien formado—. El riesgo no es el contenido: es el camino. **El
manifiesto es justo la mitigación**, y solo funciona si la actualización compara
en vez de sobrescribir.

Una cosa que el manifiesto NO prueba, y conviene tenerla escrita: atestigua que
los bytes no han cambiado **desde la descarga**, no que sean los que MinSalud
publicó. No hay firma del emisor, y el ancla de confianza es el TLS de una
descarga hecha una vez. Es lo mejor disponible, y es lo que es.

## El peso, medido

28 MB en el árbol de trabajo, **1,2 MB empaquetados en git** — los artefactos son
casi el mismo documento repetido y el delta los aplasta. Es un 2 % del `.git`,
no la mitad. Del volumen, el `snapshot` es el 52 %, la narrativa HTML el 39 %, y
lo verdaderamente normativo y propio —el `differential`— el **3,4 %**.

Podar el 91 % ahorraría en el repositorio del orden de 1 MB **y destruiría la
única propiedad que justifica todo esto**: la suma verificable contra la fuente.
No se poda. Los números están aquí para que dentro de un año nadie reabra la
discusión de memoria.

## Para qué se usan

Los lee el banco de conformidad de `medico-dominio`, que contrasta los `Bundle`
que ensambla `ihce/` contra lo que los perfiles EXIGEN: elementos con `min>=1`,
valores fijos (`fixedCode`, `fixedUri`, `fixedString`, `fixedCodeableConcept`) y
enlaces de terminología obligatorios. Son 2 058 reglas repartidas en 75 perfiles.

Es la clase de defecto que ninguna prueba propia encuentra: el `Bundle` sale bien
formado, el JSON es válido, y el servidor nacional lo rechaza — o, peor, lo
acepta y lo guarda en el sitio que no es.
