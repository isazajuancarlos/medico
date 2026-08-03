// SPDX-License-Identifier: LicenseRef-Propietario
import { describe, expect, it } from 'vitest';

/**
 * Que una directiva de Material escrita en una plantilla esté CABLEADA.
 *
 * Nace de un fallo real: el barrido que cambió `pTooltip` por `matTooltip`
 * (commit 6cc41ad) no añadió `MatTooltipModule` a los `imports` de cinco
 * pantallas. `matTooltip="Cancelar cita"` es, para Angular, un atributo
 * estático corriente —como `title` o `data-x`—, así que **nada falla**: el
 * compilador no se queja, `ng build` pasa, el banco de humo monta la pantalla y
 * las pruebas quedan verdes. Simplemente el tooltip no existe. Se descubrió a
 * ojo, un día después.
 *
 * Es distinto de un elemento (`<mat-table>` sin su módulo es un elemento
 * desconocido y rompe la compilación) y de un enlace (`[matTooltip]="x"` da
 * NG8002). El agujero son exactamente las directivas escritas como ATRIBUTO
 * ESTÁTICO, que es como se escribe la mayoría de ellas.
 *
 * Por eso el guardián lee el archivo fuente y no monta nada: lo que hay que
 * comprobar es una ausencia en la lista de `imports`, y una ausencia no se ve
 * ejecutando.
 */

/**
 * Marca en la plantilla → módulos que la aportan. Basta UNO.
 *
 * Las alternativas no son laxitud: `MatInputModule` reexporta
 * `MatFormFieldModule`, así que exigir el segundo cuando ya está el primero
 * sería una alarma falsa — y una alarma que resulta falsa dos veces es una que
 * a la tercera nadie mira (directiva 23).
 */
const CABLEADO: ReadonlyArray<{ marca: RegExp; modulos: string[] }> = [
  { marca: /\bmatTooltip\b/, modulos: ['MatTooltipModule'] },
  { marca: /\bmat(Button|IconButton|Fab|MiniFab)\b/, modulos: ['MatButtonModule'] },
  { marca: /\bmatInput\b/, modulos: ['MatInputModule'] },
  { marca: /\bmat(Suffix|Prefix|Label|Error|Hint)\b/, modulos: ['MatFormFieldModule', 'MatInputModule'] },
  { marca: /\bmat(ColumnDef|HeaderCellDef|CellDef|HeaderRowDef|RowDef)\b/, modulos: ['MatTableModule'] },
  { marca: /\bmat-(cell|header-cell|row|header-row)\b/, modulos: ['MatTableModule'] },
  { marca: /\bmat(Sort|SortHeader)\b/, modulos: ['MatSortModule'] },
  { marca: /\bmat-dialog-(title|content|actions|close)\b/, modulos: ['MatDialogModule'] },
  { marca: /\bmatDialogClose\b/, modulos: ['MatDialogModule'] },
  { marca: /\bmatBadge\b/, modulos: ['MatBadgeModule'] },
  { marca: /\bmatRipple\b/, modulos: ['MatRippleModule', 'MatCommonModule'] },
  { marca: /\bmatDatepicker\b/, modulos: ['MatDatepickerModule'] },
  { marca: /\bmatAutocomplete\b/, modulos: ['MatAutocompleteModule'] },
  { marca: /\bmatMenuTriggerFor\b/, modulos: ['MatMenuModule'] },
  { marca: /\bmat-(card-title|card-content|card-actions|card-subtitle)\b/, modulos: ['MatCardModule'] },
];

// El fuente se lee con el glob de Vite —no con `node:fs`— para no declarar
// `@types/node` solo por esto: lo que hay en node_modules hoy llegó de rebote
// como dependencia de otra cosa, y apoyarse en eso es apoyarse en un accidente.
const FUENTES = import.meta.glob('./**/*.ts', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

const ARCHIVOS = Object.entries(FUENTES).filter(([ruta]) => !ruta.endsWith('.spec.ts'));

/**
 * Un componente: su plantilla y la lista de `imports` que la sostiene.
 *
 * Se mira POR COMPONENTE y contra su lista de `imports`, no contra el archivo
 * entero. Las dos razones son fallos que este guardián dejaba pasar, medidos el
 * 2026-08-02 quitando `MatTooltipModule` de la lista de `historias` y viendo
 * las 295 pruebas en verde:
 *
 * 1. La línea `import { MatTooltipModule } from '@angular/material/tooltip'`
 *    hacía verdadera la comprobación aunque el módulo NO estuviera en la lista
 *    del componente — que es justo lo único que Angular mira.
 * 2. Desde la migración a MatDialog, un archivo lleva entre dos y cinco
 *    componentes (la pantalla y sus diálogos). El `imports` de la pantalla
 *    tapaba el del diálogo: `matTooltip` dentro de un diálogo sin
 *    `MatTooltipModule` en SU lista pasaba en verde.
 *
 * El corte por `@Component(` deja fuera la cabecera del archivo —donde viven
 * los `import`— a propósito: es la mitad del agujero.
 */
interface Componente {
  archivo: string;
  nombre: string;
  cuerpo: string;
  /** El texto de dentro de `imports: [ … ]`, o `null` si no lo declara. */
  lista: string | null;
}

function componentesDe(archivo: string, fuente: string): Componente[] {
  const trozos = fuente.split('@Component(');
  return trozos.slice(1).map((cuerpo, i) => {
    const lista = /imports:\s*\[([^\]]*)\]/.exec(cuerpo);
    const clase = /export class (\w+)/.exec(cuerpo);
    return {
      archivo,
      nombre: clase ? clase[1] : `${archivo}#${i + 1}`,
      cuerpo,
      lista: lista ? lista[1] : null,
    };
  });
}

const COMPONENTES = ARCHIVOS.flatMap(([ruta, fuente]) => componentesDe(ruta, fuente));

describe('las directivas de Material están cableadas', () => {
  // Si el recorrido dejara de encontrar componentes, el bucle de abajo tendría
  // cero iteraciones y pasaría en verde sin comprobar nada. Misma guardia que
  // el banco de humo.
  it('encuentra los componentes del proyecto', () => {
    expect(ARCHIVOS.length).toBeGreaterThan(40);
    expect(COMPONENTES.length).toBeGreaterThan(50);
  });

  for (const componente of COMPONENTES) {
    for (const { marca, modulos } of CABLEADO) {
      if (!marca.test(componente.cuerpo)) continue;

      it(`${componente.nombre} (${componente.archivo}) importa ${modulos[0]}`, () => {
        const lista = componente.lista ?? '';
        const cableado = modulos.some((m) => new RegExp(`\\b${m}\\b`).test(lista));
        expect(
          cableado,
          `usa ${marca.source} pero su lista de imports no trae ninguno de: ` +
            `${modulos.join(', ')}. Como es un atributo estático, Angular no se ` +
            'queja y la directiva simplemente no hace nada.',
        ).toBe(true);
      });
    }
  }
});
