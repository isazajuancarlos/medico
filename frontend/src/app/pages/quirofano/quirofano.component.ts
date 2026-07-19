import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { TextareaModule } from 'primeng/textarea';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { ToastModule } from 'primeng/toast';
import { SelectModule } from 'primeng/select';
import { TooltipModule } from 'primeng/tooltip';
import { DialogModule } from 'primeng/dialog';
import { DividerModule } from 'primeng/divider';
import { CheckboxModule } from 'primeng/checkbox';
import { MessageService } from 'primeng/api';
import {
  QuirofanoService, Sala, Programacion, ProgramacionCreate,
  ChecklistUpdate, CompletarCirugia, MedicoBasic,
} from '../../services/quirofano.service';

type Vista = 'lista' | 'programar';
type Tab = 'agenda' | 'salas';

@Component({
  selector: 'app-quirofano',
  standalone: true,
  providers: [MessageService],
  imports: [
    CommonModule, FormsModule,
    ButtonModule, InputTextModule, TextareaModule, TableModule,
    TagModule, ToastModule, SelectModule, TooltipModule,
    DialogModule, DividerModule, CheckboxModule,
  ],
  template: `
    <div class="page-card">

      <!-- ═══════════════════════════ VISTA LISTA ═══════════════════════════ -->
      @if (vista() === 'lista') {
        <div class="list-header">
          <h2 class="page-title">Quirófano</h2>
          <div style="display:flex;gap:0.5rem;align-items:center">
            <p-select [(ngModel)]="filtroEstado" [options]="opcionesEstado"
              optionLabel="label" optionValue="value"
              (onChange)="cargarProgramaciones()"
              placeholder="Todos los estados" style="width:200px" />
            <p-button label="Nueva programación" icon="pi pi-plus"
              (onClick)="vista.set('programar')" />
          </div>
        </div>

        <!-- Stats -->
        <div class="stats-row">
          <div class="stat-card">
            <div class="stat-value">{{ countEstado('PROGRAMADA') }}</div>
            <div class="stat-label">Programadas</div>
          </div>
          <div class="stat-card stat-yellow">
            <div class="stat-value">{{ countEstado('EN_CURSO') }}</div>
            <div class="stat-label">En curso</div>
          </div>
          <div class="stat-card stat-green">
            <div class="stat-value">{{ countEstado('COMPLETADA') }}</div>
            <div class="stat-label">Completadas</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">{{ salasDisponibles() }}</div>
            <div class="stat-label">Salas disponibles</div>
          </div>
        </div>

        <!-- Tabs -->
        <div class="tab-bar">
          <button class="tab-btn" [class.active]="tab() === 'agenda'"
            (click)="tab.set('agenda')">
            <i class="pi pi-calendar"></i> Agenda quirúrgica
          </button>
          <button class="tab-btn" [class.active]="tab() === 'salas'"
            (click)="tab.set('salas')">
            <i class="pi pi-building"></i> Salas
          </button>
        </div>

        <!-- Tab Agenda -->
        @if (tab() === 'agenda') {
          @if (cargando()) {
            <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
          } @else if (programaciones().length === 0) {
            <div class="sin-resultados">
              <i class="pi pi-inbox" style="font-size:2rem;opacity:0.4"></i>
              <p>No hay cirugías con el filtro seleccionado.</p>
            </div>
          } @else {
            <p-table [value]="programaciones()" [rowHover]="true" styleClass="p-datatable-sm">
              <ng-template pTemplate="header">
                <tr>
                  <th>Paciente</th>
                  <th>Procedimiento</th>
                  <th>Sala</th>
                  <th>Cirujano</th>
                  <th>Fecha / Hora</th>
                  <th style="text-align:center">Duración</th>
                  <th style="text-align:center">Checklist</th>
                  <th style="text-align:center">Estado</th>
                  <th style="text-align:center">Acciones</th>
                </tr>
              </ng-template>
              <ng-template pTemplate="body" let-p>
                <tr>
                  <td>
                    <strong>{{ p.paciente_nombre || '—' }}</strong>
                    @if (p.paciente_identificacion) {
                      <div style="font-size:0.78rem;color:#94a3b8">{{ p.paciente_identificacion }}</div>
                    }
                  </td>
                  <td>
                    <span style="font-size:0.88rem">{{ p.procedimiento }}</span>
                    @if (p.codigo_cups) {
                      <div style="font-size:0.75rem;color:#94a3b8">CUPS: {{ p.codigo_cups }}</div>
                    }
                  </td>
                  <td>
                    <span class="sala-badge">{{ p.quirofano_nombre || '—' }}</span>
                  </td>
                  <td style="font-size:0.85rem">{{ p.cirujano_nombre || '—' }}</td>
                  <td style="font-size:0.85rem">
                    {{ p.fecha_programada | date:'dd/MM/yyyy' }}
                    <div style="font-size:0.78rem;color:#94a3b8">{{ p.fecha_programada | date:'HH:mm' }}</div>
                  </td>
                  <td style="text-align:center">
                    <span class="dur-badge">{{ p.duracion_estimada_min }} min</span>
                  </td>
                  <td style="text-align:center">
                    <span [class]="checklistCompleto(p) ? 'check-ok' : 'check-pendiente'">
                      <i [class]="checklistCompleto(p) ? 'pi pi-check-circle' : 'pi pi-exclamation-circle'"></i>
                      {{ checklistItems(p) }}/4
                    </span>
                  </td>
                  <td style="text-align:center">
                    <p-tag [value]="estadoLabel(p.estado)" [severity]="estadoSeverity(p.estado)" />
                  </td>
                  <td style="text-align:center">
                    <p-button icon="pi pi-eye" [text]="true" size="small"
                      pTooltip="Ver detalle" (onClick)="abrirDetalle(p)" />
                    @if (p.estado === 'PROGRAMADA') {
                      <p-button icon="pi pi-play" [text]="true" size="small"
                        severity="success" pTooltip="Iniciar cirugía"
                        (onClick)="iniciarCirugia(p)" />
                    }
                    @if (p.estado === 'EN_CURSO') {
                      <p-button icon="pi pi-stop-circle" [text]="true" size="small"
                        severity="warn" pTooltip="Completar cirugía"
                        (onClick)="abrirCompletar(p)" />
                    }
                  </td>
                </tr>
              </ng-template>
            </p-table>
          }
        }

        <!-- Tab Salas -->
        @if (tab() === 'salas') {
          @if (cargandoSalas()) {
            <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando salas...</div>
          } @else if (salas().length === 0) {
            <div class="sin-resultados">
              <i class="pi pi-inbox" style="font-size:2rem;opacity:0.4"></i>
              <p>No hay salas de quirófano registradas.</p>
            </div>
          } @else {
            <div class="salas-grid">
              @for (sala of salas(); track sala.id) {
                <div class="sala-card" [class]="'sala-' + sala.estado.toLowerCase()">
                  <div class="sala-icono"><i class="pi pi-building"></i></div>
                  <div class="sala-nombre">{{ sala.nombre }}</div>
                  <div class="sala-tipo">{{ sala.tipo }}</div>
                  <div class="sala-estado">{{ sala.estado.replace('_', ' ') }}</div>
                </div>
              }
            </div>
          }
        }
      }

      <!-- ═══════════════════════════ VISTA PROGRAMAR ═══════════════════════ -->
      @if (vista() === 'programar') {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <p-button icon="pi pi-arrow-left" [text]="true" (onClick)="volverLista()" pTooltip="Volver" />
          <h2 class="page-title" style="margin:0">Nueva Programación Quirúrgica</h2>
        </div>

        <div class="form-card">

          <!-- Paciente -->
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
                <p-button icon="pi pi-times" [text]="true" size="small"
                  severity="secondary" (onClick)="pacienteSeleccionado = null" />
              </div>
            }
          </div>

          <p-divider />

          <!-- Procedimiento -->
          <div class="form-section">
            <div class="section-title">Procedimiento quirúrgico</div>
            <div class="form-grid">
              <div class="field" style="grid-column:1/-1">
                <label>Descripción del procedimiento *</label>
                <input pInputText [(ngModel)]="nForm.procedimiento"
                  class="w-full" placeholder="Ej: Colecistectomía laparoscópica, Apendicectomía..." />
              </div>
              <div class="field">
                <label>Código CUPS</label>
                <input pInputText [(ngModel)]="nForm.codigo_cups"
                  class="w-full" placeholder="Ej: 472200" />
              </div>
              <div class="field">
                <label>Tipo de anestesia</label>
                <p-select [(ngModel)]="nForm.tipo_anestesia"
                  [options]="opcionesAnestesia"
                  optionLabel="label" optionValue="value"
                  placeholder="Seleccionar..."
                  class="w-full" style="width:100%" />
              </div>
            </div>
          </div>

          <p-divider />

          <!-- Sala, cirujano, fecha -->
          <div class="form-section">
            <div class="section-title">Asignación</div>
            <div class="form-grid">
              <div class="field">
                <label>Sala de quirófano *</label>
                <p-select [(ngModel)]="nForm.quirofano_id"
                  [options]="salasDisp()"
                  optionLabel="label" optionValue="value"
                  [filter]="true"
                  placeholder="Seleccione sala..."
                  class="w-full" style="width:100%" />
              </div>
              <div class="field">
                <label>Cirujano *</label>
                <p-select [(ngModel)]="nForm.cirujano_id"
                  [options]="medicos()"
                  optionLabel="nombre_completo" optionValue="id"
                  [filter]="true"
                  placeholder="Seleccione cirujano..."
                  class="w-full" style="width:100%" />
              </div>
              <div class="field">
                <label>Anestesiólogo</label>
                <p-select [(ngModel)]="nForm.anestesiologo_id"
                  [options]="anestesiologos()"
                  optionLabel="nombre_completo" optionValue="id"
                  [filter]="true" [showClear]="true"
                  placeholder="Seleccione anestesiólogo..."
                  class="w-full" style="width:100%" />
              </div>
              <div class="field">
                <label>Duración estimada (min)</label>
                <input pInputText type="number" [(ngModel)]="nForm.duracion_estimada_min"
                  class="w-full" placeholder="60" />
              </div>
              <div class="field" style="grid-column:1/-1">
                <label>Fecha y hora programada *</label>
                <input type="datetime-local" [(ngModel)]="nForm.fecha_programada"
                  class="w-full datetime-input" />
              </div>
            </div>
          </div>

          <div class="step-actions">
            <p-button label="Cancelar" [text]="true" (onClick)="volverLista()" />
            <p-button label="Programar cirugía" icon="pi pi-check"
              [loading]="guardando()"
              [disabled]="!pacienteSeleccionado || !nForm.quirofano_id || !nForm.cirujano_id || !nForm.procedimiento || !nForm.fecha_programada"
              (onClick)="guardar()" />
          </div>
        </div>
      }
    </div>

    <!-- ══ DIALOG DETALLE ══ -->
    <p-dialog header="Detalle de Cirugía" [(visible)]="mostrarDetalle"
      [style]="{width:'600px'}" [modal]="true" [closable]="true">
      @if (progDetalle()) {
        <div class="detalle-header">
          <div>
            <div class="det-label">Paciente</div>
            <div class="det-valor">{{ progDetalle()!.paciente_nombre || '—' }}</div>
            <div class="det-sub">{{ progDetalle()!.paciente_identificacion }}</div>
          </div>
          <p-tag [value]="estadoLabel(progDetalle()!.estado)"
            [severity]="estadoSeverity(progDetalle()!.estado)" />
        </div>

        <p-divider />

        <div class="det-grid">
          <div>
            <div class="det-label">Procedimiento</div>
            <div class="det-valor">{{ progDetalle()!.procedimiento }}</div>
          </div>
          <div>
            <div class="det-label">Sala</div>
            <div class="det-valor">{{ progDetalle()!.quirofano_nombre || '—' }}</div>
          </div>
          <div>
            <div class="det-label">Cirujano</div>
            <div class="det-valor">{{ progDetalle()!.cirujano_nombre || '—' }}</div>
          </div>
          <div>
            <div class="det-label">Anestesiólogo</div>
            <div class="det-valor">{{ progDetalle()!.anestesiologo_nombre || 'No asignado' }}</div>
          </div>
          <div>
            <div class="det-label">Fecha programada</div>
            <div class="det-valor">{{ progDetalle()!.fecha_programada | date:'dd/MM/yyyy HH:mm' }}</div>
          </div>
          <div>
            <div class="det-label">Duración estimada</div>
            <div class="det-valor">{{ progDetalle()!.duracion_estimada_min }} min</div>
          </div>
          @if (progDetalle()!.tipo_anestesia) {
            <div>
              <div class="det-label">Anestesia</div>
              <div class="det-valor">{{ progDetalle()!.tipo_anestesia }}</div>
            </div>
          }
          @if (progDetalle()!.hora_inicio_real) {
            <div>
              <div class="det-label">Hora inicio real</div>
              <div class="det-valor">{{ progDetalle()!.hora_inicio_real | date:'HH:mm' }}</div>
            </div>
          }
          @if (progDetalle()!.hora_fin_real) {
            <div>
              <div class="det-label">Hora fin real</div>
              <div class="det-valor">{{ progDetalle()!.hora_fin_real | date:'HH:mm' }}</div>
            </div>
          }
        </div>

        <!-- Checklist -->
        @if (progDetalle()!.estado === 'PROGRAMADA') {
          <p-divider />
          <div class="section-title" style="margin-bottom:0.75rem">Checklist prequirúrgico</div>
          <div class="checklist-grid">
            <div class="checklist-item">
              <p-checkbox [(ngModel)]="detChecklist.ayuno_verificado" [binary]="true"
                (onChange)="guardarChecklist('ayuno_verificado', $event.checked)" />
              <span [class.check-text-ok]="detChecklist.ayuno_verificado">Ayuno verificado</span>
            </div>
            <div class="checklist-item">
              <p-checkbox [(ngModel)]="detChecklist.labs_prequirurgicos" [binary]="true"
                (onChange)="guardarChecklist('labs_prequirurgicos', $event.checked)" />
              <span [class.check-text-ok]="detChecklist.labs_prequirurgicos">Laboratorios prequirúrgicos</span>
            </div>
            <div class="checklist-item">
              <p-checkbox [(ngModel)]="detChecklist.valoracion_preanestesica" [binary]="true"
                (onChange)="guardarChecklist('valoracion_preanestesica', $event.checked)" />
              <span [class.check-text-ok]="detChecklist.valoracion_preanestesica">Valoración preanestésica</span>
            </div>
            <div class="checklist-item">
              <p-checkbox [(ngModel)]="detChecklist.reserva_sangre" [binary]="true"
                (onChange)="guardarChecklist('reserva_sangre', $event.checked)" />
              <span [class.check-text-ok]="detChecklist.reserva_sangre">Reserva de sangre</span>
            </div>
          </div>
        }

        <!-- Nota quirúrgica (completada) -->
        @if (progDetalle()!.nota_quirurgica) {
          <p-divider />
          <div class="det-label">Nota quirúrgica</div>
          <div class="nota-box">{{ progDetalle()!.nota_quirurgica }}</div>
        }
        @if (progDetalle()!.hallazgos_quirurgicos) {
          <div class="det-label" style="margin-top:0.75rem">Hallazgos</div>
          <div class="nota-box">{{ progDetalle()!.hallazgos_quirurgicos }}</div>
        }
        @if (progDetalle()!.complicaciones) {
          <div class="det-label" style="margin-top:0.75rem">Complicaciones / Notas</div>
          <div class="nota-box warn">{{ progDetalle()!.complicaciones }}</div>
        }

        <!-- Acciones -->
        <div style="display:flex;justify-content:space-between;gap:0.5rem;padding-top:1rem;border-top:1px solid rgba(255,255,255,0.1);margin-top:1rem">
          <div>
            @if (progDetalle()!.estado === 'PROGRAMADA') {
              <p-button label="Cancelar cirugía" icon="pi pi-times" [text]="true"
                severity="danger" (onClick)="cancelarCirugia()" [loading]="accionando()" />
            }
          </div>
          <div style="display:flex;gap:0.5rem">
            <p-button label="Cerrar" [text]="true" (onClick)="mostrarDetalle = false" />
            @if (progDetalle()!.estado === 'PROGRAMADA') {
              <p-button label="Iniciar cirugía" icon="pi pi-play"
                severity="success" (onClick)="iniciarDesdeDetalle()" [loading]="accionando()" />
            }
          </div>
        </div>
      }
    </p-dialog>

    <!-- ══ DIALOG COMPLETAR ══ -->
    <p-dialog header="Completar Cirugía" [(visible)]="mostrarCompletar"
      [style]="{width:'560px'}" [modal]="true" [closable]="true">
      @if (progCompletar()) {
        <div style="margin-bottom:1rem;padding:0.75rem;background:rgba(99,102,241,0.1);border-radius:6px;border:1px solid rgba(99,102,241,0.2)">
          <div style="font-weight:600;color:#f1f5f9">{{ progCompletar()!.paciente_nombre }}</div>
          <div style="font-size:0.82rem;color:#94a3b8">{{ progCompletar()!.procedimiento }}</div>
        </div>
        <div class="form-grid">
          <div class="field" style="grid-column:1/-1">
            <label class="field-label">Nota quirúrgica / descripción del procedimiento</label>
            <textarea pTextarea [(ngModel)]="cForm.nota_quirurgica" rows="4" class="w-full"
              placeholder="Descripción de la técnica quirúrgica..."></textarea>
          </div>
          <div class="field" style="grid-column:1/-1">
            <label class="field-label">Hallazgos quirúrgicos</label>
            <textarea pTextarea [(ngModel)]="cForm.hallazgos_quirurgicos" rows="3" class="w-full"
              placeholder="Hallazgos intraoperatorios..."></textarea>
          </div>
          <div class="field">
            <label class="field-label">Sangrado estimado (mL)</label>
            <input pInputText type="number" [(ngModel)]="cForm.sangrado_estimado_ml"
              class="w-full" placeholder="0" />
          </div>
          <div class="field">
            <label class="field-label">Especímenes para patología</label>
            <input pInputText [(ngModel)]="cForm.especimenes_patologia"
              class="w-full" placeholder="Vesícula biliar, apéndice..." />
          </div>
          <div class="field" style="grid-column:1/-1">
            <label class="field-label">Complicaciones intraoperatorias</label>
            <textarea pTextarea [(ngModel)]="cForm.complicaciones" rows="2" class="w-full"
              placeholder="Ninguna / descripción..."></textarea>
          </div>
        </div>
        <div style="display:flex;justify-content:flex-end;gap:0.5rem;padding-top:0.75rem;border-top:1px solid rgba(255,255,255,0.1)">
          <p-button label="Cancelar" [text]="true" (onClick)="mostrarCompletar = false" />
          <p-button label="Registrar cierre" icon="pi pi-check"
            severity="warn" [loading]="accionando()"
            (onClick)="confirmarCompletar()" />
        </div>
      }
    </p-dialog>

    <p-toast />
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
    .stat-card.stat-yellow { border-color: rgba(251,191,36,0.2); background: rgba(251,191,36,0.05); }
    .stat-card.stat-green { border-color: rgba(74,222,128,0.2); background: rgba(74,222,128,0.05); }
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

    .sala-badge {
      display: inline-block; padding: 0.15rem 0.5rem; border-radius: 4px;
      background: rgba(99,102,241,0.15); color: #a5b4fc;
      font-size: 0.82rem; font-weight: 600; border: 1px solid rgba(99,102,241,0.25);
    }
    .dur-badge {
      display: inline-block; padding: 0.15rem 0.5rem; border-radius: 999px;
      background: rgba(255,255,255,0.06); color: #e2e8f0; font-size: 0.78rem;
    }
    .check-ok { color: #4ade80; font-size: 0.82rem; display: flex; align-items: center; gap: 0.3rem; }
    .check-pendiente { color: #fb923c; font-size: 0.82rem; display: flex; align-items: center; gap: 0.3rem; }

    .salas-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 0.75rem; margin-top: 0.5rem; }
    .sala-card {
      border-radius: 10px; padding: 1.25rem;
      border: 1px solid rgba(255,255,255,0.1);
      background: rgba(255,255,255,0.03);
      display: flex; flex-direction: column; align-items: center; gap: 0.35rem; text-align: center;
    }
    .sala-disponible { border-color: rgba(74,222,128,0.3); background: rgba(74,222,128,0.06); }
    .sala-en_uso { border-color: rgba(248,113,113,0.35); background: rgba(248,113,113,0.08); }
    .sala-mantenimiento, .sala-bloqueado { border-color: rgba(148,163,184,0.2); }
    .sala-icono { font-size: 1.5rem; color: #818cf8; }
    .sala-nombre { font-size: 0.95rem; font-weight: 700; color: #f1f5f9; }
    .sala-tipo { font-size: 0.72rem; color: #94a3b8; }
    .sala-estado { font-size: 0.68rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; color: #64748b; }
    .sala-disponible .sala-estado { color: #4ade80; }
    .sala-en_uso .sala-estado { color: #f87171; }

    .form-card {
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
    .datetime-input {
      background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.15);
      border-radius: 6px; padding: 0.5rem 0.75rem; color: #f1f5f9; font-size: 0.875rem;
      color-scheme: dark;
    }

    .detalle-header { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 0.5rem; }
    .det-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 0.75rem 1.5rem; }
    .det-label { font-size: 0.72rem; color: #64748b; text-transform: uppercase; letter-spacing: 0.05em; }
    .det-valor { font-size: 0.9rem; color: #f1f5f9; font-weight: 500; margin-top: 0.1rem; }
    .det-sub { font-size: 0.78rem; color: #94a3b8; }

    .checklist-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem 1rem; }
    .checklist-item { display: flex; align-items: center; gap: 0.5rem; font-size: 0.875rem; color: #cbd5e1; }
    .check-text-ok { color: #4ade80; }

    .nota-box {
      background: rgba(255,255,255,0.04); border: 1px solid rgba(255,255,255,0.08);
      border-radius: 6px; padding: 0.75rem; font-size: 0.85rem; color: #cbd5e1;
      white-space: pre-wrap; margin-top: 0.25rem;
    }
    .nota-box.warn { border-color: rgba(251,191,36,0.2); background: rgba(251,191,36,0.05); color: #fde68a; }

    ::ng-deep textarea { resize: vertical; }
  `]
})
export class QuirofanoComponent implements OnInit {
  private svc = inject(QuirofanoService);
  private messageService = inject(MessageService);

