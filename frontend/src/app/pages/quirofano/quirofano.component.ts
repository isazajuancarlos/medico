import { Component, Inject, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
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
  QuirofanoService, Sala, Programacion, ProgramacionCreate,
  ChecklistUpdate, CompletarCirugia, MedicoBasic,
} from '../../services/quirofano.service';

type Vista = 'lista' | 'programar';
type Tab = 'agenda' | 'salas';

/** El vocabulario del estado, en un solo sitio: lo leen la pantalla y el
 *  diálogo de detalle. */
const ESTADO_LABEL: Record<string, string> = {
  PROGRAMADA: 'Programada', EN_CURSO: 'En curso', COMPLETADA: 'Completada',
  CANCELADA: 'Cancelada', SUSPENDIDA: 'Suspendida',
};

function estadoSeveridad(estado: string): 'info' | 'success' | 'warn' | 'danger' | 'secondary' {
  if (estado === 'PROGRAMADA') return 'info';
  if (estado === 'EN_CURSO') return 'warn';
  if (estado === 'COMPLETADA') return 'success';
  if (estado === 'CANCELADA' || estado === 'SUSPENDIDA') return 'danger';
  return 'secondary';
}

/** Lo que el diálogo de detalle pide hacer al cerrarse. */
type AccionDetalle = 'iniciar' | 'cancelar';

/**
 * Detalle de una cirugía, con su checklist prequirúrgico.
 *
 * El checklist se guarda AQUÍ, marca a marca, y no al cerrar: cada casilla es
 * una afirmación sobre el paciente —ayuno verificado, sangre reservada— y
 * perderlas porque alguien cerró con Escape sería peor que no ofrecerlas. Lo
 * que sí sale por afterClosed() es la acción sobre la cirugía, que es la que
 * cambia su estado.
 */
