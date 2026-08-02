import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TableModule } from 'primeng/table';
import { ButtonModule } from 'primeng/button';
import { TabsModule } from 'primeng/tabs';
import { DialogModule } from 'primeng/dialog';
import { InputTextModule } from 'primeng/inputtext';
import { SelectModule } from 'primeng/select';
import { DatePickerModule } from 'primeng/datepicker';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { Avisos } from '../../shared/ui/avisos.service';
import { CardModule } from 'primeng/card';
import { ToggleSwitchModule } from 'primeng/toggleswitch';
import { MatTooltipModule } from '@angular/material/tooltip';
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

/** `Date` → `YYYY-MM-DD HH:MM`, que es lo que guarda la columna. `toISOString()`
 *  daba `2026-08-10T12:00:00.000Z`: además de otro formato, es UTC, y con eso un
 *  bloqueo de la tarde en Colombia se guardaba al día siguiente. */
function aInstante(d: Date | null | undefined): string | null {
  if (!d) return null;
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ` +
         `${p(d.getHours())}:${p(d.getMinutes())}`;
}

@Component({
  selector: 'app-agenda-medica',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    TabsModule, TableModule, ButtonModule, DialogModule,
    InputTextModule, SelectModule, DatePickerModule, EtiquetaComponent,
    CardModule, ToggleSwitchModule, MatTooltipModule,
  ],
  template: `
<div class="page-card">
  <h2 class="page-title">Agenda Médica</h2>
  <p style="font-size:.8rem;color:#64748b;margin-top:-.5rem;margin-bottom:1rem">
    Gestión de horarios de atención conforme a Res. 256/2016 · Res. 2003/2014 · Decreto 780/2016
  </p>

  <p-tabs [value]="activeTab" (valueChange)="onTabChange($event)">
    <p-tablist>
      <p-tab value="agendas">Horarios Mensuales</p-tab>
      <p-tab value="bloqueos">Bloqueos</p-tab>
      <p-tab value="indicadores">Indicadores (Res. 256/2016)</p-tab>
    </p-tablist>

    <!-- ── Tab 1: Agendas ─────────────────────────────────────── -->
    <p-tabpanels>
      <p-tabpanel value="agendas">
        <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem">
          <div style="display:flex;gap:.5rem;flex-wrap:wrap">
            <p-select
              [(ngModel)]="filterMedico"
              [options]="medicos()"
              optionLabel="nombre"
              optionValue="id"
              placeholder="Todos los médicos"
              [showClear]="true"
              (onChange)="loadAgendas()"
              styleClass="p-inputtext-sm"
            />
            <p-select
              [(ngModel)]="filterAnio"
              [options]="anioOpts"
              placeholder="Año"
              (onChange)="loadAgendas()"
              styleClass="p-inputtext-sm"
            />
            <p-select
              [(ngModel)]="filterMes"
              [options]="mesOpts"
              optionLabel="label"
              optionValue="value"
              placeholder="Mes"
              [showClear]="true"
              (onChange)="loadAgendas()"
              styleClass="p-inputtext-sm"
            />
          </div>
          <p-button label="Nueva Agenda" icon="pi pi-plus" size="small" (onClick)="openNewAgenda()" />
        </div>

        <p-table [value]="agendas()" [loading]="loadingAgendas()" styleClass="p-datatable-sm" dataKey="id">
          <ng-template pTemplate="header">
            <tr>
              <th>Médico</th><th>Mes / Año</th><th>Bloques</th><th>Estado</th><th>Acciones</th>
            </tr>
          </ng-template>
          <ng-template pTemplate="body" let-a>
            <tr>
              <td>{{ a.medico_nombre }}</td>
              <td>{{ mesNombre(a.mes) }} {{ getAnio(a) }}</td>
              <td>
                <div style="display:flex;flex-wrap:wrap;gap:.25rem">
                  @for (b of a.bloques; track b.id) {
                    <span style="background:#dbeafe;color:#1d4ed8;padding:.125rem .5rem;border-radius:.75rem;font-size:.75rem">
                      {{ b.dia_nombre }}: {{ b.hora_inicio }}-{{ b.hora_fin }}
                    </span>
                  }
                  @if (a.bloques.length === 0) {
                    <span style="color:#9ca3af;font-size:.8rem">Sin bloques</span>
                  }
                </div>
              </td>
              <td>
                <x-etiqueta [value]="a.activa ? 'Activa' : 'Inactiva'" [severity]="a.activa ? 'success' : 'secondary'" />
              </td>
              <td style="display:flex;gap:.25rem">
                <p-button icon="pi pi-pencil" size="small" [text]="true" (onClick)="editAgenda(a)" matTooltip="Editar bloques" />
                <p-button icon="pi pi-trash" size="small" severity="danger" [text]="true" (onClick)="deleteAgenda(a.id)" />
              </td>
            </tr>
          </ng-template>
          <ng-template pTemplate="emptymessage">
            <tr><td colspan="5" style="text-align:center;padding:2rem;color:#64748b">No hay agendas configuradas</td></tr>
          </ng-template>
        </p-table>
      </p-tabpanel>

      <!-- ── Tab 2: Bloqueos ─────────────────────────────────────── -->
      <p-tabpanel value="bloqueos">
        <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem">
          <p style="font-size:.875rem;color:#64748b;margin:0">
            Vacaciones, incapacidades, reuniones u otros períodos sin atención
          </p>
          <p-button label="Nuevo Bloqueo" icon="pi pi-plus" size="small" (onClick)="openNewBloqueo()" />
        </div>

        <p-table [value]="bloqueos()" [loading]="loadingBloqueos()" styleClass="p-datatable-sm" dataKey="id">
          <ng-template pTemplate="header">
            <tr>
              <th>Médico</th><th>Tipo</th><th>Desde</th><th>Hasta</th><th>Motivo</th><th>Acciones</th>
            </tr>
          </ng-template>
          <ng-template pTemplate="body" let-b>
            <tr>
              <td>{{ b.medico_nombre }}</td>
              <td><x-etiqueta [value]="tipoEnPantalla(b.tipo)" severity="warn" /></td>
              <td>{{ b.inicio }}</td>
              <td>{{ b.fin }}</td>
              <td>{{ b.motivo }}</td>
              <td>
                <p-button icon="pi pi-trash" size="small" severity="danger" [text]="true" (onClick)="deleteBloqueo(b.id)" />
              </td>
            </tr>
          </ng-template>
          <ng-template pTemplate="emptymessage">
            <tr><td colspan="6" style="text-align:center;padding:2rem;color:#64748b">No hay bloqueos registrados</td></tr>
          </ng-template>
        </p-table>
      </p-tabpanel>

      <!-- ── Tab 3: Indicadores ─────────────────────────────────────── -->
      <p-tabpanel value="indicadores">
        <div style="display:flex;gap:.5rem;flex-wrap:wrap;align-items:flex-end;margin-bottom:1.5rem">
          <div>
            <label style="display:block;font-size:.8rem;color:#64748b;margin-bottom:.2rem">Médico</label>
            <p-select
              [(ngModel)]="indFilterMedico"
              [options]="medicos()"
              optionLabel="nombre"
              optionValue="id"
              placeholder="Todos"
              [showClear]="true"
              styleClass="p-inputtext-sm"
            />
          </div>
          <div>
            <label style="display:block;font-size:.8rem;color:#64748b;margin-bottom:.2rem">Desde</label>
            <p-datepicker [(ngModel)]="indDesde" dateFormat="yy-mm-dd" inputStyleClass="p-inputtext-sm" />
          </div>
          <div>
            <label style="display:block;font-size:.8rem;color:#64748b;margin-bottom:.2rem">Hasta</label>
            <p-datepicker [(ngModel)]="indHasta" dateFormat="yy-mm-dd" inputStyleClass="p-inputtext-sm" />
          </div>
          <p-button label="Consultar" icon="pi pi-refresh" size="small" (onClick)="loadIndicadores()" [loading]="loadingInd()" />
        </div>

        @if (indicadores()) {
          <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(200px,1fr));gap:1rem;margin-bottom:1.5rem">
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
          <div style="display:grid;grid-template-columns:repeat(auto-fill,minmax(240px,1fr));gap:1rem;margin-bottom:1.5rem">
            <div class="ind-card"
                 [class.ind-ok]="indicadores().oportunidad_dias_promedio !== null && indicadores().oportunidad_dias_promedio <= indicadores().meta_medicina_general_dias"
                 [class.ind-warn]="indicadores().oportunidad_dias_promedio !== null && indicadores().oportunidad_dias_promedio > indicadores().meta_medicina_general_dias">
              <div style="font-size:1.5rem;font-weight:700">
                {{ indicadores().oportunidad_dias_promedio ?? 'sin datos' }}
                @if (indicadores().oportunidad_dias_promedio !== null) { <span style="font-size:.9rem"> días</span> }
              </div>
              <div style="font-weight:600">Oportunidad promedio</div>
              <div style="font-size:.75rem;color:#64748b;margin-top:.25rem">
                Medida sobre {{ indicadores().oportunidad_medidas }} de
                {{ indicadores().oportunidad_medidas + indicadores().oportunidad_sin_dato }} citas
                &middot; Meta: ≤ {{ indicadores().meta_medicina_general_dias }} días (general),
                ≤ {{ indicadores().meta_especialista_dias }} (especialista)
              </div>
            </div>
            @if (indicadores().oportunidad_sin_dato > 0) {
              <div class="ind-card ind-warn">
                <div style="font-size:1.5rem;font-weight:700">{{ indicadores().oportunidad_sin_dato }}</div>
                <div style="font-weight:600">Sin fecha de solicitud</div>
                <div style="font-size:.75rem;color:#64748b;margin-top:.25rem">
                  No cuentan para la oportunidad. Registre la fecha en que el
                  paciente PIDIÓ la cita para que entren.
                </div>
              </div>
            }
            @if (indicadores().oportunidad_incoherentes > 0) {
              <div class="ind-card ind-warn">
                <div style="font-size:1.5rem;font-weight:700">{{ indicadores().oportunidad_incoherentes }}</div>
                <div style="font-weight:600">Con solicitud posterior a la cita</div>
                <div style="font-size:.75rem;color:#64748b;margin-top:.25rem">
                  Imposible: nadie pide una cita después de que ocurra. No se
                  promedian —darían días negativos que MEJORAN el indicador— y
                  hay que corregir la fecha en esas citas.
                </div>
              </div>
            }
          </div>

          <div style="display:grid;grid-template-columns:1fr 1fr;gap:1rem">
            <div class="ind-card" [class.ind-ok]="indicadores().tasa_inasistencia_pct <= 20" [class.ind-warn]="indicadores().tasa_inasistencia_pct > 20">
              <div style="font-size:1.5rem;font-weight:700">{{ indicadores().tasa_inasistencia_pct }}%</div>
              <div style="font-weight:600">Tasa de Inasistencia</div>
              <div style="font-size:.75rem;color:#64748b;margin-top:.25rem">Meta: ≤ 20%</div>
            </div>
            <div class="ind-card" [class.ind-ok]="indicadores().tasa_cancelacion_pct <= 10" [class.ind-warn]="indicadores().tasa_cancelacion_pct > 10">
              <div style="font-size:1.5rem;font-weight:700">{{ indicadores().tasa_cancelacion_pct }}%</div>
              <div style="font-weight:600">Tasa de Cancelación</div>
              <div style="font-size:.75rem;color:#64748b;margin-top:.25rem">Meta: ≤ 10%</div>
            </div>
          </div>

          <div style="margin-top:1rem;padding:.75rem;background:#eff6ff;border-radius:.5rem;font-size:.8rem;color:#1e40af">
            <strong>Normativa aplicada:</strong> {{ indicadores().normativa }}
            (Decreto 780/2016, Ley 1438/2011)
          </div>
        } @else if (!loadingInd()) {
          <p style="color:#64748b;text-align:center;padding:2rem">
            Seleccione el período y presione "Consultar" para ver los indicadores
          </p>
        }
      </p-tabpanel>
    </p-tabpanels>
  </p-tabs>
