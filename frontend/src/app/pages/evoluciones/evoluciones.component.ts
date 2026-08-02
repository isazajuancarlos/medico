import { Component, OnInit, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatTableModule } from '@angular/material/table';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatDividerModule } from '@angular/material/divider';
import { Avisos } from '../../shared/ui/avisos.service';
import { Confirmar } from '../../shared/ui/confirmar.service';
import { EvolucionService, Evolucion, EvolucionCreate } from '../../services/evolucion.service';

type Vista = 'lista' | 'form';

@Component({
  selector: 'app-evoluciones',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatFormFieldModule, MatInputModule, MatTableModule,
    MatTooltipModule, MatDividerModule, MatSelectModule,
  ],
  template: `
    <div class="page-card">

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA LISTA                               -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'lista') {
        <div class="list-header">
          <h2 class="page-title">Evoluciones Médicas</h2>
          <button matButton="filled" (click)="abrirNueva()">
            <i class="pi pi-plus"></i> Nueva evolución
          </button>
        </div>

        <!-- Filtros -->
        <div class="filtros-bar">
          <div style="display:flex;gap:0.5rem;align-items:flex-start;flex:1">
            <mat-form-field appearance="outline" subscriptSizing="dynamic" style="flex:1">
              <mat-label>Paciente</mat-label>
              <input matInput [(ngModel)]="filtroPaciente"
                placeholder="Nombre o identificación…" (keyup.enter)="cargar()" />
            </mat-form-field>
            <button matButton="outlined" [disabled]="cargando()" (click)="cargar()"
              matTooltip="Buscar" aria-label="Buscar">
              <i [class]="cargando() ? 'pi pi-spin pi-spinner' : 'pi pi-search'"></i>
            </button>
            <button matIconButton (click)="limpiarFiltros()"
              matTooltip="Limpiar filtros" aria-label="Limpiar filtros">
              <i class="pi pi-refresh"></i>
            </button>
          </div>
          <div style="display:flex;gap:0.5rem">
            <mat-form-field appearance="outline" subscriptSizing="dynamic" class="fecha">
              <mat-label>Desde</mat-label>
              <input matInput [(ngModel)]="filtroDesde" type="date" (change)="cargar()" />
            </mat-form-field>
            <mat-form-field appearance="outline" subscriptSizing="dynamic" class="fecha">
              <mat-label>Hasta</mat-label>
              <input matInput [(ngModel)]="filtroHasta" type="date" (change)="cargar()" />
            </mat-form-field>
          </div>
        </div>

        @if (cargando()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (evoluciones().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-file-edit" style="font-size:2rem;opacity:0.4"></i>
            <p>No se encontraron evoluciones con los filtros aplicados.</p>
          </div>
        } @else {
          <table mat-table [dataSource]="evoluciones()">
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
              <th mat-header-cell *matHeaderCellDef>Médico</th>
              <td mat-cell *matCellDef="let e" class="menudo">{{ e.medico_nombre || '—' }}</td>
            </ng-container>
            <ng-container matColumnDef="fecha">
              <th mat-header-cell *matHeaderCellDef>Fecha</th>
              <td mat-cell *matCellDef="let e" class="menudo sin-cortar">
                {{ e.fecha | date:'dd/MM/yyyy HH:mm' }}
              </td>
            </ng-container>
            <ng-container matColumnDef="motivo">
              <th mat-header-cell *matHeaderCellDef>Motivo (subjetivo)</th>
              <td mat-cell *matCellDef="let e">
                <span class="texto-recortado">{{ e.subjetivo }}</span>
              </td>
            </ng-container>
            <ng-container matColumnDef="vitales">
              <th mat-header-cell *matHeaderCellDef class="centro">Signos vitales</th>
              <td mat-cell *matCellDef="let e" class="centro">
                @if (e.tension_arterial || e.frecuencia_cardiaca || e.temperatura || e.peso) {
                  <i class="pi pi-heart" style="color:#f472b6"
                    matTooltip="Signos vitales registrados"></i>
                } @else {
                  <span class="nada">—</span>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="acciones">
              <th mat-header-cell *matHeaderCellDef class="centro">Acciones</th>
              <td mat-cell *matCellDef="let e" class="centro sin-cortar">
                <button matIconButton matTooltip="Ver / editar" (click)="abrirEditar(e)"
                  aria-label="Ver o editar">
                  <i class="pi pi-file-edit"></i>
                </button>
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
      <!--  VISTA FORMULARIO SOAP                     -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'form') {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <button matIconButton (click)="volverLista()" matTooltip="Volver" aria-label="Volver">
            <i class="pi pi-arrow-left"></i>
          </button>
          <h2 class="page-title" style="margin:0">
            {{ editando() ? 'Editar evolución' : 'Nueva evolución médica' }}
          </h2>
        </div>

        <div class="form-card">

          <!-- Búsqueda de paciente (solo en nueva) -->
          @if (!editando()) {
            <div class="form-section">
              <div class="section-label">Paciente</div>
              <div style="display:flex;gap:0.5rem;align-items:flex-start">
                <mat-form-field appearance="outline" subscriptSizing="dynamic" style="flex:1">
                  <mat-label>Nombre o número de identificación</mat-label>
                  <input matInput [(ngModel)]="busquedaPaciente"
                    (keyup.enter)="buscarPaciente()" />
                </mat-form-field>
                <button matButton="filled" [disabled]="buscandoPaciente()"
                  (click)="buscarPaciente()">
                  <i [class]="buscandoPaciente() ? 'pi pi-spin pi-spinner' : 'pi pi-search'"></i>
                  Buscar
                </button>
              </div>
              @if (resultadosPaciente().length > 0) {
                <!-- mat-select no trae el buscador que p-select tenía dentro
                     ([filter] sobre nombre e identificación). No hace falta:
                     esta lista ES ya el resultado de una búsqueda explícita, y
                     la identificación pasa a leerse en la propia opción, que
                     es para lo que servía filtrar por ella — distinguir dos
                     pacientes con el mismo nombre. -->
                <mat-form-field appearance="outline" subscriptSizing="dynamic"
                  class="w-full separa">
                  <mat-label>Seleccione el paciente</mat-label>
                  <mat-select [(ngModel)]="pacienteSeleccionado">
                    @for (p of resultadosPaciente(); track p.id) {
                      <mat-option [value]="p">
                        {{ p.nombre_completo }} — {{ p.identificacion }}
                      </mat-option>
                    }
                  </mat-select>
                </mat-form-field>
              }
              @if (pacienteSeleccionado) {
                <div class="paciente-chip">
                  <i class="pi pi-user" style="color:#818cf8"></i>
                  <div>
                    <span style="font-weight:600;color:#f1f5f9">{{ pacienteSeleccionado.nombre_completo }}</span>
                    <span style="font-size:0.82rem;color:#94a3b8;margin-left:0.5rem">{{ pacienteSeleccionado.identificacion }}</span>
                  </div>
                </div>
              }
            </div>
            <mat-divider />
          } @else {
            <div class="paciente-chip" style="margin-bottom:1rem">
              <i class="pi pi-user" style="color:#818cf8"></i>
              <div>
                <span style="font-weight:600;color:#f1f5f9">{{ evolucionActual()?.paciente_nombre }}</span>
                <span style="font-size:0.82rem;color:#94a3b8;margin-left:0.5rem">{{ evolucionActual()?.paciente_identificacion }}</span>
              </div>
            </div>
          }

          <!-- SOAP -->
          <div class="form-section">
            <div class="section-label">Nota de evolución — formato SOAP</div>
            <div class="soap-grid">
              <div class="soap-field">
                <div class="soap-letter soap-s">S</div>
                <mat-form-field appearance="outline" subscriptSizing="dynamic" class="w-full">
                  <mat-label>Subjetivo *</mat-label>
                  <textarea matInput [(ngModel)]="form.subjetivo" rows="4"
                    placeholder="Lo que refiere el paciente: síntomas, quejas, dolor (escala 1-10), evolución desde última consulta…"></textarea>
                </mat-form-field>
              </div>
              <div class="soap-field">
                <div class="soap-letter soap-o">O</div>
                <mat-form-field appearance="outline" subscriptSizing="dynamic" class="w-full">
                  <mat-label>Objetivo *</mat-label>
                  <textarea matInput [(ngModel)]="form.objetivo" rows="4"
                    placeholder="Hallazgos del examen físico: aspecto general, signos, auscultación, palpación…"></textarea>
                </mat-form-field>
              </div>
              <div class="soap-field">
                <div class="soap-letter soap-a">A</div>
                <mat-form-field appearance="outline" subscriptSizing="dynamic" class="w-full">
                  <mat-label>Análisis / Evaluación *</mat-label>
                  <textarea matInput [(ngModel)]="form.analisis" rows="4"
                    placeholder="Interpretación clínica, diagnósticos diferenciales, evolución de la condición…"></textarea>
                </mat-form-field>
              </div>
              <div class="soap-field">
                <div class="soap-letter soap-p">P</div>
                <mat-form-field appearance="outline" subscriptSizing="dynamic" class="w-full">
                  <mat-label>Plan *</mat-label>
                  <textarea matInput [(ngModel)]="form.plan" rows="4"
                    placeholder="Conducta: cambios en medicación, nuevas órdenes, interconsultas, próxima cita…"></textarea>
                </mat-form-field>
              </div>
            </div>
          </div>

          <mat-divider />

          <!-- Signos vitales -->
          <div class="form-section">
            <div class="section-label">Signos vitales de control <span style="color:#64748b;font-weight:400">(opcional)</span></div>
            <div class="vitales-grid">
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Tensión arterial</mat-label>
                <input matInput [(ngModel)]="form.tension_arterial" placeholder="120/80 mmHg" />
              </mat-form-field>
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Frecuencia cardíaca</mat-label>
                <input matInput [(ngModel)]="form.frecuencia_cardiaca" type="number" />
                <span matTextSuffix>lpm</span>
              </mat-form-field>
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Temperatura</mat-label>
                <input matInput [(ngModel)]="form.temperatura" type="number"
                  step="0.1" placeholder="36.5" />
                <span matTextSuffix>°C</span>
              </mat-form-field>
              <mat-form-field appearance="outline" subscriptSizing="dynamic">
                <mat-label>Peso</mat-label>
                <input matInput [(ngModel)]="form.peso" type="number"
                  step="0.1" placeholder="70.0" />
                <span matTextSuffix>kg</span>
              </mat-form-field>
            </div>
          </div>

          <div class="step-actions">
            <button matButton (click)="volverLista()">Cancelar</button>
            <button matButton="filled" (click)="guardar()"
              [disabled]="guardando() || !puedeGuardar()">
              <i [class]="guardando() ? 'pi pi-spin pi-spinner' : 'pi pi-check'"></i>
              {{ editando() ? 'Guardar cambios' : 'Registrar evolución' }}
            </button>
          </div>
        </div>
      }

    </div>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .page-card { padding: 1.5rem; }
    .page-title { font-size: 1.3rem; font-weight: 700; color: #f1f5f9; margin: 0 0 1.5rem; }
    .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
    .list-header .page-title { margin-bottom: 0; }
    .cargando { padding: 3rem; text-align: center; color: #64748b; display: flex; align-items: center; justify-content: center; gap: 0.5rem; }
    .sin-resultados { text-align: center; padding: 3rem 1rem; color: #64748b; display: flex; flex-direction: column; align-items: center; gap: 0.75rem; }

    .filtros-bar { display: flex; gap: 0.75rem; align-items: flex-start; margin-bottom: 1rem; flex-wrap: wrap; }
    table { width: 100%; }
    .centro { text-align: center; }
    .menudo { font-size: 0.85rem; }
    .sin-cortar { white-space: nowrap; }
    .sub { font-size: 0.78rem; color: #94a3b8; }
    .nada { color: #475569; font-size: 0.8rem; }
    .fecha { width: 160px; }
    .separa { margin-top: 0.75rem; }
    .peligro:not(:disabled) { color: #dc2626; }

    .texto-recortado {
      display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical;
      overflow: hidden; font-size: 0.85rem; color: #cbd5e1; max-width: 300px;
    }

    /* Form */
    .form-card {
      background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.07);
      border-radius: 10px; padding: 1.5rem;
    }
    .form-section { margin-bottom: 0.25rem; }
    .section-label { font-size: 0.73rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em; color: #818cf8; margin-bottom: 1rem; }
    .req { color: #f87171; }

    .paciente-chip {
      display: flex; align-items: center; gap: 0.75rem;
      background: rgba(99,102,241,0.1); border: 1px solid rgba(99,102,241,0.25);
      border-radius: 8px; padding: 0.6rem 1rem; margin-top: 0.75rem;
    }

    /* SOAP grid */
    .soap-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
    .soap-field { display: flex; gap: 0.75rem; align-items: flex-start; }
    .soap-letter {
      width: 28px; height: 28px; border-radius: 6px; flex-shrink: 0;
      display: flex; align-items: center; justify-content: center;
      font-size: 0.875rem; font-weight: 800; margin-top: 1.6rem;
    }
    .soap-s { background: rgba(99,102,241,0.2); color: #818cf8; }
    .soap-o { background: rgba(56,189,248,0.2); color: #38bdf8; }
    .soap-a { background: rgba(251,191,36,0.2); color: #fbbf24; }
    .soap-p { background: rgba(74,222,128,0.2); color: #4ade80; }

    .vitales-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 1rem; }

    .field { display: flex; flex-direction: column; gap: 0.35rem; flex: 1; }
    .field label { font-weight: 500; font-size: 0.875rem; color: #e2e8f0; }
    .w-full { width: 100%; }
    .step-actions { display: flex; gap: 0.5rem; justify-content: flex-end; padding-top: 1.25rem; border-top: 1px solid rgba(255,255,255,0.1); margin-top: 1rem; }

    ::ng-deep textarea { resize: vertical; }
  `]
})
export class EvolucionesComponent implements OnInit {
  private svc = inject(EvolucionService);
  private messageService = inject(Avisos);
  private confirmationService = inject(Confirmar);

