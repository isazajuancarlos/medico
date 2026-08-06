import { Component, OnInit, inject, signal, computed } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatTableModule } from '@angular/material/table';
import { MatTooltipModule } from '@angular/material/tooltip';
import { ChartOptions } from 'chart.js';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { GraficoComponent } from '../../shared/ui/grafico.component';
import { Avisos } from '../../shared/ui/avisos.service';
import {
  EstadisticasService, Resumen, EstadisticasCitas, EstadisticasPacientes,
  EstadisticasDiagnosticos, EstadisticasProduccion, EstadisticasFinanciero,
  EstadisticasUrgencias,
} from '../../services/estadisticas.service';

type Tab = 'resumen' | 'citas' | 'pacientes' | 'diagnosticos' | 'produccion' | 'financiero' | 'urgencias';

const CHART_COLORS = [
  '#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6',
  '#06b6d4', '#f97316', '#84cc16', '#ec4899', '#6366f1',
];
const TRIAGE_COLORS: Record<string, string> = {
  'Rojo (I)': '#ef4444', 'Naranja (II)': '#f97316',
  'Amarillo (III)': '#eab308', 'Verde (IV)': '#22c55e', 'Azul (V)': '#3b82f6',
};

@Component({
  selector: 'app-estadisticas',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatFormFieldModule, MatInputModule, MatTableModule,
    MatTooltipModule, EtiquetaComponent, GraficoComponent,
  ],
  template: `
    <div class="page-card">

      <div class="list-header">
        <h2 class="page-title">Estadísticas e Indicadores</h2>
        <div style="display:flex;gap:.5rem;align-items:center">
          <!-- Estos dos campos llevaban las clases p-inputtext y p-component a
               mano: eran las clases de PrimeNG puestas sobre un input crudo, no
               su directiva. Se habrían quedado sin estilo el día que salga la
               hoja de PrimeNG, sin que ningún import lo delatara. -->
          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Desde</mat-label>
            <input matInput type="date" [(ngModel)]="fechaInicio" />
          </mat-form-field>
          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Hasta</mat-label>
            <input matInput type="date" [(ngModel)]="fechaFin" />
          </mat-form-field>
          <button matButton="filled" (click)="aplicarFiltro()">
            <i class="pi pi-filter"></i> Aplicar
          </button>
          <button matButton (click)="setEsteMes()">Este mes</button>
          <button matButton (click)="setEsteAnio()">Este año</button>
        </div>
      </div>

      <!-- Tabs -->
      <div class="tab-bar" style="margin-bottom:1.5rem">
        @for (t of tabs; track t.key) {
          <button class="tab-btn" [class.active]="tab() === t.key" (click)="setTab(t.key)">
            <i [class]="t.icon" style="margin-right:.4rem"></i>{{ t.label }}
          </button>
        }
      </div>

      <!-- ════════ RESUMEN ════════ -->
      @if (tab() === 'resumen') {
        @if (resumen()) {
          <div class="kpi-grid">
            <div class="kpi-card blue">
              <div class="kpi-icon"><i class="pi pi-users"></i></div>
              <div><div class="kpi-val">{{ resumen()!.total_pacientes | number }}</div><div class="kpi-lbl">Pacientes registrados</div></div>
            </div>
            <div class="kpi-card green">
              <div class="kpi-icon"><i class="pi pi-calendar"></i></div>
              <div><div class="kpi-val">{{ resumen()!.citas_hoy }}</div><div class="kpi-lbl">Citas hoy</div></div>
            </div>
            <div class="kpi-card purple">
              <div class="kpi-icon"><i class="pi pi-calendar-plus"></i></div>
              <div><div class="kpi-val">{{ resumen()!.citas_mes }}</div><div class="kpi-lbl">Citas este mes</div></div>
            </div>
            <div class="kpi-card teal">
              <div class="kpi-icon"><i class="pi pi-file-edit"></i></div>
              <div><div class="kpi-val">{{ resumen()!.consultas_mes }}</div><div class="kpi-lbl">Consultas este mes</div></div>
            </div>
            <div class="kpi-card orange">
              <div class="kpi-icon"><i class="pi pi-bolt"></i></div>
              <div><div class="kpi-val">{{ resumen()!.urgencias_activas }}</div><div class="kpi-lbl">Urgencias activas</div></div>
            </div>
            <div class="kpi-card red">
              <div class="kpi-icon"><i class="pi pi-home"></i></div>
              <div><div class="kpi-val">{{ resumen()!.hospitalizados }}</div><div class="kpi-lbl">Hospitalizados</div></div>
            </div>
            <div class="kpi-card gray">
              <div class="kpi-icon"><i class="pi pi-th-large"></i></div>
              <div>
                <div class="kpi-val">{{ resumen()!.camas_ocupadas }}/{{ resumen()!.total_camas }}</div>
                <div class="kpi-lbl">Ocupación camas ({{ resumen()!.tasa_ocupacion }}%)</div>
              </div>
            </div>
            <div class="kpi-card yellow">
              <div class="kpi-icon"><i class="pi pi-check-circle"></i></div>
              <div><div class="kpi-val">{{ resumen()!.tasa_asistencia }}%</div><div class="kpi-lbl">Tasa asistencia</div></div>
            </div>
            <div class="kpi-card indigo">
              <div class="kpi-icon"><i class="pi pi-dollar"></i></div>
              <div>
                <div class="kpi-val">{{ resumen()!.valor_pendiente | currency:'COP':'symbol-narrow':'1.0-0' }}</div>
                <div class="kpi-lbl">Valor pendiente cobro ({{ resumen()!.facturas_pendientes }} facturas)</div>
              </div>
            </div>
          </div>

          <!-- Ocupación camas barra -->
          <div class="chart-row" style="margin-top:1.5rem">
            <div class="chart-box" style="flex:1">
              <h4 class="chart-title">Ocupación de Camas</h4>
              <div style="height:1rem;background:#e2e8f0;border-radius:8px;overflow:hidden;margin:1rem 0">
                <div style="height:100%;background:#ef4444;border-radius:8px;transition:width .3s"
                  [style.width.%]="resumen()!.tasa_ocupacion"></div>
              </div>
              <div style="display:flex;justify-content:space-between;font-size:.8rem;color:#64748b">
                <span>Disponibles: {{ resumen()!.camas_disponibles }}</span>
                <span>{{ resumen()!.tasa_ocupacion }}% ocupadas</span>
                <span>Ocupadas: {{ resumen()!.camas_ocupadas }}</span>
              </div>
            </div>
          </div>
        } @else {
          <div class="loading-state"><i class="pi pi-spin pi-spinner"></i> Cargando resumen...</div>
        }
      }

      <!-- ════════ CITAS ════════ -->
      @if (tab() === 'citas') {
        @if (citas()) {
          <div class="stat-header">
            Total citas en período: <strong>{{ citas()!.total }}</strong>
          </div>
          <div class="chart-row">
            <div class="chart-box">
              <h4 class="chart-title">Por Estado</h4>
              <x-grafico type="doughnut" [data]="citasEstadoChart()" [options]="doughnutOpts" height="260px" />
            </div>
            <div class="chart-box">
              <h4 class="chart-title">Por Tipo</h4>
              <x-grafico type="doughnut" [data]="citasTipoChart()" [options]="doughnutOpts" height="260px" />
            </div>
            <div class="chart-box" style="flex:2">
              <h4 class="chart-title">Tendencia</h4>
              <x-grafico type="bar" [data]="citasTendenciaChart()" [options]="barOpts" height="260px" />
            </div>
          </div>
          <div class="chart-row" style="margin-top:1rem">
            <div class="chart-box" style="flex:1">
              <h4 class="chart-title">Producción por Médico</h4>
              <x-grafico type="bar" [data]="citasMedicoChart()" [options]="hbarOpts" height="300px" />
            </div>
          </div>
        } @else {
          <div class="loading-state"><i class="pi pi-spin pi-spinner"></i> Cargando estadísticas de citas...</div>
        }
      }

      <!-- ════════ PACIENTES ════════ -->
      @if (tab() === 'pacientes') {
        @if (pacientes()) {
          <div class="stat-header">Total pacientes: <strong>{{ pacientes()!.total | number }}</strong></div>
          <div class="chart-row">
            <div class="chart-box">
              <h4 class="chart-title">Por Sexo</h4>
              <x-grafico type="pie" [data]="pacSexoChart()" [options]="doughnutOpts" height="260px" />
            </div>
            <div class="chart-box">
              <h4 class="chart-title">Por Régimen</h4>
              <x-grafico type="doughnut" [data]="pacTipoChart()" [options]="doughnutOpts" height="260px" />
            </div>
            <div class="chart-box">
              <h4 class="chart-title">Por Zona</h4>
              <x-grafico type="pie" [data]="pacZonaChart()" [options]="doughnutOpts" height="260px" />
            </div>
          </div>
          <div class="chart-row" style="margin-top:1rem">
            <div class="chart-box" style="flex:1">
              <h4 class="chart-title">Pirámide de Edad</h4>
              <x-grafico type="bar" [data]="pacEdadChart()" [options]="barOpts" height="260px" />
            </div>
            <div class="chart-box" style="flex:1">
              <h4 class="chart-title">Top EPS</h4>
              <x-grafico type="bar" [data]="pacEpsChart()" [options]="hbarOpts" height="260px" />
            </div>
          </div>
        } @else {
          <div class="loading-state"><i class="pi pi-spin pi-spinner"></i> Cargando estadísticas de pacientes...</div>
        }
      }

      <!-- ════════ DIAGNÓSTICOS ════════ -->
      @if (tab() === 'diagnosticos') {
        @if (diagnosticos()) {
          <div class="chart-row">
            <div class="chart-box" style="flex:1">
              <h4 class="chart-title">Top 10 Diagnósticos (CIE-10)</h4>
              <x-grafico type="bar" [data]="dxChart()" [options]="hbarOpts" height="420px" />
            </div>
            <div class="chart-box" style="flex:1">
              <h4 class="chart-title">Detalle</h4>
              <!-- El scroll va en un div: mat-table no trae el
                   [scrollable]/scrollHeight que tenía p-table. -->
              <div class="con-scroll" style="max-height:380px">
                <table mat-table [dataSource]="diagnosticos()!.diagnosticos">
                  <ng-container matColumnDef="codigo">
                    <th mat-header-cell *matHeaderCellDef style="width:90px">Código</th>
                    <td mat-cell *matCellDef="let dx"><span class="badge-code">{{ dx.codigo }}</span></td>
                  </ng-container>
                  <ng-container matColumnDef="descripcion">
                    <th mat-header-cell *matHeaderCellDef>Descripción</th>
                    <td mat-cell *matCellDef="let dx" class="menudo">{{ dx.descripcion }}</td>
                  </ng-container>
                  <ng-container matColumnDef="casos">
                    <th mat-header-cell *matHeaderCellDef class="derecha" style="width:80px">Casos</th>
                    <td mat-cell *matCellDef="let dx" class="derecha fuerte">{{ dx.cantidad }}</td>
                  </ng-container>
                  <tr mat-header-row *matHeaderRowDef="colDx; sticky: true"></tr>
                  <tr mat-row *matRowDef="let fila; columns: colDx"></tr>
                </table>
              </div>
            </div>
          </div>
        } @else {
          <div class="loading-state"><i class="pi pi-spin pi-spinner"></i> Cargando diagnósticos...</div>
        }
      }

      <!-- ════════ PRODUCCIÓN ════════ -->
      @if (tab() === 'produccion') {
        @if (produccion()) {
          <div class="chart-row">
            <div class="chart-box" style="flex:1">
              <h4 class="chart-title">Citas Atendidas por Médico</h4>
              <x-grafico type="bar" [data]="prodChart()" [options]="hbarOpts" height="360px" />
            </div>
            <div class="chart-box" style="flex:1.2">
              <h4 class="chart-title">Detalle de Producción</h4>
              <div class="con-scroll" style="max-height:340px">
                <table mat-table [dataSource]="produccion()!.medicos">
                  <ng-container matColumnDef="medico">
                    <th mat-header-cell *matHeaderCellDef>Médico</th>
                    <td mat-cell *matCellDef="let m">{{ m.nombre }}</td>
                  </ng-container>
                  <ng-container matColumnDef="especialidad">
                    <th mat-header-cell *matHeaderCellDef style="width:120px">Especialidad</th>
                    <td mat-cell *matCellDef="let m" class="tenue">{{ m.especialidad || '—' }}</td>
                  </ng-container>
                  <ng-container matColumnDef="citas">
                    <th mat-header-cell *matHeaderCellDef class="derecha" style="width:90px">Citas</th>
                    <td mat-cell *matCellDef="let m" class="derecha fuerte azul">{{ m.citas_atendidas }}</td>
                  </ng-container>
                  <ng-container matColumnDef="historias">
                    <th mat-header-cell *matHeaderCellDef class="derecha" style="width:90px">Historias</th>
                    <td mat-cell *matCellDef="let m" class="derecha fuerte verde">{{ m.historias_creadas }}</td>
                  </ng-container>
                  <tr mat-header-row *matHeaderRowDef="colProd; sticky: true"></tr>
                  <tr mat-row *matRowDef="let fila; columns: colProd"></tr>
                </table>
              </div>
            </div>
          </div>
        } @else {
          <div class="loading-state"><i class="pi pi-spin pi-spinner"></i> Cargando producción...</div>
        }
      }

      <!-- ════════ FINANCIERO ════════ -->
      @if (tab() === 'financiero') {
        @if (financiero()) {
          <div class="stat-header">
            Total facturado en período: <strong>{{ financiero()!.total_valor | currency:'COP':'symbol-narrow':'1.0-0' }}</strong>
            &nbsp;|&nbsp; Valor glosado pendiente: <strong style="color:#ef4444">{{ financiero()!.valor_glosado_pendiente | currency:'COP':'symbol-narrow':'1.0-0' }}</strong>
          </div>
          <div class="chart-row">
            <div class="chart-box">
              <h4 class="chart-title">Facturas por Estado</h4>
              <x-grafico type="doughnut" [data]="finEstadoChart()" [options]="doughnutOpts" height="260px" />
            </div>
            <div class="chart-box" style="flex:2">
              <h4 class="chart-title">Tendencia Mensual (valor)</h4>
              <x-grafico type="line" [data]="finTendenciaChart()" [options]="lineOpts" height="260px" />
            </div>
          </div>
          <div class="chart-row" style="margin-top:1rem">
            <div class="chart-box" style="flex:1">
              <h4 class="chart-title">Glosas por Estado</h4>
              <table mat-table [dataSource]="financiero()!.glosas">
                <ng-container matColumnDef="estado">
                  <th mat-header-cell *matHeaderCellDef>Estado</th>
                  <td mat-cell *matCellDef="let g">
                    <x-etiqueta [value]="g.estado" [severity]="severidadGlosa(g.estado)" />
                  </td>
                </ng-container>
                <ng-container matColumnDef="cantidad">
                  <th mat-header-cell *matHeaderCellDef class="derecha">Cantidad</th>
                  <td mat-cell *matCellDef="let g" class="derecha">{{ g.cantidad }}</td>
                </ng-container>
                <ng-container matColumnDef="valor">
                  <th mat-header-cell *matHeaderCellDef class="derecha">Valor</th>
                  <td mat-cell *matCellDef="let g" class="derecha fuerte">
                    {{ g.valor | currency:'COP':'symbol-narrow':'1.0-0' }}
                  </td>
                </ng-container>
                <tr mat-header-row *matHeaderRowDef="colGlosas"></tr>
                <tr mat-row *matRowDef="let fila; columns: colGlosas"></tr>
              </table>
            </div>
            <div class="chart-box" style="flex:2">
              <h4 class="chart-title">Detalle por Estado</h4>
              <table mat-table [dataSource]="financiero()!.by_estado">
                <ng-container matColumnDef="estado">
                  <th mat-header-cell *matHeaderCellDef>Estado</th>
                  <td mat-cell *matCellDef="let e">
                    <x-etiqueta [value]="e.estado" [severity]="estadoSev(e.estado)" />
                  </td>
                </ng-container>
                <ng-container matColumnDef="facturas">
                  <th mat-header-cell *matHeaderCellDef class="derecha">Facturas</th>
                  <td mat-cell *matCellDef="let e" class="derecha">{{ e.cantidad }}</td>
                </ng-container>
                <ng-container matColumnDef="valor">
                  <th mat-header-cell *matHeaderCellDef class="derecha">Valor Total</th>
                  <td mat-cell *matCellDef="let e" class="derecha fuerte">
                    {{ e.valor | currency:'COP':'symbol-narrow':'1.0-0' }}
                  </td>
                </ng-container>
                <tr mat-header-row *matHeaderRowDef="colFacturas"></tr>
                <tr mat-row *matRowDef="let fila; columns: colFacturas"></tr>
              </table>
            </div>
          </div>
        } @else {
          <div class="loading-state"><i class="pi pi-spin pi-spinner"></i> Cargando financiero...</div>
        }
      }

      <!-- ════════ URGENCIAS ════════ -->
      @if (tab() === 'urgencias') {
        @if (urgencias()) {
          <div class="stat-header">
            Total atenciones: <strong>{{ urgencias()!.total }}</strong>
            &nbsp;|&nbsp; Estancia promedio: <strong>{{ urgencias()!.estancia_promedio_min }} min</strong>
          </div>
          <div class="chart-row">
            <div class="chart-box" style="flex:1">
              <h4 class="chart-title">Distribución por Triage</h4>
              <x-grafico type="doughnut" [data]="triageChart()" [options]="doughnutOpts" height="300px" />
            </div>
            <div class="chart-box" style="flex:1">
              <h4 class="chart-title">Destino de Salida</h4>
              <x-grafico type="bar" [data]="destinoChart()" [options]="barOpts" height="300px" />
            </div>
            <div class="chart-box" style="flex:1">
              <h4 class="chart-title">Triage — Semáforo</h4>
              @for (entry of triageEntries(); track entry.label) {
                <div class="triage-row">
                  <span class="triage-dot" [style.background]="entry.color"></span>
                  <span style="flex:1;font-size:.88rem">{{ entry.label }}</span>
                  <span style="font-weight:700;font-size:1.1rem">{{ entry.value }}</span>
                  <div class="triage-bar-bg">
                    <div class="triage-bar-fill" [style.width.%]="entry.pct" [style.background]="entry.color"></div>
                  </div>
                </div>
              }
            </div>
          </div>
        } @else {
          <div class="loading-state"><i class="pi pi-spin pi-spinner"></i> Cargando urgencias...</div>
        }
      }

    </div>
  `,
  styles: [`
    table { width:100% }
    .con-scroll { overflow:auto }
    .derecha { text-align:right }
    .fuerte { font-weight:600 }
    .menudo { font-size:.85rem }
    .tenue { font-size:.82rem; color:#64748b }
    .azul { color:#3b82f6 }
    .verde { color:#10b981 }

    .list-header { display:flex; justify-content:space-between; align-items:center; margin-bottom:1.5rem; flex-wrap:wrap; gap:.5rem }
    .page-title { font-size:1.4rem; font-weight:700; color:#1e293b; margin:0 }

    .tab-bar { display:flex; gap:.25rem; border-bottom:2px solid #e2e8f0; overflow-x:auto }
    .tab-btn { background:none; border:none; padding:.6rem 1rem; cursor:pointer; font-size:.88rem; color:#64748b; border-bottom:2px solid transparent; margin-bottom:-2px; white-space:nowrap; transition:all .15s }
    .tab-btn.active { color:#3b82f6; border-bottom-color:#3b82f6; font-weight:600 }
    .tab-btn:hover:not(.active) { color:#1e293b; background:#f8fafc }

    .kpi-grid { display:grid; grid-template-columns:repeat(auto-fill,minmax(220px,1fr)); gap:1rem }
    .kpi-card { background:white; border-radius:12px; padding:1.2rem; box-shadow:0 1px 3px rgba(0,0,0,.08); display:flex; align-items:center; gap:1rem; border-left:4px solid }
    .kpi-card.blue { border-color:#3b82f6 } .kpi-card.green { border-color:#10b981 }
    .kpi-card.purple { border-color:#8b5cf6 } .kpi-card.teal { border-color:#06b6d4 }
    .kpi-card.orange { border-color:#f97316 } .kpi-card.red { border-color:#ef4444 }
    .kpi-card.gray { border-color:#94a3b8 } .kpi-card.yellow { border-color:#eab308 }
    .kpi-card.indigo { border-color:#6366f1 }
    .kpi-icon { width:44px;height:44px;border-radius:10px;display:flex;align-items:center;justify-content:center;font-size:1.2rem;background:#f1f5f9;color:#475569;flex-shrink:0 }
    .kpi-val { font-size:1.5rem; font-weight:700; color:#1e293b }
    .kpi-lbl { font-size:.75rem; color:#64748b; margin-top:.1rem }

    .stat-header { color:#475569; margin-bottom:1rem; font-size:.9rem }

    .chart-row { display:flex; gap:1rem; flex-wrap:wrap }
    .chart-box { background:white; border-radius:12px; padding:1.2rem; box-shadow:0 1px 3px rgba(0,0,0,.08); flex:1; min-width:220px }
    .chart-title { font-size:.9rem; font-weight:600; color:#374151; margin:0 0 .75rem }

    .badge-code { background:#eff6ff; color:#1d4ed8; padding:.15rem .4rem; border-radius:4px; font-size:.8rem; font-weight:600 }

    .triage-row { display:flex; align-items:center; gap:.75rem; margin:.6rem 0 }
    .triage-dot { width:12px;height:12px;border-radius:50%;flex-shrink:0 }
    .triage-bar-bg { flex:1;height:8px;background:#e2e8f0;border-radius:4px;overflow:hidden }
    .triage-bar-fill { height:100%;border-radius:4px;transition:width .3s }

    .loading-state { text-align:center; padding:3rem; color:#94a3b8; font-size:1rem }
  `],
})
export class EstadisticasComponent implements OnInit {
  private svc = inject(EstadisticasService);

