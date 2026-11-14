# Cómo se migra un formulario del sistema original

> **HISTORIA DEL REPOSITORIO, NO INSTRUCCIONES VIGENTES.** Angular salió el
> 2026-08-08: las 29 pantallas se tejen en el servidor con `maud` + htmx dentro
> del binario (directiva 26), y `frontend/` ya no existe. Este documento se
> conserva porque describe decisiones que se tomaron y se pagaron, y porque su
> MÉTODO —extraer el comportamiento del original antes de suponerlo— sigue
> valiendo. Lo que no sirve es seguir sus pasos: no hay frontal donde aplicarlos.

Guía de trabajo para las 37 pantallas del sistema en Visual FoxPro. Escrita
después de extraer el comportamiento real del original, no de suponerlo.

> **El orden es: migración completa primero, mejoras después.** Una pantalla se
> da por migrada cuando hace lo mismo que la vieja. Lo que se quiera mejorar se
> anota y se hace en una segunda pasada, para no mezclar "no funciona" con "no
> me gusta cómo funcionaba".

---

## El hallazgo que cambia el trabajo

Los 37 formularios **no son 37 diseños distintos**. Todos instancian el mismo
marco. Se descubrió con `herramientas/vfp_lector.py --interacciones`:

| Método compartido (`medico1`) | Usos | Qué hacía |
|---|---|---|
| `zoomx` | 522 | amplía el contenido con clic derecho |
| `mousemovex` | 108 | realce visual al pasar el ratón |
| `activar_objetos` | 72 | conmuta qué campos aceptan escritura |
| `botones` | 72 | conmuta la barra de botones |
| `primero/anterior/siguiente/ultimo` | 72 | navegación entre registros |
| `teclas_calientes` | 16 | atajos de teclado |

Y 18 formularios comparten la misma barra CRUD: `wznew`, `wzedit`, `wzdelete`,
`wzsave`, `wzundo`.

**Por eso el marco se construye una vez** y cada formulario es configuración.
Migrar pantalla por pantalla sería repetir 37 veces el mismo trabajo, y el
resultado se sentiría como 37 aplicaciones distintas en vez de una.

---

## Lo que ya está construido

`src/app/shared/formulario/`

| Pieza | Reemplaza a | Qué resuelve |
|---|---|---|
| `estado-formulario.ts` | `activar_objetos` + `botones` | la máquina de estados: consulta / nuevo / edición |
| `barra-crud.component.ts` | `wznew…wzundo` + navegación | la barra, idéntica en todas las pantallas |
| `zoom.directive.ts` | `zoomx` | clic derecho amplía; además F2, que el original no tenía |
| `teclas.directive.ts` | los `KeyPress` | Esc cierra, Ctrl+Enter confirma, Enter avanza de campo |

**Una decisión que se apartó del original a propósito:** `activar_objetos` y
`botones` eran dos métodos que había que acordarse de llamar juntos. Se unieron
en un solo estado, porque su modo de fallo era dejar los campos abiertos con el
botón de guardar deshabilitado. Ahora es imposible por construcción, y hay una
comprobación que lo fija: los campos son editables **exactamente cuando** se
puede guardar.

---

## Receta para migrar una pantalla

### 1. Leer qué hace la original, no adivinarlo

```bash
python herramientas/vfp_lector.py --salida ./extraido /mnt/data/aprograms/medico/forms
less extraido/<formulario>.prg.txt
```

Buscar en ese archivo, en este orden:

- **`PROCEDURE Init`** — qué tablas abre. Ahí están sus dependencias reales.
- **`PROCEDURE Valid`** — las reglas de negocio. Son las más fáciles de perder
  porque están repartidas campo por campo: hay **353 en 31 formularios**.
- **`PROCEDURE When`** — cuándo un campo acepta el foco. Suele codificar
  dependencias entre campos ("no puede escribir el diagnóstico sin paciente").
- **`PROCEDURE Click`** de los botones que no son de la barra CRUD — lo propio
  de esa pantalla.
- **`MESSAGEBOX`** — los mensajes al usuario, con su texto. Copiarlos: son el
  vocabulario que el usuario conoce.

### 2. Backend antes que interfaz

Las reglas de `Valid` van al backend, no solo al formulario. En el original la
validación vivía en la pantalla porque no había otro sitio; si se queda solo ahí,
la API queda abierta.

### 3. Armar la pantalla con el marco

```html
<form appTeclas (confirmar)="guardar()" (cancelar)="deshacer()">
  <app-barra-crud
    [modo]="modo()"
    (accion)="ejecutar($event)"
    (modoCambia)="modo.set($event)"
    (navegar)="ir($event)" />

  <label appZoom>Identificación</label>
  <input appZoom [readonly]="!editable()" [(ngModel)]="paciente.identificacion" />
</form>
```

`appZoom` va en **etiquetas y campos**, no solo en campos: en el original estaba
en los dos.

