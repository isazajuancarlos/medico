import { Component, Inject, OnInit, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { forkJoin, of } from 'rxjs';
import { MatButtonModule } from '@angular/material/button';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MAT_DIALOG_DATA, MatDialog, MatDialogModule } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatTableModule } from '@angular/material/table';
import { MatTabsModule } from '@angular/material/tabs';
import { MatTooltipModule } from '@angular/material/tooltip';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { Avisos } from '../../shared/ui/avisos.service';
import { diaLocal } from '../../shared/fecha';
import { CitaService } from '../../services/cita.service';
import { MedicoDisponible } from '../../models/appointment.models';

const DIAS = ['Lunes', 'Martes', 'Miércoles', 'Jueves', 'Viernes', 'Sábado', 'Domingo'];
const MESES = ['Enero','Febrero','Marzo','Abril','Mayo','Junio','Julio','Agosto','Septiembre','Octubre','Noviembre','Diciembre'];
/** El tipo de bloqueo se traduce en las dos direcciones, igual que el estado y
 *  el tipo de la cita: la columna guarda el vocabulario de la base y la pantalla
 *  enseña el del usuario. Guardar «Día Libre» con tilde y espacio en una columna
 *  es lo que dejó muerta la pantalla de laboratorio (ver la migración 0027). */
const TIPOS_BLOQUEO: Record<string, string> = {
  Vacaciones: 'VACACIONES',
  'Capacitación': 'CAPACITACION',
  Incapacidad: 'INCAPACIDAD',
  'Reunión': 'REUNION',
  'Día Libre': 'DIA_LIBRE',
  Otro: 'OTRO',
};

const TIPO_BLOQUEO_A_PANTALLA: Record<string, string> = Object.fromEntries(
  Object.entries(TIPOS_BLOQUEO).map(([pantalla, api]) => [api, pantalla]),
);

const ANIOS = [new Date().getFullYear(), new Date().getFullYear() + 1];
const MESES_OPT = MESES.map((m, i) => ({ label: m, value: i + 1 }));
const DIAS_OPT = DIAS.map((d, i) => ({ label: d, value: i }));

/**
 * `2026-08-10T14:30` → `2026-08-10 14:30`, que es lo que guarda la columna.
 *
 * El campo es un `datetime-local`, o sea la hora de pared tal como la escribió
 * quien la escribió. Antes era un calendario que entregaba un `Date` y se
 * convertía con `toISOString()`: además de otro formato, es UTC, y con eso un
 * bloqueo de la tarde en Colombia se guardaba al día siguiente.
 */
function aInstante(v: string | null | undefined): string | null {
  if (!v) return null;
  return v.replace('T', ' ').slice(0, 16);
}

interface BloqueForm {
  id?: number;
  dia_semana: number;
  hora_inicio: string;
  hora_fin: string;
  activo: boolean;
}

interface AgendaForm {
  medico_id: number | null;
  anio: number;
  mes: number | null;
  observaciones: string;
  bloques: BloqueForm[];
}

