import { CommonModule } from '@angular/common';
import { Component, Inject, inject, OnInit, ViewChild, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { HttpClient, HttpParams } from '@angular/common/http';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
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
import { MensajeComponent } from '../../shared/ui/mensaje.component';
import { ZoomDirective } from '../../shared/formulario/zoom.directive';

/**
 * Consentimiento informado. Migrado de `modules/consentimiento.py` (NiceGUI).
 *
 * La pantalla NO decide si un consentimiento vale: lo pregunta. El servidor
 * devuelve `valido` y la lista de lo que falta, y aquí solo se muestra. La
 * regla —qué hace que un consentimiento sea informado— vive en un solo sitio,
 * porque es la que hay que poder defender ante un juez y no puede tener dos
 * versiones.
 */

interface Consentimiento {
  id: number;
  fecha_firma: string;
  tipo_procedimiento: string;
  descripcion_procedimiento: string;
  paciente: string | null;
  medico: string | null;
  paciente_comprende: boolean;
  acepta_procedimiento: boolean;
  revocado: boolean;
  fecha_revocacion: string | null;
  motivo_revocacion: string | null;
  valido: boolean;
  faltantes: string[];
  resumen: string;
}

/** El detalle, que era el primero de los dos `p-dialog` de la pantalla. */
@Component({
  selector: 'app-consentimiento-detalle',
  standalone: true,
  imports: [CommonModule, MatDialogModule, MatButtonModule, MensajeComponent],
  template: `
    <h2 mat-dialog-title>Consentimiento</h2>
    <mat-dialog-content>
      @if (!c.valido) {
        <x-mensaje [severity]="c.revocado ? 'error' : 'warn'" [text]="c.resumen" />
      }
      <dl class="detalle">
        <dt>Paciente</dt><dd>{{ c.paciente }}</dd>
        <dt>Procedimiento</dt><dd>{{ c.tipo_procedimiento }}</dd>
        <dt>Descripción</dt><dd>{{ c.descripcion_procedimiento }}</dd>
        <dt>Comprendió</dt><dd>{{ c.paciente_comprende ? 'Sí' : 'No' }}</dd>
        <dt>Aceptó</dt><dd>{{ c.acepta_procedimiento ? 'Sí' : 'No' }}</dd>
        @if (c.revocado) {
          <dt>Revocado el</dt><dd>{{ c.fecha_revocacion | date: 'yyyy-MM-dd HH:mm' }}</dd>
          <dt>Manifestó</dt><dd>{{ c.motivo_revocacion }}</dd>
        }
        <dt>Médico</dt><dd>{{ c.medico }}</dd>
      </dl>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cerrar</button>
    </mat-dialog-actions>
  `,
  styles: [`
    x-mensaje { display:block; margin-bottom:1rem; }
    .detalle { display:grid; grid-template-columns:auto 1fr; gap:.4rem 1rem; margin:0; }
    .detalle dt { font-weight:600; color:var(--text-color-secondary); font-size:.85rem; }
    .detalle dd { margin:0; }
  `],
})
export class ConsentimientoDetalleComponent {
  constructor(@Inject(MAT_DIALOG_DATA) public c: Consentimiento) {}
}

/**
 * La revocación. Es el otro patrón de diálogo: **devuelve un dato**.
 *
 * Con `p-dialog` el motivo se escribía en un campo de la pantalla anfitriona y
 * el diálogo era solo su envoltorio visible; aquí el texto vive dentro y sale
 * por `afterClosed()`. La diferencia importa: cerrar por Escape devuelve
 * `undefined` y no queda un motivo a medio escribir esperando al siguiente.
 */
@Component({
  selector: 'app-consentimiento-revocacion',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule,
    MatFormFieldModule, MatInputModule,
  ],
  template: `
    <h2 mat-dialog-title>Registrar revocación</h2>
    <mat-dialog-content>
      <p class="aviso">
        El paciente puede retirar su consentimiento en cualquier momento y sin
        dar razones. Escriba lo que manifestó, solo para que quede constancia.
      </p>
      <mat-form-field class="ancho">
        <mat-label>Lo que dijo el paciente</mat-label>
        <textarea matInput [(ngModel)]="motivo" rows="3"></textarea>
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" class="peligro" [disabled]="!motivo.trim()"
        [mat-dialog-close]="motivo.trim()">
        Registrar revocación
      </button>
    </mat-dialog-actions>
  `,
  styles: [`
    .aviso { font-size:.85rem; color:var(--text-color-secondary); margin:0 0 .75rem; }
    .ancho { width:100%; }
    /* El tema es M3: el input color de MatButton no pinta nada ahí. */
    .peligro:not(:disabled) { background:#dc2626; color:#fff; }
  `],
})
export class ConsentimientoRevocacionComponent {
  motivo = '';
}