</div>

<!-- Dialog: Nueva/Editar Agenda -->
<p-dialog [(visible)]="showAgendaDialog" [header]="editingAgenda ? 'Editar Agenda' : 'Nueva Agenda Mensual'"
  [modal]="true" [style]="{width:'700px'}">

  <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:1rem;margin-bottom:1.25rem">
    @if (!editingAgenda) {
      <div>
        <label class="form-label">Médico *</label>
        <p-select
          [(ngModel)]="agendaForm.medico_id"
          [options]="medicos()"
          optionLabel="nombre"
          optionValue="id"
          placeholder="Seleccionar"
          [style]="{width:'100%'}"
        />
      </div>
      <div>
        <label class="form-label">Año *</label>
        <p-select [(ngModel)]="agendaForm.anio" [options]="anioOpts" [style]="{width:'100%'}" />
      </div>
      <div>
        <label class="form-label">Mes *</label>
        <p-select
          [(ngModel)]="agendaForm.mes"
          [options]="mesOpts"
          optionLabel="label"
          optionValue="value"
          placeholder="Seleccionar"
          [style]="{width:'100%'}"
        />
      </div>
    }
  </div>

  <div style="margin-bottom:.75rem">
    <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:.5rem">
      <strong style="font-size:.9rem">Bloques de Atención</strong>
      <p-button label="Agregar bloque" icon="pi pi-plus" size="small" [text]="true" (onClick)="addBloqueForm()" />
    </div>
    <p style="font-size:.75rem;color:#64748b;margin:.25rem 0 .75rem">
      Duración mínima por consulta: medicina general ≥20 min · especialista ≥30 min (Circular 049/2008 Supersalud)
    </p>

    @for (bloque of agendaForm.bloques; track $index; let i = $index) {
      <div style="display:grid;grid-template-columns:2fr 1fr 1fr auto;gap:.5rem;align-items:center;margin-bottom:.5rem;padding:.5rem;background:#f8fafc;border-radius:.375rem">
        <p-select
          [(ngModel)]="bloque.dia_semana"
          [options]="diasOpts"
          optionLabel="label"
          optionValue="value"
          placeholder="Día"
          [style]="{width:'100%'}"
        />
        <input pInputText type="time" [(ngModel)]="bloque.hora_inicio" style="width:100%" />
        <input pInputText type="time" [(ngModel)]="bloque.hora_fin" style="width:100%" />
        <p-button icon="pi pi-trash" severity="danger" [text]="true" size="small" (onClick)="removeBloqueForm(i)" />
      </div>
    }

    @if (agendaForm.bloques.length === 0) {
      <p style="color:#9ca3af;font-size:.875rem;text-align:center;padding:.75rem">
        No hay bloques. Agrega al menos uno para la semana laboral.
      </p>
    }
  </div>

  <div>
    <label class="form-label">Observaciones</label>
    <input pInputText [(ngModel)]="agendaForm.observaciones" placeholder="Notas opcionales" style="width:100%" />
  </div>

  <ng-template pTemplate="footer">
    <p-button label="Cancelar" severity="secondary" [text]="true" (onClick)="showAgendaDialog=false" />
    <p-button label="Guardar" icon="pi pi-check" (onClick)="saveAgenda()" [loading]="savingAgenda()" />
  </ng-template>
