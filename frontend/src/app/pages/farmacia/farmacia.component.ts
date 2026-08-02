import { Component, Inject, OnInit, inject, signal, computed, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MAT_DIALOG_DATA, MatDialog, MatDialogModule } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatTableModule } from '@angular/material/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatDividerModule } from '@angular/material/divider';
import { Avisos } from '../../shared/ui/avisos.service';
import {
  FarmaciaService, ItemInventario, ItemInventarioCreate,
  EntradaStock, FormulaPendiente, DispensarCreate,
  Dispensacion, Movimiento, StatsInventario,
  Proveedor, ProveedorCreate,
} from '../../services/farmacia.service';

type Tab = 'inventario' | 'formulas' | 'dispensaciones' | 'movimientos' | 'proveedores';

/** Opción de un selector: lo que se lee y lo que se guarda. */
interface Opcion { label: string; value: string | number }

const TIPOS: Opcion[] = [
  { label: 'Medicamento', value: 'MEDICAMENTO' },
  { label: 'Insumo', value: 'INSUMO' },
  { label: 'Dispositivo', value: 'DISPOSITIVO' },
  { label: 'Reactivo', value: 'REACTIVO' },
];

const UNIDADES: Opcion[] = [
  { label: 'Unidad', value: 'UNIDAD' },
  { label: 'Caja', value: 'CAJA' },
  { label: 'mL', value: 'ML' },
  { label: 'mg', value: 'MG' },
  { label: 'g', value: 'G' },
  { label: 'Frasco', value: 'FRASCO' },
  { label: 'Ampolla', value: 'AMPOLLA' },
];

const UBICACIONES: Opcion[] = [
  { label: 'Farmacia', value: 'FARMACIA' },
  { label: 'UCI', value: 'UCI' },
  { label: 'Urgencias', value: 'URGENCIAS' },
  { label: 'Quirófano', value: 'QUIROFANO' },
  { label: 'Hospitalización', value: 'HOSPITALIZACION' },
];

/** Alta y edición de un proveedor. Devuelve los campos, o nada. */
@Component({
  selector: 'app-farmacia-proveedor',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatInputModule,
  ],
  template: `
    <h2 mat-dialog-title>{{ editando ? 'Editar proveedor' : 'Nuevo proveedor' }}</h2>
    <mat-dialog-content>
      <div class="rejilla">
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Razón social *</mat-label>
          <input matInput [(ngModel)]="f.razon_social" placeholder="Ej: Distribuidora Andina S.A.S." />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>NIT</mat-label>
          <input matInput [(ngModel)]="f.nit" placeholder="Opcional: no todo proveedor tiene NIT" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Suministra</mat-label>
          <input matInput [(ngModel)]="f.suministra" placeholder="Medicamentos, insumos, reactivos..." />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Contacto</mat-label>
          <input matInput [(ngModel)]="f.contacto" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Teléfono</mat-label>
          <input matInput [(ngModel)]="f.telefono" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Correo</mat-label>
          <input matInput [(ngModel)]="f.email" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Ciudad</mat-label>
          <input matInput [(ngModel)]="f.ciudad" />
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Dirección</mat-label>
          <input matInput [(ngModel)]="f.direccion" />
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Observaciones</mat-label>
          <textarea matInput [(ngModel)]="f.observaciones" rows="2"></textarea>
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!f.razon_social" [mat-dialog-close]="resultado()">
        <i class="pi pi-check"></i> {{ editando ? 'Guardar' : 'Crear proveedor' }}
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .rejilla { display:grid; grid-template-columns:1fr 1fr; gap:.875rem; padding-top:.5rem;
      min-width:min(34rem, 85vw); }
    .ancho { grid-column:1/-1; }
    mat-form-field { width:100%; }
  `],
})
export class FarmaciaProveedorComponent {
  readonly editando: boolean;
  f: Partial<ProveedorCreate>;

  constructor(@Inject(MAT_DIALOG_DATA) p: Proveedor | null) {
    this.editando = !!p;
    this.f = p
      ? {
          razon_social: p.razon_social, nit: p.nit, contacto: p.contacto,
          telefono: p.telefono, email: p.email, direccion: p.direccion,
          ciudad: p.ciudad, suministra: p.suministra, observaciones: p.observaciones,
        }
      : { razon_social: '' };
  }

  resultado(): Partial<ProveedorCreate> | null {
    return this.f.razon_social ? this.f : null;
  }
}

/** Alta y edición de un item de inventario. Devuelve los campos, o nada. */
@Component({
  selector: 'app-farmacia-item',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatCheckboxModule,
    MatFormFieldModule, MatInputModule, MatSelectModule,
  ],
  template: `
    <h2 mat-dialog-title>{{ editando ? 'Editar item' : 'Nuevo item de inventario' }}</h2>
    <mat-dialog-content>
      <div class="rejilla">
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Nombre *</mat-label>
          <input matInput [(ngModel)]="f.nombre" placeholder="Ej: Amoxicilina 500mg" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Tipo *</mat-label>
          <mat-select [(ngModel)]="f.tipo">
            @for (o of tipos; track o.value) { <mat-option [value]="o.value">{{ o.label }}</mat-option> }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Categoría</mat-label>
          <input matInput [(ngModel)]="f.categoria" placeholder="Analgésico, Antibiótico..." />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Presentación</mat-label>
          <input matInput [(ngModel)]="f.presentacion" placeholder="Tableta, Cápsula, Jarabe..." />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Lote</mat-label>
          <input matInput [(ngModel)]="f.lote" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Fecha vencimiento</mat-label>
          <input matInput type="date" [(ngModel)]="f.fecha_vencimiento" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Registro INVIMA</mat-label>
          <input matInput [(ngModel)]="f.registro_invima" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Unidad de medida</mat-label>
          <mat-select [(ngModel)]="f.unidad_medida">
            @for (o of unidades; track o.value) { <mat-option [value]="o.value">{{ o.label }}</mat-option> }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Ubicación</mat-label>
          <mat-select [(ngModel)]="f.ubicacion">
            @for (o of ubicaciones; track o.value) { <mat-option [value]="o.value">{{ o.label }}</mat-option> }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Costo unitario (COP)</mat-label>
          <input matInput type="number" [(ngModel)]="f.costo_unitario" placeholder="0" />
        </mat-form-field>
        @if (!editando) {
          <mat-form-field appearance="outline">
            <mat-label>Stock inicial</mat-label>
            <input matInput type="number" [(ngModel)]="f.cantidad_actual" placeholder="0" />
          </mat-form-field>
        }
        <mat-form-field appearance="outline">
          <mat-label>Stock mínimo (alerta)</mat-label>
          <input matInput type="number" [(ngModel)]="f.cantidad_minima" placeholder="10" />
        </mat-form-field>
        <div class="marcas ancho">
          <mat-checkbox [(ngModel)]="f.requiere_refrigeracion">Requiere refrigeración</mat-checkbox>
          <mat-checkbox [(ngModel)]="f.controlado">Medicamento controlado</mat-checkbox>
        </div>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!f.nombre" [mat-dialog-close]="resultado()">
        <i class="pi pi-check"></i> {{ editando ? 'Guardar cambios' : 'Crear item' }}
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .rejilla { display:grid; grid-template-columns:1fr 1fr; gap:.875rem; padding-top:.5rem;
      min-width:min(36rem, 85vw); }
    .ancho { grid-column:1/-1; }
    .marcas { display:flex; gap:2rem; flex-wrap:wrap; }
    mat-form-field { width:100%; }
  `],
})
export class FarmaciaItemComponent {
  readonly tipos = TIPOS;
  readonly unidades = UNIDADES;
  readonly ubicaciones = UBICACIONES;

