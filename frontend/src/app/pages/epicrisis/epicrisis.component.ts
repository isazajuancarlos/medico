import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatTableModule } from '@angular/material/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatDividerModule } from '@angular/material/divider';
import { Avisos } from '../../shared/ui/avisos.service';
import { Confirmar } from '../../shared/ui/confirmar.service';
import { EpicrisisService, Epicrisis, EpicrisisCreate } from '../../services/epicrisis.service';

type Vista = 'lista' | 'form' | 'detalle';

@Component({
  selector: 'app-epicrisis',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatFormFieldModule, MatInputModule, MatTableModule,
    EtiquetaComponent, MatTooltipModule, MatSelectModule,
    MatDividerModule,
  ],
  template: `
    <div class="page-card">

      <!-- ═══════════════════════════════════════════ -->
      <!--  LISTA                                     -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'lista') {
        <div class="list-header">
          <h2 class="page-title">Epicrisis</h2>
          <button matButton="filled" (click)="abrirNueva()">
            <i class="pi pi-plus"></i> Nueva epicrisis
          </button>
        </div>

        @if (cargando()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (lista().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-file" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay epicrisis registradas.</p>
          </div>
        } @else {
          <table mat-table [dataSource]="lista()">
            <ng-container matColumnDef="paciente">
              <th mat-header-cell *matHeaderCellDef>Paciente</th>
              <td mat-cell *matCellDef="let e">
                <strong>{{ e.paciente_nombre || '—' }}</strong>
                @if (e.paciente_identificacion) {
                  <div class="sub">{{ e.paciente_identificacion }}</div>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="medico">
              <th mat-header-cell *matHeaderCellDef>Médico tratante</th>
              <td mat-cell *matCellDef="let e" class="menudo">{{ e.medico_nombre || '—' }}</td>
            </ng-container>
            <ng-container matColumnDef="ingreso">
              <th mat-header-cell *matHeaderCellDef>Ingreso</th>
              <td mat-cell *matCellDef="let e" class="menudo">{{ e.fecha_ingreso | date:'dd/MM/yyyy' }}</td>
            </ng-container>
            <ng-container matColumnDef="egreso">
              <th mat-header-cell *matHeaderCellDef>Egreso</th>
              <td mat-cell *matCellDef="let e" class="menudo">{{ e.fecha_egreso | date:'dd/MM/yyyy' }}</td>
            </ng-container>
            <ng-container matColumnDef="dias">
              <th mat-header-cell *matHeaderCellDef class="centro">Días</th>
              <td mat-cell *matCellDef="let e" class="centro">
                <span class="dias-badge">{{ e.dias_estancia }}</span>
              </td>
            </ng-container>
            <ng-container matColumnDef="pronostico">
              <th mat-header-cell *matHeaderCellDef class="centro">Pronóstico</th>
              <td mat-cell *matCellDef="let e" class="centro">
                <x-etiqueta [value]="e.pronostico" [severity]="pronosticoSeverity(e.pronostico)" />
              </td>
            </ng-container>
            <ng-container matColumnDef="incapacidad">
              <th mat-header-cell *matHeaderCellDef class="centro">Incapacidad</th>
              <td mat-cell *matCellDef="let e" class="centro menudo">
                @if (e.incapacidad_dias > 0) {
                  <span style="color:#fbbf24">{{ e.incapacidad_dias }} días</span>
                } @else {
                  <span class="nada">—</span>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="acciones">
              <th mat-header-cell *matHeaderCellDef class="centro">Acciones</th>
              <td mat-cell *matCellDef="let e" class="centro sin-cortar">
                <button matIconButton matTooltip="Ver detalle" (click)="abrirDetalle(e)"
                  aria-label="Ver detalle"><i class="pi pi-eye"></i></button>
                <button matIconButton matTooltip="Editar" (click)="abrirEditar(e)"
                  aria-label="Editar"><i class="pi pi-file-edit"></i></button>
                <button matIconButton class="peligro" matTooltip="Eliminar"
                  (click)="confirmarEliminar(e)" aria-label="Eliminar">
                  <i class="pi pi-trash"></i>
                </button>
              </td>
            </ng-container>

            <tr mat-header-row *matHeaderRowDef="columnas"></tr>
            <tr mat-row *matRowDef="let fila; columns: columnas"></tr>
          </table>
        }
      }

      <!-- ═══════════════════════════════════════════ -->
      <!--  FORMULARIO                                -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'form') {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <button matIconButton (click)="volverLista()" matTooltip="Volver" aria-label="Volver">
            <i class="pi pi-arrow-left"></i>
          </button>
          <h2 class="page-title" style="margin:0">
            {{ editando() ? 'Editar epicrisis' : 'Nueva epicrisis' }}
          </h2>
        </div>

        <div class="form-card">

          <!-- Paciente (solo en nueva) -->
          @if (!editando()) {
            <div class="form-section">
              <div class="section-label">Paciente</div>
              <div style="display:flex;gap:0.5rem;align-items:flex-start">
                <mat-form-field appearance="outline" subscriptSizing="dynamic" style="flex:1">
                  <mat-label>Nombre o identificación</mat-label>
                  <input matInput [(ngModel)]="busquedaPaciente" (keyup.enter)="buscarPaciente()" />
                </mat-form-field>
                <button matButton="filled" [disabled]="buscandoPaciente()" (click)="buscarPaciente()">
                  <i [class]="buscandoPaciente() ? 'pi pi-spin pi-spinner' : 'pi pi-search'"></i>
                  Buscar
                </button>
              </div>
              @if (resultadosPaciente().length > 0) {
                <!-- La identificación va en la opción: mat-select no trae el
                     buscador de p-select, y es lo que distingue a dos pacientes
                     con el mismo nombre. -->
                <mat-form-field appearance="outline" subscriptSizing="dynamic" class="w-full separa">
                  <mat-label>Seleccione paciente</mat-label>
                  <mat-select [(ngModel)]="pacienteSeleccionado">
                    @for (pa of resultadosPaciente(); track pa.id) {
                      <mat-option [value]="pa">{{ pa.nombre_completo }} — {{ pa.identificacion }}</mat-option>
                    }
                  </mat-select>
                </mat-form-field>
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
          } @else {
            <div class="paciente-chip" style="margin-bottom:1rem">
              <i class="pi pi-user" style="color:#818cf8"></i>
              <strong style="color:#f1f5f9">{{ actual()?.paciente_nombre }}</strong>
              <span style="color:#94a3b8;font-size:0.82rem">{{ actual()?.paciente_identificacion }}</span>
            </div>
          }

          <!-- Fechas y estancia -->
          <div class="form-section">
            <div class="section-label">Período de hospitalización</div>
            <div class="grid-3">
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Fecha de ingreso *</mat-label>
                <input matInput [(ngModel)]="f.fecha_ingreso" type="datetime-local" (change)="calcularDias()" />
              </mat-form-field>
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Fecha de egreso *</mat-label>
                <input matInput [(ngModel)]="f.fecha_egreso" type="datetime-local" (change)="calcularDias()" />
              </mat-form-field>
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Días de estancia</mat-label>
                <input matInput [(ngModel)]="f.dias_estancia" type="number" min="0" />
              </mat-form-field>
            </div>
          </div>

          <mat-divider />

          <!-- Diagnósticos -->
          <div class="form-section">
            <div class="section-label">Diagnósticos</div>
            <div class="grid-3">
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Diagnóstico de ingreso (CIE-10) *</mat-label>
                <input matInput [(ngModel)]="f.diagnostico_ingreso" placeholder="Ej: J18.9" />
              </mat-form-field>
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Diagnóstico de egreso (CIE-10) *</mat-label>
                <input matInput [(ngModel)]="f.diagnostico_egreso" placeholder="Ej: J18.0" />
              </mat-form-field>
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Complicaciones</mat-label>
                <input matInput [(ngModel)]="f.complicaciones" placeholder="Ninguna / descripción…" />
              </mat-form-field>
            </div>
          </div>

          <mat-divider />

          <!-- Resumen clínico -->
          <div class="form-section">
            <div class="section-label">Resumen clínico</div>
            <div class="form-grid">
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Resumen de evolución *</mat-label>
                <textarea matInput [(ngModel)]="f.resumen_evolucion" rows="5"
                  placeholder="Evolución del paciente durante la hospitalización, respuesta al tratamiento…"></textarea>
              </mat-form-field>
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Procedimientos realizados *</mat-label>
                <textarea matInput [(ngModel)]="f.procedimientos_realizados" rows="5"
                  placeholder="Cirugías, procedimientos diagnósticos, terapias realizadas…"></textarea>
              </mat-form-field>
              <mat-form-field appearance="outline" subscriptSizing="dynamic" class="ancho-total">
                <mat-label>Exámenes relevantes</mat-label>
                <textarea matInput [(ngModel)]="f.examenes_relevantes" rows="3"
                  placeholder="Resultados de laboratorio, imágenes diagnósticas relevantes…"></textarea>
              </mat-form-field>
            </div>
          </div>

          <mat-divider />

          <!-- Plan de egreso -->
          <div class="form-section">
            <div class="section-label">Plan de egreso</div>
            <div class="form-grid">
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Recomendaciones de egreso *</mat-label>
                <textarea matInput [(ngModel)]="f.recomendaciones_egreso" rows="4"
                  placeholder="Cuidados en casa, restricciones, signos de alarma…"></textarea>
              </mat-form-field>
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Medicamentos al egreso</mat-label>
                <textarea matInput [(ngModel)]="f.medicamentos_egreso" rows="4"
                  placeholder="Nombre, dosis, frecuencia, duración…"></textarea>
              </mat-form-field>
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Controles programados</mat-label>
                <textarea matInput [(ngModel)]="f.controles_programados" rows="3"
                  placeholder="Citas de seguimiento, especialidades, fechas sugeridas…"></textarea>
              </mat-form-field>
              <div style="display:flex;gap:1rem">
                <mat-form-field appearance="outline" subscriptSizing="dynamic" style="flex:1">
                  <mat-label>Pronóstico</mat-label>
                  <mat-select [(ngModel)]="f.pronostico">
                    @for (o of opcionesPronostico; track o.value) {
                      <mat-option [value]="o.value">{{ o.label }}</mat-option>
                    }
                  </mat-select>
                </mat-form-field>
                <mat-form-field appearance="outline" subscriptSizing="dynamic" style="flex:1">
                  <mat-label>Incapacidad (días)</mat-label>
                  <input matInput [(ngModel)]="f.incapacidad_dias" type="number"
                    min="0" placeholder="0" />
                </mat-form-field>
              </div>
            </div>
          </div>

          <div class="step-actions">
            <button matButton (click)="volverLista()">Cancelar</button>
            <button matButton="filled" (click)="guardar()"
              [disabled]="guardando() || camposRequeridos()">
              <i [class]="guardando() ? 'pi pi-spin pi-spinner' : 'pi pi-check'"></i>
              {{ editando() ? 'Guardar cambios' : 'Registrar epicrisis' }}
            </button>
          </div>
        </div>
      }

      <!-- ═══════════════════════════════════════════ -->
      <!--  DETALLE (SOLO LECTURA)                    -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'detalle' && actual()) {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <button matIconButton (click)="volverLista()" matTooltip="Volver" aria-label="Volver">
            <i class="pi pi-arrow-left"></i>
          </button>
          <h2 class="page-title" style="margin:0">Epicrisis — Resumen de egreso</h2>
          <div style="margin-left:auto">
            <button matButton (click)="abrirEditar(actual()!)">
              <i class="pi pi-file-edit"></i> Editar
            </button>
          </div>
        </div>

        <!-- Info header -->
        <div class="info-header">
          <div class="info-block">
            <div class="info-label">Paciente</div>
            <div class="info-name">{{ actual()!.paciente_nombre }}</div>
            <div class="info-row">{{ actual()!.paciente_identificacion }}</div>
          </div>
          <div class="info-block sep">
            <div class="info-label">Médico tratante</div>
            <div class="info-name" style="font-size:0.95rem">{{ actual()!.medico_nombre }}</div>
          </div>
          <div class="info-block sep">
            <div class="info-label">Período</div>
            <div class="info-name" style="font-size:0.9rem">
              {{ actual()!.fecha_ingreso | date:'dd/MM/yyyy' }} → {{ actual()!.fecha_egreso | date:'dd/MM/yyyy' }}
            </div>
            <div class="info-row">{{ actual()!.dias_estancia }} días de estancia</div>
          </div>
          <div class="info-block sep">
            <div class="info-label">Pronóstico</div>
            <div style="margin-top:0.25rem">
              <x-etiqueta [value]="actual()!.pronostico" [severity]="pronosticoSeverity(actual()!.pronostico)" />
            </div>
            @if (actual()!.incapacidad_dias > 0) {
              <div class="info-row" style="margin-top:0.3rem;color:#fbbf24">
                {{ actual()!.incapacidad_dias }} días de incapacidad
              </div>
            }
          </div>
        </div>

        <!-- Diagnósticos -->
        <div class="dx-row">
          <div class="dx-card">
            <div class="section-label">Diagnóstico de ingreso</div>
            <div class="dx-code">{{ actual()!.diagnostico_ingreso }}</div>
          </div>
          <div class="dx-card">
            <div class="section-label">Diagnóstico de egreso</div>
            <div class="dx-code">{{ actual()!.diagnostico_egreso }}</div>
          </div>
          @if (actual()!.complicaciones) {
            <div class="dx-card">
              <div class="section-label">Complicaciones</div>
              <div class="dx-code" style="color:#fbbf24">{{ actual()!.complicaciones }}</div>
            </div>
          }
        </div>

        <!-- Secciones clínicas -->
        <div class="detalle-grid">
          <div class="detalle-bloque">
            <div class="section-label">Resumen de evolución</div>
            <div class="texto-bloque">{{ actual()!.resumen_evolucion }}</div>
          </div>
          <div class="detalle-bloque">
            <div class="section-label">Procedimientos realizados</div>
            <div class="texto-bloque">{{ actual()!.procedimientos_realizados }}</div>
          </div>
          @if (actual()!.examenes_relevantes) {
            <div class="detalle-bloque" style="grid-column:1/-1">
              <div class="section-label">Exámenes relevantes</div>
              <div class="texto-bloque">{{ actual()!.examenes_relevantes }}</div>
            </div>
          }
        </div>

        <mat-divider />

        <div class="detalle-grid">
          <div class="detalle-bloque">
            <div class="section-label">Recomendaciones de egreso</div>
            <div class="texto-bloque">{{ actual()!.recomendaciones_egreso }}</div>
          </div>
          @if (actual()!.medicamentos_egreso) {
            <div class="detalle-bloque">
              <div class="section-label">Medicamentos al egreso</div>
              <div class="texto-bloque">{{ actual()!.medicamentos_egreso }}</div>
            </div>
          }
          @if (actual()!.controles_programados) {
            <div class="detalle-bloque" style="grid-column:1/-1">
              <div class="section-label">Controles programados</div>
              <div class="texto-bloque">{{ actual()!.controles_programados }}</div>
            </div>
          }
        </div>
      }

    </div>
  `,
  styles: [`
    table { width: 100%; }
    .centro { text-align: center; }
    .menudo { font-size: 0.85rem; }
    .sin-cortar { white-space: nowrap; }
    .sub { font-size: 0.78rem; color: #94a3b8; }
    .nada { color: #475569; }
    .separa { margin-top: 0.75rem; }
    .ancho-total { grid-column: 1 / -1; }
    .peligro:not(:disabled) { color: #dc2626; }

    .page-card { padding: 1.5rem; }
    .page-title { font-size: 1.3rem; font-weight: 700; color: #f1f5f9; margin: 0 0 1.5rem; }
    .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.25rem; }
    .list-header .page-title { margin-bottom: 0; }
    .cargando { padding: 3rem; text-align: center; color: #64748b; display: flex; align-items: center; justify-content: center; gap: 0.5rem; }
    .sin-resultados { text-align: center; padding: 3rem 1rem; color: #64748b; display: flex; flex-direction: column; align-items: center; gap: 0.75rem; }

    .dias-badge {
      display: inline-block; padding: 0.15rem 0.6rem; border-radius: 999px;
      background: rgba(255,255,255,0.06); color: #e2e8f0; font-size: 0.82rem; font-weight: 600;
    }

    /* Form */
    .form-card {
      background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.07);
      border-radius: 10px; padding: 1.5rem;
    }
    .form-section { margin-bottom: 0.25rem; }
    .section-label { font-size: 0.73rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em; color: #818cf8; margin-bottom: 0.75rem; }
    .form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
    .grid-3 { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 1rem; }
    .field { display: flex; flex-direction: column; gap: 0.35rem; }
    .field label { font-weight: 500; font-size: 0.875rem; color: #e2e8f0; }
    .w-full { width: 100%; }

    .paciente-chip {
      display: flex; align-items: center; gap: 0.75rem;
      background: rgba(99,102,241,0.1); border: 1px solid rgba(99,102,241,0.25);
      border-radius: 8px; padding: 0.6rem 1rem; margin-top: 0.5rem;
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

    /* Diagnósticos */
    .dx-row { display: flex; gap: 1rem; margin-bottom: 1.25rem; flex-wrap: wrap; }
    .dx-card {
      flex: 1; min-width: 150px;
      background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08);
      border-radius: 8px; padding: 0.875rem 1rem;
    }
    .dx-code { font-size: 1.05rem; font-weight: 700; color: #a5b4fc; margin-top: 0.25rem; }

    /* Detalle */
    .detalle-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; margin-bottom: 1.25rem; }
    .detalle-bloque {}
    .texto-bloque {
      background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.07);
      border-radius: 6px; padding: 0.875rem 1rem;
      font-size: 0.875rem; color: #cbd5e1; line-height: 1.6; white-space: pre-wrap;
    }

    .step-actions { display: flex; gap: 0.5rem; justify-content: flex-end; border-top: 1px solid rgba(255,255,255,0.1); margin-top: 1.25rem; padding-top: 1rem; }

    ::ng-deep textarea { resize: vertical; }
  `]
})
export class EpicrisisComponent implements OnInit {
  private svc = inject(EpicrisisService);
  private messageService = inject(Avisos);
  private confirmationService = inject(Confirmar);

