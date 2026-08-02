import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatTableModule } from '@angular/material/table';
import { MatPaginatorModule, PageEvent } from '@angular/material/paginator';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatDialog } from '@angular/material/dialog';
import { Router } from '@angular/router';
import { PacienteFormComponent } from './paciente-form.component';
import { PacienteService } from '../../services/paciente.service';
import { Patient } from '../../models/patient.models';
import { PaginatedResponse } from '../../models/common.models';

@Component({
  selector: 'app-pacientes',
  standalone: true,
  imports: [CommonModule, FormsModule, MatTableModule, MatPaginatorModule, MatButtonModule,
            MatFormFieldModule, MatInputModule, MatTooltipModule],
  template: `
    <div class="page-card">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;">
        <h2 class="page-title" style="margin-bottom: 0">Pacientes</h2>
        <button matButton="filled" (click)="nuevo()">
          <i class="pi pi-plus"></i> Nuevo Paciente
        </button>
      </div>

      <mat-form-field appearance="outline" class="buscador">
        <mat-label>Buscar</mat-label>
        <input
          matInput
          [(ngModel)]="searchQuery"
          (ngModelChange)="onSearch($event)"
          placeholder="Nombre, apellido o identificación..."
        />
      </mat-form-field>

      <table mat-table [dataSource]="pacientes()" class="tabla">
        <ng-container matColumnDef="identificacion">
          <th mat-header-cell *matHeaderCellDef>Identificación</th>
          <td mat-cell *matCellDef="let p">{{ p.tipo_id }} {{ p.identificacion }}</td>
        </ng-container>
        <ng-container matColumnDef="nombre">
          <th mat-header-cell *matHeaderCellDef>Nombre Completo</th>
          <td mat-cell *matCellDef="let p">
            {{ p.nombre1 }} {{ p.nombre2 }} {{ p.apellido1 }} {{ p.apellido2 }}
          </td>
        </ng-container>
        <ng-container matColumnDef="nacimiento">
          <th mat-header-cell *matHeaderCellDef>Fecha Nacimiento</th>
          <td mat-cell *matCellDef="let p">{{ p.fecha_nacimiento }}</td>
        </ng-container>
        <ng-container matColumnDef="celular">
          <th mat-header-cell *matHeaderCellDef>Celular</th>
          <td mat-cell *matCellDef="let p">{{ p.celular }}</td>
        </ng-container>
        <ng-container matColumnDef="tipo">
          <th mat-header-cell *matHeaderCellDef>Tipo Usuario</th>
          <td mat-cell *matCellDef="let p">
            <span class="badge" [class]="getTipoClass(p.tipo_usuario)">
              {{ getTipoLabel(p.tipo_usuario) }}
            </span>
          </td>
        </ng-container>
        <ng-container matColumnDef="acciones">
          <th mat-header-cell *matHeaderCellDef>Acciones</th>
          <td mat-cell *matCellDef="let p">
            <button matIconButton matTooltip="Ver Historia" (click)="verHistoria(p)">
              <i class="pi pi-eye"></i>
            </button>
            <button matIconButton matTooltip="Editar" (click)="editar(p)">
              <i class="pi pi-pencil"></i>
            </button>
          </td>
        </ng-container>

        <tr mat-header-row *matHeaderRowDef="columnas"></tr>
        <tr mat-row *matRowDef="let fila; columns: columnas"></tr>
      </table>

      @if (!loading() && pacientes().length === 0) {
        <p class="vacio">No se encontraron pacientes</p>
      }

      <!-- Paginación DEL SERVIDOR, como antes: se le dice el total y él avisa de
           la página pedida. No se le da un MatTableDataSource, que pagina en
           cliente sobre lo que ya tiene — con miles de pacientes eso mostraría
           veinte y diría que no hay más. -->
      <mat-paginator
        [length]="total()"
        [pageSize]="pageSize"
        [pageSizeOptions]="[10, 20, 50]"
        (page)="alPaginar($event)"
      />
    </div>
  `,
  styles: [`
    .tabla { width: 100%; }
    .buscador { width: 100%; max-width: 400px; margin-bottom: 1rem; }
    .vacio { text-align: center; padding: 2rem; color: #64748b; }
  `],
})
export class PacientesComponent implements OnInit {
  private svc = inject(PacienteService);
  private router = inject(Router);
  private dialogo = inject(MatDialog);

  readonly columnas = ['identificacion', 'nombre', 'nacimiento', 'celular', 'tipo', 'acciones'];
  pacientes = signal<Patient[]>([]);
  loading = signal(true);
  total = signal(0);
  pageSize = 20;
  private primerVisible = 0;
  searchQuery = '';
  private searchTimer: ReturnType<typeof setTimeout> | null = null;

  ngOnInit(): void {
    this.loadPacientes(0, this.pageSize);
  }

  /** `first` es el índice de la primera fila; `rows`, el tamaño de página. */
  loadPacientes(first: number, rows: number): void {
    this.loading.set(true);
    this.primerVisible = first;
    this.pageSize = rows;
    const page = Math.floor(first / rows) + 1;
    this.svc.list(page, rows, this.searchQuery || undefined).subscribe({
      next: (r: PaginatedResponse<Patient>) => {
        this.pacientes.set(r.items);
        this.total.set(r.total);
        this.loading.set(false);
      },
      error: () => this.loading.set(false),
    });
  }

  alPaginar(e: PageEvent): void {
    this.loadPacientes(e.pageIndex * e.pageSize, e.pageSize);
  }

  onSearch(_q: string): void {
    if (this.searchTimer) clearTimeout(this.searchTimer);
    this.searchTimer = setTimeout(() => this.loadPacientes(0, this.pageSize), 400);
  }

  nuevo(): void {
    this.abrirFormulario(null);
  }

  editar(p: Patient): void {
    this.abrirFormulario(p);
  }

  /**
   * El formulario es un `MatDialog`, no un hijo con `[abierto]`.
   *
   * `disableClose` porque cerrarlo con cambios pendientes pregunta —lo hace el
   * propio formulario, como en el FoxPro—; un clic fuera del cuadro no es una
   * respuesta.
   */
  private abrirFormulario(paciente: Patient | null): void {
    this.dialogo
      .open(PacienteFormComponent, {
        data: paciente,
        width: '52rem',
        maxWidth: '95vw',
        disableClose: true,
      })
      .afterClosed()
      // Solo recarga si volvió un paciente: cerrar sin guardar no cambió nada.
      .subscribe((guardado?: Patient) => { if (guardado) this.alGuardar(); });
  }

  verHistoria(p: Patient): void {
    this.router.navigate(['/historias'], { queryParams: { paciente: p.id } });
  }

  /** Tras guardar se recarga la página actual, no la primera: perder el sitio
   *  en una lista de miles obliga a volver a buscar. */
  alGuardar(): void {
    this.loadPacientes(this.primerVisible, this.pageSize);
  }

  getTipoLabel(tipo: string): string {
    const labels: Record<string, string> = {
      '01': 'Contributivo', '02': 'Subsidiado', '03': 'Vinculado',
      '04': 'Particular', '05': 'Otro',
    };
    return labels[tipo] || tipo;
  }

  getTipoClass(tipo: string): string {
    return tipo === '01' ? 'badge-blue' : tipo === '02' ? 'badge-green' : 'badge-gray';
  }
}
