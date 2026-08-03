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
import {
  NutricionService, ValoracionNutricional, PlanAlimentacion,
  ValoracionCreate, PlanCreate,
} from '../../services/nutricion.service';

type Vista = 'lista' | 'valoracion' | 'detalle';

@Component({
  selector: 'app-nutricion',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatFormFieldModule, MatInputModule, MatSelectModule,
    MatTableModule, EtiquetaComponent, MatTooltipModule, MatDividerModule,
  ],
  template: `
    <div class="page-card">

      <!-- ═══════════════════════════════════════ LISTA ═ -->
      @if (vista() === 'lista') {
        <div class="list-header">
          <h2 class="page-title">Nutrición Clínica</h2>
          <div style="display:flex;gap:0.5rem;align-items:flex-start">
            <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:185px">
              <mat-label>Riesgo nutricional</mat-label>
              <mat-select [(ngModel)]="filtroRiesgo" (selectionChange)="cargar()">
                @for (o of opcionesRiesgo; track o.value) {
                  <mat-option [value]="o.value">{{ o.label }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
            <button matButton="filled" (click)="abrirNueva()">
              <i class="pi pi-plus"></i> Nueva valoración
            </button>
          </div>
        </div>

        @if (cargando()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (lista().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-chart-bar" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay valoraciones nutricionales registradas.</p>
          </div>
        } @else {
          <table mat-table [dataSource]="lista()" class="tabla">
            <ng-container matColumnDef="paciente">
              <th mat-header-cell *matHeaderCellDef>Paciente</th>
              <td mat-cell *matCellDef="let v">
                <strong>{{ v.paciente_nombre || '—' }}</strong>
                @if (v.paciente_identificacion) {
                  <div style="font-size:0.78rem;color:#94a3b8">{{ v.paciente_identificacion }}</div>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="nutricionista">
              <th mat-header-cell *matHeaderCellDef>Nutricionista</th>
              <td mat-cell *matCellDef="let v" style="font-size:0.85rem">{{ v.nutricionista_nombre || '—' }}</td>
            </ng-container>
            <ng-container matColumnDef="fecha">
              <th mat-header-cell *matHeaderCellDef>Fecha</th>
              <td mat-cell *matCellDef="let v" style="font-size:0.85rem">
                {{ v.fecha_valoracion | date:'dd/MM/yyyy' }}
              </td>
            </ng-container>
            <ng-container matColumnDef="peso">
              <th mat-header-cell *matHeaderCellDef class="centro">Peso</th>
              <td mat-cell *matCellDef="let v" class="centro" style="font-size:0.85rem">{{ v.peso }} kg</td>
            </ng-container>
            <ng-container matColumnDef="imc">
              <th mat-header-cell *matHeaderCellDef class="centro">IMC</th>
              <td mat-cell *matCellDef="let v" class="centro">
                <span [class]="'imc-badge imc-' + imcCategoria(v.imc)">{{ v.imc | number:'1.1-1' }}</span>
              </td>
            </ng-container>
            <ng-container matColumnDef="screening">
              <th mat-header-cell *matHeaderCellDef class="centro">Screening</th>
              <td mat-cell *matCellDef="let v" class="centro" style="font-size:0.82rem">
                {{ v.screening_nutricional }}
              </td>
            </ng-container>
            <ng-container matColumnDef="riesgo">
              <th mat-header-cell *matHeaderCellDef class="centro">Riesgo</th>
              <td mat-cell *matCellDef="let v" class="centro">
                <x-etiqueta [value]="v.riesgo_nutricional" [severity]="riesgoSeverity(v.riesgo_nutricional)" />
              </td>
            </ng-container>
            <ng-container matColumnDef="acciones">
              <th mat-header-cell *matHeaderCellDef class="centro">Acciones</th>
              <td mat-cell *matCellDef="let v" class="centro">
                <button matIconButton matTooltip="Ver detalle" (click)="abrirDetalle(v)">
                  <i class="pi pi-eye"></i>
                </button>
                <button matIconButton matTooltip="Editar" (click)="abrirEditar(v)">
                  <i class="pi pi-file-edit"></i>
                </button>
                <button matIconButton class="peligro" matTooltip="Eliminar" (click)="confirmarEliminar(v)">
                  <i class="pi pi-trash"></i>
                </button>
              </td>
            </ng-container>

            <tr mat-header-row *matHeaderRowDef="columnas"></tr>
            <tr mat-row *matRowDef="let fila; columns: columnas"></tr>
          </table>
        }
      }

      <!-- ═══════════════════════════════════ FORMULARIO ═ -->
      @if (vista() === 'valoracion') {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <button matIconButton (click)="volverLista()"><i class="pi pi-arrow-left"></i></button>
          <h2 class="page-title" style="margin:0">
            {{ editando() ? 'Editar valoración' : 'Nueva valoración nutricional' }}
          </h2>
        </div>

        <div class="form-card">

          <!-- Paciente -->
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

          <!-- Datos antropométricos -->
          <div class="form-section">
            <div class="section-label">Datos antropométricos</div>
            <div class="grid-4">
              <mat-form-field appearance="outline">
                <mat-label>Peso (kg) *</mat-label>
                <input matInput [(ngModel)]="f.peso" type="number" step="0.1"
                  placeholder="70.0" (input)="calcularIMC()" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Talla (m) *</mat-label>
                <input matInput [(ngModel)]="f.talla" type="number" step="0.01"
                  placeholder="1.70" (input)="calcularIMC()" />
              </mat-form-field>
              <div class="field">
                <label>IMC (calculado)</label>
                <div class="imc-display" [class]="'imc-' + imcCategoria(f.imc || 0)">
                  {{ f.imc ? (f.imc | number:'1.1-1') : '—' }}
                  @if (f.imc) {
                    <span class="imc-cat">{{ imcLabel(f.imc) }}</span>
                  }
                </div>
              </div>
              <mat-form-field appearance="outline">
                <mat-label>Circunferencia de brazo (cm)</mat-label>
                <input matInput [(ngModel)]="f.circunferencia_brazo" type="number"
                  step="0.1" placeholder="Opcional" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Pliegue tricipital (mm)</mat-label>
                <input matInput [(ngModel)]="f.pliegue_tricipital" type="number"
                  step="0.1" placeholder="Opcional" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Albúmina (g/dL)</mat-label>
                <input matInput [(ngModel)]="f.albumina" type="number" step="0.1" placeholder="Opcional" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Prealbúmina (mg/dL)</mat-label>
                <input matInput [(ngModel)]="f.prealbumina" type="number" step="0.1" placeholder="Opcional" />
              </mat-form-field>
            </div>
          </div>

          <mat-divider />

          <!-- Screening y riesgo -->
          <div class="form-section">
            <div class="section-label">Screening nutricional</div>
            <div class="grid-3">
              <mat-form-field appearance="outline">
                <mat-label>Herramienta de screening</mat-label>
                <mat-select [(ngModel)]="f.screening_nutricional">
                  @for (o of opcionesScreening; track o.value) {
                    <mat-option [value]="o.value">{{ o.label }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Puntaje obtenido</mat-label>
                <input matInput [(ngModel)]="f.puntaje_screening" type="number" placeholder="0" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Riesgo nutricional</mat-label>
                <mat-select [(ngModel)]="f.riesgo_nutricional">
                  @for (o of riesgosElegibles; track o.value) {
                    <mat-option [value]="o.value">{{ o.label }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
            </div>
          </div>

          <mat-divider />

          <!-- Diagnóstico y requerimientos -->
          <div class="form-section">
            <div class="section-label">Diagnóstico y requerimientos</div>
            <div class="form-grid">
              <mat-form-field appearance="outline" style="grid-column:1/-1">
                <mat-label>Diagnóstico nutricional *</mat-label>
                <textarea matInput [(ngModel)]="f.diagnostico_nutricional" rows="3"
                  placeholder="Estado nutricional actual, deficiencias identificadas..."></textarea>
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Requerimiento calórico (kcal/día)</mat-label>
                <input matInput [(ngModel)]="f.requerimiento_calorico" type="number" placeholder="2000" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Requerimiento proteico (g/día)</mat-label>
                <input matInput [(ngModel)]="f.requerimiento_proteico" type="number" placeholder="80" />
              </mat-form-field>
              <mat-form-field appearance="outline" style="grid-column:1/-1">
                <mat-label>Observaciones</mat-label>
                <textarea matInput [(ngModel)]="f.observaciones" rows="3"
                  placeholder="Observaciones adicionales de la valoración..."></textarea>
              </mat-form-field>
            </div>
          </div>

          <div class="step-actions">
            <button matButton (click)="volverLista()">Cancelar</button>
            <button matButton="filled" (click)="guardar()"
              [disabled]="guardando() || !f.peso || !f.talla || !f.diagnostico_nutricional || (!editando() && !pacienteSeleccionado)">
              <i [class]="guardando() ? 'pi pi-spin pi-spinner' : 'pi pi-check'"></i>
              {{ editando() ? 'Guardar cambios' : 'Registrar valoración' }}
            </button>
          </div>
        </div>
      }

      <!-- ═══════════════════════════════════════ DETALLE ═ -->
      @if (vista() === 'detalle' && actual()) {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <button matIconButton (click)="volverLista()"><i class="pi pi-arrow-left"></i></button>
          <h2 class="page-title" style="margin:0">Valoración Nutricional</h2>
          <div style="margin-left:auto;display:flex;gap:0.5rem">
            <button matButton (click)="abrirEditar(actual()!)">
              <i class="pi pi-file-edit"></i> Editar valoración
            </button>
          </div>
        </div>

        <!-- Header -->
        <div class="info-header">
          <div class="info-block">
            <div class="info-label">Paciente</div>
            <div class="info-name">{{ actual()!.paciente_nombre }}</div>
            <div class="info-row">{{ actual()!.paciente_identificacion }}</div>
          </div>
          <div class="info-block sep">
            <div class="info-label">Nutricionista</div>
            <div class="info-name" style="font-size:0.95rem">{{ actual()!.nutricionista_nombre }}</div>
            <div class="info-row">{{ actual()!.fecha_valoracion | date:'dd/MM/yyyy' }}</div>
          </div>
          <div class="info-block sep">
            <div class="info-label">Screening</div>
            <div class="info-name" style="font-size:0.9rem">{{ actual()!.screening_nutricional }}</div>
            @if (actual()!.puntaje_screening !== null && actual()!.puntaje_screening !== undefined) {
              <div class="info-row">Puntaje: {{ actual()!.puntaje_screening }}</div>
            }
          </div>
          <div class="info-block sep">
            <div class="info-label">Riesgo nutricional</div>
            <div style="margin-top:0.25rem">
              <x-etiqueta [value]="actual()!.riesgo_nutricional" [severity]="riesgoSeverity(actual()!.riesgo_nutricional)" />
            </div>
          </div>
        </div>

        <!-- Datos antropométricos -->
        <div class="antropo-grid">
          <div class="antropo-card">
            <div class="antropo-label">Peso</div>
            <div class="antropo-value">{{ actual()!.peso }} <span class="antropo-unit">kg</span></div>
          </div>
          <div class="antropo-card">
            <div class="antropo-label">Talla</div>
            <div class="antropo-value">{{ actual()!.talla }} <span class="antropo-unit">m</span></div>
          </div>
          <div class="antropo-card" [class]="'imc-card-' + imcCategoria(actual()!.imc)">
            <div class="antropo-label">IMC</div>
            <div class="antropo-value">{{ actual()!.imc | number:'1.1-1' }}</div>
            <div class="antropo-cat">{{ imcLabel(actual()!.imc) }}</div>
          </div>
          @if (actual()!.requerimiento_calorico) {
            <div class="antropo-card">
              <div class="antropo-label">Req. calórico</div>
              <div class="antropo-value">{{ actual()!.requerimiento_calorico }} <span class="antropo-unit">kcal</span></div>
            </div>
          }
          @if (actual()!.requerimiento_proteico) {
            <div class="antropo-card">
              <div class="antropo-label">Req. proteico</div>
              <div class="antropo-value">{{ actual()!.requerimiento_proteico }} <span class="antropo-unit">g/día</span></div>
            </div>
          }
          @if (actual()!.albumina) {
            <div class="antropo-card">
              <div class="antropo-label">Albúmina</div>
              <div class="antropo-value">{{ actual()!.albumina }} <span class="antropo-unit">g/dL</span></div>
            </div>
          }
        </div>

        <!-- Diagnóstico -->
        <div style="margin-bottom:1.25rem">
          <div class="section-label">Diagnóstico nutricional</div>
          <div class="texto-bloque">{{ actual()!.diagnostico_nutricional }}</div>
        </div>
        @if (actual()!.observaciones) {
          <div style="margin-bottom:1.25rem">
            <div class="section-label">Observaciones</div>
            <div class="texto-bloque">{{ actual()!.observaciones }}</div>
          </div>
        }

        <mat-divider />

        <!-- Plan de alimentación -->
        <div class="section-label" style="margin-bottom:0.75rem">Plan de alimentación</div>
        @if (cargandoPlan()) {
          <div class="cargando" style="padding:1rem"><i class="pi pi-spin pi-spinner"></i></div>
        } @else if (!planActual()) {
          @if (!mostrarFormPlan()) {
            <div style="display:flex;align-items:center;gap:1rem;padding:1rem;background:rgba(255,255,255,0.02);border:1px dashed rgba(255,255,255,0.1);border-radius:8px">
              <span style="color:#64748b;font-size:0.875rem">No hay plan de alimentación registrado para esta valoración.</span>
              <button matButton="filled" (click)="mostrarFormPlan.set(true)">
                <i class="pi pi-plus"></i> Crear plan
              </button>
            </div>
          } @else {
            <div class="form-card" style="margin-top:0">
              <div class="grid-3" style="margin-bottom:1rem">
                <mat-form-field appearance="outline">
                  <mat-label>Tipo de dieta *</mat-label>
                  <mat-select [(ngModel)]="pForm.tipo_dieta">
                    @for (o of opcionesDieta; track o.value) {
                      <mat-option [value]="o.value">{{ o.label }}</mat-option>
                    }
                  </mat-select>
                </mat-form-field>
                <mat-form-field appearance="outline">
                  <mat-label>Vía de alimentación</mat-label>
                  <mat-select [(ngModel)]="pForm.via_alimentacion">
                    @for (o of opcionesVia; track o.value) {
                      <mat-option [value]="o.value">{{ o.label }}</mat-option>
                    }
                  </mat-select>
                </mat-form-field>
                <mat-form-field appearance="outline">
                  <mat-label>Calorías totales (kcal/día) *</mat-label>
                  <input matInput [(ngModel)]="pForm.calorias_totales" type="number" placeholder="2000" />
                </mat-form-field>
              </div>
              <div class="form-grid" style="margin-bottom:1rem">
                <mat-form-field appearance="outline">
                  <mat-label>Restricciones alimentarias</mat-label>
                  <textarea matInput [(ngModel)]="pForm.restricciones" rows="3"
                    placeholder="Alergias, intolerancias, restricciones médicas..."></textarea>
                </mat-form-field>
                <mat-form-field appearance="outline">
                  <mat-label>Suplementos indicados</mat-label>
                  <textarea matInput [(ngModel)]="pForm.suplementos" rows="3"
                    placeholder="Vitaminas, minerales, suplementos proteicos..."></textarea>
                </mat-form-field>
                <mat-form-field appearance="outline" style="grid-column:1/-1">
                  <mat-label>Observaciones del plan</mat-label>
                  <textarea matInput [(ngModel)]="pForm.observaciones" rows="2"
                    placeholder="Indicaciones adicionales..."></textarea>
                </mat-form-field>
              </div>
              <div style="display:flex;gap:0.5rem;justify-content:flex-end">
                <button matButton (click)="mostrarFormPlan.set(false)">Cancelar</button>
                <button matButton="filled" (click)="guardarPlan()"
                  [disabled]="guardandoPlan() || !pForm.tipo_dieta || !pForm.calorias_totales">
                  <i [class]="guardandoPlan() ? 'pi pi-spin pi-spinner' : 'pi pi-check'"></i> Guardar plan
                </button>
              </div>
            </div>
          }
        } @else {
          <div class="plan-card">
            <div class="plan-header">
              <div>
                <span class="plan-tipo">{{ planActual()!.tipo_dieta }}</span>
                <span class="plan-via">· {{ planActual()!.via_alimentacion }}</span>
              </div>
              <div style="display:flex;align-items:center;gap:0.75rem">
                <span class="plan-kcal">{{ planActual()!.calorias_totales }} kcal/día</span>
                <button matIconButton matTooltip="Editar plan" (click)="editarPlanActual()">
                  <i class="pi pi-file-edit"></i>
                </button>
              </div>
            </div>
            @if (planActual()!.restricciones) {
              <div class="plan-row">
                <span class="plan-key">Restricciones:</span>
                <span>{{ planActual()!.restricciones }}</span>
              </div>
            }
            @if (planActual()!.suplementos) {
              <div class="plan-row">
                <span class="plan-key">Suplementos:</span>
                <span>{{ planActual()!.suplementos }}</span>
              </div>
            }
            @if (planActual()!.observaciones) {
              <div class="plan-row">
                <span class="plan-key">Obs.:</span>
                <span>{{ planActual()!.observaciones }}</span>
              </div>
            }
          </div>
        }
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
    .peligro { color: #dc2626; }
    .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.25rem; }
    .list-header .page-title { margin-bottom: 0; }
    .cargando { padding: 3rem; text-align: center; color: #64748b; display: flex; align-items: center; justify-content: center; gap: 0.5rem; }
    .sin-resultados { text-align: center; padding: 3rem 1rem; color: #64748b; display: flex; flex-direction: column; align-items: center; gap: 0.75rem; }

    /* IMC badges */
    .imc-badge {
      display: inline-block; padding: 0.15rem 0.5rem; border-radius: 4px;
      font-size: 0.82rem; font-weight: 700;
    }
    .imc-bajo { background: rgba(56,189,248,0.15); color: #38bdf8; }
    .imc-normal { background: rgba(74,222,128,0.15); color: #4ade80; }
    .imc-sobrepeso { background: rgba(251,191,36,0.15); color: #fbbf24; }
    .imc-obesidad { background: rgba(248,113,113,0.15); color: #f87171; }

    /* Form */
    .form-card { background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.07); border-radius: 10px; padding: 1.5rem; }
    .form-section { margin-bottom: 0.25rem; }
    .section-label { font-size: 0.73rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em; color: #818cf8; margin-bottom: 0.75rem; }
    .form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
    .grid-3 { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 1rem; }
    .grid-4 { display: grid; grid-template-columns: repeat(4, 1fr); gap: 1rem; }
    .field { display: flex; flex-direction: column; gap: 0.35rem; }
    .field label { font-weight: 500; font-size: 0.875rem; color: #e2e8f0; }
    .w-full { width: 100%; }

    .imc-display {
      padding: 0.5rem 0.75rem; border-radius: 6px; font-size: 1.1rem; font-weight: 700;
      border: 1px solid rgba(255,255,255,0.1);
      display: flex; align-items: center; gap: 0.5rem; min-height: 38px;
    }
    .imc-cat { font-size: 0.72rem; font-weight: 500; opacity: 0.8; }

    .paciente-chip { display: flex; align-items: center; gap: 0.75rem; background: rgba(99,102,241,0.1); border: 1px solid rgba(99,102,241,0.25); border-radius: 8px; padding: 0.6rem 1rem; margin-top: 0.75rem; }

    /* Info header */
    .info-header { display: flex; gap: 1.5rem; flex-wrap: wrap; background: #0f172a; border: 1px solid rgba(255,255,255,0.1); border-radius: 8px; padding: 1rem 1.25rem; margin-bottom: 1.25rem; }
    .info-block { display: flex; flex-direction: column; gap: 0.2rem; }
    .info-block.sep { border-left: 1px solid rgba(255,255,255,0.1); padding-left: 1.25rem; }
    .info-label { font-size: 0.7rem; font-weight: 600; text-transform: uppercase; color: #94a3b8; letter-spacing: 0.05em; }
    .info-name { font-size: 1rem; font-weight: 600; color: #f1f5f9; }
    .info-row { font-size: 0.82rem; color: #cbd5e1; }

    /* Datos antropométricos */
    .antropo-grid { display: flex; gap: 0.75rem; flex-wrap: wrap; margin-bottom: 1.25rem; }
    .antropo-card { background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 8px; padding: 0.75rem 1rem; min-width: 110px; }
    .imc-card-normal { border-color: rgba(74,222,128,0.25); background: rgba(74,222,128,0.05); }
    .imc-card-sobrepeso { border-color: rgba(251,191,36,0.25); background: rgba(251,191,36,0.05); }
    .imc-card-obesidad { border-color: rgba(248,113,113,0.25); background: rgba(248,113,113,0.05); }
    .imc-card-bajo { border-color: rgba(56,189,248,0.25); background: rgba(56,189,248,0.05); }
    .antropo-label { font-size: 0.7rem; font-weight: 600; text-transform: uppercase; color: #94a3b8; letter-spacing: 0.04em; }
    .antropo-value { font-size: 1.4rem; font-weight: 700; color: #f1f5f9; line-height: 1.2; margin-top: 0.15rem; }
    .antropo-unit { font-size: 0.75rem; font-weight: 400; color: #94a3b8; }
    .antropo-cat { font-size: 0.72rem; color: #94a3b8; margin-top: 0.15rem; }

    .texto-bloque { background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.07); border-radius: 6px; padding: 0.875rem 1rem; font-size: 0.875rem; color: #cbd5e1; line-height: 1.6; white-space: pre-wrap; }

    /* Plan */
    .plan-card { background: rgba(99,102,241,0.05); border: 1px solid rgba(99,102,241,0.2); border-radius: 8px; padding: 1rem 1.25rem; }
    .plan-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.75rem; }
    .plan-tipo { font-weight: 700; color: #a5b4fc; font-size: 1rem; }
    .plan-via { color: #64748b; font-size: 0.85rem; margin-left: 0.4rem; }
    .plan-kcal { font-weight: 700; color: #4ade80; }
    .plan-row { font-size: 0.875rem; color: #cbd5e1; margin-bottom: 0.35rem; }
    .plan-key { color: #94a3b8; font-weight: 500; margin-right: 0.4rem; }

    .step-actions { display: flex; gap: 0.5rem; justify-content: flex-end; border-top: 1px solid rgba(255,255,255,0.1); margin-top: 1.25rem; padding-top: 1rem; }
    ::ng-deep textarea { resize: vertical; }
  `]
})
export class NutricionComponent implements OnInit {
  private svc = inject(NutricionService);
  private messageService = inject(Avisos);
  private confirmationService = inject(Confirmar);

