import { CommonModule } from '@angular/common';
import { Component, Inject, inject, OnInit, ViewChild, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { HttpClient, HttpParams } from '@angular/common/http';
import { MatButtonModule } from '@angular/material/button';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatPaginator, MatPaginatorModule } from '@angular/material/paginator';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatTableDataSource, MatTableModule } from '@angular/material/table';
import { MatTooltipModule } from '@angular/material/tooltip';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
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

/**
 * La certificación, que en PrimeNG era un `p-dialog` declarativo con
 * `[(visible)]` dentro de la misma plantilla.
 *
 * `MatDialog` es imperativo: el contenido es un componente y se abre con una
 * llamada. Eso obliga a partirlo, y el reparto sale bien — el diálogo solo
 * pinta un `Detalle` que ya llegó, así que no inyecta nada, no pide nada y no
 * conoce la pantalla que lo abrió.
 */
@Component({
  selector: 'app-incapacidad-certificacion',
  standalone: true,
  imports: [MatDialogModule, MatButtonModule],
  template: `
    <h2 mat-dialog-title>Certificación de incapacidad</h2>
    <mat-dialog-content>
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
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cerrar</button>
    </mat-dialog-actions>
  `,
  styles: [`
    .detalle { display:grid; grid-template-columns:auto 1fr; gap:.4rem 1rem; margin:0; }
    .detalle dt { font-weight:600; color:var(--text-color-secondary); font-size:.85rem; }
    .detalle dd { margin:0; }
  `],
})
export class IncapacidadCertificacionComponent {
  constructor(@Inject(MAT_DIALOG_DATA) public d: Detalle) {}
}

