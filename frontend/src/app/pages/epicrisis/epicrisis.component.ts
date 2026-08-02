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
import { EpicrisisService, Epicrisis, EpicrisisCreate } from '../../services/epicrisis.service';

type Vista = 'lista' | 'form' | 'detalle';

@Component({
  selector: 'app-epicrisis',
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
      <!--  LISTA                                     -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'lista') {
        <div class="list-header">
          <h2 class="page-title">Epicrisis</h2>
          <p-button label="Nueva epicrisis" icon="pi pi-plus" (onClick)="abrirNueva()" />
        </div>

        @if (cargando()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (lista().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-file" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay epicrisis registradas.</p>
          </div>
        } @else {
          <p-table [value]="lista()" [rowHover]="true" styleClass="p-datatable-sm">
            <ng-template pTemplate="header">
              <tr>
                <th>Paciente</th>
                <th>Médico tratante</th>
                <th>Ingreso</th>
                <th>Egreso</th>
                <th style="text-align:center">Días</th>
                <th style="text-align:center">Pronóstico</th>
                <th style="text-align:center">Incapacidad</th>
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
                <td style="font-size:0.85rem">{{ e.fecha_ingreso | date:'dd/MM/yyyy' }}</td>
                <td style="font-size:0.85rem">{{ e.fecha_egreso | date:'dd/MM/yyyy' }}</td>
                <td style="text-align:center">
                  <span class="dias-badge">{{ e.dias_estancia }}</span>
                </td>
                <td style="text-align:center">
                  <x-etiqueta [value]="e.pronostico" [severity]="pronosticoSeverity(e.pronostico)" />
                </td>
                <td style="text-align:center;font-size:0.85rem">
                  @if (e.incapacidad_dias > 0) {
                    <span style="color:#fbbf24">{{ e.incapacidad_dias }} días</span>
                  } @else {
                    <span style="color:#475569">—</span>
                  }
                </td>
                <td style="text-align:center">
                  <p-button icon="pi pi-eye" [text]="true" size="small"
                    matTooltip="Ver detalle" (onClick)="abrirDetalle(e)" />
                  <p-button icon="pi pi-file-edit" [text]="true" size="small"
                    matTooltip="Editar" (onClick)="abrirEditar(e)" />
                  <p-button icon="pi pi-trash" [text]="true" size="small" severity="danger"
                    matTooltip="Eliminar" (onClick)="confirmarEliminar(e)" />
                </td>
              </tr>
            </ng-template>
          </p-table>
        }
      }

      <!-- ═══════════════════════════════════════════ -->
      <!--  FORMULARIO                                -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'form') {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <p-button icon="pi pi-arrow-left" [text]="true" (onClick)="volverLista()" />
          <h2 class="page-title" style="margin:0">
            {{ editando() ? 'Editar epicrisis' : 'Nueva epicrisis' }}
          </h2>
        </div>

        <div class="form-card">

          <!-- Paciente (solo en nueva) -->
          @if (!editando()) {
            <div class="form-section">
              <div class="section-label">Paciente</div>
              <div style="display:flex;gap:0.5rem;align-items:flex-end">
                <div class="field" style="flex:1">
                  <input pInputText [(ngModel)]="busquedaPaciente" class="w-full"
                    placeholder="Nombre o identificación..."
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
              <div class="field">
                <label>Fecha de ingreso *</label>
                <input pInputText [(ngModel)]="f.fecha_ingreso" type="datetime-local"
                  class="w-full" (change)="calcularDias()" />
              </div>
              <div class="field">
                <label>Fecha de egreso *</label>
                <input pInputText [(ngModel)]="f.fecha_egreso" type="datetime-local"
                  class="w-full" (change)="calcularDias()" />
              </div>
              <div class="field">
                <label>Días de estancia</label>
                <input pInputText [(ngModel)]="f.dias_estancia" type="number"
                  class="w-full" min="0" />
              </div>
            </div>
          </div>

          <mat-divider />

          <!-- Diagnósticos -->
          <div class="form-section">
            <div class="section-label">Diagnósticos</div>
            <div class="grid-3">
              <div class="field">
                <label>Diagnóstico de ingreso (CIE-10) *</label>
                <input pInputText [(ngModel)]="f.diagnostico_ingreso"
                  class="w-full" placeholder="Ej: J18.9" />
              </div>
              <div class="field">
                <label>Diagnóstico de egreso (CIE-10) *</label>
                <input pInputText [(ngModel)]="f.diagnostico_egreso"
                  class="w-full" placeholder="Ej: J18.0" />
              </div>
              <div class="field">
                <label>Complicaciones</label>
                <input pInputText [(ngModel)]="f.complicaciones"
                  class="w-full" placeholder="Ninguna / descripción..." />
              </div>
            </div>
          </div>

          <mat-divider />

          <!-- Resumen clínico -->
          <div class="form-section">
            <div class="section-label">Resumen clínico</div>
            <div class="form-grid">
              <div class="field">
                <label>Resumen de evolución *</label>
                <textarea pTextarea [(ngModel)]="f.resumen_evolucion" rows="5" class="w-full"
                  placeholder="Evolución del paciente durante la hospitalización, respuesta al tratamiento..."></textarea>
              </div>
              <div class="field">
                <label>Procedimientos realizados *</label>
                <textarea pTextarea [(ngModel)]="f.procedimientos_realizados" rows="5" class="w-full"
                  placeholder="Cirugías, procedimientos diagnósticos, terapias realizadas..."></textarea>
              </div>
              <div class="field" style="grid-column:1/-1">
                <label>Exámenes relevantes</label>
                <textarea pTextarea [(ngModel)]="f.examenes_relevantes" rows="3" class="w-full"
                  placeholder="Resultados de laboratorio, imágenes diagnósticas relevantes..."></textarea>
              </div>
            </div>
          </div>

          <mat-divider />

          <!-- Plan de egreso -->
          <div class="form-section">
            <div class="section-label">Plan de egreso</div>
            <div class="form-grid">
              <div class="field">
                <label>Recomendaciones de egreso *</label>
                <textarea pTextarea [(ngModel)]="f.recomendaciones_egreso" rows="4" class="w-full"
                  placeholder="Cuidados en casa, restricciones, signos de alarma..."></textarea>
              </div>
              <div class="field">
                <label>Medicamentos al egreso</label>
                <textarea pTextarea [(ngModel)]="f.medicamentos_egreso" rows="4" class="w-full"
                  placeholder="Nombre, dosis, frecuencia, duración..."></textarea>
              </div>
              <div class="field">
                <label>Controles programados</label>
                <textarea pTextarea [(ngModel)]="f.controles_programados" rows="3" class="w-full"
                  placeholder="Citas de seguimiento, especialidades, fechas sugeridas..."></textarea>
              </div>
              <div class="field">
                <div style="display:flex;gap:1rem">
                  <div class="field" style="flex:1">
                    <label>Pronóstico</label>
                    <p-select [(ngModel)]="f.pronostico" [options]="opcionesPronostico"
                      optionLabel="label" optionValue="value"
                      class="w-full" style="width:100%" />
                  </div>
                  <div class="field" style="flex:1">
                    <label>Incapacidad (días)</label>
                    <input pInputText [(ngModel)]="f.incapacidad_dias" type="number"
                      class="w-full" min="0" placeholder="0" />
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div class="step-actions">
            <p-button label="Cancelar" [text]="true" (onClick)="volverLista()" />
            <p-button [label]="editando() ? 'Guardar cambios' : 'Registrar epicrisis'"
              icon="pi pi-check" [loading]="guardando()"
              [disabled]="camposRequeridos()"
              (onClick)="guardar()" />
          </div>
        </div>
      }

      <!-- ═══════════════════════════════════════════ -->
      <!--  DETALLE (SOLO LECTURA)                    -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'detalle' && actual()) {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <p-button icon="pi pi-arrow-left" [text]="true" (onClick)="volverLista()" />
          <h2 class="page-title" style="margin:0">Epicrisis — Resumen de egreso</h2>
          <div style="margin-left:auto">
            <p-button icon="pi pi-file-edit" [text]="true" label="Editar"
              (onClick)="abrirEditar(actual()!)" />
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
