import { Component, OnInit, inject, signal, computed } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { TextareaModule } from 'primeng/textarea';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { ToastModule } from 'primeng/toast';
import { SelectModule } from 'primeng/select';
import { TooltipModule } from 'primeng/tooltip';
import { DialogModule } from 'primeng/dialog';
import { DividerModule } from 'primeng/divider';
import { CheckboxModule } from 'primeng/checkbox';
import { MessageService } from 'primeng/api';
import {
  FarmaciaService, ItemInventario, ItemInventarioCreate,
  EntradaStock, FormulaPendiente, DispensarCreate,
  Dispensacion, Movimiento, StatsInventario,
  Proveedor, ProveedorCreate,
} from '../../services/farmacia.service';

type Tab = 'inventario' | 'formulas' | 'dispensaciones' | 'movimientos' | 'proveedores';

@Component({
  selector: 'app-farmacia',
  standalone: true,
  providers: [MessageService],
  imports: [
    CommonModule, FormsModule,
    ButtonModule, InputTextModule, TextareaModule, TableModule,
    TagModule, ToastModule, SelectModule, TooltipModule,
    DialogModule, DividerModule, CheckboxModule,
  ],
  template: `
    <div class="page-card">
      <div class="list-header">
        <h2 class="page-title">Farmacia e Inventario</h2>
        @if (tab() === 'inventario') {
          <p-button label="Nuevo item" icon="pi pi-plus" (onClick)="abrirCrear()" />
        }
        @if (tab() === 'proveedores') {
          <p-button label="Nuevo proveedor" icon="pi pi-plus" (onClick)="abrirCrearProveedor()" />
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
          <span class="p-input-icon-left">
            <i class="pi pi-search"></i>
            <input pInputText [(ngModel)]="busqInventario" placeholder="Buscar medicamento..."
              (input)="cargarInventario()" class="search-input" />
          </span>
          <p-select [(ngModel)]="filtroTipo" [options]="opcionesTipo"
            optionLabel="label" optionValue="value" [showClear]="true"
            placeholder="Tipo" style="width:150px"
            (onChange)="cargarInventario()" />
          <p-select [(ngModel)]="filtroUbicacion" [options]="opcionesUbicacion"
            optionLabel="label" optionValue="value" [showClear]="true"
            placeholder="Ubicación" style="width:150px"
            (onChange)="cargarInventario()" />
          <p-button [label]="soloAlertas ? 'Ver todo' : 'Solo alertas'"
            [severity]="soloAlertas ? 'warn' : 'secondary'"
            [outlined]="true" size="small"
            icon="pi pi-bell"
            (onClick)="toggleAlertas()" />
        </div>

        @if (cargandoInv()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (inventario().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-inbox" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay items en el inventario.</p>
          </div>
        } @else {
          <p-table [value]="inventario()" [rowHover]="true" styleClass="p-datatable-sm">
            <ng-template pTemplate="header">
              <tr>
                <th>Medicamento / Item</th>
                <th>Tipo</th>
                <th style="text-align:center">Stock</th>
                <th style="text-align:center">Mínimo</th>
                <th>Lote</th>
                <th>Vencimiento</th>
                <th>Ubicación</th>
                <th style="text-align:right">Costo unit.</th>
                <th style="text-align:center">Acciones</th>
              </tr>
            </ng-template>
            <ng-template pTemplate="body" let-item>
              <tr [class.row-alerta]="item.alerta_stock || item.alerta_vencimiento">
                <td>
                  <strong>{{ item.nombre }}</strong>
                  @if (item.presentacion) {
                    <div style="font-size:0.75rem;color:#94a3b8">{{ item.presentacion }}</div>
                  }
                  @if (item.controlado) {
                    <span class="tag-controlado">Controlado</span>
                  }
                </td>
                <td><p-tag [value]="item.tipo" severity="secondary" /></td>
                <td style="text-align:center">
                  <span [class]="item.alerta_stock ? 'stock-bajo' : 'stock-ok'">
                    {{ item.cantidad_actual }} {{ item.unidad_medida }}
                  </span>
                </td>
                <td style="text-align:center;font-size:0.82rem;color:#94a3b8">
                  {{ item.cantidad_minima }}
                </td>
                <td style="font-size:0.82rem">{{ item.lote || '—' }}</td>
                <td>
                  @if (item.fecha_vencimiento) {
                    <span [class]="item.alerta_vencimiento ? 'venc-alerta' : 'venc-ok'">
                      {{ item.fecha_vencimiento | date:'dd/MM/yyyy' }}
                    </span>
                  } @else {
                    <span style="color:#64748b">—</span>
                  }
                </td>
                <td style="font-size:0.82rem">{{ item.ubicacion }}</td>
                <td style="text-align:right;font-size:0.82rem">
                  {{ item.costo_unitario | currency:'COP':'symbol-narrow':'1.0-0' }}
                </td>
                <td style="text-align:center;white-space:nowrap">
                  <p-button icon="pi pi-arrow-down" [text]="true" size="small"
                    severity="success" pTooltip="Entrada de stock"
                    (onClick)="abrirEntrada(item)" />
                  <p-button icon="pi pi-pencil" [text]="true" size="small"
                    pTooltip="Editar" (onClick)="abrirEditar(item)" />
                  <p-button icon="pi pi-trash" [text]="true" size="small"
                    severity="danger" pTooltip="Eliminar"
                    (onClick)="eliminarItem(item)" />
                </td>
              </tr>
            </ng-template>
          </p-table>
        }
      }

      <!-- ═══════════════════ TAB FÓRMULAS ═══════════════════ -->
      @if (tab() === 'formulas') {
        <div class="filter-bar">
          <span class="p-input-icon-left">
            <i class="pi pi-search"></i>
            <input pInputText [(ngModel)]="busqFormulas" placeholder="Buscar paciente o medicamento..."
              (input)="cargarFormulas()" class="search-input" />
          </span>
        </div>

        @if (cargandoForm()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (formulas().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-check-circle" style="font-size:2rem;opacity:0.4;color:#4ade80"></i>
            <p>No hay fórmulas pendientes de dispensación.</p>
          </div>
        } @else {
          <p-table [value]="formulas()" [rowHover]="true" styleClass="p-datatable-sm">
            <ng-template pTemplate="header">
              <tr>
                <th>Paciente</th>
                <th>Medicamento</th>
                <th>Dosis</th>
                <th style="text-align:center">Cantidad</th>
                <th>Médico</th>
                <th>Fecha fórmula</th>
                <th style="text-align:center">Dispensar</th>
              </tr>
            </ng-template>
            <ng-template pTemplate="body" let-f>
              <tr>
                <td>
                  <strong>{{ f.paciente_nombre || '—' }}</strong>
                </td>
                <td>
                  <span>{{ f.medicamento }}</span>
                  @if (f.codigo_cum) {
                    <div style="font-size:0.75rem;color:#94a3b8">CUM: {{ f.codigo_cum }}</div>
                  }
                </td>
                <td style="font-size:0.85rem">{{ f.dosis }}</td>
                <td style="text-align:center">
                  <span class="dur-badge">{{ f.cantidad }} {{ f.unidad || 'und' }}</span>
                </td>
                <td style="font-size:0.82rem;color:#94a3b8">{{ f.medico_nombre || '—' }}</td>
                <td style="font-size:0.82rem">{{ f.fecha_formula | date:'dd/MM/yyyy HH:mm' }}</td>
                <td style="text-align:center">
                  <p-button icon="pi pi-send" label="Dispensar" size="small"
                    severity="success" (onClick)="abrirDispensar(f)" />
                </td>
              </tr>
            </ng-template>
          </p-table>
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
          <p-table [value]="dispensaciones()" [rowHover]="true" styleClass="p-datatable-sm">
            <ng-template pTemplate="header">
              <tr>
                <th>Paciente</th>
                <th>Medicamento</th>
                <th>Item dispensado</th>
                <th style="text-align:center">Cantidad</th>
                <th>Lote</th>
                <th>Regente</th>
                <th>Fecha</th>
              </tr>
            </ng-template>
            <ng-template pTemplate="body" let-d>
              <tr>
                <td><strong>{{ d.paciente_nombre || '—' }}</strong></td>
                <td style="font-size:0.85rem">{{ d.medicamento_nombre || '—' }}</td>
                <td style="font-size:0.85rem">{{ d.item_nombre || '—' }}</td>
                <td style="text-align:center">
                  <span class="dur-badge">{{ d.cantidad_dispensada }}</span>
                </td>
                <td style="font-size:0.82rem;color:#94a3b8">{{ d.lote_dispensado || '—' }}</td>
                <td style="font-size:0.82rem;color:#94a3b8">{{ d.regente_nombre || '—' }}</td>
                <td style="font-size:0.82rem">{{ d.fecha_dispensacion | date:'dd/MM/yyyy HH:mm' }}</td>
              </tr>
            </ng-template>
          </p-table>
        }
      }

      <!-- ═══════════════════ TAB MOVIMIENTOS ═══════════════════ -->
      @if (tab() === 'movimientos') {
        <div class="filter-bar">
          <p-select [(ngModel)]="filtroMovTipo" [options]="opcionesMovTipo"
            optionLabel="label" optionValue="value" [showClear]="true"
            placeholder="Tipo movimiento" style="width:180px"
            (onChange)="cargarMovimientos()" />
        </div>

        @if (cargandoMov()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (movimientos().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-inbox" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay movimientos registrados.</p>
          </div>
        } @else {
          <p-table [value]="movimientos()" [rowHover]="true" styleClass="p-datatable-sm">
            <ng-template pTemplate="header">
              <tr>
                <th>Item</th>
                <th style="text-align:center">Tipo</th>
                <th style="text-align:center">Cantidad</th>
                <th>Motivo</th>
                <th>Proveedor / Soporte</th>
                <th>Usuario</th>
                <th>Fecha</th>
              </tr>
            </ng-template>
            <ng-template pTemplate="body" let-m>
              <tr>
                <td><strong>{{ m.item_nombre || '—' }}</strong></td>
                <td style="text-align:center">
                  <p-tag [value]="m.tipo_movimiento" [severity]="movSeverity(m.tipo_movimiento)" />
                </td>
                <td style="text-align:center">
                  <span [class]="m.tipo_movimiento === 'ENTRADA' ? 'cant-entrada' : 'cant-salida'">
                    {{ m.tipo_movimiento === 'ENTRADA' ? '+' : '-' }}{{ m.cantidad }}
                  </span>
                </td>
                <td style="font-size:0.82rem">{{ m.motivo }}</td>
                <td style="font-size:0.82rem;color:#94a3b8">
                  <!-- Un guion, no un espacio en blanco: «no vino de una compra»
                       y «se me olvidó registrarlo» se ven igual si no se dice. -->
                  {{ m.proveedor_nombre || '—' }}
                  @if (m.documento_soporte) {
                    <div style="font-size:0.72rem;color:#64748b">{{ m.documento_soporte }}</div>
                  }
                </td>
                <td style="font-size:0.82rem;color:#94a3b8">{{ m.usuario_nombre || '—' }}</td>
                <td style="font-size:0.82rem">{{ m.fecha | date:'dd/MM/yyyy HH:mm' }}</td>
              </tr>
            </ng-template>
          </p-table>
        }
      }

      <!-- ═══════════════════ TAB PROVEEDORES ═══════════════════ -->
      @if (tab() === 'proveedores') {
        <div class="filter-bar">
          <span class="p-input-icon-left">
            <i class="pi pi-search"></i>
            <input pInputText [(ngModel)]="busqProveedores" placeholder="Razón social, NIT o contacto..."
              (input)="cargarProveedores()" class="search-input" />
          </span>
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
          <p-table [value]="proveedores()" [rowHover]="true" styleClass="p-datatable-sm">
            <ng-template pTemplate="header">
              <tr>
                <th>Razón social</th>
                <th>NIT</th>
                <th>Contacto</th>
                <th>Suministra</th>
                <th style="text-align:center">Remesas</th>
                <th style="text-align:center">Estado</th>
                <th style="width:110px"></th>
              </tr>
            </ng-template>
            <ng-template pTemplate="body" let-p>
              <tr [style.opacity]="p.activo ? 1 : 0.55">
                <td>
                  <strong>{{ p.razon_social }}</strong>
                  @if (p.ciudad) {
                    <div style="font-size:0.75rem;color:#94a3b8">{{ p.ciudad }}</div>
                  }
                </td>
                <td style="font-size:0.82rem;color:#94a3b8">{{ p.nit || '—' }}</td>
                <td style="font-size:0.82rem">
                  {{ p.contacto || '—' }}
                  @if (p.telefono) {
                    <div style="font-size:0.72rem;color:#64748b">{{ p.telefono }}</div>
                  }
                </td>
                <td style="font-size:0.82rem;color:#94a3b8">{{ p.suministra || '—' }}</td>
                <td style="text-align:center"><span class="dur-badge">{{ p.entradas }}</span></td>
                <td style="text-align:center">
                  <p-tag [value]="p.activo ? 'Activo' : 'Inactivo'"
                    [severity]="p.activo ? 'success' : 'secondary'" />
                </td>
                <td style="text-align:right">
                  <p-button icon="pi pi-pencil" [text]="true" size="small"
                    pTooltip="Editar" (onClick)="abrirEditarProveedor(p)" />
                  <!-- Con remesas registradas no se ofrece borrar: se desactiva.
                       Ofrecer el botón para que el servidor conteste 409 es hacer
                       que el usuario descubra la regla chocándose con ella. -->
                  @if (p.entradas === 0) {
                    <p-button icon="pi pi-trash" [text]="true" size="small" severity="danger"
                      pTooltip="Eliminar" (onClick)="eliminarProveedor(p)" />
                  } @else {
                    <p-button [icon]="p.activo ? 'pi pi-ban' : 'pi pi-check'" [text]="true" size="small"
                      [severity]="p.activo ? 'warn' : 'success'"
                      [pTooltip]="p.activo ? 'Desactivar' : 'Reactivar'"
                      (onClick)="alternarProveedor(p)" />
                  }
                </td>
              </tr>
            </ng-template>
          </p-table>
        }
      }
    </div>

    <!-- ══ DIALOG CREAR / EDITAR PROVEEDOR ══ -->
    <p-dialog [header]="editandoProveedor ? 'Editar proveedor' : 'Nuevo proveedor'"
      [(visible)]="mostrarFormProveedor" [style]="{width:'620px'}" [modal]="true">
      <div class="form-grid" style="gap:0.875rem">
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Razón social *</label>
          <input pInputText [(ngModel)]="pForm.razon_social" class="w-full"
            placeholder="Ej: Distribuidora Andina S.A.S." />
        </div>
        <div class="field">
          <label class="field-label">NIT</label>
          <input pInputText [(ngModel)]="pForm.nit" class="w-full"
            placeholder="Opcional: no todo proveedor tiene NIT" />
        </div>
        <div class="field">
          <label class="field-label">Suministra</label>
          <input pInputText [(ngModel)]="pForm.suministra" class="w-full"
            placeholder="Medicamentos, insumos, reactivos..." />
        </div>
        <div class="field">
          <label class="field-label">Contacto</label>
          <input pInputText [(ngModel)]="pForm.contacto" class="w-full" />
        </div>
        <div class="field">
          <label class="field-label">Teléfono</label>
          <input pInputText [(ngModel)]="pForm.telefono" class="w-full" />
        </div>
        <div class="field">
          <label class="field-label">Correo</label>
          <input pInputText [(ngModel)]="pForm.email" class="w-full" />
        </div>
        <div class="field">
          <label class="field-label">Ciudad</label>
          <input pInputText [(ngModel)]="pForm.ciudad" class="w-full" />
        </div>
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Dirección</label>
          <input pInputText [(ngModel)]="pForm.direccion" class="w-full" />
        </div>
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Observaciones</label>
          <textarea pTextarea [(ngModel)]="pForm.observaciones" rows="2" class="w-full"></textarea>
        </div>
      </div>
      <div class="dialog-footer">
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarFormProveedor = false" />
        <p-button [label]="editandoProveedor ? 'Guardar' : 'Crear proveedor'" icon="pi pi-check"
          [loading]="guardandoProv()" [disabled]="!pForm.razon_social"
          (onClick)="guardarProveedor()" /></div>
    </p-dialog>

    <!-- ══ DIALOG CREAR / EDITAR ITEM ══ -->
    <p-dialog [header]="editandoItem ? 'Editar Item' : 'Nuevo Item de Inventario'"
      [(visible)]="mostrarFormItem" [style]="{width:'640px'}" [modal]="true">
      <div class="form-grid" style="gap:0.875rem">
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Nombre *</label>
          <input pInputText [(ngModel)]="iForm.nombre" class="w-full"
            placeholder="Ej: Amoxicilina 500mg" />
        </div>
        <div class="field">
          <label class="field-label">Tipo *</label>
          <p-select [(ngModel)]="iForm.tipo" [options]="opcionesTipo"
            optionLabel="label" optionValue="value"
            class="w-full" style="width:100%" />
        </div>
        <div class="field">
          <label class="field-label">Categoría</label>
          <input pInputText [(ngModel)]="iForm.categoria" class="w-full"
            placeholder="Analgésico, Antibiótico..." />
        </div>
        <div class="field">
          <label class="field-label">Presentación</label>
          <input pInputText [(ngModel)]="iForm.presentacion" class="w-full"
            placeholder="Tableta, Cápsula, Jarabe..." />
        </div>
        <div class="field">
          <label class="field-label">Lote</label>
          <input pInputText [(ngModel)]="iForm.lote" class="w-full" />
        </div>
        <div class="field">
          <label class="field-label">Fecha vencimiento</label>
          <input type="date" [(ngModel)]="iForm.fecha_vencimiento" class="w-full date-input" />
        </div>
        <div class="field">
          <label class="field-label">Registro INVIMA</label>
          <input pInputText [(ngModel)]="iForm.registro_invima" class="w-full" />
        </div>
        <div class="field">
          <label class="field-label">Unidad de medida</label>
          <p-select [(ngModel)]="iForm.unidad_medida" [options]="opcionesUnidad"
            optionLabel="label" optionValue="value"
            class="w-full" style="width:100%" />
        </div>
        <div class="field">
          <label class="field-label">Ubicación</label>
          <p-select [(ngModel)]="iForm.ubicacion" [options]="opcionesUbicacion"
            optionLabel="label" optionValue="value"
            class="w-full" style="width:100%" />
        </div>
        <div class="field">
          <label class="field-label">Costo unitario (COP)</label>
          <input pInputText type="number" [(ngModel)]="iForm.costo_unitario" class="w-full" placeholder="0" />
        </div>
        @if (!editandoItem) {
          <div class="field">
            <label class="field-label">Stock inicial</label>
            <input pInputText type="number" [(ngModel)]="iForm.cantidad_actual" class="w-full" placeholder="0" />
          </div>
        }
        <div class="field">
          <label class="field-label">Stock mínimo (alerta)</label>
          <input pInputText type="number" [(ngModel)]="iForm.cantidad_minima" class="w-full" placeholder="10" />
        </div>
        <div class="field" style="grid-column:1/-1;display:flex;gap:2rem">
          <div style="display:flex;align-items:center;gap:0.5rem">
            <p-checkbox [(ngModel)]="iForm.requiere_refrigeracion" [binary]="true" />
            <label>Requiere refrigeración</label>
          </div>
          <div style="display:flex;align-items:center;gap:0.5rem">
            <p-checkbox [(ngModel)]="iForm.controlado" [binary]="true" />
            <label>Medicamento controlado</label>
          </div>
        </div>
      </div>
      <div class="dialog-footer">
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarFormItem = false" />
        <p-button [label]="editandoItem ? 'Guardar cambios' : 'Crear item'"
          icon="pi pi-check" [loading]="guardandoItem()"
          [disabled]="!iForm.nombre"
          (onClick)="guardarItem()" />
      </div>
    </p-dialog>

    <!-- ══ DIALOG ENTRADA STOCK ══ -->
    <p-dialog header="Registrar Entrada de Stock" [(visible)]="mostrarEntrada"
      [style]="{width:'440px'}" [modal]="true">
      @if (itemEntrada()) {
        <div class="item-info-card">
          <i class="pi pi-box" style="color:#818cf8;font-size:1.2rem"></i>
          <div>
            <div style="font-weight:600;color:#f1f5f9">{{ itemEntrada()!.nombre }}</div>
            <div style="font-size:0.82rem;color:#94a3b8">Stock actual: {{ itemEntrada()!.cantidad_actual }} {{ itemEntrada()!.unidad_medida }}</div>
          </div>
        </div>
        <div class="form-grid" style="gap:0.875rem;margin-top:1rem">
          <div class="field" style="grid-column:1/-1">
            <label class="field-label">Cantidad a ingresar *</label>
            <input pInputText type="number" [(ngModel)]="eForm.cantidad"
              class="w-full" placeholder="0" />
          </div>
          <div class="field" style="grid-column:1/-1">
            <label class="field-label">Motivo</label>
            <input pInputText [(ngModel)]="eForm.motivo" class="w-full"
              placeholder="Compra, Donación, Devolución..." />
          </div>
          <div class="field">
            <label class="field-label">Nuevo lote (opcional)</label>
            <input pInputText [(ngModel)]="eForm.lote" class="w-full" />
          </div>
          <div class="field">
            <label class="field-label">Nueva fecha vencimiento</label>
            <input type="date" [(ngModel)]="eForm.fecha_vencimiento" class="w-full date-input" />
          </div>
          <div class="field" style="grid-column:1/-1">
            <label class="field-label">Nuevo costo unitario (opcional)</label>
            <input pInputText type="number" [(ngModel)]="eForm.costo_unitario"
              class="w-full" placeholder="Dejar vacío para mantener actual" />
          </div>
          <div class="field">
            <label class="field-label">Proveedor</label>
            <p-select [(ngModel)]="eForm.proveedor_id" [options]="proveedoresActivos()"
              optionLabel="label" optionValue="value" [filter]="true" [showClear]="true"
              placeholder="Sin proveedor (donación, traslado…)"
              class="w-full" style="width:100%" />
          </div>
          <div class="field">
            <label class="field-label">Factura / remisión</label>
            <input pInputText [(ngModel)]="eForm.documento_soporte" class="w-full"
              placeholder="Ej: FV-1024" />
          </div>
        </div>
      }
      <div class="dialog-footer">
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarEntrada = false" />
        <p-button label="Registrar entrada" icon="pi pi-arrow-down"
          severity="success" [loading]="guardandoEntrada()"
          [disabled]="!eForm.cantidad || eForm.cantidad <= 0"
          (onClick)="confirmarEntrada()" />
      </div>
    </p-dialog>

    <!-- ══ DIALOG DISPENSAR ══ -->
    <p-dialog header="Dispensar Medicamento" [(visible)]="mostrarDispensar"
      [style]="{width:'520px'}" [modal]="true">
      @if (formulaDispensar()) {
        <div class="item-info-card" style="margin-bottom:1rem">
          <i class="pi pi-user" style="color:#818cf8;font-size:1.2rem"></i>
          <div>
            <div style="font-weight:600;color:#f1f5f9">{{ formulaDispensar()!.paciente_nombre }}</div>
            <div style="font-size:0.85rem;color:#e2e8f0;margin-top:0.2rem">
              <strong>{{ formulaDispensar()!.medicamento }}</strong> — {{ formulaDispensar()!.dosis }}
            </div>
            <div style="font-size:0.78rem;color:#94a3b8">
              Cantidad prescrita: {{ formulaDispensar()!.cantidad }} {{ formulaDispensar()!.unidad || 'und' }}
            </div>
          </div>
        </div>

        <div class="form-grid" style="gap:0.875rem">
          <div class="field" style="grid-column:1/-1">
            <label class="field-label">Seleccionar item del inventario *</label>
            <p-select [(ngModel)]="dForm.item_inventario_id"
              [options]="inventarioParaDispensa()"
              optionLabel="label" optionValue="value"
              [filter]="true" placeholder="Buscar en inventario..."
              class="w-full" style="width:100%" />
          </div>
          <div class="field">
            <label class="field-label">Cantidad a dispensar *</label>
            <input pInputText type="number" [(ngModel)]="dForm.cantidad_dispensada"
              class="w-full" [placeholder]="String(formulaDispensar()!.cantidad)" />
          </div>
          <div class="field">
            <label class="field-label">Lote dispensado</label>
            <input pInputText [(ngModel)]="dForm.lote_dispensado" class="w-full"
              placeholder="Se toma del item seleccionado" />
          </div>
          <div class="field" style="grid-column:1/-1">
            <label class="field-label">Observaciones</label>
            <textarea pTextarea [(ngModel)]="dForm.observaciones" rows="2" class="w-full"
              placeholder="Instrucciones adicionales..."></textarea>
          </div>
        </div>
      }
      <div class="dialog-footer">
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarDispensar = false" />
        <p-button label="Confirmar dispensación" icon="pi pi-send"
          severity="success" [loading]="dispensando()"
          [disabled]="!dForm.item_inventario_id || !dForm.cantidad_dispensada"
          (onClick)="confirmarDispensa()" />
      </div>
    </p-dialog>

    <p-toast />
  `,
  styles: [`
    .page-card { padding: 1.5rem; }
    .page-title { font-size: 1.3rem; font-weight: 700; color: #f1f5f9; margin: 0; }
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
  private messageService = inject(MessageService);

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

  // Dialog item
  mostrarFormItem = false;
  editandoItem: ItemInventario | null = null;
  guardandoItem = signal(false);
  iForm: Partial<ItemInventarioCreate> = this._defaultIForm();

  // Dialog entrada
  mostrarEntrada = false;
  itemEntrada = signal<ItemInventario | null>(null);
  guardandoEntrada = signal(false);
  eForm: Partial<EntradaStock> = { motivo: 'Compra', cantidad: 0 };

  // Proveedores
  proveedores = signal<Proveedor[]>([]);
  cargandoProv = signal(false);
  busqProveedores = '';
  mostrarFormProveedor = false;
  editandoProveedor: Proveedor | null = null;
  guardandoProv = signal(false);
  pForm: Partial<ProveedorCreate> = { razon_social: '' };
  private _provTimer: ReturnType<typeof setTimeout> | undefined;

  /** Solo los activos van al selector de la entrada: a un proveedor con el que
   *  ya no se trabaja no se le apunta una remesa nueva. */
  proveedoresActivos = computed(() =>
    this.proveedores()
      .filter(p => p.activo)
      .map(p => ({ label: p.nit ? `${p.razon_social} — ${p.nit}` : p.razon_social, value: p.id }))
  );

  // Dialog dispensar
  mostrarDispensar = false;
  formulaDispensar = signal<FormulaPendiente | null>(null);
  dispensando = signal(false);
  dForm: Partial<DispensarCreate> = {};

  inventarioParaDispensa = computed(() =>
    this.inventario().map(i => ({
      label: `${i.nombre} — Stock: ${i.cantidad_actual} ${i.unidad_medida}`,
      value: i.id,
    }))
  );

  readonly opcionesTipo = [
    { label: 'Medicamento', value: 'MEDICAMENTO' },
    { label: 'Insumo', value: 'INSUMO' },
    { label: 'Dispositivo', value: 'DISPOSITIVO' },
    { label: 'Reactivo', value: 'REACTIVO' },
  ];

  readonly opcionesUnidad = [
    { label: 'Unidad', value: 'UNIDAD' },
    { label: 'Caja', value: 'CAJA' },
    { label: 'mL', value: 'ML' },
    { label: 'mg', value: 'MG' },
    { label: 'g', value: 'G' },
    { label: 'Frasco', value: 'FRASCO' },
    { label: 'Ampolla', value: 'AMPOLLA' },
  ];

  readonly opcionesUbicacion = [
    { label: 'Farmacia', value: 'FARMACIA' },
    { label: 'UCI', value: 'UCI' },
    { label: 'Urgencias', value: 'URGENCIAS' },
    { label: 'Quirófano', value: 'QUIROFANO' },
    { label: 'Hospitalización', value: 'HOSPITALIZACION' },
  ];

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

  abrirCrearProveedor(): void {
    this.editandoProveedor = null;
    this.pForm = { razon_social: '' };
    this.mostrarFormProveedor = true;
  }

  abrirEditarProveedor(p: Proveedor): void {
    this.editandoProveedor = p;
    this.pForm = {
      razon_social: p.razon_social, nit: p.nit, contacto: p.contacto,
      telefono: p.telefono, email: p.email, direccion: p.direccion,
      ciudad: p.ciudad, suministra: p.suministra, observaciones: p.observaciones,
    };
    this.mostrarFormProveedor = true;
  }

  guardarProveedor(): void {
    if (!this.pForm.razon_social) return;
    this.guardandoProv.set(true);
    const editando = this.editandoProveedor;
    const peticion = editando
      ? this.svc.actualizarProveedor(editando.id, this.pForm)
      : this.svc.crearProveedor(this.pForm as ProveedorCreate);
    peticion.subscribe({
      next: () => {
        this.guardandoProv.set(false);
        this.mostrarFormProveedor = false;
        this.ok(editando ? 'Proveedor actualizado' : 'Proveedor creado', this.pForm.razon_social!);
        this.cargarProveedores();
      },
      error: err => {
        this.guardandoProv.set(false);
        this.err(err?.error?.detail ?? 'No se pudo guardar el proveedor');
      },
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

  abrirCrear(): void {
    this.editandoItem = null;
    this.iForm = this._defaultIForm();
    this.mostrarFormItem = true;
  }

  abrirEditar(item: ItemInventario): void {
    this.editandoItem = item;
    this.iForm = {
      nombre: item.nombre,
      tipo: item.tipo,
      categoria: item.categoria,
      presentacion: item.presentacion,
      lote: item.lote,
      fecha_vencimiento: item.fecha_vencimiento,
      registro_invima: item.registro_invima,
      cantidad_actual: item.cantidad_actual,
      cantidad_minima: item.cantidad_minima,
      unidad_medida: item.unidad_medida,
      ubicacion: item.ubicacion,
      costo_unitario: item.costo_unitario,
      requiere_refrigeracion: item.requiere_refrigeracion,
      controlado: item.controlado,
    };
    this.mostrarFormItem = true;
  }

  guardarItem(): void {
    if (!this.iForm.nombre) return;
    this.guardandoItem.set(true);
    const data = { ...this.iForm } as ItemInventarioCreate;
    const obs = this.editandoItem
      ? this.svc.actualizarItem(this.editandoItem.id, data)
      : this.svc.crearItem(data);
    obs.subscribe({
      next: () => {
        this.guardandoItem.set(false);
        this.mostrarFormItem = false;
        this.ok(this.editandoItem ? 'Item actualizado' : 'Item creado');
        this.cargarInventario();
        this.cargarStats();
      },
      error: err => {
        this.guardandoItem.set(false);
        this.err(err?.error?.detail ?? 'Error al guardar');
      },
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

  abrirEntrada(item: ItemInventario): void {
    this.itemEntrada.set(item);
    this.eForm = { motivo: 'Compra', cantidad: 0 };
    // El selector de proveedor tiene que estar poblado antes de que se abra el
    // diálogo: si llega vacío, el usuario concluye «no hay proveedores» y guarda
    // la entrada sin procedencia.
    if (this.proveedores().length === 0) this.cargarProveedores();
    this.mostrarEntrada = true;
  }

  confirmarEntrada(): void {
    const item = this.itemEntrada();
    if (!item || !this.eForm.cantidad || this.eForm.cantidad <= 0) return;
    this.guardandoEntrada.set(true);
    const payload: EntradaStock = {
      cantidad: Number(this.eForm.cantidad),
      motivo: this.eForm.motivo || 'Compra',
      lote: this.eForm.lote,
      fecha_vencimiento: this.eForm.fecha_vencimiento,
      costo_unitario: this.eForm.costo_unitario ? Number(this.eForm.costo_unitario) : undefined,
      proveedor_id: this.eForm.proveedor_id ?? undefined,
      documento_soporte: this.eForm.documento_soporte || undefined,
    };
    this.svc.entradaStock(item.id, payload).subscribe({
      next: updated => {
        this.guardandoEntrada.set(false);
        this.mostrarEntrada = false;
        this.ok('Entrada registrada', `Stock actualizado: ${updated.cantidad_actual} ${updated.unidad_medida}`);
        this.cargarInventario();
        this.cargarStats();
      },
      error: err => {
        this.guardandoEntrada.set(false);
        this.err(err?.error?.detail ?? 'Error al registrar entrada');
      },
    });
  }

  // ── Dispensación ─────────────────────────────────

  abrirDispensar(f: FormulaPendiente): void {
    this.formulaDispensar.set(f);
    this.dForm = { formula_id: f.id, cantidad_dispensada: f.cantidad };
    if (this.inventario().length === 0) this.cargarInventario();
    this.mostrarDispensar = true;
  }

  confirmarDispensa(): void {
    if (!this.dForm.item_inventario_id || !this.dForm.cantidad_dispensada) return;
    const itemSel = this.inventario().find(i => i.id === this.dForm.item_inventario_id);
    const payload: DispensarCreate = {
      formula_id: this.dForm.formula_id!,
      item_inventario_id: this.dForm.item_inventario_id!,
      cantidad_dispensada: Number(this.dForm.cantidad_dispensada),
      lote_dispensado: this.dForm.lote_dispensado || itemSel?.lote || '',
      observaciones: this.dForm.observaciones,
    };
    this.dispensando.set(true);
    this.svc.dispensar(payload).subscribe({
      next: () => {
        this.dispensando.set(false);
        this.mostrarDispensar = false;
        this.ok('Dispensación registrada', `Medicamento entregado al paciente`);
        this.cargarFormulas();
        this.cargarInventario();
        this.cargarStats();
      },
      error: err => {
        this.dispensando.set(false);
        this.err(err?.error?.detail ?? 'Error al dispensar');
      },
    });
  }

  movSeverity(tipo: string): 'success' | 'danger' | 'warn' | 'secondary' {
    if (tipo === 'ENTRADA') return 'success';
    if (tipo === 'SALIDA') return 'danger';
    if (tipo === 'AJUSTE') return 'warn';
    return 'secondary';
  }

  private _defaultIForm(): Partial<ItemInventarioCreate> {
    return {
      tipo: 'MEDICAMENTO', unidad_medida: 'UNIDAD', ubicacion: 'FARMACIA',
      cantidad_actual: 0, cantidad_minima: 10, costo_unitario: 0,
      requiere_refrigeracion: false, controlado: false, lote: '',
    };
  }

  private ok(summary: string, detail?: string): void {
    this.messageService.add({ severity: 'success', summary, detail });
  }

  private err(detail: string): void {
    this.messageService.add({ severity: 'error', summary: 'Error', detail });
  }
}
