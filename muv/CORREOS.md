# Cómo conseguir la validación, y a quién escribirle

El prestador **valida a mano**, con la aplicación cliente-servidor. Nunca recibió
el paquete de la API-Docker, así que pedírselo no lleva a ninguna parte.

Quedan dos caminos, y no compiten: uno responde hoy, el otro sirve al producto.

| | Qué responde | Qué cuesta |
|---|---|---|
| **A. Que él valide un archivo nuestro** | ¿El JSON que generamos pasa? | Un correo. Nada de infraestructura. |
| **B. Pedir el paquete al Ministerio** | Validación automatizable, en nuestra máquina | Un trámite, con su espera |

**Ninguno de estos correos se envía sin que Juan lo revise.** En ninguno van
credenciales: se piden archivos y resultados, nunca accesos.

---

## A. Al prestador — que pase nuestro archivo por su validador

**Asunto:** ¿Me validas un archivo de prueba en el FEV-RIPS?

> Hola [NOMBRE]:
>
> Necesito comprobar si los archivos RIPS que genera el sistema que estoy
> desarrollando pasan las validaciones del Ministerio, y no tengo forma de
> probarlo sin el validador.
>
> Te adjunto un archivo JSON. **Son datos inventados**: no hay ningún paciente
> real, ninguna factura real y ningún dato tuyo dentro. Lo generó el propio
> sistema para esto.
>
> Lo que te pido es que lo cargues en el validador FEV-RIPS como haces con los
> tuyos, **en el ambiente de PRUEBAS, nunca en el de producción**, y me cuentes
> qué respondió:
>
> - Si lo aceptó, el CUV que devolvió.
> - Si lo rechazó, la lista de hallazgos tal cual aparece — códigos y
>   descripciones. Con una captura de pantalla me basta.
>
> Es un envío de prueba con `numNota` propio, así que no interfiere con tu
> numeración ni con nada tuyo.
>
> Si prefieres no hacerlo, dímelo sin problema y busco otra vía.
>
> Gracias,
> Juan Carlos

**Antes de mandarlo:** el archivo adjunto tiene que ejercitar los cinco tipos de
atención, no solo consultas (tarea #25). Enviar uno solo de consultas y concluir
«pasamos» sería medir un quinto y afirmar el todo.

---

## B. A la mesa de ayuda del Ministerio

`soporte-fev-rips@minsalud.gov.co` · Bogotá (601) 330 5043 · resto del país
018000960020 · L-V 7:00–18:00, sábados 8:00–13:00.

Lo ideal es que lo envíe **el prestador desde su correo**, porque el trámite va
atado a la entidad. Si lo manda Juan, conviene decir desde el principio a nombre
de quién.

**Asunto:** Solicitud de archivos de despliegue API-Docker FEV-RIPS (facturador
con desarrollo propio)

> Buen día:
>
> Escribo en calidad de [prestador / persona que apoya técnicamente al prestador
> **[RAZÓN SOCIAL]**, NIT **[NIT]**], que factura electrónicamente y reporta
> RIPS conforme a la Resolución 948 de 2026.
>
> Actualmente la validación se hace con la aplicación cliente-servidor.
> Queremos migrar a la modalidad de **facturador con desarrollo propio**,
> consumiendo la API en contenedor Docker. Ya tenemos la *Guía de Instalación
> API Docker FEV-RIPS v1.0* y el *Manual de Consumo API-Docker FEV-RIPS v4.3
> (v2.0)*, descargados del micrositio de Factura Electrónica.
>
> La guía describe cómo **construir** las dos imágenes, pero los artefactos que
> se copian dentro no vienen con ella. Solicitamos, por favor, el paquete de
> despliegue con:
>
> 1. `FevsRipsLocal.bak` y/o el script `FEVRIPS-script.sql` de creación de la
>    base `FevsRipsApix` (sección 11.3 de la guía).
> 2. La carpeta de publicación `FVE1/`, con `FVE.WebAPI.dll` y sus dependencias
>    (sección 11.4).
>
> Si el paquete se descarga de alguna sección del micrositio que no hemos
> ubicado, agradecemos la ruta exacta; y si requiere una solicitud o un permiso
> previo asociado a la entidad, agradecemos indicarnos el procedimiento.
>
> Aprovecho para reportar una posible errata de la guía, por si les es útil: la
> sección 11.3.5 usa la ruta `/opt/mssql-tools/bin/sqlcmd`, que no existe en la
> imagen `mcr.microsoft.com/mssql/server:2022-latest` que la propia guía indica
> como imagen base. Allí la herramienta está en `/opt/mssql-tools18/bin/sqlcmd`
> y requiere el parámetro `-C`. Siguiendo la guía literalmente, el contenedor
> arranca pero falla en cada consulta.
>
> Quedo atento y agradezco su gestión.
>
> Cordialmente,
> [NOMBRE] — [CARGO / en apoyo a [RAZÓN SOCIAL], NIT [NIT]]
> [TELÉFONO] · [CORREO]

---

## Por qué está redactado así

- **Se piden archivos y resultados, nunca accesos.** Ya se sabe que no existe
  credencial de integrador (caso 2026403888): el acceso está atado a una persona
  vinculada a la entidad. Volver a pedirlo gasta el turno.
- **Se nombra el artefacto como lo nombra la guía** (`FVE1`, `FevsRipsLocal.bak`,
  secciones 11.3 y 11.4). Pedir «la imagen Docker del MUV» invita a que
  respondan con el enlace al manual que ya tenemos.
- **Se dice «ambiente de PRUEBAS, nunca producción» en negrita.** El validador
  de producción tiene efectos jurídicos y financieros reales; meter ahí una
  factura inventada no es una prueba, es un hecho.
- **Se le da salida al prestador.** Está haciendo un favor con una herramienta
  atada a su identidad; que pueda decir que no sin incomodidad es lo mínimo.
- **La errata se reporta.** Cuesta dos líneas, es verificable, y hace la
  solicitud de alguien que ya leyó el documento.