  tab = signal<Tab>('resumen');

  resumen = signal<Resumen | null>(null);
  citas = signal<EstadisticasCitas | null>(null);
  pacientes = signal<EstadisticasPacientes | null>(null);
  diagnosticos = signal<EstadisticasDiagnosticos | null>(null);
  produccion = signal<EstadisticasProduccion | null>(null);
  financiero = signal<EstadisticasFinanciero | null>(null);
  urgencias = signal<EstadisticasUrgencias | null>(null);

  fechaInicio = '';
  fechaFin = '';

  tabs = [
    { key: 'resumen' as Tab, label: 'Resumen', icon: 'pi pi-home' },
    { key: 'citas' as Tab, label: 'Citas', icon: 'pi pi-calendar' },
    { key: 'pacientes' as Tab, label: 'Pacientes', icon: 'pi pi-users' },
    { key: 'diagnosticos' as Tab, label: 'Diagnósticos', icon: 'pi pi-heart' },
    { key: 'produccion' as Tab, label: 'Producción', icon: 'pi pi-chart-bar' },
    { key: 'financiero' as Tab, label: 'Financiero', icon: 'pi pi-dollar' },
    { key: 'urgencias' as Tab, label: 'Urgencias', icon: 'pi pi-bolt' },
  ];

  // Chart options
  readonly colDx = ['codigo', 'descripcion', 'casos'];
  readonly colProd = ['medico', 'especialidad', 'citas', 'historias'];
  readonly colGlosas = ['estado', 'cantidad', 'valor'];
  readonly colFacturas = ['estado', 'facturas', 'valor'];

