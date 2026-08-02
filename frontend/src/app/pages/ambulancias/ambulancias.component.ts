import { Component, Inject, OnInit, computed, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MAT_DIALOG_DATA, MatDialog, MatDialogModule } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatTableModule } from '@angular/material/table';
import { MatTooltipModule } from '@angular/material/tooltip';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { Avisos } from '../../shared/ui/avisos.service';
import {
  AmbulanciasService, Ambulancia, AmbulanciaCreate, Traslado, TrasladoCreate,
} from '../../services/ambulancias.service';
import { PacienteService } from '../../services/paciente.service';

type Tab = 'parque' | 'traslados';

const TIPOS = [
  { label: 'TAB — básico', value: 'TAB' },
  { label: 'TAM — medicalizado', value: 'TAM' },
];

/** Alta y edición de una ambulancia. Devuelve los campos, o nada. */
@Component({
  selector: 'app-ambulancia-form',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatSelectModule,
  ],
  template: `
    <h2 mat-dialog-title>{{ editando ? 'Editar ambulancia' : 'Nueva ambulancia' }}</h2>
    <mat-dialog-content>
      <div class="rejilla">
        <mat-form-field appearance="outline">
          <mat-label>Placa *</mat-label>
          <input matInput [(ngModel)]="f.placa" placeholder="WXY123" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Tipo *</mat-label>
          <mat-select [(ngModel)]="f.tipo">
            @for (t of tipos; track t.value) {
              <mat-option [value]="t.value">{{ t.label }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Número interno</mat-label>
          <input matInput [(ngModel)]="f.numero_interno" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Marca</mat-label>
          <input matInput [(ngModel)]="f.marca" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Modelo</mat-label>
          <input matInput [(ngModel)]="f.modelo" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Vence SOAT</mat-label>
          <input matInput type="date" [(ngModel)]="f.vence_soat" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Vence tecnomecánica</mat-label>
          <input matInput type="date" [(ngModel)]="f.vence_tecnomecanica" />
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Observaciones</mat-label>
          <textarea matInput [(ngModel)]="f.observaciones" rows="2"></textarea>
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!f.placa" [mat-dialog-close]="resultado()">
        <i class="pi pi-check"></i> {{ editando ? 'Guardar' : 'Crear' }}
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .rejilla {
      display:grid; grid-template-columns:1fr 1fr; gap:.875rem; padding-top:.5rem;
      min-width:min(34rem, 80vw);
    }
    .ancho { grid-column:1/-1; }
    mat-form-field { width:100%; }
  `],
})
export class AmbulanciaFormComponent {
  readonly tipos = TIPOS;
  readonly editando: boolean;
  f: Partial<AmbulanciaCreate>;

  constructor(@Inject(MAT_DIALOG_DATA) a: Ambulancia | null) {
    this.editando = !!a;
    this.f = a
      ? {
          placa: a.placa, tipo: a.tipo, numero_interno: a.numero_interno,
          marca: a.marca, modelo: a.modelo, vence_soat: a.vence_soat,
          vence_tecnomecanica: a.vence_tecnomecanica, observaciones: a.observaciones,
        }
      : { tipo: 'TAB' };
  }

  resultado(): Partial<AmbulanciaCreate> | null {
    return this.f.placa ? this.f : null;
  }
}

/**
 * Despacho de una ambulancia.
 *
 * El selector ofrece SOLO las que el servidor dejaría salir ahora mismo —la
 * lista la calcula la pantalla y entra por MAT_DIALOG_DATA—, porque hacer que
 * el usuario descubra la regla chocándose con un 409 es enseñársela dos veces.
 * Los pacientes los pide el propio diálogo: es su campo, no el de la pantalla.
 */
