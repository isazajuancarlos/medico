import { Component, OnInit, ViewChild, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatPaginator, MatPaginatorModule } from '@angular/material/paginator';
import { MatSelectModule } from '@angular/material/select';
import { MatTableDataSource, MatTableModule } from '@angular/material/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatDividerModule } from '@angular/material/divider';
import { Avisos } from '../../shared/ui/avisos.service';
import { Confirmar } from '../../shared/ui/confirmar.service';
import { NotaOperatoriaService, NotaOperatoria, NotaOperatoriaCreate } from '../../services/nota-operatoria.service';
import { ESTADOS_SALIDA, duracion, estadoValido, formatoDuracion, revisar } from './nota-operatoria.dominio';

/**
 * Nota operatoria (protocolo quirúrgico). Es un DOCUMENTO CLÍNICO de tipo
 * NOTA_OPERATORIA sobre la espina /api/documentos, hermano de epicrisis; sus
 * campos viven en `contenido`. Las reglas —duración derivada de las horas,
 * estado de salida obligatorio, avisos que señalan sin bloquear— están en
 * `nota-operatoria.dominio`, probadas aparte del formulario.
 */

type Vista = 'lista' | 'form' | 'detalle';

@Component({
  selector: 'app-nota-operatoria',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatFormFieldModule, MatInputModule, MatSelectModule,
    MatTableModule, MatPaginatorModule,
    EtiquetaComponent, MatTooltipModule, MatDividerModule,
  ],
  template: `
    <div class="page-card">

      <!-- ═══════════════ LISTA ═══════════════ -->
      @if (vista() === 'lista') {
        <div class="list-header">
          <h2 class="page-title">Notas operatorias</h2>
          <button matButton="filled" (click)="abrirNueva()">
            <i class="pi pi-plus"></i> Nueva nota
          </button>
        </div>

        @if (cargando()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (datos.data.length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-file" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay notas operatorias.</p>
          </div>
        } @else {
          <table mat-table [dataSource]="datos" class="tabla">
            <ng-container matColumnDef="fecha">
              <th mat-header-cell *matHeaderCellDef>Fecha</th>
              <td mat-cell *matCellDef="let n" style="font-size:0.85rem">{{ n.fecha | date:'dd/MM/yyyy' }}</td>
            </ng-container>
            <ng-container matColumnDef="paciente">
              <th mat-header-cell *matHeaderCellDef>Paciente</th>
              <td mat-cell *matCellDef="let n">
                <strong>{{ n.paciente_nombre || '—' }}</strong>
                @if (n.paciente_identificacion) {
                  <div style="font-size:0.78rem;color:#94a3b8">{{ n.paciente_identificacion }}</div>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="procedimiento">
              <th mat-header-cell *matHeaderCellDef>Procedimiento</th>
              <td mat-cell *matCellDef="let n" style="font-size:0.85rem">{{ n.procedimiento_realizado }}</td>
            </ng-container>
            <ng-container matColumnDef="duracion">
              <th mat-header-cell *matHeaderCellDef class="centro">Duración</th>
              <td mat-cell *matCellDef="let n" class="centro" style="font-size:0.85rem">{{ duracionTexto(n) }}</td>
            </ng-container>
            <ng-container matColumnDef="salida">
              <th mat-header-cell *matHeaderCellDef class="centro">Salida</th>
              <td mat-cell *matCellDef="let n" class="centro">
                <x-etiqueta [value]="n.estado_paciente_salida || '—'"
                  [severity]="estadoSeverity(n.estado_paciente_salida)" />
              </td>
            </ng-container>
            <ng-container matColumnDef="cirujano">
              <th mat-header-cell *matHeaderCellDef>Cirujano</th>
              <td mat-cell *matCellDef="let n" style="font-size:0.85rem">{{ n.cirujano_nombre || '—' }}</td>
            </ng-container>
            <ng-container matColumnDef="revisar">
              <th mat-header-cell *matHeaderCellDef class="centro">Revisar</th>
              <td mat-cell *matCellDef="let n" class="centro">
                @if (avisosDe(n).length) {
                  <i class="pi pi-exclamation-triangle" style="color:#fbbf24"
                    [matTooltip]="avisosDe(n).join(' · ')"></i>
                } @else { <span style="color:#475569">—</span> }
              </td>
            </ng-container>
            <ng-container matColumnDef="acciones">
              <th mat-header-cell *matHeaderCellDef class="centro">Acciones</th>
              <td mat-cell *matCellDef="let n" class="centro">
                <button matIconButton matTooltip="Ver detalle" (click)="abrirDetalle(n)">
                  <i class="pi pi-eye"></i>
                </button>
                <button matIconButton matTooltip="Editar" (click)="abrirEditar(n)">
                  <i class="pi pi-file-edit"></i>
                </button>
                <button matIconButton class="peligro" matTooltip="Eliminar" (click)="confirmarEliminar(n)">
                  <i class="pi pi-trash"></i>
                </button>
              </td>
            </ng-container>

            <tr mat-header-row *matHeaderRowDef="columnas"></tr>
            <tr mat-row *matRowDef="let fila; columns: columnas"></tr>
          </table>
          <mat-paginator [pageSize]="20" [pageSizeOptions]="[10, 20, 50]" />
        }
      }

      <!-- ═══════════════ FORMULARIO ═══════════════ -->
      @if (vista() === 'form') {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <button matIconButton (click)="volverLista()"><i class="pi pi-arrow-left"></i></button>
          <h2 class="page-title" style="margin:0">
            {{ editando() ? 'Editar nota operatoria' : 'Nueva nota operatoria' }}
          </h2>
        </div>

        <div class="form-card">

          @if (!editando()) {
            <div class="form-section">
              <div class="section-label">Paciente</div>
              <div style="display:flex;gap:0.5rem;align-items:flex-start">
                <mat-form-field appearance="outline" subscriptSizing="dynamic" style="flex:1">
                  <mat-label>Paciente</mat-label>
                  <input matInput [(ngModel)]="busquedaPaciente"
                    placeholder="Nombre o identificación..." (keyup.enter)="buscarPaciente()" />
                </mat-form-field>
                <button matButton="filled" [disabled]="buscandoPaciente()" (click)="buscarPaciente()">
                  <i [class]="buscandoPaciente() ? 'pi pi-spin pi-spinner' : 'pi pi-search'"></i> Buscar
                </button>
              </div>
              @if (resultadosPaciente().length > 0) {
                <mat-form-field appearance="outline" subscriptSizing="dynamic" style="margin-top:0.75rem">
                  <mat-label>Seleccione paciente…</mat-label>
                  <mat-select [(ngModel)]="pacienteSeleccionado">
                    @for (p of resultadosPaciente(); track p.id) {
                      <mat-option [value]="p">{{ p.nombre_completo }}</mat-option>
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

          <!-- Tiempos -->
          <div class="form-section">
            <div class="section-label">Tiempo quirúrgico</div>
            <div class="grid-3">
              <mat-form-field appearance="outline">
                <mat-label>Hora de inicio *</mat-label>
                <input matInput [(ngModel)]="f.hora_inicio" type="datetime-local" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Hora de fin *</mat-label>
                <input matInput [(ngModel)]="f.hora_fin" type="datetime-local" />
              </mat-form-field>
              <div class="field">
                <label>Duración (calculada)</label>
                <div class="calculado" [class.malo]="horasInvalidas()">
                  {{ duracionFormulario() }}
                </div>
              </div>
            </div>
          </div>

          <mat-divider />

          <!-- Procedimiento y diagnósticos -->
          <div class="form-section">
            <div class="section-label">Procedimiento</div>
            <div class="form-grid">
              <mat-form-field appearance="outline" style="grid-column:1/-1">
                <mat-label>Procedimiento realizado *</mat-label>
                <input matInput [(ngModel)]="f.procedimiento_realizado"
                  placeholder="Ej: Apendicectomía laparoscópica" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Diagnóstico preoperatorio</mat-label>
                <input matInput [(ngModel)]="f.diagnostico_preoperatorio" placeholder="Ej: K35.8" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Diagnóstico postoperatorio *</mat-label>
                <input matInput [(ngModel)]="f.diagnostico_postoperatorio" placeholder="Ej: K35.3" />
              </mat-form-field>
              <mat-form-field appearance="outline" style="grid-column:1/-1">
                <mat-label>Hallazgos</mat-label>
                <textarea matInput [(ngModel)]="f.hallazgos" rows="3"
                  placeholder="Hallazgos intraoperatorios..."></textarea>
              </mat-form-field>
            </div>
          </div>

          <mat-divider />

          <!-- Salida -->
          <div class="form-section">
            <div class="section-label">Salida de quirófano</div>
            <div class="grid-3">
              <!-- Sin opción por defecto y a propósito: el estado a la salida
                   es uno de los campos que el sistema rellenaba en nombre del
                   médico. Elegirlo es la afirmación. -->
              <mat-form-field appearance="outline">
                <mat-label>Estado del paciente a la salida *</mat-label>
                <mat-select [(ngModel)]="f.estado_paciente_salida">
                  @for (o of opcionesEstado; track o.value) {
                    <mat-option [value]="o.value">{{ o.label }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Sangrado estimado (ml)</mat-label>
                <input matInput [(ngModel)]="f.sangrado_estimado_ml" type="number" min="0" placeholder="—" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Complicaciones</mat-label>
                <input matInput [(ngModel)]="f.complicaciones" placeholder="Ninguna / descripción..." />
              </mat-form-field>
            </div>
          </div>

          <!-- Avisos EN VIVO: señalan sin bloquear -->
          @if (avisosFormulario().length) {
            <div class="avisos-box">
              <div class="avisos-titulo"><i class="pi pi-info-circle"></i> Para revisar (no impide guardar)</div>
              <ul>@for (a of avisosFormulario(); track a) { <li>{{ a }}</li> }</ul>
            </div>
          }

          <div class="step-actions">
            <button matButton (click)="volverLista()">Cancelar</button>
            <button matButton="filled" [disabled]="camposRequeridos() || guardando()" (click)="guardar()">
              <i [class]="guardando() ? 'pi pi-spin pi-spinner' : 'pi pi-check'"></i>
              {{ editando() ? 'Guardar cambios' : 'Registrar nota' }}
            </button>
          </div>
        </div>
      }

      <!-- ═══════════════ DETALLE ═══════════════ -->
      @if (vista() === 'detalle' && actual()) {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <button matIconButton (click)="volverLista()"><i class="pi pi-arrow-left"></i></button>
          <h2 class="page-title" style="margin:0">Nota operatoria</h2>
          <div style="margin-left:auto;display:flex;gap:0.5rem">
            @if (!actual()!.firmada) {
              <button matButton (click)="firmar(actual()!)"><i class="pi pi-verified"></i> Firmar</button>
              <button matButton (click)="abrirEditar(actual()!)"><i class="pi pi-file-edit"></i> Editar</button>
            } @else {
              <x-etiqueta value="Firmada" severity="success" icon="pi pi-lock" />
            }
          </div>
        </div>

        <div class="info-header">
          <div class="info-block">
            <div class="info-label">Paciente</div>
            <div class="info-name">{{ actual()!.paciente_nombre }}</div>
            <div class="info-row">{{ actual()!.paciente_identificacion }}</div>
          </div>
          <div class="info-block sep">
            <div class="info-label">Cirujano</div>
            <div class="info-name" style="font-size:0.95rem">{{ actual()!.cirujano_nombre }}</div>
          </div>
          <div class="info-block sep">
            <div class="info-label">Tiempo quirúrgico</div>
            <div class="info-name" style="font-size:0.9rem">{{ duracionTexto(actual()!) }}</div>
          </div>
          <div class="info-block sep">
            <div class="info-label">Salida</div>
            <div class="info-name" style="font-size:0.9rem">{{ actual()!.estado_paciente_salida }}</div>
          </div>
        </div>

        @if (avisosDe(actual()!).length) {
          <div class="avisos-box">
            <div class="avisos-titulo"><i class="pi pi-info-circle"></i> Para revisar</div>
            <ul>@for (a of avisosDe(actual()!); track a) { <li>{{ a }}</li> }</ul>
          </div>
        }

        <div class="detalle-grid">
          <div class="detalle-bloque">
            <div class="section-label">Procedimiento realizado</div>
            <div class="texto-bloque">{{ actual()!.procedimiento_realizado }}</div>
          </div>
          <div class="detalle-bloque">
            <div class="section-label">Diagnósticos</div>
            <div class="texto-bloque">Pre: {{ actual()!.diagnostico_preoperatorio || '—' }}
Post: {{ actual()!.diagnostico_postoperatorio || '—' }}</div>
          </div>
          <div class="detalle-bloque" style="grid-column:1/-1">
            <div class="section-label">Hallazgos</div>
            <div class="texto-bloque">{{ actual()!.hallazgos || '—' }}</div>
          </div>
          <div class="detalle-bloque">
            <div class="section-label">Sangrado estimado</div>
            <div class="texto-bloque">{{ actual()!.sangrado_estimado_ml != null ? actual()!.sangrado_estimado_ml + ' ml' : 'No registrado' }}</div>
          </div>
          <div class="detalle-bloque">
            <div class="section-label">Complicaciones</div>
            <div class="texto-bloque">{{ actual()!.complicaciones || 'Ninguna' }}</div>
          </div>
        </div>
      }
    </div>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.25rem; }
    .list-header .page-title { margin-bottom: 0; }
    .cargando { padding: 3rem; text-align: center; color: #64748b; display: flex; align-items: center; justify-content: center; gap: 0.5rem; }
    .sin-resultados { text-align: center; padding: 3rem 1rem; color: #64748b; display: flex; flex-direction: column; align-items: center; gap: 0.75rem; }

    .form-card { background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.07); border-radius: 10px; padding: 1.5rem; }
    .form-section { margin-bottom: 0.25rem; }
    .section-label { font-size: 0.73rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em; color: #818cf8; margin-bottom: 0.75rem; }
    .form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
    .grid-3 { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 1rem; }
    .field { display: flex; flex-direction: column; gap: 0.35rem; }
    .field label { font-weight: 500; font-size: 0.875rem; color: #e2e8f0; }
    .w-full { width: 100%; }
    .calculado { padding: 0.55rem 0.75rem; border-radius: 6px; background: rgba(255,255,255,0.04); color: #e2e8f0; font-weight: 600; }
    .calculado.malo { color: #fca5a5; background: rgba(239,68,68,0.1); }

    .paciente-chip { display: flex; align-items: center; gap: 0.75rem; background: rgba(99,102,241,0.1); border: 1px solid rgba(99,102,241,0.25); border-radius: 8px; padding: 0.6rem 1rem; margin-top: 0.5rem; }

    .avisos-box { margin-top: 1rem; background: rgba(251,191,36,0.08); border: 1px solid rgba(251,191,36,0.3); border-radius: 8px; padding: 0.75rem 1rem; }
    .avisos-titulo { font-size: 0.8rem; font-weight: 600; color: #fbbf24; margin-bottom: 0.35rem; }
    .avisos-box ul { margin: 0; padding-left: 1.1rem; font-size: 0.82rem; color: #fcd34d; }

    .info-header { display: flex; gap: 1.5rem; flex-wrap: wrap; background: #0f172a; border: 1px solid rgba(255,255,255,0.1); border-radius: 8px; padding: 1rem 1.25rem; margin-bottom: 1.25rem; }
    .info-block { display: flex; flex-direction: column; gap: 0.2rem; }
    .info-block.sep { border-left: 1px solid rgba(255,255,255,0.1); padding-left: 1.25rem; }
    .info-label { font-size: 0.7rem; font-weight: 600; text-transform: uppercase; color: #94a3b8; letter-spacing: 0.05em; }
    .info-name { font-size: 1rem; font-weight: 600; color: #f1f5f9; }
    .info-row { font-size: 0.82rem; color: #cbd5e1; }

    .detalle-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; margin-bottom: 1.25rem; }
    .texto-bloque { background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.07); border-radius: 6px; padding: 0.875rem 1rem; font-size: 0.875rem; color: #cbd5e1; line-height: 1.6; white-space: pre-wrap; }

    .step-actions { display: flex; gap: 0.5rem; justify-content: flex-end; border-top: 1px solid rgba(255,255,255,0.1); margin-top: 1.25rem; padding-top: 1rem; }
    ::ng-deep textarea { resize: vertical; }

    .tabla { width: 100%; }
    .centro { text-align: center; }
    mat-form-field { width: 100%; }
    /* El input color de un botón no pinta nada en M3: lo destructivo va por
       CSS. Ver MIGRACION_MATERIAL.md. */
    .peligro { color: #dc2626; }
  `],
})
export class NotaOperatoriaComponent implements OnInit {
  private svc = inject(NotaOperatoriaService);
  private messageService = inject(Avisos);
  private confirmationService = inject(Confirmar);

