import { CommonModule } from '@angular/common';
import { Component, inject, OnInit, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { HttpClient, HttpParams } from '@angular/common/http';
import { ButtonModule } from 'primeng/button';
import { DialogModule } from 'primeng/dialog';
import { InputTextModule } from 'primeng/inputtext';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { ZoomDirective } from '../../shared/formulario/zoom.directive';

/**
 * Incapacidades médicas. Migrado de `incapacidad.scx` (FoxPro) y de
 * `modules/incapacidad.py` (NiceGUI).
 *
 * La fecha de fin la calcula el servidor, no esta pantalla: es el dato del que
 * depende cuándo se reincorpora el trabajador y hasta cuándo reconoce la EPS, y
 * no puede haber dos versiones del cálculo. Las dos implementaciones anteriores
 * lo hacían en la interfaz y las dos tenían el mismo día de más.
 */

export interface Incapacidad {
  historia_id: number;
  codigo_historia: string;
  fecha_consulta: string;
  paciente: string;
  identificacion: string;
  dias: number;
  fecha_inicio: string | null;
  fecha_fin: string | null;
  diagnostico: string;
  medico: string;
}

interface Detalle extends Incapacidad {
  motivo_consulta?: string;
  diagnostico_relacionado?: string;
  observaciones?: string;
}

interface Resumen {
  cantidad: number;
  dias_totales: number;
  promedio_dias: number;
}

@Component({
  selector: 'app-incapacidades',
  standalone: true,
  imports: [
    CommonModule, FormsModule, TableModule, ButtonModule, InputTextModule,
    TagModule, DialogModule, ZoomDirective,
  ],
  template: `
    <div class="page-card">
      <div class="cabecera">
        <h2 class="page-title">Incapacidades médicas</h2>
        <p-button icon="pi pi-refresh" label="Actualizar" size="small"
          [text]="true" (onClick)="cargar()" />
      </div>

      <div class="filtros">
        <label>
          <span>Desde</span>
          <input pInputText type="date" [(ngModel)]="desde" (change)="cargar()" />
        </label>
        <label>
          <span>Hasta</span>
          <input pInputText type="date" [(ngModel)]="hasta" (change)="cargar()" />
        </label>
        <label class="crece">
          <span>Paciente</span>
          <input pInputText placeholder="Nombre, apellido o documento"
            [(ngModel)]="buscar" (ngModelChange)="alBuscar()" />
        </label>
      </div>

      @if (resumen(); as r) {
        <div class="resumen">
          <div class="dato">
            <span class="numero">{{ r.cantidad }}</span>
            <span class="etiqueta">incapacidades</span>
          </div>
          <div class="dato">
            <span class="numero">{{ r.dias_totales }}</span>
            <span class="etiqueta">días en total</span>
          </div>
          <div class="dato">
            <span class="numero">{{ r.promedio_dias }}</span>
            <span class="etiqueta">días de promedio</span>
          </div>
        </div>
      }

      <p-table [value]="filas()" [loading]="cargando()" [paginator]="true"
        [rows]="20" dataKey="historia_id" styleClass="p-datatable-sm">
        <ng-template pTemplate="header">
          <tr>
            <th>Historia</th>
            <th>Paciente</th>
            <th>Documento</th>
            <th>Diagnóstico</th>
            <th>Desde</th>
            <th>Hasta</th>
            <th>Días</th>
            <th>Médico</th>
            <th></th>
          </tr>
        </ng-template>
        <ng-template pTemplate="body" let-i>
          <tr>
            <td>{{ i.codigo_historia }}</td>
            <td appZoom>{{ i.paciente }}</td>
            <td appZoom>{{ i.identificacion }}</td>
            <td appZoom>{{ i.diagnostico }}</td>
            <td>{{ i.fecha_inicio ?? '—' }}</td>
            <td>{{ i.fecha_fin ?? '—' }}</td>
            <td><p-tag [value]="i.dias" severity="info" /></td>
            <td>{{ i.medico }}</td>
            <td>
              <p-button icon="pi pi-file" [text]="true" size="small"
                pTooltip="Certificación" (onClick)="verDetalle(i)" />
            </td>
          </tr>
        </ng-template>
        <ng-template pTemplate="emptymessage">
          <tr><td colspan="9" class="vacio">
            No hay incapacidades en el periodo seleccionado.
          </td></tr>
        </ng-template>
      </p-table>
    </div>

    <p-dialog [(visible)]="detalleAbierto" [modal]="true" header="Certificación de incapacidad"
      [style]="{ width: '38rem', maxWidth: '95vw' }">
      @if (detalle(); as d) {
        <dl class="detalle">
          <dt>Paciente</dt><dd>{{ d.paciente }} — {{ d.identificacion }}</dd>
          <dt>Historia</dt><dd>{{ d.codigo_historia }}</dd>
          <dt>Diagnóstico</dt><dd>{{ d.diagnostico }}</dd>
          @if (d.diagnostico_relacionado) {
            <dt>Relacionado</dt><dd>{{ d.diagnostico_relacionado }}</dd>
          }
          <dt>Periodo</dt>
          <dd><strong>{{ d.fecha_inicio }} a {{ d.fecha_fin }}</strong> ({{ d.dias }} días)</dd>
          <dt>Motivo</dt><dd>{{ d.motivo_consulta || '—' }}</dd>
          <dt>Indicaciones</dt><dd>{{ d.observaciones || '—' }}</dd>
          <dt>Médico</dt><dd>{{ d.medico }}</dd>
        </dl>
      }
    </p-dialog>
  `,
  styles: [`
    .cabecera { display:flex; justify-content:space-between; align-items:center; margin-bottom:1rem; }
    .filtros { display:flex; gap:.75rem; flex-wrap:wrap; margin-bottom:1rem; align-items:flex-end; }
    .filtros label { display:flex; flex-direction:column; gap:.25rem; }
    .filtros label.crece { flex:1; min-width:14rem; }
    .filtros span { font-size:.75rem; font-weight:600; color:var(--text-color-secondary); }
    .resumen { display:flex; gap:2rem; padding:.85rem 1rem; margin-bottom:1rem;
               background:var(--surface-50); border-radius:.5rem; }
    .dato { display:flex; flex-direction:column; }
    .numero { font-size:1.6rem; font-weight:700; color:var(--primary-color); line-height:1; }
    .etiqueta { font-size:.75rem; color:var(--text-color-secondary); }
    .vacio { text-align:center; padding:2rem; color:var(--text-color-secondary); }
    .detalle { display:grid; grid-template-columns:auto 1fr; gap:.4rem 1rem; margin:0; }
    .detalle dt { font-weight:600; color:var(--text-color-secondary); font-size:.85rem; }
    .detalle dd { margin:0; }
  `],
})
export class IncapacidadesComponent implements OnInit {
  private http = inject(HttpClient);

  filas = signal<Incapacidad[]>([]);
  resumen = signal<Resumen | null>(null);
  detalle = signal<Detalle | null>(null);
  cargando = signal(false);
  detalleAbierto = false;

  desde = '';
  hasta = '';
  buscar = '';
  private temporizador: ReturnType<typeof setTimeout> | null = null;

  ngOnInit(): void {
    this.cargar();
  }

  private parametros(): HttpParams {
    let p = new HttpParams();
    if (this.desde) p = p.set('desde', this.desde);
    if (this.hasta) p = p.set('hasta', this.hasta);
    if (this.buscar.trim()) p = p.set('buscar', this.buscar.trim());
    return p;
  }

  cargar(): void {
    this.cargando.set(true);
    const params = this.parametros();
    this.http.get<Incapacidad[]>('/api/incapacidades/', { params }).subscribe({
      next: (r) => { this.filas.set(r); this.cargando.set(false); },
      error: () => { this.filas.set([]); this.cargando.set(false); },
    });
    // El resumen se pide con LOS MISMOS filtros. En el original salía de otra
    // consulta y el número de arriba no cuadraba con las filas de abajo.
    this.http.get<Resumen>('/api/incapacidades/resumen', { params })
      .subscribe({ next: (r) => this.resumen.set(r), error: () => this.resumen.set(null) });
  }

  alBuscar(): void {
    if (this.temporizador) clearTimeout(this.temporizador);
    this.temporizador = setTimeout(() => this.cargar(), 400);
  }

  verDetalle(i: Incapacidad): void {
    this.http.get<Detalle>(`/api/incapacidades/${i.historia_id}`).subscribe({
      next: (d) => { this.detalle.set(d); this.detalleAbierto = true; },
    });
  }
}
