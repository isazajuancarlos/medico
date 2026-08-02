<!-- SPDX-License-Identifier: LicenseRef-Propietario -->
# PrimeNG → Angular Material (#255)

Receta de la migración del frontend de medico, escrita **mientras se hace** y con
las trampas ya pagadas. Es el mismo método de `MIGRACION_FORMULARIOS.md`: se
construye el vocabulario una vez y cada pantalla pasa a ser traducción.

## Por qué

Directiva 26: el estándar de todos los desarrollos es Angular con
`@angular/material`. Y hay una fecha: **PrimeNG 22 dejó de ser MIT** —es
«PrimeUI», comercial, con clave obligatoria que sin ella estampa *«Invalid
PrimeUI License»* sobre la aplicación—. Un binario que vive offline en casa de un
cliente no puede llevar dentro una clave que caduca.

Eso deja a medico clavado en `primeng` 21.1.9, que declara peer
`@angular/core ^21.0.7`, y por tanto **bloquea #254** (una sola versión de
Angular: informes ya está en 22.1.0).

> **Dato medido el 2026-08-02, por si algún día urge:** Angular 22 + PrimeNG 21
> *sí* compila y pasa las pruebas — `ng update` avisa de los peers incompatibles
> y sigue. O sea que existía un atajo. No se tomó: correr una librería fuera de
> su rango declarado, en un producto que se instala offline y donde
> `--legacy-peer-deps` ya está encendido de fábrica (así que el aviso no
> volvería a aparecer), es deuda que nadie va a auditar. Y PrimeNG 21 no va a
> recibir parches para Angular 22.

## Orden decidido

1. PrimeNG → Material **con Angular 21**, verde.
2. Después, el salto de medico a Angular 22.

Un cambio a la vez: si se hacen juntos y algo se rompe, no se sabe cuál fue.

## Estado

| momento | importaciones `from 'primeng/…'` | archivos |
|---|---|---|
| antes | 287 | 35 |
| tras servicios + vocabulario barato | 170 | 30 |

`ng build` y las 82 pruebas del frontend en verde en **cada** paso — esa es la
red, y por eso los lotes son pequeños.

## Lo que Material no da, y se hace propio

En `src/app/shared/ui/`. Son cuatro piezas y ningún paquete nuevo.

| pieza | sustituye a | por qué propia |
|---|---|---|
| `<x-etiqueta>` | `<p-tag>` | Material tiene `mat-chip`, que es otra cosa: un control interactivo de un conjunto seleccionable. Pintar «Pendiente» en una tabla con un chip mete un elemento activo donde solo hay que leer. |
| `Avisos` | `MessageService` + `<p-toast>` | sobre `MatSnackBar` |
| `Confirmar` | `ConfirmationService` + `<p-confirmDialog>` | sobre `MatDialog` |
| `<x-mensaje>` | `<p-message>` | el aviso que se queda en la página mientras la condición dure, no el que se va solo. Material solo tiene `mat-error` (atado a un `mat-form-field`) y la Snackbar, que es temporal. |

**`Avisos` y `Confirmar` conservan la firma de PrimeNG a propósito** —
`add({ severity, summary, detail })` y `confirm({ header, message, accept })`.
Son 80 y 11 llamadas: cambiar de librería y de vocabulario en el mismo diff
mezcla dos cambios, y si algo se rompe no se sabe cuál fue. Renombrarlas a
`exito`/`error` es una pasada aparte, mecánica, cuando esta esté verde. El
servicio es propio; PrimeNG no queda debajo.

## Equivalencias

| PrimeNG | Material | notas |
|---|---|---|
| `<p-tag [value] [severity] icon>` | `<x-etiqueta>` | mismos inputs |
| `<p-divider />` | `<mat-divider />` | |
| `pTooltip="…"` | `matTooltip="…"` | |
| `tooltipPosition="bottom"` | `matTooltipPosition="below"` | **los valores NO coinciden** |
| `<p-message severity text>` | `<x-mensaje>` | |
| `<p-toast />` | *nada* | `MatSnackBar` se monta en un overlay del CDK |
| `pInputText` | `matInput` dentro de `<mat-form-field>` | exige envolver, no solo cambiar el atributo |
| `<p-password [toggleMask]>` | `matInput` + botón de ojo propio | Material no trae el ojo; son cuatro líneas |
| `<p-button>` | `<button matButton>` | **no es 1:1**, ver abajo |
| `<p-table>` | `<table mat-table>` | **cambia de paradigma**, ver abajo |