  vista = signal<Vista>('lista');
  cargando = signal(false);

  /** El orden de las columnas lo fija esta lista, no la plantilla. */
  readonly columnas = ['fecha', 'paciente', 'procedimiento', 'duracion', 'salida',
                       'cirujano', 'revisar', 'acciones'];
  readonly datos = new MatTableDataSource<NotaOperatoria>([]);

  /** Por setter: la tabla vive dentro de un @if, así que el paginador aparece
   *  y desaparece. Ver MIGRACION_MATERIAL.md. */
  @ViewChild(MatPaginator)
  set paginador(p: MatPaginator | undefined) { if (p) this.datos.paginator = p; }

  actual = signal<NotaOperatoria | null>(null);
  editando = signal(false);
  guardando = signal(false);

  busquedaPaciente = '';
  buscandoPaciente = signal(false);
  resultadosPaciente = signal<any[]>([]);
  pacienteSeleccionado: any = null;

  readonly opcionesEstado = ESTADOS_SALIDA.map(e => ({ label: this.capitaliza(e), value: e }));

  // Objeto plano para `[(ngModel)]` (como epicrisis); duración y avisos se
  // calculan como métodos, reevaluados por la detección de cambios en cada tecleo.
  f: Partial<NotaOperatoriaCreate> = { estado_paciente_salida: '', sangrado_estimado_ml: null };