@Component({
  selector: 'app-quirofano-detalle',
  standalone: true,
  imports: [
    CommonModule, FormsModule, MatDialogModule, MatButtonModule,
    MatCheckboxModule, MatDividerModule, EtiquetaComponent,
  ],
  template: `
    <h2 mat-dialog-title>Detalle de cirugía</h2>
    <mat-dialog-content>
      <div class="detalle-header">
        <div>
          <div class="det-label">Paciente</div>
          <div class="det-valor">{{ p().paciente_nombre || '—' }}</div>
          <div class="det-sub">{{ p().paciente_identificacion }}</div>
        </div>
        <x-etiqueta [value]="estadoLabel(p().estado)" [severity]="estadoSeverity(p().estado)" />
      </div>

      <mat-divider />

      <div class="det-grid">
        <div>
          <div class="det-label">Procedimiento</div>
          <div class="det-valor">{{ p().procedimiento }}</div>
        </div>
        <div>
          <div class="det-label">Sala</div>
          <div class="det-valor">{{ p().quirofano_nombre || '—' }}</div>
        </div>
        <div>
          <div class="det-label">Cirujano</div>
          <div class="det-valor">{{ p().cirujano_nombre || '—' }}</div>
        </div>
        <div>
          <div class="det-label">Anestesiólogo</div>
          <div class="det-valor">{{ p().anestesiologo_nombre || 'No asignado' }}</div>
        </div>
        <div>
          <div class="det-label">Fecha programada</div>
          <div class="det-valor">{{ p().fecha_programada | date:'dd/MM/yyyy HH:mm' }}</div>
        </div>
        <div>
          <div class="det-label">Duración estimada</div>
          <div class="det-valor">{{ p().duracion_estimada_min }} min</div>
        </div>
        @if (p().tipo_anestesia) {
          <div>
            <div class="det-label">Anestesia</div>
            <div class="det-valor">{{ p().tipo_anestesia }}</div>
          </div>
        }
        @if (p().hora_inicio_real) {
          <div>
            <div class="det-label">Hora inicio real</div>
            <div class="det-valor">{{ p().hora_inicio_real | date:'HH:mm' }}</div>
          </div>
        }
        @if (p().hora_fin_real) {
          <div>
            <div class="det-label">Hora fin real</div>
            <div class="det-valor">{{ p().hora_fin_real | date:'HH:mm' }}</div>
          </div>
        }
      </div>

      @if (p().estado === 'PROGRAMADA') {
        <mat-divider />
        <div class="section-title">Checklist prequirúrgico</div>
        <div class="checklist-grid">
          <mat-checkbox [(ngModel)]="checklist.ayuno_verificado"
            (change)="guardar('ayuno_verificado', $event.checked)">Ayuno verificado</mat-checkbox>
          <mat-checkbox [(ngModel)]="checklist.labs_prequirurgicos"
            (change)="guardar('labs_prequirurgicos', $event.checked)">Laboratorios prequirúrgicos</mat-checkbox>
          <mat-checkbox [(ngModel)]="checklist.valoracion_preanestesica"
            (change)="guardar('valoracion_preanestesica', $event.checked)">Valoración preanestésica</mat-checkbox>
          <mat-checkbox [(ngModel)]="checklist.reserva_sangre"
            (change)="guardar('reserva_sangre', $event.checked)">Reserva de sangre</mat-checkbox>
        </div>
      }

      @if (p().nota_quirurgica) {
        <mat-divider />
        <div class="det-label">Nota quirúrgica</div>
        <div class="nota-box">{{ p().nota_quirurgica }}</div>
      }
      @if (p().hallazgos_quirurgicos) {
        <div class="det-label" style="margin-top:0.75rem">Hallazgos</div>
        <div class="nota-box">{{ p().hallazgos_quirurgicos }}</div>
      }
      @if (p().complicaciones) {
        <div class="det-label" style="margin-top:0.75rem">Complicaciones / Notas</div>
        <div class="nota-box warn">{{ p().complicaciones }}</div>
      }
    </mat-dialog-content>
    <mat-dialog-actions class="acciones">
      <div>
        @if (p().estado === 'PROGRAMADA') {
          <button matButton class="peligro" [mat-dialog-close]="'cancelar'">
            <i class="pi pi-times"></i> Cancelar cirugía
          </button>
        }
      </div>
      <div style="display:flex;gap:0.5rem">
        <button matButton mat-dialog-close>Cerrar</button>
        @if (p().estado === 'PROGRAMADA') {
          <button matButton="filled" class="exito" [mat-dialog-close]="'iniciar'">
            <i class="pi pi-play"></i> Iniciar cirugía
          </button>
        }
      </div>
    </mat-dialog-actions>
  `,
  styles: [`
    mat-dialog-content { min-width: min(36rem, 88vw); }
    .detalle-header { display:flex; justify-content:space-between; align-items:flex-start; gap:1rem; margin-bottom:1rem; }
    .det-label { font-size:0.7rem; font-weight:600; text-transform:uppercase; color:#94a3b8; letter-spacing:0.05em; }
    .det-valor { font-weight:600; color:#f1f5f9; }
    .det-sub { font-size:0.82rem; color:#94a3b8; }
    .det-grid { display:grid; grid-template-columns:1fr 1fr; gap:0.9rem; margin:1rem 0; }
    .section-title { font-size:0.75rem; font-weight:700; text-transform:uppercase; letter-spacing:0.07em;
      color:#818cf8; margin:1rem 0 0.75rem; }
    .checklist-grid { display:flex; flex-direction:column; gap:0.5rem; }
    .nota-box { background:rgba(255,255,255,0.03); border:1px solid rgba(255,255,255,0.07);
      border-radius:6px; padding:0.75rem; font-size:0.875rem; color:#cbd5e1; white-space:pre-wrap; }
    .nota-box.warn { border-color:rgba(251,191,36,0.3); background:rgba(251,191,36,0.06); }
    .acciones { display:flex; justify-content:space-between; }
    .peligro { color:#dc2626; }
    .exito { background:#16a34a; color:#fff; }
  `],
})
export class QuirofanoDetalleComponent {
  private svc = inject(QuirofanoService);
  private avisos = inject(Avisos);

