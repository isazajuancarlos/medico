import { Component, Inject, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MAT_DIALOG_DATA, MatDialog, MatDialogModule } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatTableModule } from '@angular/material/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatDividerModule } from '@angular/material/divider';
import { Avisos } from '../../shared/ui/avisos.service';
import {
  HospitalizacionService, Hospitalizacion, Cama,
  MedicoBasic, IngresoCreate, EgresoUpdate,
} from '../../services/hospitalizacion.service';

type Vista = 'lista' | 'ingresar';
type Tab = 'pacientes' | 'camas';

const TIPOS_DE_EGRESO = [
  { label: 'Alta médica', value: 'ALTA_MEDICA' },
  { label: 'Alta voluntaria', value: 'ALTA_VOLUNTARIA' },
  { label: 'Remisión', value: 'REMISION' },
  { label: 'Defunción', value: 'DEFUNCION' },
];

/**
 * Egreso de un paciente hospitalizado. Devuelve el egreso, o nada.
 *
 * El tipo de egreso viene con «Alta médica» puesto porque es el único de los
 * cuatro que NO cambia la conducta si se elige mal por descuido: los otros tres
 * —voluntaria, remisión y defunción— tienen efecto jurídico. Aun así la cama se
 * libera con cualquiera de ellos, así que el campo se ofrece siempre abierto.
 */
@Component({
  selector: 'app-hospitalizacion-egreso',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatSelectModule,
  ],
  template: `
    <h2 mat-dialog-title>Registrar egreso</h2>
    <mat-dialog-content>
      <div class="cabecera">
        <div class="rotulo">Paciente</div>
        <div class="nombre">{{ h.paciente_nombre }}</div>
        <div class="detalle">Cama {{ h.cama_numero }} · {{ h.cama_servicio }}</div>
      </div>
      <div class="campos">
        <mat-form-field appearance="outline">
          <mat-label>Tipo de egreso *</mat-label>
          <mat-select [(ngModel)]="f.tipo_egreso">
            @for (o of tipos; track o.value) {
              <mat-option [value]="o.value">{{ o.label }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Diagnóstico de egreso (CIE-10)</mat-label>
          <input matInput [(ngModel)]="f.diagnostico_egreso" placeholder="Código CIE-10..." />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Resumen clínico del egreso</mat-label>
          <textarea matInput [(ngModel)]="f.resumen_clinico" rows="4"
            placeholder="Evolución durante la hospitalización, condición al alta..."></textarea>
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!f.tipo_egreso" [mat-dialog-close]="resultado()">
        <i class="pi pi-sign-out"></i> Registrar egreso
      </button>
    </mat-dialog-actions>
  `,
  styles: [`
    .cabecera { margin-bottom:1rem; min-width:min(28rem, 80vw); }
    .rotulo { color:#94a3b8; font-size:.78rem; text-transform:uppercase; letter-spacing:.05em; }
    .nombre { font-weight:600; color:#f1f5f9; margin-top:.2rem; }
    .detalle { font-size:.82rem; color:#94a3b8; }
    .campos { display:flex; flex-direction:column; gap:.5rem; }
    mat-form-field { width:100%; }
  `],
})
export class HospitalizacionEgresoComponent {
  readonly tipos = TIPOS_DE_EGRESO;
  f: EgresoUpdate = { tipo_egreso: 'ALTA_MEDICA' };

  constructor(@Inject(MAT_DIALOG_DATA) public h: Hospitalizacion) {}

  resultado(): EgresoUpdate | null {
    return this.f.tipo_egreso ? this.f : null;
  }
}

