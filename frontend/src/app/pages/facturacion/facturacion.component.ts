import { Component, Inject, OnInit, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MAT_DIALOG_DATA, MatDialog, MatDialogModule } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatTableModule } from '@angular/material/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { MatSelectModule } from '@angular/material/select';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatDividerModule } from '@angular/material/divider';
import { Avisos } from '../../shared/ui/avisos.service';
import { diaLocal } from '../../shared/fecha';
import {
  FacturacionService, Factura, FacturaCreate, DetalleCreate, DetalleFactura,
  Glosa, Contrato, ContratoCreate, Entidad, StatsFacturacion,
} from '../../services/facturacion.service';

type Tab = 'facturas' | 'contratos' | 'glosas';
type Vista = 'lista' | 'detalle' | 'nueva';


/** Opción de un selector: lo que se lee y lo que se guarda. */
interface OpcionSelect { label: string; value: number | string }

/** Los manuales tarifarios con los que se contrata en Colombia. */
const TARIFAS = [
  { label: 'CUPS', value: 'CUPS' },
  { label: 'ISS', value: 'ISS' },
  { label: 'SOAT', value: 'SOAT' },
  { label: 'Personalizada', value: 'PERSONALIZADA' },
];

/** Radicación de una factura. Devuelve el radicado, o nada. */
@Component({
  selector: 'app-facturacion-radicar',
  standalone: true,
  imports: [
    CommonModule, FormsModule, MatDialogModule, MatButtonModule,
    MatFormFieldModule, MatInputModule,
  ],
  template: `
    <h2 mat-dialog-title>Radicar factura</h2>
    <mat-dialog-content>
      <div class="ficha">
        <div style="font-weight:600;color:#f1f5f9">{{ f.numero_factura }}</div>
        <div style="font-size:0.82rem;color:#94a3b8">
          {{ f.entidad_nombre }} — {{ f.valor_neto | currency:'COP':'symbol-narrow':'1.0-0' }}
        </div>
      </div>
      <mat-form-field appearance="outline">
        <mat-label>Número de radicado *</mat-label>
        <input matInput [(ngModel)]="numero" placeholder="RAD-2026-XXXXX" />
      </mat-form-field>
      <mat-form-field appearance="outline">
        <mat-label>Fecha de radicación</mat-label>
        <input matInput type="date" [(ngModel)]="fecha" />
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" class="exito" [disabled]="!numero.trim()"
        [mat-dialog-close]="resultado()">
        <i class="pi pi-send"></i> Radicar factura
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    mat-dialog-content { min-width:min(26rem, 85vw); display:flex; flex-direction:column; gap:.5rem; }
    .ficha { margin-bottom:.5rem; padding:.75rem; background:rgba(99,102,241,0.1);
      border-radius:6px; border:1px solid rgba(99,102,241,0.2); }
    mat-form-field { width:100%; }
    .exito { background:#16a34a; color:#fff; }
  `],
})
export class FacturacionRadicarComponent {
  numero = '';
  /** El día LOCAL: ver shared/fecha. */
  fecha = diaLocal(new Date());

  constructor(@Inject(MAT_DIALOG_DATA) public f: Factura) {}

  resultado(): { numero: string; fecha: string } | null {
    return this.numero.trim() ? { numero: this.numero.trim(), fecha: this.fecha } : null;
  }
}

/** Un servicio que se añade a una factura en borrador. */
@Component({
  selector: 'app-facturacion-servicio',
  standalone: true,
  imports: [FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatInputModule],
  template: `
    <h2 mat-dialog-title>Agregar servicio</h2>
    <mat-dialog-content>
      <div class="rejilla">
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Descripción del servicio *</mat-label>
          <input matInput [(ngModel)]="f.servicio"
            placeholder="Ej: Consulta médica general - HC 2026-001" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Código CUPS</mat-label>
          <input matInput [(ngModel)]="f.codigo_cups" placeholder="890201" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Cantidad</mat-label>
          <input matInput type="number" min="1" [(ngModel)]="f.cantidad" />
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Valor unitario (COP) *</mat-label>
          <input matInput type="number" [(ngModel)]="f.valor_unitario" placeholder="0" />
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!completo()" [mat-dialog-close]="resultado()">
        <i class="pi pi-check"></i> Agregar
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .rejilla { display:grid; grid-template-columns:1fr 1fr; gap:.875rem;
      min-width:min(28rem, 85vw); }
    .ancho { grid-column:1/-1; }
    mat-form-field { width:100%; }
  `],
})
export class FacturacionServicioComponent {
  f: Partial<DetalleCreate> = { codigo_cups: '890201', cantidad: 1 };

  completo(): boolean { return !!this.f.servicio && !!this.f.valor_unitario; }

  resultado(): DetalleCreate | null {
    if (!this.completo()) return null;
    return {
      servicio: this.f.servicio!,
      codigo_cups: this.f.codigo_cups || '890201',
      cantidad: Number(this.f.cantidad) || 1,
      valor_unitario: Number(this.f.valor_unitario),
    };
  }
}

/** Una glosa nueva sobre una factura. */
@Component({
  selector: 'app-facturacion-glosa',
  standalone: true,
  imports: [FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatInputModule],
  template: `
    <h2 mat-dialog-title>Registrar glosa</h2>
    <mat-dialog-content>
      <mat-form-field appearance="outline">
        <mat-label>Item glosado *</mat-label>
        <input matInput [(ngModel)]="item" placeholder="Ej: Consulta del 2026-01-05" />
      </mat-form-field>
      <mat-form-field appearance="outline">
        <mat-label>Motivo de la glosa *</mat-label>
        <textarea matInput [(ngModel)]="motivo" rows="2"
          placeholder="Falta autorización, documentación incompleta..."></textarea>
      </mat-form-field>
      <mat-form-field appearance="outline">
        <mat-label>Valor glosado (COP) *</mat-label>
        <input matInput type="number" [(ngModel)]="valor" placeholder="0" />
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" class="aviso" [disabled]="!completo()"
        [mat-dialog-close]="resultado()">
        <i class="pi pi-exclamation-circle"></i> Registrar glosa
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    mat-dialog-content { min-width:min(28rem, 85vw); display:flex; flex-direction:column; gap:.5rem; }
    mat-form-field { width:100%; }
    .aviso { background:#f59e0b; color:#0f172a; }
  `],
})
export class FacturacionGlosaComponent {
  item = '';
  motivo = '';
  valor: number | null = null;

  completo(): boolean { return !!this.item.trim() && !!this.motivo.trim() && !!this.valor; }

  resultado(): { item_glosado: string; motivo: string; valor_glosado: number } | null {
    if (!this.completo()) return null;
    return {
      item_glosado: this.item.trim(),
      motivo: this.motivo.trim(),
      valor_glosado: Number(this.valor),
    };
  }
}

/**
 * Respuesta a una glosa. Devuelve el texto Y si se acepta.
 *
 * Aceptar y rechazar son dos botones distintos y ninguno es el «por defecto»:
 * aceptar una glosa es renunciar a cobrar ese valor, y rechazarla es sostener
 * el cobro ante la EPS. No es una confirmación, es una decisión.
 */
