import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { TextareaModule } from 'primeng/textarea';
import { TableModule } from 'primeng/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { MatTooltipModule } from '@angular/material/tooltip';
import { SelectModule } from 'primeng/select';
import { MatDividerModule } from '@angular/material/divider';
import { Avisos } from '../../shared/ui/avisos.service';
import { Confirmar } from '../../shared/ui/confirmar.service';
import {
  InterconsultaService, Interconsulta, InterconsultaCreate, InterconsultaResponder,
} from '../../services/interconsulta.service';

type Vista = 'lista' | 'nueva' | 'detalle';

@Component({
  selector: 'app-interconsultas',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    ButtonModule, InputTextModule, TextareaModule, TableModule,
    EtiquetaComponent, MatTooltipModule, SelectModule,
    MatDividerModule,
  ],
  template: `
    <div class="page-card">

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA LISTA                               -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'lista') {
        <div class="list-header">
          <h2 class="page-title">Interconsultas</h2>
          <p-button label="Nueva interconsulta" icon="pi pi-plus"
            (onClick)="abrirNueva()" />
        </div>

        <!-- Filtros -->
        <div class="filtros-bar">
          <div class="tab-bar">
            <button class="tab-btn" [class.active]="tabActivo() === 'todas'"
              (click)="cambiarTab('todas')">Todas</button>
            <button class="tab-btn" [class.active]="tabActivo() === 'mias'"
              (click)="cambiarTab('mias')">Mis solicitudes</button>
            <button class="tab-btn" [class.active]="tabActivo() === 'para_mi'"
              (click)="cambiarTab('para_mi')">Para mí</button>
          </div>
          <div style="display:flex;gap:0.5rem;margin-left:auto">
            <p-select [(ngModel)]="filtroEstado" [options]="opcionesEstado"
              optionLabel="label" optionValue="value"
              (onChange)="cargar()" placeholder="Estado" style="width:155px" />
            <p-select [(ngModel)]="filtroPrioridad" [options]="opcionesPrioridad"
              optionLabel="label" optionValue="value"
              (onChange)="cargar()" placeholder="Prioridad" style="width:140px" />
          </div>
        </div>

        @if (cargando()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (interconsultas().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-send" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay interconsultas con los filtros seleccionados.</p>
          </div>
        } @else {
          <p-table [value]="interconsultas()" [rowHover]="true" styleClass="p-datatable-sm">
            <ng-template pTemplate="header">
              <tr>
                <th>Paciente</th>
                <th>Especialidad solicitada</th>
                <th>Solicita</th>
                <th>Fecha</th>
                <th style="text-align:center">Prioridad</th>
                <th style="text-align:center">Estado</th>
                <th style="text-align:center">Acción</th>
              </tr>
            </ng-template>
            <ng-template pTemplate="body" let-ic>
              <tr>
                <td>
                  <strong>{{ ic.paciente_nombre || '—' }}</strong>
                  @if (ic.paciente_identificacion) {
                    <div style="font-size:0.78rem;color:#94a3b8">{{ ic.paciente_identificacion }}</div>
                  }
                </td>
                <td style="font-weight:500">{{ ic.especialidad_nombre || '—' }}</td>
                <td style="font-size:0.85rem">{{ ic.medico_solicita_nombre || '—' }}</td>
                <td style="font-size:0.85rem;white-space:nowrap">{{ ic.fecha_solicitud | date:'dd/MM/yyyy' }}</td>
                <td style="text-align:center">
                  <x-etiqueta [value]="ic.prioridad" [severity]="prioridadSeverity(ic.prioridad)" />
                </td>
                <td style="text-align:center">
                  <x-etiqueta [value]="estadoLabel(ic.estado)" [severity]="estadoSeverity(ic.estado)" />
                </td>
                <td style="text-align:center">
                  <p-button icon="pi pi-file-edit" [text]="true" size="small"
                    matTooltip="Ver detalle" (onClick)="abrirDetalle(ic)" />
                </td>
              </tr>
            </ng-template>
          </p-table>
        }
      }

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA NUEVA INTERCONSULTA                 -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'nueva') {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <p-button icon="pi pi-arrow-left" [text]="true" (onClick)="volverLista()" />
          <h2 class="page-title" style="margin:0">Nueva Interconsulta</h2>
        </div>

        <div class="form-card">
          <!-- Paciente -->
          <div class="form-section">
            <div class="section-label">Paciente</div>
            <div style="display:flex;gap:0.5rem;align-items:flex-end">
              <div class="field" style="flex:1">
                <input pInputText [(ngModel)]="busquedaPaciente" class="w-full"
                  placeholder="Nombre o identificación del paciente..."
                  (keyup.enter)="buscarPaciente()" />
              </div>
              <p-button label="Buscar" icon="pi pi-search"
                [loading]="buscandoPaciente()" (onClick)="buscarPaciente()" />
            </div>
            @if (resultadosPaciente().length > 0) {
              <div style="margin-top:0.75rem">
                <p-select [(ngModel)]="pacienteSeleccionado"
                  [options]="resultadosPaciente()"
                  optionLabel="nombre_completo" [filter]="true"
                  placeholder="Seleccione paciente..."
                  class="w-full" style="width:100%" />
              </div>
            }
            @if (pacienteSeleccionado) {
              <div class="paciente-chip">
                <i class="pi pi-user" style="color:#818cf8"></i>
                <strong style="color:#f1f5f9">{{ pacienteSeleccionado.nombre_completo }}</strong>
                <span style="color:#94a3b8;font-size:0.82rem">{{ pacienteSeleccionado.identificacion }}</span>
              </div>
            }
          </div>

          <mat-divider />

          <!-- Especialidad, prioridad -->
          <div class="form-section">
            <div class="section-label">Solicitud</div>
            <div class="form-grid">
              <div class="field">
                <label>Especialidad solicitada *</label>
                <p-select [(ngModel)]="nForm.especialidad_solicitada_id"
                  [options]="especialidades()"
                  optionLabel="nombre" optionValue="id"
                  [filter]="true" filterBy="nombre"
                  placeholder="Seleccione especialidad..."
                  class="w-full" style="width:100%" />
              </div>
              <div class="field">
                <label>Prioridad</label>
                <p-select [(ngModel)]="nForm.prioridad"
                  [options]="opcionesPrioridad.slice(1)"
                  optionLabel="label" optionValue="value"
                  class="w-full" style="width:100%" />
              </div>
            </div>

            <div class="field" style="margin-top:1rem">
              <label>Motivo de la interconsulta *</label>
              <textarea pTextarea [(ngModel)]="nForm.motivo" rows="3" class="w-full"
                placeholder="Pregunta clínica, motivo de consulta al especialista..."></textarea>
            </div>

            <div class="field" style="margin-top:1rem">
              <label>Resumen de historia clínica</label>
              <textarea pTextarea [(ngModel)]="nForm.historia_clinica_resumen" rows="5" class="w-full"
                placeholder="Antecedentes relevantes, diagnósticos actuales, medicamentos, exámenes recientes..."></textarea>
            </div>
          </div>

          <div class="step-actions">
            <p-button label="Cancelar" [text]="true" (onClick)="volverLista()" />
            <p-button label="Enviar interconsulta" icon="pi pi-send"
              [loading]="guardando()"
              [disabled]="!pacienteSeleccionado || !nForm.especialidad_solicitada_id || !nForm.motivo"
              (onClick)="crear()" />
          </div>
        </div>
      }

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA DETALLE / RESPUESTA                 -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'detalle' && icActual()) {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <p-button icon="pi pi-arrow-left" [text]="true" (onClick)="volverLista()" />
          <h2 class="page-title" style="margin:0">Detalle de Interconsulta</h2>
          <div style="margin-left:auto;display:flex;gap:0.5rem">
            @if (icActual()!.estado === 'SOLICITADA') {
              <p-button label="Aceptar" icon="pi pi-check" severity="info"
                [loading]="accionando()" (onClick)="aceptar()" />
              <p-button label="Cancelar solicitud" icon="pi pi-times" severity="danger"
                [text]="true" [loading]="accionando()" (onClick)="cancelar()" />
            }
          </div>
        </div>

        <!-- Header info -->
        <div class="info-header">
          <div class="info-block">
            <div class="info-label">Paciente</div>
            <div class="info-name">{{ icActual()!.paciente_nombre || '—' }}</div>
            <div class="info-row">{{ icActual()!.paciente_identificacion }}</div>
          </div>
          <div class="info-block sep">
            <div class="info-label">Especialidad solicitada</div>
            <div class="info-name">{{ icActual()!.especialidad_nombre || '—' }}</div>
          </div>
          <div class="info-block sep">
            <div class="info-label">Médico solicitante</div>
            <div class="info-name" style="font-size:0.95rem">{{ icActual()!.medico_solicita_nombre || '—' }}</div>
            <div class="info-row">{{ icActual()!.fecha_solicitud | date:'dd/MM/yyyy HH:mm' }}</div>
          </div>
          <div class="info-block sep" style="flex:0 0 auto">
            <div class="info-label">Prioridad</div>
            <div style="margin-top:0.25rem">
              <x-etiqueta [value]="icActual()!.prioridad" [severity]="prioridadSeverity(icActual()!.prioridad)" />
            </div>
            <div style="margin-top:0.4rem">
              <x-etiqueta [value]="estadoLabel(icActual()!.estado)" [severity]="estadoSeverity(icActual()!.estado)" />
            </div>
          </div>
        </div>

        <!-- Solicitud -->
        <div class="detalle-section">
          <div class="section-label">Motivo de la interconsulta</div>
          <div class="texto-bloque">{{ icActual()!.motivo }}</div>
        </div>

        @if (icActual()!.historia_clinica_resumen) {
          <div class="detalle-section">
            <div class="section-label">Resumen de historia clínica</div>
            <div class="texto-bloque">{{ icActual()!.historia_clinica_resumen }}</div>
          </div>
        }

        <mat-divider />

        <!-- Respuesta -->
        @if (icActual()!.estado === 'RESPONDIDA') {
          <div class="detalle-section">
            <div class="section-label" style="color:#4ade80">Respuesta del especialista</div>
            @if (icActual()!.medico_responde_nombre) {
              <div style="font-size:0.82rem;color:#94a3b8;margin-bottom:0.5rem">
                Dr. {{ icActual()!.medico_responde_nombre }} · {{ icActual()!.fecha_respuesta | date:'dd/MM/yyyy HH:mm' }}
              </div>
            }
            <div class="texto-bloque">{{ icActual()!.respuesta }}</div>
          </div>
          @if (icActual()!.recomendaciones) {
            <div class="detalle-section">
              <div class="section-label">Recomendaciones</div>
              <div class="texto-bloque">{{ icActual()!.recomendaciones }}</div>
            </div>
          }
        } @else if (icActual()!.estado === 'CANCELADA') {
          <div style="text-align:center;padding:1.5rem;color:#f87171">
            <i class="pi pi-times-circle" style="font-size:1.5rem"></i>
            <p>Interconsulta cancelada</p>
          </div>
        } @else {
          <!-- Formulario de respuesta -->
          <div class="form-section">
            <div class="section-label">
              @if (icActual()!.estado === 'ACEPTADA') { Responder interconsulta }
              @else { Responder (aceptar primero para habilitar) }
            </div>
            <div class="field" style="margin-bottom:1rem">
              <label>Respuesta del especialista *</label>
              <textarea pTextarea [(ngModel)]="rForm.respuesta" rows="5" class="w-full"
                placeholder="Evaluación clínica, hallazgos, impresión diagnóstica..."
                [disabled]="icActual()!.estado !== 'ACEPTADA'"></textarea>
            </div>
            <div class="field" style="margin-bottom:1rem">
              <label>Recomendaciones</label>
              <textarea pTextarea [(ngModel)]="rForm.recomendaciones" rows="3" class="w-full"
                placeholder="Plan sugerido, medicamentos, seguimiento, próxima cita..."
                [disabled]="icActual()!.estado !== 'ACEPTADA'"></textarea>
            </div>
            <div class="step-actions" style="padding-top:0.75rem">
              <p-button label="Guardar respuesta" icon="pi pi-check-circle"
                severity="success" [loading]="respondiendo()"
                [disabled]="!rForm.respuesta || icActual()!.estado !== 'ACEPTADA'"
                (onClick)="responder()" />
            </div>
          </div>
        }
      }

    </div>
  `,
  styles: [`
    .page-card { padding: 1.5rem; }
    .page-title { font-size: 1.3rem; font-weight: 700; color: #f1f5f9; margin: 0 0 1.5rem; }
    .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
    .list-header .page-title { margin-bottom: 0; }
    .cargando { padding: 3rem; text-align: center; color: #64748b; display: flex; align-items: center; justify-content: center; gap: 0.5rem; }
    .sin-resultados { text-align: center; padding: 3rem 1rem; color: #64748b; display: flex; flex-direction: column; align-items: center; gap: 0.75rem; }

    .filtros-bar { display: flex; align-items: center; gap: 0.75rem; margin-bottom: 1rem; flex-wrap: wrap; }
    .tab-bar { display: flex; gap: 0.25rem; }
    .tab-btn {
      background: none; border: none; color: #94a3b8; cursor: pointer;
      padding: 0.4rem 0.9rem; font-size: 0.85rem; border-radius: 6px; transition: all 0.15s;
    }
    .tab-btn:hover { background: rgba(255,255,255,0.06); color: #e2e8f0; }
    .tab-btn.active { background: rgba(99,102,241,0.15); color: #818cf8; font-weight: 600; }

    /* Form */
    .form-card {
      background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.07);
      border-radius: 10px; padding: 1.5rem;
    }
    .form-section { margin-bottom: 0.25rem; }
    .section-label { font-size: 0.73rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em; color: #818cf8; margin-bottom: 0.75rem; }
    .form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
    .field { display: flex; flex-direction: column; gap: 0.35rem; }
    .field label { font-weight: 500; font-size: 0.875rem; color: #e2e8f0; }
    .w-full { width: 100%; }

    .paciente-chip {
      display: flex; align-items: center; gap: 0.75rem;
      background: rgba(99,102,241,0.1); border: 1px solid rgba(99,102,241,0.25);
      border-radius: 8px; padding: 0.6rem 1rem; margin-top: 0.75rem;
    }

    /* Info header */
    .info-header {
      display: flex; gap: 1.5rem; flex-wrap: wrap;
      background: #0f172a; border: 1px solid rgba(255,255,255,0.1);
      border-radius: 8px; padding: 1rem 1.25rem; margin-bottom: 1.25rem;
    }
    .info-block { display: flex; flex-direction: column; gap: 0.2rem; }
    .info-block.sep { border-left: 1px solid rgba(255,255,255,0.1); padding-left: 1.25rem; }
    .info-label { font-size: 0.7rem; font-weight: 600; text-transform: uppercase; color: #94a3b8; letter-spacing: 0.05em; }
    .info-name { font-size: 1rem; font-weight: 600; color: #f1f5f9; }
    .info-row { font-size: 0.82rem; color: #cbd5e1; }

    /* Detalle */
    .detalle-section { margin-bottom: 1.25rem; }
    .texto-bloque {
      background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.07);
      border-radius: 6px; padding: 0.875rem 1rem;
      font-size: 0.9rem; color: #cbd5e1; line-height: 1.6; white-space: pre-wrap;
    }

    .step-actions { display: flex; gap: 0.5rem; justify-content: flex-end; border-top: 1px solid rgba(255,255,255,0.1); margin-top: 1rem; padding-top: 1rem; }

    ::ng-deep textarea { resize: vertical; }
  `]
})
export class InterconsultasComponent implements OnInit {
  private svc = inject(InterconsultaService);
  private messageService = inject(Avisos);
  private confirmationService = inject(Confirmar);

