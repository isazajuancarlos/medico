import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { TextareaModule } from 'primeng/textarea';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { ToastModule } from 'primeng/toast';
import { CardModule } from 'primeng/card';
import { DividerModule } from 'primeng/divider';
import { PanelModule } from 'primeng/panel';
import { SelectModule } from 'primeng/select';
import { CheckboxModule } from 'primeng/checkbox';
import { TooltipModule } from 'primeng/tooltip';
import { ConfirmDialogModule } from 'primeng/confirmdialog';
import { InputGroupModule } from 'primeng/inputgroup';
import { InputGroupAddonModule } from 'primeng/inputgroupaddon';
import { MessageService, ConfirmationService } from 'primeng/api';
import {
  LaboratorioService, OrdenLab, ResultadoLab, ResultadoCreate,
  PerfilLab, PerfilCreate, ParametroResultado,
} from '../../services/laboratorio.service';

type Vista = 'lista' | 'procesar' | 'perfiles';

interface FilaResultado {
  nombre: string;
  valor: string;
  unidad: string;
  ref_min: string;
  ref_max: string;
  critico: boolean;
}

function defaultFila(): FilaResultado {
  return { nombre: '', valor: '', unidad: '', ref_min: '', ref_max: '', critico: false };
}

@Component({
  selector: 'app-laboratorio',
  standalone: true,
  providers: [MessageService, ConfirmationService],
  imports: [
    CommonModule, FormsModule,
    ButtonModule, InputTextModule, TextareaModule, TableModule,
    TagModule, ToastModule, CardModule, DividerModule, PanelModule,
    SelectModule, CheckboxModule, TooltipModule, ConfirmDialogModule,
    InputGroupModule, InputGroupAddonModule,
  ],
  template: `
    <div class="page-card">

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA LISTA DE ÓRDENES                    -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'lista') {
        <div class="list-header">
          <h2 class="page-title">Laboratorio Clínico</h2>
          <div style="display:flex;gap:0.5rem">
            <p-select
              [(ngModel)]="filtroEstado"
              [options]="opcionesEstado"
              optionLabel="label"
              optionValue="value"
              (onChange)="cargarOrdenes()"
              placeholder="Todos los estados"
              style="width:190px" />
            <p-button label="Perfiles" icon="pi pi-list" [outlined]="true"
              (onClick)="vista.set('perfiles')" />
          </div>
        </div>

        @if (cargando()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando órdenes...</div>
        } @else if (ordenes().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-inbox" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay órdenes de laboratorio con el filtro seleccionado.</p>
          </div>
        } @else {
          <p-table [value]="ordenes()" [rowHover]="true" styleClass="p-datatable-sm">
            <ng-template pTemplate="header">
              <tr>
                <th>Paciente</th>
                <th>Examen solicitado</th>
                <th>Médico solicitante</th>
                <th>Fecha orden</th>
                <th style="text-align:center">Estado</th>
                <th style="text-align:center">Resultado</th>
                <th style="text-align:center">Acciones</th>
              </tr>
            </ng-template>
            <ng-template pTemplate="body" let-o>
              <tr>
                <td>
                  <strong>{{ o.paciente_nombre || '—' }}</strong>
                  @if (o.paciente_identificacion) {
                    <div style="font-size:0.78rem;color:#94a3b8">{{ o.paciente_identificacion }}</div>
                  }
                </td>
                <td>
                  <span style="font-weight:500">{{ o.descripcion }}</span>
                  @if (o.codigo_cups) {
                    <div style="font-size:0.75rem;color:#64748b">CUPS: {{ o.codigo_cups }}</div>
                  }
                </td>
                <td style="font-size:0.85rem">{{ o.medico_nombre || '—' }}</td>
                <td style="font-size:0.85rem">{{ o.fecha_orden | date:'dd/MM/yyyy HH:mm' }}</td>
                <td style="text-align:center">
                  <p-tag [value]="o.estado" [severity]="estadoSeverity(o.estado)" />
                </td>
                <td style="text-align:center">
                  @if (o.tiene_resultado) {
                    <p-tag value="Registrado" severity="success" icon="pi pi-check-circle" />
                  } @else {
                    <p-tag value="Pendiente" severity="warn" icon="pi pi-clock" />
                  }
                </td>
                <td style="text-align:center">
                  <p-button icon="pi pi-file-edit" [text]="true" size="small"
                    [pTooltip]="o.tiene_resultado ? 'Ver / editar resultado' : 'Ingresar resultado'"
                    (onClick)="abrirOrden(o)" />
                </td>
              </tr>
            </ng-template>
          </p-table>
        }
      }

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA PROCESAR ORDEN                      -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'procesar' && ordenActual()) {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <p-button icon="pi pi-arrow-left" [text]="true" (onClick)="volverLista()" pTooltip="Volver" />
          <h2 class="page-title" style="margin:0">Procesar Examen de Laboratorio</h2>
        </div>

        <!-- Info header -->
        <div class="info-header">
          <div class="info-block">
            <div class="info-label">Paciente</div>
            <div class="info-name">{{ ordenActual()!.paciente_nombre || '—' }}</div>
            <div class="info-row">{{ ordenActual()!.paciente_identificacion }}</div>
          </div>
          <div class="info-block" style="border-left:1px solid rgba(255,255,255,0.1);padding-left:1.25rem">
            <div class="info-label">Examen solicitado</div>
            <div class="info-name">{{ ordenActual()!.descripcion }}</div>
            @if (ordenActual()!.codigo_cups) {
              <div class="info-row">CUPS: {{ ordenActual()!.codigo_cups }}</div>
            }
          </div>
          <div class="info-block" style="border-left:1px solid rgba(255,255,255,0.1);padding-left:1.25rem">
            <div class="info-label">Médico solicitante</div>
            <div class="info-name" style="font-size:0.95rem">{{ ordenActual()!.medico_nombre || '—' }}</div>
            <div class="info-row">{{ ordenActual()!.fecha_orden | date:'dd/MM/yyyy' }}</div>
          </div>
          <div class="info-block" style="border-left:1px solid rgba(255,255,255,0.1);padding-left:1.25rem;flex:0 0 auto">
            <div class="info-label">Estado</div>
            <div style="margin-top:0.25rem">
              <p-tag [value]="ordenActual()!.estado" [severity]="estadoSeverity(ordenActual()!.estado)" />
            </div>
            @if (ordenActual()!.estado === 'Pendiente') {
              <div style="margin-top:0.5rem">
                <p-button label="Iniciar proceso" icon="pi pi-play" size="small"
                  [loading]="cambiandoEstado()" (onClick)="iniciarProceso()" />
              </div>
            }
          </div>
        </div>

        <!-- Justificación -->
        <div class="justificacion-box">
          <span class="info-label">Justificación clínica: </span>
          {{ ordenActual()!.justificacion }}
        </div>

        <!-- Cuerpo resultados -->
        <p-panel header="Resultados del Examen" styleClass="resultado-panel">
          <ng-template pTemplate="header">
            <div style="display:flex;align-items:center;justify-content:space-between;width:100%;gap:1rem">
              <span style="font-weight:600;color:#f1f5f9">Resultados del Examen</span>
              <div style="display:flex;gap:0.5rem">
                @if (perfiles().length > 0) {
                  <p-select
                    [(ngModel)]="perfilSeleccionado"
                    [options]="perfiles()"
                    optionLabel="nombre"
                    (onChange)="cargarDesdePerfil()"
                    placeholder="Cargar perfil..."
                    style="width:200px" />
                }
                <p-button label="Agregar parámetro" icon="pi pi-plus" size="small"
                  [outlined]="true" (onClick)="agregarFila()" />
              </div>
            </div>
          </ng-template>

          @if (filas().length === 0) {
            <div class="sin-resultados" style="padding:1.5rem">
              <i class="pi pi-flask" style="font-size:1.5rem;opacity:0.4"></i>
              <p style="margin:0">Agregue los parámetros del examen o cargue un perfil predefinido.</p>
            </div>
          } @else {
            <div class="tabla-parametros">
              <div class="param-header">
                <span>Parámetro</span>
                <span>Valor</span>
                <span>Unidad</span>
                <span>Ref. mín</span>
                <span>Ref. máx</span>
                <span style="text-align:center">Crítico</span>
                <span></span>
              </div>
              @for (fila of filas(); track $index) {
                <div class="param-row" [class.param-critico]="fila.critico">
                  <input pInputText [(ngModel)]="fila.nombre" placeholder="Ej: Hemoglobina"
                    class="param-input" [disabled]="resultadoValidado()" />
                  <input pInputText [(ngModel)]="fila.valor" placeholder="13.5"
                    class="param-input" [disabled]="resultadoValidado()"
                    [class.valor-critico]="fila.critico" />
                  <input pInputText [(ngModel)]="fila.unidad" placeholder="g/dL"
                    class="param-input" [disabled]="resultadoValidado()" />
                  <input pInputText [(ngModel)]="fila.ref_min" placeholder="12"
                    class="param-input" [disabled]="resultadoValidado()" />
                  <input pInputText [(ngModel)]="fila.ref_max" placeholder="16"
                    class="param-input" [disabled]="resultadoValidado()" />
                  <div style="display:flex;justify-content:center">
                    <p-checkbox [(ngModel)]="fila.critico" [binary]="true"
                      [disabled]="resultadoValidado()" />
                  </div>
                  @if (!resultadoValidado()) {
                    <p-button icon="pi pi-times" [text]="true" size="small" severity="danger"
                      (onClick)="eliminarFila($index)" />
                  } @else {
                    <span></span>
                  }
                </div>
              }
            </div>
          }

          <div class="field-full" style="margin-top:1rem">
            <label style="font-weight:500;font-size:0.875rem;color:#e2e8f0">Observaciones del bioanalista</label>
            <textarea pTextarea [(ngModel)]="observaciones" rows="3" class="w-full"
              placeholder="Notas adicionales, interferencias, calidad de la muestra..."
              [disabled]="resultadoValidado()"></textarea>
          </div>

          <div style="display:flex;align-items:center;gap:1rem;margin-top:0.75rem">
            <p-checkbox [(ngModel)]="hayValoresCriticos" [binary]="true"
              inputId="chk_crit" [disabled]="resultadoValidado()" />
            <label for="chk_crit" style="color:#fbbf24;font-weight:500;cursor:pointer">
              Contiene valores críticos que requieren notificación inmediata al médico
            </label>
          </div>
        </p-panel>

        <!-- Acciones -->
        <div class="step-actions" style="margin-top:1.5rem">
          @if (!resultadoValidado()) {
            <p-button label="Guardar resultados" icon="pi pi-save"
              [loading]="guardando()" [disabled]="filas().length === 0"
              (onClick)="guardarResultado()" />
            @if (resultadoActual()) {
              <p-button label="Validar y completar" icon="pi pi-check-circle"
                severity="success" [loading]="validando()"
                [disabled]="filas().length === 0"
                (onClick)="validarResultado()" />
            }
          } @else {
            <div style="display:flex;align-items:center;gap:0.75rem;color:#4ade80">
              <i class="pi pi-check-circle"></i>
              <span>Resultado validado el {{ resultadoActual()!.fecha_validacion | date:'dd/MM/yyyy HH:mm' }}</span>
            </div>
            <p-button label="Volver a la lista" icon="pi pi-arrow-left" [text]="true"
              (onClick)="volverLista()" />
          }
        </div>
      }

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA PERFILES                            -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'perfiles') {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <p-button icon="pi pi-arrow-left" [text]="true" (onClick)="vista.set('lista')" />
          <h2 class="page-title" style="margin:0">Perfiles de Laboratorio</h2>
          <span style="margin-left:auto">
            <p-button label="Nuevo perfil" icon="pi pi-plus"
              (onClick)="mostrarFormPerfil.set(true)" />
          </span>
        </div>

        @if (mostrarFormPerfil()) {
          <div class="formula-form" style="margin-bottom:1.5rem">
            <h4 style="margin:0 0 1rem;color:#f1f5f9">Nuevo perfil de exámenes</h4>
            <div class="form-grid">
              <div class="field">
                <label>Nombre del perfil *</label>
                <input pInputText [(ngModel)]="pForm.nombre" class="w-full"
                  placeholder="Ej: Hemograma completo, Perfil lipídico..." />
              </div>
              <div class="field">
                <label>Descripción</label>
                <input pInputText [(ngModel)]="pForm.descripcion" class="w-full"
                  placeholder="Descripción opcional..." />
              </div>
            </div>
            <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:0.75rem">
              <strong style="color:#e2e8f0;font-size:0.875rem">Parámetros del perfil</strong>
              <p-button label="Agregar parámetro" icon="pi pi-plus" size="small"
                [text]="true" (onClick)="agregarParamPerfil()" />
            </div>
            @for (p of pForm.examenes; track $index) {
              <div class="param-row" style="margin-bottom:0.5rem">
                <input pInputText [(ngModel)]="p['nombre']" placeholder="Parámetro" class="param-input" />
                <span style="opacity:0.3;font-size:0.8rem;padding:0 0.25rem">valor</span>
                <input pInputText [(ngModel)]="p['unidad']" placeholder="Unidad" class="param-input" />
                <input pInputText [(ngModel)]="p['ref_min']" placeholder="Ref. mín" class="param-input" />
                <input pInputText [(ngModel)]="p['ref_max']" placeholder="Ref. máx" class="param-input" />
                <span></span>
                <p-button icon="pi pi-times" [text]="true" size="small" severity="danger"
                  (onClick)="eliminarParamPerfil($index)" />
              </div>
            }
            <div class="step-actions" style="margin-top:1rem">
              <p-button label="Cancelar" [text]="true" (onClick)="cerrarFormPerfil()" />
              <p-button label="Guardar perfil" icon="pi pi-check"
                [loading]="guardandoPerfil()" (onClick)="guardarPerfil()" />
            </div>
          </div>
        }

        @if (perfiles().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-list" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay perfiles de laboratorio definidos.</p>
          </div>
        } @else {
          @for (p of perfiles(); track p.id) {
            <div class="perfil-card">
              <div style="display:flex;justify-content:space-between;align-items:flex-start">
                <div>
                  <div style="font-weight:600;color:#f1f5f9;margin-bottom:0.25rem">{{ p.nombre }}</div>
                  @if (p.descripcion) {
                    <div style="font-size:0.82rem;color:#94a3b8;margin-bottom:0.5rem">{{ p.descripcion }}</div>
                  }
                  <div style="display:flex;flex-wrap:wrap;gap:0.4rem">
                    @for (e of p.examenes; track e['nombre']) {
                      <span class="param-chip">{{ e['nombre'] }}</span>
                    }
                  </div>
                </div>
                <p-button icon="pi pi-trash" [text]="true" size="small" severity="danger"
                  pTooltip="Desactivar perfil" (onClick)="confirmarEliminarPerfil(p)" />
              </div>
            </div>
          }
        }
      }

    </div>
    <p-toast />
    <p-confirmDialog />
  `,
  styles: [`
    .page-card { padding: 1.5rem; }
    .page-title { font-size: 1.3rem; font-weight: 700; color: #f1f5f9; margin: 0 0 1.5rem; }
    .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem; }
    .list-header .page-title { margin-bottom: 0; }
    .cargando { padding: 3rem; text-align: center; color: #64748b; display: flex; align-items: center; justify-content: center; gap: 0.5rem; }
    .sin-resultados { text-align: center; padding: 3rem 1rem; color: #64748b; display: flex; flex-direction: column; align-items: center; gap: 0.75rem; }

    /* Info header */
    .info-header {
      display: flex; gap: 1.5rem; flex-wrap: wrap;
      background: #0f172a; border: 1px solid rgba(255,255,255,0.1);
      border-radius: 8px; padding: 1rem 1.25rem; margin-bottom: 1rem;
    }
    .info-block { display: flex; flex-direction: column; gap: 0.2rem; }
    .info-label { font-size: 0.7rem; font-weight: 600; text-transform: uppercase; color: #94a3b8; letter-spacing: 0.05em; }
    .info-name { font-size: 1rem; font-weight: 600; color: #f1f5f9; }
    .info-row { font-size: 0.82rem; color: #cbd5e1; }

    .justificacion-box {
      background: rgba(99,102,241,0.08); border: 1px solid rgba(99,102,241,0.2);
      border-radius: 6px; padding: 0.75rem 1rem; margin-bottom: 1.25rem;
      font-size: 0.875rem; color: #cbd5e1;
    }

    /* Tabla de parámetros */
    .tabla-parametros { display: flex; flex-direction: column; gap: 0.5rem; }
    .param-header {
      display: grid; grid-template-columns: 2fr 1fr 1fr 1fr 1fr 60px 36px;
      gap: 0.5rem; padding: 0.4rem 0.5rem;
      font-size: 0.72rem; font-weight: 600; text-transform: uppercase;
      color: #64748b; letter-spacing: 0.04em;
    }
    .param-row {
      display: grid; grid-template-columns: 2fr 1fr 1fr 1fr 1fr 60px 36px;
      gap: 0.5rem; align-items: center;
      padding: 0.4rem 0.5rem; border-radius: 6px;
      border: 1px solid rgba(255,255,255,0.06);
      background: rgba(255,255,255,0.02);
    }
    .param-row.param-critico { border-color: rgba(251,191,36,0.3); background: rgba(251,191,36,0.05); }
    .param-input { width: 100%; font-size: 0.875rem; }
    .valor-critico { color: #fbbf24 !important; font-weight: 700; }

    /* Form grid */
    .form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; margin-bottom: 1rem; }
    .field { display: flex; flex-direction: column; gap: 0.4rem; }
    .field label { font-weight: 500; font-size: 0.875rem; color: #e2e8f0; }
    .field-full { display: flex; flex-direction: column; gap: 0.4rem; }
    .w-full { width: 100%; }

    .step-actions { display: flex; gap: 0.5rem; justify-content: flex-end; padding-top: 1rem; border-top: 1px solid rgba(255,255,255,0.1); }

    /* Fórmula / perfil form */
    .formula-form {
      background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08);
      border-radius: 8px; padding: 1.25rem;
    }

    /* Perfil cards */
    .perfil-card {
      background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08);
      border-radius: 8px; padding: 1rem 1.25rem; margin-bottom: 0.75rem;
    }
    .param-chip {
      font-size: 0.75rem; padding: 0.2rem 0.6rem; border-radius: 999px;
      background: rgba(99,102,241,0.15); color: #a5b4fc;
      border: 1px solid rgba(99,102,241,0.25);
    }

    ::ng-deep .resultado-panel .p-panel-header { background: #1e293b !important; }
    ::ng-deep textarea { resize: vertical; }
  `]
})
export class LaboratorioComponent implements OnInit {
  private svc = inject(LaboratorioService);
  private messageService = inject(MessageService);
  private confirmationService = inject(ConfirmationService);