@Component({
  selector: 'app-facturacion-responder-glosa',
  standalone: true,
  imports: [
    CommonModule, FormsModule, MatDialogModule, MatButtonModule,
    MatFormFieldModule, MatInputModule,
  ],
  template: `
    <h2 mat-dialog-title>Responder glosa</h2>
    <mat-dialog-content>
      <div class="ficha">
        <div style="font-weight:600;color:#f1f5f9">{{ g.item_glosado }}</div>
        <div style="font-size:0.82rem;color:#94a3b8">{{ g.motivo }}</div>
        <div style="font-size:0.82rem;color:#f87171;margin-top:0.25rem">
          {{ g.valor_glosado | currency:'COP':'symbol-narrow':'1.0-0' }}
        </div>
      </div>
      <mat-form-field appearance="outline">
        <mat-label>Respuesta / sustentación *</mat-label>
        <textarea matInput [(ngModel)]="texto" rows="4"
          placeholder="Explique por qué acepta o rechaza la glosa..."></textarea>
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" class="exito" [disabled]="!texto.trim()"
        [mat-dialog-close]="resultado(true)">
        <i class="pi pi-check"></i> Aceptar glosa
      </button>
      <button matButton="filled" class="peligro-fondo" [disabled]="!texto.trim()"
        [mat-dialog-close]="resultado(false)">
        <i class="pi pi-times"></i> Rechazar glosa
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    mat-dialog-content { min-width:min(28rem, 85vw); }
    .ficha { margin-bottom:1rem; padding:.75rem; background:rgba(99,102,241,0.1);
      border-radius:6px; border:1px solid rgba(99,102,241,0.2); }
    mat-form-field { width:100%; }
    .exito { background:#16a34a; color:#fff; }
    .peligro-fondo { background:#dc2626; color:#fff; }
  `],
})
export class FacturacionResponderGlosaComponent {
  texto = '';

  constructor(@Inject(MAT_DIALOG_DATA) public g: Glosa) {}

  resultado(aceptar: boolean): { texto: string; aceptar: boolean } | null {
    return this.texto.trim() ? { texto: this.texto.trim(), aceptar } : null;
  }
}

/** Alta y edición de un contrato. */
@Component({
  selector: 'app-facturacion-contrato',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatSelectModule,
  ],
  template: `
    <h2 mat-dialog-title>{{ editando ? 'Editar contrato' : 'Nuevo contrato' }}</h2>
    <mat-dialog-content>
      <div class="rejilla">
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Entidad / EPS *</mat-label>
          <mat-select [(ngModel)]="f.entidad_id">
            @for (o of entidades; track o.value) {
              <mat-option [value]="o.value">{{ o.label }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Nombre del contrato *</mat-label>
          <input matInput [(ngModel)]="f.nombre" placeholder="Ej: Contrato SURA 2026" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Número de contrato *</mat-label>
          <input matInput [(ngModel)]="f.numero_contrato" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Tipo de tarifa</mat-label>
          <mat-select [(ngModel)]="f.tipo_tarifa">
            @for (o of tarifas; track o.value) {
              <mat-option [value]="o.value">{{ o.label }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Fecha inicio</mat-label>
          <input matInput type="date" [(ngModel)]="f.fecha_inicio" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Fecha fin</mat-label>
          <input matInput type="date" [(ngModel)]="f.fecha_fin" />
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>% de incremento sobre tarifa base</mat-label>
          <input matInput type="number" [(ngModel)]="f.porcentaje_incremento" placeholder="0" />
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Observaciones</mat-label>
          <textarea matInput [(ngModel)]="f.observaciones" rows="2"></textarea>
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!completo()" [mat-dialog-close]="resultado()">
        <i class="pi pi-check"></i> {{ editando ? 'Guardar cambios' : 'Crear contrato' }}
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .rejilla { display:grid; grid-template-columns:1fr 1fr; gap:.875rem;
      min-width:min(32rem, 85vw); }
    .ancho { grid-column:1/-1; }
    mat-form-field { width:100%; }
  `],
})
export class FacturacionContratoComponent {
  readonly tarifas = TARIFAS;
  readonly entidades: OpcionSelect[];
  readonly editando: boolean;
  f: Partial<ContratoCreate>;

  constructor(
    @Inject(MAT_DIALOG_DATA) datos: { contrato: Contrato | null; entidades: OpcionSelect[] },
  ) {
    this.entidades = datos.entidades;
    this.editando = !!datos.contrato;
    const c = datos.contrato;
    this.f = c
      ? { entidad_id: c.entidad_id, nombre: c.nombre, numero_contrato: c.numero_contrato,
          fecha_inicio: c.fecha_inicio, fecha_fin: c.fecha_fin, tipo_tarifa: c.tipo_tarifa,
          porcentaje_incremento: c.porcentaje_incremento, observaciones: c.observaciones }
      : { tipo_tarifa: 'CUPS', porcentaje_incremento: 0 };
  }

  completo(): boolean {
    return !!this.f.nombre && !!this.f.numero_contrato && !!this.f.entidad_id;
  }

  resultado(): Partial<ContratoCreate> | null {
    return this.completo() ? this.f : null;
  }
}