  vista = signal<Vista>('lista');
  tabActivo = signal<'todas' | 'mias' | 'para_mi'>('todas');
  cargando = signal(false);
  interconsultas = signal<Interconsulta[]>([]);
  filtroEstado = '';
  filtroPrioridad = '';
  especialidades = signal<any[]>([]);

  readonly opcionesEstado = [
    { label: 'Todos', value: '' },
    { label: 'Solicitada', value: 'SOLICITADA' },
    { label: 'Aceptada', value: 'ACEPTADA' },
    { label: 'Respondida', value: 'RESPONDIDA' },
    { label: 'Cancelada', value: 'CANCELADA' },
  ];

  readonly opcionesPrioridad = [
    { label: 'Todas', value: '' },
    { label: 'Urgente', value: 'URGENTE' },
    { label: 'Normal', value: 'NORMAL' },
    { label: 'Diferible', value: 'DIFERIBLE' },
  ];

  // Nueva
  busquedaPaciente = '';
  buscandoPaciente = signal(false);
  resultadosPaciente = signal<any[]>([]);
  pacienteSeleccionado: any = null;
  guardando = signal(false);
  nForm: Partial<InterconsultaCreate> = { prioridad: 'NORMAL' };

  // Detalle
  icActual = signal<Interconsulta | null>(null);
  accionando = signal(false);
  respondiendo = signal(false);
  rForm: Partial<InterconsultaResponder> = {};