/** Alta o edición de una agenda mensual. Devuelve el formulario, o nada. */
@Component({
  selector: 'app-agenda-dialogo',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatSelectModule, MatTooltipModule,
  ],
  template: `
    <h2 mat-dialog-title>{{ editando ? 'Editar agenda' : 'Nueva agenda mensual' }}</h2>
    <mat-dialog-content>
      @if (!editando) {
        <div class="cabecera">
          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Médico *</mat-label>
            <mat-select [(ngModel)]="f.medico_id">
              @for (m of medicos; track m.id) {
                <mat-option [value]="m.id">{{ m.nombre }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Año *</mat-label>
            <mat-select [(ngModel)]="f.anio">
              @for (a of anios; track a) { <mat-option [value]="a">{{ a }}</mat-option> }
            </mat-select>
          </mat-form-field>
          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Mes *</mat-label>
            <mat-select [(ngModel)]="f.mes">
              @for (m of meses; track m.value) {
                <mat-option [value]="m.value">{{ m.label }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
        </div>
      }

      <div class="bloques-cabecera">
        <strong>Bloques de atención</strong>
        <button matButton (click)="agregarBloque()"><i class="pi pi-plus"></i> Agregar bloque</button>
      </div>
      <p class="nota">
        Duración mínima por consulta: medicina general ≥20 min · especialista ≥30 min
        (Circular 049/2008 Supersalud)
      </p>

      @for (bloque of f.bloques; track $index; let i = $index) {
        <div class="bloque">
          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Día</mat-label>
            <mat-select [(ngModel)]="bloque.dia_semana">
              @for (d of dias; track d.value) {
                <mat-option [value]="d.value">{{ d.label }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Desde</mat-label>
            <input matInput type="time" [(ngModel)]="bloque.hora_inicio" />
          </mat-form-field>
          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Hasta</mat-label>
            <input matInput type="time" [(ngModel)]="bloque.hora_fin" />
          </mat-form-field>
          <button matIconButton class="peligro" matTooltip="Quitar el bloque" (click)="quitarBloque(i)">
            <i class="pi pi-trash"></i>
          </button>
        </div>
      }

      @if (f.bloques.length === 0) {
        <p class="vacio">No hay bloques. Agregue al menos uno para la semana laboral.</p>
      }

      <mat-form-field appearance="outline" class="ancho">
        <mat-label>Observaciones</mat-label>
        <input matInput [(ngModel)]="f.observaciones" placeholder="Notas opcionales" />
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!completo()" [mat-dialog-close]="resultado()">
        <i class="pi pi-check"></i> Guardar
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .cabecera { display:grid; grid-template-columns:1fr 1fr 1fr; gap:1rem; margin-bottom:1.25rem; }
    .bloques-cabecera { display:flex; justify-content:space-between; align-items:center; margin-bottom:.5rem; }
    .nota { font-size:.75rem; color:#64748b; margin:.25rem 0 .75rem; }
    .bloque {
      display:grid; grid-template-columns:2fr 1fr 1fr auto; gap:.5rem;
      align-items:center; margin-bottom:.5rem;
    }
    .vacio { color:#9ca3af; font-size:.875rem; text-align:center; padding:.75rem; }
    .ancho { width:100%; margin-top:1rem; }
    mat-dialog-content { min-width:min(42rem, 88vw); }
    mat-form-field { width:100%; }
    .peligro { color:#dc2626; }
  `],
})
export class AgendaDialogoComponent {
  readonly anios = ANIOS;
  readonly meses = MESES_OPT;
  readonly dias = DIAS_OPT;

  readonly medicos: MedicoDisponible[];
  readonly editando: boolean;
  f: AgendaForm;

  constructor(
    @Inject(MAT_DIALOG_DATA) datos: { medicos: MedicoDisponible[]; agenda: any | null },
  ) {
    this.medicos = datos.medicos;
    this.editando = !!datos.agenda;
    const a = datos.agenda;
    this.f = a
      ? {
          medico_id: a.medico_id,
          anio: a.anio,
          mes: a.mes,
          observaciones: a.observaciones ?? '',
          bloques: a.bloques.map((b: any) => ({
            id: b.id,
            dia_semana: b.dia_semana,
            hora_inicio: b.hora_inicio,
            hora_fin: b.hora_fin,
            activo: b.activo,
          })),
        }
      : { medico_id: null, anio: ANIOS[0], mes: null, observaciones: '', bloques: [] };
  }

  agregarBloque(): void {
    this.f.bloques.push({ dia_semana: 0, hora_inicio: '08:00', hora_fin: '12:00', activo: true });
  }

  quitarBloque(i: number): void { this.f.bloques.splice(i, 1); }

  completo(): boolean {
    return this.editando || (!!this.f.medico_id && !!this.f.mes);
  }

  resultado(): AgendaForm | null {
    return this.completo() ? this.f : null;
  }
}