@Component({
  selector: 'app-ambulancia-despacho',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatSelectModule,
  ],
  template: `
    <h2 mat-dialog-title>Despachar ambulancia</h2>
    <mat-dialog-content>
      <div class="rejilla">
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Ambulancia *</mat-label>
          <mat-select [(ngModel)]="f.ambulancia_id"
            placeholder="Solo se ofrecen las que pueden salir ahora">
            @for (a of ambulancias; track a.value) {
              <mat-option [value]="a.value">{{ a.label }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Paciente *</mat-label>
          <mat-select [(ngModel)]="f.paciente_id">
            @for (p of pacientes(); track p.value) {
              <mat-option [value]="p.value">{{ p.label }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Tipo de traslado *</mat-label>
          <mat-select [(ngModel)]="f.tipo_traslado">
            @for (t of tiposTraslado; track t.value) {
              <mat-option [value]="t.value">{{ t.label }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Conductor</mat-label>
          <input matInput [(ngModel)]="f.conductor" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Origen *</mat-label>
          <input matInput [(ngModel)]="f.origen" placeholder="Vía Las Palmas km 12" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Destino *</mat-label>
          <input matInput [(ngModel)]="f.destino" placeholder="Urgencias sede norte" />
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Motivo *</mat-label>
          <input matInput [(ngModel)]="f.motivo" placeholder="Accidente de tránsito" />
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Personal asistencial</mat-label>
          <input matInput [(ngModel)]="f.personal_asistencial" />
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" class="exito" [disabled]="!completo()"
        [mat-dialog-close]="resultado()">
        <i class="pi pi-send"></i> Despachar
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .rejilla {
      display:grid; grid-template-columns:1fr 1fr; gap:.875rem; padding-top:.5rem;
      min-width:min(34rem, 80vw);
    }
    .ancho { grid-column:1/-1; }
    mat-form-field { width:100%; }
    .exito { background:#16a34a; color:#fff; }
  `],
})
export class AmbulanciaDespachoComponent {
  private pacienteSvc = inject(PacienteService);

  readonly tiposTraslado = [
    { label: 'Primario — desde el sitio del evento', value: 'Primario' },
    { label: 'Secundario — entre prestadores', value: 'Secundario' },
  ];

  readonly ambulancias: { label: string; value: number }[];
  readonly pacientes = signal<{ label: string; value: number }[]>([]);

  f: Partial<TrasladoCreate> = { tipo_traslado: 'Primario' };

  constructor(@Inject(MAT_DIALOG_DATA) datos: { ambulancias: { label: string; value: number }[] }) {
    this.ambulancias = datos.ambulancias;
    this.pacienteSvc.list(1, 500).subscribe({
      next: r => this.pacientes.set(r.items.map(p => ({
        label: `${p.nombre1} ${p.apellido1} — ${p.identificacion}`,
        value: p.id,
      }))),
    });
  }

  completo(): boolean {
    const f = this.f;
    return !!(f.ambulancia_id && f.paciente_id && f.origen && f.destino && f.motivo);
  }

  resultado(): TrasladoCreate | null {
    return this.completo() ? (this.f as TrasladoCreate) : null;
  }
}

/**
 * Transporte asistencial.
 *
 * La pantalla NO decide cuándo una ambulancia puede salir: lo decide el
 * servidor, y aquí se muestra el motivo que devuelve. Lo que sí hace es no
 * ofrecer lo que se sabe que va a fallar —una ambulancia ocupada, con el SOAT
 * vencido o dada de baja no aparece en el selector de despacho—, porque hacer
 * que el usuario descubra la regla chocándose con un 409 es enseñársela dos
 * veces.
 *
 * El SOAT tiene TRES estados y no dos: vigente, vencido y **sin registrar**. El
 * tercero no se pinta como los otros dos ni se calla: no saber no es estar al
 * día, y tampoco es motivo para no despachar una urgencia.
 */