  readonly p = signal<Programacion>(null as unknown as Programacion);
  checklist: ChecklistUpdate;

  constructor(@Inject(MAT_DIALOG_DATA) prog: Programacion) {
    this.p.set(prog);
    this.checklist = {
      ayuno_verificado: prog.ayuno_verificado,
      labs_prequirurgicos: prog.labs_prequirurgicos,
      valoracion_preanestesica: prog.valoracion_preanestesica,
      reserva_sangre: prog.reserva_sangre,
    };
  }

  guardar(campo: keyof ChecklistUpdate, valor: boolean): void {
    this.svc.updateChecklist(this.p().id, { [campo]: valor }).subscribe({
      next: (actualizada) => this.p.set(actualizada),
      error: () => this.avisos.add({
        severity: 'error', summary: 'Error', detail: 'No se pudo actualizar el checklist',
      }),
    });
  }

  estadoLabel(estado: string): string { return ESTADO_LABEL[estado] ?? estado; }
  estadoSeverity(estado: string): 'info' | 'success' | 'warn' | 'danger' | 'secondary' {
    return estadoSeveridad(estado);
  }
}

/** Cierre de una cirugía. Devuelve lo que se registra, o nada. */
@Component({
  selector: 'app-quirofano-completar',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatInputModule,
  ],
  template: `
    <h2 mat-dialog-title>Completar cirugía</h2>
    <mat-dialog-content>
      <div class="ficha">
        <div style="font-weight:600;color:#f1f5f9">{{ p.paciente_nombre }}</div>
        <div style="font-size:0.82rem;color:#94a3b8">{{ p.procedimiento }}</div>
      </div>
      <div class="rejilla">
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Nota quirúrgica / descripción del procedimiento</mat-label>
          <textarea matInput [(ngModel)]="f.nota_quirurgica" rows="4"
            placeholder="Descripción de la técnica quirúrgica..."></textarea>
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Hallazgos quirúrgicos</mat-label>
          <textarea matInput [(ngModel)]="f.hallazgos_quirurgicos" rows="3"
            placeholder="Hallazgos intraoperatorios..."></textarea>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Sangrado estimado (mL)</mat-label>
          <input matInput type="number" [(ngModel)]="f.sangrado_estimado_ml" placeholder="0" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Especímenes para patología</mat-label>
          <input matInput [(ngModel)]="f.especimenes_patologia" placeholder="Vesícula biliar, apéndice..." />
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Complicaciones intraoperatorias</mat-label>
          <textarea matInput [(ngModel)]="f.complicaciones" rows="2"
            placeholder="Ninguna / descripción..."></textarea>
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [mat-dialog-close]="f">
        <i class="pi pi-check"></i> Registrar cierre
      </button>
    </mat-dialog-actions>
  `,
  styles: [`
    .ficha { margin-bottom:1rem; padding:0.75rem; background:rgba(99,102,241,0.1);
      border-radius:6px; border:1px solid rgba(99,102,241,0.2); }
    .rejilla { display:grid; grid-template-columns:1fr 1fr; gap:1rem;
      min-width:min(32rem, 85vw); }
    .ancho { grid-column:1/-1; }
    mat-form-field { width:100%; }
  `],
})
export class QuirofanoCompletarComponent {
  f: CompletarCirugia = {};
  constructor(@Inject(MAT_DIALOG_DATA) public p: Programacion) {}
}