@Component({
  selector: 'app-facturacion',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatFormFieldModule, MatInputModule, MatSelectModule,
    MatTableModule, EtiquetaComponent, MatTooltipModule, MatDividerModule,
  ],
  template: `
    <div class="page-card">

      <!-- ══════════════════ VISTA LISTA ══════════════════ -->
      @if (vista() === 'lista') {

        <div class="list-header">
          <h2 class="page-title">Facturación</h2>
          @if (tab() === 'facturas') {
            <button matButton="filled" (click)="abrirNuevaFactura()">
              <i class="pi pi-plus"></i> Nueva factura
            </button>
          }
          @if (tab() === 'contratos') {
            <button matButton="filled" (click)="abrirFormContrato(null)">
              <i class="pi pi-plus"></i> Nuevo contrato
            </button>
          }
        </div>

        <!-- Stats -->
        <div class="stats-row">
          <div class="stat-card">
            <div class="stat-value">{{ stats()?.total_facturas ?? '—' }}</div>
            <div class="stat-label">Total facturas</div>
          </div>
          <div class="stat-card stat-blue">
            <div class="stat-value">{{ stats()?.radicadas ?? '—' }}</div>
            <div class="stat-label">Radicadas</div>
          </div>
          <div class="stat-card stat-green">
            <div class="stat-value">{{ stats()?.pagadas ?? '—' }}</div>
            <div class="stat-label">Pagadas</div>
          </div>
          <div class="stat-card stat-yellow">
            <div class="stat-value">{{ stats()?.glosadas ?? '—' }}</div>
            <div class="stat-label">Glosadas</div>
          </div>
          <div class="stat-card stat-purple">
            <div class="stat-value">{{ (stats()?.valor_total_radicado ?? 0) | currency:'COP':'symbol-narrow':'1.0-0' }}</div>
            <div class="stat-label">Valor radicado</div>
          </div>
          <div class="stat-card stat-green2">
            <div class="stat-value">{{ (stats()?.valor_total_pagado ?? 0) | currency:'COP':'symbol-narrow':'1.0-0' }}</div>
            <div class="stat-label">Valor pagado</div>
          </div>
        </div>

        <!-- Tabs -->
        <div class="tab-bar">
          <button class="tab-btn" [class.active]="tab() === 'facturas'" (click)="cambiarTab('facturas')">
            <i class="pi pi-file-text"></i> Facturas
          </button>
          <button class="tab-btn" [class.active]="tab() === 'contratos'" (click)="cambiarTab('contratos')">
            <i class="pi pi-briefcase"></i> Contratos
          </button>
          <button class="tab-btn" [class.active]="tab() === 'glosas'" (click)="cambiarTab('glosas')">
            <i class="pi pi-exclamation-circle"></i> Glosas
            @if ((stats()?.glosas_pendientes ?? 0) > 0) {
              <span class="badge-count">{{ stats()!.glosas_pendientes }}</span>
            }
          </button>
        </div>

        <!-- ─── TAB FACTURAS ─── -->
        @if (tab() === 'facturas') {
          <div class="filter-bar">
            <mat-form-field appearance="outline" subscriptSizing="dynamic" class="search-input">
              <mat-label>Buscar</mat-label>
              <input matInput [(ngModel)]="busqFactura" placeholder="Buscar por número..."
                (input)="cargarFacturas()" />
            </mat-form-field>
            <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:160px">
              <mat-label>Estado</mat-label>
              <mat-select [(ngModel)]="filtroEstadoFact" (selectionChange)="cargarFacturas()">
                <mat-option [value]="''">Todos</mat-option>
                @for (o of opcionesEstadoFact; track o.value) {
                  <mat-option [value]="o.value">{{ o.label }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
            <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:200px">
              <mat-label>Entidad</mat-label>
              <mat-select [(ngModel)]="filtroEntidad" (selectionChange)="cargarFacturas()">
                <mat-option [value]="null">Todas</mat-option>
                @for (o of entidadesOpts(); track o.value) {
                  <mat-option [value]="o.value">{{ o.label }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
          </div>

          @if (cargandoFact()) {
            <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
          } @else if (facturas().length === 0) {
            <div class="sin-resultados">
              <i class="pi pi-inbox" style="font-size:2rem;opacity:0.4"></i>
              <p>No hay facturas con los filtros seleccionados.</p>
            </div>
          } @else {
            <table mat-table [dataSource]="facturas()" class="tabla">
              <ng-container matColumnDef="numero">
                <th mat-header-cell *matHeaderCellDef>Número</th>
                <td mat-cell *matCellDef="let f">
                  <strong class="num-factura">{{ f.numero_factura }}</strong>
                  @if (f.num_radicado) {
                    <div style="font-size:0.72rem;color:#94a3b8">Rad: {{ f.num_radicado }}</div>
                  }
                </td>
              </ng-container>
              <ng-container matColumnDef="entidad">
                <th mat-header-cell *matHeaderCellDef>Entidad / EPS</th>
                <td mat-cell *matCellDef="let f">
                  <span>{{ f.entidad_nombre || '—' }}</span>
                  @if (f.contrato_nombre) {
                    <div style="font-size:0.75rem;color:#94a3b8">{{ f.contrato_nombre }}</div>
                  }
                </td>
              </ng-container>
              <ng-container matColumnDef="periodo">
                <th mat-header-cell *matHeaderCellDef>Período</th>
                <td mat-cell *matCellDef="let f" style="font-size:0.82rem">
                  {{ f.periodo_inicio | date:'dd/MM/yy' }} – {{ f.periodo_fin | date:'dd/MM/yy' }}
                </td>
              </ng-container>
              <ng-container matColumnDef="fecha">
                <th mat-header-cell *matHeaderCellDef>Fecha</th>
                <td mat-cell *matCellDef="let f" style="font-size:0.82rem">
                  {{ f.fecha_factura | date:'dd/MM/yyyy' }}
                </td>
              </ng-container>
              <ng-container matColumnDef="valor">
                <th mat-header-cell *matHeaderCellDef class="derecha">Valor neto</th>
                <td mat-cell *matCellDef="let f" class="derecha" style="font-weight:600">
                  {{ f.valor_neto | currency:'COP':'symbol-narrow':'1.0-0' }}
                  @if (f.descuentos > 0) {
                    <div style="font-size:0.72rem;color:#94a3b8">
                      -{{ f.descuentos | currency:'COP':'symbol-narrow':'1.0-0' }}
                    </div>
                  }
                </td>
              </ng-container>
              <ng-container matColumnDef="estado">
                <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
                <td mat-cell *matCellDef="let f" class="centro">
                  <x-etiqueta [value]="estadoLabel(f.estado)" [severity]="estadoSeverity(f.estado)" />
                  @if (f.glosada) {
                    <div style="font-size:0.68rem;color:#fbbf24;margin-top:2px">⚠ glosada</div>
                  }
                </td>
              </ng-container>
              <ng-container matColumnDef="items">
                <th mat-header-cell *matHeaderCellDef class="centro">Items</th>
                <td mat-cell *matCellDef="let f" class="centro">
                  <span class="dur-badge">{{ f.total_detalles }}</span>
                </td>
              </ng-container>
              <ng-container matColumnDef="acciones">
                <th mat-header-cell *matHeaderCellDef class="centro">Acciones</th>
                <td mat-cell *matCellDef="let f" class="centro" style="white-space:nowrap">
                  <button matIconButton matTooltip="Ver detalle" (click)="abrirDetalle(f.id)">
                    <i class="pi pi-eye"></i>
                  </button>
                  @if (f.estado === 'BORRADOR') {
                    <button matIconButton class="exito-texto" matTooltip="Radicar" (click)="abrirRadicar(f)">
                      <i class="pi pi-send"></i>
                    </button>
                    <button matIconButton class="peligro" matTooltip="Eliminar" (click)="eliminarFactura(f)">
                      <i class="pi pi-trash"></i>
                    </button>
                  }
                  @if (f.estado === 'RADICADA') {
                    <button matIconButton class="exito-texto" matTooltip="Marcar aceptada"
                      (click)="cambiarEstado(f, 'ACEPTADA')">
                      <i class="pi pi-check"></i>
                    </button>
                  }
                  @if (f.estado === 'ACEPTADA') {
                    <button matIconButton class="exito-texto" matTooltip="Marcar pagada"
                      (click)="cambiarEstado(f, 'PAGADA')">
                      <i class="pi pi-dollar"></i>
                    </button>
                  }
                </td>
              </ng-container>

              <tr mat-header-row *matHeaderRowDef="columnasFacturas"></tr>
              <tr mat-row *matRowDef="let fila; columns: columnasFacturas"></tr>
            </table>
          }
        }

        <!-- ─── TAB CONTRATOS ─── -->
        @if (tab() === 'contratos') {
          @if (cargandoCont()) {
            <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
          } @else if (contratos().length === 0) {
            <div class="sin-resultados">
              <i class="pi pi-inbox" style="font-size:2rem;opacity:0.4"></i>
              <p>No hay contratos registrados.</p>
            </div>
          } @else {
            <table mat-table [dataSource]="contratos()" class="tabla">
              <ng-container matColumnDef="nombre">
                <th mat-header-cell *matHeaderCellDef>Nombre contrato</th>
                <td mat-cell *matCellDef="let c"><strong>{{ c.nombre }}</strong></td>
              </ng-container>
              <ng-container matColumnDef="numero">
                <th mat-header-cell *matHeaderCellDef>N° contrato</th>
                <td mat-cell *matCellDef="let c" style="font-size:0.82rem;color:#94a3b8">
                  {{ c.numero_contrato }}
                </td>
              </ng-container>
              <ng-container matColumnDef="entidad">
                <th mat-header-cell *matHeaderCellDef>Entidad</th>
                <td mat-cell *matCellDef="let c" style="font-size:0.85rem">{{ c.entidad_nombre || '—' }}</td>
              </ng-container>
              <ng-container matColumnDef="tarifa">
                <th mat-header-cell *matHeaderCellDef>Tarifa base</th>
                <td mat-cell *matCellDef="let c">
                  <x-etiqueta [value]="c.tipo_tarifa" severity="secondary" />
                  @if (c.porcentaje_incremento) {
                    <span style="font-size:0.75rem;color:#94a3b8;margin-left:4px">
                      +{{ c.porcentaje_incremento }}%
                    </span>
                  }
                </td>
              </ng-container>
              <ng-container matColumnDef="vigencia">
                <th mat-header-cell *matHeaderCellDef>Vigencia</th>
                <td mat-cell *matCellDef="let c" style="font-size:0.82rem">
                  {{ c.fecha_inicio | date:'dd/MM/yyyy' }} – {{ c.fecha_fin | date:'dd/MM/yyyy' }}
                </td>
              </ng-container>
              <ng-container matColumnDef="estado">
                <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
                <td mat-cell *matCellDef="let c" class="centro">
                  <x-etiqueta [value]="c.activo ? 'Activo' : 'Inactivo'"
                    [severity]="c.activo ? 'success' : 'secondary'" />
                </td>
              </ng-container>
              <ng-container matColumnDef="acciones">
                <th mat-header-cell *matHeaderCellDef class="centro">Acciones</th>
                <td mat-cell *matCellDef="let c" class="centro">
                  <button matIconButton matTooltip="Editar" (click)="abrirFormContrato(c)">
                    <i class="pi pi-pencil"></i>
                  </button>
                  <button matIconButton [class.aviso-color]="c.activo" [class.exito-texto]="!c.activo"
                    [matTooltip]="c.activo ? 'Desactivar' : 'Activar'" (click)="toggleContrato(c)">
                    <i [class]="c.activo ? 'pi pi-ban' : 'pi pi-check'"></i>
                  </button>
                </td>
              </ng-container>

              <tr mat-header-row *matHeaderRowDef="columnasContratos"></tr>
              <tr mat-row *matRowDef="let fila; columns: columnasContratos"></tr>
            </table>
          }
        }

        <!-- ─── TAB GLOSAS ─── -->
        @if (tab() === 'glosas') {
          <div class="filter-bar">
            <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:160px">
              <mat-label>Estado</mat-label>
              <mat-select [(ngModel)]="filtroEstadoGlosa" (selectionChange)="cargarGlosas()">
                <mat-option [value]="''">Todos</mat-option>
                @for (o of opcionesEstadoGlosa; track o.value) {
                  <mat-option [value]="o.value">{{ o.label }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
          </div>

          @if (cargandoGlosa()) {
            <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
          } @else if (glosas().length === 0) {
            <div class="sin-resultados">
              <i class="pi pi-check-circle" style="font-size:2rem;opacity:0.4;color:#4ade80"></i>
              <p>No hay glosas con el filtro seleccionado.</p>
            </div>
          } @else {
            <table mat-table [dataSource]="glosas()" class="tabla">
              <ng-container matColumnDef="factura">
                <th mat-header-cell *matHeaderCellDef>Factura</th>
                <td mat-cell *matCellDef="let g">
                  <strong class="num-factura">{{ g.numero_factura || '—' }}</strong>
                </td>
              </ng-container>
              <ng-container matColumnDef="item">
                <th mat-header-cell *matHeaderCellDef>Item glosado</th>
                <td mat-cell *matCellDef="let g" style="font-size:0.85rem">{{ g.item_glosado }}</td>
              </ng-container>
              <ng-container matColumnDef="motivo">
                <th mat-header-cell *matHeaderCellDef>Motivo</th>
                <td mat-cell *matCellDef="let g" style="font-size:0.82rem">{{ g.motivo }}</td>
              </ng-container>
              <ng-container matColumnDef="valor">
                <th mat-header-cell *matHeaderCellDef class="derecha">Valor</th>
                <td mat-cell *matCellDef="let g" class="derecha" style="font-weight:600;color:#f87171">
                  {{ g.valor_glosado | currency:'COP':'symbol-narrow':'1.0-0' }}
                </td>
              </ng-container>
              <ng-container matColumnDef="fecha">
                <th mat-header-cell *matHeaderCellDef>Fecha</th>
                <td mat-cell *matCellDef="let g" style="font-size:0.82rem">
                  {{ g.fecha_glosa | date:'dd/MM/yyyy' }}
                </td>
              </ng-container>
              <ng-container matColumnDef="estado">
                <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
                <td mat-cell *matCellDef="let g" class="centro">
                  <x-etiqueta [value]="g.estado"
                    [severity]="g.estado === 'PENDIENTE' ? 'warn' : g.estado === 'RECHAZADA' ? 'danger' : 'success'" />
                </td>
              </ng-container>
              <ng-container matColumnDef="accion">
                <th mat-header-cell *matHeaderCellDef class="centro">Acción</th>
                <td mat-cell *matCellDef="let g" class="centro">
                  @if (g.estado === 'PENDIENTE') {
                    <button matIconButton matTooltip="Responder glosa" (click)="abrirResponderGlosa(g)">
                      <i class="pi pi-reply"></i>
                    </button>
                  } @else {
                    <span style="font-size:0.75rem;color:#64748b" [matTooltip]="g.respuesta">
                      {{ g.fecha_respuesta | date:'dd/MM/yy' }}
                    </span>
                  }
                </td>
              </ng-container>

              <tr mat-header-row *matHeaderRowDef="columnasGlosas"></tr>
              <tr mat-row *matRowDef="let fila; columns: columnasGlosas"></tr>
            </table>
          }
        }

      }

      <!-- ══════════════════ VISTA NUEVA FACTURA ══════════════════ -->
      @if (vista() === 'nueva') {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <button matIconButton (click)="volverLista()"><i class="pi pi-arrow-left"></i></button>
          <h2 class="page-title" style="margin:0">Nueva Factura</h2>
        </div>

        <div class="form-card">
          <div class="form-section">
            <div class="section-title">Datos de la factura</div>
            <div class="form-grid">
              <mat-form-field appearance="outline">
                <mat-label>Entidad / EPS *</mat-label>
                <mat-select [(ngModel)]="nFact.entidad_id" (selectionChange)="onEntidadChange()">
                  @for (o of entidadesOpts(); track o.value) {
                    <mat-option [value]="o.value">{{ o.label }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Contrato (opcional)</mat-label>
                <mat-select [(ngModel)]="nFact.contrato_id">
                  <mat-option [value]="null">Sin contrato</mat-option>
                  @for (o of contratosEntidad(); track o.value) {
                    <mat-option [value]="o.value">{{ o.label }}</mat-option>
                  }
                </mat-select>
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Período inicio *</mat-label>
                <input matInput type="date" [(ngModel)]="nFact.periodo_inicio" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Período fin *</mat-label>
                <input matInput type="date" [(ngModel)]="nFact.periodo_fin" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Descuentos (COP)</mat-label>
                <input matInput type="number" [(ngModel)]="nFact.descuentos" placeholder="0" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Observaciones</mat-label>
                <input matInput [(ngModel)]="nFact.observaciones" />
              </mat-form-field>
            </div>
          </div>

          <mat-divider />

          <!-- Líneas de detalle -->
          <div class="form-section">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem">
              <div class="section-title" style="margin-bottom:0">Servicios prestados</div>
              <button matButton="outlined" (click)="agregarLineaTemporal()">
                <i class="pi pi-plus"></i> Agregar servicio
              </button>
            </div>

            @if (lineasTemp().length === 0) {
              <div class="sin-resultados" style="padding:1.5rem">
                <p style="font-size:0.85rem">Agregue los servicios prestados en el período.</p>
              </div>
            } @else {
              <div class="detalles-container">
                <div class="detalle-header-row">
                  <span>Servicio</span>
                  <span>CUPS</span>
                  <span style="text-align:center">Cant.</span>
                  <span style="text-align:right">Valor unit.</span>
                  <span style="text-align:right">Total</span>
                  <span></span>
                </div>
                @for (lin of lineasTemp(); track $index) {
                  <!-- Rejilla densa: los campos van desnudos con el CSS de la
                       página. Envolver siete controles por fila en mat-form-field
                       no cabe en la pantalla. -->
                  <div class="detalle-row">
                    <input [(ngModel)]="lin.servicio" class="linea-input" placeholder="Descripción servicio" />
                    <input [(ngModel)]="lin.codigo_cups" class="linea-input" style="width:100px" placeholder="CUPS" />
                    <input type="number" [(ngModel)]="lin.cantidad" class="linea-input"
                      style="width:70px;text-align:center" min="1" (input)="calcLine(lin)" />
                    <input type="number" [(ngModel)]="lin.valor_unitario" class="linea-input"
                      style="width:130px;text-align:right" (input)="calcLine(lin)" />
                    <span class="total-linea">{{ ((lin.valor_unitario ?? 0) * (lin.cantidad ?? 0)) | currency:'COP':'symbol-narrow':'1.0-0' }}</span>
                    <button matIconButton class="peligro" matTooltip="Quitar la línea"
                      (click)="quitarLinea($index)">
                      <i class="pi pi-times"></i>
                    </button>
                  </div>
                }
                <div class="total-factura">
                  <span>Subtotal:</span>
                  <span>{{ subtotalTemp() | currency:'COP':'symbol-narrow':'1.0-0' }}</span>
                </div>
                <div class="total-factura" style="color:#fbbf24">
                  <span>Descuentos:</span>
                  <span>-{{ (nFact.descuentos || 0) | currency:'COP':'symbol-narrow':'1.0-0' }}</span>
                </div>
                <div class="total-factura total-neto">
                  <span>Total neto:</span>
                  <span>{{ (subtotalTemp() - (nFact.descuentos || 0)) | currency:'COP':'symbol-narrow':'1.0-0' }}</span>
                </div>
              </div>
            }
          </div>

          <div class="step-actions">
            <button matButton (click)="volverLista()">Cancelar</button>
            <button matButton="filled" (click)="crearFactura()"
              [disabled]="guardando() || !nFact.entidad_id || !nFact.periodo_inicio || !nFact.periodo_fin">
              <i [class]="guardando() ? 'pi pi-spin pi-spinner' : 'pi pi-check'"></i> Crear factura
            </button>
          </div>
        </div>
      }

      <!-- ══════════════════ VISTA DETALLE FACTURA ══════════════════ -->
      @if (vista() === 'detalle' && facturaDetalle()) {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <button matIconButton (click)="volverLista()"><i class="pi pi-arrow-left"></i></button>
          <h2 class="page-title" style="margin:0">{{ facturaDetalle()!.numero_factura }}</h2>
          <x-etiqueta [value]="estadoLabel(facturaDetalle()!.estado)"
            [severity]="estadoSeverity(facturaDetalle()!.estado)" />
        </div>

        <div class="form-card">
          <!-- Cabecera -->
          <div class="det-grid4">
            <div><div class="det-label">Entidad</div><div class="det-valor">{{ facturaDetalle()!.entidad_nombre }}</div></div>
            <div>
              <div class="det-label">Contrato</div>
              <div class="det-valor">{{ facturaDetalle()!.contrato_nombre || 'Sin contrato' }}</div>
            </div>
            <div>
              <div class="det-label">Período</div>
              <div class="det-valor">{{ facturaDetalle()!.periodo_inicio | date:'dd/MM/yyyy' }} – {{ facturaDetalle()!.periodo_fin | date:'dd/MM/yyyy' }}</div>
            </div>
            <div>
              <div class="det-label">Fecha factura</div>
              <div class="det-valor">{{ facturaDetalle()!.fecha_factura | date:'dd/MM/yyyy' }}</div>
            </div>
            @if (facturaDetalle()!.num_radicado) {
              <div>
                <div class="det-label">N° radicado</div>
                <div class="det-valor">{{ facturaDetalle()!.num_radicado }}</div>
              </div>
              <div>
                <div class="det-label">Fecha radicado</div>
                <div class="det-valor">{{ facturaDetalle()!.fecha_radicado | date:'dd/MM/yyyy' }}</div>
              </div>
            }
            @if (facturaDetalle()!.observaciones) {
              <div style="grid-column:1/-1">
                <div class="det-label">Observaciones</div>
                <div class="det-valor">{{ facturaDetalle()!.observaciones }}</div>
              </div>
            }
          </div>

          <!-- Valores -->
          <div class="valores-row">
            <div class="valor-card">
              <div class="det-label">Valor bruto</div>
              <div class="valor-num">{{ facturaDetalle()!.valor_bruto | currency:'COP':'symbol-narrow':'1.0-0' }}</div>
            </div>
            <div class="valor-card" style="border-color:rgba(251,191,36,0.2)">
              <div class="det-label">Descuentos</div>
              <div class="valor-num" style="color:#fbbf24">-{{ facturaDetalle()!.descuentos | currency:'COP':'symbol-narrow':'1.0-0' }}</div>
            </div>
            <div class="valor-card" style="border-color:rgba(74,222,128,0.3);background:rgba(74,222,128,0.05)">
              <div class="det-label">Valor neto</div>
              <div class="valor-num" style="color:#4ade80">{{ facturaDetalle()!.valor_neto | currency:'COP':'symbol-narrow':'1.0-0' }}</div>
            </div>
          </div>

          <mat-divider />

          <!-- Detalles -->
          <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:0.75rem">
            <div class="section-title" style="margin-bottom:0">Servicios ({{ facturaDetalle()!.detalles.length }})</div>
            @if (facturaDetalle()!.estado === 'BORRADOR') {
              <button matButton="outlined" (click)="abrirAgregarDetalle()">
                <i class="pi pi-plus"></i> Agregar
              </button>
            }
          </div>

          @if (facturaDetalle()!.detalles.length > 0) {
            <table mat-table [dataSource]="facturaDetalle()!.detalles" class="tabla">
              <ng-container matColumnDef="servicio">
                <th mat-header-cell *matHeaderCellDef>Servicio</th>
                <td mat-cell *matCellDef="let d">
                  {{ d.servicio }}
                  @if (d.paciente_nombre) {
                    <div style="font-size:0.75rem;color:#94a3b8">{{ d.paciente_nombre }}</div>
                  }
                </td>
              </ng-container>
              <ng-container matColumnDef="cups">
                <th mat-header-cell *matHeaderCellDef>CUPS</th>
                <td mat-cell *matCellDef="let d" style="font-size:0.82rem;color:#94a3b8">{{ d.codigo_cups }}</td>
              </ng-container>
              <ng-container matColumnDef="cantidad">
                <th mat-header-cell *matHeaderCellDef class="centro">Cant.</th>
                <td mat-cell *matCellDef="let d" class="centro">{{ d.cantidad }}</td>
              </ng-container>
              <ng-container matColumnDef="unitario">
                <th mat-header-cell *matHeaderCellDef class="derecha">Valor unit.</th>
                <td mat-cell *matCellDef="let d" class="derecha" style="font-size:0.82rem">
                  {{ d.valor_unitario | currency:'COP':'symbol-narrow':'1.0-0' }}
                </td>
              </ng-container>
              <ng-container matColumnDef="total">
                <th mat-header-cell *matHeaderCellDef class="derecha">Total</th>
                <td mat-cell *matCellDef="let d" class="derecha" style="font-weight:600">
                  {{ d.valor_total | currency:'COP':'symbol-narrow':'1.0-0' }}
                </td>
              </ng-container>
              <ng-container matColumnDef="quitar">
                <th mat-header-cell *matHeaderCellDef></th>
                <td mat-cell *matCellDef="let d" class="centro">
                  <button matIconButton class="peligro" matTooltip="Quitar el servicio"
                    [disabled]="accionando()" (click)="eliminarDetalle(d)">
                    <i class="pi pi-times"></i>
                  </button>
                </td>
              </ng-container>

              <tr mat-header-row *matHeaderRowDef="columnasDetalle()"></tr>
              <tr mat-row *matRowDef="let fila; columns: columnasDetalle()"></tr>
            </table>
          }

          <!-- Glosas -->
          @if (facturaDetalle()!.glosas.length > 0) {
            <mat-divider />
            <div class="section-title">Glosas ({{ facturaDetalle()!.glosas.length }})</div>
            <table mat-table [dataSource]="facturaDetalle()!.glosas" class="tabla">
              <ng-container matColumnDef="item">
                <th mat-header-cell *matHeaderCellDef>Item glosado</th>
                <td mat-cell *matCellDef="let g">{{ g.item_glosado }}</td>
              </ng-container>
              <ng-container matColumnDef="motivo">
                <th mat-header-cell *matHeaderCellDef>Motivo</th>
                <td mat-cell *matCellDef="let g" style="font-size:0.82rem">{{ g.motivo }}</td>
              </ng-container>
              <ng-container matColumnDef="valor">
                <th mat-header-cell *matHeaderCellDef class="derecha">Valor</th>
                <td mat-cell *matCellDef="let g" class="derecha" style="color:#f87171;font-weight:600">
                  {{ g.valor_glosado | currency:'COP':'symbol-narrow':'1.0-0' }}
                </td>
              </ng-container>
              <ng-container matColumnDef="estado">
                <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
                <td mat-cell *matCellDef="let g" class="centro">
                  <x-etiqueta [value]="g.estado"
                    [severity]="g.estado === 'PENDIENTE' ? 'warn' : g.estado === 'RECHAZADA' ? 'danger' : 'success'" />
                </td>
              </ng-container>
              <ng-container matColumnDef="respuesta">
                <th mat-header-cell *matHeaderCellDef>Respuesta</th>
                <td mat-cell *matCellDef="let g" style="font-size:0.78rem;color:#94a3b8">
                  {{ g.respuesta || '—' }}
                </td>
              </ng-container>

              <tr mat-header-row *matHeaderRowDef="columnasGlosasDetalle"></tr>
              <tr mat-row *matRowDef="let fila; columns: columnasGlosasDetalle"></tr>
            </table>
          }

          <!-- Acciones -->
          <mat-divider />
          <div style="display:flex;gap:0.5rem;justify-content:flex-end;flex-wrap:wrap">
            @if (facturaDetalle()!.estado === 'BORRADOR' || facturaDetalle()!.estado === 'RADICADA') {
              <button matButton="outlined" class="aviso-color" (click)="abrirAgregarGlosa()">
                <i class="pi pi-exclamation-circle"></i> Agregar glosa
              </button>
            }
            @if (facturaDetalle()!.estado === 'BORRADOR') {
              <button matButton="filled" class="exito" (click)="abrirRadicar(facturaDetalle()!)">
                <i class="pi pi-send"></i> Radicar
              </button>
            }
            @if (facturaDetalle()!.estado === 'RADICADA') {
              <button matButton="filled" class="exito" [disabled]="accionando()"
                (click)="cambiarEstado(facturaDetalle()!, 'ACEPTADA')">
                <i class="pi pi-check"></i> Marcar aceptada
              </button>
            }
            @if (facturaDetalle()!.estado === 'ACEPTADA') {
              <button matButton="filled" class="exito" [disabled]="accionando()"
                (click)="cambiarEstado(facturaDetalle()!, 'PAGADA')">
                <i class="pi pi-dollar"></i> Marcar pagada
              </button>
            }
            @if (!['PAGADA','ANULADA'].includes(facturaDetalle()!.estado)) {
              <button matButton="outlined" class="peligro" [disabled]="accionando()"
                (click)="cambiarEstado(facturaDetalle()!, 'ANULADA')">
                <i class="pi pi-ban"></i> Anular
              </button>
            }
          </div>
        </div>
      }

    </div>

  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .page-card { padding: 1.5rem; }
    .page-title { font-size: 1.3rem; font-weight: 700; color: #f1f5f9; margin: 0; }
    .tabla { width: 100%; }
    .centro { text-align: center; }
    .derecha { text-align: right; }
    mat-form-field { width: 100%; }
    /* Las líneas de la factura nueva son una rejilla densa: input desnudos con
       el aspecto puesto aquí, como hacía pInputText. */
    .linea-input {
      background: rgba(255,255,255,0.04); color: #e2e8f0;
      border: 1px solid rgba(255,255,255,0.12); border-radius: 5px;
      padding: 0.4rem 0.5rem; font-size: 0.875rem; width: 100%;
    }
    .linea-input:focus { outline: none; border-color: #818cf8; }
    .linea-input::placeholder { color: #64748b; }
    /* En M3 el input color de un botón no pinta nada: el color va por CSS. */
    .peligro { color: #dc2626; }
    .exito-texto { color: #16a34a; }
    .exito { background: #16a34a; color: #fff; }
    .aviso-color { color: #f59e0b; }
    .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.25rem; }
    .cargando { padding: 3rem; text-align: center; color: #64748b; display: flex; align-items: center; justify-content: center; gap: 0.5rem; }
    .sin-resultados { text-align: center; padding: 3rem 1rem; color: #64748b; display: flex; flex-direction: column; align-items: center; gap: 0.75rem; }

    .stats-row { display: flex; gap: 0.75rem; margin-bottom: 1.25rem; flex-wrap: wrap; }
    .stat-card {
      flex: 1; min-width: 90px;
      background: rgba(255,255,255,0.04); border: 1px solid rgba(255,255,255,0.08);
      border-radius: 8px; padding: 0.75rem 1rem;
    }
    .stat-card.stat-blue { border-color: rgba(96,165,250,0.2); background: rgba(96,165,250,0.05); }
    .stat-card.stat-green { border-color: rgba(74,222,128,0.2); background: rgba(74,222,128,0.05); }
    .stat-card.stat-green2 { border-color: rgba(52,211,153,0.2); background: rgba(52,211,153,0.05); }
    .stat-card.stat-yellow { border-color: rgba(251,191,36,0.2); background: rgba(251,191,36,0.05); }
    .stat-card.stat-purple { border-color: rgba(167,139,250,0.2); background: rgba(167,139,250,0.05); }
    .stat-value { font-size: 1.4rem; font-weight: 700; color: #f1f5f9; line-height: 1; }
    .stat-label { font-size: 0.68rem; color: #94a3b8; margin-top: 0.2rem; text-transform: uppercase; letter-spacing: 0.04em; }

    .tab-bar { display: flex; gap: 0.25rem; margin-bottom: 1rem; border-bottom: 1px solid rgba(255,255,255,0.1); }
    .tab-btn {
      background: none; border: none; color: #94a3b8; cursor: pointer;
      padding: 0.5rem 1rem; font-size: 0.875rem; border-bottom: 2px solid transparent;
      display: flex; align-items: center; gap: 0.4rem; transition: all 0.15s; margin-bottom: -1px;
    }
    .tab-btn:hover { color: #e2e8f0; }
    .tab-btn.active { color: #818cf8; border-bottom-color: #818cf8; font-weight: 600; }
    .badge-count {
      background: #ef4444; color: white; border-radius: 999px;
      font-size: 0.65rem; font-weight: 700; padding: 0.1rem 0.4rem; line-height: 1.4;
    }

    .filter-bar { display: flex; gap: 0.5rem; align-items: center; margin-bottom: 0.875rem; flex-wrap: wrap; }
    .search-input { width: 220px; }
    .num-factura { font-family: monospace; font-size: 0.9rem; color: #a5b4fc; }
    .dur-badge {
      display: inline-block; padding: 0.15rem 0.5rem; border-radius: 999px;
      background: rgba(255,255,255,0.06); color: #e2e8f0; font-size: 0.82rem;
    }

    .form-card {
      background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.07);
      border-radius: 10px; padding: 1.5rem;
    }
    .form-section { margin-bottom: 0.25rem; }
    .section-title { font-size: 0.75rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em; color: #818cf8; margin-bottom: 1rem; }
    .form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
    .field { display: flex; flex-direction: column; gap: 0.35rem; }
    .field-label { font-weight: 500; font-size: 0.875rem; color: #e2e8f0; }
    .w-full { width: 100%; }
    .date-input {
      background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.15);
      border-radius: 6px; padding: 0.5rem 0.75rem; color: #f1f5f9; font-size: 0.875rem;
      color-scheme: dark;
    }
    .step-actions { display: flex; gap: 0.5rem; justify-content: flex-end; padding-top: 1.25rem; border-top: 1px solid rgba(255,255,255,0.1); margin-top: 1rem; }

    .detalles-container { background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.07); border-radius: 8px; overflow: hidden; }
    .detalle-header-row {
      display: grid; grid-template-columns: 1fr 110px 70px 130px 130px 36px;
      gap: 0.5rem; padding: 0.5rem 0.75rem;
      background: rgba(255,255,255,0.04); font-size: 0.72rem; font-weight: 600;
      text-transform: uppercase; letter-spacing: 0.05em; color: #64748b;
    }
    .detalle-row {
      display: grid; grid-template-columns: 1fr 110px 70px 130px 130px 36px;
      gap: 0.5rem; padding: 0.5rem 0.75rem; align-items: center;
      border-top: 1px solid rgba(255,255,255,0.05);
    }
    .total-linea { text-align: right; font-weight: 600; font-size: 0.875rem; color: #f1f5f9; }
    .total-factura {
      display: flex; justify-content: flex-end; gap: 2rem;
      padding: 0.4rem 1rem; font-size: 0.85rem; color: #94a3b8;
      border-top: 1px solid rgba(255,255,255,0.05);
    }
    .total-neto { font-weight: 700; font-size: 1rem; color: #4ade80; border-top: 2px solid rgba(74,222,128,0.2); }

    .det-grid4 { display: grid; grid-template-columns: repeat(4, 1fr); gap: 1rem; margin-bottom: 1rem; }
    .det-label { font-size: 0.72rem; color: #64748b; text-transform: uppercase; letter-spacing: 0.05em; }
    .det-valor { font-size: 0.9rem; color: #f1f5f9; font-weight: 500; margin-top: 0.1rem; }
    .valores-row { display: flex; gap: 1rem; margin: 1rem 0; }
    .valor-card {
      flex: 1; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08);
      border-radius: 8px; padding: 0.875rem 1rem;
    }
    .valor-num { font-size: 1.3rem; font-weight: 700; color: #f1f5f9; margin-top: 0.25rem; }

    .dialog-footer {
      display: flex; justify-content: flex-end; gap: 0.5rem;
      padding-top: 1rem; border-top: 1px solid rgba(255,255,255,0.1); margin-top: 1rem;
    }
    .item-info-card {
      display: flex; gap: 0.75rem; align-items: flex-start;
      background: rgba(99,102,241,0.1); border: 1px solid rgba(99,102,241,0.25);
      border-radius: 8px; padding: 0.75rem 1rem;
    }
    ::ng-deep textarea { resize: vertical; }
  `]
})
export class FacturacionComponent implements OnInit {
  private svc = inject(FacturacionService);
  private dialogo = inject(MatDialog);

