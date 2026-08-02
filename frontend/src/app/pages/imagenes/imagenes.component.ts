import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatTableModule } from '@angular/material/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { MatDividerModule } from '@angular/material/divider';
import { MatTooltipModule } from '@angular/material/tooltip';
import { Avisos } from '../../shared/ui/avisos.service';
import { Confirmar } from '../../shared/ui/confirmar.service';
import { ImagenesService, OrdenImagen, InformeImagen, InformeCreate } from '../../services/imagenes.service';

type Vista = 'lista' | 'informe';

@Component({
  selector: 'app-imagenes',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatFormFieldModule, MatInputModule, MatTableModule,
    EtiquetaComponent, MatDividerModule,
    MatSelectModule, MatTooltipModule,
  ],
  template: `
    <div class="page-card">

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA LISTA                               -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'lista') {
        <div class="list-header">
          <h2 class="page-title">Imágenes Diagnósticas</h2>
          <mat-form-field appearance="outline" subscriptSizing="dynamic" class="filtro">
            <mat-label>Estado</mat-label>
            <mat-select [(ngModel)]="filtroEstado" (ngModelChange)="cargarOrdenes()">
              @for (o of opcionesEstado; track o.value) {
                <mat-option [value]="o.value">{{ o.label }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
        </div>

        @if (cargando()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando órdenes...</div>
        } @else if (ordenes().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-image" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay órdenes de imagenología con el filtro seleccionado.</p>
          </div>
        } @else {
          <table mat-table [dataSource]="ordenes()">
            <ng-container matColumnDef="paciente">
              <th mat-header-cell *matHeaderCellDef>Paciente</th>
              <td mat-cell *matCellDef="let o">
                <strong>{{ o.paciente_nombre || '—' }}</strong>
                @if (o.paciente_identificacion) {
                  <div class="sub">{{ o.paciente_identificacion }}</div>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="estudio">
              <th mat-header-cell *matHeaderCellDef>Estudio solicitado</th>
              <td mat-cell *matCellDef="let o">
                <span style="font-weight:500">{{ o.descripcion }}</span>
                @if (o.codigo_cups) {
                  <div class="sub">CUPS: {{ o.codigo_cups }}</div>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="medico">
              <th mat-header-cell *matHeaderCellDef>Médico solicitante</th>
              <td mat-cell *matCellDef="let o" class="menudo">{{ o.medico_nombre || '—' }}</td>
            </ng-container>
            <ng-container matColumnDef="fecha">
              <th mat-header-cell *matHeaderCellDef>Fecha</th>
              <td mat-cell *matCellDef="let o" class="menudo">
                {{ o.fecha_orden | date:'dd/MM/yyyy' }}
              </td>
            </ng-container>
            <ng-container matColumnDef="estado">
              <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
              <td mat-cell *matCellDef="let o" class="centro">
                <x-etiqueta [value]="o.estado" [severity]="estadoSeverity(o.estado)" />
              </td>
            </ng-container>
            <ng-container matColumnDef="informe">
              <th mat-header-cell *matHeaderCellDef class="centro">Informe</th>
              <td mat-cell *matCellDef="let o" class="centro">
                @if (o.tiene_resultado) {
                  <x-etiqueta value="Emitido" severity="success" icon="pi pi-file-check" />
                } @else {
                  <x-etiqueta value="Pendiente" severity="warn" icon="pi pi-clock" />
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="accion">
              <th mat-header-cell *matHeaderCellDef class="centro">Acción</th>
              <td mat-cell *matCellDef="let o" class="centro">
                <button matIconButton (click)="abrirOrden(o)"
                  [matTooltip]="o.tiene_resultado ? 'Ver / editar informe' : 'Crear informe'"
                  [attr.aria-label]="o.tiene_resultado ? 'Ver o editar informe' : 'Crear informe'">
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
      <!--  VISTA INFORME                             -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'informe' && ordenActual()) {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <button matIconButton (click)="volverLista()" matTooltip="Volver" aria-label="Volver">
            <i class="pi pi-arrow-left"></i>
          </button>
          <h2 class="page-title" style="margin:0">Informe Radiológico</h2>
        </div>

        <!-- Info header -->
        <div class="info-header">
          <div class="info-block">
            <div class="info-label">Paciente</div>
            <div class="info-name">{{ ordenActual()!.paciente_nombre || '—' }}</div>
            <div class="info-row">{{ ordenActual()!.paciente_identificacion }}</div>
          </div>
          <div class="info-block" style="border-left:1px solid rgba(255,255,255,0.1);padding-left:1.25rem">
            <div class="info-label">Estudio solicitado</div>
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
                  <i [class]="cambiandoEstado() ? 'pi pi-spin pi-spinner' : 'pi pi-play'"></i> Iniciar
                </button>
              </div>
            }
          </div>
        </div>

        <!-- Justificación -->
        <div class="justificacion-box">
          <span class="info-label">Indicación clínica: </span>
          {{ ordenActual()!.justificacion }}
        </div>

        <!-- Formulario del informe -->
        <div class="informe-form">
          <mat-form-field appearance="outline" subscriptSizing="dynamic" class="w-full">
            <mat-label>Técnica utilizada *</mat-label>
            <input matInput [(ngModel)]="iForm.tecnica"
              placeholder="Ej: Proyecciones PA y lateral de tórax con equipo digital…"
              [disabled]="informeFirmado()" />
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic" class="w-full separa">
            <mat-label>Hallazgos *</mat-label>
            <textarea matInput [(ngModel)]="iForm.hallazgos" rows="6"
              placeholder="Descripción detallada de los hallazgos observados en el estudio…"
              [disabled]="informeFirmado()"></textarea>
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic" class="w-full separa">
            <mat-label>Impresión diagnóstica *</mat-label>
            <textarea matInput [(ngModel)]="iForm.impresion_diagnostica" rows="4"
              placeholder="Conclusión e impresión diagnóstica del estudio…"
              [disabled]="informeFirmado()"></textarea>
          </mat-form-field>

          <div class="form-grid separa">
            <mat-form-field appearance="outline" subscriptSizing="dynamic">
              <mat-label>Recomendaciones</mat-label>
              <textarea matInput [(ngModel)]="iForm.recomendaciones" rows="3"
                placeholder="Estudios complementarios sugeridos, seguimiento…"
                [disabled]="informeFirmado()"></textarea>
            </mat-form-field>
            <mat-form-field appearance="outline" subscriptSizing="dynamic">
              <mat-label>Clasificación (BIRADS, ACR, Lung-RADS…)</mat-label>
              <input matInput [(ngModel)]="iForm.clasificacion"
                placeholder="Ej: BIRADS 3, ACR C, Lung-RADS 2…"
                [disabled]="informeFirmado()" />
            </mat-form-field>
          </div>

          <!-- Archivos adjuntos -->
          @if (informeActual()) {
            <div style="margin-top:1.25rem">
              <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:0.75rem">
                <strong style="color:#e2e8f0;font-size:0.875rem">Archivos adjuntos</strong>
                @if (!informeFirmado()) {
                  <div>
                    <input #fileInput type="file" style="display:none"
                      accept="image/*,.pdf,.dcm"
                      (change)="onFileSelected($event)" />
                    <button matButton [disabled]="uploadingFile()" (click)="fileInput.click()">
                      <i [class]="uploadingFile() ? 'pi pi-spin pi-spinner' : 'pi pi-upload'"></i>
                      Adjuntar imagen
                    </button>
                  </div>
                }
              </div>
              @if (informeActual()!.imagenes_paths.length > 0) {
                <div class="archivos-grid">
                  @for (path of informeActual()!.imagenes_paths; track path) {
                    <a [href]="path" target="_blank" class="archivo-chip">
                      <i class="pi pi-file"></i>
                      {{ path.split('/').pop() }}
                    </a>
                  }
                </div>
              } @else {
                <div style="font-size:0.85rem;color:#64748b">Sin archivos adjuntos.</div>
              }
            </div>
          }
        </div>

        <!-- Acciones -->
        <div class="step-actions">
          @if (!informeFirmado()) {
            <button matButton="filled" (click)="guardarInforme()"
              [disabled]="guardando() || !completo()">
              <i [class]="guardando() ? 'pi pi-spin pi-spinner' : 'pi pi-save'"></i>
              Guardar informe
            </button>
            @if (informeActual()) {
              <button matButton="filled" class="exito" (click)="confirmarFirmar()"
                [disabled]="firmando() || !completo()">
                <i [class]="firmando() ? 'pi pi-spin pi-spinner' : 'pi pi-check-circle'"></i>
                Firmar y completar
              </button>
            }
          } @else {
            <div style="display:flex;align-items:center;gap:0.75rem;color:#4ade80">
              <i class="pi pi-check-circle"></i>
              <span>Informe firmado — solo lectura</span>
            </div>
            <button matButton (click)="volverLista()">
              <i class="pi pi-arrow-left"></i> Volver
            </button>
          }
        </div>
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

    .informe-form {
      background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.07);
      border-radius: 8px; padding: 1.25rem; margin-bottom: 1rem;
    }
    .form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
    .w-full { width: 100%; }
    .separa { margin-top: 1rem; }
    table { width: 100%; }
    .filtro { width: 190px; }
    .centro { text-align: center; }
    .menudo { font-size: 0.85rem; }
    .sub { font-size: 0.78rem; color: #94a3b8; }

    /* El tema es M3: el input color de MatButton no pinta nada ahí. */
    .exito:not(:disabled) { background: #16a34a; color: #fff; }

    .archivos-grid { display: flex; flex-wrap: wrap; gap: 0.5rem; }
    .archivo-chip {
      display: flex; align-items: center; gap: 0.4rem;
      font-size: 0.8rem; padding: 0.3rem 0.75rem; border-radius: 999px;
      background: rgba(99,102,241,0.15); color: #a5b4fc;
      border: 1px solid rgba(99,102,241,0.25); text-decoration: none;
      transition: background 0.15s;
    }
    .archivo-chip:hover { background: rgba(99,102,241,0.25); }

    .step-actions { display: flex; gap: 0.5rem; justify-content: flex-end; padding-top: 1rem; border-top: 1px solid rgba(255,255,255,0.1); margin-top: 1rem; }

    ::ng-deep textarea { resize: vertical; }
  `]
})
export class ImagenesComponent implements OnInit {
  private svc = inject(ImagenesService);
  private messageService = inject(Avisos);
  private confirmationService = inject(Confirmar);