/** Un bloqueo de agenda. Devuelve lo que espera el servidor, o nada. */
@Component({
  selector: 'app-bloqueo-dialogo',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatSelectModule,
  ],
  template: `
    <h2 mat-dialog-title>Nuevo bloqueo de agenda</h2>
    <mat-dialog-content>
      <div class="campos">
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Médico *</mat-label>
          <mat-select [(ngModel)]="medicoId">
            @for (m of medicos; track m.id) {
              <mat-option [value]="m.id">{{ m.nombre }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Desde *</mat-label>
          <input matInput type="datetime-local" [(ngModel)]="inicio" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Hasta *</mat-label>
          <input matInput type="datetime-local" [(ngModel)]="fin" />
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Tipo *</mat-label>
          <mat-select [(ngModel)]="tipo">
            @for (t of tipos; track t) { <mat-option [value]="t">{{ t }}</mat-option> }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Motivo *</mat-label>
          <input matInput [(ngModel)]="motivo" placeholder="Descripción del bloqueo" />
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!completo()" [mat-dialog-close]="resultado()">
        <i class="pi pi-check"></i> Guardar
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .campos {
      display:grid; grid-template-columns:1fr 1fr; gap:1rem; padding-top:.5rem;
      min-width:min(32rem, 85vw);
    }
    .ancho { grid-column:1/-1; }
    mat-form-field { width:100%; }
  `],
})
export class BloqueoDialogoComponent {
  readonly tipos = Object.keys(TIPOS_BLOQUEO);
  readonly medicos: MedicoDisponible[];

  medicoId: number | null = null;
  inicio = '';
  fin = '';
  tipo = 'Otro';
  motivo = '';

  constructor(@Inject(MAT_DIALOG_DATA) datos: { medicos: MedicoDisponible[] }) {
    this.medicos = datos.medicos;
  }

  completo(): boolean {
    return !!this.medicoId && !!this.inicio && !!this.fin && !!this.motivo.trim();
  }

  resultado(): Record<string, unknown> | null {
    if (!this.completo()) return null;
    return {
      medico_id: this.medicoId,
      motivo: this.motivo.trim(),
      tipo: TIPOS_BLOQUEO[this.tipo] ?? 'OTRO',
      inicio: aInstante(this.inicio),
      fin: aInstante(this.fin),
    };
  }
}