  /** El orden de las columnas lo fija esta lista, no la plantilla. */
  readonly columnasFacturas = ['numero', 'entidad', 'periodo', 'fecha', 'valor',
                               'estado', 'items', 'acciones'];
  readonly columnasContratos = ['nombre', 'numero', 'entidad', 'tarifa', 'vigencia',
                                'estado', 'acciones'];
  readonly columnasGlosas = ['factura', 'item', 'motivo', 'valor', 'fecha', 'estado', 'accion'];
  readonly columnasGlosasDetalle = ['item', 'motivo', 'valor', 'estado', 'respuesta'];

  /** La columna de quitar solo existe mientras la factura sea un borrador:
   *  radicada ya salió hacia la EPS. */
  columnasDetalle(): string[] {
    const base = ['servicio', 'cups', 'cantidad', 'unitario', 'total'];
    return this.facturaDetalle()?.estado === 'BORRADOR' ? [...base, 'quitar'] : base;
  }
  private messageService = inject(Avisos);

  vista = signal<Vista>('lista');
  tab = signal<Tab>('facturas');
  stats = signal<StatsFacturacion | null>(null);

  // Facturas
  cargandoFact = signal(false);
  facturas = signal<Factura[]>([]);
  busqFactura = '';
  filtroEstadoFact = '';
  filtroEntidad: number | null = null;
  private _factTimer: any;