</p-dialog>

<!-- Dialog: Nuevo Bloqueo -->
<p-dialog [(visible)]="showBloqueoDialog" header="Nuevo Bloqueo de Agenda"
  [modal]="true" [style]="{width:'500px'}">

  <div style="display:grid;gap:1rem;padding:.5rem 0">
    <div>
      <label class="form-label">Médico *</label>
      <p-select
        [(ngModel)]="bloqueoForm.medico_id"
        [options]="medicos()"
        optionLabel="nombre"
        optionValue="id"
        placeholder="Seleccionar médico"
        [style]="{width:'100%'}"
      />
    </div>
    <div style="display:grid;grid-template-columns:1fr 1fr;gap:.75rem">
      <div>
        <label class="form-label">Fecha inicio *</label>
        <p-datepicker
          [(ngModel)]="bloqueoForm.fecha_inicio"
          [showTime]="true"
          hourFormat="24"
          dateFormat="yy-mm-dd"
          [style]="{width:'100%'}"
        />
      </div>
      <div>
        <label class="form-label">Fecha fin *</label>
        <p-datepicker
          [(ngModel)]="bloqueoForm.fecha_fin"
          [showTime]="true"
          hourFormat="24"
          dateFormat="yy-mm-dd"
          [style]="{width:'100%'}"
        />
      </div>
    </div>
    <div>
      <label class="form-label">Tipo *</label>
      <p-select
        [(ngModel)]="bloqueoForm.tipo"
        [options]="tiposBloqueo"
        placeholder="Tipo de bloqueo"
        [style]="{width:'100%'}"
      />
    </div>
    <div>
      <label class="form-label">Motivo *</label>
      <input pInputText [(ngModel)]="bloqueoForm.motivo" placeholder="Descripción del bloqueo" style="width:100%" />
    </div>
  </div>

  <ng-template pTemplate="footer">
    <p-button label="Cancelar" severity="secondary" [text]="true" (onClick)="showBloqueoDialog=false" />
    <p-button label="Guardar" icon="pi pi-check" (onClick)="saveBloqueo()" [loading]="savingBloqueo()" />
  </ng-template>