  readonly columnas = ['paciente', 'medico', 'fecha', 'motivo', 'vitales', 'acciones'];

  vista = signal<Vista>('lista');
  cargando = signal(false);
  evoluciones = signal<Evolucion[]>([]);

  // Filtros lista
  filtroPaciente = '';
  filtroDesde = '';
  filtroHasta = '';

  // Form
  editando = signal(false);
  evolucionActual = signal<Evolucion | null>(null);
  guardando = signal(false);
  form: Partial<EvolucionCreate> & {
    tension_arterial?: string;
    frecuencia_cardiaca?: number;
    temperatura?: number;
    peso?: number;
  } = {};

  // Búsqueda paciente
  busquedaPaciente = '';
  buscandoPaciente = signal(false);
  resultadosPaciente = signal<any[]>([]);
  pacienteSeleccionado: any = null;

  ngOnInit(): void {
    this.cargar();
  }

  cargar(): void {
    this.cargando.set(true);
    const params: any = {};
    if (this.filtroDesde) params.fecha_desde = this.filtroDesde;
    if (this.filtroHasta) params.fecha_hasta = this.filtroHasta;
    // If patient filter typed, search first
    this.svc.list(params).subscribe({
      next: es => { this.evoluciones.set(es); this.cargando.set(false); },
      error: () => { this.cargando.set(false); this.err('Error al cargar evoluciones'); },
    });
  }

