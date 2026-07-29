import { Component, OnInit, inject, signal, computed } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { Router } from '@angular/router';
import { TableModule } from 'primeng/table';
import { ButtonModule } from 'primeng/button';
import { TagModule } from 'primeng/tag';
import { DialogModule } from 'primeng/dialog';
import { InputTextModule } from 'primeng/inputtext';
import { SelectModule } from 'primeng/select';
import { DatePickerModule } from 'primeng/datepicker';
import { AutoCompleteModule } from 'primeng/autocomplete';
import { ToastModule } from 'primeng/toast';
import { MessageService } from 'primeng/api';
import { CitaService } from '../../services/cita.service';
import { PacienteService } from '../../services/paciente.service';
import { Appointment, AppointmentCreate, SlotDisponible, MedicoDisponible } from '../../models/appointment.models';

@Component({
  selector: 'app-citas',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    TableModule, ButtonModule, TagModule, DialogModule,
    InputTextModule, SelectModule, DatePickerModule, AutoCompleteModule, ToastModule,
  ],
  providers: [MessageService],
  template: `
<p-toast />
<div class="page-card">
  <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem">
    <h2 class="page-title" style="margin-bottom:0">Agenda de Citas</h2>
    <p-button label="Nueva Cita" icon="pi pi-plus" size="small" (onClick)="openNew()" />
  </div>

  <!-- Filters -->
  <div style="display:flex;gap:.75rem;flex-wrap:wrap;margin-bottom:1rem">
    <p-datepicker
      [(ngModel)]="filtroFecha"
      placeholder="Filtrar por fecha"
      dateFormat="yy-mm-dd"
      [showClear]="true"
      (onSelect)="applyFilters()"
      (onClear)="applyFilters()"
      inputStyleClass="p-inputtext-sm"
    />
    <p-select
      [(ngModel)]="filtroMedico"
      [options]="medicos()"
      optionLabel="nombre"
      optionValue="id"
      placeholder="Todos los médicos"
      [showClear]="true"
      (onChange)="applyFilters()"
      styleClass="p-inputtext-sm"
    />
    <p-select
      [(ngModel)]="filtroEstado"
      [options]="estadoOpts"
      placeholder="Todos los estados"
      [showClear]="true"
      (onChange)="applyFilters()"
      styleClass="p-inputtext-sm"
    />
  </div>

  <p-table
    [value]="citas()"
    [loading]="loading()"
    dataKey="id"
    styleClass="p-datatable-sm"
    [paginator]="true"
    [rows]="20"
  >
    <ng-template pTemplate="header">
      <tr>
        <th>Fecha</th><th>Hora</th><th>Paciente</th><th>Médico</th>
        <th>Tipo</th><th>Motivo</th><th>Estado</th><th>Acciones</th>
      </tr>
    </ng-template>
    <ng-template pTemplate="body" let-c>
      <tr>
        <td>{{ c.fecha }}</td>
        <td>{{ c.hora }}</td>
        <td>{{ c.paciente_nombre || c.paciente_id }}</td>
        <td>{{ c.medico_nombre || c.medico_id }}</td>
        <td>{{ c.tipo }}</td>
        <td>{{ c.motivo }}</td>
        <td>
          <p-tag [value]="c.estado" [severity]="estadoSeverity(c.estado)" />
        </td>
        <td style="display:flex;gap:.25rem">
          @if (c.estado === 'Pendiente' || c.estado === 'Confirmada') {
            <p-button
              label="Atender" icon="pi pi-play" size="small" severity="success"
              (onClick)="atenderCita(c)"
              [loading]="attendingId() === c.id"
            />
            <p-button
              icon="pi pi-times" size="small" severity="danger" [text]="true"
              pTooltip="Cancelar cita"
              (onClick)="cancelarCita(c)"
            />
          }
        </td>
      </tr>
    </ng-template>
    <ng-template pTemplate="emptymessage">
      <tr><td colspan="8" style="text-align:center;padding:2rem;color:#64748b">
        No hay citas para los filtros seleccionados
      </td></tr>
    </ng-template>
  </p-table>
</div>

<!-- Nueva Cita Dialog -->
<p-dialog
  [(visible)]="showDialog"
  header="Nueva Cita"
  [modal]="true"
  [style]="{width:'650px'}"
  [closable]="true"
>
  <div style="display:grid;grid-template-columns:1fr 1fr;gap:1rem;padding:.5rem 0">

    <!-- Paciente search -->
    <div style="grid-column:1/-1">
      <label class="form-label">Paciente *</label>
      <p-autoComplete
        [(ngModel)]="selectedPaciente"
        [suggestions]="pacienteSuggestions()"
        (completeMethod)="searchPaciente($event)"
        field="label"
        placeholder="Buscar por nombre o identificación..."
        [style]="{width:'100%'}"
        (onSelect)="onPacienteSelect($event)"
        [forceSelection]="true"
      >
        <ng-template pTemplate="item" let-item>
          <div>{{ item.label }}</div>
          <small style="color:#64748b">{{ item.identificacion }}</small>
        </ng-template>
      </p-autoComplete>
    </div>

    <!-- Médico -->
    <div>
      <label class="form-label">Médico *</label>
      <p-select
        [(ngModel)]="form.medico_id"
        [options]="medicos()"
        optionLabel="nombre"
        optionValue="id"
        placeholder="Seleccionar médico"
        [style]="{width:'100%'}"
        (onChange)="onMedicoChange()"
      />
    </div>

    <!-- Tipo -->
    <div>
      <label class="form-label">Tipo de Cita *</label>
      <p-select
        [(ngModel)]="form.tipo"
        [options]="tipoOpts"
        placeholder="Tipo"
        [style]="{width:'100%'}"
      />
    </div>

    <!-- Fecha -->
    <div>
      <label class="form-label">Fecha *</label>
      <p-datepicker
        [(ngModel)]="form.fecha"
        dateFormat="yy-mm-dd"
        [minDate]="today"
        placeholder="Seleccionar fecha"
        [style]="{width:'100%'}"
        (onSelect)="onFechaChange()"
      />
    </div>

    <!-- Duración -->
    <div>
      <label class="form-label">Duración (min)</label>
      <p-select
        [(ngModel)]="form.duracion_minutos"
        [options]="duracionOpts"
        [style]="{width:'100%'}"
      />
    </div>

    <!-- Hora (alta manual: la agenda por slots aún no está portada al medico-api) -->
    <div>
      <label class="form-label">Hora *</label>
      <input
        type="time"
        [(ngModel)]="form.hora"
        style="width:100%;padding:.5rem;border:1px solid #e2e8f0;border-radius:.375rem"
      />
    </div>

    <!-- Motivo -->
    <div style="grid-column:1/-1">
      <label class="form-label">Motivo de consulta</label>
      <input pInputText [(ngModel)]="form.motivo" placeholder="Motivo de la visita" style="width:100%" />
    </div>
  </div>

  <ng-template pTemplate="footer">
    <p-button label="Cancelar" severity="secondary" [text]="true" (onClick)="showDialog=false" />
    <p-button
      label="Agendar Cita" icon="pi pi-check"
      (onClick)="saveCita()"
      [loading]="saving()"
      [disabled]="!isFormValid()"
    />
  </ng-template>
</p-dialog>
  `,
  styles: [`
    .form-label { display:block; font-size:.875rem; font-weight:500; color:#374151; margin-bottom:.25rem }
    .slot-btn {
      padding:.25rem .5rem; border-radius:.375rem; font-size:.8rem; cursor:pointer;
      border:1px solid #e2e8f0; background:#f8fafc; transition:all .15s;
    }
    .slot-disponible:hover { background:#dbeafe; border-color:#3b82f6 }
    .slot-ocupado { background:#fee2e2; color:#9ca3af; cursor:not-allowed; border-color:#fca5a5 }
    .slot-selected { background:#3b82f6 !important; color:#fff !important; border-color:#2563eb !important }
  `],
})
export class CitasComponent implements OnInit {
  private svc = inject(CitaService);
  private pacienteSvc = inject(PacienteService);
  private router = inject(Router);
  private msg = inject(MessageService);