</p-dialog>
  `,
  styles: [`
    .form-label { display:block;font-size:.875rem;font-weight:500;color:#374151;margin-bottom:.25rem }
    .kpi-card {
      background:#f8fafc;border:1px solid #e2e8f0;border-radius:.5rem;
      padding:1rem;text-align:center;
    }
    .kpi-value { font-size:2rem;font-weight:700;color:#1e293b;line-height:1 }
    .kpi-label { font-size:.8rem;color:#64748b;margin-top:.25rem }
    .ind-card {
      padding:1.25rem;border-radius:.5rem;border:1px solid #e2e8f0;
    }
    .ind-ok { background:#f0fdf4;border-color:#86efac }
    .ind-warn { background:#fff7ed;border-color:#fdba74 }
  `],
})
export class AgendaMedicaComponent implements OnInit {
  private svc = inject(CitaService);
  private msg = inject(Avisos);

  activeTab = 'agendas';
  medicos = signal<MedicoDisponible[]>([]);
  agendas = signal<any[]>([]);
  bloqueos = signal<any[]>([]);
  indicadores = signal<any>(null);

  loadingAgendas = signal(false);
  loadingBloqueos = signal(false);
  loadingInd = signal(false);
  savingAgenda = signal(false);
  savingBloqueo = signal(false);

  // Filters
  filterMedico: number | null = null;
  filterAnio: number = new Date().getFullYear();
  filterMes: number | null = null;
  indFilterMedico: number | null = null;
  indDesde: Date = new Date(new Date().getFullYear(), new Date().getMonth(), 1);
  indHasta: Date = new Date();

  // Dialogs
  showAgendaDialog = false;
  showBloqueoDialog = false;
  editingAgenda: any = null;

  agendaForm: any = { medico_id: null, anio: new Date().getFullYear(), mes: null, observaciones: '', bloques: [] };
  bloqueoForm: any = { medico_id: null, fecha_inicio: null, fecha_fin: null, tipo: 'Otro', motivo: '' };

  anioOpts = [new Date().getFullYear(), new Date().getFullYear() + 1];
  mesOpts = MESES.map((m, i) => ({ label: m, value: i + 1 }));
  diasOpts = DIAS.map((d, i) => ({ label: d, value: i }));
  tiposBloqueo = Object.keys(TIPOS_BLOQUEO);

  ngOnInit(): void {
    this.svc.getMedicos().subscribe(m => this.medicos.set(m));
    this.loadAgendas();
    this.loadBloqueos();
  }

  onTabChange(tab: string | number | undefined): void {
    if (tab === undefined || typeof tab === 'number') return;
    this.activeTab = tab;
    if (tab === 'bloqueos') this.loadBloqueos();
  }

  mesNombre(mes: number): string {
    return MESES[mes - 1] ?? '';
  }

  loadAgendas(): void {
    this.loadingAgendas.set(true);
    this.svc.getAgendas(
      this.filterMedico ?? undefined,
      this.filterAnio,
      this.filterMes ?? undefined
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
    const desde = this.toDateStr(this.indDesde);
    const hasta = this.toDateStr(this.indHasta);
    this.svc.getIndicadoresOportunidad(
      this.indFilterMedico ?? undefined,
      desde,
      hasta
    ).subscribe({
      next: d => { this.indicadores.set(d); this.loadingInd.set(false); },
      error: () => this.loadingInd.set(false),
    });
  }

  /** El binario manda `anio`, sin eñe: una columna con tilde en SQLite es una
   *  fuente de líos de codificación y el Python la tenía por descuido, no por
   *  decisión. El acceso por corchetes era el rodeo que hacía falta con `año`. */
  getAnio(a: any): number { return a.anio; }

  tipoEnPantalla(t: string): string { return TIPO_BLOQUEO_A_PANTALLA[t] ?? t; }

  openNewAgenda(): void {
    this.editingAgenda = null;
    this.agendaForm = {
      medico_id: null,
      anio: new Date().getFullYear(),
      mes: null,
      observaciones: '',
      bloques: [],
    };
    this.showAgendaDialog = true;
  }

  editAgenda(agenda: any): void {
    this.editingAgenda = agenda;
    this.agendaForm = {
      medico_id: agenda.medico_id,
      anio: agenda.anio,
      mes: agenda.mes,
      observaciones: agenda.observaciones ?? '',
      bloques: agenda.bloques.map((b: any) => ({
        id: b.id,
        dia_semana: b.dia_semana,
        hora_inicio: b.hora_inicio,
        hora_fin: b.hora_fin,
        activo: b.activo,
      })),
    };
    this.showAgendaDialog = true;
  }

  addBloqueForm(): void {
    this.agendaForm.bloques.push({ dia_semana: 0, hora_inicio: '08:00', hora_fin: '12:00', activo: true });
  }

  removeBloqueForm(idx: number): void {
    this.agendaForm.bloques.splice(idx, 1);
  }

  saveAgenda(): void {
    this.savingAgenda.set(true);
    if (this.editingAgenda) {
      // Update: toggle active + update bloques individually
      this.svc.updateAgenda(this.editingAgenda.id, { observaciones: this.agendaForm.observaciones }).subscribe({
        next: () => {
          this.savingAgenda.set(false);
          this.showAgendaDialog = false;
          this.msg.add({ severity: 'success', summary: 'Agenda actualizada' });
          this.loadAgendas();
        },
        error: () => this.savingAgenda.set(false),
      });
    } else {
      this.svc.createAgenda(this.agendaForm).subscribe({
        next: () => {
          this.savingAgenda.set(false);
          this.showAgendaDialog = false;
          this.msg.add({ severity: 'success', summary: 'Agenda creada' });
          this.loadAgendas();
        },
        error: (err) => {
          this.savingAgenda.set(false);
          const detail = err?.error?.detail ?? 'No se pudo crear la agenda';
          this.msg.add({ severity: 'error', summary: 'Error', detail });
        },
      });
    }
  }

  deleteAgenda(id: number): void {
    this.svc.deleteAgenda(id).subscribe({
      next: () => {
        this.msg.add({ severity: 'info', summary: 'Agenda eliminada' });
        this.loadAgendas();
      },
    });
  }

  openNewBloqueo(): void {
    this.bloqueoForm = { medico_id: null, fecha_inicio: null, fecha_fin: null, tipo: 'Otro', motivo: '' };
    this.showBloqueoDialog = true;
  }

  saveBloqueo(): void {
    this.savingBloqueo.set(true);
    const payload = {
      medico_id: this.bloqueoForm.medico_id,
      motivo: this.bloqueoForm.motivo,
      tipo: TIPOS_BLOQUEO[this.bloqueoForm.tipo] ?? 'OTRO',
      inicio: aInstante(this.bloqueoForm.fecha_inicio),
      fin: aInstante(this.bloqueoForm.fecha_fin),
    };
    this.svc.createBloqueo(payload).subscribe({
      next: (r) => {
        this.savingBloqueo.set(false);
        this.showBloqueoDialog = false;
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
      error: () => this.savingBloqueo.set(false),
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

  private toDateStr(d: Date): string {
    return d.toISOString().slice(0, 10);
  }
}