  vista = signal<Vista>('lista');
  cargando = signal(false);
  ordenes = signal<OrdenLab[]>([]);
  filtroEstado = '';

  readonly opcionesEstado = [
    { label: 'Todos', value: '' },
    { label: 'Pendiente', value: 'Pendiente' },
    { label: 'En Proceso', value: 'En Proceso' },
    { label: 'Completada', value: 'Completada' },
    { label: 'Cancelada', value: 'Cancelada' },
  ];

  // ── Estado al procesar una orden ──
  ordenActual = signal<OrdenLab | null>(null);
  resultadoActual = signal<ResultadoLab | null>(null);
  filas = signal<FilaResultado[]>([]);
  observaciones = '';
  hayValoresCriticos = false;
  guardando = signal(false);
  validando = signal(false);
  cambiandoEstado = signal(false);
  perfilSeleccionado: PerfilLab | null = null;

  // ── Perfiles ──
  perfiles = signal<PerfilLab[]>([]);
  mostrarFormPerfil = signal(false);
  guardandoPerfil = signal(false);
  pForm: PerfilCreate = { nombre: '', examenes: [] };

  resultadoValidado = () => !!this.resultadoActual()?.validado;

  ngOnInit(): void {
    this.cargarOrdenes();
    this.cargarPerfiles();
  }