@Component({
  selector: 'app-incapacidades',
  standalone: true,
  imports: [
    CommonModule, FormsModule, MatTableModule, MatPaginatorModule,
    MatButtonModule, MatFormFieldModule, MatInputModule, MatTooltipModule,
    MatProgressBarModule, EtiquetaComponent, ZoomDirective,
  ],
  template: `
    <div class="page-card">
      <div class="cabecera">
        <h2 class="page-title">Incapacidades médicas</h2>
        <button matButton (click)="cargar()">
          <i class="pi pi-refresh"></i> Actualizar
        </button>
      </div>

      <div class="filtros">
        <mat-form-field>
          <mat-label>Desde</mat-label>
          <input matInput type="date" [(ngModel)]="desde" (change)="cargar()" />
        </mat-form-field>
        <mat-form-field>
          <mat-label>Hasta</mat-label>
          <input matInput type="date" [(ngModel)]="hasta" (change)="cargar()" />
        </mat-form-field>
        <mat-form-field class="crece">
          <mat-label>Paciente</mat-label>
          <input matInput placeholder="Nombre, apellido o documento"
            [(ngModel)]="buscar" (ngModelChange)="alBuscar()" />
        </mat-form-field>
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

      @if (cargando()) { <mat-progress-bar mode="indeterminate" /> }

      <table mat-table [dataSource]="datos" class="mat-elevation-z0">
        <ng-container matColumnDef="historia">
          <th mat-header-cell *matHeaderCellDef>Historia</th>
          <td mat-cell *matCellDef="let i">{{ i.codigo_historia }}</td>
        </ng-container>
        <ng-container matColumnDef="paciente">
          <th mat-header-cell *matHeaderCellDef>Paciente</th>
          <td mat-cell *matCellDef="let i" appZoom>{{ i.paciente }}</td>
        </ng-container>
        <ng-container matColumnDef="documento">
          <th mat-header-cell *matHeaderCellDef>Documento</th>
          <td mat-cell *matCellDef="let i" appZoom>{{ i.identificacion }}</td>
        </ng-container>
        <ng-container matColumnDef="diagnostico">
          <th mat-header-cell *matHeaderCellDef>Diagnóstico</th>
          <td mat-cell *matCellDef="let i" appZoom>{{ i.diagnostico }}</td>
        </ng-container>
        <ng-container matColumnDef="desde">
          <th mat-header-cell *matHeaderCellDef>Desde</th>
          <td mat-cell *matCellDef="let i">{{ i.fecha_inicio ?? '—' }}</td>
        </ng-container>
        <ng-container matColumnDef="hasta">
          <th mat-header-cell *matHeaderCellDef>Hasta</th>
          <td mat-cell *matCellDef="let i">{{ i.fecha_fin ?? '—' }}</td>
        </ng-container>
        <ng-container matColumnDef="dias">
          <th mat-header-cell *matHeaderCellDef>Días</th>
          <td mat-cell *matCellDef="let i"><x-etiqueta [value]="i.dias" severity="info" /></td>
        </ng-container>
        <ng-container matColumnDef="medico">
          <th mat-header-cell *matHeaderCellDef>Médico</th>
          <td mat-cell *matCellDef="let i">{{ i.medico }}</td>
        </ng-container>
        <ng-container matColumnDef="acciones">
          <th mat-header-cell *matHeaderCellDef></th>
          <td mat-cell *matCellDef="let i">
            <button matIconButton matTooltip="Certificación" (click)="verDetalle(i)"
              aria-label="Ver certificación">
              <i class="pi pi-file"></i>
            </button>
          </td>
        </ng-container>

        <tr mat-header-row *matHeaderRowDef="columnas"></tr>
        <tr mat-row *matRowDef="let fila; columns: columnas"></tr>
      </table>

      <!-- Fuera de la tabla: mat-table no hereda el pTemplate="emptymessage" de
           p-table, y una tabla sin filas y sin frase se lee como «cargando». -->
      @if (!cargando() && datos.data.length === 0) {
        <p class="vacio">No hay incapacidades en el periodo seleccionado.</p>
      }

      <!-- Paginación EN CLIENTE: /api/incapacidades devuelve el periodo entero,
           no una página. -->
      <mat-paginator [pageSize]="20" [pageSizeOptions]="[10, 20, 50]" />
    </div>
  `,
  styles: [`
    .cabecera { display:flex; justify-content:space-between; align-items:center; margin-bottom:1rem; }
    .filtros { display:flex; gap:.75rem; flex-wrap:wrap; margin-bottom:1rem; align-items:flex-start; }
    .filtros .crece { flex:1; min-width:14rem; }
    table { width:100%; }
    .resumen { display:flex; gap:2rem; padding:.85rem 1rem; margin-bottom:1rem;
               background:var(--surface-50); border-radius:.5rem; }
    .dato { display:flex; flex-direction:column; }
    .numero { font-size:1.6rem; font-weight:700; color:var(--primary-color); line-height:1; }
    .etiqueta { font-size:.75rem; color:var(--text-color-secondary); }
    .vacio { text-align:center; padding:2rem; color:var(--text-color-secondary); }
  `],
})
export class IncapacidadesComponent implements OnInit {
  private http = inject(HttpClient);
  private dialogo = inject(MatDialog);

  readonly columnas = [
    'historia', 'paciente', 'documento', 'diagnostico',
    'desde', 'hasta', 'dias', 'medico', 'acciones',
  ];
  readonly datos = new MatTableDataSource<Incapacidad>([]);

  // Por SETTER: el paginador vive junto a una tabla que se repinta, y con la
  // forma estática se engancharía una vez o ninguna, dejando la paginación
  // muerta sin que nada falle.
  @ViewChild(MatPaginator)
  set paginador(p: MatPaginator | undefined) { if (p) this.datos.paginator = p; }

  resumen = signal<Resumen | null>(null);
  cargando = signal(false);

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
    this.http.get<Incapacidad[]>('/api/incapacidades', { params }).subscribe({
      next: (r) => { this.datos.data = r; this.cargando.set(false); },
      error: () => { this.datos.data = []; this.cargando.set(false); },
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
      next: (d) => {
        this.dialogo.open(IncapacidadCertificacionComponent, {
          data: d,
          width: '38rem',
          maxWidth: '95vw',
        });
      },
    });
  }
}