  vista = signal<Vista>('lista');
  cargando = signal(false);
  lista = signal<ValoracionNutricional[]>([]);
  actual = signal<ValoracionNutricional | null>(null);
  editando = signal(false);
  guardando = signal(false);
  filtroRiesgo = '';

  // Plan
  planActual = signal<PlanAlimentacion | null>(null);
  cargandoPlan = signal(false);
  mostrarFormPlan = signal(false);
  guardandoPlan = signal(false);
  pForm: Partial<PlanCreate> = { via_alimentacion: 'ORAL' };

  // Búsqueda paciente
  busquedaPaciente = '';
  buscandoPaciente = signal(false);
  resultadosPaciente = signal<any[]>([]);
  pacienteSeleccionado: any = null;

  f: Partial<ValoracionCreate> = { screening_nutricional: 'NRS-2002', riesgo_nutricional: 'BAJO' };

  readonly opcionesRiesgo = [
    { label: 'Todos', value: '' },
    { label: 'Bajo', value: 'BAJO' },
    { label: 'Moderado', value: 'MODERADO' },
    { label: 'Alto', value: 'ALTO' },
  ];

  /** Los mismos sin «Todos»: eso es un filtro, no un riesgo que se pueda
   *  afirmar de un paciente. */
  readonly riesgosElegibles = this.opcionesRiesgo.slice(1);