  limpiarFiltros(): void {
    this.filtroPaciente = '';
    this.filtroDesde = '';
    this.filtroHasta = '';
    this.cargar();
  }

  abrirNueva(): void {
    this.editando.set(false);
    this.evolucionActual.set(null);
    this.pacienteSeleccionado = null;
    this.busquedaPaciente = '';
    this.resultadosPaciente.set([]);
    this.form = {};
    this.vista.set('form');
  }

  abrirEditar(e: Evolucion): void {
    this.editando.set(true);
    this.evolucionActual.set(e);
    this.form = {
      subjetivo: e.subjetivo,
      objetivo: e.objetivo,
      analisis: e.analisis,
      plan: e.plan,
      tension_arterial: e.tension_arterial,
      frecuencia_cardiaca: e.frecuencia_cardiaca,
      temperatura: e.temperatura,
      peso: e.peso,
    };
    this.vista.set('form');
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

  /**
   * Las cuatro letras del SOAP y, si es un alta, el paciente.
   *
   * Estaba escrito dos veces —una en el `[disabled]` del botón y otra al
   * entrar en `guardar()`— y las dos versiones **no decían lo mismo**: la del
   * botón exigía además el paciente y la del método no. Una nota sin paciente
   * no la frenaba el código, solo el botón.
   */
  puedeGuardar(): boolean {
    const f = this.form;
    const soap = !!f.subjetivo && !!f.objetivo && !!f.analisis && !!f.plan;
    return soap && (this.editando() || !!this.pacienteSeleccionado);
  }

  guardar(): void {
    if (!this.puedeGuardar()) return;
    this.guardando.set(true);

    if (this.editando()) {
      this.svc.update(this.evolucionActual()!.id, this.form).subscribe({
        next: () => {
          this.guardando.set(false);
          this.ok('Guardado', 'Evolución actualizada correctamente');
          this.volverLista();
        },
        error: err => { this.guardando.set(false); this.err(err?.error?.detail ?? 'Error al guardar'); },
      });
    } else {
      if (!this.pacienteSeleccionado) return;
      const payload: EvolucionCreate = {
        paciente_id: this.pacienteSeleccionado.id,
        subjetivo: this.form.subjetivo!,
        objetivo: this.form.objetivo!,
        analisis: this.form.analisis!,
        plan: this.form.plan!,
        tension_arterial: this.form.tension_arterial,
        frecuencia_cardiaca: this.form.frecuencia_cardiaca,
        temperatura: this.form.temperatura,
        peso: this.form.peso,
      };
      this.svc.create(payload).subscribe({
        next: () => {
          this.guardando.set(false);
          this.ok('Registrado', 'Evolución médica registrada');
          this.volverLista();
        },
        error: err => { this.guardando.set(false); this.err(err?.error?.detail ?? 'Error al registrar'); },
      });
    }
  }

  confirmarEliminar(e: Evolucion): void {
    this.confirmationService.confirm({
      message: `¿Eliminar la evolución del ${new Date(e.fecha).toLocaleDateString('es-CO')} de <strong>${e.paciente_nombre}</strong>?`,
      header: 'Confirmar eliminación',
      icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Eliminar',
      rejectLabel: 'Cancelar',
      peligroso: true,
      accept: () => {
        this.svc.delete(e.id).subscribe({
          next: () => {
            this.evoluciones.update(es => es.filter(x => x.id !== e.id));
            this.ok('Eliminado', 'Evolución eliminada');
          },
          error: () => this.err('Error al eliminar'),
        });
      },
    });
  }

  private ok(summary: string, detail?: string): void {
    this.messageService.add({ severity: 'success', summary, detail });
  }

  private err(detail: string): void {
    this.messageService.add({ severity: 'error', summary: 'Error', detail });
  }
}