  vista = signal<Vista>('lista');
  readonly columnas = [
    'paciente', 'medico', 'ingreso', 'egreso',
    'dias', 'pronostico', 'incapacidad', 'acciones',
  ];

  cargando = signal(false);
  lista = signal<Epicrisis[]>([]);
  actual = signal<Epicrisis | null>(null);
  editando = signal(false);
  guardando = signal(false);

  // Búsqueda paciente
  busquedaPaciente = '';
  buscandoPaciente = signal(false);
  resultadosPaciente = signal<any[]>([]);
  pacienteSeleccionado: any = null;

  readonly opcionesPronostico = [
    { label: 'Bueno', value: 'BUENO' },
    { label: 'Reservado', value: 'RESERVADO' },
    { label: 'Malo', value: 'MALO' },
  ];

  f: Partial<EpicrisisCreate> = { pronostico: 'BUENO', incapacidad_dias: 0 };

  camposRequeridos = () => {
    const ok = this.f.fecha_ingreso && this.f.fecha_egreso &&
      this.f.diagnostico_ingreso && this.f.diagnostico_egreso &&
      this.f.resumen_evolucion && this.f.procedimientos_realizados &&
      this.f.recomendaciones_egreso;
    if (!this.editando() && !this.pacienteSeleccionado) return true;
    return !ok;
  };