  /** El orden de las columnas lo fija esta lista, no la plantilla. */
  readonly columnas = ['paciente', 'nutricionista', 'fecha', 'peso', 'imc',
                       'screening', 'riesgo', 'acciones'];

  readonly opcionesScreening = [
    { label: 'NRS-2002', value: 'NRS-2002' },
    { label: 'MST', value: 'MST' },
    { label: 'MUST', value: 'MUST' },
    { label: 'MNA', value: 'MNA' },
  ];

  readonly opcionesDieta = [
    { label: 'Normal', value: 'NORMAL' },
    { label: 'Blanda', value: 'BLANDA' },
    { label: 'Líquida', value: 'LIQUIDA' },
    { label: 'Hipocalórica', value: 'HIPOCALORICA' },
    { label: 'Hipoproteica', value: 'HIPOPROTEICA' },
    { label: 'Hiperproteica', value: 'HIPERPROTEICA' },
    { label: 'Diabética', value: 'DIABETICA' },
    { label: 'Sin gluten', value: 'SIN_GLUTEN' },
    { label: 'Sin lactosa', value: 'SIN_LACTOSA' },
    { label: 'Otra', value: 'OTRA' },
  ];

  readonly opcionesVia = [
    { label: 'Oral', value: 'ORAL' },
    { label: 'Enteral', value: 'ENTERAL' },
    { label: 'Parenteral', value: 'PARENTERAL' },
    { label: 'Mixta', value: 'MIXTA' },
  ];

