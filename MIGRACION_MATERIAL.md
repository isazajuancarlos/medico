<!-- SPDX-License-Identifier: LicenseRef-Propietario -->
# PrimeNG → Angular Material (#255)

> **HISTORIA DEL REPOSITORIO, NO INSTRUCCIONES VIGENTES.** Angular salió el
> 2026-08-08: las 29 pantallas se tejen en el servidor con `maud` + htmx dentro
> del binario (directiva 26), y `frontend/` ya no existe. Este documento se
> conserva porque describe decisiones que se tomaron y se pagaron, y porque su
> MÉTODO —extraer el comportamiento del original antes de suponerlo— sigue
> valiendo. Lo que no sirve es seguir sus pasos: no hay frontal donde aplicarlos.

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

1. PrimeNG → Material **con Angular 21**, verde. — **HECHO el 2026-08-02.**
2. Después, el salto de medico a Angular 22 (#254), que esto desbloquea.

Un cambio a la vez: si se hacen juntos y algo se rompe, no se sabe cuál fue.

> **Terminado.** `primeng` y `@primeuix/themes` salieron del `package.json` y de
> `node_modules`; `providePrimeNG` salió de `app.config.ts`. Quedan **cero**
> importaciones `from 'primeng/…'`. `primeicons` se queda: es MIT, es solo una
> fuente de iconos y no tiene el problema de licencia de `primeng`.

## Estado

| momento | importaciones `from 'primeng/…'` | archivos |
|---|---|---|
| antes | 287 | 35 |
| tras servicios + vocabulario barato | 170 | 30 |
| tras rips, contrarreferencia y pacientes | 159 | 27 |
| tras remisiones | 154 | 26 |
| tras barra CRUD, cabecera, incapacidades y consentimiento | 140 | 22 |
| tras urgencias y superadmin | 128 | 20 |
| tras el formulario de paciente | 122 | 19 |
| tras imágenes diagnósticas | 116 | 18 |
| tras estadísticas (y el gráfico propio) | 112 | 17 |
| tras evoluciones | 107 | 16 |
| tras epicrisis | 102 | 15 |
| tras citas y rehabilitación | 87 | 13 |
| tras nota operatoria e interconsultas | 77 | 11 |
| tras ambulancias y hospitalización | 65 | 9 |
| tras agenda médica y laboratorio | 46 | 7 |
| tras nutrición y quirófano | 34 | 5 |
| tras farmacia | 27 | 4 |
| tras facturación | 21 | 3 |
| tras administración | 14 | 2 |
| **tras historias clínicas y `app.config`** | **0** | **0** |

`ng build` y las pruebas del frontend en verde en **cada** paso — esa es la red,
y por eso los lotes son pequeños.

## La red: el banco de humo

De las 30 páginas, **tres** tenían prueba. Para las otras 27 la única red era
`ng build`, que dice que el TypeScript compila y **no** dice si la pantalla se
abre: cambiar un `<p-table>` por un `<table mat-table>` puede dejar la tabla
vacía sin que nada falle. Migrar 18 000 líneas así es migrar a ciegas.

`src/app/pantallas-montan.spec.ts` monta **todas** las pantallas y comprueba que
un ciclo de detección de cambios no lanza. Pasó de 82 a **130 pruebas**.

Dos decisiones que lo hacen útil:

- **No enumera componentes: recorre `app.routes.ts`.** Una pantalla nueva entra
  sola y una borrada sale sola. Un banco escrito a mano nace completo y caduca en
  el primer commit. Lleva además una guardia que falla si la tabla de rutas deja
  de exponer pantallas — si no, un refactor convertiría el bucle en cero
  iteraciones y pasaría en verde sin probar nada.
- **Las peticiones se interceptan y no se responden**, y no hay `http.verify()`:
  se comprueba el montaje con la pantalla vacía, que es el estado en el que
  siempre se abre de verdad. Dejar peticiones sin atender es el caso, no un
  descuido.

**Discrimina, comprobado**: con un `throw` en el `ngOnInit` de una pantalla, el
banco la señala a ella y solo a ella.

## La segunda red: el guardián de las directivas mudas

`src/app/directivas-de-material-cableadas.spec.ts`. Comprueba que una directiva
de Material escrita en una plantilla esté **cableada** en los `imports` de su
componente.

Existe porque el banco de humo **no** llega ahí. Una directiva de Material se
escribe casi siempre como atributo estático —`matTooltip="Ver"`,
`matInput`, `matIconButton`, `mat-cell`—, y para Angular un atributo que ningún
módulo reclama es un atributo corriente, como `title` o `data-x`. Sin el módulo:
el compilador calla, `ng build` pasa, la pantalla monta, las pruebas quedan
verdes, y la directiva **no hace nada**.

Solo son mudos los atributos. Un elemento (`<mat-table>`) sin su módulo rompe la
compilación, y un enlace (`[matTooltip]="x"`) da NG8002. El agujero es
exactamente la forma más común.

Lee el fuente con el glob de Vite y no monta nada, porque lo que hay que
comprobar es **una ausencia en una lista**, y una ausencia no se ve ejecutando.
Cada marca admite varios módulos —`MatInputModule` reexporta
`MatFormFieldModule`— para no producir alarmas falsas (directiva 23), y lleva la
misma guardia de recuento que el banco de humo.

> **El guardián no discriminaba, y se midió (2026-08-02).** Comprobaba que el
> nombre del módulo apareciera **en el archivo**, no en la lista de `imports`
> del componente. Se le quitó `MatTooltipModule` de la lista a `historias`
> dejando arriba su línea `import` —el caso exacto que dice cazar— y las 295
> pruebas siguieron en verde. Dos agujeros, no uno:
>
> 1. la línea `import { MatTooltipModule } from …` hacía verdadera la
>    comprobación aunque Angular no viera el módulo por ningún lado;
> 2. desde que los diálogos son componentes propios, **un archivo lleva de dos a
>    cinco componentes**, y el `imports` de la pantalla tapaba el del diálogo.
>
> Hoy corta el fuente por `@Component(` —lo que deja fuera la cabecera del
> archivo a propósito— y comprueba cada componente contra **su** lista. Pasó de
> 156 a 227 comprobaciones y **discrimina**: con la lista rota nombra
> `HistoriasComponent` y solo a él.
>
> La lección es la de siempre: *el guardián que nadie ha visto ponerse rojo no
> es un guardián.*

`src/entorno-de-pruebas.ts` suple `ResizeObserver`, que jsdom no trae y cualquier
navegador sí. Lo destapó `agenda-medica`; lo pedirá igual Material
(`mat-tab-group`, observadores del CDK). El doble es **inerte a propósito** —
registra y no dispara nada—: uno que inventara redimensiones haría pasar por
probado un camino que en las pruebas no ocurre.

## Lo que Material no da, y se hace propio

En `src/app/shared/ui/`. Son cinco piezas y ningún paquete nuevo.

| pieza | sustituye a | por qué propia |
|---|---|---|
| `<x-etiqueta>` | `<p-tag>` | Material tiene `mat-chip`, que es otra cosa: un control interactivo de un conjunto seleccionable. Pintar «Pendiente» en una tabla con un chip mete un elemento activo donde solo hay que leer. |
| `Avisos` | `MessageService` + `<p-toast>` | sobre `MatSnackBar` |
| `Confirmar` | `ConfirmationService` + `<p-confirmDialog>` | sobre `MatDialog` |
| `<x-mensaje>` | `<p-message>` | el aviso que se queda en la página mientras la condición dure, no el que se va solo. Material solo tiene `mat-error` (atado a un `mat-form-field`) y la Snackbar, que es temporal. |
| `<x-grafico>` | `<p-chart>` | Material no trae gráficos, y **`chart.js` ya era dependencia directa**: `p-chart` no era más que una envoltura de Angular alrededor de ella. Quitarla no añade un paquete; quita uno. |

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

## Las tablas: dos patrones, y elegir mal no falla

`p-table` aparece 25 veces y **no** hay un solo reemplazo. Hay dos, según de
dónde salga la paginación:

**(a) Paginación en cliente** — la tabla recibe todas las filas
(`contrarreferencia`):

```ts
readonly columnas = ['fecha', 'paciente', /* … */];
readonly datos = new MatTableDataSource<Fila>([]);

// Por SETTER, no por @ViewChild a secas: la tabla vive dentro de un @if en
// varias pantallas, así que el elemento aparece y desaparece. Con la forma
// estática se engancha una vez —o ninguna— y la paginación queda muerta sin
// que nada falle.
@ViewChild(MatPaginator)
set paginador(p: MatPaginator | undefined) { if (p) this.datos.paginator = p; }
```

**(b) Paginación del servidor** — la tabla recibe una página (`pacientes`):

```html
<table mat-table [dataSource]="filas()">…</table>
<mat-paginator [length]="total()" [pageSize]="pageSize" (page)="alPaginar($event)" />
```

**Darle un `MatTableDataSource` a una tabla paginada por el servidor lo pagina en
cliente sobre lo que ya tiene**: con miles de pacientes muestra veinte y afirma
que no hay más. Compila, se ve bien, y es falso.

**Dos tablas en la misma pantalla** (`superadmin`) se enganchan por referencia de
plantilla, `@ViewChild('pagIps')`, no por tipo: `@ViewChild(MatPaginator)`
devuelve el primero y deja el segundo mudo sin que nada falle.

Y hay tablas que **no se paginan a propósito**: la cola de `urgencias` llega
ordenada por prioridad, y una segunda página escondería al rojo.

Y una tercera cosa que no se hereda: **`mat-table` no tiene plantilla de vacío**
como el `pTemplate="emptymessage"` de `p-table`. El «no hay nada» va fuera de la
tabla, y hay que ponerlo: una tabla sin filas y sin frase se lee como «todavía
está cargando».

## Los diálogos: dos patrones, y el segundo cambia el código

`p-dialog` era declarativo —`[(visible)]`, contenido en la misma plantilla—.
`MatDialog` es imperativo: el contenido es **otro componente** y se abre con una
llamada. Eso obliga a partir la pantalla, y hay dos formas según lo que el
diálogo haga:

**(a) Solo muestra** (`incapacidades`, el detalle de `consentimiento`): el
componente recibe el dato por `MAT_DIALOG_DATA`, no inyecta nada y no conoce a
quien lo abrió.

```ts
constructor(@Inject(MAT_DIALOG_DATA) public d: Detalle) {}
// …
this.dialogo.open(CertificacionComponent, { data: d, width: '38rem', maxWidth: '95vw' });
```

**(b) Devuelve un dato** (la revocación de `consentimiento`): el campo vive
**dentro** del diálogo y sale por `afterClosed()`.

```html
<button matButton="filled" [disabled]="!motivo.trim()" [mat-dialog-close]="motivo.trim()">
```
```ts
.afterClosed().subscribe((motivo?: string) => { if (motivo) this.revocar(c, motivo); });
```

Con `p-dialog` ese texto era un campo de la pantalla anfitriona y el diálogo solo
su envoltorio visible. La diferencia no es estética: cerrar por Escape devuelve
`undefined`, y así no queda un motivo a medio escribir esperando al siguiente
paciente. En `urgencias` y `superadmin` eso borró de la clase anfitriona seis
campos y tres métodos `abrirX()` que no hacían más que limpiarlos.

Y el formulario completo **valida dentro**: el botón lleva
`[mat-dialog-close]="resultado()"`, y `resultado()` devuelve `null` si falta
algo. El botón ya está inerte, pero un diálogo capaz de cerrarse devolviendo un
triage a medias sería un paciente en cola sin nivel.

**Lo que sí se pierde y hay que reponer**: el `[loading]` que el botón del
`p-dialog` mostraba mientras se guardaba. Ahora el diálogo se cierra y la
petición sigue viva, así que la pantalla se queda igual y parece que no pasó
nada. Va una `<mat-progress-bar>` con `cargando() || guardando()`.

## Los colores: el input `color` de un botón no pinta nada

El tema es **M3** (`azure-blue` prebuilt), y ahí el input `color` de `MatButton`
**no tiene efecto** — lo dice la propia documentación de Material. Traducir
`severity="danger"` a `color="warn"` compila, pasa las pruebas y deja el botón de
eliminar en gris: un aviso que desaparece sin que nada falle.

Lo destructivo y lo afirmativo van por CSS, con las clases `.peligro` y `.exito`
(`#dc2626` / `#16a34a`), que es lo mismo que ya hacía `confirmar.service.ts`.

## Lo que Material no trae y no merece un módulo

- **`p-avatar`** (la inicial en un círculo de la cabecera) → un `<span>` con
  `border-radius: 50%`. Traer un componente entero por esto es desproporcionado.
- **`[loading]` de `p-button`** → el icono pasa a `pi pi-spin pi-spinner` y el
  botón queda inerte. Las dos cosas, no solo la segunda: un botón que se apaga
  sin decir por qué se lee como averiado.

## El gráfico: lo que probarlo puede y no puede alcanzar

`x-grafico` conserva la firma de `p-chart` —`type`, `[data]`, `[options]`,
`height`— por la misma razón que `Avisos` y `Confirmar`.

Dos cosas que el cambio destapó:

- **El tipo empezó a comprobar.** `p-chart` recibía `any`, así que
  `plugins: { legend: { position: 'bottom' } }` se ensanchaba a `string` y un
  `'abajo'` habría compilado, con chart.js ignorándolo en silencio. La
  anotación va en la **declaración** (`readonly hbarOpts: ChartOptions<'bar'>`),
  no en el input del componente: ese es polimórfico —`indexAxis` solo existe en
  barras, `elements.line` solo en líneas— y cerrarlo rechazaría la mitad de los
  quince gráficos.
- **El banco de humo no lo alcanza.** Abre `estadisticas` en la pestaña
  «resumen», donde no hay ningún gráfico; y jsdom no da contexto 2D, así que un
  lienzo de chart.js no llega a existir en pruebas. Por eso la regla que decide
  *si aparece un gráfico* se sacó a una función exportada, `hayQueDibujar()`,
  con sus propias pruebas: un `datasets: [{ data: [] }]` es la espera, no un
  gráfico vacío, y dibujarlo pinta ejes en cero que en un panel de indicadores
  se leen como «no hubo atenciones». Un cero **sí** es un dato y sí se dibuja.

## El calendario

`provideNativeDateAdapter()` y `{ provide: MAT_DATE_LOCALE, useValue: 'es-CO' }`,
una vez en `app.config.ts` para las cinco pantallas que lo usan. **El locale no
es cosmético**: sin él Material escribe las fechas en el formato de Estados
Unidos —mes/día/año—, y «03/04» leído al revés en una historia clínica es un mes
de diferencia.

## Trampas ya pagadas

1. **Backticks dentro del template literal. LA MÁS FÁCIL DE REPETIR — van ya
   CINCO veces, estando escrita aquí desde la primera.** El conteo se deja a la
   vista porque es un dato sobre el método, no sobre el código: leer la trampa
   no la evita. Lo que la evita es una regla de escritura —**en un comentario
   dentro de una plantilla, ningún nombre lleva comillas invertidas**— y
   compilar el lote pequeño. Un comentario HTML que nombre
   `` `p-password` `` cierra la cadena del `template:`, y el compilador escupe
   diez errores sin relación (`shorthand property 'p'`, `Cannot find name
   'styles'`, `Incorrect number of arguments to @Component`). Si aparecen
   errores absurdos tras editar una plantilla, es esto y solo esto. **En un
   comentario dentro de una plantilla, los nombres van sin comillas invertidas.**

   > Se llegó a escribir una prueba que lo cazara —directiva 6: *prueba que
   > falla* > *directiva que recordar*— y **se borró tras medirla**: el error
   > rompe la compilación, así que la prueba nunca llega a ejecutarse. No añadía
   > detección sobre el compilador; solo habría dado una falsa sensación de red.
   > La salvaguarda es el `ng build` de cada lote, que ya la caza siempre.
   >
   > Se volvió a evaluar en el quinto tropiezo, buscando un guardián que diera
   > el diagnóstico en una línea en vez de diez errores absurdos. **No hay
   > sitio donde ponerlo**: una prueba no llega a correr (el archivo roto lo
   > importa el banco de humo, así que la suite entera deja de compilar) y un
   > guion suelto que nadie invoca es característica falsa. Queda el compilador,
   > que tarda doce segundos y acierta siempre.

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

5-bis. **La directiva muda: cambiar el atributo sin cablear el módulo.** El
   barrido de tooltips dejó `matTooltip` en cinco pantallas sin
   `MatTooltipModule`, y estuvo un día así con todo en verde — es la trampa 4 al
   revés (allí sobraba el elemento, aquí falta el módulo), y la mitad silenciosa
   de las dos. Ya no puede repetirse: la caza
   `directivas-de-material-cableadas.spec.ts`. **Al migrar una pantalla se
   comprueba la lista de `imports`, no solo la plantilla.**

6. **El banco de humo montaba una aplicación que no existía.** Traía su propia
   lista de proveedores, así que nada de `app.config.ts` estaba bajo prueba: lo
   destapó `/rips`, que falló por un `DateAdapter` que *sí* estaba en la
   aplicación. Ahora extiende `appConfig.providers` y solo pisa lo que una
   prueba debe pisar (HttpClient de pruebas, router vacío, animaciones inertes),
   aprovechando que en Angular gana el último proveedor. Una lista paralela se
   desincroniza en el primer cambio.

7. **Datos que la interfaz nueva descartaría en silencio.** El compilador cazó
   `life: 3000` en los avisos y `icon` en las etiquetas: alguien eligió esos
   valores, así que el tipo propio los acepta en vez de perderlos
   (directiva 20). En cambio `acceptButtonStyleClass: 'p-button-danger'` sí se
   tradujo a `peligroso: true` — vocabulario ajeno que no había que heredar.

## Cómo se hizo lo caro (y por qué no con sustitución ciega)

Los cuatro vocabularios caros —`p-button`, `p-table`, `pInputText`/`textarea`,
`p-select`, `p-dialog`— fueron **pantalla por pantalla y a mano**, de la más
pequeña a la mayor, con `ng build` y las pruebas entre lote y lote. Ninguno
mapea 1:1: `severity`/`text`/`outlined` no son `matButton`/`="filled"`/
`="outlined"`, y un reemplazo ciego produce botones equivocados **que
compilan**; `p-table` cambia de paradigma (`pTemplate="body"` → un
`matColumnDef` por columna); `pInputText` exige envolver en `<mat-form-field>`.

**Dos veces sí se automatizó, y con la misma regla**: en `admin` (46 campos) y
en `historias` (40) los bloques `<div class="field"><label>…</label><control/>`
se convirtieron con un **recorrido consciente de bloques** —contando los `<div>`
anidados—, no con una expresión regular sobre el HTML (directiva 17). Lo que no
encajaba en «etiqueta + un control» se apartó y se hizo a mano: 8 en `admin`,
7 en `historias`. El recuento de convertidos y apartados se imprime, porque un
transformador que salta en silencio es peor que uno que no corre.

Las pantallas que fijan un patrón:

| pantalla | fija |
|---|---|
| `contrarreferencia` | tabla con paginación en cliente |
| `pacientes` | tabla con paginación del servidor |
| `remisiones` | el `mat-select` |
| `incapacidades` | el diálogo que solo muestra |
| `consentimiento` | el diálogo que devuelve un dato |
| `urgencias` | el diálogo con formulario completo y su validación dentro |
| `superadmin` | dos paginadores en una pantalla; el calendario y el ojo de la clave |
| `paciente-form` | el diálogo que **el padre** abría con `[abierto]` |
| `barra-crud` | el botón de icono y el color por CSS |
| `ambulancias` | el diálogo que recibe por `MAT_DIALOG_DATA` una lista **calculada** (las que pueden salir), y el que pide sus propios datos |
| `quirófano` | el diálogo que guarda por dentro (el checklist) y devuelve solo la acción |
| `farmacia` | el selector que hay que **poblar antes de abrir**, para que nadie concluya «no hay proveedores» |
| `facturación` | el diálogo con dos salidas distintas (aceptar / rechazar la glosa), que no es una confirmación |
| `agenda médica` | `mat-stepper` no; aquí `mat-tab-group` y el `datetime-local` que quita la conversión de zona |
| `historias` | `mat-stepper` gobernado por una señal: `[selectedIndex]` la lee, `(selectionChange)` la escribe |
| `laboratorio` | la rejilla densa que NO se envuelve en `mat-form-field`: `input` desnudos con CSS de la página |

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