  duracionFormulario(): string {
    const { hora_inicio, hora_fin } = this.f;
    if (!hora_inicio || !hora_fin) return '—';
    const d = duracion(hora_inicio, hora_fin);
    return d ? formatoDuracion(d.minutos) : 'Las horas no describen una cirugía válida';
  }

  horasInvalidas(): boolean {
    const { hora_inicio, hora_fin } = this.f;
    return !!hora_inicio && !!hora_fin && duracion(hora_inicio, hora_fin) === null;
  }

  avisosFormulario(): string[] {
    const v = this.f;
    if (!v.hora_inicio || !v.hora_fin) return [];
    return revisar({
      hora_inicio: v.hora_inicio,
      hora_fin: v.hora_fin,
      sangrado_estimado_ml: v.sangrado_estimado_ml ?? null,
      complicaciones: v.complicaciones,
      estado_paciente_salida: v.estado_paciente_salida,
    });
  }

  camposRequeridos = (): boolean => {
    const v = this.f;
    const ok = v.procedimiento_realizado && v.diagnostico_postoperatorio &&
      v.hora_inicio && v.hora_fin && estadoValido(v.estado_paciente_salida) &&
      duracion(v.hora_inicio, v.hora_fin) !== null;
    if (!this.editando() && !this.pacienteSeleccionado) return true;
    return !ok;
  };