  readonly columnas = [
    'paciente', 'estudio', 'medico', 'fecha', 'estado', 'informe', 'accion',
  ];

  vista = signal<Vista>('lista');
  cargando = signal(false);
  ordenes = signal<OrdenImagen[]>([]);
  filtroEstado = '';

  readonly opcionesEstado = [
    { label: 'Todos', value: '' },
    { label: 'Pendiente', value: 'Pendiente' },
    { label: 'En Proceso', value: 'En Proceso' },
    { label: 'Completada', value: 'Completada' },
    { label: 'Cancelada', value: 'Cancelada' },
  ];

  ordenActual = signal<OrdenImagen | null>(null);
  informeActual = signal<InformeImagen | null>(null);
  iForm: Partial<InformeCreate> = { tecnica: '', hallazgos: '', impresion_diagnostica: '' };
  guardando = signal(false);
  firmando = signal(false);
  cambiandoEstado = signal(false);
  uploadingFile = signal(false);

  informeFirmado = () => !!this.informeActual()?.firmado;

  /**
   * Los tres campos que el informe no puede no tener. Estaba escrito tres veces
   * en la plantilla y una cuarta en `guardarInforme()`; con Material había que
   * escribirlo otra vez, así que pasa a ser uno solo.
   */
  completo = () =>
    !!this.iForm.tecnica && !!this.iForm.hallazgos && !!this.iForm.impresion_diagnostica;

