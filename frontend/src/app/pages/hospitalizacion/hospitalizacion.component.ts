import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { TextareaModule } from 'primeng/textarea';
import { TableModule } from 'primeng/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { SelectModule } from 'primeng/select';
import { MatTooltipModule } from '@angular/material/tooltip';
import { DialogModule } from 'primeng/dialog';
import { MatDividerModule } from '@angular/material/divider';
import { Avisos } from '../../shared/ui/avisos.service';
import {
  HospitalizacionService, Hospitalizacion, Cama,
  MedicoBasic, IngresoCreate, EgresoUpdate,
} from '../../services/hospitalizacion.service';

type Vista = 'lista' | 'ingresar';
type Tab = 'pacientes' | 'camas';

@Component({
  selector: 'app-hospitalizacion',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    ButtonModule, InputTextModule, TextareaModule, TableModule,
    EtiquetaComponent, SelectModule, MatTooltipModule,
    DialogModule, MatDividerModule,
  ],
  template: `
    <div class="page-card">

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA LISTA                               -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'lista') {
        <div class="list-header">
          <h2 class="page-title">Hospitalización</h2>
          <div style="display:flex;gap:0.5rem;align-items:center">
            <p-select [(ngModel)]="filtroEstado" [options]="opcionesEstado"
              optionLabel="label" optionValue="value"
              (onChange)="cargarHospitalizaciones()"
              placeholder="Estado" style="width:170px" />
            <p-button label="Nuevo ingreso" icon="pi pi-plus"
              (onClick)="vista.set('ingresar')" />
          </div>
        </div>

        <!-- Stats -->
        <div class="stats-row">
          <div class="stat-card">
            <div class="stat-value">{{ hospitalizaciones().length }}</div>
            <div class="stat-label">Hospitalizados</div>
          </div>
          <div class="stat-card stat-green">
            <div class="stat-value">{{ camasDisponibles() }}</div>
            <div class="stat-label">Camas disponibles</div>
          </div>
          <div class="stat-card stat-red">
            <div class="stat-value">{{ camasOcupadas() }}</div>
            <div class="stat-label">Camas ocupadas</div>
          </div>
          <div class="stat-card stat-gray">
            <div class="stat-value">{{ todasCamas().length }}</div>
            <div class="stat-label">Total camas</div>
          </div>
        </div>

        <!-- Tabs -->
        <div class="tab-bar">
          <button class="tab-btn" [class.active]="tab() === 'pacientes'"
            (click)="tab.set('pacientes')">
            <i class="pi pi-users"></i> Pacientes
          </button>
          <button class="tab-btn" [class.active]="tab() === 'camas'"
            (click)="tab.set('camas')">
            <i class="pi pi-th-large"></i> Censo de camas
          </button>
        </div>

        <!-- Tab Pacientes -->
        @if (tab() === 'pacientes') {
          @if (cargando()) {
            <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
          } @else if (hospitalizaciones().length === 0) {
            <div class="sin-resultados">
              <i class="pi pi-inbox" style="font-size:2rem;opacity:0.4"></i>
              <p>No hay pacientes hospitalizados con el filtro seleccionado.</p>
            </div>
          } @else {
            <p-table [value]="hospitalizaciones()" [rowHover]="true" styleClass="p-datatable-sm">
              <ng-template pTemplate="header">
                <tr>
                  <th>Paciente</th>
                  <th>Cama / Piso</th>
                  <th>Servicio</th>
                  <th>Médico tratante</th>
                  <th>Ingreso</th>
                  <th style="text-align:center">Días</th>
                  <th style="text-align:center">Estado</th>
                  <th style="text-align:center">Egreso</th>
                </tr>
              </ng-template>
              <ng-template pTemplate="body" let-h>
                <tr>
                  <td>
                    <strong>{{ h.paciente_nombre || '—' }}</strong>
                    @if (h.paciente_identificacion) {
                      <div style="font-size:0.78rem;color:#94a3b8">{{ h.paciente_identificacion }}</div>
                    }
                  </td>
                  <td>
                    <span class="cama-badge">{{ h.cama_numero || '—' }}</span>
                    @if (h.cama_piso) {
                      <div style="font-size:0.78rem;color:#94a3b8">Piso {{ h.cama_piso }}</div>
                    }
                  </td>
                  <td style="font-size:0.85rem">{{ h.cama_servicio || '—' }}</td>
                  <td style="font-size:0.85rem">{{ h.medico_nombre || '—' }}</td>
                  <td style="font-size:0.85rem">{{ h.fecha_ingreso | date:'dd/MM/yyyy' }}</td>
                  <td style="text-align:center">
                    <span class="dias-badge">{{ diasEstancia(h.fecha_ingreso) }}</span>
                  </td>
                  <td style="text-align:center">
                    <x-etiqueta [value]="h.estado" [severity]="estadoSeverity(h.estado)" />
                  </td>
                  <td style="text-align:center">
                    @if (h.estado === 'ACTIVA') {
                      <p-button icon="pi pi-sign-out" [text]="true" size="small"
                        severity="warn" matTooltip="Registrar egreso"
                        (onClick)="abrirEgreso(h)" />
                    } @else {
                      <span style="font-size:0.78rem;color:#64748b">{{ h.fecha_egreso | date:'dd/MM/yy' }}</span>
                    }
                  </td>
                </tr>
              </ng-template>
            </p-table>
          }
        }

        <!-- Tab Camas -->
        @if (tab() === 'camas') {
          @if (cargandoCamas()) {
            <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando camas...</div>
          } @else if (todasCamas().length === 0) {
            <div class="sin-resultados">
              <i class="pi pi-inbox" style="font-size:2rem;opacity:0.4"></i>
              <p>No hay camas registradas en el sistema.</p>
            </div>
          } @else {
            <div class="camas-grid">
              @for (cama of todasCamas(); track cama.id) {
                <div class="cama-card" [class]="'cama-' + cama.estado.toLowerCase()">
                  <div class="cama-numero">{{ cama.numero }}</div>
                  <div class="cama-info">Piso {{ cama.piso }} · {{ cama.servicio }}</div>
                  <div class="cama-estado">{{ cama.estado }}</div>
                  <div class="cama-icons">
                    @if (cama.tiene_oxigeno) {
                      <i class="pi pi-circle-fill" matTooltip="Oxígeno" style="color:#38bdf8;font-size:0.6rem"></i>
                    }
                    @if (cama.tiene_monitor) {
                      <i class="pi pi-heart-fill" matTooltip="Monitor" style="color:#f472b6;font-size:0.6rem"></i>
                    }
                    @if (cama.es_aislamiento) {
                      <i class="pi pi-lock" matTooltip="Aislamiento" style="color:#fb923c;font-size:0.6rem"></i>
                    }
                  </div>
                </div>
              }
            </div>
          }
        }
      }

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA INGRESAR PACIENTE                   -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'ingresar') {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <p-button icon="pi pi-arrow-left" [text]="true" (onClick)="volverLista()" matTooltip="Volver" />
          <h2 class="page-title" style="margin:0">Nuevo Ingreso Hospitalario</h2>
        </div>

        <div class="ingreso-form">

          <!-- Búsqueda paciente -->
          <div class="form-section">
            <div class="section-title">Paciente</div>
            <div style="display:flex;gap:0.5rem;align-items:flex-end">
              <div class="field" style="flex:1">
                <label>Buscar por nombre o identificación</label>
                <input pInputText [(ngModel)]="busquedaPaciente" class="w-full"
                  placeholder="Ingrese nombre o número de identificación..."
                  (keyup.enter)="buscarPaciente()" />
              </div>
              <p-button label="Buscar" icon="pi pi-search"
                [loading]="buscandoPaciente()" (onClick)="buscarPaciente()" />
            </div>
            @if (pacientesResultados().length > 0) {
              <div style="margin-top:0.75rem">
                <label class="field-label">Seleccionar paciente</label>
                <p-select [(ngModel)]="pacienteSeleccionado"
                  [options]="pacientesResultados()"
                  optionLabel="nombre_completo"
                  [filter]="true" filterBy="nombre_completo,identificacion"
                  placeholder="Seleccione el paciente..."
                  class="w-full" style="width:100%" />
              </div>
            }
            @if (pacienteSeleccionado) {
              <div class="paciente-card">
                <i class="pi pi-user" style="color:#818cf8"></i>
                <div>
                  <div style="font-weight:600;color:#f1f5f9">{{ pacienteSeleccionado.nombre_completo }}</div>
                  <div style="font-size:0.82rem;color:#94a3b8">{{ pacienteSeleccionado.identificacion }}</div>
                </div>
                <p-button icon="pi pi-times" [text]="true" size="small" severity="secondary"
                  (onClick)="pacienteSeleccionado = null" />
              </div>
            }
          </div>

          <mat-divider />

          <!-- Cama y médico -->
          <div class="form-section">
            <div class="section-title">Asignación</div>
            <div class="form-grid">
              <div class="field">
                <label>Cama disponible *</label>
                <p-select [(ngModel)]="iForm.cama_id"
                  [options]="camasDisp()"
                  optionLabel="label" optionValue="value"
                  [filter]="true"
                  placeholder="Seleccione cama..."
                  class="w-full" style="width:100%" />
              </div>
              <div class="field">
                <label>Médico tratante *</label>
                <p-select [(ngModel)]="iForm.medico_id"
                  [options]="medicos()"
                  optionLabel="nombre_completo" optionValue="id"
                  [filter]="true"
                  placeholder="Seleccione médico..."
                  class="w-full" style="width:100%" />
              </div>
              <div class="field">
                <label>Vía de ingreso</label>
                <p-select [(ngModel)]="iForm.via_ingreso"
                  [options]="opcionesVia"
                  optionLabel="label" optionValue="value"
                  class="w-full" style="width:100%" />
              </div>
              <div class="field">
                <label>Diagnóstico de ingreso (CIE-10)</label>
                <input pInputText [(ngModel)]="iForm.diagnostico_ingreso"
                  class="w-full" placeholder="Ej: J18.9, Z00.0..." />
              </div>
            </div>
          </div>

          <mat-divider />

          <!-- Notas clínicas -->
          <div class="form-section">
            <div class="section-title">Notas clínicas</div>
            <div class="form-grid">
              <div class="field">
                <label>Motivo de ingreso</label>
                <textarea pTextarea [(ngModel)]="iForm.motivo_ingreso"
                  rows="3" class="w-full"
                  placeholder="Descripción del motivo de hospitalización..."></textarea>
              </div>
              <div class="field">
                <label>Observaciones de ingreso</label>
                <textarea pTextarea [(ngModel)]="iForm.observaciones_ingreso"
                  rows="3" class="w-full"
                  placeholder="Observaciones adicionales al momento del ingreso..."></textarea>
              </div>
            </div>
          </div>

          <div class="step-actions">
            <p-button label="Cancelar" [text]="true" (onClick)="volverLista()" />
            <p-button label="Registrar ingreso" icon="pi pi-check"
              [loading]="ingresando()"
              [disabled]="!pacienteSeleccionado || !iForm.cama_id || !iForm.medico_id"
              (onClick)="registrarIngreso()" />
          </div>
        </div>
      }

    </div>

    <!-- ══ DIALOG EGRESO ══ -->
    <p-dialog header="Registrar Egreso" [(visible)]="mostrarEgreso"
      [style]="{width:'520px'}" [modal]="true" [closable]="true">
      @if (hospEgreso()) {
        <div style="margin-bottom:1rem">
          <div style="color:#94a3b8;font-size:0.78rem;text-transform:uppercase;letter-spacing:0.05em">Paciente</div>
          <div style="font-weight:600;color:#f1f5f9;margin-top:0.2rem">{{ hospEgreso()!.paciente_nombre }}</div>
          <div style="font-size:0.82rem;color:#94a3b8">Cama {{ hospEgreso()!.cama_numero }} · {{ hospEgreso()!.cama_servicio }}</div>
        </div>
        <div class="field" style="margin-bottom:1rem">
          <label class="field-label">Tipo de egreso *</label>
          <p-select [(ngModel)]="eForm.tipo_egreso" [options]="opcionesEgreso"
            optionLabel="label" optionValue="value"
            class="w-full" style="width:100%" />
        </div>
        <div class="field" style="margin-bottom:1rem">
          <label class="field-label">Diagnóstico de egreso (CIE-10)</label>
          <input pInputText [(ngModel)]="eForm.diagnostico_egreso" class="w-full"
            placeholder="Código CIE-10..." />
        </div>
        <div class="field" style="margin-bottom:1rem">
          <label class="field-label">Resumen clínico del egreso</label>
          <textarea pTextarea [(ngModel)]="eForm.resumen_clinico" rows="4" class="w-full"
            placeholder="Evolución durante la hospitalización, condición al alta..."></textarea>
        </div>
        <div style="display:flex;justify-content:flex-end;gap:0.5rem;padding-top:0.75rem;border-top:1px solid rgba(255,255,255,0.1)">
          <p-button label="Cancelar" [text]="true" (onClick)="mostrarEgreso = false" />
          <p-button label="Registrar egreso" icon="pi pi-sign-out"
            severity="warn" [loading]="registrandoEgreso()"
            (onClick)="registrarEgreso()" />
        </div>
      }
    </p-dialog>

  `,
  styles: [`
    .page-card { padding: 1.5rem; }
    .page-title { font-size: 1.3rem; font-weight: 700; color: #f1f5f9; margin: 0 0 1.5rem; }
    .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.25rem; }
    .list-header .page-title { margin-bottom: 0; }
    .cargando { padding: 3rem; text-align: center; color: #64748b; display: flex; align-items: center; justify-content: center; gap: 0.5rem; }
    .sin-resultados { text-align: center; padding: 3rem 1rem; color: #64748b; display: flex; flex-direction: column; align-items: center; gap: 0.75rem; }

    .stats-row { display: flex; gap: 1rem; margin-bottom: 1.25rem; flex-wrap: wrap; }
    .stat-card {
      flex: 1; min-width: 110px;
      background: rgba(255,255,255,0.04); border: 1px solid rgba(255,255,255,0.08);
      border-radius: 8px; padding: 0.875rem 1rem;
    }
    .stat-card.stat-green { border-color: rgba(74,222,128,0.2); background: rgba(74,222,128,0.05); }
    .stat-card.stat-red { border-color: rgba(248,113,113,0.2); background: rgba(248,113,113,0.05); }
    .stat-card.stat-gray { border-color: rgba(148,163,184,0.15); }
    .stat-value { font-size: 1.75rem; font-weight: 700; color: #f1f5f9; line-height: 1; }
    .stat-label { font-size: 0.72rem; color: #94a3b8; margin-top: 0.25rem; text-transform: uppercase; letter-spacing: 0.04em; }

    .tab-bar { display: flex; gap: 0.25rem; margin-bottom: 1rem; border-bottom: 1px solid rgba(255,255,255,0.1); }
    .tab-btn {
      background: none; border: none; color: #94a3b8; cursor: pointer;
      padding: 0.5rem 1rem; font-size: 0.875rem; border-bottom: 2px solid transparent;
      display: flex; align-items: center; gap: 0.4rem; transition: all 0.15s;
      margin-bottom: -1px;
    }
    .tab-btn:hover { color: #e2e8f0; }
    .tab-btn.active { color: #818cf8; border-bottom-color: #818cf8; font-weight: 600; }

    .camas-grid {
      display: grid; grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
      gap: 0.75rem; margin-top: 0.5rem;
    }
    .cama-card {
      border-radius: 8px; padding: 0.875rem;
      border: 1px solid rgba(255,255,255,0.1);
      background: rgba(255,255,255,0.03);
      display: flex; flex-direction: column; gap: 0.2rem;
    }
    .cama-disponible { border-color: rgba(74,222,128,0.3); background: rgba(74,222,128,0.06); }
    .cama-ocupada { border-color: rgba(248,113,113,0.35); background: rgba(248,113,113,0.08); }
    .cama-en_aseo { border-color: rgba(251,191,36,0.3); background: rgba(251,191,36,0.06); }
    .cama-bloqueada, .cama-mantenimiento { border-color: rgba(148,163,184,0.2); }
    .cama-numero { font-size: 1.1rem; font-weight: 700; color: #f1f5f9; }
    .cama-info { font-size: 0.72rem; color: #94a3b8; }
    .cama-estado { font-size: 0.68rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; color: #64748b; margin-top: 0.15rem; }
    .cama-disponible .cama-estado { color: #4ade80; }
    .cama-ocupada .cama-estado { color: #f87171; }
    .cama-en_aseo .cama-estado { color: #fbbf24; }
    .cama-icons { display: flex; gap: 0.35rem; margin-top: 0.35rem; }

    .cama-badge {
      display: inline-block; padding: 0.15rem 0.5rem; border-radius: 4px;
      background: rgba(99,102,241,0.15); color: #a5b4fc;
      font-size: 0.82rem; font-weight: 600; border: 1px solid rgba(99,102,241,0.25);
    }
    .dias-badge {
      display: inline-block; padding: 0.15rem 0.6rem; border-radius: 999px;
      background: rgba(255,255,255,0.06); color: #e2e8f0; font-size: 0.82rem; font-weight: 600;
    }

    .ingreso-form {
      background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.07);
      border-radius: 10px; padding: 1.5rem;
    }
    .form-section { margin-bottom: 0.25rem; }
    .section-title { font-size: 0.75rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em; color: #818cf8; margin-bottom: 1rem; }
    .form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
    .field { display: flex; flex-direction: column; gap: 0.35rem; }
    .field label, .field-label { font-weight: 500; font-size: 0.875rem; color: #e2e8f0; }
    .w-full { width: 100%; }

    .paciente-card {
      display: flex; align-items: center; gap: 0.75rem;
      background: rgba(99,102,241,0.1); border: 1px solid rgba(99,102,241,0.25);
      border-radius: 8px; padding: 0.75rem 1rem; margin-top: 0.75rem;
    }

    .step-actions { display: flex; gap: 0.5rem; justify-content: flex-end; padding-top: 1.25rem; border-top: 1px solid rgba(255,255,255,0.1); margin-top: 1rem; }

    ::ng-deep textarea { resize: vertical; }
  `]
})
export class HospitalizacionComponent implements OnInit {
  private svc = inject(HospitalizacionService);
  private messageService = inject(Avisos);

