import { CommonModule } from '@angular/common';
import { Component, inject, OnInit, ViewChild, signal, ChangeDetectionStrategy } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { HttpClient, HttpParams } from '@angular/common/http';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatPaginator, MatPaginatorModule } from '@angular/material/paginator';
import { MatTableDataSource, MatTableModule } from '@angular/material/table';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MensajeComponent } from '../../shared/ui/mensaje.component';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { ZoomDirective } from '../../shared/formulario/zoom.directive';

/**
 * Contrarreferencia. Migrado de `modules/contrarreferencia.py` (NiceGUI).
 *
 * Muestra lo que el servidor dice que pasó con la remisión al registrarla. En
 * el original no pasaba nada —la remisión seguía pendiente— y el usuario no
 * tenía forma de saberlo.
 */

interface Contrarreferencia {
  id: number;
  fecha_creacion: string;
  paciente: string | null;
  diagnostico_final: string;
  recomendaciones: string | null;
  requiere_seguimiento: boolean;
  fecha_control_sugerida: string | null;
  recibida: boolean;
  fecha_recepcion: string | null;
  cierre: string | null;
}

@Component({
  selector: 'app-contrarreferencia',
  standalone: true,
  imports: [CommonModule, FormsModule, MatTableModule, MatPaginatorModule, MatButtonModule,
            MatCheckboxModule, MatTooltipModule, EtiquetaComponent, MensajeComponent,
            ZoomDirective],
  template: `
    <div class="page-card">
      <div class="cabecera">
        <h2 class="page-title">Contrarreferencias</h2>
        <mat-checkbox [(ngModel)]="soloPendientes" (change)="cargar()">
          Solo sin acusar recibo
        </mat-checkbox>
      </div>

      @if (aviso(); as a) { <x-mensaje severity="info" [text]="a" /> }
      @if (error(); as e) { <x-mensaje severity="error" [text]="e" /> }

      <table mat-table [dataSource]="datos" class="tabla">
        <ng-container matColumnDef="fecha">
          <th mat-header-cell *matHeaderCellDef>Fecha</th>
          <td mat-cell *matCellDef="let c">{{ c.fecha_creacion | date: 'yyyy-MM-dd' }}</td>
        </ng-container>
        <ng-container matColumnDef="paciente">
          <th mat-header-cell *matHeaderCellDef>Paciente</th>
          <td mat-cell *matCellDef="let c" appZoom>{{ c.paciente }}</td>
        </ng-container>
        <ng-container matColumnDef="diagnostico">
          <th mat-header-cell *matHeaderCellDef>Diagnóstico final</th>
          <td mat-cell *matCellDef="let c" appZoom>{{ c.diagnostico_final }}</td>
        </ng-container>
        <ng-container matColumnDef="seguimiento">
          <th mat-header-cell *matHeaderCellDef>Seguimiento</th>
          <td mat-cell *matCellDef="let c">
            @if (c.requiere_seguimiento) {
              <x-etiqueta [value]="c.fecha_control_sugerida || 'Sí'" severity="warn" />
            } @else { <span class="tenue">—</span> }
          </td>
        </ng-container>
        <ng-container matColumnDef="recibida">
          <th mat-header-cell *matHeaderCellDef>Recibida</th>
          <td mat-cell *matCellDef="let c">
            @if (c.recibida) {
              <x-etiqueta [value]="(c.fecha_recepcion | date: 'yyyy-MM-dd') ?? 'Recibida'" severity="success" />
            } @else { <x-etiqueta value="Pendiente" severity="warn" /> }
          </td>
        </ng-container>
        <ng-container matColumnDef="acciones">
          <th mat-header-cell *matHeaderCellDef></th>
          <td mat-cell *matCellDef="let c">
            @if (!c.recibida) {
              <button matIconButton matTooltip="Acusar recibo" (click)="recibir(c)">
                <i class="pi pi-check"></i>
              </button>
            }
          </td>
        </ng-container>

        <tr mat-header-row *matHeaderRowDef="columnas"></tr>
        <tr mat-row *matRowDef="let fila; columns: columnas"></tr>
      </table>

      <!-- El «no hay nada» va FUERA de la tabla: mat-table no tiene plantilla de
           vacío como la tenía p-table, y una tabla sin filas y sin frase se lee
           como «todavía está cargando». -->
      @if (!cargando() && datos.data.length === 0) {
        <p class="vacio">No hay contrarreferencias.</p>
      }
      <mat-paginator [pageSize]="20" [pageSizeOptions]="[10, 20, 50]" />
    </div>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .cabecera { display:flex; justify-content:space-between; align-items:center; margin-bottom:1rem; }
    .tabla { width: 100%; }
    .tenue { color:#64748b; }
    .vacio { text-align:center; padding:2rem; color:#64748b; }
    x-mensaje { display:block; margin-bottom:1rem; }
  `],
})
export class ContrarreferenciaComponent implements OnInit {
  private http = inject(HttpClient);

  /** El orden de las columnas de la tabla lo fija esta lista, no la plantilla. */
  readonly columnas = ['fecha', 'paciente', 'diagnostico', 'seguimiento', 'recibida', 'acciones'];
  readonly datos = new MatTableDataSource<Contrarreferencia>([]);

  /**
   * El paginador llega por `setter` y no por `@ViewChild` a secas: la tabla vive
   * dentro de un `@if` en varias pantallas, así que el elemento aparece y
   * desaparece. Con la forma estática se engancharía una vez —o ninguna— y la
   * paginación quedaría muerta sin que nada fallara.
   */
  @ViewChild(MatPaginator)
  set paginador(p: MatPaginator | undefined) {
    if (p) this.datos.paginator = p;
  }

  cargando = signal(false);
  aviso = signal<string | null>(null);
  error = signal<string | null>(null);
  soloPendientes = false;

  ngOnInit(): void { this.cargar(); }

  cargar(): void {
    this.cargando.set(true);
    let params = new HttpParams();
    if (this.soloPendientes) params = params.set('solo_pendientes', 'true');
    this.http.get<Contrarreferencia[]>('/api/contrarreferencia', { params }).subscribe({
      next: (r) => { this.datos.data = r; this.cargando.set(false); },
      error: () => { this.datos.data = []; this.cargando.set(false); },
    });
  }

  recibir(c: Contrarreferencia): void {
    this.error.set(null);
    this.http.post(`/api/contrarreferencia/${c.id}/recibir`, {}).subscribe({
      next: () => { this.aviso.set('Acuse de recibo registrado.'); this.cargar(); },
      error: (e) => this.error.set(e?.error?.detail ?? 'No se pudo registrar el acuse.'),
    });
  }
}