### 4. Verificar contra la original

Antes de dar la pantalla por migrada:

- [ ] Todas las reglas de `Valid` están, y también en el backend
- [ ] Los mensajes al usuario dicen lo mismo que los `MESSAGEBOX`
- [ ] Los botones se habilitan y deshabilitan igual
- [ ] Esc, Ctrl+Enter y Enter se comportan igual
- [ ] Clic derecho amplía en los mismos sitios
- [ ] Las fechas vacías se rellenan con hoy donde el original lo hacía
- [ ] Lo que se decidió mejorar quedó anotado, no hecho a escondidas

---

## Los comportamientos que hay que reproducir

Medido sobre los 37 formularios desplegados con
`vfp_lector.py --comportamiento`:

| Comportamiento | Usos | Formularios |
|---|---|---|
| zoom con clic derecho | 828 | 31 |
| validación al salir del campo | 353 | 31 |
| aviso explícito al usuario | 228 | 29 |
| botón condicionado al resultado | 211 | 22 |
| Ctrl+Enter confirma | 89 | 22 |
| indicador de "un momento por favor" | 63 | 20 |
| campo condicionado (`When`) | 66 | 19 |
| fecha que se autocompleta con hoy | 23 | 12 |
| Esc cierra | 8 | 8 |
| **Enter avanza, no envía** | 4 | 4 |

El último es el más fácil de romper sin darse cuenta: en la web Enter envía el
formulario por defecto. Quien lleva años digitando en serie —dato, Enter, dato,
Enter— va a guardar a medio llenar una y otra vez si no se intercepta.

---

## Lo que ya se leyó del sistema (no hay que preguntarlo)

Estaba todo en los `.scx` y en `clases/medico.vcx`. Se había dado por
inaccesible sin comprobarlo.

### Las pestañas

Vienen en la columna `PROPERTIES` del pageframe, con su `Caption` y su orden.

| Formulario | Pestañas |
|---|---|
| `historia` | GRAL · AF · AP · AGO · API · MVTO |
| `historia3` | las anteriores + MEDICA · R.PARACLINICOS |
| `historia`/`historia3`, dentro de MVTO | MOTIVO · ENFER. · R.SISTE. · DIAG. · OTROS |
| `pacientes` | NOMBRES · TELS · DIRECCION · OTROS · PERSONALES |
| `exa_fisico_gen` | 9 pestañas (ver el `.scx`) |

### Los métodos compartidos, tal como son

```
zoomx(obj)              -> DO FORM zoomx WITH obj      ES UN MODAL, no un tooltip

activar_objetos(bool)   -> setall("readonly", b, "textbox" | "checkbox"
                                              | "editbox" | "spinner")
                           setall("enabled", NOT b, "combobox")

teclas_calientes(k, s)  -> Esc PREGUNTA («¿Está seguro de cerrar…?», con
                           tiempo de espera de 5 s) y solo entonces wzundo+wzexit
                           Alt+1/2/3 -> wznew / wzedit / wzdelete

mousemovex(obj, n)      -> obj.fontsize = 18 si entra, 14 si sale
```

### Las fórmulas clínicas

```
peso ideal   (>12 años)  talla² × 0.23  hombres    (IMC objetivo 23)
                         talla² × 0.22  mujeres    (IMC objetivo 22)
             (≤12 años)  la rama de la tabla `pesotalla` está COMENTADA:
                         hoy devuelve 0. Preguntar si eso es intencional.

talla ideal              no es fórmula: se busca en la tabla `pesotalla`
                         por edad + unidad + sexo
```

---

## Diferido a la etapa de mejoras

Decisión de Juan: los EFECTOS VISUALES no entran en la migración. Se anotan aquí
para que "después" no se vuelva "nunca".

- **`mousemovex`**: la fuente del control crece de 14 a 18 puntos al pasar el
  ratón. 108 usos.
- **La presentación del zoom**: se conserva la función —ver el contenido
  ampliado— pero su apariencia se resuelve luego.
- Tipografías, colores y `Arial Black` de las pestañas.

La línea trazada: **efecto visual → mejoras; comportamiento que cambia lo que el
usuario puede hacer o perder → migración**. Por eso la confirmación de Esc sí se
migra: sin ella se pierde una historia a medio escribir.

---

## Lo que sigue necesitando al prestador

Solo dos, y las dos dependen de datos históricos o del estado del mundo, no del
código:

1. **El evento de vigilancia** (`cve1`): en el sistema nuevo se DERIVA del
   diagnóstico. ¿Alguna vez registró uno distinto del diagnóstico principal? Si
   sí, los informes darán números diferentes de los que ha venido enviando.
2. **El validador 1.4.8.0**: ¿el archivo RIPS que produce hoy pasa limpio? Desde
   el 1-jun-2026 trece reglas que solo avisaban son causal de rechazo.