  // La anotación de tipo es la que hace que se comprueben: p-chart recibía any,
  // así que un position: 'abajo' habría compilado y chart.js lo habría ignorado
  // en silencio, dejando la leyenda donde no era. Y va aquí, en la declaración,
  // no en el input del componente: ese es polimórfico y no puede cerrarse.
  readonly doughnutOpts: ChartOptions = {
    plugins: { legend: { position: 'bottom' } },
    responsive: true, maintainAspectRatio: false,
  };
  readonly barOpts: ChartOptions = {
    plugins: { legend: { display: false } },
    responsive: true, maintainAspectRatio: false,
    scales: { x: { grid: { display: false } } },
  };
  readonly hbarOpts: ChartOptions<'bar'> = {
    indexAxis: 'y',
    plugins: { legend: { display: false } },
    responsive: true, maintainAspectRatio: false,
    scales: { x: { grid: { color: '#f1f5f9' } } },
  };
  readonly lineOpts: ChartOptions<'line'> = {
    plugins: { legend: { display: false } },
    responsive: true, maintainAspectRatio: false,
    scales: { x: { grid: { display: false } }, y: { grid: { color: '#f1f5f9' } } },
    elements: { line: { tension: 0.4 } },
  };

  ngOnInit(): void {
    this.setEsteMes();
    this.cargarTodo();
  }