  ngOnInit(): void { this.cargar(); }

  cargar(): void {
    this.cargando.set(true);
    this.svc.listValoraciones(undefined, this.filtroRiesgo || undefined).subscribe({
      next: vs => { this.lista.set(vs); this.cargando.set(false); },
      error: () => { this.cargando.set(false); this.err('Error al cargar valoraciones'); },
    });
  }

  abrirNueva(): void {
    this.editando.set(false);
    this.actual.set(null);
    this.busquedaPaciente = '';
    this.resultadosPaciente.set([]);
    this.pacienteSeleccionado = null;
    this.f = { screening_nutricional: 'NRS-2002', riesgo_nutricional: 'BAJO' };
    this.vista.set('valoracion');
  }

  abrirEditar(v: ValoracionNutricional): void {
    this.editando.set(true);
    this.actual.set(v);
    this.f = {
      peso: v.peso, talla: v.talla, imc: v.imc,
      circunferencia_brazo: v.circunferencia_brazo,
      pliegue_tricipital: v.pliegue_tricipital,
      screening_nutricional: v.screening_nutricional,
      puntaje_screening: v.puntaje_screening,
      riesgo_nutricional: v.riesgo_nutricional,
      diagnostico_nutricional: v.diagnostico_nutricional,
      requerimiento_calorico: v.requerimiento_calorico,
      requerimiento_proteico: v.requerimiento_proteico,
      albumina: v.albumina, prealbumina: v.prealbumina,
      observaciones: v.observaciones,
    };
    this.vista.set('valoracion');
  }