  citas = signal<Appointment[]>([]);
  medicos = signal<MedicoDisponible[]>([]);
  slots = signal<SlotDisponible[]>([]);
  pacienteSuggestions = signal<any[]>([]);

  loading = signal(true);
  loadingSlots = signal(false);
  saving = signal(false);
  attendingId = signal<number | null>(null);

  showDialog = false;
  today = new Date();

  // Filters
  filtroFecha: Date | null = null;
  filtroMedico: number | null = null;
  filtroEstado: string | null = null;

  // Form state
  selectedPaciente: any = null;
  form: Partial<AppointmentCreate> & { fecha?: Date } = {
    tipo: 'Primera Vez',
    duracion_minutos: 30,
    motivo: '',
  };

  estadoOpts = ['Pendiente', 'Confirmada', 'Atendida', 'Cancelada', 'No Asistió'];
  tipoOpts = ['Primera Vez', 'Control / Revisión', 'Urgencia'];
  duracionOpts = [
    { label: '20 min', value: 20 },
    { label: '30 min', value: 30 },
    { label: '45 min', value: 45 },
    { label: '60 min', value: 60 },
  ];

  ngOnInit(): void {
    this.loadCitas();
    this.svc.getMedicos().subscribe(m => this.medicos.set(m));
  }

  loadCitas(): void {
    this.loading.set(true);
    const fechaStr = this.filtroFecha ? this.toDateStr(this.filtroFecha) : undefined;
    this.svc.list(1, 100, fechaStr, this.filtroMedico ?? undefined, this.filtroEstado ?? undefined).subscribe({
      next: r => { this.citas.set(r.items); this.loading.set(false); },
      error: () => this.loading.set(false),
    });
  }

