import { Component, Input, ChangeDetectionStrategy } from '@angular/core';

/**
 * La etiqueta de estado — lo que en PrimeNG era `<p-tag>` (25 usos) y en
 * Angular Material **no existe**: Material tiene `mat-chip`, que es otra cosa
 * (un elemento interactivo de un conjunto seleccionable, con foco y borrado).
 * Usar un chip para pintar «Pendiente» en una tabla mete un control activo
 * donde solo hay que leer un estado.
 *
 * Así que se hace propia, que es lo que dice la directiva 26 para lo que
 * Material no da. Son 30 líneas y ningún paquete nuevo.
 *
 * Los cinco tonos son los de PrimeNG a propósito —`success`, `info`, `warn`,
 * `danger`, `secondary`—: son los que ya están escritos en las 25 pantallas, y
 * cambiarles el nombre en la misma pasada en que se cambia de librería
 * mezclaría dos cambios en un solo diff.
 */
@Component({
  selector: 'x-etiqueta',
  standalone: true,
  template: `<span class="x-etiqueta" [class]="'tono-' + severity">
    @if (icon) {
      <i [class]="icon"></i>
    }
    {{ value }}
  </span>`,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [
    `
      .x-etiqueta {
        display: inline-block;
        padding: 0.15rem 0.5rem;
        border-radius: 6px;
        font-size: 0.75rem;
        font-weight: 600;
        line-height: 1.4;
        white-space: nowrap;
      }
      .tono-success {
        background: #dcfce7;
        color: #166534;
      }
      .tono-info {
        background: #dbeafe;
        color: #1e40af;
      }
      .tono-warn {
        background: #fef3c7;
        color: #92400e;
      }
      .tono-danger {
        background: #fee2e2;
        color: #991b1b;
      }
      .tono-secondary {
        background: #e2e8f0;
        color: #334155;
      }
    `,
  ],
})
export class EtiquetaComponent {
  @Input() value = '';
  @Input() severity: 'success' | 'info' | 'warn' | 'danger' | 'secondary' = 'secondary';
  /**
   * Clase del icono (`pi pi-…`). Existe porque nueve de las etiquetas ya lo
   * traían: descartarlo al cambiar de librería sería quitar de la pantalla algo
   * que alguien puso, y en una tabla de estados el icono es la mitad de la
   * lectura.
   */
  @Input() icon?: string;
}