@Component({
  selector: 'app-ambulancias',
  standalone: true,
  imports: [
    CommonModule, FormsModule, MatButtonModule, MatFormFieldModule, MatInputModule,
    MatSelectModule, MatTableModule, MatTooltipModule, EtiquetaComponent,
  ],
  template: `
    <div class="page-card">
      <div class="list-header">
        <h2 class="page-title">Ambulancias y traslados</h2>
        <div style="display:flex;gap:0.5rem">
          <button matButton="filled" class="exito" [disabled]="disponibles().length === 0"
            (click)="abrirDespacho()">
            <i class="pi pi-send"></i> Despachar
          </button>
          @if (tab() === 'parque') {
            <button matButton="filled" (click)="abrirCrear()">
              <i class="pi pi-plus"></i> Nueva ambulancia
            </button>
          }
        </div>
      </div>

      <div class="stats-row">
        <div class="stat-card stat-green">
          <div class="stat-value">{{ disponibles().length }}</div>
          <div class="stat-label">Disponibles ahora</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{{ enCurso().length }}</div>
          <div class="stat-label">Traslados en curso</div>
        </div>
        <div class="stat-card stat-red">
          <div class="stat-value">{{ conSoatVencido().length }}</div>
          <div class="stat-label">SOAT vencido</div>
        </div>
        <div class="stat-card stat-yellow">
          <div class="stat-value">{{ sinSoatRegistrado().length }}</div>
          <div class="stat-label">SOAT sin registrar</div>
        </div>
      </div>

      @if (conSoatVencido().length || sinSoatRegistrado().length) {
        <div class="aviso">
          <i class="pi pi-exclamation-triangle"></i>
          <span>
            @if (conSoatVencido().length) {
              <strong>{{ conSoatVencido().length }}</strong> ambulancia(s) no pueden circular: SOAT vencido.
            }
            @if (sinSoatRegistrado().length) {
              <strong>{{ sinSoatRegistrado().length }}</strong> sin SOAT registrado — el sistema no
              puede afirmar que estén al día, y por eso no lo afirma.
            }
          </span>
        </div>
      }

      <div class="tab-bar">
        <button class="tab-btn" [class.active]="tab() === 'parque'" (click)="tab.set('parque')">
          <i class="pi pi-truck"></i> Parque
        </button>
        <button class="tab-btn" [class.active]="tab() === 'traslados'" (click)="cambiarATraslados()">
          <i class="pi pi-map-marker"></i> Traslados
          @if (enCurso().length) { <span class="badge-count">{{ enCurso().length }}</span> }
        </button>
      </div>

      @if (tab() === 'parque') {
        <div class="filter-bar">
          <mat-form-field appearance="outline" subscriptSizing="dynamic" class="search-input">
            <mat-label>Buscar</mat-label>
            <input matInput [(ngModel)]="busq" placeholder="Placa o número interno..." (input)="cargar()" />
          </mat-form-field>
          <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:180px">
            <mat-label>Estado</mat-label>
            <mat-select [(ngModel)]="filtroEstado" (selectionChange)="cargar()">
              <mat-option [value]="null">Todos</mat-option>
              @for (o of opcionesEstado; track o.value) {
                <mat-option [value]="o.value">{{ o.label }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
        </div>

        @if (cargando()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (ambulancias().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-truck" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay ambulancias registradas.</p>
          </div>
        } @else {
          <table mat-table [dataSource]="ambulancias()" class="tabla">
            <ng-container matColumnDef="placa">
              <th mat-header-cell *matHeaderCellDef>Placa</th>
              <td mat-cell *matCellDef="let a" [style.opacity]="a.activa ? 1 : 0.55">
                <strong>{{ a.placa }}</strong>
                @if (a.numero_interno) {
                  <div style="font-size:0.75rem;color:#94a3b8">{{ a.numero_interno }}</div>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="tipo">
              <th mat-header-cell *matHeaderCellDef class="centro">Tipo</th>
              <td mat-cell *matCellDef="let a" class="centro" [style.opacity]="a.activa ? 1 : 0.55">
                <x-etiqueta [value]="a.tipo" [severity]="a.tipo === 'TAM' ? 'warn' : 'info'"
                  [matTooltip]="a.tipo === 'TAM' ? 'Transporte asistencial medicalizado' : 'Transporte asistencial básico'" />
              </td>
            </ng-container>
            <ng-container matColumnDef="vehiculo">
              <th mat-header-cell *matHeaderCellDef>Vehículo</th>
              <td mat-cell *matCellDef="let a" style="font-size:0.82rem;color:#94a3b8"
                [style.opacity]="a.activa ? 1 : 0.55">
                {{ a.marca || '—' }} {{ a.modelo || '' }}
              </td>
            </ng-container>
            <ng-container matColumnDef="estado">
              <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
              <td mat-cell *matCellDef="let a" class="centro" [style.opacity]="a.activa ? 1 : 0.55">
                <x-etiqueta [value]="a.activa ? a.estado : 'Dada de baja'" [severity]="colorEstado(a)" />
                @if (a.traslado_en_curso) {
                  <div style="font-size:0.7rem;color:#64748b">traslado #{{ a.traslado_en_curso }}</div>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="soat">
              <th mat-header-cell *matHeaderCellDef>SOAT</th>
              <td mat-cell *matCellDef="let a" style="font-size:0.82rem"
                [style.opacity]="a.activa ? 1 : 0.55">
                @if (a.soat_sin_registrar) {
                  <span class="soat-desconocido">sin registrar</span>
                } @else if (a.soat_vencido) {
                  <span class="soat-vencido">venció {{ a.vence_soat }}</span>
                } @else if (a.soat_por_vencer) {
                  <span class="soat-pronto">vence {{ a.vence_soat }}</span>
                } @else {
                  <span style="color:#94a3b8">{{ a.vence_soat }}</span>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="acciones">
              <th mat-header-cell *matHeaderCellDef style="width:120px"></th>
              <td mat-cell *matCellDef="let a" style="text-align:right">
                <button matIconButton matTooltip="Editar" (click)="abrirEditar(a)">
                  <i class="pi pi-pencil"></i>
                </button>
                @if (!a.traslado_en_curso) {
                  <button matIconButton [matTooltip]="a.activa ? 'Dar de baja' : 'Reactivar'"
                    [class.peligro]="a.activa" [class.exito-texto]="!a.activa"
                    (click)="alternarBaja(a)">
                    <i [class]="a.activa ? 'pi pi-ban' : 'pi pi-check'"></i>
                  </button>
                }
              </td>
            </ng-container>

            <tr mat-header-row *matHeaderRowDef="columnasParque"></tr>
            <tr mat-row *matRowDef="let fila; columns: columnasParque"></tr>
          </table>
        }
      }

      @if (tab() === 'traslados') {
        @if (cargandoTraslados()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (traslados().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-map-marker" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay traslados registrados.</p>
          </div>
        } @else {
          <table mat-table [dataSource]="traslados()" class="tabla">
            <ng-container matColumnDef="paciente">
              <th mat-header-cell *matHeaderCellDef>Paciente</th>
              <td mat-cell *matCellDef="let t">
                <strong>{{ t.paciente_nombre || '—' }}</strong>
                <div style="font-size:0.75rem;color:#94a3b8">{{ t.paciente_identificacion }}</div>
              </td>
            </ng-container>
            <ng-container matColumnDef="ambulancia">
              <th mat-header-cell *matHeaderCellDef>Ambulancia</th>
              <td mat-cell *matCellDef="let t" style="font-size:0.82rem">
                {{ t.ambulancia_placa }}
                <div style="font-size:0.72rem;color:#64748b">{{ t.ambulancia_tipo }}</div>
              </td>
            </ng-container>
            <ng-container matColumnDef="tipo">
              <th mat-header-cell *matHeaderCellDef class="centro">Tipo</th>
              <td mat-cell *matCellDef="let t" class="centro">
                <x-etiqueta [value]="t.tipo_traslado" severity="secondary" />
              </td>
            </ng-container>
            <ng-container matColumnDef="ruta">
              <th mat-header-cell *matHeaderCellDef>Ruta</th>
              <td mat-cell *matCellDef="let t" style="font-size:0.82rem">
                {{ t.origen }} <i class="pi pi-arrow-right" style="font-size:0.7rem;opacity:0.5"></i> {{ t.destino }}
                <div style="font-size:0.72rem;color:#64748b">{{ t.motivo }}</div>
              </td>
            </ng-container>
            <ng-container matColumnDef="estado">
              <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
              <td mat-cell *matCellDef="let t" class="centro">
                <x-etiqueta [value]="t.estado" [severity]="colorTraslado(t.estado)" />
              </td>
            </ng-container>
            <ng-container matColumnDef="tiempos">
              <th mat-header-cell *matHeaderCellDef>Salida / Llegada</th>
              <td mat-cell *matCellDef="let t" style="font-size:0.78rem">
                {{ t.fecha_salida | date:'dd/MM HH:mm' }}
                <div style="color:#64748b">
                  <!-- Un cancelado no llegó a ninguna parte, y el guion lo dice
                       en vez de dejar el hueco en blanco. -->
                  {{ t.fecha_llegada ? (t.fecha_llegada | date:'dd/MM HH:mm') : '—' }}
                </div>
              </td>
            </ng-container>
            <ng-container matColumnDef="acciones">
              <th mat-header-cell *matHeaderCellDef style="width:120px"></th>
              <td mat-cell *matCellDef="let t" style="text-align:right">
                @if (t.estado === 'En Curso') {
                  <button matIconButton class="exito-texto" matTooltip="Finalizar" (click)="finalizar(t)">
                    <i class="pi pi-check"></i>
                  </button>
                  <button matIconButton class="peligro" matTooltip="Cancelar" (click)="cancelar(t)">
                    <i class="pi pi-times"></i>
                  </button>
                }
              </td>
            </ng-container>

            <tr mat-header-row *matHeaderRowDef="columnasTraslados"></tr>
            <tr mat-row *matRowDef="let fila; columns: columnasTraslados"></tr>
          </table>
        }
      }
    </div>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .page-card { padding: 1.5rem; }
    .page-title { font-size: 1.3rem; font-weight: 700; color: #f1f5f9; margin: 0; }
    .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.25rem; }
    .stats-row { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 0.75rem; margin-bottom: 1rem; }
    .stat-card { background: #1e293b; border: 1px solid #334155; border-radius: 8px; padding: 0.85rem 1rem; }
    .stat-value { font-size: 1.5rem; font-weight: 700; color: #f1f5f9; }
    .stat-label { font-size: 0.72rem; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.03em; }
    .stat-green .stat-value { color: #4ade80; }
    .stat-red .stat-value { color: #f87171; }
    .stat-yellow .stat-value { color: #fbbf24; }
    .aviso { display: flex; gap: 0.6rem; align-items: center; background: rgba(251,191,36,0.08);
      border: 1px solid rgba(251,191,36,0.3); color: #fcd34d; border-radius: 8px;
      padding: 0.65rem 0.9rem; font-size: 0.82rem; margin-bottom: 1rem; }
    .tab-bar { display: flex; gap: 0.25rem; border-bottom: 1px solid #334155; margin-bottom: 1rem; }
    .tab-btn { background: none; border: none; color: #94a3b8; padding: 0.6rem 1rem;
      cursor: pointer; font-size: 0.875rem; border-bottom: 2px solid transparent;
      display: flex; align-items: center; gap: 0.4rem; }
    .tab-btn.active { color: #93c5fd; border-bottom-color: #3b82f6; }
    .badge-count { background: #3b82f6; color: white; border-radius: 10px; padding: 0 0.4rem; font-size: 0.7rem; }
    .filter-bar { display: flex; gap: 0.6rem; margin-bottom: 1rem; flex-wrap: wrap; align-items: flex-start; }
    .search-input { width: 260px; }
    .cargando { padding: 3rem; text-align: center; color: #64748b; }
    .sin-resultados { text-align: center; padding: 3rem 1rem; color: #64748b;
      display: flex; flex-direction: column; align-items: center; gap: 0.75rem; }
    .tabla { width: 100%; }
    .centro { text-align: center; }
    /* En M3 el input color de un botón no pinta nada: el color va por CSS. */
    .peligro { color: #dc2626; }
    .exito-texto { color: #16a34a; }
    .exito { background: #16a34a; color: #fff; }
    /* Los tres estados del SOAT se ven distintos a propósito: «sin registrar» no
       se pinta como «vigente» ni como «vencido», porque no es ninguno de los dos. */
    .soat-vencido { color: #f87171; font-weight: 600; }
    .soat-pronto { color: #fbbf24; }
    .soat-desconocido { color: #94a3b8; font-style: italic; }
  `],
})
export class AmbulanciasComponent implements OnInit {
  private svc = inject(AmbulanciasService);
  private dialogo = inject(MatDialog);
  private toast = inject(Avisos);