  // Detalle
  facturaDetalle = signal<Factura | null>(null);
  cargandoDet = signal(false);

  // Contratos
  cargandoCont = signal(false);
  contratos = signal<Contrato[]>([]);

  // Glosas
  cargandoGlosa = signal(false);
  glosas = signal<Glosa[]>([]);
  filtroEstadoGlosa = 'PENDIENTE';

  // Entidades
  entidades = signal<Entidad[]>([]);
  entidadesOpts = () => this.entidades().map(e => ({ label: `${e.nombre} (${e.codigo_eps})`, value: e.id }));
  contratosEntidad = () => this.contratos()
    .filter(c => !this.nFact.entidad_id || c.entidad_id === this.nFact.entidad_id)
    .map(c => ({ label: c.nombre, value: c.id }));

  guardando = signal(false);
  accionando = signal(false);

  // Nueva factura
  nFact: Partial<FacturaCreate> & { detalles?: any[] } = { descuentos: 0, detalles: [] };
  lineasTemp = signal<Partial<DetalleCreate>[]>([]);
  subtotalTemp = () => this.lineasTemp().reduce((s, l) => s + (l.valor_unitario || 0) * (l.cantidad || 1), 0);

  // Dialog radicar


  // Dialog detalle (agregar línea)


  // Dialog glosa