  vista = signal<Vista>('lista');
  tab = signal<Tab>('agenda');
  filtroEstado = '';

  cargando = signal(false);
  cargandoSalas = signal(false);
  programaciones = signal<Programacion[]>([]);
  salas = signal<Sala[]>([]);
  medicos = signal<MedicoBasic[]>([]);

  salasDisponibles = () => this.salas().filter(s => s.estado === 'DISPONIBLE').length;
  salasDisp = () => this.salas()
    .filter(s => s.estado !== 'MANTENIMIENTO' && s.estado !== 'BLOQUEADO')
    .map(s => ({ label: `${s.nombre} — ${s.tipo}`, value: s.id }));
  anestesiologos = () => this.medicos().filter(m => m.rol === 'anestesiologo');
  countEstado = (e: string) => this.programaciones().filter(p => p.estado === e).length;

  readonly opcionesEstado = [
    { label: 'Todos', value: '' },
    { label: 'Programadas', value: 'PROGRAMADA' },
    { label: 'En curso', value: 'EN_CURSO' },
    { label: 'Completadas', value: 'COMPLETADA' },
    { label: 'Canceladas', value: 'CANCELADA' },
    { label: 'Suspendidas', value: 'SUSPENDIDA' },
  ];

  readonly opcionesAnestesia = [
    { label: 'General', value: 'GENERAL' },
    { label: 'Regional', value: 'REGIONAL' },
    { label: 'Local', value: 'LOCAL' },
    { label: 'Sedación', value: 'SEDACION' },
  ];