  setEsteMes(): void {
    const hoy = new Date();
    this.fechaInicio = new Date(hoy.getFullYear(), hoy.getMonth(), 1).toISOString().slice(0, 10);
    this.fechaFin = hoy.toISOString().slice(0, 10);
  }

  setEsteAnio(): void {
    const hoy = new Date();
    this.fechaInicio = `${hoy.getFullYear()}-01-01`;
    this.fechaFin = hoy.toISOString().slice(0, 10);
    this.aplicarFiltro();
  }

  aplicarFiltro(): void {
    this.citas.set(null);
    this.diagnosticos.set(null);
    this.produccion.set(null);
    this.financiero.set(null);
    this.urgencias.set(null);
    this.cargarConFiltro();
  }

  setTab(t: Tab): void {
    this.tab.set(t);
    if (t === 'resumen' && !this.resumen()) this.svc.getResumen().subscribe(r => this.resumen.set(r));
    if (t === 'citas' && !this.citas()) this.svc.getCitas(this.fechaInicio, this.fechaFin).subscribe(r => this.citas.set(r));
    if (t === 'pacientes' && !this.pacientes()) this.svc.getPacientes().subscribe(r => this.pacientes.set(r));
    if (t === 'diagnosticos' && !this.diagnosticos()) this.svc.getDiagnosticos(this.fechaInicio, this.fechaFin).subscribe(r => this.diagnosticos.set(r));
    if (t === 'produccion' && !this.produccion()) this.svc.getProduccion(this.fechaInicio, this.fechaFin).subscribe(r => this.produccion.set(r));
    if (t === 'financiero' && !this.financiero()) this.svc.getFinanciero(this.fechaInicio, this.fechaFin).subscribe(r => this.financiero.set(r));
    if (t === 'urgencias' && !this.urgencias()) this.svc.getUrgencias(this.fechaInicio, this.fechaFin).subscribe(r => this.urgencias.set(r));
  }