  vista = signal<Vista>('lista');
  tab = signal<Tab>('pacientes');
  filtroEstado = '';
  cargando = signal(false);
  cargandoCamas = signal(false);
  hospitalizaciones = signal<Hospitalizacion[]>([]);
  todasCamas = signal<Cama[]>([]);
  medicos = signal<MedicoBasic[]>([]);

  camasDisponibles = () => this.todasCamas().filter(c => c.estado === 'DISPONIBLE').length;
  camasOcupadas = () => this.todasCamas().filter(c => c.estado === 'OCUPADA').length;
  camasDisp = () => this.todasCamas()
    .filter(c => c.estado === 'DISPONIBLE')
    .map(c => ({ label: `${c.numero} — Piso ${c.piso} · ${c.servicio}`, value: c.id }));

  readonly opcionesEstado = [
    { label: 'Activos', value: '' },
    { label: 'Alta médica', value: 'ALTA_MEDICA' },
    { label: 'Alta voluntaria', value: 'ALTA_VOLUNTARIA' },
    { label: 'Remisión', value: 'REMISION' },
    { label: 'Defunción', value: 'DEFUNCION' },
  ];

  readonly opcionesVia = [
    { label: 'Programada', value: 'PROGRAMADA' },
    { label: 'Urgencias', value: 'URGENCIAS' },
    { label: 'Remisión', value: 'REMISION' },
  ];