  // Dialog responder glosa


  // Dialog contrato


  readonly opcionesEstadoFact = [
    { label: 'Borrador', value: 'BORRADOR' },
    { label: 'Radicada', value: 'RADICADA' },
    { label: 'Aceptada', value: 'ACEPTADA' },
    { label: 'Glosada', value: 'GLOSADA' },
    { label: 'Pagada', value: 'PAGADA' },
    { label: 'Anulada', value: 'ANULADA' },
  ];

  readonly opcionesEstadoGlosa = [
    { label: 'Pendientes', value: 'PENDIENTE' },
    { label: 'Aceptadas', value: 'ACEPTADA' },
    { label: 'Rechazadas', value: 'RECHAZADA' },
  ];

  readonly opcionesTarifa = TARIFAS;

  ngOnInit(): void {
    this.svc.listEntidades().subscribe({ next: es => this.entidades.set(es) });
    this.svc.listContratos().subscribe({ next: cs => this.contratos.set(cs) });
    this.cargarStats();
    this.cargarFacturas();
  }

  cambiarTab(t: Tab): void {
    this.tab.set(t);
    if (t === 'facturas' && this.facturas().length === 0) this.cargarFacturas();
    if (t === 'contratos') this.cargarContratos();
    if (t === 'glosas') this.cargarGlosas();
  }