  private cargarTodo(): void {
    this.svc.getResumen().subscribe(r => this.resumen.set(r));
    this.svc.getCitas(this.fechaInicio, this.fechaFin).subscribe(r => this.citas.set(r));
    this.svc.getPacientes().subscribe(r => this.pacientes.set(r));
    this.svc.getDiagnosticos(this.fechaInicio, this.fechaFin).subscribe(r => this.diagnosticos.set(r));
    this.svc.getProduccion(this.fechaInicio, this.fechaFin).subscribe(r => this.produccion.set(r));
    this.svc.getFinanciero(this.fechaInicio, this.fechaFin).subscribe(r => this.financiero.set(r));
    this.svc.getUrgencias(this.fechaInicio, this.fechaFin).subscribe(r => this.urgencias.set(r));
  }

  private cargarConFiltro(): void {
    this.svc.getCitas(this.fechaInicio, this.fechaFin).subscribe(r => this.citas.set(r));
    this.svc.getDiagnosticos(this.fechaInicio, this.fechaFin).subscribe(r => this.diagnosticos.set(r));
    this.svc.getProduccion(this.fechaInicio, this.fechaFin).subscribe(r => this.produccion.set(r));
    this.svc.getFinanciero(this.fechaInicio, this.fechaFin).subscribe(r => this.financiero.set(r));
    this.svc.getUrgencias(this.fechaInicio, this.fechaFin).subscribe(r => this.urgencias.set(r));
  }

