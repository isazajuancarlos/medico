import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TableModule } from 'primeng/table';
import { ButtonModule } from 'primeng/button';
import { TagModule } from 'primeng/tag';
import { SelectModule } from 'primeng/select';
import { InputTextModule } from 'primeng/inputtext';
import { TextareaModule } from 'primeng/textarea';
import { DialogModule } from 'primeng/dialog';
import { ToastModule } from 'primeng/toast';
import { MessageService } from 'primeng/api';
import {
  UrgenciasService, EntradaCola, NIVELES_TRIAGE, DESTINOS_SALIDA,
} from '../../services/urgencias.service';

/**
 * Urgencias — cola de triage. Backend Rust propio (/api/urgencias). La cola
 * llega YA ordenada por prioridad desde el servidor (rojo primero); aquí solo se
 * pinta, se registra un triage nuevo y se egresa.
 */
@Component({
  selector: 'app-urgencias',
  standalone: true,
  providers: [MessageService],
  imports: [
    CommonModule, FormsModule, TableModule, ButtonModule, TagModule,
    SelectModule, InputTextModule, TextareaModule, DialogModule, ToastModule,
  ],
  template: `
    <p-toast />
    <div class="page-card">
      <div class="list-header">
        <h2 class="page-title">Urgencias — Cola de triage</h2>
        <p-button label="Registrar paciente" icon="pi pi-plus" (onClick)="abrirRegistro()" />
      </div>

      <p-table [value]="cola()" [loading]="cargando()" dataKey="id" styleClass="p-datatable-sm">
        <ng-template pTemplate="header">
          <tr>
            <th>Triage</th><th>Paciente</th><th>Motivo</th>
            <th>Ingreso</th><th style="text-align:center">Acción</th>
          </tr>
        </ng-template>
        <ng-template pTemplate="body" let-p>
          <tr>
            <td><p-tag [value]="p.triage_label" [severity]="severidad(p.triage)" /></td>
            <td>{{ p.paciente_id }}</td>
            <td>{{ p.motivo_consulta }}</td>
            <td>{{ p.fecha_ingreso | date:'HH:mm' }}</td>
            <td style="text-align:center">
              <p-button label="Egresar" icon="pi pi-sign-out" [text]="true" size="small"
                (onClick)="abrirEgreso(p)" />
            </td>
          </tr>
        </ng-template>
        <ng-template pTemplate="emptymessage">
          <tr><td colspan="5" style="text-align:center;padding:2rem;color:#64748b">
            Cola de urgencias vacía
          </td></tr>
        </ng-template>
      </p-table>
    </div>

    <!-- Registrar triage -->
    <p-dialog [(visible)]="mostrarRegistro" header="Registrar paciente en triage"
      [modal]="true" [style]="{ width: '32rem' }">
      <div class="dlg">
        <div class="field">
          <label>Paciente</label>
          <div style="display:flex;gap:0.5rem">
            <input pInputText [(ngModel)]="busqueda" class="w-full"
              placeholder="Nombre o identificación..." (keyup.enter)="buscar()" />
            <p-button icon="pi pi-search" [loading]="buscando()" (onClick)="buscar()" />
          </div>
          @if (pacientes().length) {
            <p-select [(ngModel)]="pacienteSel" [options]="pacientes()" optionLabel="nombre_completo"
              [filter]="true" placeholder="Seleccione..." class="w-full" style="width:100%;margin-top:0.5rem" />
          }
        </div>
        <div class="field">
          <label>Nivel de triage *</label>
          <p-select [(ngModel)]="triageSel" [options]="niveles" optionLabel="label" optionValue="value"
            placeholder="Elija el nivel" class="w-full" style="width:100%" />
        </div>
        <div class="field">
          <label>Motivo de consulta *</label>
          <textarea pTextarea [(ngModel)]="motivo" rows="3" class="w-full"
            placeholder="Motivo por el que consulta..."></textarea>
        </div>
      </div>
      <ng-template pTemplate="footer">
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarRegistro.set(false)" />
        <p-button label="Registrar" icon="pi pi-check" [loading]="guardando()"
          [disabled]="!puedeRegistrar()" (onClick)="registrar()" />
      </ng-template>
    </p-dialog>

    <!-- Egreso -->
    <p-dialog [(visible)]="mostrarEgreso" header="Registrar egreso" [modal]="true" [style]="{ width: '28rem' }">
      <div class="dlg">
        <div class="field">
          <label>Destino de salida *</label>
          <p-select [(ngModel)]="destinoSel" [options]="destinos" optionLabel="label" optionValue="value"
            placeholder="Elija el destino" class="w-full" style="width:100%" />
        </div>
        <div class="field">
          <label>Observaciones</label>
          <textarea pTextarea [(ngModel)]="obsEgreso" rows="2" class="w-full"
            placeholder="Opcional..."></textarea>
        </div>
      </div>
      <ng-template pTemplate="footer">
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarEgreso.set(false)" />
        <p-button label="Egresar" icon="pi pi-check" [loading]="guardando()"
          [disabled]="!destinoSel" (onClick)="confirmarEgreso()" />
      </ng-template>
    </p-dialog>
  `,
  styles: [`
    .list-header { display:flex; justify-content:space-between; align-items:center; margin-bottom:1.25rem; }
    .list-header .page-title { margin-bottom:0; }
    .dlg { display:flex; flex-direction:column; gap:1rem; }
    .field { display:flex; flex-direction:column; gap:0.35rem; }
    .field label { font-weight:500; font-size:0.875rem; color:#e2e8f0; }
    .w-full { width:100%; }
    ::ng-deep textarea { resize:vertical; }
  `],
})
export class UrgenciasComponent implements OnInit {
  private svc = inject(UrgenciasService);
  private messages = inject(MessageService);

