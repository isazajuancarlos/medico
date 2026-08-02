import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { TextareaModule } from 'primeng/textarea';
import { TableModule } from 'primeng/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatDividerModule } from '@angular/material/divider';
import { SelectModule } from 'primeng/select';
import { Avisos } from '../../shared/ui/avisos.service';
import { Confirmar } from '../../shared/ui/confirmar.service';
import { EvolucionService, Evolucion, EvolucionCreate } from '../../services/evolucion.service';

type Vista = 'lista' | 'form';

@Component({
  selector: 'app-evoluciones',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    ButtonModule, InputTextModule, TextareaModule, TableModule,
    EtiquetaComponent, MatTooltipModule, MatDividerModule,
    SelectModule,
  ],
  template: `
    <div class="page-card">

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA LISTA                               -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'lista') {
        <div class="list-header">
          <h2 class="page-title">Evoluciones Médicas</h2>
          <p-button label="Nueva evolución" icon="pi pi-plus"
            (onClick)="abrirNueva()" />
        </div>

        <!-- Filtros -->
        <div class="filtros-bar">
          <div style="display:flex;gap:0.5rem;align-items:flex-end;flex:1">
            <div style="flex:1">
              <input pInputText [(ngModel)]="filtroPaciente" class="w-full"
                placeholder="Buscar paciente por nombre o ID..."
                (keyup.enter)="cargar()" />
            </div>
            <p-button icon="pi pi-search" [outlined]="true"
              [loading]="cargando()" (onClick)="cargar()" matTooltip="Buscar" />
            <p-button icon="pi pi-refresh" [text]="true"
              (onClick)="limpiarFiltros()" matTooltip="Limpiar filtros" />
          </div>
          <div style="display:flex;gap:0.5rem">
            <input pInputText [(ngModel)]="filtroDesde" type="date"
              style="width:145px" (change)="cargar()" />
            <input pInputText [(ngModel)]="filtroHasta" type="date"
              style="width:145px" (change)="cargar()" />
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
          <p-table [value]="evoluciones()" [rowHover]="true" styleClass="p-datatable-sm">
            <ng-template pTemplate="header">
              <tr>
                <th>Paciente</th>
                <th>Médico</th>
                <th>Fecha</th>
                <th>Motivo (subjetivo)</th>
                <th style="text-align:center">Signos vitales</th>
                <th style="text-align:center">Acciones</th>
              </tr>
            </ng-template>
            <ng-template pTemplate="body" let-e>
              <tr>
                <td>
                  <strong>{{ e.paciente_nombre || '—' }}</strong>
                  @if (e.paciente_identificacion) {
                    <div style="font-size:0.78rem;color:#94a3b8">{{ e.paciente_identificacion }}</div>
                  }
                </td>
                <td style="font-size:0.85rem">{{ e.medico_nombre || '—' }}</td>
                <td style="font-size:0.85rem;white-space:nowrap">
                  {{ e.fecha | date:'dd/MM/yyyy HH:mm' }}
                </td>
                <td>
                  <span class="texto-recortado">{{ e.subjetivo }}</span>
                </td>
                <td style="text-align:center">
                  @if (e.tension_arterial || e.frecuencia_cardiaca || e.temperatura || e.peso) {
                    <i class="pi pi-heart" style="color:#f472b6" matTooltip="Signos vitales registrados"></i>
                  } @else {
                    <span style="color:#475569;font-size:0.8rem">—</span>
                  }
                </td>
                <td style="text-align:center">
                  <p-button icon="pi pi-file-edit" [text]="true" size="small"
                    matTooltip="Ver / editar" (onClick)="abrirEditar(e)" />
                  <p-button icon="pi pi-trash" [text]="true" size="small" severity="danger"
                    matTooltip="Eliminar" (onClick)="confirmarEliminar(e)" />
                </td>
              </tr>
            </ng-template>
          </p-table>
        }
      }

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA FORMULARIO SOAP                     -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'form') {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <p-button icon="pi pi-arrow-left" [text]="true" (onClick)="volverLista()" matTooltip="Volver" />
          <h2 class="page-title" style="margin:0">
            {{ editando() ? 'Editar evolución' : 'Nueva evolución médica' }}
          </h2>
        </div>

        <div class="form-card">

          <!-- Búsqueda de paciente (solo en nueva) -->
          @if (!editando()) {
            <div class="form-section">
              <div class="section-label">Paciente</div>
              <div style="display:flex;gap:0.5rem;align-items:flex-end">
                <div class="field" style="flex:1">
                  <input pInputText [(ngModel)]="busquedaPaciente" class="w-full"
                    placeholder="Nombre o número de identificación..."
                    (keyup.enter)="buscarPaciente()" />
                </div>
                <p-button label="Buscar" icon="pi pi-search"
                  [loading]="buscandoPaciente()" (onClick)="buscarPaciente()" />
              </div>
              @if (resultadosPaciente().length > 0) {
                <div style="margin-top:0.75rem">
                  <p-select [(ngModel)]="pacienteSeleccionado"
                    [options]="resultadosPaciente()"
                    optionLabel="nombre_completo"
                    [filter]="true" filterBy="nombre_completo,identificacion"
                    placeholder="Seleccione el paciente..."
                    class="w-full" style="width:100%" />
                </div>
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
                <div class="field">
                  <label>Subjetivo <span class="req">*</span></label>
                  <textarea pTextarea [(ngModel)]="form.subjetivo" rows="4" class="w-full"
                    placeholder="Lo que refiere el paciente: síntomas, quejas, dolor (escala 1-10), evolución desde última consulta..."></textarea>
                </div>
              </div>
              <div class="soap-field">
                <div class="soap-letter soap-o">O</div>
                <div class="field">
                  <label>Objetivo <span class="req">*</span></label>
                  <textarea pTextarea [(ngModel)]="form.objetivo" rows="4" class="w-full"
                    placeholder="Hallazgos del examen físico: aspecto general, signos, auscultación, palpación..."></textarea>
                </div>
              </div>
              <div class="soap-field">
                <div class="soap-letter soap-a">A</div>
                <div class="field">
                  <label>Análisis / Evaluación <span class="req">*</span></label>
                  <textarea pTextarea [(ngModel)]="form.analisis" rows="4" class="w-full"
                    placeholder="Interpretación clínica, diagnósticos diferenciales, evolución de la condición..."></textarea>
                </div>
              </div>
              <div class="soap-field">
                <div class="soap-letter soap-p">P</div>
                <div class="field">
                  <label>Plan <span class="req">*</span></label>
                  <textarea pTextarea [(ngModel)]="form.plan" rows="4" class="w-full"
                    placeholder="Conducta: cambios en medicación, nuevas órdenes, interconsultas, próxima cita..."></textarea>
                </div>
              </div>
            </div>
          </div>

          <mat-divider />

          <!-- Signos vitales -->
          <div class="form-section">
            <div class="section-label">Signos vitales de control <span style="color:#64748b;font-weight:400">(opcional)</span></div>
            <div class="vitales-grid">
              <div class="field">
                <label>Tensión arterial</label>
                <input pInputText [(ngModel)]="form.tension_arterial" class="w-full"
                  placeholder="120/80 mmHg" />
              </div>
              <div class="field">
                <label>Frecuencia cardíaca</label>
                <input pInputText [(ngModel)]="form.frecuencia_cardiaca" type="number"
                  class="w-full" placeholder="lpm" />
              </div>
              <div class="field">
                <label>Temperatura (°C)</label>
                <input pInputText [(ngModel)]="form.temperatura" type="number"
                  step="0.1" class="w-full" placeholder="36.5" />
              </div>
              <div class="field">
                <label>Peso (kg)</label>
                <input pInputText [(ngModel)]="form.peso" type="number"
                  step="0.1" class="w-full" placeholder="70.0" />
              </div>
            </div>
          </div>

          <div class="step-actions">
            <p-button label="Cancelar" [text]="true" (onClick)="volverLista()" />
            <p-button [label]="editando() ? 'Guardar cambios' : 'Registrar evolución'"
              icon="pi pi-check" [loading]="guardando()"
              [disabled]="!form.subjetivo || !form.objetivo || !form.analisis || !form.plan || (!editando() && !pacienteSeleccionado)"
              (onClick)="guardar()" />
          </div>
        </div>
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

    .filtros-bar { display: flex; gap: 0.75rem; align-items: flex-end; margin-bottom: 1rem; flex-wrap: wrap; }

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

  guardar(): void {
    if (!this.form.subjetivo || !this.form.objetivo || !this.form.analisis || !this.form.plan) return;
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