  // ── Chart computed signals ──────────────────────────────────────────────

  citasEstadoChart = computed(() => {
    const data = this.citas()?.by_estado ?? {};
    return {
      labels: Object.keys(data),
      datasets: [{ data: Object.values(data), backgroundColor: CHART_COLORS }],
    };
  });

  citasTipoChart = computed(() => {
    const data = this.citas()?.by_tipo ?? {};
    return {
      labels: Object.keys(data),
      datasets: [{ data: Object.values(data), backgroundColor: CHART_COLORS }],
    };
  });

  citasTendenciaChart = computed(() => {
    const trend = this.citas()?.tendencia ?? [];
    return {
      labels: trend.map(t => t.fecha),
      datasets: [{
        label: 'Citas',
        data: trend.map(t => t.cantidad),
        backgroundColor: '#3b82f6',
      }],
    };
  });

  citasMedicoChart = computed(() => {
    const data = this.citas()?.by_medico ?? [];
    return {
      labels: data.map(m => m.medico),
      datasets: [{
        data: data.map(m => m.cantidad),
        backgroundColor: CHART_COLORS,
      }],
    };
  });

  pacSexoChart = computed(() => {
    const d = this.pacientes()?.by_sexo ?? {};
    const labels = Object.keys(d).map(k => k === 'M' ? 'Masculino' : k === 'F' ? 'Femenino' : k);
    return { labels, datasets: [{ data: Object.values(d), backgroundColor: ['#3b82f6', '#ec4899', '#94a3b8'] }] };
  });