@Component({
  selector: 'app-agenda-medica',
  standalone: true,
  imports: [
    FormsModule, MatTabsModule, MatTableModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatSelectModule, MatDatepickerModule, MatProgressBarModule,
    MatTooltipModule, EtiquetaComponent,
  ],
  template: `
<div class="page-card">
  <h2 class="page-title">Agenda Médica</h2>
  <p class="normativa">
    Gestión de horarios de atención conforme a Res. 256/2016 · Res. 2003/2014 · Decreto 780/2016
  </p>

  <mat-tab-group [(selectedIndex)]="pestana" (selectedIndexChange)="alCambiarPestana($event)">
    <!-- ── Horarios mensuales ─────────────────────────────────── -->
    <mat-tab label="Horarios Mensuales">
      <div class="panel">
        <div class="barra">
          <div class="filtros">
            <mat-form-field appearance="outline" subscriptSizing="dynamic">
              <mat-label>Médico</mat-label>
              <mat-select [(ngModel)]="filterMedico" (selectionChange)="loadAgendas()">
                <mat-option [value]="null">Todos los médicos</mat-option>
                @for (m of medicos(); track m.id) {
                  <mat-option [value]="m.id">{{ m.nombre }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
            <mat-form-field appearance="outline" subscriptSizing="dynamic">
              <mat-label>Año</mat-label>
              <mat-select [(ngModel)]="filterAnio" (selectionChange)="loadAgendas()">
                @for (a of anios; track a) { <mat-option [value]="a">{{ a }}</mat-option> }
              </mat-select>
            </mat-form-field>
            <mat-form-field appearance="outline" subscriptSizing="dynamic">
              <mat-label>Mes</mat-label>
              <mat-select [(ngModel)]="filterMes" (selectionChange)="loadAgendas()">
                <mat-option [value]="null">Todos los meses</mat-option>
                @for (m of meses; track m.value) {
                  <mat-option [value]="m.value">{{ m.label }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
          </div>
          <button matButton="filled" (click)="abrirAgenda(null)">
            <i class="pi pi-plus"></i> Nueva agenda
          </button>
        </div>

        @if (loadingAgendas()) { <mat-progress-bar mode="indeterminate" /> }
        <table mat-table [dataSource]="agendas()" class="tabla">
          <ng-container matColumnDef="medico">
            <th mat-header-cell *matHeaderCellDef>Médico</th>
            <td mat-cell *matCellDef="let a">{{ a.medico_nombre }}</td>
          </ng-container>
          <ng-container matColumnDef="periodo">
            <th mat-header-cell *matHeaderCellDef>Mes / Año</th>
            <td mat-cell *matCellDef="let a">{{ mesNombre(a.mes) }} {{ a.anio }}</td>
          </ng-container>
          <ng-container matColumnDef="bloques">
            <th mat-header-cell *matHeaderCellDef>Bloques</th>
            <td mat-cell *matCellDef="let a">
              <div class="chips">
                @for (b of a.bloques; track b.id) {
                  <span class="chip">{{ b.dia_nombre }}: {{ b.hora_inicio }}-{{ b.hora_fin }}</span>
                }
                @if (a.bloques.length === 0) { <span class="tenue">Sin bloques</span> }
              </div>
            </td>
          </ng-container>
          <ng-container matColumnDef="estado">
            <th mat-header-cell *matHeaderCellDef>Estado</th>
            <td mat-cell *matCellDef="let a">
              <x-etiqueta [value]="a.activa ? 'Activa' : 'Inactiva'"
                [severity]="a.activa ? 'success' : 'secondary'" />
            </td>
          </ng-container>
          <ng-container matColumnDef="acciones">
            <th mat-header-cell *matHeaderCellDef>Acciones</th>
            <td mat-cell *matCellDef="let a">
              <button matIconButton matTooltip="Editar bloques" (click)="abrirAgenda(a)">
                <i class="pi pi-pencil"></i>
              </button>
              <button matIconButton class="peligro" matTooltip="Eliminar" (click)="deleteAgenda(a.id)">
                <i class="pi pi-trash"></i>
              </button>
            </td>
          </ng-container>

          <tr mat-header-row *matHeaderRowDef="columnasAgendas"></tr>
          <tr mat-row *matRowDef="let fila; columns: columnasAgendas"></tr>
        </table>
        @if (!loadingAgendas() && agendas().length === 0) {
          <p class="vacio">No hay agendas configuradas</p>
        }
      </div>
    </mat-tab>

    <!-- ── Bloqueos ───────────────────────────────────────────── -->
    <mat-tab label="Bloqueos">
      <div class="panel">
        <div class="barra">
          <p class="tenue" style="margin:0">
            Vacaciones, incapacidades, reuniones u otros períodos sin atención
          </p>
          <button matButton="filled" (click)="abrirBloqueo()">
            <i class="pi pi-plus"></i> Nuevo bloqueo
          </button>
        </div>

        @if (loadingBloqueos()) { <mat-progress-bar mode="indeterminate" /> }
        <table mat-table [dataSource]="bloqueos()" class="tabla">
          <ng-container matColumnDef="medico">
            <th mat-header-cell *matHeaderCellDef>Médico</th>
            <td mat-cell *matCellDef="let b">{{ b.medico_nombre }}</td>
          </ng-container>
          <ng-container matColumnDef="tipo">
            <th mat-header-cell *matHeaderCellDef>Tipo</th>
            <td mat-cell *matCellDef="let b">
              <x-etiqueta [value]="tipoEnPantalla(b.tipo)" severity="warn" />
            </td>
          </ng-container>
          <ng-container matColumnDef="desde">
            <th mat-header-cell *matHeaderCellDef>Desde</th>
            <td mat-cell *matCellDef="let b">{{ b.inicio }}</td>
          </ng-container>
          <ng-container matColumnDef="hasta">
            <th mat-header-cell *matHeaderCellDef>Hasta</th>
            <td mat-cell *matCellDef="let b">{{ b.fin }}</td>
          </ng-container>
          <ng-container matColumnDef="motivo">
            <th mat-header-cell *matHeaderCellDef>Motivo</th>
            <td mat-cell *matCellDef="let b">{{ b.motivo }}</td>
          </ng-container>
          <ng-container matColumnDef="acciones">
            <th mat-header-cell *matHeaderCellDef>Acciones</th>
            <td mat-cell *matCellDef="let b">
              <button matIconButton class="peligro" matTooltip="Eliminar" (click)="deleteBloqueo(b.id)">
                <i class="pi pi-trash"></i>
              </button>
            </td>
          </ng-container>

          <tr mat-header-row *matHeaderRowDef="columnasBloqueos"></tr>
          <tr mat-row *matRowDef="let fila; columns: columnasBloqueos"></tr>
        </table>
        @if (!loadingBloqueos() && bloqueos().length === 0) {
          <p class="vacio">No hay bloqueos registrados</p>
        }
      </div>
    </mat-tab>

    <!-- ── Indicadores ────────────────────────────────────────── -->
    <mat-tab label="Indicadores (Res. 256/2016)">
      <div class="panel">
        <div class="filtros" style="margin-bottom:1.5rem">
          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Médico</mat-label>
            <mat-select [(ngModel)]="indFilterMedico">
              <mat-option [value]="null">Todos</mat-option>
              @for (m of medicos(); track m.id) {
                <mat-option [value]="m.id">{{ m.nombre }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Desde</mat-label>
            <input matInput [matDatepicker]="calD" [(ngModel)]="indDesde" />
            <mat-datepicker-toggle matIconSuffix [for]="calD" />
            <mat-datepicker #calD />
          </mat-form-field>
          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Hasta</mat-label>
            <input matInput [matDatepicker]="calH" [(ngModel)]="indHasta" />
            <mat-datepicker-toggle matIconSuffix [for]="calH" />
            <mat-datepicker #calH />
          </mat-form-field>
          <button matButton="filled" [disabled]="loadingInd()" (click)="loadIndicadores()">
            <i [class]="loadingInd() ? 'pi pi-spin pi-spinner' : 'pi pi-refresh'"></i> Consultar
          </button>
        </div>

        @if (indicadores()) {
          <div class="kpis">
            <div class="kpi-card">
              <div class="kpi-value">{{ indicadores().atendidas }}</div>
              <div class="kpi-label">Citas Atendidas</div>
            </div>
            <div class="kpi-card">
              <div class="kpi-value" style="color:#f59e0b">{{ indicadores().canceladas }}</div>
              <div class="kpi-label">Canceladas</div>
            </div>
            <div class="kpi-card">
              <div class="kpi-value" style="color:#ef4444">{{ indicadores().no_asistio }}</div>
              <div class="kpi-label">No Asistieron</div>
            </div>
            <div class="kpi-card">
              <div class="kpi-value">{{ indicadores().total }}</div>
              <div class="kpi-label">Total Programadas</div>
            </div>
          </div>

          <!-- LA OPORTUNIDAD, QUE HASTA HOY NO SE CALCULABA.
               Este panel imprimía las metas de la Resolución 256 al lado de tres
               indicadores que no son oportunidad, y quien lo leía entendía que su
               IPS la estaba midiendo. Ya se mide —días entre la solicitud y la
               cita— y se dice SOBRE CUÁNTAS: sin ninguna medible el número es
               «sin datos», no un cero, que sería el mejor resultado posible. -->
          <div class="indicadores">
            <div class="ind-card"
                 [class.ind-ok]="indicadores().oportunidad_dias_promedio !== null && indicadores().oportunidad_dias_promedio <= indicadores().meta_medicina_general_dias"
                 [class.ind-warn]="indicadores().oportunidad_dias_promedio !== null && indicadores().oportunidad_dias_promedio > indicadores().meta_medicina_general_dias">
              <div class="ind-valor">
                {{ indicadores().oportunidad_dias_promedio ?? 'sin datos' }}
                @if (indicadores().oportunidad_dias_promedio !== null) { <span style="font-size:.9rem"> días</span> }
              </div>
              <div style="font-weight:600">Oportunidad promedio</div>
              <div class="ind-nota">
                Medida sobre {{ indicadores().oportunidad_medidas }} de
                {{ indicadores().oportunidad_medidas + indicadores().oportunidad_sin_dato }} citas
                &middot; Meta: ≤ {{ indicadores().meta_medicina_general_dias }} días (general),
                ≤ {{ indicadores().meta_especialista_dias }} (especialista)
              </div>
            </div>
            @if (indicadores().oportunidad_sin_dato > 0) {
              <div class="ind-card ind-warn">
                <div class="ind-valor">{{ indicadores().oportunidad_sin_dato }}</div>
                <div style="font-weight:600">Sin fecha de solicitud</div>
                <div class="ind-nota">
                  No cuentan para la oportunidad. Registre la fecha en que el
                  paciente PIDIÓ la cita para que entren.
                </div>
              </div>
            }
            @if (indicadores().oportunidad_incoherentes > 0) {
              <div class="ind-card ind-warn">
                <div class="ind-valor">{{ indicadores().oportunidad_incoherentes }}</div>
                <div style="font-weight:600">Con solicitud posterior a la cita</div>
                <div class="ind-nota">
                  Imposible: nadie pide una cita después de que ocurra. No se
                  promedian —darían días negativos que MEJORAN el indicador— y
                  hay que corregir la fecha en esas citas.
                </div>
              </div>
            }
          </div>

          <div class="tasas">
            <div class="ind-card" [class.ind-ok]="indicadores().tasa_inasistencia_pct <= 20" [class.ind-warn]="indicadores().tasa_inasistencia_pct > 20">
              <div class="ind-valor">{{ indicadores().tasa_inasistencia_pct }}%</div>
              <div style="font-weight:600">Tasa de Inasistencia</div>
              <div class="ind-nota">Meta: ≤ 20%</div>
            </div>
            <div class="ind-card" [class.ind-ok]="indicadores().tasa_cancelacion_pct <= 10" [class.ind-warn]="indicadores().tasa_cancelacion_pct > 10">
              <div class="ind-valor">{{ indicadores().tasa_cancelacion_pct }}%</div>
              <div style="font-weight:600">Tasa de Cancelación</div>
              <div class="ind-nota">Meta: ≤ 10%</div>
            </div>
          </div>

          <div class="normativa-box">
            <strong>Normativa aplicada:</strong> {{ indicadores().normativa }}
            (Decreto 780/2016, Ley 1438/2011)
          </div>
        } @else if (!loadingInd()) {
          <p class="vacio">
            Seleccione el período y presione «Consultar» para ver los indicadores
          </p>
        }
      </div>
    </mat-tab>
  </mat-tab-group>
</div>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .normativa { font-size:.8rem; color:#64748b; margin-top:-.5rem; margin-bottom:1rem; }
    .panel { padding-top:1rem; }
    .barra { display:flex; justify-content:space-between; align-items:flex-start; gap:1rem;
      margin-bottom:1rem; flex-wrap:wrap; }
    .filtros { display:flex; gap:.5rem; flex-wrap:wrap; }
    .tabla { width:100%; }
    .tenue { color:#64748b; font-size:.875rem; }
    .vacio { text-align:center; padding:2rem; color:#64748b; }
    .chips { display:flex; flex-wrap:wrap; gap:.25rem; }
    .chip { background:#dbeafe; color:#1d4ed8; padding:.125rem .5rem; border-radius:.75rem; font-size:.75rem; }
    .kpis { display:grid; grid-template-columns:repeat(auto-fill,minmax(200px,1fr)); gap:1rem; margin-bottom:1.5rem; }
    .kpi-card { background:#f8fafc; border:1px solid #e2e8f0; border-radius:.5rem; padding:1rem; text-align:center; }
    .kpi-value { font-size:2rem; font-weight:700; color:#1e293b; line-height:1; }
    .kpi-label { font-size:.8rem; color:#64748b; margin-top:.25rem; }
    .indicadores { display:grid; grid-template-columns:repeat(auto-fill,minmax(240px,1fr)); gap:1rem; margin-bottom:1.5rem; }
    .tasas { display:grid; grid-template-columns:1fr 1fr; gap:1rem; }
    .ind-card { padding:1.25rem; border-radius:.5rem; border:1px solid #e2e8f0; }
    .ind-valor { font-size:1.5rem; font-weight:700; }
    .ind-nota { font-size:.75rem; color:#64748b; margin-top:.25rem; }
    .ind-ok { background:#f0fdf4; border-color:#86efac; }
    .ind-warn { background:#fff7ed; border-color:#fdba74; }
    .normativa-box { margin-top:1rem; padding:.75rem; background:#eff6ff; border-radius:.5rem;
      font-size:.8rem; color:#1e40af; }
    /* En M3 el input color de un botón no pinta nada: lo destructivo va por CSS. */
    .peligro { color:#dc2626; }
  `],
})
export class AgendaMedicaComponent implements OnInit {
  private svc = inject(CitaService);
  private dialogo = inject(MatDialog);
  private msg = inject(Avisos);

