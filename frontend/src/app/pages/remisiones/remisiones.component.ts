import { CommonModule } from '@angular/common';
import { Component, inject, OnInit, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { HttpClient, HttpParams } from '@angular/common/http';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { SelectModule } from 'primeng/select';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { MessageModule } from 'primeng/message';
import { ZoomDirective } from '../../shared/formulario/zoom.directive';

/**
 * Remisiones a especialista. Migrado de `reemision.scx` (FoxPro) y
 * `modules/remisiones.py` (NiceGUI).
 *
 * La pantalla NO decide qué estados son posibles: los recibe del servidor en
 * `estados_posibles`. En el original la lista de opciones estaba escrita en la
 * interfaz y el servidor guardaba lo que llegara, así que ofrecía cambios que
 * no tenían sentido y los aceptaba. Teniendo la regla en un solo sitio, lo que
 * se ofrece y lo que se acepta no pueden discrepar.
 */

interface Remision {
  id: number;
  fecha_orden: string;
  estado: string;
  paciente: string;
  identificacion: string;
  descripcion: string;
  justificacion: string;
  especialidad_destino: string | null;
  institucion_destino: string | null;
  medico_ordena: string | null;
  estados_posibles: string[];
  abierta: boolean;
}

interface Resumen {
  total: number;
  abiertas: number;
  por_estado: Record<string, number>;
}

@Component({
  selector: 'app-remisiones',
  standalone: true,
  imports: [
    CommonModule, FormsModule, TableModule, ButtonModule, InputTextModule,
    SelectModule, TagModule, MessageModule, ZoomDirective,
  ],
  template: `
    <div class="page-card">
      <div class="cabecera">
        <h2 class="page-title">Remisiones a especialista</h2>
        <p-button icon="pi pi-refresh" label="Actualizar" size="small"
          [text]="true" (onClick)="cargar()" />
      </div>

      <div class="filtros">
        <label>
          <span>Estado</span>
          <p-select [options]="opcionesEstado" optionLabel="etiqueta" optionValue="valor"
            [(ngModel)]="estado" (ngModelChange)="cargar()" [showClear]="true"
            placeholder="Todos" appendTo="body" />
        </label>
        <label><span>Desde</span>
          <input pInputText type="date" [(ngModel)]="desde" (change)="cargar()" /></label>
        <label><span>Hasta</span>
          <input pInputText type="date" [(ngModel)]="hasta" (change)="cargar()" /></label>
        <label class="crece"><span>Paciente</span>
          <input pInputText placeholder="Nombre, apellido o documento"
            [(ngModel)]="buscar" (ngModelChange)="alBuscar()" /></label>
      </div>

      @if (resumen(); as r) {
        <div class="resumen">
          <div class="dato"><span class="numero">{{ r.abiertas }}</span>
            <span class="etiqueta">abiertas</span></div>
          <div class="dato"><span class="numero">{{ r.total }}</span>
            <span class="etiqueta">en el periodo</span></div>
          @for (e of r.por_estado | keyvalue; track e.key) {
            @if (e.value) {
              <div class="dato"><span class="numero pequeno">{{ e.value }}</span>
                <span class="etiqueta">{{ e.key }}</span></div>
            }
          }
        </div>
      }

      @if (error(); as e) { <p-message severity="warn" [text]="e" styleClass="w-full" /> }

      <p-table [value]="filas()" [loading]="cargando()" [paginator]="true"
        [rows]="20" dataKey="id" styleClass="p-datatable-sm">
        <ng-template pTemplate="header">
          <tr>
            <th>Fecha</th><th>Paciente</th><th>Documento</th>
            <th>Remitido a</th><th>Justificación</th><th>Estado</th>
            <th>Cambiar a</th>
          </tr>
        </ng-template>
        <ng-template pTemplate="body" let-r>
          <tr>
            <td>{{ r.fecha_orden | date: 'yyyy-MM-dd' }}</td>
            <td appZoom>{{ r.paciente }}</td>
            <td appZoom>{{ r.identificacion }}</td>
            <td appZoom>{{ r.especialidad_destino || r.descripcion }}</td>
            <td appZoom>{{ r.justificacion }}</td>
            <td><p-tag [value]="r.estado" [severity]="colorDe(r.estado)" /></td>
            <td>
              @if (r.estados_posibles.length) {
                <p-select [options]="r.estados_posibles" placeholder="—"
                  appendTo="body" styleClass="cambio"
                  (onChange)="cambiar(r, $event.value)" />
              } @else {
                <span class="cerrada">cerrada</span>
              }
            </td>
          </tr>
        </ng-template>
        <ng-template pTemplate="emptymessage">
          <tr><td colspan="7" class="vacio">
            No hay remisiones con esos filtros.
          </td></tr>
        </ng-template>
      </p-table>
    </div>
  `,
  styles: [`
    .cabecera { display:flex; justify-content:space-between; align-items:center; margin-bottom:1rem; }
    .filtros { display:flex; gap:.75rem; flex-wrap:wrap; margin-bottom:1rem; align-items:flex-end; }
    .filtros label { display:flex; flex-direction:column; gap:.25rem; }
    .filtros label.crece { flex:1; min-width:14rem; }
    .filtros span { font-size:.75rem; font-weight:600; color:var(--text-color-secondary); }
    .resumen { display:flex; gap:2rem; padding:.85rem 1rem; margin-bottom:1rem;
               background:var(--surface-50); border-radius:.5rem; flex-wrap:wrap; }
    .dato { display:flex; flex-direction:column; }
    .numero { font-size:1.6rem; font-weight:700; color:var(--primary-color); line-height:1; }
    .numero.pequeno { font-size:1.1rem; color:var(--text-color); }
    .etiqueta { font-size:.75rem; color:var(--text-color-secondary); }
    .cerrada { font-size:.8rem; color:var(--text-color-secondary); font-style:italic; }
    .vacio { text-align:center; padding:2rem; color:var(--text-color-secondary); }
  `],
})
export class RemisionesComponent implements OnInit {
  private http = inject(HttpClient);

  filas = signal<Remision[]>([]);
  resumen = signal<Resumen | null>(null);
  cargando = signal(false);
  error = signal<string | null>(null);

  readonly opcionesEstado = [
    { valor: 'Pendiente', etiqueta: 'Pendiente' },
    { valor: 'En Proceso', etiqueta: 'En proceso' },
    { valor: 'Completada', etiqueta: 'Completada' },
    { valor: 'Cancelada', etiqueta: 'Cancelada' },
  ];

  estado: string | null = null;
  desde = '';
  hasta = '';
  buscar = '';
  private temporizador: ReturnType<typeof setTimeout> | null = null;

  ngOnInit(): void { this.cargar(); }

  colorDe(estado: string): 'success' | 'info' | 'warn' | 'danger' {
    if (estado === 'Completada') return 'success';
    if (estado === 'En Proceso') return 'info';
    if (estado === 'Cancelada') return 'danger';
    return 'warn';
  }

  private parametros(): HttpParams {
    let p = new HttpParams();
    if (this.estado) p = p.set('estado', this.estado);
    if (this.desde) p = p.set('desde', this.desde);
    if (this.hasta) p = p.set('hasta', this.hasta);
    if (this.buscar.trim()) p = p.set('buscar', this.buscar.trim());
    return p;
  }

  cargar(): void {
    this.cargando.set(true);
    this.error.set(null);
    const params = this.parametros();
    this.http.get<Remision[]>('/api/remisiones', { params }).subscribe({
      next: (r) => { this.filas.set(r); this.cargando.set(false); },
      error: () => { this.filas.set([]); this.cargando.set(false); },
    });
    this.http.get<Resumen>('/api/remisiones/resumen', { params }).subscribe({
      next: (r) => this.resumen.set(r), error: () => this.resumen.set(null),
    });
  }

  alBuscar(): void {
    if (this.temporizador) clearTimeout(this.temporizador);
    this.temporizador = setTimeout(() => this.cargar(), 400);
  }

  cambiar(r: Remision, estado: string): void {
    if (!estado) return;
    this.http.put<Remision>(`/api/remisiones/${r.id}/estado`, { estado }).subscribe({
      next: () => this.cargar(),
      // El servidor manda 409 con el motivo. Se muestra tal cual: explica por
      // qué no se puede mejor de lo que lo haría un mensaje genérico.
      error: (e) => this.error.set(e?.error?.detail ?? 'No se pudo cambiar el estado.'),
    });
  }
}