  cargarStats(): void {
    this.svc.getStats().subscribe({ next: s => this.stats.set(s) });
  }

  cargarFacturas(): void {
    clearTimeout(this._factTimer);
    this._factTimer = setTimeout(() => {
      this.cargandoFact.set(true);
      const params: Record<string, string> = {};
      if (this.busqFactura) params['q'] = this.busqFactura;
      if (this.filtroEstadoFact) params['estado'] = this.filtroEstadoFact;
      if (this.filtroEntidad) params['entidad_id'] = String(this.filtroEntidad);
      this.svc.listFacturas(params).subscribe({
        next: fs => { this.facturas.set(fs); this.cargandoFact.set(false); },
        error: () => this.cargandoFact.set(false),
      });
    }, 300);
  }

  cargarContratos(): void {
    this.cargandoCont.set(true);
    this.svc.listContratos().subscribe({
      next: cs => { this.contratos.set(cs); this.cargandoCont.set(false); },
      error: () => this.cargandoCont.set(false),
    });
  }

  cargarGlosas(): void {
    this.cargandoGlosa.set(true);
    this.svc.listGlosas(this.filtroEstadoGlosa || undefined).subscribe({
      next: gs => { this.glosas.set(gs); this.cargandoGlosa.set(false); },
      error: () => this.cargandoGlosa.set(false),
    });
  }

  // ── Nueva Factura ────────────────────────────────

  abrirNuevaFactura(): void {
    this.nFact = { descuentos: 0 };
    this.lineasTemp.set([]);
    this.vista.set('nueva');
  }

  onEntidadChange(): void {
    this.nFact.contrato_id = undefined;
  }

  agregarLineaTemporal(): void {
    this.lineasTemp.update(ls => [...ls, { codigo_cups: '890201', cantidad: 1, valor_unitario: 0, servicio: '' }]);
  }

  quitarLinea(idx: number): void {
    this.lineasTemp.update(ls => ls.filter((_, i) => i !== idx));
  }

  calcLine(_: any): void {
    this.lineasTemp.update(ls => [...ls]);
  }