  ngOnInit(): void {
    this.cargarOrdenes();
  }

  cargarOrdenes(): void {
    this.cargando.set(true);
    this.svc.listOrdenes(this.filtroEstado || undefined).subscribe({
      next: os => { this.ordenes.set(os); this.cargando.set(false); },
      error: () => { this.cargando.set(false); this.err('Error al cargar las órdenes'); },
    });
  }

  abrirOrden(o: OrdenImagen): void {
    this.ordenActual.set(o);
    this.informeActual.set(null);
    this.iForm = { tecnica: '', hallazgos: '', impresion_diagnostica: '' };
    if (o.tiene_resultado) {
      this.svc.getInforme(o.id).subscribe({
        next: inf => {
          this.informeActual.set(inf);
          this.iForm = {
            tecnica: inf.tecnica,
            hallazgos: inf.hallazgos,
            impresion_diagnostica: inf.impresion_diagnostica,
            recomendaciones: inf.recomendaciones,
            clasificacion: inf.clasificacion,
          };
        },
        error: () => this.err('Error al cargar el informe'),
      });
    }
    this.vista.set('informe');
  }

  volverLista(): void {
    this.ordenActual.set(null);
    this.informeActual.set(null);
    this.cargarOrdenes();
    this.vista.set('lista');
  }