  ngOnInit(): void {
    this.cargar();
    this.svc.listEspecialidades().subscribe({ next: es => this.especialidades.set(es) });
  }

  cargar(): void {
    this.cargando.set(true);
    const params: any = {};
    if (this.filtroEstado) params.estado = this.filtroEstado;
    if (this.filtroPrioridad) params.prioridad = this.filtroPrioridad;
    if (this.tabActivo() === 'mias') params.solo_mias = true;
    if (this.tabActivo() === 'para_mi') params.solo_para_mi = true;
    this.svc.list(params).subscribe({
      next: ics => { this.interconsultas.set(ics); this.cargando.set(false); },
      error: () => { this.cargando.set(false); this.err('Error al cargar interconsultas'); },
    });
  }

  cambiarTab(tab: 'todas' | 'mias' | 'para_mi'): void {
    this.tabActivo.set(tab);
    this.cargar();
  }

  abrirNueva(): void {
    this.busquedaPaciente = '';
    this.resultadosPaciente.set([]);
    this.pacienteSeleccionado = null;
    this.nForm = { prioridad: 'NORMAL' };
    this.vista.set('nueva');
  }

  abrirDetalle(ic: Interconsulta): void {
    this.icActual.set(ic);
    this.rForm = {};
    this.vista.set('detalle');
  }