  cargarOrdenes(): void {
    this.cargando.set(true);
    this.svc.listOrdenes(this.filtroEstado || undefined).subscribe({
      next: os => { this.ordenes.set(os); this.cargando.set(false); },
      error: () => { this.cargando.set(false); this.err('Error al cargar las órdenes'); },
    });
  }

  cargarPerfiles(): void {
    this.svc.listPerfiles().subscribe({ next: ps => this.perfiles.set(ps) });
  }

  abrirOrden(o: OrdenLab): void {
    this.ordenActual.set(o);
    this.resultadoActual.set(null);
    this.filas.set([]);
    this.observaciones = '';
    this.hayValoresCriticos = false;
    this.perfilSeleccionado = null;
    if (o.tiene_resultado) {
      this.svc.getResultado(o.id).subscribe({
        next: r => {
          this.resultadoActual.set(r);
          this.filas.set(this._resultadoToFilas(r.resultados_json));
          this.observaciones = r.observaciones ?? '';
          this.hayValoresCriticos = r.valores_criticos;
        },
        error: () => this.err('Error al cargar el resultado'),
      });
    }
    this.vista.set('procesar');
  }

  volverLista(): void {
    this.ordenActual.set(null);
    this.resultadoActual.set(null);
    this.filas.set([]);
    this.cargarOrdenes();
    this.vista.set('lista');
  }