@Component({
  selector: 'app-consentimiento',
  standalone: true,
  imports: [
    CommonModule, FormsModule, MatTableModule, MatPaginatorModule,
    MatButtonModule, MatCheckboxModule, MatTooltipModule, MatProgressBarModule,
    EtiquetaComponent, MensajeComponent, ZoomDirective,
  ],
  template: `
    <div class="page-card">
      <div class="cabecera">
        <h2 class="page-title">Consentimiento informado</h2>
        <mat-checkbox [(ngModel)]="soloValidos" (change)="cargar()">
          Solo los completos
        </mat-checkbox>
      </div>

      @if (error(); as e) { <x-mensaje severity="error" [text]="e" /> }

      @if (cargando()) { <mat-progress-bar mode="indeterminate" /> }

      <table mat-table [dataSource]="datos">
        <ng-container matColumnDef="fecha">
          <th mat-header-cell *matHeaderCellDef>Fecha</th>
          <td mat-cell *matCellDef="let c">{{ c.fecha_firma | date: 'yyyy-MM-dd' }}</td>
        </ng-container>
        <ng-container matColumnDef="paciente">
          <th mat-header-cell *matHeaderCellDef>Paciente</th>
          <td mat-cell *matCellDef="let c" appZoom>{{ c.paciente }}</td>
        </ng-container>
        <ng-container matColumnDef="procedimiento">
          <th mat-header-cell *matHeaderCellDef>Procedimiento</th>
          <td mat-cell *matCellDef="let c" appZoom>{{ c.tipo_procedimiento }}</td>
        </ng-container>
        <ng-container matColumnDef="estado">
          <th mat-header-cell *matHeaderCellDef>Estado</th>
          <td mat-cell *matCellDef="let c">
            @if (c.revocado) {
              <x-etiqueta value="Revocado" severity="danger" />
            } @else if (c.valido) {
              <x-etiqueta value="Completo" severity="success" />
            } @else {
              <x-etiqueta value="Incompleto" severity="warn" />
            }
          </td>
        </ng-container>
        <ng-container matColumnDef="medico">
          <th mat-header-cell *matHeaderCellDef>Médico</th>
          <td mat-cell *matCellDef="let c">{{ c.medico }}</td>
        </ng-container>
        <ng-container matColumnDef="acciones">
          <th mat-header-cell *matHeaderCellDef></th>
          <td mat-cell *matCellDef="let c" class="acciones">
            <button matIconButton matTooltip="Ver" (click)="ver(c)" aria-label="Ver">
              <i class="pi pi-eye"></i>
            </button>
            @if (!c.revocado) {
              <button matIconButton class="peligro" matTooltip="Registrar revocación"
                (click)="pedirRevocacion(c)" aria-label="Registrar revocación">
                <i class="pi pi-ban"></i>
              </button>
            }
          </td>
        </ng-container>

        <tr mat-header-row *matHeaderRowDef="columnas"></tr>
        <tr mat-row *matRowDef="let fila; columns: columnas"
          [class.revocado]="fila.revocado"></tr>
      </table>

      @if (!cargando() && datos.data.length === 0) {
        <p class="vacio">No hay consentimientos registrados.</p>
      }

      <!-- Paginación EN CLIENTE: el endpoint devuelve la lista entera. -->
      <mat-paginator [pageSize]="20" [pageSizeOptions]="[10, 20, 50]" />
    </div>
  `,
  styles: [`
    .cabecera { display:flex; justify-content:space-between; align-items:center; margin-bottom:1rem; }
    table { width:100%; }
    tr.revocado { opacity:.65; }
    .acciones { white-space:nowrap; }
    .peligro { color:#dc2626; }
    .vacio { text-align:center; padding:2rem; color:var(--text-color-secondary); }
  `],
})
export class ConsentimientoComponent implements OnInit {
  private http = inject(HttpClient);
  private dialogo = inject(MatDialog);

  readonly columnas = ['fecha', 'paciente', 'procedimiento', 'estado', 'medico', 'acciones'];
  readonly datos = new MatTableDataSource<Consentimiento>([]);

  @ViewChild(MatPaginator)
  set paginador(p: MatPaginator | undefined) { if (p) this.datos.paginator = p; }

  cargando = signal(false);
  error = signal<string | null>(null);

  soloValidos = false;

  ngOnInit(): void { this.cargar(); }

  cargar(): void {
    this.cargando.set(true);
    let params = new HttpParams();
    if (this.soloValidos) params = params.set('solo_validos', 'true');
    this.http.get<Consentimiento[]>('/api/consentimiento/', { params }).subscribe({
      next: (r) => { this.datos.data = r; this.cargando.set(false); },
      error: () => { this.datos.data = []; this.cargando.set(false); },
    });
  }

  ver(c: Consentimiento): void {
    this.dialogo.open(ConsentimientoDetalleComponent, {
      data: c, width: '38rem', maxWidth: '95vw',
    });
  }

  pedirRevocacion(c: Consentimiento): void {
    this.dialogo
      .open(ConsentimientoRevocacionComponent, { width: '32rem', maxWidth: '95vw' })
      .afterClosed()
      // Solo un motivo escrito revoca. Cerrar con Escape o con Cancelar
      // devuelve `undefined`, y eso no es una revocación a medias: es ninguna.
      .subscribe((motivo?: string) => { if (motivo) this.revocar(c, motivo); });
  }

  private revocar(c: Consentimiento, motivo: string): void {
    this.http.post<Consentimiento>(
      `/api/consentimiento/${c.id}/revocar`,
      { motivo },
    ).subscribe({
      next: () => this.cargar(),
      error: (e) => this.error.set(e?.error?.detail ?? 'No se pudo registrar la revocación.'),
    });
  }
}
