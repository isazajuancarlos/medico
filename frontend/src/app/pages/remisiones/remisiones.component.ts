import { CommonModule } from '@angular/common';
import { Component, inject, OnInit, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { HttpClient, HttpParams } from '@angular/common/http';
import { ViewChild } from '@angular/core';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatPaginator, MatPaginatorModule } from '@angular/material/paginator';
import { MatTableDataSource, MatTableModule } from '@angular/material/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { MensajeComponent } from '../../shared/ui/mensaje.component';
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
    CommonModule, FormsModule, MatTableModule, MatPaginatorModule, MatButtonModule,
    MatFormFieldModule, MatInputModule, MatSelectModule, EtiquetaComponent,
    MensajeComponent, ZoomDirective,
  ],
  template: `
    <div class="page-card">
      <div class="cabecera">
        <h2 class="page-title">Remisiones a especialista</h2>
        <button matButton (click)="cargar()">
          <i class="pi pi-refresh"></i> Actualizar
        </button>
      </div>

      <div class="filtros">
        <mat-form-field appearance="outline" subscriptSizing="dynamic">
          <mat-label>Estado</mat-label>
          <mat-select [(ngModel)]="estado" (ngModelChange)="cargar()">
            <!-- La opción vacía hace de [showClear] de PrimeNG: sin ella no hay
                 forma de volver a «todos» una vez elegido un estado. -->
            <mat-option [value]="null">Todos</mat-option>
            @for (o of opcionesEstado; track o.valor) {
              <mat-option [value]="o.valor">{{ o.etiqueta }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline" subscriptSizing="dynamic">
          <mat-label>Desde</mat-label>
          <input matInput type="date" [(ngModel)]="desde" (change)="cargar()" />
        </mat-form-field>
        <mat-form-field appearance="outline" subscriptSizing="dynamic">
          <mat-label>Hasta</mat-label>
          <input matInput type="date" [(ngModel)]="hasta" (change)="cargar()" />
        </mat-form-field>
        <mat-form-field appearance="outline" subscriptSizing="dynamic" class="crece">
          <mat-label>Paciente</mat-label>
          <input matInput placeholder="Nombre, apellido o documento"
                 [(ngModel)]="buscar" (ngModelChange)="alBuscar()" />
        </mat-form-field>
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

      @if (error(); as e) { <x-mensaje severity="warn" [text]="e" /> }

      <table mat-table [dataSource]="datos" class="tabla">
        <ng-container matColumnDef="fecha">
          <th mat-header-cell *matHeaderCellDef>Fecha</th>
          <td mat-cell *matCellDef="let r">{{ r.fecha_orden | date: 'yyyy-MM-dd' }}</td>
        </ng-container>
        <ng-container matColumnDef="paciente">
          <th mat-header-cell *matHeaderCellDef>Paciente</th>
          <td mat-cell *matCellDef="let r" appZoom>{{ r.paciente }}</td>
        </ng-container>
        <ng-container matColumnDef="documento">
          <th mat-header-cell *matHeaderCellDef>Documento</th>
          <td mat-cell *matCellDef="let r" appZoom>{{ r.identificacion }}</td>
        </ng-container>
        <ng-container matColumnDef="destino">
          <th mat-header-cell *matHeaderCellDef>Remitido a</th>
          <td mat-cell *matCellDef="let r" appZoom>
            {{ r.especialidad_destino || r.descripcion }}
          </td>
        </ng-container>
        <ng-container matColumnDef="justificacion">
          <th mat-header-cell *matHeaderCellDef>Justificación</th>
          <td mat-cell *matCellDef="let r" appZoom>{{ r.justificacion }}</td>
        </ng-container>
        <ng-container matColumnDef="estado">
          <th mat-header-cell *matHeaderCellDef>Estado</th>
          <td mat-cell *matCellDef="let r">
            <x-etiqueta [value]="r.estado" [severity]="colorDe(r.estado)" />
          </td>
        </ng-container>
        <ng-container matColumnDef="cambiar">
          <th mat-header-cell *matHeaderCellDef>Cambiar a</th>
          <td mat-cell *matCellDef="let r">
            <!-- Los estados que se ofrecen siguen viniendo del servidor, del
                 campo estados_posibles y no de una lista escrita aquí: es la
                 razón de ser de esta pantalla, y no cambia con la librería. -->
            @if (r.estados_posibles.length) {
              <mat-form-field appearance="outline" subscriptSizing="dynamic" class="cambio">
                <mat-select placeholder="—" (selectionChange)="cambiar(r, $event.value)">
                  @for (e of r.estados_posibles; track e) {
                    <mat-option [value]="e">{{ e }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
            } @else {
              <span class="cerrada">cerrada</span>
            }
          </td>
        </ng-container>

        <tr mat-header-row *matHeaderRowDef="columnas"></tr>
        <tr mat-row *matRowDef="let fila; columns: columnas"></tr>
      </table>

      @if (!cargando() && datos.data.length === 0) {
        <p class="vacio">No hay remisiones con esos filtros.</p>
      }
      <mat-paginator [pageSize]="20" [pageSizeOptions]="[10, 20, 50]" />
    </div>
  `,
  styles: [`
    .cabecera { display:flex; justify-content:space-between; align-items:center; margin-bottom:1rem; }
    .filtros { display:flex; gap:.75rem; flex-wrap:wrap; margin-bottom:1rem; align-items:flex-end; }
    .filtros .crece { flex:1; min-width:14rem; }
    .tabla { width:100%; }
    .cambio { width: 10rem; }
    x-mensaje { display:block; margin-bottom:1rem; }
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

  readonly columnas = ['fecha', 'paciente', 'documento', 'destino', 'justificacion',
                       'estado', 'cambiar'];
  readonly datos = new MatTableDataSource<Remision>([]);

  @ViewChild(MatPaginator)
  set paginador(p: MatPaginator | undefined) {
    if (p) this.datos.paginator = p;
  }

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
      next: (r) => { this.datos.data = r; this.cargando.set(false); },
      error: () => { this.datos.data = []; this.cargando.set(false); },
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