## Trampas ya pagadas

1. **Backticks dentro del template literal.** Escribir un comentario HTML que
   nombre `` `p-password` `` cierra la cadena del `template:` y el compilador
   escupe diez errores sin relación (`shorthand property 'p'`, `Cannot find name
   'styles'`). Si aparecen errores absurdos tras editar una plantilla, es esto.

2. **`tooltipPosition="bottom"` no es `matTooltipPosition="bottom"`.** Material
   usa `above`/`below`/`before`/`after`. Traducirlo mal no falla: deja el
   tooltip en su posición por defecto, y eso se ve tarde y en producción.

3. **Sustitución encadenada.** Reemplazar el import y luego el símbolo pasa dos
   veces sobre el mismo texto: `TooltipModule` → `MatTooltipModule` →
   `MatMatTooltipModule`. Y el `grep` de verificación que filtra por `Mat` para
   quitar ruido **esconde justo eso**. Verificar sin filtros.

4. **Quitar el provider sin quitar el elemento.** Los 21
   `providers: [MessageService]` locales se fueron, pero las plantillas seguían
   con `<p-toast />`, y ese componente inyecta el servicio de PrimeNG: la
   aplicación compilaba y las pruebas se caían con `NG0201`. Provider y elemento
   se van juntos. *(Lo cazaron las pruebas, que es para lo que están.)*

5. **`<p-confirmdialog />` en minúsculas.** Angular no lo reconoce como el mismo
   selector que `<p-confirmDialog />`, así que una sustitución por la forma
   camelCase se deja uno atrás.

6. **Datos que la interfaz nueva descartaría en silencio.** El compilador cazó
   `life: 3000` en los avisos y `icon` en las etiquetas: alguien eligió esos
   valores, así que el tipo propio los acepta en vez de perderlos
   (directiva 20). En cambio `acceptButtonStyleClass: 'p-button-danger'` sí se
   tradujo a `peligroso: true` — vocabulario ajeno que no había que heredar.

## Lo que queda, y por qué no se hace con sustitución ciega

170 importaciones, todas de los cuatro vocabularios caros:

| pendiente | usos | dificultad |
|---|---|---|
| `p-button` | 29 | `severity`/`text`/`outlined`/`raised` no mapean 1:1 a `matButton`/`="filled"`/`="outlined"`/`="tonal"`/`matIconButton`. Un reemplazo ciego produce botones equivocados **que compilan**. |
| `p-table` | 25 | cambia de paradigma: `<ng-template pTemplate="body">` → `matColumnDef` por columna. Es reescribir la tabla, no traducirla. |
| `pInputText` / `textarea` | 41 | Material exige envolver en `<mat-form-field>` con su `<mat-label>`; no es un cambio de atributo. |
| `p-select` | 21 | `optionLabel`/`optionValue` → `<mat-option>` con `*ngFor`. |
| `p-dialog` | 13 | declarativo con `[(visible)]` → `MatDialog` imperativo. |
| cola | ~21 | checkbox, card, datepicker (necesita adaptador de fecha y locale es-CO), toggleswitch, tabs, panel, inputnumber, inputgroup, stepper, progressbar, chart, avatar, autocomplete |

Estos van **pantalla por pantalla y a mano**, de la más pequeña a la mayor, con
`ng build` y las pruebas entre lote y lote.

## Lo que NO se toca en esta migración

- **`primeicons` se queda.** Es MIT y es solo una fuente de iconos: no tiene el
  problema de licencia de `primeng`, y cambiarla son ~300 iconos en las
  plantillas sin ningún desbloqueo a cambio. Por eso los componentes propios
  usan `<i class="pi pi-…">` y no `<mat-icon>`, que además obligaría a
  empaquetar otra fuente.
- **`providePrimeNG` en `app.config.ts`** se queda mientras alguna pantalla siga
  usando PrimeNG. Se va con la última, junto con `primeng` y `@primeuix/themes`
  del `package.json`.

## Tema

`node_modules/@angular/material/prebuilt-themes/azure-blue.css`, referenciado
desde `angular.json`. **Nunca un CDN**: el binario va offline a casa del cliente.
La tipografía Inter ya venía empaquetada igual, con `@fontsource`.