  readonly editando: boolean;
  f: Partial<ItemInventarioCreate>;

  constructor(@Inject(MAT_DIALOG_DATA) item: ItemInventario | null) {
    this.editando = !!item;
    this.f = item
      ? {
          nombre: item.nombre, tipo: item.tipo, categoria: item.categoria,
          presentacion: item.presentacion, lote: item.lote,
          fecha_vencimiento: item.fecha_vencimiento, registro_invima: item.registro_invima,
          cantidad_actual: item.cantidad_actual, cantidad_minima: item.cantidad_minima,
          unidad_medida: item.unidad_medida, ubicacion: item.ubicacion,
          costo_unitario: item.costo_unitario,
          requiere_refrigeracion: item.requiere_refrigeracion, controlado: item.controlado,
        }
      : {
          tipo: 'MEDICAMENTO', unidad_medida: 'UNIDAD', ubicacion: 'FARMACIA',
          cantidad_actual: 0, cantidad_minima: 10, costo_unitario: 0,
          requiere_refrigeracion: false, controlado: false, lote: '',
        };
  }

  resultado(): Partial<ItemInventarioCreate> | null {
    return this.f.nombre ? this.f : null;
  }
}

/** Entrada de existencias. Devuelve lo que se registra, o nada. */
@Component({
  selector: 'app-farmacia-entrada',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatSelectModule,
  ],
  template: `
    <h2 mat-dialog-title>Registrar entrada de stock</h2>
    <mat-dialog-content>
      <div class="ficha">
        <i class="pi pi-box" style="color:#818cf8;font-size:1.2rem"></i>
        <div>
          <div style="font-weight:600;color:#f1f5f9">{{ item.nombre }}</div>
          <div style="font-size:0.82rem;color:#94a3b8">
            Stock actual: {{ item.cantidad_actual }} {{ item.unidad_medida }}
          </div>
        </div>
      </div>
      <div class="rejilla">
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Cantidad a ingresar *</mat-label>
          <input matInput type="number" [(ngModel)]="f.cantidad" placeholder="0" />
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Motivo</mat-label>
          <input matInput [(ngModel)]="f.motivo" placeholder="Compra, Donación, Devolución..." />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Nuevo lote (opcional)</mat-label>
          <input matInput [(ngModel)]="f.lote" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Nueva fecha vencimiento</mat-label>
          <input matInput type="date" [(ngModel)]="f.fecha_vencimiento" />
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Nuevo costo unitario (opcional)</mat-label>
          <input matInput type="number" [(ngModel)]="f.costo_unitario"
            placeholder="Dejar vacío para mantener actual" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Proveedor</mat-label>
          <mat-select [(ngModel)]="f.proveedor_id">
            <!-- «Sin proveedor» es una opción de verdad —donación, traslado—, no
                 el hueco de no haber elegido. -->
            <mat-option [value]="null">Sin proveedor (donación, traslado…)</mat-option>
            @for (o of proveedores; track o.value) {
              <mat-option [value]="o.value">{{ o.label }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Factura / remisión</mat-label>
          <input matInput [(ngModel)]="f.documento_soporte" placeholder="Ej: FV-1024" />
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" class="exito" [disabled]="!completo()"
        [mat-dialog-close]="resultado()">
        <i class="pi pi-arrow-down"></i> Registrar entrada
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .ficha { display:flex; gap:.75rem; align-items:center; margin-bottom:1rem;
      padding:.75rem; background:rgba(99,102,241,0.1); border-radius:6px;
      border:1px solid rgba(99,102,241,0.2); }
    .rejilla { display:grid; grid-template-columns:1fr 1fr; gap:.875rem;
      min-width:min(30rem, 85vw); }
    .ancho { grid-column:1/-1; }
    mat-form-field { width:100%; }
    .exito { background:#16a34a; color:#fff; }
  `],
})
export class FarmaciaEntradaComponent {
  readonly item: ItemInventario;
  readonly proveedores: Opcion[];
  f: Partial<EntradaStock> = { motivo: 'Compra', cantidad: 0 };

  constructor(@Inject(MAT_DIALOG_DATA) datos: { item: ItemInventario; proveedores: Opcion[] }) {
    this.item = datos.item;
    this.proveedores = datos.proveedores;
  }

  completo(): boolean { return !!this.f.cantidad && Number(this.f.cantidad) > 0; }

  resultado(): EntradaStock | null {
    if (!this.completo()) return null;
    return {
      cantidad: Number(this.f.cantidad),
      motivo: this.f.motivo || 'Compra',
      lote: this.f.lote,
      fecha_vencimiento: this.f.fecha_vencimiento,
      costo_unitario: this.f.costo_unitario ? Number(this.f.costo_unitario) : undefined,
      proveedor_id: this.f.proveedor_id ?? undefined,
      documento_soporte: this.f.documento_soporte || undefined,
    };
  }
}