  abrirDetalle(v: ValoracionNutricional): void {
    this.actual.set(v);
    this.planActual.set(null);
    this.mostrarFormPlan.set(false);
    this.pForm = { via_alimentacion: 'ORAL' };
    this.cargandoPlan.set(true);
    this.svc.getPlan(v.id).subscribe({
      next: p => { this.planActual.set(p); this.cargandoPlan.set(false); },
      error: () => this.cargandoPlan.set(false),
    });
    this.vista.set('detalle');
  }

  volverLista(): void { this.vista.set('lista'); this.cargar(); }

  calcularIMC(): void {
    const p = Number(this.f.peso);
    const t = Number(this.f.talla);
    if (p > 0 && t > 0) {
      this.f.imc = Math.round((p / (t * t)) * 10) / 10;
    }
  }

  buscarPaciente(): void {
    if (!this.busquedaPaciente.trim()) return;
    this.buscandoPaciente.set(true);
    this.svc.searchPacientes(this.busquedaPaciente).subscribe({
      next: ps => { this.resultadosPaciente.set(ps); this.buscandoPaciente.set(false); if (!ps.length) this.err('No se encontraron pacientes'); },
      error: () => { this.buscandoPaciente.set(false); this.err('Error al buscar'); },
    });
  }

  guardar(): void {
    this.guardando.set(true);
    if (this.editando()) {
      this.svc.updateValoracion(this.actual()!.id, this.f).subscribe({
        next: () => { this.guardando.set(false); this.ok('Guardado'); this.volverLista(); },
        error: err => { this.guardando.set(false); this.err(err?.error?.detail ?? 'Error'); },
      });
    } else {
      this.f.paciente_id = this.pacienteSeleccionado.id;
      this.svc.createValoracion(this.f as ValoracionCreate).subscribe({
        next: () => { this.guardando.set(false); this.ok('Valoración registrada'); this.volverLista(); },
        error: err => { this.guardando.set(false); this.err(err?.error?.detail ?? 'Error'); },
      });
    }
  }