  applyFilters(): void {
    this.loadCitas();
  }

  openNew(): void {
    this.selectedPaciente = null;
    this.form = { tipo: 'Primera Vez', duracion_minutos: 30, motivo: '' };
    this.slots.set([]);
    this.showDialog = true;
  }

  searchPaciente(event: { query: string }): void {
    this.pacienteSvc.buscar(event.query).subscribe({
      next: (items: any[]) => {
        this.pacienteSuggestions.set(
          items.map((p: any) => ({
            id: p.id,
            label: `${p.nombre1} ${p.apellido1}`,
            identificacion: p.identificacion,
          }))
        );
      },
      error: () => this.pacienteSuggestions.set([]),
    });
  }

  onPacienteSelect(item: any): void {
    this.form.paciente_id = item.id;
  }

  onMedicoChange(): void {
    this.form.hora = undefined;
    this.slots.set([]);
    if (this.form.medico_id && this.form.fecha) {
      this.loadSlots();
    }
  }

  onFechaChange(): void {
    this.form.hora = undefined;
    this.slots.set([]);
    if (this.form.medico_id && this.form.fecha) {
      this.loadSlots();
    }
  }

  loadSlots(): void {
    if (!this.form.medico_id || !this.form.fecha) return;
    this.loadingSlots.set(true);
    const fechaStr = this.toDateStr(this.form.fecha as Date);
    this.svc.getSlots(this.form.medico_id, fechaStr).subscribe({
      next: s => { this.slots.set(s); this.loadingSlots.set(false); },
      error: () => this.loadingSlots.set(false),
    });
  }

  selectSlot(slot: SlotDisponible): void {
    if (slot.disponible) this.form.hora = slot.hora;
  }

  isFormValid(): boolean {
    return !!(this.form.paciente_id && this.form.medico_id && this.form.fecha && this.form.hora);
  }

  saveCita(): void {
    if (!this.isFormValid()) return;
    this.saving.set(true);
    const payload: AppointmentCreate = {
      paciente_id: this.form.paciente_id!,
      medico_id: this.form.medico_id!,
      fecha: this.toDateStr(this.form.fecha as Date),
      hora: this.form.hora!,
      tipo: this.form.tipo as any,
      motivo: this.form.motivo,
      duracion_minutos: this.form.duracion_minutos ?? 30,
    };
    this.svc.create(payload).subscribe({
      next: () => {
        this.saving.set(false);
        this.showDialog = false;
        this.msg.add({ severity: 'success', summary: 'Cita agendada', detail: 'La cita fue registrada exitosamente' });
        this.loadCitas();
      },
      error: (err) => {
        this.saving.set(false);
        const detail = err?.error?.detail ?? 'No se pudo agendar la cita';
        this.msg.add({ severity: 'error', summary: 'Error', detail });
      },
    });
  }

  atenderCita(cita: Appointment): void {
    this.attendingId.set(cita.id);
    // La apertura de la historia clínica llegará cuando ese módulo se porte; por
    // ahora, atender = registrar el estado ATENDIDA.
    this.svc.marcarAtendida(cita.id).subscribe({
      next: () => {
        this.attendingId.set(null);
        this.msg.add({ severity: 'success', summary: 'Cita atendida' });
        this.loadCitas();
      },
      error: () => this.attendingId.set(null),
    });
  }

  cancelarCita(cita: Appointment): void {
    this.svc.cancel(cita.id).subscribe({
      next: () => {
        this.msg.add({ severity: 'info', summary: 'Cita cancelada' });
        this.loadCitas();
      },
    });
  }

  estadoSeverity(estado: string): 'info' | 'success' | 'warn' | 'danger' | 'secondary' {
    const map: Record<string, any> = {
      'Pendiente': 'warn', 'Confirmada': 'info',
      'Atendida': 'success', 'Cancelada': 'danger', 'No Asistió': 'secondary',
    };
    return map[estado] ?? 'secondary';
  }

  private toDateStr(d: Date): string {
    return d.toISOString().slice(0, 10);
  }
}