  readonly opcionesEgreso = [
    { label: 'Alta médica', value: 'ALTA_MEDICA' },
    { label: 'Alta voluntaria', value: 'ALTA_VOLUNTARIA' },
    { label: 'Remisión', value: 'REMISION' },
    { label: 'Defunción', value: 'DEFUNCION' },
  ];

  // Ingreso
  busquedaPaciente = '';
  buscandoPaciente = signal(false);
  pacientesResultados = signal<any[]>([]);
  pacienteSeleccionado: any = null;
  ingresando = signal(false);
  iForm: Partial<IngresoCreate> = { diagnostico_ingreso: 'Z00.0', via_ingreso: 'PROGRAMADA' };

  // Egreso
  mostrarEgreso = false;
  hospEgreso = signal<Hospitalizacion | null>(null);
  registrandoEgreso = signal(false);
  eForm: EgresoUpdate = { tipo_egreso: 'ALTA_MEDICA' };

  ngOnInit(): void {
    this.cargarHospitalizaciones();
    this.cargarCamas();
    this.svc.listMedicos().subscribe({ next: ms => this.medicos.set(ms) });
  }

  cargarHospitalizaciones(): void {
    this.cargando.set(true);
    this.svc.listHospitalizaciones(this.filtroEstado || undefined).subscribe({
      next: hs => { this.hospitalizaciones.set(hs); this.cargando.set(false); },
      error: () => { this.cargando.set(false); this.err('Error al cargar hospitalizaciones'); },
    });
  }