  ngOnInit(): void { this.cargar(); }

  cargar(): void {
    this.cargando.set(true);
    this.svc.list().subscribe({
      next: es => { this.lista.set(es); this.cargando.set(false); },
      error: () => { this.cargando.set(false); this.err('Error al cargar epicrisis'); },
    });
  }

  abrirNueva(): void {
    this.editando.set(false);
    this.actual.set(null);
    this.busquedaPaciente = '';
    this.resultadosPaciente.set([]);
    this.pacienteSeleccionado = null;
    this.f = { pronostico: 'BUENO', incapacidad_dias: 0 };
    this.vista.set('form');
  }

  abrirEditar(e: Epicrisis): void {
    this.editando.set(true);
    this.actual.set(e);
    const toLocal = (iso: string) => iso ? iso.substring(0, 16) : '';
    this.f = {
      fecha_ingreso: toLocal(e.fecha_ingreso),
      fecha_egreso: toLocal(e.fecha_egreso),
      dias_estancia: e.dias_estancia,
      diagnostico_ingreso: e.diagnostico_ingreso,
      diagnostico_egreso: e.diagnostico_egreso,
      complicaciones: e.complicaciones,
      resumen_evolucion: e.resumen_evolucion,
      procedimientos_realizados: e.procedimientos_realizados,
      examenes_relevantes: e.examenes_relevantes,
      recomendaciones_egreso: e.recomendaciones_egreso,
      medicamentos_egreso: e.medicamentos_egreso,
      controles_programados: e.controles_programados,
      pronostico: e.pronostico,
      incapacidad_dias: e.incapacidad_dias,
    };
    this.vista.set('form');
  }