  iniciarProceso(): void {
    const o = this.ordenActual();
    if (!o) return;
    this.cambiandoEstado.set(true);
    this.svc.updateEstado(o.id, 'En Proceso').subscribe({
      next: updated => {
        this.ordenActual.set(updated);
        this.cambiandoEstado.set(false);
        this.ok('Estado actualizado', 'Orden marcada como En Proceso');
      },
      error: () => { this.cambiandoEstado.set(false); this.err('Error al actualizar estado'); },
    });
  }

  agregarFila(): void {
    this.filas.update(fs => [...fs, defaultFila()]);
  }

  eliminarFila(idx: number): void {
    this.filas.update(fs => fs.filter((_, i) => i !== idx));
  }

  cargarDesdePerfil(): void {
    if (!this.perfilSeleccionado) return;
    const nuevas: FilaResultado[] = this.perfilSeleccionado.examenes.map(e => ({
      nombre: e['nombre'] ?? '',
      valor: '',
      unidad: e['unidad'] ?? '',
      ref_min: e['ref_min'] ?? '',
      ref_max: e['ref_max'] ?? '',
      critico: false,
    }));
    this.filas.set(nuevas);
    this.perfilSeleccionado = null;
  }

  guardarResultado(): void {
    if (!this.ordenActual()) return;
    const payload = this._filasToPayload();
    this.guardando.set(true);
    const obs$ = this.resultadoActual()
      ? this.svc.updateResultado(this.ordenActual()!.id, payload)
      : this.svc.createResultado(this.ordenActual()!.id, payload);
    obs$.subscribe({
      next: r => {
        this.resultadoActual.set(r);
        this.guardando.set(false);
        // Refresh order to update estado and tiene_resultado
        this.svc.getOrden(this.ordenActual()!.id).subscribe(o => this.ordenActual.set(o));
        this.ok('Guardado', 'Resultados guardados correctamente');
      },
      error: err => {
        this.guardando.set(false);
        this.err(err?.error?.detail ?? 'Error al guardar el resultado');
      },
    });
  }