@Component({
  selector: 'app-hospitalizacion',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatFormFieldModule, MatInputModule, MatSelectModule,
    MatTableModule, EtiquetaComponent, MatTooltipModule, MatDividerModule,
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
            <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:170px">
              <mat-label>Estado</mat-label>
              <mat-select [(ngModel)]="filtroEstado" (selectionChange)="cargarHospitalizaciones()">
                @for (o of opcionesEstado; track o.value) {
                  <mat-option [value]="o.value">{{ o.label }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
            <button matButton="filled" (click)="vista.set('ingresar')">
              <i class="pi pi-plus"></i> Nuevo ingreso
            </button>
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
            <table mat-table [dataSource]="hospitalizaciones()" class="tabla">
              <ng-container matColumnDef="paciente">
                <th mat-header-cell *matHeaderCellDef>Paciente</th>
                <td mat-cell *matCellDef="let h">
                  <strong>{{ h.paciente_nombre || '—' }}</strong>
                  @if (h.paciente_identificacion) {
                    <div style="font-size:0.78rem;color:#94a3b8">{{ h.paciente_identificacion }}</div>
                  }
                </td>
              </ng-container>
              <ng-container matColumnDef="cama">
                <th mat-header-cell *matHeaderCellDef>Cama / Piso</th>
                <td mat-cell *matCellDef="let h">
                  <span class="cama-badge">{{ h.cama_numero || '—' }}</span>
                  @if (h.cama_piso) {
                    <div style="font-size:0.78rem;color:#94a3b8">Piso {{ h.cama_piso }}</div>
                  }
                </td>
              </ng-container>
              <ng-container matColumnDef="servicio">
                <th mat-header-cell *matHeaderCellDef>Servicio</th>
                <td mat-cell *matCellDef="let h" style="font-size:0.85rem">{{ h.cama_servicio || '—' }}</td>
              </ng-container>
              <ng-container matColumnDef="medico">
                <th mat-header-cell *matHeaderCellDef>Médico tratante</th>
                <td mat-cell *matCellDef="let h" style="font-size:0.85rem">{{ h.medico_nombre || '—' }}</td>
              </ng-container>
              <ng-container matColumnDef="ingreso">
                <th mat-header-cell *matHeaderCellDef>Ingreso</th>
                <td mat-cell *matCellDef="let h" style="font-size:0.85rem">{{ h.fecha_ingreso | date:'dd/MM/yyyy' }}</td>
              </ng-container>
              <ng-container matColumnDef="dias">
                <th mat-header-cell *matHeaderCellDef class="centro">Días</th>
                <td mat-cell *matCellDef="let h" class="centro">
                  <span class="dias-badge">{{ diasEstancia(h.fecha_ingreso) }}</span>
                </td>
              </ng-container>
              <ng-container matColumnDef="estado">
                <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
                <td mat-cell *matCellDef="let h" class="centro">
                  <x-etiqueta [value]="h.estado" [severity]="estadoSeverity(h.estado)" />
                </td>
              </ng-container>
              <ng-container matColumnDef="egreso">
                <th mat-header-cell *matHeaderCellDef class="centro">Egreso</th>
                <td mat-cell *matCellDef="let h" class="centro">
                  @if (h.estado === 'ACTIVA') {
                    <button matIconButton class="aviso-color" matTooltip="Registrar egreso"
                      (click)="abrirEgreso(h)">
                      <i class="pi pi-sign-out"></i>
                    </button>
                  } @else {
                    <span style="font-size:0.78rem;color:#64748b">{{ h.fecha_egreso | date:'dd/MM/yy' }}</span>
                  }
                </td>
              </ng-container>

              <tr mat-header-row *matHeaderRowDef="columnas"></tr>
              <tr mat-row *matRowDef="let fila; columns: columnas"></tr>
            </table>
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
          <button matIconButton matTooltip="Volver" (click)="volverLista()">
            <i class="pi pi-arrow-left"></i>
          </button>
          <h2 class="page-title" style="margin:0">Nuevo Ingreso Hospitalario</h2>
        </div>

        <div class="ingreso-form">

          <!-- Búsqueda paciente -->
          <div class="form-section">
            <div class="section-title">Paciente</div>
            <div style="display:flex;gap:0.5rem;align-items:flex-start">
              <mat-form-field appearance="outline" subscriptSizing="dynamic" style="flex:1">
                <mat-label>Buscar por nombre o identificación</mat-label>
                <input matInput [ngModel]="busquedaPaciente()" (ngModelChange)="busquedaPaciente.set($event)"
                  placeholder="Ingrese nombre o número de identificación..."
                  (keyup.enter)="buscarPaciente()" />
              </mat-form-field>
              <button matButton="filled" [disabled]="buscandoPaciente()" (click)="buscarPaciente()">
                <i [class]="buscandoPaciente() ? 'pi pi-spin pi-spinner' : 'pi pi-search'"></i> Buscar
              </button>
            </div>
            @if (pacientesResultados().length > 0) {
              <mat-form-field appearance="outline" subscriptSizing="dynamic" style="margin-top:0.75rem">
                <mat-label>Seleccionar paciente</mat-label>
                <mat-select [ngModel]="pacienteSeleccionado()" (ngModelChange)="pacienteSeleccionado.set($event)">
                  @for (p of pacientesResultados(); track p.id) {
                    <mat-option [value]="p">{{ p.nombre_completo }} — {{ p.identificacion }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
            }
            @if (pacienteSeleccionado(); as sel) {
              <div class="paciente-card">
                <i class="pi pi-user" style="color:#818cf8"></i>
                <div>
                  <div style="font-weight:600;color:#f1f5f9">{{ sel.nombre_completo }}</div>
                  <div style="font-size:0.82rem;color:#94a3b8">{{ sel.identificacion }}</div>
                </div>
                <button matIconButton matTooltip="Quitar" (click)="pacienteSeleccionado.set(null)">
                  <i class="pi pi-times"></i>
                </button>
              </div>
            }
          </div>

          <mat-divider />

          <!-- Cama y médico -->
          <div class="form-section">
            <div class="section-title">Asignación</div>
            <div class="form-grid">
              <mat-form-field appearance="outline">
                <mat-label>Cama disponible *</mat-label>
                <mat-select [(ngModel)]="iForm().cama_id">
                  @for (c of camasDisp(); track c.value) {
                    <mat-option [value]="c.value">{{ c.label }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Médico tratante *</mat-label>
                <mat-select [(ngModel)]="iForm().medico_id">
                  @for (m of medicos(); track m.id) {
                    <mat-option [value]="m.id">{{ m.nombre_completo }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Vía de ingreso</mat-label>
                <mat-select [(ngModel)]="iForm().via_ingreso">
                  @for (o of opcionesVia; track o.value) {
                    <mat-option [value]="o.value">{{ o.label }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Diagnóstico de ingreso (CIE-10)</mat-label>
                <input matInput [(ngModel)]="iForm().diagnostico_ingreso" placeholder="Ej: J18.9, Z00.0..." />
              </mat-form-field>
            </div>
          </div>

          <mat-divider />

          <!-- Notas clínicas -->
          <div class="form-section">
            <div class="section-title">Notas clínicas</div>
            <div class="form-grid">
              <mat-form-field appearance="outline">
                <mat-label>Motivo de ingreso</mat-label>
                <textarea matInput [(ngModel)]="iForm().motivo_ingreso" rows="3"
                  placeholder="Descripción del motivo de hospitalización..."></textarea>
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Observaciones de ingreso</mat-label>
                <textarea matInput [(ngModel)]="iForm().observaciones_ingreso" rows="3"
                  placeholder="Observaciones adicionales al momento del ingreso..."></textarea>
              </mat-form-field>
            </div>
          </div>

          <div class="step-actions">
            <button matButton (click)="volverLista()">Cancelar</button>
            <button matButton="filled" (click)="registrarIngreso()"
              [disabled]="ingresando() || !pacienteSeleccionado() || !iForm().cama_id || !iForm().medico_id">
              <i [class]="ingresando() ? 'pi pi-spin pi-spinner' : 'pi pi-check'"></i> Registrar ingreso
            </button>
          </div>
        </div>
      }

    </div>

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

    .tabla { width: 100%; }
    .centro { text-align: center; }
    mat-form-field { width: 100%; }
    /* En M3 el input color de un botón no pinta nada: el color va por CSS. */
    .aviso-color { color: #f59e0b; }
  `]
})
export class HospitalizacionComponent implements OnInit {
  private svc = inject(HospitalizacionService);
  private dialogo = inject(MatDialog);
  private messageService = inject(Avisos);

  /** El orden de las columnas lo fija esta lista, no la plantilla. */
  readonly columnas = ['paciente', 'cama', 'servicio', 'medico', 'ingreso',
                       'dias', 'estado', 'egreso'];

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

  // Ingreso
  readonly busquedaPaciente = signal('');
  buscandoPaciente = signal(false);
  pacientesResultados = signal<any[]>([]);
  readonly pacienteSeleccionado = signal<any>(null);
  ingresando = signal(false);
  /** Señales: `volverLista()` las reinicia y lo llaman los `subscribe` del
   *  ingreso y del egreso, o sea después de que la detección haya pasado. Con
   *  un campo llano el formulario se quedaba con el paciente anterior. */
  readonly iForm = signal<Partial<IngresoCreate>>(
    { diagnostico_ingreso: 'Z00.0', via_ingreso: 'PROGRAMADA' });

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
    this.busquedaPaciente.set('');
    this.pacientesResultados.set([]);
    this.pacienteSeleccionado.set(null);
    this.iForm.set({ diagnostico_ingreso: 'Z00.0', via_ingreso: 'PROGRAMADA' });
    this.cargarHospitalizaciones();
    this.cargarCamas();
  }

  buscarPaciente(): void {
    if (!this.busquedaPaciente().trim()) return;
    this.buscandoPaciente.set(true);
    this.svc.searchPacientes(this.busquedaPaciente()).subscribe({
      next: ps => {
        this.pacientesResultados.set(ps);
        this.buscandoPaciente.set(false);
        if (ps.length === 0) this.err('No se encontraron pacientes con ese criterio');
      },
      error: () => { this.buscandoPaciente.set(false); this.err('Error al buscar pacientes'); },
    });
  }

  registrarIngreso(): void {
    if (!this.pacienteSeleccionado() || !this.iForm().cama_id || !this.iForm().medico_id) return;
    const payload: IngresoCreate = {
      paciente_id: this.pacienteSeleccionado().id,
      cama_id: this.iForm().cama_id!,
      medico_id: this.iForm().medico_id!,
      diagnostico_ingreso: this.iForm().diagnostico_ingreso || 'Z00.0',
      via_ingreso: this.iForm().via_ingreso || 'PROGRAMADA',
      motivo_ingreso: this.iForm().motivo_ingreso,
      observaciones_ingreso: this.iForm().observaciones_ingreso,
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
    this.dialogo
      .open(HospitalizacionEgresoComponent, { data: h, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((egreso?: EgresoUpdate) => { if (egreso) this.registrarEgreso(h, egreso); });
  }

  private registrarEgreso(h: Hospitalizacion, egreso: EgresoUpdate): void {
    this.svc.egreso(h.id, egreso).subscribe({
      next: () => {
        this.ok('Egreso registrado', `Cama ${h.cama_numero} liberada`);
        this.cargarHospitalizaciones();
        this.cargarCamas();
      },
      error: err => this.err(err?.error?.detail ?? 'Error al registrar el egreso'),
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
