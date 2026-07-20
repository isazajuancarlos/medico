import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TableModule, TableLazyLoadEvent } from 'primeng/table';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { TagModule } from 'primeng/tag';
import { TooltipModule } from 'primeng/tooltip';
import { Router } from '@angular/router';
import { PacienteFormComponent } from './paciente-form.component';
import { PacienteService } from '../../services/paciente.service';
import { Patient } from '../../models/patient.models';
import { PaginatedResponse } from '../../models/common.models';

@Component({
  selector: 'app-pacientes',
  standalone: true,
  imports: [CommonModule, FormsModule, TableModule, ButtonModule, InputTextModule, TagModule, TooltipModule, PacienteFormComponent],
  template: `
    <div class="page-card">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;">
        <h2 class="page-title" style="margin-bottom: 0">Pacientes</h2>
        <p-button label="Nuevo Paciente" icon="pi pi-plus" size="small" (onClick)="nuevo()" />
      </div>

      <div style="margin-bottom: 1rem;">
        <input
          pInputText
          [(ngModel)]="searchQuery"
          (ngModelChange)="onSearch($event)"
          placeholder="Buscar por nombre, apellido o identificación..."
          style="width: 100%; max-width: 400px;"
        />
      </div>

      <p-table
        [value]="pacientes()"
        [loading]="loading()"
        [paginator]="true"
        [rows]="pageSize"
        [totalRecords]="total()"
        [lazy]="true"
        (onLazyLoad)="loadPacientes($event)"
        dataKey="id"
        styleClass="p-datatable-sm"
      >
        <ng-template pTemplate="header">
          <tr>
            <th>Identificación</th>
            <th>Nombre Completo</th>
            <th>Fecha Nacimiento</th>
            <th>Celular</th>
            <th>Tipo Usuario</th>
            <th>Acciones</th>
          </tr>
        </ng-template>
        <ng-template pTemplate="body" let-p>
          <tr>
            <td>{{ p.tipo_id }} {{ p.identificacion }}</td>
            <td>{{ p.nombre1 }} {{ p.nombre2 }} {{ p.apellido1 }} {{ p.apellido2 }}</td>
            <td>{{ p.fecha_nacimiento }}</td>
            <td>{{ p.celular }}</td>
            <td>
              <span class="badge" [class]="getTipoClass(p.tipo_usuario)">
                {{ getTipoLabel(p.tipo_usuario) }}
              </span>
            </td>
            <td>
              <p-button icon="pi pi-eye" [text]="true" size="small" pTooltip="Ver Historia"
                (onClick)="verHistoria(p)" />
              <p-button icon="pi pi-pencil" [text]="true" size="small" pTooltip="Editar"
                severity="secondary" (onClick)="editar(p)" />
            </td>
          </tr>
        </ng-template>
        <ng-template pTemplate="emptymessage">
          <tr><td colspan="6" style="text-align:center; padding: 2rem; color: #64748b">
            No se encontraron pacientes
          </td></tr>
        </ng-template>
      </p-table>
    </div>

    <app-paciente-form
      [abierto]="formularioAbierto()"
      [paciente]="enEdicion()"
      (abiertoChange)="formularioAbierto.set($event)"
      (guardado)="alGuardar()"
    />
  `,
})
export class PacientesComponent implements OnInit {
  private svc = inject(PacienteService);
  private router = inject(Router);

  pacientes = signal<Patient[]>([]);
  loading = signal(true);
  total = signal(0);
  pageSize = 20;
  private primerVisible = 0;
  searchQuery = '';
  private searchTimer: ReturnType<typeof setTimeout> | null = null;

  ngOnInit(): void {
    this.loadPacientes({ first: 0, rows: this.pageSize });
  }

  loadPacientes(event: TableLazyLoadEvent): void {
    this.loading.set(true);
    const first = event.first ?? 0;
    const rows = event.rows ?? this.pageSize;
    this.primerVisible = first;
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

  onSearch(_q: string): void {
    if (this.searchTimer) clearTimeout(this.searchTimer);
    this.searchTimer = setTimeout(() => this.loadPacientes({ first: 0, rows: this.pageSize, filters: {} }), 400);
  }

  formularioAbierto = signal(false);
  enEdicion = signal<Patient | null>(null);

  nuevo(): void {
    this.enEdicion.set(null);
    this.formularioAbierto.set(true);
  }

  editar(p: Patient): void {
    this.enEdicion.set(p);
    this.formularioAbierto.set(true);
  }

  verHistoria(p: Patient): void {
    this.router.navigate(['/historias'], { queryParams: { paciente: p.id } });
  }

  /** Tras guardar se recarga la página actual, no la primera: perder el sitio
   *  en una lista de miles obliga a volver a buscar. */
  alGuardar(): void {
    this.loadPacientes({ first: this.primerVisible, rows: this.pageSize });
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