  ngOnInit(): void { this.cargar(); }

  cargar(): void {
    this.cargando.set(true);
    this.svc.list().subscribe({
      next: ns => { this.datos.data = ns; this.cargando.set(false); },
      error: () => { this.cargando.set(false); this.err('Error al cargar las notas operatorias'); },
    });
  }

  abrirNueva(): void {
    this.editando.set(false);
    this.actual.set(null);
    this.busquedaPaciente = '';
    this.resultadosPaciente.set([]);
    this.pacienteSeleccionado = null;
    this.f = { estado_paciente_salida: '', sangrado_estimado_ml: null };
    this.vista.set('form');
  }

  abrirEditar(n: NotaOperatoria): void {
    this.editando.set(true);
    this.actual.set(n);
    const toLocal = (iso?: string) => (iso ? iso.substring(0, 16) : '');
    this.f = {
      procedimiento_realizado: n.procedimiento_realizado,
      diagnostico_preoperatorio: n.diagnostico_preoperatorio,
      diagnostico_postoperatorio: n.diagnostico_postoperatorio,
      hallazgos: n.hallazgos,
      hora_inicio: toLocal(n.hora_inicio),
      hora_fin: toLocal(n.hora_fin),
      sangrado_estimado_ml: n.sangrado_estimado_ml,
      complicaciones: n.complicaciones,
      estado_paciente_salida: n.estado_paciente_salida,
    };
    this.vista.set('form');
  }