/** Dispensación contra una fórmula. Devuelve lo dispensado, o nada. */
@Component({
  selector: 'app-farmacia-dispensar',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatSelectModule,
  ],
  template: `
    <h2 mat-dialog-title>Dispensar medicamento</h2>
    <mat-dialog-content>
      <div class="ficha">
        <i class="pi pi-user" style="color:#818cf8;font-size:1.2rem"></i>
        <div>
          <div style="font-weight:600;color:#f1f5f9">{{ formula.paciente_nombre }}</div>
          <div style="font-size:0.85rem;color:#e2e8f0;margin-top:0.2rem">
            <strong>{{ formula.medicamento }}</strong> — {{ formula.dosis }}
          </div>
          <div style="font-size:0.78rem;color:#94a3b8">
            Cantidad prescrita: {{ formula.cantidad }} {{ formula.unidad || 'und' }}
          </div>
        </div>
      </div>
      <div class="rejilla">
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Item del inventario *</mat-label>
          <mat-select [(ngModel)]="f.item_inventario_id">
            @for (o of items; track o.value) {
              <mat-option [value]="o.value">{{ o.label }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Cantidad a dispensar *</mat-label>
          <input matInput type="number" [(ngModel)]="f.cantidad_dispensada" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Lote dispensado</mat-label>
          <input matInput [(ngModel)]="f.lote_dispensado" placeholder="Se toma del item seleccionado" />
        </mat-form-field>
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Observaciones</mat-label>
          <textarea matInput [(ngModel)]="f.observaciones" rows="2"
            placeholder="Instrucciones adicionales..."></textarea>
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" class="exito" [disabled]="!completo()"
        [mat-dialog-close]="resultado()">
        <i class="pi pi-send"></i> Confirmar dispensación
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .ficha { display:flex; gap:.75rem; align-items:flex-start; margin-bottom:1rem;
      padding:.75rem; background:rgba(99,102,241,0.1); border-radius:6px;
      border:1px solid rgba(99,102,241,0.2); }
    .rejilla { display:grid; grid-template-columns:1fr 1fr; gap:.875rem;
      min-width:min(30rem, 85vw); }
    .ancho { grid-column:1/-1; }
    mat-form-field { width:100%; }
    .exito { background:#16a34a; color:#fff; }
  `],
})
export class FarmaciaDispensarComponent {
  readonly formula: FormulaPendiente;
  readonly items: Opcion[];
  /** El lote sale del item elegido si nadie escribe otro: quien dispensa suele
   *  entregar el que está en el estante, y obligarlo a copiarlo a mano es
   *  fricción que se rodea escribiendo cualquier cosa. */
  private readonly loteDe: Map<number, string>;
  f: Partial<DispensarCreate>;

  constructor(
    @Inject(MAT_DIALOG_DATA)
    datos: { formula: FormulaPendiente; items: Opcion[]; lotes: Map<number, string> },
  ) {
    this.formula = datos.formula;
    this.items = datos.items;
    this.loteDe = datos.lotes;
    this.f = { formula_id: datos.formula.id, cantidad_dispensada: datos.formula.cantidad };
  }

  completo(): boolean {
    return !!this.f.item_inventario_id && !!this.f.cantidad_dispensada;
  }

  resultado(): DispensarCreate | null {
    if (!this.completo()) return null;
    const id = this.f.item_inventario_id!;
    return {
      formula_id: this.f.formula_id!,
      item_inventario_id: id,
      cantidad_dispensada: Number(this.f.cantidad_dispensada),
      lote_dispensado: this.f.lote_dispensado || this.loteDe.get(id) || '',
      observaciones: this.f.observaciones,
    };
  }
}