  // Formulario nueva programación
  busquedaPaciente = '';
  buscandoPaciente = signal(false);
  pacientesResultados = signal<any[]>([]);
  pacienteSeleccionado: any = null;
  guardando = signal(false);
  nForm: Partial<ProgramacionCreate> = { duracion_estimada_min: 60 };

  // Detalle dialog
  mostrarDetalle = false;
  progDetalle = signal<Programacion | null>(null);
  detChecklist: ChecklistUpdate = {};
  accionando = signal(false);

  // Completar dialog
  mostrarCompletar = false;
  progCompletar = signal<Programacion | null>(null);
  cForm: CompletarCirugia = {};

  ngOnInit(): void {
    this.cargarProgramaciones();
    this.cargarSalas();
    this.svc.listMedicos().subscribe({ next: ms => this.medicos.set(ms) });
  }

  cargarProgramaciones(): void {
    this.cargando.set(true);
    this.svc.listProgramaciones(this.filtroEstado || undefined).subscribe({
      next: ps => { this.programaciones.set(ps); this.cargando.set(false); },
      error: () => { this.cargando.set(false); this.err('Error al cargar programaciones'); },
    });
  }

  cargarSalas(): void {
    this.cargandoSalas.set(true);
    this.svc.listSalas().subscribe({
      next: ss => { this.salas.set(ss); this.cargandoSalas.set(false); },
      error: () => this.cargandoSalas.set(false),
    });
  }