  /** El orden de las columnas lo fija esta lista, no la plantilla. */
  readonly columnasParque = ['placa', 'tipo', 'vehiculo', 'estado', 'soat', 'acciones'];
  readonly columnasTraslados = ['paciente', 'ambulancia', 'tipo', 'ruta', 'estado', 'tiempos', 'acciones'];

  tab = signal<Tab>('parque');
  ambulancias = signal<Ambulancia[]>([]);
  traslados = signal<Traslado[]>([]);
  cargando = signal(false);
  cargandoTraslados = signal(false);
  busq = '';
  filtroEstado: string | null = null;

  readonly opcionesEstado = [
    { label: 'Disponible', value: 'Disponible' },
    { label: 'En Traslado', value: 'En Traslado' },
    { label: 'En Mantenimiento', value: 'En Mantenimiento' },
    { label: 'Fuera de Servicio', value: 'Fuera de Servicio' },
  ];

  /** Las que el servidor dejaría salir ahora mismo. Es la misma regla que aplica
   *  `ambulancia::despachar`, escrita aquí para no ofrecer lo que va a fallar. */
  disponibles = computed(() =>
    this.ambulancias().filter(
      a => a.activa && a.estado === 'Disponible' && !a.traslado_en_curso && !a.soat_vencido,
    ),
  );
  conSoatVencido = computed(() => this.ambulancias().filter(a => a.activa && a.soat_vencido));
  sinSoatRegistrado = computed(() => this.ambulancias().filter(a => a.activa && a.soat_sin_registrar));
  enCurso = computed(() => this.traslados().filter(t => t.estado === 'En Curso'));