  pacTipoChart = computed(() => {
    const d = this.pacientes()?.by_tipo_usuario ?? {};
    return { labels: Object.keys(d), datasets: [{ data: Object.values(d), backgroundColor: CHART_COLORS }] };
  });

  pacZonaChart = computed(() => {
    const d = this.pacientes()?.by_zona ?? {};
    return { labels: Object.keys(d), datasets: [{ data: Object.values(d), backgroundColor: ['#10b981', '#f59e0b'] }] };
  });

  pacEdadChart = computed(() => {
    const d = this.pacientes()?.by_grupo_edad ?? {};
    return {
      labels: Object.keys(d),
      datasets: [{ label: 'Pacientes', data: Object.values(d), backgroundColor: '#8b5cf6' }],
    };
  });

  pacEpsChart = computed(() => {
    const d = this.pacientes()?.by_eps ?? [];
    return {
      labels: d.map(e => e.eps),
      datasets: [{ data: d.map(e => e.cantidad), backgroundColor: CHART_COLORS }],
    };
  });

  dxChart = computed(() => {
    const dx = this.diagnosticos()?.diagnosticos ?? [];
    return {
      labels: dx.map(d => `${d.codigo} — ${d.descripcion.slice(0, 40)}${d.descripcion.length > 40 ? '…' : ''}`),
      datasets: [{ data: dx.map(d => d.cantidad), backgroundColor: '#ef4444' }],
    };
  });