  volverLista(): void {
    this.vista.set('lista');
    this.busquedaPaciente = '';
    this.pacientesResultados.set([]);
    this.pacienteSeleccionado = null;
    this.nForm = { duracion_estimada_min: 60 };
    this.cargarProgramaciones();
    this.cargarSalas();
  }

  buscarPaciente(): void {
    if (!this.busquedaPaciente.trim()) return;
    this.buscandoPaciente.set(true);
    this.svc.searchPacientes(this.busquedaPaciente).subscribe({
      next: ps => {
        this.pacientesResultados.set(ps);
        this.buscandoPaciente.set(false);
        if (ps.length === 0) this.err('No se encontraron pacientes');
      },
      error: () => { this.buscandoPaciente.set(false); this.err('Error al buscar pacientes'); },
    });
  }

  guardar(): void {
    if (!this.pacienteSeleccionado || !this.nForm.quirofano_id || !this.nForm.cirujano_id || !this.nForm.procedimiento || !this.nForm.fecha_programada) return;
    const payload: ProgramacionCreate = {
      quirofano_id: this.nForm.quirofano_id!,
      paciente_id: this.pacienteSeleccionado.id,
      cirujano_id: this.nForm.cirujano_id!,
      anestesiologo_id: this.nForm.anestesiologo_id,
      fecha_programada: new Date(this.nForm.fecha_programada!).toISOString(),
      duracion_estimada_min: Number(this.nForm.duracion_estimada_min) || 60,
      procedimiento: this.nForm.procedimiento!,
      codigo_cups: this.nForm.codigo_cups,
      tipo_anestesia: this.nForm.tipo_anestesia,
    };
    this.guardando.set(true);
    this.svc.crear(payload).subscribe({
      next: p => {
        this.guardando.set(false);
        this.ok('Cirugía programada', `${p.procedimiento} — ${p.quirofano_nombre}`);
        this.volverLista();
      },
      error: err => {
        this.guardando.set(false);
        this.err(err?.error?.detail ?? 'Error al programar la cirugía');
      },
    });
  }