  abrirDetalle(e: Epicrisis): void {
    this.actual.set(e);
    this.vista.set('detalle');
  }

  volverLista(): void {
    this.vista.set('lista');
    this.cargar();
  }

  calcularDias(): void {
    if (this.f.fecha_ingreso && this.f.fecha_egreso) {
      const diff = new Date(this.f.fecha_egreso).getTime() - new Date(this.f.fecha_ingreso).getTime();
      this.f.dias_estancia = Math.max(0, Math.floor(diff / 86_400_000));
    }
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
      error: () => { this.buscandoPaciente.set(false); this.err('Error al buscar'); },
    });
  }

  guardar(): void {
    this.guardando.set(true);
    if (this.editando()) {
      this.svc.update(this.actual()!.id, this.f as Partial<EpicrisisCreate>).subscribe({
        next: () => { this.guardando.set(false); this.ok('Guardado', 'Epicrisis actualizada'); this.volverLista(); },
        error: err => { this.guardando.set(false); this.err(err?.error?.detail ?? 'Error al guardar'); },
      });
    } else {
      this.f.paciente_id = this.pacienteSeleccionado.id;
      this.svc.create(this.f as EpicrisisCreate).subscribe({
        next: () => { this.guardando.set(false); this.ok('Registrado', 'Epicrisis creada'); this.volverLista(); },
        error: err => { this.guardando.set(false); this.err(err?.error?.detail ?? 'Error al crear'); },
      });
    }
  }

  confirmarEliminar(e: Epicrisis): void {
    this.confirmationService.confirm({
      message: `¿Eliminar la epicrisis de <strong>${e.paciente_nombre}</strong>?`,
      header: 'Confirmar eliminación',
      icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Eliminar',
      rejectLabel: 'Cancelar',
      peligroso: true,
      accept: () => {
        this.svc.delete(e.id).subscribe({
          next: () => { this.lista.update(es => es.filter(x => x.id !== e.id)); this.ok('Eliminado'); },
          error: () => this.err('Error al eliminar'),
        });
      },
    });
  }

  pronosticoSeverity(p: string): 'success' | 'warn' | 'danger' {
    if (p === 'BUENO') return 'success';
    if (p === 'RESERVADO') return 'warn';
    return 'danger';
  }

  private ok(summary: string, detail?: string): void {
    this.messageService.add({ severity: 'success', summary, detail });
  }

  private err(detail: string): void {
    this.messageService.add({ severity: 'error', summary: 'Error', detail });
  }
}