  validarResultado(): void {
    if (!this.ordenActual() || !this.resultadoActual()) return;
    // Save first, then validate
    const payload = this._filasToPayload();
    this.validando.set(true);
    this.svc.updateResultado(this.ordenActual()!.id, payload).subscribe({
      next: () => {
        this.svc.validarResultado(this.ordenActual()!.id).subscribe({
          next: r => {
            this.resultadoActual.set(r);
            this.validando.set(false);
            this.svc.getOrden(this.ordenActual()!.id).subscribe(o => this.ordenActual.set(o));
            this.ok('Validado', 'Resultado validado y orden completada');
          },
          error: err => { this.validando.set(false); this.err(err?.error?.detail ?? 'Error al validar'); },
        });
      },
      error: err => { this.validando.set(false); this.err(err?.error?.detail ?? 'Error al guardar antes de validar'); },
    });
  }

  // ── Perfiles ──────────────────────────────────────────────────────
  agregarParamPerfil(): void {
    this.pForm.examenes.push({ nombre: '', unidad: '', ref_min: '', ref_max: '' } as any);
  }

  eliminarParamPerfil(idx: number): void {
    this.pForm.examenes = this.pForm.examenes.filter((_, i) => i !== idx);
  }

  cerrarFormPerfil(): void {
    this.pForm = { nombre: '', examenes: [] };
    this.mostrarFormPerfil.set(false);
  }