  editarPlanActual(): void {
    const p = this.planActual();
    if (!p) return;
    this.pForm = {
      tipo_dieta: p.tipo_dieta, via_alimentacion: p.via_alimentacion,
      calorias_totales: p.calorias_totales, restricciones: p.restricciones,
      suplementos: p.suplementos, observaciones: p.observaciones,
    };
    this.planActual.set(null);
    this.mostrarFormPlan.set(true);
  }

  guardarPlan(): void {
    const v = this.actual();
    if (!v || !this.pForm.tipo_dieta || !this.pForm.calorias_totales) return;
    this.guardandoPlan.set(true);
    const payload: PlanCreate = {
      tipo_dieta: this.pForm.tipo_dieta!,
      calorias_totales: Number(this.pForm.calorias_totales),
      via_alimentacion: this.pForm.via_alimentacion || 'ORAL',
      distribucion_calorica: {}, menu_diario: {},
      restricciones: this.pForm.restricciones,
      suplementos: this.pForm.suplementos,
      observaciones: this.pForm.observaciones,
    };
    this.svc.createPlan(v.id, payload).subscribe({
      next: p => { this.planActual.set(p); this.mostrarFormPlan.set(false); this.guardandoPlan.set(false); this.ok('Plan guardado'); },
      error: () => {
        // If already exists, try update (shouldn't happen normally)
        this.guardandoPlan.set(false);
        this.err('Error al guardar el plan');
      },
    });
  }

