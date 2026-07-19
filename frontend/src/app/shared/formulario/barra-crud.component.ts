import { CommonModule } from '@angular/common';
import { Component, computed, EventEmitter, Input, Output, signal } from '@angular/core';
import { ButtonModule } from 'primeng/button';
import { TooltipModule } from 'primeng/tooltip';
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
  imports: [CommonModule, ButtonModule, TooltipModule],
  template: `
    <div class="barra-crud">
      <p-button
        icon="pi pi-plus" severity="secondary" size="small"
        [disabled]="!barra().nuevo" (onClick)="ejecutar('nuevo')"
        pTooltip="Nuevo" tooltipPosition="bottom" />
      <p-button
        icon="pi pi-pencil" severity="secondary" size="small"
        [disabled]="!barra().editar" (onClick)="ejecutar('editar')"
        pTooltip="Editar" tooltipPosition="bottom" />
      <p-button
        icon="pi pi-trash" severity="danger" size="small"
        [disabled]="!barra().eliminar" (onClick)="ejecutar('eliminar')"
        pTooltip="Eliminar" tooltipPosition="bottom" />

      <span class="separador"></span>

      <p-button
        icon="pi pi-check" severity="success" size="small"
        [disabled]="!barra().guardar" (onClick)="ejecutar('guardar')"
        pTooltip="Guardar (Ctrl+Enter)" tooltipPosition="bottom" />
      <p-button
        icon="pi pi-undo" severity="secondary" size="small"
        [disabled]="!barra().deshacer" (onClick)="ejecutar('deshacer')"
        pTooltip="Deshacer (Esc)" tooltipPosition="bottom" />

      @if (mostrarNavegacion) {
        <span class="separador"></span>
        <p-button icon="pi pi-angle-double-left" severity="secondary" size="small"
          [disabled]="!barra().navegar" (onClick)="navegar.emit('primero')"
          pTooltip="Primero" tooltipPosition="bottom" />
        <p-button icon="pi pi-angle-left" severity="secondary" size="small"
          [disabled]="!barra().navegar" (onClick)="navegar.emit('anterior')"
          pTooltip="Anterior" tooltipPosition="bottom" />
        <p-button icon="pi pi-angle-right" severity="secondary" size="small"
          [disabled]="!barra().navegar" (onClick)="navegar.emit('siguiente')"
          pTooltip="Siguiente" tooltipPosition="bottom" />
        <p-button icon="pi pi-angle-double-right" severity="secondary" size="small"
          [disabled]="!barra().navegar" (onClick)="navegar.emit('ultimo')"
          pTooltip="Último" tooltipPosition="bottom" />
      }

      @if (modo !== 'consulta') {
        <span class="aviso-edicion">
          {{ modo === 'nuevo' ? 'Registro nuevo sin guardar' : 'Editando' }}
        </span>
      }
    </div>
  `,
  styles: [`
    .barra-crud { display: flex; align-items: center; gap: .35rem; flex-wrap: wrap; }
    .separador { width: 1px; height: 1.5rem; background: var(--surface-300); margin: 0 .4rem; }
    .aviso-edicion { margin-left: auto; font-size: .8rem; color: var(--primary-color); font-weight: 600; }
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