  readonly columnasAgendas = ['medico', 'periodo', 'bloques', 'estado', 'acciones'];
  readonly columnasBloqueos = ['medico', 'tipo', 'desde', 'hasta', 'motivo', 'acciones'];
  readonly anios = ANIOS;
  readonly meses = MESES_OPT;

  pestana = 0;
  medicos = signal<MedicoDisponible[]>([]);
  agendas = signal<any[]>([]);
  bloqueos = signal<any[]>([]);
  indicadores = signal<any>(null);

  loadingAgendas = signal(false);
  loadingBloqueos = signal(false);
  loadingInd = signal(false);

  filterMedico: number | null = null;
  filterAnio: number = new Date().getFullYear();
  filterMes: number | null = null;
  indFilterMedico: number | null = null;
  indDesde: Date = new Date(new Date().getFullYear(), new Date().getMonth(), 1);
  indHasta: Date = new Date();

  ngOnInit(): void {
    this.svc.getMedicos().subscribe(m => this.medicos.set(m));
    this.loadAgendas();
    this.loadBloqueos();
  }

  alCambiarPestana(i: number): void {
    if (i === 1) this.loadBloqueos();
  }

  mesNombre(mes: number): string {
    return MESES[mes - 1] ?? '';
  }

  loadAgendas(): void {
    this.loadingAgendas.set(true);
    this.svc.getAgendas(
      this.filterMedico ?? undefined,
      this.filterAnio,
      this.filterMes ?? undefined,
    ).subscribe({
      next: a => { this.agendas.set(a); this.loadingAgendas.set(false); },
      error: () => this.loadingAgendas.set(false),
    });
  }