  confirmarEliminar(v: ValoracionNutricional): void {
    this.confirmationService.confirm({
      message: `¿Eliminar la valoración de <strong>${v.paciente_nombre}</strong>?`,
      header: 'Confirmar', icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Eliminar', rejectLabel: 'Cancelar',
      peligroso: true,
      accept: () => {
        this.svc.deleteValoracion(v.id).subscribe({
          next: () => { this.lista.update(vs => vs.filter(x => x.id !== v.id)); this.ok('Eliminado'); },
          error: () => this.err('Error al eliminar'),
        });
      },
    });
  }

  imcCategoria(imc: number): string {
    if (imc < 18.5) return 'bajo';
    if (imc < 25) return 'normal';
    if (imc < 30) return 'sobrepeso';
    return 'obesidad';
  }

  imcLabel(imc: number): string {
    if (imc < 18.5) return 'Bajo peso';
    if (imc < 25) return 'Normal';
    if (imc < 30) return 'Sobrepeso';
    if (imc < 35) return 'Obesidad I';
    if (imc < 40) return 'Obesidad II';
    return 'Obesidad III';
  }

  riesgoSeverity(r: string): 'success' | 'warn' | 'danger' {
    if (r === 'BAJO') return 'success';
    if (r === 'MODERADO') return 'warn';
    return 'danger';
  }

  private ok(summary: string, detail?: string): void {
    this.messageService.add({ severity: 'success', summary, detail });
  }

  private err(detail: string): void {
    this.messageService.add({ severity: 'error', summary: 'Error', detail });
  }
}