  cargarCamas(): void {
    this.cargandoCamas.set(true);
    this.svc.listCamas().subscribe({
      next: cs => { this.todasCamas.set(cs); this.cargandoCamas.set(false); },
      error: () => this.cargandoCamas.set(false),
    });
  }

  volverLista(): void {
    this.vista.set('lista');
    this.busquedaPaciente = '';
    this.pacientesResultados.set([]);
    this.pacienteSeleccionado = null;
    this.iForm = { diagnostico_ingreso: 'Z00.0', via_ingreso: 'PROGRAMADA' };
    this.cargarHospitalizaciones();
    this.cargarCamas();
  }

  buscarPaciente(): void {
    if (!this.busquedaPaciente.trim()) return;
    this.buscandoPaciente.set(true);
    this.svc.searchPacientes(this.busquedaPaciente).subscribe({
      next: ps => {
        this.pacientesResultados.set(ps);
        this.buscandoPaciente.set(false);
        if (ps.length === 0) this.err('No se encontraron pacientes con ese criterio');
      },
      error: () => { this.buscandoPaciente.set(false); this.err('Error al buscar pacientes'); },
    });
  }

  registrarIngreso(): void {
    if (!this.pacienteSeleccionado || !this.iForm.cama_id || !this.iForm.medico_id) return;
    const payload: IngresoCreate = {
      paciente_id: this.pacienteSeleccionado.id,
      cama_id: this.iForm.cama_id!,
      medico_id: this.iForm.medico_id!,
      diagnostico_ingreso: this.iForm.diagnostico_ingreso || 'Z00.0',
      via_ingreso: this.iForm.via_ingreso || 'PROGRAMADA',
      motivo_ingreso: this.iForm.motivo_ingreso,
      observaciones_ingreso: this.iForm.observaciones_ingreso,
    };
    this.ingresando.set(true);
    this.svc.ingresar(payload).subscribe({
      next: h => {
        this.ingresando.set(false);
        this.ok('Ingreso registrado', `Paciente asignado a cama ${h.cama_numero}`);
        this.volverLista();
      },
      error: err => {
        this.ingresando.set(false);
        this.err(err?.error?.detail ?? 'Error al registrar el ingreso');
      },
    });
  }