  loadBloqueos(): void {
    this.loadingBloqueos.set(true);
    this.svc.getBloqueos().subscribe({
      next: b => { this.bloqueos.set(b); this.loadingBloqueos.set(false); },
      error: () => this.loadingBloqueos.set(false),
    });
  }

  loadIndicadores(): void {
    this.loadingInd.set(true);
    this.svc.getIndicadoresOportunidad(
      this.indFilterMedico ?? undefined,
      diaLocal(this.indDesde),
      diaLocal(this.indHasta),
    ).subscribe({
      next: d => { this.indicadores.set(d); this.loadingInd.set(false); },
      error: () => this.loadingInd.set(false),
    });
  }

  tipoEnPantalla(t: string): string { return TIPO_BLOQUEO_A_PANTALLA[t] ?? t; }

  abrirAgenda(agenda: any | null): void {
    this.dialogo
      .open(AgendaDialogoComponent, { data: { medicos: this.medicos(), agenda }, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((f?: AgendaForm) => { if (f) this.guardarAgenda(agenda, f); });
  }

  /**
   * Al editar se guardan TAMBIÉN los bloques.
   *
   * Antes solo se mandaban las observaciones: el diálogo dejaba agregar y quitar
   * bloques, se cerraba diciendo «Agenda actualizada» y el horario del médico
   * quedaba igual. Un cambio que se acepta y no ocurre es peor que uno que
   * falla. Los nuevos son los que no tienen `id`; los quitados, los `id` que
   * estaban antes y ya no están.
   */
  private guardarAgenda(previa: any | null, f: AgendaForm): void {
    if (!previa) {
      this.svc.createAgenda(f).subscribe({
        next: () => { this.msg.add({ severity: 'success', summary: 'Agenda creada' }); this.loadAgendas(); },
        error: (err) => this.msg.add({
          severity: 'error', summary: 'Error',
          detail: err?.error?.detail ?? 'No se pudo crear la agenda',
        }),
      });
      return;
    }

    const quedan = new Set(f.bloques.filter(b => b.id).map(b => b.id));
    const eliminados: number[] = previa.bloques
      .map((b: any) => b.id)
      .filter((id: number) => !quedan.has(id));
    const nuevos = f.bloques.filter(b => !b.id);

    forkJoin([
      this.svc.updateAgenda(previa.id, { observaciones: f.observaciones }),
      ...eliminados.map(id => this.svc.deleteBloque(previa.id, id)),
      ...nuevos.map(b => this.svc.addBloque(previa.id, b)),
      of(null),
    ]).subscribe({
      next: () => { this.msg.add({ severity: 'success', summary: 'Agenda actualizada' }); this.loadAgendas(); },
      error: (err) => this.msg.add({
        severity: 'error', summary: 'Error',
        detail: err?.error?.detail ?? 'No se pudo actualizar la agenda',
      }),
    });
  }

  deleteAgenda(id: number): void {
    this.svc.deleteAgenda(id).subscribe({
      next: () => {
        this.msg.add({ severity: 'info', summary: 'Agenda eliminada' });
        this.loadAgendas();
      },
    });
  }

  abrirBloqueo(): void {
    this.dialogo
      .open(BloqueoDialogoComponent, { data: { medicos: this.medicos() }, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((payload?: Record<string, unknown>) => { if (payload) this.guardarBloqueo(payload); });
  }

  private guardarBloqueo(payload: Record<string, unknown>): void {
    this.svc.createBloqueo(payload).subscribe({
      next: (r) => {
        // El bloqueo NO cancela las citas que quedan dentro: si las hay, hay que
        // reasignarlas a mano, y por eso el aviso es `warn` y no `success`.
        const enConflicto = r.citas_en_conflicto ?? 0;
        this.msg.add({
          severity: enConflicto > 0 ? 'warn' : 'success',
          summary: 'Bloqueo creado',
          detail: enConflicto > 0
            ? `Hay ${enConflicto} cita(s) en ese período que requieren reasignación`
            : 'Bloqueo registrado',
        });
        this.loadBloqueos();
      },
      error: (err) => this.msg.add({
        severity: 'error', summary: 'Error',
        detail: err?.error?.detail ?? 'No se pudo crear el bloqueo',
      }),
    });
  }

  deleteBloqueo(id: number): void {
    this.svc.deleteBloqueo(id).subscribe({
      next: () => {
        this.msg.add({ severity: 'info', summary: 'Bloqueo eliminado' });
        this.loadBloqueos();
      },
    });
  }
}