  readonly niveles = NIVELES_TRIAGE;
  readonly destinos = DESTINOS_SALIDA;

  cola = signal<EntradaCola[]>([]);
  cargando = signal(true);
  guardando = signal(false);

  // Registro
  mostrarRegistro = signal(false);
  busqueda = '';
  buscando = signal(false);
  pacientes = signal<any[]>([]);
  pacienteSel: any = null;
  triageSel: string | null = null;
  motivo = '';

  // Egreso
  mostrarEgreso = signal(false);
  egresando: EntradaCola | null = null;
  destinoSel: string | null = null;
  obsEgreso = '';

  ngOnInit(): void { this.cargar(); }

  cargar(): void {
    this.cargando.set(true);
    this.svc.cola().subscribe({
      next: c => { this.cola.set(c); this.cargando.set(false); },
      error: () => { this.cargando.set(false); this.err('No se pudo cargar la cola'); },
    });
  }

  severidad(clave: string): 'danger' | 'warn' | 'success' | 'info' | 'secondary' {
    switch (clave) {
      case 'TRIAGE_1': return 'danger';
      case 'TRIAGE_2': return 'warn';
      case 'TRIAGE_3': return 'warn';
      case 'TRIAGE_4': return 'success';
      case 'TRIAGE_5': return 'info';
      default: return 'secondary';
    }
  }

  abrirRegistro(): void {
    this.busqueda = ''; this.pacientes.set([]); this.pacienteSel = null;
    this.triageSel = null; this.motivo = '';
    this.mostrarRegistro.set(true);
  }

  buscar(): void {
    if (!this.busqueda.trim()) return;
    this.buscando.set(true);
    this.svc.searchPacientes().subscribe({
      next: ps => {
        const q = this.busqueda.trim().toLowerCase();
        this.pacientes.set(ps.filter(p =>
          p.nombre_completo.toLowerCase().includes(q) || String(p.identificacion).includes(q)));
        this.buscando.set(false);
        if (!this.pacientes().length) this.err('No se encontraron pacientes');
      },
      error: () => { this.buscando.set(false); this.err('Error al buscar'); },
    });
  }

  puedeRegistrar(): boolean {
    return !!this.pacienteSel && !!this.triageSel && !!this.motivo.trim();
  }

  registrar(): void {
    if (!this.puedeRegistrar()) return;
    this.guardando.set(true);
    this.svc.registrar({
      paciente_id: this.pacienteSel.id,
      triage: this.triageSel!,
      motivo_consulta: this.motivo.trim(),
    }).subscribe({
      next: () => { this.guardando.set(false); this.mostrarRegistro.set(false); this.ok('Paciente en cola'); this.cargar(); },
      error: () => { this.guardando.set(false); this.err('No se pudo registrar'); },
    });
  }

  abrirEgreso(p: EntradaCola): void {
    this.egresando = p; this.destinoSel = null; this.obsEgreso = '';
    this.mostrarEgreso.set(true);
  }

  confirmarEgreso(): void {
    if (!this.egresando || !this.destinoSel) return;
    this.guardando.set(true);
    this.svc.egreso(this.egresando.id, this.destinoSel, this.obsEgreso.trim() || undefined).subscribe({
      next: () => { this.guardando.set(false); this.mostrarEgreso.set(false); this.ok('Egreso registrado'); this.cargar(); },
      error: () => { this.guardando.set(false); this.err('No se pudo registrar el egreso'); },
    });
  }

  private ok(detail: string): void { this.messages.add({ severity: 'success', summary: 'Listo', detail }); }
  private err(detail: string): void { this.messages.add({ severity: 'error', summary: 'Error', detail }); }
}