  ngOnInit(): void {
    this.cargar();
    this.cargarTraslados();
  }

  cargar(): void {
    this.cargando.set(true);
    this.svc.listar(this.busq || undefined, this.filtroEstado ?? undefined).subscribe({
      next: a => { this.ambulancias.set(a); this.cargando.set(false); },
      error: () => this.cargando.set(false),
    });
  }

  cambiarATraslados(): void {
    this.tab.set('traslados');
    this.cargarTraslados();
  }

  cargarTraslados(): void {
    this.cargandoTraslados.set(true);
    this.svc.listarTraslados().subscribe({
      next: t => { this.traslados.set(t); this.cargandoTraslados.set(false); },
      error: () => this.cargandoTraslados.set(false),
    });
  }

  colorEstado(a: Ambulancia): 'success' | 'info' | 'warn' | 'danger' | 'secondary' {
    if (!a.activa) return 'secondary';
    if (a.estado === 'Disponible') return 'success';
    if (a.estado === 'En Traslado') return 'info';
    if (a.estado === 'En Mantenimiento') return 'warn';
    return 'danger';
  }

  colorTraslado(estado: string): 'success' | 'info' | 'danger' | 'secondary' {
    if (estado === 'Finalizado') return 'success';
    if (estado === 'En Curso') return 'info';
    if (estado === 'Cancelado') return 'danger';
    return 'secondary';
  }