  abrirDetalle(n: NotaOperatoria): void {
    this.actual.set(n);
    this.vista.set('detalle');
  }

  volverLista(): void {
    this.vista.set('lista');
    this.cargar();
  }

  buscarPaciente(): void {
    if (!this.busquedaPaciente.trim()) return;
    this.buscandoPaciente.set(true);
    this.svc.searchPacientes().subscribe({
      next: ps => {
        const q = this.busquedaPaciente.trim().toLowerCase();
        const filtrados = ps.filter(p =>
          p.nombre_completo.toLowerCase().includes(q) || String(p.identificacion).includes(q));
        this.resultadosPaciente.set(filtrados);
        this.buscandoPaciente.set(false);
        if (!filtrados.length) this.err('No se encontraron pacientes');
      },
      error: () => { this.buscandoPaciente.set(false); this.err('Error al buscar'); },
    });
  }

  guardar(): void {
    this.guardando.set(true);
    const datos = this.f as NotaOperatoriaCreate;
    if (this.editando()) {
      this.svc.update(this.actual()!.id, datos).subscribe({
        next: () => { this.guardando.set(false); this.ok('Guardado', 'Nota operatoria actualizada'); this.volverLista(); },
        error: err => { this.guardando.set(false); this.err(err?.error?.detail ?? 'Error al guardar'); },
      });
    } else {
      datos.paciente_id = this.pacienteSeleccionado.id;
      this.svc.create(datos).subscribe({
        next: () => { this.guardando.set(false); this.ok('Registrada', 'Nota operatoria creada'); this.volverLista(); },
        error: err => { this.guardando.set(false); this.err(err?.error?.detail ?? 'Error al crear'); },
      });
    }
  }

