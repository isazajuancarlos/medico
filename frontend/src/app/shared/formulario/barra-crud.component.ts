import { CommonModule } from '@angular/common';
import { Component, computed, EventEmitter, Input, Output, signal, ChangeDetectionStrategy } from '@angular/core';
import { MatButtonModule } from '@angular/material/button';
import { MatTooltipModule } from '@angular/material/tooltip';
import {
  AccionCrud,
  EstadoBarra,
  ModoFormulario,
  barraDe,
  siguienteModo,
} from './estado-formulario';

/**
 * La barra CRUD del sistema original, presente en 18 de los 37 formularios.
 *
 * En FoxPro eran cinco botones —`wznew`, `wzedit`, `wzdelete`, `wzsave`,
 * `wzundo`— más la navegación de `medico1`
 * (`primero`/`anterior`/`siguiente`/`ultimo`). Todos los formularios la tenían
 * igual, en el mismo sitio y con los mismos atajos: por eso el sistema se
 * sentía uno solo aunque fueran decenas de pantallas.
 *
 * Aquí es UN componente. Lo que en el original había que recordar cablear en
 * cada formulario nuevo, ahora viene dado.
 */
@Component({
  selector: 'app-barra-crud',
  standalone: true,
  imports: [CommonModule, MatButtonModule, MatTooltipModule],
  template: `
    <div class="barra-crud">
      <button matIconButton
        [disabled]="!barra().nuevo" (click)="ejecutar('nuevo')"
        matTooltip="Nuevo" matTooltipPosition="below" aria-label="Nuevo">
        <i class="pi pi-plus"></i>
      </button>
      <button matIconButton
        [disabled]="!barra().editar" (click)="ejecutar('editar')"
        matTooltip="Editar" matTooltipPosition="below" aria-label="Editar">
        <i class="pi pi-pencil"></i>
      </button>
      <button matIconButton class="peligro"
        [disabled]="!barra().eliminar" (click)="ejecutar('eliminar')"
        matTooltip="Eliminar" matTooltipPosition="below" aria-label="Eliminar">
        <i class="pi pi-trash"></i>
      </button>

      <span class="separador"></span>

      <button matIconButton class="exito"
        [disabled]="!barra().guardar" (click)="ejecutar('guardar')"
        matTooltip="Guardar (Ctrl+Enter)" matTooltipPosition="below" aria-label="Guardar">
        <i class="pi pi-check"></i>
      </button>
      <button matIconButton
        [disabled]="!barra().deshacer" (click)="ejecutar('deshacer')"
        matTooltip="Deshacer (Esc)" matTooltipPosition="below" aria-label="Deshacer">
        <i class="pi pi-undo"></i>
      </button>

      @if (mostrarNavegacion) {
        <span class="separador"></span>
        <button matIconButton
          [disabled]="!barra().navegar" (click)="navegar.emit('primero')"
          matTooltip="Primero" matTooltipPosition="below" aria-label="Primero">
          <i class="pi pi-angle-double-left"></i>
        </button>
        <button matIconButton
          [disabled]="!barra().navegar" (click)="navegar.emit('anterior')"
          matTooltip="Anterior" matTooltipPosition="below" aria-label="Anterior">
          <i class="pi pi-angle-left"></i>
        </button>
        <button matIconButton
          [disabled]="!barra().navegar" (click)="navegar.emit('siguiente')"
          matTooltip="Siguiente" matTooltipPosition="below" aria-label="Siguiente">
          <i class="pi pi-angle-right"></i>
        </button>
        <button matIconButton
          [disabled]="!barra().navegar" (click)="navegar.emit('ultimo')"
          matTooltip="Último" matTooltipPosition="below" aria-label="Último">
          <i class="pi pi-angle-double-right"></i>
        </button>
      }

      @if (modo !== 'consulta') {
        <span class="aviso-edicion">
          {{ modo === 'nuevo' ? 'Registro nuevo sin guardar' : 'Editando' }}
        </span>
      }
    </div>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .barra-crud { display: flex; align-items: center; gap: .35rem; flex-wrap: wrap; }
    .separador { width: 1px; height: 1.5rem; background: var(--surface-300); margin: 0 .4rem; }
    .aviso-edicion { margin-left: auto; font-size: .8rem; color: var(--primary-color); font-weight: 600; }

    /* El color va aquí y no en un input del botón: el tema de Material es M3, y
       ahí el input color de MatButton NO tiene efecto —lo dice su propia
       documentación—. Escribirlo compilaría y pintaría el botón de gris, que es
       justo el fallo que no se ve: el de eliminar dejaría de avisar. */
    .peligro:not(:disabled) { color: #dc2626; }
    .exito:not(:disabled) { color: #16a34a; }
  `],
})
export class BarraCrudComponent {
  /** Modo actual. Lo gobierna el formulario anfitrión. */
  @Input({ required: true }) modo: ModoFormulario = 'consulta';

  /** Los formularios de un solo registro no navegan. */
  @Input() mostrarNavegacion = true;

  @Output() accion = new EventEmitter<AccionCrud>();
  @Output() modoCambia = new EventEmitter<ModoFormulario>();
  @Output() navegar = new EventEmitter<'primero' | 'anterior' | 'siguiente' | 'ultimo'>();

  barra = (): EstadoBarra => barraDe(this.modo);

  ejecutar(accion: AccionCrud): void {
    const destino = siguienteModo(this.modo, accion);
    if (destino === null) {
      return; // acción no aplicable en este modo; el original tenía el botón inerte
    }
    this.accion.emit(accion);
    this.modoCambia.emit(destino);
  }
}