  iniciarProceso(): void {
    const o = this.ordenActual();
    if (!o) return;
    this.cambiandoEstado.set(true);
    this.svc.updateEstado(o.id, 'En Proceso').subscribe({
      next: updated => { this.ordenActual.set(updated); this.cambiandoEstado.set(false); },
      error: () => { this.cambiandoEstado.set(false); this.err('Error al actualizar estado'); },
    });
  }

  guardarInforme(): void {
    const o = this.ordenActual();
    if (!o || !this.completo()) return;
    const payload = this.iForm as InformeCreate;
    this.guardando.set(true);
    const obs$ = this.informeActual()
      ? this.svc.updateInforme(o.id, payload)
      : this.svc.createInforme(o.id, payload);
    obs$.subscribe({
      next: inf => {
        this.informeActual.set(inf);
        this.guardando.set(false);
        this.svc.getOrden(o.id).subscribe(updated => this.ordenActual.set(updated));
        this.ok('Guardado', 'Informe guardado correctamente');
      },
      error: err => { this.guardando.set(false); this.err(err?.error?.detail ?? 'Error al guardar'); },
    });
  }

  confirmarFirmar(): void {
    this.confirmationService.confirm({
      message: 'Una vez firmado el informe no podrá ser modificado. ¿Confirma?',
      header: 'Firmar informe',
      icon: 'pi pi-lock',
      acceptLabel: 'Firmar y completar',
      rejectLabel: 'Cancelar',
      accept: () => this._firmar(),
    });
  }

  private _firmar(): void {
    const o = this.ordenActual();
    if (!o) return;
    // Save then sign
    const payload = this.iForm as InformeCreate;
    this.firmando.set(true);
    const save$ = this.informeActual()
      ? this.svc.updateInforme(o.id, payload)
      : this.svc.createInforme(o.id, payload);
    save$.subscribe({
      next: () => {
        this.svc.firmarInforme(o.id).subscribe({
          next: inf => {
            this.informeActual.set(inf);
            this.firmando.set(false);
            this.svc.getOrden(o.id).subscribe(updated => this.ordenActual.set(updated));
            this.ok('Informe firmado', 'Orden marcada como Completada');
          },
          error: err => { this.firmando.set(false); this.err(err?.error?.detail ?? 'Error al firmar'); },
        });
      },
      error: err => { this.firmando.set(false); this.err(err?.error?.detail ?? 'Error al guardar'); },
    });
  }

  onFileSelected(event: Event): void {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file || !this.informeActual()) return;
    this.uploadingFile.set(true);
    this.svc.uploadImagen(this.ordenActual()!.id, file).subscribe({
      next: inf => {
        this.informeActual.set(inf);
        this.uploadingFile.set(false);
        this.ok('Archivo adjuntado', file.name);
      },
      error: () => { this.uploadingFile.set(false); this.err('Error al subir el archivo'); },
    });
  }

  estadoSeverity(estado: string): 'info' | 'success' | 'warn' | 'danger' | 'secondary' {
    if (estado === 'Completada') return 'success';
    if (estado === 'Cancelada') return 'danger';
    if (estado === 'En Proceso') return 'info';
    return 'warn';
  }

  private ok(summary: string, detail?: string): void {
    this.messageService.add({ severity: 'success', summary, detail });
  }

  private err(detail: string): void {
    this.messageService.add({ severity: 'error', summary: 'Error', detail });
  }
}