  abrirDetalle(p: Programacion): void {
    this.progDetalle.set(p);
    this.detChecklist = {
      ayuno_verificado: p.ayuno_verificado,
      labs_prequirurgicos: p.labs_prequirurgicos,
      valoracion_preanestesica: p.valoracion_preanestesica,
      reserva_sangre: p.reserva_sangre,
    };
    this.mostrarDetalle = true;
  }

  guardarChecklist(campo: keyof ChecklistUpdate, valor: boolean): void {
    const p = this.progDetalle();
    if (!p) return;
    this.svc.updateChecklist(p.id, { [campo]: valor }).subscribe({
      next: updated => {
        this.progDetalle.set(updated);
        const idx = this.programaciones().findIndex(x => x.id === updated.id);
        if (idx >= 0) {
          const arr = [...this.programaciones()];
          arr[idx] = updated;
          this.programaciones.set(arr);
        }
      },
      error: () => this.err('Error al actualizar checklist'),
    });
  }

  iniciarCirugia(p: Programacion): void {
    this.accionando.set(true);
    this.svc.iniciar(p.id).subscribe({
      next: updated => {
        this.accionando.set(false);
        this.ok('Cirugía iniciada', `${updated.paciente_nombre} — ${updated.quirofano_nombre}`);
        this.cargarProgramaciones();
        this.cargarSalas();
      },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error al iniciar'); },
    });
  }

