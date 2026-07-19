import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { HttpClient } from '@angular/common/http';
import { TableModule } from 'primeng/table';
import { ButtonModule } from 'primeng/button';
import { TagModule } from 'primeng/tag';

interface TriageEntry {
  id: number;
  paciente_id: number;
  triage: string;
  motivo_consulta: string;
  fecha_ingreso: string;
  estado: string;
}

@Component({
  selector: 'app-urgencias',
  standalone: true,
  imports: [CommonModule, TableModule, ButtonModule, TagModule],
  template: `
    <div class="page-card">
      <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;">
        <h2 class="page-title" style="margin-bottom: 0">Urgencias - Cola de Triage</h2>
        <p-button label="Registrar Paciente" icon="pi pi-plus" size="small" />
      </div>

      <p-table [value]="cola()" [loading]="loading()" dataKey="id" styleClass="p-datatable-sm">
        <ng-template pTemplate="header">
          <tr>
            <th>Triage</th>
            <th>Paciente</th>
            <th>Motivo</th>
            <th>Ingreso</th>
            <th>Estado</th>
            <th>Acciones</th>
          </tr>
        </ng-template>
        <ng-template pTemplate="body" let-p>
          <tr>
            <td>
              <span class="badge" [ngClass]="getTriageClass(p.triage)">{{ p.triage }}</span>
            </td>
            <td>{{ p.paciente_id }}</td>
            <td>{{ p.motivo_consulta }}</td>
            <td>{{ p.fecha_ingreso | date:'HH:mm' }}</td>
            <td>{{ p.estado }}</td>
            <td>
              <p-button label="Atender" icon="pi pi-play" size="small" />
            </td>
          </tr>
        </ng-template>
        <ng-template pTemplate="emptymessage">
          <tr><td colspan="6" style="text-align:center; padding: 2rem; color: #64748b">
            Cola de urgencias vacía
          </td></tr>
        </ng-template>
      </p-table>
    </div>
  `,
})
export class UrgenciasComponent implements OnInit {
  private http = inject(HttpClient);
  cola = signal<TriageEntry[]>([]);
  loading = signal(true);

  ngOnInit(): void {
    this.http.get<TriageEntry[]>('/api/urgencias/cola').subscribe({
      next: data => { this.cola.set(data); this.loading.set(false); },
      error: () => this.loading.set(false),
    });
  }

  getTriageClass(triage: string): string {
    if (triage.includes('Rojo')) return 'triage-rojo';
    if (triage.includes('Naranja')) return 'triage-naranja';
    if (triage.includes('Amarillo')) return 'triage-amarillo';
    if (triage.includes('Verde')) return 'triage-verde';
    return 'triage-azul';
  }
}