  volverLista(): void {
    this.vista.set('lista');
    this.cargar();
  }

  buscarPaciente(): void {
    if (!this.busquedaPaciente.trim()) return;
    this.buscandoPaciente.set(true);
    this.svc.searchPacientes(this.busquedaPaciente).subscribe({
      next: ps => {
        this.resultadosPaciente.set(ps);
        this.buscandoPaciente.set(false);
        if (!ps.length) this.err('No se encontraron pacientes');
      },
      error: () => { this.buscandoPaciente.set(false); this.err('Error al buscar pacientes'); },
    });
  }

  crear(): void {
    if (!this.pacienteSeleccionado || !this.nForm.especialidad_solicitada_id || !this.nForm.motivo) return;
    this.guardando.set(true);
    const payload: InterconsultaCreate = {
      paciente_id: this.pacienteSeleccionado.id,
      especialidad_solicitada_id: this.nForm.especialidad_solicitada_id!,
      prioridad: this.nForm.prioridad || 'NORMAL',
      motivo: this.nForm.motivo!,
      historia_clinica_resumen: this.nForm.historia_clinica_resumen,
    };
    this.svc.create(payload).subscribe({
      next: () => {
        this.guardando.set(false);
        this.ok('Interconsulta enviada', `Solicitud a ${this.especialidades().find(e => e.id === payload.especialidad_solicitada_id)?.nombre}`);
        this.volverLista();
      },
      error: err => { this.guardando.set(false); this.err(err?.error?.detail ?? 'Error al crear'); },
    });
  }