  crearFactura(): void {
    const lineas = this.lineasTemp().filter(l => l.servicio && l.valor_unitario);
    const payload: FacturaCreate = {
      entidad_id: this.nFact.entidad_id!,
      contrato_id: this.nFact.contrato_id,
      periodo_inicio: this.nFact.periodo_inicio!,
      periodo_fin: this.nFact.periodo_fin!,
      descuentos: Number(this.nFact.descuentos) || 0,
      observaciones: this.nFact.observaciones,
      detalles: lineas.map(l => ({
        servicio: l.servicio!,
        codigo_cups: l.codigo_cups || '890201',
        cantidad: Number(l.cantidad) || 1,
        valor_unitario: Number(l.valor_unitario) || 0,
      })),
    };
    this.guardando.set(true);
    this.svc.crearFactura(payload).subscribe({
      next: f => {
        this.guardando.set(false);
        this.ok('Factura creada', f.numero_factura);
        this.cargarFacturas();
        this.cargarStats();
        this.vista.set('lista');
      },
      error: err => { this.guardando.set(false); this.err(err?.error?.detail ?? 'Error al crear factura'); },
    });
  }

  // ── Detalle factura ──────────────────────────────

  abrirDetalle(id: number): void {
    this.cargandoDet.set(true);
    this.svc.getFactura(id).subscribe({
      next: f => { this.facturaDetalle.set(f); this.cargandoDet.set(false); this.vista.set('detalle'); },
      error: () => { this.cargandoDet.set(false); this.err('Error al cargar factura'); },
    });
  }

  volverLista(): void {
    this.vista.set('lista');
    this.facturaDetalle.set(null);
    this.cargarFacturas();
    this.cargarStats();
  }

  eliminarFactura(f: Factura): void {
    this.accionando.set(true);
    this.svc.eliminarFactura(f.id).subscribe({
      next: () => {
        this.accionando.set(false);
        this.ok('Factura eliminada');
        this.cargarFacturas();
        this.cargarStats();
      },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error al eliminar'); },
    });
  }

  cambiarEstado(f: Factura, estado: string): void {
    this.accionando.set(true);
    this.svc.cambiarEstado(f.id, estado).subscribe({
      next: updated => {
        this.accionando.set(false);
        this.ok(`Factura ${this.estadoLabel(estado).toLowerCase()}`, updated.numero_factura);
        if (this.vista() === 'detalle') this.facturaDetalle.set(updated);
        this.cargarFacturas();
        this.cargarStats();
      },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error al cambiar estado'); },
    });
  }

  // ── Radicar ──────────────────────────────────────

  abrirRadicar(f: Factura): void {
    this.dialogo
      .open(FacturacionRadicarComponent, { data: f, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((r?: { numero: string; fecha: string }) => { if (r) this.radicar(f, r); });
  }

  private radicar(f: Factura, r: { numero: string; fecha: string }): void {
    this.accionando.set(true);
    this.svc.radicarFactura(f.id, r.numero, r.fecha).subscribe({
      next: updated => {
        this.accionando.set(false);
        this.ok('Factura radicada', updated.numero_factura);
        if (this.vista() === 'detalle') this.facturaDetalle.set(updated);
        this.cargarFacturas();
        this.cargarStats();
      },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error al radicar'); },
    });
  }

  // ── Detalles ─────────────────────────────────────

  abrirAgregarDetalle(): void {
    const f = this.facturaDetalle();
    if (!f) return;
    this.dialogo
      .open(FacturacionServicioComponent, { maxWidth: '95vw' })
      .afterClosed()
      .subscribe((d?: DetalleCreate) => { if (d) this.agregarDetalle(f, d); });
  }

  private agregarDetalle(f: Factura, d: DetalleCreate): void {
    this.accionando.set(true);
    this.svc.agregarDetalle(f.id, d).subscribe({
      next: updated => {
        this.accionando.set(false);
        this.facturaDetalle.set(updated);
        this.ok('Servicio agregado');
      },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error al agregar'); },
    });
  }

  eliminarDetalle(d: DetalleFactura): void {
    const f = this.facturaDetalle();
    if (!f) return;
    this.accionando.set(true);
    this.svc.eliminarDetalle(f.id, d.id).subscribe({
      next: updated => { this.accionando.set(false); this.facturaDetalle.set(updated); this.ok('Servicio eliminado'); },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error al eliminar'); },
    });
  }

  // ── Glosas ───────────────────────────────────────

  abrirAgregarGlosa(): void {
    const id = this.facturaDetalle()?.id;
    if (!id) return;
    this.dialogo
      .open(FacturacionGlosaComponent, { maxWidth: '95vw' })
      .afterClosed()
      .subscribe((g?: { item_glosado: string; motivo: string; valor_glosado: number }) => {
        if (g) this.agregarGlosa(id, g);
      });
  }

  private agregarGlosa(
    facturaId: number,
    g: { item_glosado: string; motivo: string; valor_glosado: number },
  ): void {
    this.accionando.set(true);
    this.svc.agregarGlosa(facturaId, g).subscribe({
      next: () => {
        this.accionando.set(false);
        this.ok('Glosa registrada');
        if (this.vista() === 'detalle') this.abrirDetalle(facturaId);
        this.cargarStats();
      },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error al registrar glosa'); },
    });
  }

  abrirResponderGlosa(g: Glosa): void {
    this.dialogo
      .open(FacturacionResponderGlosaComponent, { data: g, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((r?: { texto: string; aceptar: boolean }) => { if (r) this.responderGlosa(g, r); });
  }

  private responderGlosa(g: Glosa, r: { texto: string; aceptar: boolean }): void {
    this.accionando.set(true);
    this.svc.responderGlosa(g.id, r.texto, r.aceptar).subscribe({
      next: () => {
        this.accionando.set(false);
        this.ok(r.aceptar ? 'Glosa aceptada' : 'Glosa rechazada');
        this.cargarGlosas();
        this.cargarStats();
      },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error al responder'); },
    });
  }

  // ── Contratos ────────────────────────────────────

  abrirFormContrato(c: Contrato | null): void {
    this.dialogo
      .open(FacturacionContratoComponent, {
        data: { contrato: c, entidades: this.entidadesOpts() },
        maxWidth: '95vw',
      })
      .afterClosed()
      .subscribe((datos?: Partial<ContratoCreate>) => { if (datos) this.guardarContrato(c, datos); });
  }

  private guardarContrato(editando: Contrato | null, datos: Partial<ContratoCreate>): void {
    const obs = editando
      ? this.svc.actualizarContrato(editando.id, datos as ContratoCreate)
      : this.svc.crearContrato(datos as ContratoCreate);
    obs.subscribe({
      next: () => {
        this.ok(editando ? 'Contrato actualizado' : 'Contrato creado');
        this.cargarContratos();
      },
      error: err => this.err(err?.error?.detail ?? 'Error al guardar contrato'),
    });
  }

  toggleContrato(c: Contrato): void {
    this.svc.actualizarContrato(c.id, { activo: !c.activo }).subscribe({
      next: () => { this.ok(c.activo ? 'Contrato desactivado' : 'Contrato activado'); this.cargarContratos(); },
      error: err => this.err(err?.error?.detail ?? 'Error'),
    });
  }

  estadoLabel(estado: string): string {
    const map: Record<string, string> = {
      BORRADOR: 'Borrador', RADICADA: 'Radicada', ACEPTADA: 'Aceptada',
      GLOSADA: 'Glosada', PAGADA: 'Pagada', ANULADA: 'Anulada',
    };
    return map[estado] ?? estado;
  }

  estadoSeverity(estado: string): 'secondary' | 'info' | 'success' | 'warn' | 'danger' {
    if (estado === 'BORRADOR') return 'secondary';
    if (estado === 'RADICADA') return 'info';
    if (estado === 'ACEPTADA' || estado === 'PAGADA') return 'success';
    if (estado === 'GLOSADA') return 'warn';
    if (estado === 'ANULADA') return 'danger';
    return 'secondary';
  }

  private ok(summary: string, detail?: string): void {
    this.messageService.add({ severity: 'success', summary, detail });
  }

  private err(detail: string): void {
    this.messageService.add({ severity: 'error', summary: 'Error', detail });
  }
}