  iniciarDesdeDetalle(): void {
    const p = this.progDetalle();
    if (!p) return;
    this.accionando.set(true);
    this.svc.iniciar(p.id).subscribe({
      next: updated => {
        this.accionando.set(false);
        this.mostrarDetalle = false;
        this.ok('Cirugía iniciada', `${updated.paciente_nombre} — ${updated.quirofano_nombre}`);
        this.cargarProgramaciones();
        this.cargarSalas();
      },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error al iniciar'); },
    });
  }

  cancelarCirugia(): void {
    const p = this.progDetalle();
    if (!p) return;
    this.accionando.set(true);
    this.svc.cancelar(p.id, 'Cancelada desde detalle').subscribe({
      next: () => {
        this.accionando.set(false);
        this.mostrarDetalle = false;
        this.ok('Cirugía cancelada');
        this.cargarProgramaciones();
        this.cargarSalas();
      },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error al cancelar'); },
    });
  }

  abrirCompletar(p: Programacion): void {
    this.progCompletar.set(p);
    this.cForm = {};
    this.mostrarCompletar = true;
  }

  confirmarCompletar(): void {
    const p = this.progCompletar();
    if (!p) return;
    this.accionando.set(true);
    this.svc.completar(p.id, this.cForm).subscribe({
      next: updated => {
        this.accionando.set(false);
        this.mostrarCompletar = false;
        this.ok('Cirugía completada', updated.paciente_nombre ?? '');
        this.cargarProgramaciones();
        this.cargarSalas();
      },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error al completar'); },
    });
  }

  checklistItems(p: Programacion): number {
    return [p.ayuno_verificado, p.labs_prequirurgicos, p.valoracion_preanestesica, p.reserva_sangre].filter(Boolean).length;
  }

  checklistCompleto(p: Programacion): boolean {
    return this.checklistItems(p) === 4;
  }

  estadoLabel(estado: string): string {
    const map: Record<string, string> = {
      PROGRAMADA: 'Programada', EN_CURSO: 'En curso', COMPLETADA: 'Completada',
      CANCELADA: 'Cancelada', SUSPENDIDA: 'Suspendida',
    };
    return map[estado] ?? estado;
  }

  estadoSeverity(estado: string): 'info' | 'success' | 'warn' | 'danger' | 'secondary' {
    if (estado === 'PROGRAMADA') return 'info';
    if (estado === 'EN_CURSO') return 'warn';
    if (estado === 'COMPLETADA') return 'success';
    if (estado === 'CANCELADA' || estado === 'SUSPENDIDA') return 'danger';
    return 'secondary';
  }

  private ok(summary: string, detail?: string): void {
    this.messageService.add({ severity: 'success', summary, detail });
  }

  private err(detail: string): void {
    this.messageService.add({ severity: 'error', summary: 'Error', detail });
  }
}