  aceptar(): void {
    const ic = this.icActual();
    if (!ic) return;
    this.accionando.set(true);
    this.svc.aceptar(ic.id).subscribe({
      next: updated => { this.icActual.set(updated); this.accionando.set(false); this.ok('Interconsulta aceptada'); },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error'); },
    });
  }

  responder(): void {
    const ic = this.icActual();
    if (!ic || !this.rForm.respuesta) return;
    this.respondiendo.set(true);
    this.svc.responder(ic.id, { respuesta: this.rForm.respuesta, recomendaciones: this.rForm.recomendaciones }).subscribe({
      next: updated => {
        this.icActual.set(updated);
        this.respondiendo.set(false);
        this.ok('Respuesta registrada', 'Interconsulta completada');
      },
      error: err => { this.respondiendo.set(false); this.err(err?.error?.detail ?? 'Error'); },
    });
  }

  cancelar(): void {
    const ic = this.icActual();
    if (!ic) return;
    this.confirmationService.confirm({
      message: '¿Cancelar esta interconsulta?',
      header: 'Confirmar cancelación',
      icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Cancelar solicitud',
      rejectLabel: 'No',
      peligroso: true,
      accept: () => {
        this.accionando.set(true);
        this.svc.cancelar(ic.id).subscribe({
          next: updated => { this.icActual.set(updated); this.accionando.set(false); this.ok('Interconsulta cancelada'); },
          error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error'); },
        });
      },
    });
  }

  estadoLabel(estado: string): string {
    const map: Record<string, string> = {
      SOLICITADA: 'Solicitada', ACEPTADA: 'Aceptada',
      RESPONDIDA: 'Respondida', CANCELADA: 'Cancelada',
    };
    return map[estado] ?? estado;
  }

  estadoSeverity(estado: string): 'info' | 'success' | 'warn' | 'danger' | 'secondary' {
    if (estado === 'RESPONDIDA') return 'success';
    if (estado === 'ACEPTADA') return 'info';
    if (estado === 'CANCELADA') return 'danger';
    return 'warn';
  }

  prioridadSeverity(p: string): 'info' | 'success' | 'warn' | 'danger' | 'secondary' {
    if (p === 'URGENTE') return 'danger';
    if (p === 'DIFERIBLE') return 'secondary';
    return 'info';
  }

  private ok(summary: string, detail?: string): void {
    this.messageService.add({ severity: 'success', summary, detail });
  }

  private err(detail: string): void {
    this.messageService.add({ severity: 'error', summary: 'Error', detail });
  }
}
