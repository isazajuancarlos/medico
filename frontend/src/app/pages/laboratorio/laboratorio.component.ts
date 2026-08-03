import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCardModule } from '@angular/material/card';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatTableModule } from '@angular/material/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { MatDividerModule } from '@angular/material/divider';
import { MatTooltipModule } from '@angular/material/tooltip';
import { Avisos } from '../../shared/ui/avisos.service';
import { Confirmar } from '../../shared/ui/confirmar.service';
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
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatCardModule, MatCheckboxModule, MatFormFieldModule,
    MatInputModule, MatSelectModule, MatTableModule,
    EtiquetaComponent, MatDividerModule, MatTooltipModule,
  ],
  template: `
    <div class="page-card">

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA LISTA DE ÓRDENES                    -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'lista') {
        <div class="list-header">
          <h2 class="page-title">Laboratorio Clínico</h2>
          <div style="display:flex;gap:0.5rem;align-items:flex-start">
            <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:190px">
              <mat-label>Estado</mat-label>
              <mat-select [(ngModel)]="filtroEstado" (selectionChange)="cargarOrdenes()">
                @for (o of opcionesEstado; track o.value) {
                  <mat-option [value]="o.value">{{ o.label }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
            <button matButton="outlined" (click)="vista.set('perfiles')">
              <i class="pi pi-list"></i> Perfiles
            </button>
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
          <table mat-table [dataSource]="ordenes()" class="tabla">
            <ng-container matColumnDef="paciente">
              <th mat-header-cell *matHeaderCellDef>Paciente</th>
              <td mat-cell *matCellDef="let o">
                <strong>{{ o.paciente_nombre || '—' }}</strong>
                @if (o.paciente_identificacion) {
                  <div style="font-size:0.78rem;color:#94a3b8">{{ o.paciente_identificacion }}</div>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="examen">
              <th mat-header-cell *matHeaderCellDef>Examen solicitado</th>
              <td mat-cell *matCellDef="let o">
                <span style="font-weight:500">{{ o.descripcion }}</span>
                @if (o.codigo_cups) {
                  <div style="font-size:0.75rem;color:#64748b">CUPS: {{ o.codigo_cups }}</div>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="medico">
              <th mat-header-cell *matHeaderCellDef>Médico solicitante</th>
              <td mat-cell *matCellDef="let o" style="font-size:0.85rem">{{ o.medico_nombre || '—' }}</td>
            </ng-container>
            <ng-container matColumnDef="fecha">
              <th mat-header-cell *matHeaderCellDef>Fecha orden</th>
              <td mat-cell *matCellDef="let o" style="font-size:0.85rem">
                {{ o.fecha_orden | date:'dd/MM/yyyy HH:mm' }}
              </td>
            </ng-container>
            <ng-container matColumnDef="estado">
              <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
              <td mat-cell *matCellDef="let o" class="centro">
                <x-etiqueta [value]="o.estado" [severity]="estadoSeverity(o.estado)" />
              </td>
            </ng-container>
            <ng-container matColumnDef="resultado">
              <th mat-header-cell *matHeaderCellDef class="centro">Resultado</th>
              <td mat-cell *matCellDef="let o" class="centro">
                @if (o.tiene_resultado) {
                  <x-etiqueta value="Registrado" severity="success" icon="pi pi-check-circle" />
                } @else {
                  <x-etiqueta value="Pendiente" severity="warn" icon="pi pi-clock" />
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="acciones">
              <th mat-header-cell *matHeaderCellDef class="centro">Acciones</th>
              <td mat-cell *matCellDef="let o" class="centro">
                <button matIconButton (click)="abrirOrden(o)"
                  [matTooltip]="o.tiene_resultado ? 'Ver / editar resultado' : 'Ingresar resultado'">
                  <i class="pi pi-file-edit"></i>
                </button>
              </td>
            </ng-container>

            <tr mat-header-row *matHeaderRowDef="columnas"></tr>
            <tr mat-row *matRowDef="let fila; columns: columnas"></tr>
          </table>
        }
      }

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA PROCESAR ORDEN                      -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'procesar' && ordenActual()) {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <button matIconButton matTooltip="Volver" (click)="volverLista()">
            <i class="pi pi-arrow-left"></i>
          </button>
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
              <x-etiqueta [value]="ordenActual()!.estado" [severity]="estadoSeverity(ordenActual()!.estado)" />
            </div>
            @if (ordenActual()!.estado === 'Pendiente') {
              <div style="margin-top:0.5rem">
                <button matButton="filled" [disabled]="cambiandoEstado()" (click)="iniciarProceso()">
                  <i [class]="cambiandoEstado() ? 'pi pi-spin pi-spinner' : 'pi pi-play'"></i> Iniciar proceso
                </button>
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
        <mat-card class="resultado-panel">
          <div class="panel-cabecera">
            <span style="font-weight:600;color:#f1f5f9">Resultados del Examen</span>
            <div style="display:flex;gap:0.5rem;align-items:flex-start">
              @if (perfiles().length > 0) {
                <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:200px">
                  <mat-label>Cargar perfil…</mat-label>
                  <mat-select [(ngModel)]="perfilSeleccionado" (selectionChange)="cargarDesdePerfil()">
                    @for (pf of perfiles(); track pf.id) {
                      <mat-option [value]="pf">{{ pf.nombre }}</mat-option>
                    }
                  </mat-select>
                </mat-form-field>
              }
              <button matButton="outlined" (click)="agregarFila()">
                <i class="pi pi-plus"></i> Agregar parámetro
              </button>
            </div>
          </div>
          <mat-card-content>

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
                  <input [(ngModel)]="fila.nombre" placeholder="Ej: Hemoglobina"
                    class="param-input" [disabled]="resultadoValidado()" />
                  <input [(ngModel)]="fila.valor" placeholder="13.5"
                    class="param-input" [disabled]="resultadoValidado()"
                    [class.valor-critico]="fila.critico" />
                  <input [(ngModel)]="fila.unidad" placeholder="g/dL"
                    class="param-input" [disabled]="resultadoValidado()" />
                  <input [(ngModel)]="fila.ref_min" placeholder="12"
                    class="param-input" [disabled]="resultadoValidado()" />
                  <input [(ngModel)]="fila.ref_max" placeholder="16"
                    class="param-input" [disabled]="resultadoValidado()" />
                  <div style="display:flex;justify-content:center">
                    <mat-checkbox [(ngModel)]="fila.critico" [disabled]="resultadoValidado()" />
                  </div>
                  @if (!resultadoValidado()) {
                    <button matIconButton class="peligro" matTooltip="Quitar el parámetro"
                      (click)="eliminarFila($index)">
                      <i class="pi pi-times"></i>
                    </button>
                  } @else {
                    <span></span>
                  }
                </div>
              }
            </div>
          }

          <mat-form-field appearance="outline" style="margin-top:1rem">
            <mat-label>Observaciones del bioanalista</mat-label>
            <textarea matInput [ngModel]="observaciones()"
              (ngModelChange)="observaciones.set($event)" rows="3"
              placeholder="Notas adicionales, interferencias, calidad de la muestra..."
              [disabled]="resultadoValidado()"></textarea>
          </mat-form-field>

          <div style="margin-top:0.75rem">
            <mat-checkbox class="critico" [ngModel]="hayValoresCriticos()"
              (ngModelChange)="hayValoresCriticos.set($event)" [disabled]="resultadoValidado()">
              Contiene valores críticos que requieren notificación inmediata al médico
            </mat-checkbox>
          </div>
          </mat-card-content>
        </mat-card>

        <!-- Acciones -->
        <div class="step-actions" style="margin-top:1.5rem">
          @if (!resultadoValidado()) {
            <button matButton="filled" [disabled]="guardando() || filas().length === 0"
              (click)="guardarResultado()">
              <i [class]="guardando() ? 'pi pi-spin pi-spinner' : 'pi pi-save'"></i> Guardar resultados
            </button>
            @if (resultadoActual()) {
              <button matButton="filled" class="exito"
                [disabled]="validando() || filas().length === 0" (click)="validarResultado()">
                <i [class]="validando() ? 'pi pi-spin pi-spinner' : 'pi pi-check-circle'"></i> Validar y completar
              </button>
            }
          } @else {
            <div style="display:flex;align-items:center;gap:0.75rem;color:#4ade80">
              <i class="pi pi-check-circle"></i>
              <span>Resultado validado el {{ resultadoActual()!.fecha_validacion | date:'dd/MM/yyyy HH:mm' }}</span>
            </div>
            <button matButton (click)="volverLista()">
              <i class="pi pi-arrow-left"></i> Volver a la lista
            </button>
          }
        </div>
      }

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA PERFILES                            -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'perfiles') {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <button matIconButton (click)="vista.set('lista')"><i class="pi pi-arrow-left"></i></button>
          <h2 class="page-title" style="margin:0">Perfiles de Laboratorio</h2>
          <span style="margin-left:auto">
            <button matButton="filled" (click)="mostrarFormPerfil.set(true)">
              <i class="pi pi-plus"></i> Nuevo perfil
            </button>
          </span>
        </div>

        @if (mostrarFormPerfil()) {
          <div class="formula-form" style="margin-bottom:1.5rem">
            <h4 style="margin:0 0 1rem;color:#f1f5f9">Nuevo perfil de exámenes</h4>
            <div class="form-grid">
              <mat-form-field appearance="outline">
                <mat-label>Nombre del perfil *</mat-label>
                <input matInput [(ngModel)]="pForm().nombre"
                  placeholder="Ej: Hemograma completo, Perfil lipídico..." />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Descripción</mat-label>
                <input matInput [(ngModel)]="pForm().descripcion" placeholder="Descripción opcional..." />
              </mat-form-field>
            </div>
            <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:0.75rem">
              <strong style="color:#e2e8f0;font-size:0.875rem">Parámetros del perfil</strong>
              <button matButton (click)="agregarParamPerfil()">
                <i class="pi pi-plus"></i> Agregar parámetro
              </button>
            </div>
            @for (p of pForm().examenes; track $index) {
              <div class="param-row" style="margin-bottom:0.5rem">
                <input [(ngModel)]="p['nombre']" placeholder="Parámetro" class="param-input" />
                <span style="opacity:0.3;font-size:0.8rem;padding:0 0.25rem">valor</span>
                <input [(ngModel)]="p['unidad']" placeholder="Unidad" class="param-input" />
                <input [(ngModel)]="p['ref_min']" placeholder="Ref. mín" class="param-input" />
                <input [(ngModel)]="p['ref_max']" placeholder="Ref. máx" class="param-input" />
                <span></span>
                <button matIconButton class="peligro" matTooltip="Quitar el parámetro"
                  (click)="eliminarParamPerfil($index)">
                  <i class="pi pi-times"></i>
                </button>
              </div>
            }
            <div class="step-actions" style="margin-top:1rem">
              <button matButton (click)="cerrarFormPerfil()">Cancelar</button>
              <button matButton="filled" [disabled]="guardandoPerfil()" (click)="guardarPerfil()">
                <i [class]="guardandoPerfil() ? 'pi pi-spin pi-spinner' : 'pi pi-check'"></i> Guardar perfil
              </button>
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
                <button matIconButton class="peligro" matTooltip="Desactivar perfil"
                  (click)="confirmarEliminarPerfil(p)">
                  <i class="pi pi-trash"></i>
                </button>
              </div>
            </div>
          }
        }
      }

    </div>
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
    /* Los parámetros son una rejilla densa: aquí NO se envuelve cada campo en
       un mat-form-field —una fila de siete controles con etiqueta flotante y
       margen no cabe en la pantalla—, así que son input desnudos con el
       aspecto puesto por esta hoja. */
    .param-input {
      width: 100%; font-size: 0.875rem;
      background: rgba(255,255,255,0.04); color: #e2e8f0;
      border: 1px solid rgba(255,255,255,0.12); border-radius: 5px;
      padding: 0.4rem 0.5rem;
    }
    .param-input:focus { outline: none; border-color: #818cf8; }
    .param-input:disabled { opacity: 0.6; }
    .param-input::placeholder { color: #64748b; }
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

    .resultado-panel { background: #0f172a; }
    .panel-cabecera {
      display: flex; align-items: center; justify-content: space-between;
      gap: 1rem; padding: 1rem; background: #1e293b; border-radius: 4px 4px 0 0;
    }
    .tabla { width: 100%; }
    .centro { text-align: center; }
    mat-form-field { width: 100%; }
    .critico { color: #fbbf24; }
    /* En M3 el input color de un botón no pinta nada: el color va por CSS. */
    .peligro { color: #dc2626; }
    .exito { background: #16a34a; color: #fff; }
    ::ng-deep textarea { resize: vertical; }
  `]
})
export class LaboratorioComponent implements OnInit {
  private svc = inject(LaboratorioService);
  private messageService = inject(Avisos);
  private confirmationService = inject(Confirmar);