  abrirEgreso(h: Hospitalizacion): void {
    this.hospEgreso.set(h);
    this.eForm = { tipo_egreso: 'ALTA_MEDICA' };
    this.mostrarEgreso = true;
  }

  registrarEgreso(): void {
    const h = this.hospEgreso();
    if (!h) return;
    this.registrandoEgreso.set(true);
    this.svc.egreso(h.id, this.eForm).subscribe({
      next: () => {
        this.registrandoEgreso.set(false);
        this.mostrarEgreso = false;
        this.ok('Egreso registrado', `Cama ${h.cama_numero} liberada`);
        this.cargarHospitalizaciones();
        this.cargarCamas();
      },
      error: err => {
        this.registrandoEgreso.set(false);
        this.err(err?.error?.detail ?? 'Error al registrar el egreso');
      },
    });
  }

  diasEstancia(fechaIngreso: string): number {
    return Math.floor((Date.now() - new Date(fechaIngreso).getTime()) / 86_400_000);
  }

  estadoSeverity(estado: string): 'info' | 'success' | 'warn' | 'danger' | 'secondary' {
    if (estado === 'ACTIVA') return 'info';
    if (estado === 'ALTA_MEDICA' || estado === 'ALTA_VOLUNTARIA') return 'success';
    if (estado === 'REMISION') return 'warn';
    if (estado === 'DEFUNCION') return 'danger';
    return 'secondary';
  }

  private ok(summary: string, detail?: string): void {
    this.messageService.add({ severity: 'success', summary, detail });
  }

  private err(detail: string): void {
    this.messageService.add({ severity: 'error', summary: 'Error', detail });
  }
}