  guardarPerfil(): void {
    if (!this.pForm.nombre.trim()) {
      this.err('El nombre del perfil es obligatorio');
      return;
    }
    this.guardandoPerfil.set(true);
    this.svc.createPerfil(this.pForm).subscribe({
      next: p => {
        this.perfiles.update(ps => [...ps, p]);
        this.cerrarFormPerfil();
        this.guardandoPerfil.set(false);
        this.ok('Perfil creado', p.nombre);
      },
      error: err => {
        this.guardandoPerfil.set(false);
        this.err(err?.error?.detail ?? 'Error al crear el perfil');
      },
    });
  }

  confirmarEliminarPerfil(p: PerfilLab): void {
    this.confirmationService.confirm({
      message: `¿Desactivar el perfil <strong>${p.nombre}</strong>?`,
      header: 'Confirmar',
      icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Desactivar',
      rejectLabel: 'Cancelar',
      acceptButtonStyleClass: 'p-button-danger',
      accept: () => {
        this.svc.deletePerfil(p.id).subscribe({
          next: () => {
            this.perfiles.update(ps => ps.filter(x => x.id !== p.id));
            this.ok('Desactivado', p.nombre);
          },
          error: () => this.err('Error al desactivar el perfil'),
        });
      },
    });
  }

  // ── Helpers ───────────────────────────────────────────────────────
  estadoSeverity(estado: string): 'info' | 'success' | 'warn' | 'danger' | 'secondary' {
    if (estado === 'Completada') return 'success';
    if (estado === 'Cancelada') return 'danger';
    if (estado === 'En Proceso') return 'info';
    return 'warn';
  }

  private _filasToPayload(): { resultados_json: Record<string, ParametroResultado>; observaciones?: string; valores_criticos: boolean } {
    const resultados_json: Record<string, ParametroResultado> = {};
    for (const f of this.filas()) {
      if (f.nombre.trim()) {
        resultados_json[f.nombre.trim()] = {
          valor: f.valor, unidad: f.unidad,
          ref_min: f.ref_min || undefined, ref_max: f.ref_max || undefined,
          critico: f.critico,
        };
      }
    }
    return { resultados_json, observaciones: this.observaciones || undefined, valores_criticos: this.hayValoresCriticos };
  }

  private _resultadoToFilas(json: Record<string, any>): FilaResultado[] {
    return Object.entries(json).map(([nombre, v]) => ({
      nombre, valor: v.valor ?? '', unidad: v.unidad ?? '',
      ref_min: v.ref_min ?? '', ref_max: v.ref_max ?? '', critico: !!v.critico,
    }));
  }

  private ok(summary: string, detail?: string): void {
    this.messageService.add({ severity: 'success', summary, detail });
  }

  private err(detail: string): void {
    this.messageService.add({ severity: 'error', summary: 'Error', detail });
  }
}