  vista = signal<Vista>('lista');
  cargando = signal(false);
  ordenes = signal<OrdenLab[]>([]);
  filtroEstado = '';

  /** El orden de las columnas lo fija esta lista, no la plantilla. */
  readonly columnas = ['paciente', 'examen', 'medico', 'fecha', 'estado',
                       'resultado', 'acciones'];

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
  /** Señales: las escribe el `subscribe` que trae el resultado ya guardado, y
   *  con OnPush un campo llano habría dejado en pantalla las observaciones del
   *  examen ANTERIOR. En un resultado de laboratorio eso no es un detalle. */
  readonly observaciones = signal('');
  readonly hayValoresCriticos = signal(false);
  guardando = signal(false);
  validando = signal(false);
  cambiandoEstado = signal(false);
  perfilSeleccionado: PerfilLab | null = null;

  // ── Perfiles ──
  perfiles = signal<PerfilLab[]>([]);
  mostrarFormPerfil = signal(false);
  guardandoPerfil = signal(false);
  /** Señal: `guardarPerfil` la vacía dentro del `subscribe`, y con OnPush el
   *  formulario se habría quedado con el perfil anterior escrito. */
  readonly pForm = signal<PerfilCreate>({ nombre: '', examenes: [] });

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
    this.observaciones.set('');
    this.hayValoresCriticos.set(false);
    this.perfilSeleccionado = null;
    if (o.tiene_resultado) {
      this.svc.getResultado(o.id).subscribe({
        next: r => {
          this.resultadoActual.set(r);
          this.filas.set(this._resultadoToFilas(r.resultados_json));
          this.observaciones.set(r.observaciones ?? '');
          this.hayValoresCriticos.set(r.valores_criticos);
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
    this.pForm.update(f => ({ ...f, examenes: [...f.examenes, { nombre: '', unidad: '', ref_min: '', ref_max: '' } as any] }));
  }

  eliminarParamPerfil(idx: number): void {
    this.pForm.update(f => ({ ...f, examenes: f.examenes.filter((_, i) => i !== idx) }));
  }

  cerrarFormPerfil(): void {
    this.pForm.set({ nombre: '', examenes: [] });
    this.mostrarFormPerfil.set(false);
  }

  guardarPerfil(): void {
    if (!this.pForm().nombre.trim()) {
      this.err('El nombre del perfil es obligatorio');
      return;
    }
    this.guardandoPerfil.set(true);
    this.svc.createPerfil(this.pForm()).subscribe({
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
      peligroso: true,
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
    return { resultados_json, observaciones: this.observaciones() || undefined, valores_criticos: this.hayValoresCriticos() };
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