  firmar(n: NotaOperatoria): void {
    this.svc.firmar(n.id).subscribe({
      next: () => { this.ok('Firmada'); this.volverLista(); },
      error: () => this.err('Error al firmar'),
    });
  }

  confirmarEliminar(n: NotaOperatoria): void {
    this.confirmationService.confirm({
      message: `¿Eliminar la nota operatoria de <strong>${n.paciente_nombre}</strong>?`,
      header: 'Confirmar eliminación',
      icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Eliminar',
      rejectLabel: 'Cancelar',
      peligroso: true,
      accept: () => {
        this.svc.delete(n.id).subscribe({
          next: () => {
            this.datos.data = this.datos.data.filter(x => x.id !== n.id);
            this.ok('Eliminada');
          },
          error: () => this.err('Error al eliminar'),
        });
      },
    });
  }

  duracionTexto(n: NotaOperatoria): string {
    const d = duracion(n.hora_inicio, n.hora_fin);
    return d ? formatoDuracion(d.minutos) : '—';
  }

  avisosDe(n: NotaOperatoria): string[] {
    return revisar({
      hora_inicio: n.hora_inicio,
      hora_fin: n.hora_fin,
      sangrado_estimado_ml: n.sangrado_estimado_ml,
      complicaciones: n.complicaciones,
      estado_paciente_salida: n.estado_paciente_salida,
      firmada: n.firmada,
    });
  }

  estadoSeverity(e: string): 'success' | 'warn' | 'danger' | 'secondary' {
    if (e === 'ESTABLE') return 'success';
    if (e === 'INESTABLE') return 'warn';
    if (e === 'CRITICO' || e === 'FALLECIDO') return 'danger';
    return 'secondary';
  }

  private capitaliza(e: string): string {
    return e.charAt(0) + e.slice(1).toLowerCase();
  }

  private ok(summary: string, detail?: string): void {
    this.messageService.add({ severity: 'success', summary, detail });
  }

  private err(detail: string): void {
    this.messageService.add({ severity: 'error', summary: 'Error', detail });
  }
}