@Component({
  selector: 'app-farmacia',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatFormFieldModule, MatInputModule, MatSelectModule,
    MatTableModule, EtiquetaComponent, MatTooltipModule, MatDividerModule,
  ],
  template: `
    <div class="page-card">
      <div class="list-header">
        <h2 class="page-title">Farmacia e Inventario</h2>
        @if (tab() === 'inventario') {
          <button matButton="filled" (click)="abrirCrear()"><i class="pi pi-plus"></i> Nuevo item</button>
        }
        @if (tab() === 'proveedores') {
          <button matButton="filled" (click)="abrirCrearProveedor()">
            <i class="pi pi-plus"></i> Nuevo proveedor
          </button>
        }
      </div>

      <!-- Stats -->
      <div class="stats-row">
        <div class="stat-card">
          <div class="stat-value">{{ stats()?.total_items ?? '—' }}</div>
          <div class="stat-label">Items en inventario</div>
        </div>
        <div class="stat-card stat-red">
          <div class="stat-value">{{ stats()?.stock_bajo ?? '—' }}</div>
          <div class="stat-label">Stock bajo</div>
        </div>
        <div class="stat-card stat-yellow">
          <div class="stat-value">{{ (stats()?.por_vencer ?? 0) + (stats()?.vencidos ?? 0) }}</div>
          <div class="stat-label">Venc. / Por vencer</div>
        </div>
        <div class="stat-card stat-purple">
          <div class="stat-value">{{ stats()?.formulas_pendientes ?? '—' }}</div>
          <div class="stat-label">Fórmulas pendientes</div>
        </div>
        <div class="stat-card stat-green">
          <div class="stat-value">{{ (stats()?.valor_total_inventario ?? 0) | currency:'COP':'symbol-narrow':'1.0-0' }}</div>
          <div class="stat-label">Valor inventario</div>
        </div>
      </div>

      <!-- Tabs -->
      <div class="tab-bar">
        <button class="tab-btn" [class.active]="tab() === 'inventario'" (click)="cambiarTab('inventario')">
          <i class="pi pi-box"></i> Inventario
        </button>
        <button class="tab-btn" [class.active]="tab() === 'formulas'" (click)="cambiarTab('formulas')">
          <i class="pi pi-file-edit"></i> Fórmulas pendientes
          @if ((stats()?.formulas_pendientes ?? 0) > 0) {
            <span class="badge-count">{{ stats()!.formulas_pendientes }}</span>
          }
        </button>
        <button class="tab-btn" [class.active]="tab() === 'dispensaciones'" (click)="cambiarTab('dispensaciones')">
          <i class="pi pi-check-circle"></i> Dispensaciones
        </button>
        <button class="tab-btn" [class.active]="tab() === 'movimientos'" (click)="cambiarTab('movimientos')">
          <i class="pi pi-arrow-right-arrow-left"></i> Movimientos
        </button>
        <button class="tab-btn" [class.active]="tab() === 'proveedores'" (click)="cambiarTab('proveedores')">
          <i class="pi pi-truck"></i> Proveedores
        </button>
      </div>

      <!-- ═══════════════════ TAB INVENTARIO ═══════════════════ -->
      @if (tab() === 'inventario') {
        <div class="filter-bar">
          <mat-form-field appearance="outline" subscriptSizing="dynamic" class="search-input">
            <mat-label>Buscar</mat-label>
            <input matInput [(ngModel)]="busqInventario" placeholder="Buscar medicamento..."
              (input)="cargarInventario()" />
          </mat-form-field>
          <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:150px">
            <mat-label>Tipo</mat-label>
            <mat-select [(ngModel)]="filtroTipo" (selectionChange)="cargarInventario()">
              <mat-option [value]="''">Todos</mat-option>
              @for (o of opcionesTipo; track o.value) {
                <mat-option [value]="o.value">{{ o.label }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
          <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:150px">
            <mat-label>Ubicación</mat-label>
            <mat-select [(ngModel)]="filtroUbicacion" (selectionChange)="cargarInventario()">
              <mat-option [value]="''">Todas</mat-option>
              @for (o of opcionesUbicacion; track o.value) {
                <mat-option [value]="o.value">{{ o.label }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
          <button matButton="outlined" [class.aviso-color]="soloAlertas" (click)="toggleAlertas()">
            <i class="pi pi-bell"></i> {{ soloAlertas ? 'Ver todo' : 'Solo alertas' }}
          </button>
        </div>

        @if (cargandoInv()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (inventario().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-inbox" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay items en el inventario.</p>
          </div>
        } @else {
          <table mat-table [dataSource]="inventario()" class="tabla">
            <ng-container matColumnDef="nombre">
              <th mat-header-cell *matHeaderCellDef>Medicamento / Item</th>
              <td mat-cell *matCellDef="let item" [class.row-alerta]="item.alerta_stock || item.alerta_vencimiento">
                <strong>{{ item.nombre }}</strong>
                @if (item.presentacion) {
                  <div style="font-size:0.75rem;color:#94a3b8">{{ item.presentacion }}</div>
                }
                @if (item.controlado) { <span class="tag-controlado">Controlado</span> }
              </td>
            </ng-container>
            <ng-container matColumnDef="tipo">
              <th mat-header-cell *matHeaderCellDef>Tipo</th>
              <td mat-cell *matCellDef="let item"><x-etiqueta [value]="item.tipo" severity="secondary" /></td>
            </ng-container>
            <ng-container matColumnDef="stock">
              <th mat-header-cell *matHeaderCellDef class="centro">Stock</th>
              <td mat-cell *matCellDef="let item" class="centro">
                <span [class]="item.alerta_stock ? 'stock-bajo' : 'stock-ok'">
                  {{ item.cantidad_actual }} {{ item.unidad_medida }}
                </span>
              </td>
            </ng-container>
            <ng-container matColumnDef="minimo">
              <th mat-header-cell *matHeaderCellDef class="centro">Mínimo</th>
              <td mat-cell *matCellDef="let item" class="centro" style="font-size:0.82rem;color:#94a3b8">
                {{ item.cantidad_minima }}
              </td>
            </ng-container>
            <ng-container matColumnDef="lote">
              <th mat-header-cell *matHeaderCellDef>Lote</th>
              <td mat-cell *matCellDef="let item" style="font-size:0.82rem">{{ item.lote || '—' }}</td>
            </ng-container>
            <ng-container matColumnDef="vencimiento">
              <th mat-header-cell *matHeaderCellDef>Vencimiento</th>
              <td mat-cell *matCellDef="let item">
                @if (item.fecha_vencimiento) {
                  <span [class]="item.alerta_vencimiento ? 'venc-alerta' : 'venc-ok'">
                    {{ item.fecha_vencimiento | date:'dd/MM/yyyy' }}
                  </span>
                } @else {
                  <span style="color:#64748b">—</span>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="ubicacion">
              <th mat-header-cell *matHeaderCellDef>Ubicación</th>
              <td mat-cell *matCellDef="let item" style="font-size:0.82rem">{{ item.ubicacion }}</td>
            </ng-container>
            <ng-container matColumnDef="costo">
              <th mat-header-cell *matHeaderCellDef class="derecha">Costo unit.</th>
              <td mat-cell *matCellDef="let item" class="derecha" style="font-size:0.82rem">
                {{ item.costo_unitario | currency:'COP':'symbol-narrow':'1.0-0' }}
              </td>
            </ng-container>
            <ng-container matColumnDef="acciones">
              <th mat-header-cell *matHeaderCellDef class="centro">Acciones</th>
              <td mat-cell *matCellDef="let item" class="centro" style="white-space:nowrap">
                <button matIconButton class="exito-texto" matTooltip="Entrada de stock"
                  (click)="abrirEntrada(item)">
                  <i class="pi pi-arrow-down"></i>
                </button>
                <button matIconButton matTooltip="Editar" (click)="abrirEditar(item)">
                  <i class="pi pi-pencil"></i>
                </button>
                <button matIconButton class="peligro" matTooltip="Eliminar" (click)="eliminarItem(item)">
                  <i class="pi pi-trash"></i>
                </button>
              </td>
            </ng-container>

            <tr mat-header-row *matHeaderRowDef="columnasInventario"></tr>
            <tr mat-row *matRowDef="let fila; columns: columnasInventario"></tr>
          </table>
        }
      }

      <!-- ═══════════════════ TAB FÓRMULAS ═══════════════════ -->
      @if (tab() === 'formulas') {
        <div class="filter-bar">
          <mat-form-field appearance="outline" subscriptSizing="dynamic" class="search-input">
            <mat-label>Buscar</mat-label>
            <input matInput [(ngModel)]="busqFormulas" placeholder="Buscar paciente o medicamento..."
              (input)="cargarFormulas()" />
          </mat-form-field>
        </div>

        @if (cargandoForm()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (formulas().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-check-circle" style="font-size:2rem;opacity:0.4;color:#4ade80"></i>
            <p>No hay fórmulas pendientes de dispensación.</p>
          </div>
        } @else {
          <table mat-table [dataSource]="formulas()" class="tabla">
            <ng-container matColumnDef="paciente">
              <th mat-header-cell *matHeaderCellDef>Paciente</th>
              <td mat-cell *matCellDef="let f"><strong>{{ f.paciente_nombre || '—' }}</strong></td>
            </ng-container>
            <ng-container matColumnDef="medicamento">
              <th mat-header-cell *matHeaderCellDef>Medicamento</th>
              <td mat-cell *matCellDef="let f">
                <span>{{ f.medicamento }}</span>
                @if (f.codigo_cum) {
                  <div style="font-size:0.75rem;color:#94a3b8">CUM: {{ f.codigo_cum }}</div>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="dosis">
              <th mat-header-cell *matHeaderCellDef>Dosis</th>
              <td mat-cell *matCellDef="let f" style="font-size:0.85rem">{{ f.dosis }}</td>
            </ng-container>
            <ng-container matColumnDef="cantidad">
              <th mat-header-cell *matHeaderCellDef class="centro">Cantidad</th>
              <td mat-cell *matCellDef="let f" class="centro">
                <span class="dur-badge">{{ f.cantidad }} {{ f.unidad || 'und' }}</span>
              </td>
            </ng-container>
            <ng-container matColumnDef="medico">
              <th mat-header-cell *matHeaderCellDef>Médico</th>
              <td mat-cell *matCellDef="let f" style="font-size:0.82rem;color:#94a3b8">
                {{ f.medico_nombre || '—' }}
              </td>
            </ng-container>
            <ng-container matColumnDef="fecha">
              <th mat-header-cell *matHeaderCellDef>Fecha fórmula</th>
              <td mat-cell *matCellDef="let f" style="font-size:0.82rem">
                {{ f.fecha_formula | date:'dd/MM/yyyy HH:mm' }}
              </td>
            </ng-container>
            <ng-container matColumnDef="dispensar">
              <th mat-header-cell *matHeaderCellDef class="centro">Dispensar</th>
              <td mat-cell *matCellDef="let f" class="centro">
                <button matButton="filled" class="exito" (click)="abrirDispensar(f)">
                  <i class="pi pi-send"></i> Dispensar
                </button>
              </td>
            </ng-container>

            <tr mat-header-row *matHeaderRowDef="columnasFormulas"></tr>
            <tr mat-row *matRowDef="let fila; columns: columnasFormulas"></tr>
          </table>
        }
      }

      <!-- ═══════════════════ TAB DISPENSACIONES ═══════════════════ -->
      @if (tab() === 'dispensaciones') {
        @if (cargandoDisp()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (dispensaciones().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-inbox" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay dispensaciones registradas.</p>
          </div>
        } @else {
          <table mat-table [dataSource]="dispensaciones()" class="tabla">
            <ng-container matColumnDef="paciente">
              <th mat-header-cell *matHeaderCellDef>Paciente</th>
              <td mat-cell *matCellDef="let d"><strong>{{ d.paciente_nombre || '—' }}</strong></td>
            </ng-container>
            <ng-container matColumnDef="medicamento">
              <th mat-header-cell *matHeaderCellDef>Medicamento</th>
              <td mat-cell *matCellDef="let d" style="font-size:0.85rem">{{ d.medicamento_nombre || '—' }}</td>
            </ng-container>
            <ng-container matColumnDef="item">
              <th mat-header-cell *matHeaderCellDef>Item dispensado</th>
              <td mat-cell *matCellDef="let d" style="font-size:0.85rem">{{ d.item_nombre || '—' }}</td>
            </ng-container>
            <ng-container matColumnDef="cantidad">
              <th mat-header-cell *matHeaderCellDef class="centro">Cantidad</th>
              <td mat-cell *matCellDef="let d" class="centro">
                <span class="dur-badge">{{ d.cantidad_dispensada }}</span>
              </td>
            </ng-container>
            <ng-container matColumnDef="lote">
              <th mat-header-cell *matHeaderCellDef>Lote</th>
              <td mat-cell *matCellDef="let d" style="font-size:0.82rem;color:#94a3b8">
                {{ d.lote_dispensado || '—' }}
              </td>
            </ng-container>
            <ng-container matColumnDef="regente">
              <th mat-header-cell *matHeaderCellDef>Regente</th>
              <td mat-cell *matCellDef="let d" style="font-size:0.82rem;color:#94a3b8">
                {{ d.regente_nombre || '—' }}
              </td>
            </ng-container>
            <ng-container matColumnDef="fecha">
              <th mat-header-cell *matHeaderCellDef>Fecha</th>
              <td mat-cell *matCellDef="let d" style="font-size:0.82rem">
                {{ d.fecha_dispensacion | date:'dd/MM/yyyy HH:mm' }}
              </td>
            </ng-container>

            <tr mat-header-row *matHeaderRowDef="columnasDispensaciones"></tr>
            <tr mat-row *matRowDef="let fila; columns: columnasDispensaciones"></tr>
          </table>
        }
      }

      <!-- ═══════════════════ TAB MOVIMIENTOS ═══════════════════ -->
      @if (tab() === 'movimientos') {
        <div class="filter-bar">
          <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:180px">
            <mat-label>Tipo de movimiento</mat-label>
            <mat-select [(ngModel)]="filtroMovTipo" (selectionChange)="cargarMovimientos()">
              <mat-option [value]="''">Todos</mat-option>
              @for (o of opcionesMovTipo; track o.value) {
                <mat-option [value]="o.value">{{ o.label }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
        </div>

        @if (cargandoMov()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (movimientos().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-inbox" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay movimientos registrados.</p>
          </div>
        } @else {
          <table mat-table [dataSource]="movimientos()" class="tabla">
            <ng-container matColumnDef="item">
              <th mat-header-cell *matHeaderCellDef>Item</th>
              <td mat-cell *matCellDef="let m"><strong>{{ m.item_nombre || '—' }}</strong></td>
            </ng-container>
            <ng-container matColumnDef="tipo">
              <th mat-header-cell *matHeaderCellDef class="centro">Tipo</th>
              <td mat-cell *matCellDef="let m" class="centro">
                <x-etiqueta [value]="m.tipo_movimiento" [severity]="movSeverity(m.tipo_movimiento)" />
              </td>
            </ng-container>
            <ng-container matColumnDef="cantidad">
              <th mat-header-cell *matHeaderCellDef class="centro">Cantidad</th>
              <td mat-cell *matCellDef="let m" class="centro">
                <span [class]="m.tipo_movimiento === 'ENTRADA' ? 'cant-entrada' : 'cant-salida'">
                  {{ m.tipo_movimiento === 'ENTRADA' ? '+' : '-' }}{{ m.cantidad }}
                </span>
              </td>
            </ng-container>
            <ng-container matColumnDef="motivo">
              <th mat-header-cell *matHeaderCellDef>Motivo</th>
              <td mat-cell *matCellDef="let m" style="font-size:0.82rem">{{ m.motivo }}</td>
            </ng-container>
            <ng-container matColumnDef="procedencia">
              <th mat-header-cell *matHeaderCellDef>Proveedor / Soporte</th>
              <td mat-cell *matCellDef="let m" style="font-size:0.82rem;color:#94a3b8">
                <!-- Un guion, no un espacio en blanco: «no vino de una compra»
                     y «se me olvidó registrarlo» se ven igual si no se dice. -->
                {{ m.proveedor_nombre || '—' }}
                @if (m.documento_soporte) {
                  <div style="font-size:0.72rem;color:#64748b">{{ m.documento_soporte }}</div>
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="usuario">
              <th mat-header-cell *matHeaderCellDef>Usuario</th>
              <td mat-cell *matCellDef="let m" style="font-size:0.82rem;color:#94a3b8">
                {{ m.usuario_nombre || '—' }}
              </td>
            </ng-container>
            <ng-container matColumnDef="fecha">
              <th mat-header-cell *matHeaderCellDef>Fecha</th>
              <td mat-cell *matCellDef="let m" style="font-size:0.82rem">
                {{ m.fecha | date:'dd/MM/yyyy HH:mm' }}
              </td>
            </ng-container>

            <tr mat-header-row *matHeaderRowDef="columnasMovimientos"></tr>
            <tr mat-row *matRowDef="let fila; columns: columnasMovimientos"></tr>
          </table>
        }
      }

      <!-- ═══════════════════ TAB PROVEEDORES ═══════════════════ -->
      @if (tab() === 'proveedores') {
        <div class="filter-bar">
          <mat-form-field appearance="outline" subscriptSizing="dynamic" class="search-input">
            <mat-label>Buscar</mat-label>
            <input matInput [(ngModel)]="busqProveedores" placeholder="Razón social, NIT o contacto..."
              (input)="cargarProveedores()" />
          </mat-form-field>
        </div>

        @if (cargandoProv()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (proveedores().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-truck" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay proveedores registrados.</p>
            <small style="color:#64748b">
              Sin ellos, una entrada dice cuánto entró pero no de quién vino.
            </small>
          </div>
        } @else {
          <table mat-table [dataSource]="proveedores()" class="tabla">
            <ng-container matColumnDef="razon">
              <th mat-header-cell *matHeaderCellDef>Razón social</th>
              <td mat-cell *matCellDef="let p" [style.opacity]="p.activo ? 1 : 0.55">
                <strong>{{ p.razon_social }}</strong>
                @if (p.ciudad) { <div style="font-size:0.75rem;color:#94a3b8">{{ p.ciudad }}</div> }
              </td>
            </ng-container>
            <ng-container matColumnDef="nit">
              <th mat-header-cell *matHeaderCellDef>NIT</th>
              <td mat-cell *matCellDef="let p" style="font-size:0.82rem;color:#94a3b8"
                [style.opacity]="p.activo ? 1 : 0.55">{{ p.nit || '—' }}</td>
            </ng-container>
            <ng-container matColumnDef="contacto">
              <th mat-header-cell *matHeaderCellDef>Contacto</th>
              <td mat-cell *matCellDef="let p" style="font-size:0.82rem"
                [style.opacity]="p.activo ? 1 : 0.55">
                {{ p.contacto || '—' }}
                @if (p.telefono) { <div style="font-size:0.72rem;color:#64748b">{{ p.telefono }}</div> }
              </td>
            </ng-container>
            <ng-container matColumnDef="suministra">
              <th mat-header-cell *matHeaderCellDef>Suministra</th>
              <td mat-cell *matCellDef="let p" style="font-size:0.82rem;color:#94a3b8"
                [style.opacity]="p.activo ? 1 : 0.55">{{ p.suministra || '—' }}</td>
            </ng-container>
            <ng-container matColumnDef="remesas">
              <th mat-header-cell *matHeaderCellDef class="centro">Remesas</th>
              <td mat-cell *matCellDef="let p" class="centro"><span class="dur-badge">{{ p.entradas }}</span></td>
            </ng-container>
            <ng-container matColumnDef="estado">
              <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
              <td mat-cell *matCellDef="let p" class="centro">
                <x-etiqueta [value]="p.activo ? 'Activo' : 'Inactivo'"
                  [severity]="p.activo ? 'success' : 'secondary'" />
              </td>
            </ng-container>
            <ng-container matColumnDef="acciones">
              <th mat-header-cell *matHeaderCellDef style="width:110px"></th>
              <td mat-cell *matCellDef="let p" style="text-align:right">
                <button matIconButton matTooltip="Editar" (click)="abrirEditarProveedor(p)">
                  <i class="pi pi-pencil"></i>
                </button>
                <!-- Con remesas registradas no se ofrece borrar: se desactiva.
                     Ofrecer el botón para que el servidor conteste 409 es hacer
                     que el usuario descubra la regla chocándose con ella. -->
                @if (p.entradas === 0) {
                  <button matIconButton class="peligro" matTooltip="Eliminar" (click)="eliminarProveedor(p)">
                    <i class="pi pi-trash"></i>
                  </button>
                } @else {
                  <button matIconButton [class.aviso-color]="p.activo" [class.exito-texto]="!p.activo"
                    [matTooltip]="p.activo ? 'Desactivar' : 'Reactivar'" (click)="alternarProveedor(p)">
                    <i [class]="p.activo ? 'pi pi-ban' : 'pi pi-check'"></i>
                  </button>
                }
              </td>
            </ng-container>

            <tr mat-header-row *matHeaderRowDef="columnasProveedores"></tr>
            <tr mat-row *matRowDef="let fila; columns: columnasProveedores"></tr>
          </table>
        }
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
      flex: 1; min-width: 100px;
      background: rgba(255,255,255,0.04); border: 1px solid rgba(255,255,255,0.08);
      border-radius: 8px; padding: 0.75rem 1rem;
    }
    .stat-card.stat-red { border-color: rgba(248,113,113,0.2); background: rgba(248,113,113,0.05); }
    .stat-card.stat-yellow { border-color: rgba(251,191,36,0.2); background: rgba(251,191,36,0.05); }
    .stat-card.stat-purple { border-color: rgba(167,139,250,0.2); background: rgba(167,139,250,0.05); }
    .stat-card.stat-green { border-color: rgba(74,222,128,0.2); background: rgba(74,222,128,0.05); }
    .stat-value { font-size: 1.5rem; font-weight: 700; color: #f1f5f9; line-height: 1; }
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
    .search-input { width: 260px; }

    .stock-ok { color: #4ade80; font-weight: 600; }
    .stock-bajo { color: #f87171; font-weight: 700; }
    .venc-ok { font-size: 0.82rem; color: #e2e8f0; }
    .venc-alerta { font-size: 0.82rem; color: #fbbf24; font-weight: 600; }
    .tag-controlado {
      display: inline-block; margin-left: 0.35rem;
      background: rgba(239,68,68,0.15); color: #f87171;
      border: 1px solid rgba(239,68,68,0.3); border-radius: 3px;
      font-size: 0.65rem; font-weight: 700; padding: 0.05rem 0.35rem; text-transform: uppercase;
    }
    ::ng-deep .row-alerta td { background: rgba(251,191,36,0.04) !important; }

    .dur-badge {
      display: inline-block; padding: 0.15rem 0.5rem; border-radius: 999px;
      background: rgba(255,255,255,0.06); color: #e2e8f0; font-size: 0.82rem;
    }
    .cant-entrada { color: #4ade80; font-weight: 700; }
    .cant-salida { color: #f87171; font-weight: 700; }

    .form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
    .field { display: flex; flex-direction: column; gap: 0.35rem; }
    .field-label { font-weight: 500; font-size: 0.875rem; color: #e2e8f0; }
    .w-full { width: 100%; }
    .date-input {
      background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.15);
      border-radius: 6px; padding: 0.5rem 0.75rem; color: #f1f5f9; font-size: 0.875rem;
      color-scheme: dark;
    }
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
export class FarmaciaComponent implements OnInit {
  private svc = inject(FarmaciaService);
  private dialogo = inject(MatDialog);
  private messageService = inject(Avisos);

  readonly String = String;

  tab = signal<Tab>('inventario');
  stats = signal<StatsInventario | null>(null);

  // Inventario
  cargandoInv = signal(false);
  inventario = signal<ItemInventario[]>([]);
  busqInventario = '';
  filtroTipo = '';
  filtroUbicacion = '';
  soloAlertas = false;
  private _invTimer: any;

  // Fórmulas
  cargandoForm = signal(false);
  formulas = signal<FormulaPendiente[]>([]);
  busqFormulas = '';
  private _formTimer: any;

  // Dispensaciones
  cargandoDisp = signal(false);
  dispensaciones = signal<Dispensacion[]>([]);

  // Movimientos
  cargandoMov = signal(false);
  movimientos = signal<Movimiento[]>([]);
  filtroMovTipo = '';

  // Proveedores
  proveedores = signal<Proveedor[]>([]);
  cargandoProv = signal(false);
  busqProveedores = '';
  private _provTimer: ReturnType<typeof setTimeout> | undefined;

  /** Solo los activos van al selector de la entrada: a un proveedor con el que
   *  ya no se trabaja no se le apunta una remesa nueva. */
  proveedoresActivos = computed(() =>
    this.proveedores()
      .filter(p => p.activo)
      .map(p => ({ label: p.nit ? `${p.razon_social} — ${p.nit}` : p.razon_social, value: p.id }))
  );

  inventarioParaDispensa = computed(() =>
    this.inventario().map(i => ({
      label: `${i.nombre} — Stock: ${i.cantidad_actual} ${i.unidad_medida}`,
      value: i.id,
    }))
  );

  readonly opcionesTipo = TIPOS;
  readonly opcionesUbicacion = UBICACIONES;

  /** El orden de las columnas lo fija esta lista, no la plantilla. */
  readonly columnasInventario = ['nombre', 'tipo', 'stock', 'minimo', 'lote',
                                 'vencimiento', 'ubicacion', 'costo', 'acciones'];
  readonly columnasFormulas = ['paciente', 'medicamento', 'dosis', 'cantidad',
                               'medico', 'fecha', 'dispensar'];
  readonly columnasDispensaciones = ['paciente', 'medicamento', 'item', 'cantidad',
                                     'lote', 'regente', 'fecha'];
  readonly columnasMovimientos = ['item', 'tipo', 'cantidad', 'motivo', 'procedencia',
                                  'usuario', 'fecha'];
  readonly columnasProveedores = ['razon', 'nit', 'contacto', 'suministra', 'remesas',
                                  'estado', 'acciones'];

  readonly opcionesMovTipo = [
    { label: 'Entradas', value: 'ENTRADA' },
    { label: 'Salidas', value: 'SALIDA' },
    { label: 'Ajustes', value: 'AJUSTE' },
    { label: 'Vencimiento', value: 'VENCIMIENTO' },
  ];

  ngOnInit(): void {
    this.cargarStats();
    this.cargarInventario();
  }

  cambiarTab(t: Tab): void {
    this.tab.set(t);
    if (t === 'inventario' && this.inventario().length === 0) this.cargarInventario();
    if (t === 'formulas') this.cargarFormulas();
    if (t === 'dispensaciones') this.cargarDispensaciones();
    if (t === 'movimientos') this.cargarMovimientos();
    if (t === 'proveedores') this.cargarProveedores();
  }

  // ── Proveedores ──────────────────────────────────────

  cargarProveedores(): void {
    clearTimeout(this._provTimer);
    this._provTimer = setTimeout(() => {
      this.cargandoProv.set(true);
      this.svc.listProveedores(this.busqProveedores || undefined).subscribe({
        next: ps => { this.proveedores.set(ps); this.cargandoProv.set(false); },
        error: () => this.cargandoProv.set(false),
      });
    }, 300);
  }

  abrirCrearProveedor(): void { this.abrirProveedor(null); }
  abrirEditarProveedor(p: Proveedor): void { this.abrirProveedor(p); }

  private abrirProveedor(p: Proveedor | null): void {
    this.dialogo
      .open(FarmaciaProveedorComponent, { data: p, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((datos?: Partial<ProveedorCreate>) => { if (datos) this.guardarProveedor(p, datos); });
  }

  private guardarProveedor(editando: Proveedor | null, datos: Partial<ProveedorCreate>): void {
    const peticion = editando
      ? this.svc.actualizarProveedor(editando.id, datos)
      : this.svc.crearProveedor(datos as ProveedorCreate);
    peticion.subscribe({
      next: () => {
        this.ok(editando ? 'Proveedor actualizado' : 'Proveedor creado', datos.razon_social!);
        this.cargarProveedores();
      },
      error: err => this.err(err?.error?.detail ?? 'No se pudo guardar el proveedor'),
    });
  }

  /** Desactivar o reactivar. Es lo que sustituye al borrado cuando ya trajo remesas. */
  alternarProveedor(p: Proveedor): void {
    this.svc.actualizarProveedor(p.id, { activo: !p.activo }).subscribe({
      next: () => {
        this.ok(p.activo ? 'Proveedor desactivado' : 'Proveedor reactivado', p.razon_social);
        this.cargarProveedores();
      },
      error: err => this.err(err?.error?.detail ?? 'No se pudo cambiar el estado'),
    });
  }

  eliminarProveedor(p: Proveedor): void {
    this.svc.eliminarProveedor(p.id).subscribe({
      next: () => { this.ok('Proveedor eliminado', p.razon_social); this.cargarProveedores(); },
      error: err => this.err(err?.error?.detail ?? 'No se pudo eliminar'),
    });
  }

  cargarStats(): void {
    this.svc.getStats().subscribe({ next: s => this.stats.set(s) });
  }

  cargarInventario(): void {
    clearTimeout(this._invTimer);
    this._invTimer = setTimeout(() => {
      this.cargandoInv.set(true);
      const params: Record<string, string | boolean> = {};
      if (this.busqInventario) params['q'] = this.busqInventario;
      if (this.filtroTipo) params['tipo'] = this.filtroTipo;
      if (this.filtroUbicacion) params['ubicacion'] = this.filtroUbicacion;
      if (this.soloAlertas) params['solo_alertas'] = true;
      this.svc.listInventario(params).subscribe({
        next: items => { this.inventario.set(items); this.cargandoInv.set(false); },
        error: () => this.cargandoInv.set(false),
      });
    }, 300);
  }

  cargarFormulas(): void {
    clearTimeout(this._formTimer);
    this._formTimer = setTimeout(() => {
      this.cargandoForm.set(true);
      this.svc.listFormulasPendientes(this.busqFormulas || undefined).subscribe({
        next: fs => { this.formulas.set(fs); this.cargandoForm.set(false); },
        error: () => this.cargandoForm.set(false),
      });
    }, 300);
  }

  cargarDispensaciones(): void {
    this.cargandoDisp.set(true);
    this.svc.listDispensaciones().subscribe({
      next: ds => { this.dispensaciones.set(ds); this.cargandoDisp.set(false); },
      error: () => this.cargandoDisp.set(false),
    });
  }

  cargarMovimientos(): void {
    this.cargandoMov.set(true);
    this.svc.listMovimientos(undefined, this.filtroMovTipo || undefined).subscribe({
      next: ms => { this.movimientos.set(ms); this.cargandoMov.set(false); },
      error: () => this.cargandoMov.set(false),
    });
  }

  toggleAlertas(): void {
    this.soloAlertas = !this.soloAlertas;
    this.cargarInventario();
  }

  // ── CRUD Inventario ──────────────────────────────

  abrirCrear(): void { this.abrirItem(null); }
  abrirEditar(item: ItemInventario): void { this.abrirItem(item); }

  private abrirItem(item: ItemInventario | null): void {
    this.dialogo
      .open(FarmaciaItemComponent, { data: item, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((datos?: Partial<ItemInventarioCreate>) => { if (datos) this.guardarItem(item, datos); });
  }

  private guardarItem(editando: ItemInventario | null, datos: Partial<ItemInventarioCreate>): void {
    const obs = editando
      ? this.svc.actualizarItem(editando.id, datos as ItemInventarioCreate)
      : this.svc.crearItem(datos as ItemInventarioCreate);
    obs.subscribe({
      next: () => {
        this.ok(editando ? 'Item actualizado' : 'Item creado');
        this.cargarInventario();
        this.cargarStats();
      },
      error: err => this.err(err?.error?.detail ?? 'Error al guardar'),
    });
  }

  eliminarItem(item: ItemInventario): void {
    if (item.cantidad_actual > 0) {
      this.err(`No se puede eliminar: tiene ${item.cantidad_actual} unidades en stock`);
      return;
    }
    this.svc.eliminarItem(item.id).subscribe({
      next: () => { this.ok('Item eliminado'); this.cargarInventario(); this.cargarStats(); },
      error: err => this.err(err?.error?.detail ?? 'Error al eliminar'),
    });
  }

  // ── Entrada stock ────────────────────────────────

  /**
   * El selector de proveedor tiene que llegar poblado al diálogo: si llega
   * vacío, quien registra concluye «no hay proveedores» y guarda la entrada sin
   * procedencia. Por eso se piden ANTES de abrirlo si aún no están.
   */
  abrirEntrada(item: ItemInventario): void {
    if (this.proveedores().length === 0) {
      this.svc.listProveedores().subscribe({
        next: ps => { this.proveedores.set(ps); this.abrirDialogoEntrada(item); },
        error: () => this.abrirDialogoEntrada(item),
      });
      return;
    }
    this.abrirDialogoEntrada(item);
  }

  private abrirDialogoEntrada(item: ItemInventario): void {
    this.dialogo
      .open(FarmaciaEntradaComponent, {
        data: { item, proveedores: this.proveedoresActivos() },
        maxWidth: '95vw',
      })
      .afterClosed()
      .subscribe((entrada?: EntradaStock) => { if (entrada) this.registrarEntrada(item, entrada); });
  }

  private registrarEntrada(item: ItemInventario, entrada: EntradaStock): void {
    this.svc.entradaStock(item.id, entrada).subscribe({
      next: updated => {
        this.ok('Entrada registrada', `Stock actualizado: ${updated.cantidad_actual} ${updated.unidad_medida}`);
        this.cargarInventario();
        this.cargarStats();
      },
      error: err => this.err(err?.error?.detail ?? 'Error al registrar entrada'),
    });
  }

  // ── Dispensación ─────────────────────────────────

  /** Mismo motivo que la entrada: sin inventario cargado el selector saldría
   *  vacío y no habría nada que dispensar. */
  abrirDispensar(f: FormulaPendiente): void {
    if (this.inventario().length === 0) {
      this.svc.listInventario({}).subscribe({
        next: items => { this.inventario.set(items); this.abrirDialogoDispensar(f); },
        error: () => this.abrirDialogoDispensar(f),
      });
      return;
    }
    this.abrirDialogoDispensar(f);
  }

  private abrirDialogoDispensar(f: FormulaPendiente): void {
    const lotes = new Map(this.inventario().map(i => [i.id, i.lote ?? '']));
    this.dialogo
      .open(FarmaciaDispensarComponent, {
        data: { formula: f, items: this.inventarioParaDispensa(), lotes },
        maxWidth: '95vw',
      })
      .afterClosed()
      .subscribe((d?: DispensarCreate) => { if (d) this.dispensar(d); });
  }

  private dispensar(datos: DispensarCreate): void {
    this.svc.dispensar(datos).subscribe({
      next: () => {
        this.ok('Dispensación registrada', 'Medicamento entregado al paciente');
        this.cargarFormulas();
        this.cargarInventario();
        this.cargarStats();
      },
      error: err => this.err(err?.error?.detail ?? 'Error al dispensar'),
    });
  }

  movSeverity(tipo: string): 'success' | 'danger' | 'warn' | 'secondary' {
    if (tipo === 'ENTRADA') return 'success';
    if (tipo === 'SALIDA') return 'danger';
    if (tipo === 'AJUSTE') return 'warn';
    return 'secondary';
  }

  private ok(summary: string, detail?: string): void {
    this.messageService.add({ severity: 'success', summary, detail });
  }

  private err(detail: string): void {
    this.messageService.add({ severity: 'error', summary: 'Error', detail });
  }
}