@Component({
  selector: 'app-quirofano',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatFormFieldModule, MatInputModule, MatSelectModule,
    MatTableModule, EtiquetaComponent, MatTooltipModule, MatDividerModule,
  ],
  template: `
    <div class="page-card">

      <!-- ═══════════════════════════ VISTA LISTA ═══════════════════════════ -->
      @if (vista() === 'lista') {
        <div class="list-header">
          <h2 class="page-title">Quirófano</h2>
          <div style="display:flex;gap:0.5rem;align-items:center">
            <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:200px">
              <mat-label>Estado</mat-label>
              <mat-select [(ngModel)]="filtroEstado" (selectionChange)="cargarProgramaciones()">
                @for (o of opcionesEstado; track o.value) {
                  <mat-option [value]="o.value">{{ o.label }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
            <button matButton="filled" (click)="vista.set('programar')">
              <i class="pi pi-plus"></i> Nueva programación
            </button>
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
            <table mat-table [dataSource]="programaciones()" class="tabla">
              <ng-container matColumnDef="paciente">
                <th mat-header-cell *matHeaderCellDef>Paciente</th>
                <td mat-cell *matCellDef="let p">
                  <strong>{{ p.paciente_nombre || '—' }}</strong>
                  @if (p.paciente_identificacion) {
                    <div style="font-size:0.78rem;color:#94a3b8">{{ p.paciente_identificacion }}</div>
                  }
                </td>
              </ng-container>
              <ng-container matColumnDef="procedimiento">
                <th mat-header-cell *matHeaderCellDef>Procedimiento</th>
                <td mat-cell *matCellDef="let p">
                  <span style="font-size:0.88rem">{{ p.procedimiento }}</span>
                  @if (p.codigo_cups) {
                    <div style="font-size:0.75rem;color:#94a3b8">CUPS: {{ p.codigo_cups }}</div>
                  }
                </td>
              </ng-container>
              <ng-container matColumnDef="sala">
                <th mat-header-cell *matHeaderCellDef>Sala</th>
                <td mat-cell *matCellDef="let p"><span class="sala-badge">{{ p.quirofano_nombre || '—' }}</span></td>
              </ng-container>
              <ng-container matColumnDef="cirujano">
                <th mat-header-cell *matHeaderCellDef>Cirujano</th>
                <td mat-cell *matCellDef="let p" style="font-size:0.85rem">{{ p.cirujano_nombre || '—' }}</td>
              </ng-container>
              <ng-container matColumnDef="fecha">
                <th mat-header-cell *matHeaderCellDef>Fecha / Hora</th>
                <td mat-cell *matCellDef="let p" style="font-size:0.85rem">
                  {{ p.fecha_programada | date:'dd/MM/yyyy' }}
                  <div style="font-size:0.78rem;color:#94a3b8">{{ p.fecha_programada | date:'HH:mm' }}</div>
                </td>
              </ng-container>
              <ng-container matColumnDef="duracion">
                <th mat-header-cell *matHeaderCellDef class="centro">Duración</th>
                <td mat-cell *matCellDef="let p" class="centro">
                  <span class="dur-badge">{{ p.duracion_estimada_min }} min</span>
                </td>
              </ng-container>
              <ng-container matColumnDef="checklist">
                <th mat-header-cell *matHeaderCellDef class="centro">Checklist</th>
                <td mat-cell *matCellDef="let p" class="centro">
                  <span [class]="checklistCompleto(p) ? 'check-ok' : 'check-pendiente'">
                    <i [class]="checklistCompleto(p) ? 'pi pi-check-circle' : 'pi pi-exclamation-circle'"></i>
                    {{ checklistItems(p) }}/4
                  </span>
                </td>
              </ng-container>
              <ng-container matColumnDef="estado">
                <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
                <td mat-cell *matCellDef="let p" class="centro">
                  <x-etiqueta [value]="estadoLabel(p.estado)" [severity]="estadoSeverity(p.estado)" />
                </td>
              </ng-container>
              <ng-container matColumnDef="acciones">
                <th mat-header-cell *matHeaderCellDef class="centro">Acciones</th>
                <td mat-cell *matCellDef="let p" class="centro">
                  <button matIconButton matTooltip="Ver detalle" (click)="abrirDetalle(p)">
                    <i class="pi pi-eye"></i>
                  </button>
                  @if (p.estado === 'PROGRAMADA') {
                    <button matIconButton class="exito-texto" matTooltip="Iniciar cirugía"
                      (click)="iniciarCirugia(p)">
                      <i class="pi pi-play"></i>
                    </button>
                  }
                  @if (p.estado === 'EN_CURSO') {
                    <button matIconButton class="aviso-color" matTooltip="Completar cirugía"
                      (click)="abrirCompletar(p)">
                      <i class="pi pi-stop-circle"></i>
                    </button>
                  }
                </td>
              </ng-container>

              <tr mat-header-row *matHeaderRowDef="columnas"></tr>
              <tr mat-row *matRowDef="let fila; columns: columnas"></tr>
            </table>
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
          <button matIconButton matTooltip="Volver" (click)="volverLista()">
            <i class="pi pi-arrow-left"></i>
          </button>
          <h2 class="page-title" style="margin:0">Nueva Programación Quirúrgica</h2>
        </div>

        <div class="form-card">

          <!-- Paciente -->
          <div class="form-section">
            <div class="section-title">Paciente</div>
            <div style="display:flex;gap:0.5rem;align-items:flex-start">
              <mat-form-field appearance="outline" subscriptSizing="dynamic" style="flex:1">
                <mat-label>Buscar por nombre o identificación</mat-label>
                <input matInput [(ngModel)]="busquedaPaciente"
                  placeholder="Ingrese nombre o número de identificación..."
                  (keyup.enter)="buscarPaciente()" />
              </mat-form-field>
              <button matButton="filled" [disabled]="buscandoPaciente()" (click)="buscarPaciente()">
                <i [class]="buscandoPaciente() ? 'pi pi-spin pi-spinner' : 'pi pi-search'"></i> Buscar
              </button>
            </div>
            @if (pacientesResultados().length > 0) {
              <mat-form-field appearance="outline" subscriptSizing="dynamic" style="margin-top:0.75rem">
                <mat-label>Seleccione el paciente…</mat-label>
                <mat-select [(ngModel)]="pacienteSeleccionado">
                  @for (p of pacientesResultados(); track p.id) {
                    <mat-option [value]="p">{{ p.nombre_completo }} — {{ p.identificacion }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
            }
            @if (pacienteSeleccionado) {
              <div class="paciente-card">
                <i class="pi pi-user" style="color:#818cf8"></i>
                <div>
                  <div style="font-weight:600;color:#f1f5f9">{{ pacienteSeleccionado.nombre_completo }}</div>
                  <div style="font-size:0.82rem;color:#94a3b8">{{ pacienteSeleccionado.identificacion }}</div>
                </div>
                <button matIconButton matTooltip="Quitar" (click)="pacienteSeleccionado = null">
                  <i class="pi pi-times"></i>
                </button>
              </div>
            }
          </div>

          <mat-divider />

          <!-- Procedimiento -->
          <div class="form-section">
            <div class="section-title">Procedimiento quirúrgico</div>
            <div class="form-grid">
              <mat-form-field appearance="outline" style="grid-column:1/-1">
                <mat-label>Descripción del procedimiento *</mat-label>
                <input matInput [(ngModel)]="nForm.procedimiento"
                  placeholder="Ej: Colecistectomía laparoscópica, Apendicectomía..." />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Código CUPS</mat-label>
                <input matInput [(ngModel)]="nForm.codigo_cups" placeholder="Ej: 472200" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Tipo de anestesia</mat-label>
                <mat-select [(ngModel)]="nForm.tipo_anestesia">
                  @for (o of opcionesAnestesia; track o.value) {
                    <mat-option [value]="o.value">{{ o.label }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
            </div>
          </div>

          <mat-divider />

          <!-- Sala, cirujano, fecha -->
          <div class="form-section">
            <div class="section-title">Asignación</div>
            <div class="form-grid">
              <mat-form-field appearance="outline">
                <mat-label>Sala de quirófano *</mat-label>
                <mat-select [(ngModel)]="nForm.quirofano_id">
                  @for (s of salasDisp(); track s.value) {
                    <mat-option [value]="s.value">{{ s.label }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Cirujano *</mat-label>
                <mat-select [(ngModel)]="nForm.cirujano_id">
                  @for (m of medicos(); track m.id) {
                    <mat-option [value]="m.id">{{ m.nombre_completo }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Anestesiólogo</mat-label>
                <mat-select [(ngModel)]="nForm.anestesiologo_id">
                  <mat-option [value]="null">Sin asignar</mat-option>
                  @for (m of anestesiologos(); track m.id) {
                    <mat-option [value]="m.id">{{ m.nombre_completo }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Duración estimada (min)</mat-label>
                <input matInput type="number" [(ngModel)]="nForm.duracion_estimada_min" placeholder="60" />
              </mat-form-field>
              <mat-form-field appearance="outline" style="grid-column:1/-1">
                <mat-label>Fecha y hora programada *</mat-label>
                <input matInput type="datetime-local" [(ngModel)]="nForm.fecha_programada" />
              </mat-form-field>
            </div>
          </div>

          <div class="step-actions">
            <button matButton (click)="volverLista()">Cancelar</button>
            <button matButton="filled" (click)="guardar()"
              [disabled]="guardando() || !pacienteSeleccionado || !nForm.quirofano_id || !nForm.cirujano_id || !nForm.procedimiento || !nForm.fecha_programada">
              <i [class]="guardando() ? 'pi pi-spin pi-spinner' : 'pi pi-check'"></i> Programar cirugía
            </button>
          </div>
        </div>
      }
    </div>

  `,
  styles: [`
    .page-card { padding: 1.5rem; }
    .page-title { font-size: 1.3rem; font-weight: 700; color: #f1f5f9; margin: 0 0 1.5rem; }
    .tabla { width: 100%; }
    .centro { text-align: center; }
    mat-form-field { width: 100%; }
    /* En M3 el input color de un botón no pinta nada: el color va por CSS. */
    .exito-texto { color: #16a34a; }
    .aviso-color { color: #f59e0b; }
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
  private dialogo = inject(MatDialog);
  private messageService = inject(Avisos);

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

  /** El orden de las columnas lo fija esta lista, no la plantilla. */
  readonly columnas = ['paciente', 'procedimiento', 'sala', 'cirujano', 'fecha',
                       'duracion', 'checklist', 'estado', 'acciones'];

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

  accionando = signal(false);

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
    this.dialogo
      .open(QuirofanoDetalleComponent, { data: p, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((accion?: AccionDetalle) => {
        // El checklist se guardó dentro del diálogo, marca a marca, así que la
        // lista se recarga pase lo que pase: el conteo de la columna cambió.
        this.cargarProgramaciones();
        if (accion === 'iniciar') this.iniciarCirugia(p);
        if (accion === 'cancelar') this.cancelarCirugia(p);
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

  cancelarCirugia(p: Programacion): void {
    this.accionando.set(true);
    this.svc.cancelar(p.id, 'Cancelada desde detalle').subscribe({
      next: () => {
        this.accionando.set(false);
        this.ok('Cirugía cancelada');
        this.cargarProgramaciones();
        this.cargarSalas();
      },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error al cancelar'); },
    });
  }

  abrirCompletar(p: Programacion): void {
    this.dialogo
      .open(QuirofanoCompletarComponent, { data: p, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((cierre?: CompletarCirugia) => { if (cierre) this.completar(p, cierre); });
  }

  private completar(p: Programacion, cierre: CompletarCirugia): void {
    this.accionando.set(true);
    this.svc.completar(p.id, cierre).subscribe({
      next: updated => {
        this.accionando.set(false);
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

  estadoLabel(estado: string): string { return ESTADO_LABEL[estado] ?? estado; }

  estadoSeverity(estado: string): 'info' | 'success' | 'warn' | 'danger' | 'secondary' {
    return estadoSeveridad(estado);
  }

  private ok(summary: string, detail?: string): void {
    this.messageService.add({ severity: 'success', summary, detail });
  }

  private err(detail: string): void {
    this.messageService.add({ severity: 'error', summary: 'Error', detail });
  }
}
