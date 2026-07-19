# Cómo se migra un formulario del sistema original

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

## Lo que falta por decidir con el prestador

1. **`mousemovex`** (108 usos): se sabe que hay realce al pasar el ratón, pero no
   qué hace exactamente. Hay que verlo funcionando o preguntar.
2. **`calcularpesoideal` y `calculartallaideal`**: fórmulas clínicas propias del
   sistema. Hay que leer su código y confirmar cuál se usa, porque hay varias
   convenciones.
3. **Los `Page1`…`Page8`**: `historia` tiene ocho pestañas. Confirmar el orden y
   qué va en cada una; el orden es memoria muscular.
4. **El evento de vigilancia** (`cve1`): en el sistema nuevo se deriva del
   diagnóstico. Preguntar si alguna vez registró uno distinto del diagnóstico,
   porque entonces los informes darían números diferentes.