  abrirCrear(): void { this.abrirFormulario(null); }
  abrirEditar(a: Ambulancia): void { this.abrirFormulario(a); }

  private abrirFormulario(a: Ambulancia | null): void {
    this.dialogo
      .open(AmbulanciaFormComponent, { data: a, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((datos?: Partial<AmbulanciaCreate>) => { if (datos) this.guardar(a, datos); });
  }

  private guardar(editando: Ambulancia | null, datos: Partial<AmbulanciaCreate>): void {
    const peticion = editando
      ? this.svc.actualizar(editando.id, datos)
      : this.svc.crear(datos as AmbulanciaCreate);
    peticion.subscribe({
      next: () => {
        this.ok(editando ? 'Ambulancia actualizada' : 'Ambulancia registrada', datos.placa!);
        this.cargar();
      },
      error: err => this.err(err?.error?.detail ?? 'No se pudo guardar'),
    });
  }

  alternarBaja(a: Ambulancia): void {
    this.svc.actualizar(a.id, { activa: !a.activa }).subscribe({
      next: () => { this.ok(a.activa ? 'Dada de baja' : 'Reactivada', a.placa); this.cargar(); },
      error: err => this.err(err?.error?.detail ?? 'No se pudo cambiar el estado'),
    });
  }

  abrirDespacho(): void {
    const ambulancias = this.disponibles().map(a => ({
      label: `${a.placa} — ${a.tipo}${a.soat_sin_registrar ? ' (SOAT sin registrar)' : ''}`,
      value: a.id,
    }));
    this.dialogo
      .open(AmbulanciaDespachoComponent, { data: { ambulancias }, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((t?: TrasladoCreate) => { if (t) this.despachar(t); });
  }

  private despachar(datos: TrasladoCreate): void {
    this.svc.despachar(datos).subscribe({
      next: t => {
        this.ok('Ambulancia despachada', `${t.ambulancia_placa} → ${t.destino}`);
        this.cargar();
        this.cargarTraslados();
      },
      error: err => this.err(err?.error?.detail ?? 'No se pudo despachar'),
    });
  }

  finalizar(t: Traslado): void {
    this.svc.finalizar(t.id).subscribe({
      next: () => { this.ok('Traslado finalizado', `${t.ambulancia_placa} disponible`); this.refrescar(); },
      error: err => this.err(err?.error?.detail ?? 'No se pudo finalizar'),
    });
  }

  cancelar(t: Traslado): void {
    this.svc.cancelar(t.id).subscribe({
      next: () => { this.ok('Traslado cancelado', 'La ambulancia queda disponible'); this.refrescar(); },
      error: err => this.err(err?.error?.detail ?? 'No se pudo cancelar'),
    });
  }

  private refrescar(): void {
    this.cargar();
    this.cargarTraslados();
  }

  private ok(summary: string, detail: string): void {
    this.toast.add({ severity: 'success', summary, detail, life: 3000 });
  }

  private err(detail: string): void {
    this.toast.add({ severity: 'error', summary: 'No se pudo', detail, life: 6000 });
  }
}