  prodChart = computed(() => {
    const m = this.produccion()?.medicos ?? [];
    return {
      labels: m.map(x => x.nombre),
      datasets: [
        { label: 'Citas atendidas', data: m.map(x => x.citas_atendidas), backgroundColor: '#3b82f6' },
        { label: 'Historias', data: m.map(x => x.historias_creadas), backgroundColor: '#10b981' },
      ],
    };
  });

  finEstadoChart = computed(() => {
    const d = this.financiero()?.by_estado ?? [];
    const estadoColors: Record<string, string> = {
      BORRADOR: '#94a3b8', RADICADA: '#3b82f6', ACEPTADA: '#10b981',
      PAGADA: '#22c55e', GLOSADA: '#f97316', ANULADA: '#ef4444',
    };
    return {
      labels: d.map(e => e.estado),
      datasets: [{ data: d.map(e => e.cantidad), backgroundColor: d.map(e => estadoColors[e.estado] ?? '#94a3b8') }],
    };
  });

  finTendenciaChart = computed(() => {
    const t = this.financiero()?.tendencia_mensual ?? [];
    return {
      labels: t.map(x => x.periodo),
      datasets: [{
        label: 'Valor facturado',
        data: t.map(x => x.valor),
        borderColor: '#3b82f6',
        backgroundColor: 'rgba(59,130,246,.1)',
        fill: true,
      }],
    };
  });

  triageChart = computed(() => {
    const d = this.urgencias()?.by_triage ?? {};
    return {
      labels: Object.keys(d),
      datasets: [{
        data: Object.values(d),
        backgroundColor: Object.keys(d).map(k => TRIAGE_COLORS[k] ?? '#94a3b8'),
      }],
    };
  });

  destinoChart = computed(() => {
    const d = this.urgencias()?.by_destino ?? {};
    return {
      labels: Object.keys(d),
      datasets: [{ label: 'Pacientes', data: Object.values(d), backgroundColor: '#06b6d4' }],
    };
  });

  triageEntries = computed(() => {
    const d = this.urgencias()?.by_triage ?? {};
    const total = Object.values(d).reduce((a, b) => a + b, 0) || 1;
    return Object.entries(d).map(([label, value]) => ({
      label, value,
      color: TRIAGE_COLORS[label] ?? '#94a3b8',
      pct: Math.round(value / total * 100),
    }));
  });

  estadoSev(estado: string): 'secondary' | 'info' | 'success' | 'warn' | 'danger' {
    const m: Record<string, 'secondary' | 'info' | 'success' | 'warn' | 'danger'> = {
      BORRADOR: 'secondary', RADICADA: 'info', ACEPTADA: 'success',
      PAGADA: 'success', GLOSADA: 'warn', ANULADA: 'danger',
    };
    return m[estado] ?? 'secondary';
  }

  /**
   * Una glosa ACEPTADA se pinta en rojo, y no es una errata: aceptarla
   * significa que el prestador da por buena la objeción y **no cobra** ese
   * valor. Estaba escrito como una cadena de ternarios dentro de la plantilla.
   */
  severidadGlosa(estado: string): 'warn' | 'danger' | 'success' {
    if (estado === 'PENDIENTE') return 'warn';
    if (estado === 'ACEPTADA') return 'danger';
    return 'success';
  }
}
