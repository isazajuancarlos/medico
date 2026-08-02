import { Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { TextareaModule } from 'primeng/textarea';
import { TableModule } from 'primeng/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { SelectModule } from 'primeng/select';
import { MatTooltipModule } from '@angular/material/tooltip';
import { DialogModule } from 'primeng/dialog';
import { MatDividerModule } from '@angular/material/divider';
import { Avisos } from '../../shared/ui/avisos.service';
import {
  FacturacionService, Factura, FacturaCreate, DetalleCreate, DetalleFactura,
  Glosa, Contrato, ContratoCreate, Entidad, StatsFacturacion,
} from '../../services/facturacion.service';

type Tab = 'facturas' | 'contratos' | 'glosas';
type Vista = 'lista' | 'detalle' | 'nueva';

@Component({
  selector: 'app-facturacion',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    ButtonModule, InputTextModule, TextareaModule, TableModule,
    EtiquetaComponent, SelectModule, MatTooltipModule,
    DialogModule, MatDividerModule,
  ],
  template: `
    <div class="page-card">

      <!-- ══════════════════ VISTA LISTA ══════════════════ -->
      @if (vista() === 'lista') {

        <div class="list-header">
          <h2 class="page-title">Facturación</h2>
          @if (tab() === 'facturas') {
            <p-button label="Nueva factura" icon="pi pi-plus" (onClick)="abrirNuevaFactura()" />
          }
          @if (tab() === 'contratos') {
            <p-button label="Nuevo contrato" icon="pi pi-plus" (onClick)="abrirFormContrato(null)" />
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
            <input pInputText [(ngModel)]="busqFactura" placeholder="Buscar por número..."
              (input)="cargarFacturas()" class="search-input" />
            <p-select [(ngModel)]="filtroEstadoFact" [options]="opcionesEstadoFact"
              optionLabel="label" optionValue="value" [showClear]="true"
              placeholder="Estado" style="width:160px" (onChange)="cargarFacturas()" />
            <p-select [(ngModel)]="filtroEntidad" [options]="entidadesOpts()"
              optionLabel="label" optionValue="value" [showClear]="true"
              placeholder="Entidad" style="width:200px" (onChange)="cargarFacturas()" />
          </div>

          @if (cargandoFact()) {
            <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
          } @else if (facturas().length === 0) {
            <div class="sin-resultados">
              <i class="pi pi-inbox" style="font-size:2rem;opacity:0.4"></i>
              <p>No hay facturas con los filtros seleccionados.</p>
            </div>
          } @else {
            <p-table [value]="facturas()" [rowHover]="true" styleClass="p-datatable-sm">
              <ng-template pTemplate="header">
                <tr>
                  <th>Número</th>
                  <th>Entidad / EPS</th>
                  <th>Período</th>
                  <th>Fecha</th>
                  <th style="text-align:right">Valor neto</th>
                  <th style="text-align:center">Estado</th>
                  <th style="text-align:center">Items</th>
                  <th style="text-align:center">Acciones</th>
                </tr>
              </ng-template>
              <ng-template pTemplate="body" let-f>
                <tr>
                  <td>
                    <strong class="num-factura">{{ f.numero_factura }}</strong>
                    @if (f.num_radicado) {
                      <div style="font-size:0.72rem;color:#94a3b8">Rad: {{ f.num_radicado }}</div>
                    }
                  </td>
                  <td>
                    <span>{{ f.entidad_nombre || '—' }}</span>
                    @if (f.contrato_nombre) {
                      <div style="font-size:0.75rem;color:#94a3b8">{{ f.contrato_nombre }}</div>
                    }
                  </td>
                  <td style="font-size:0.82rem">
                    {{ f.periodo_inicio | date:'dd/MM/yy' }} – {{ f.periodo_fin | date:'dd/MM/yy' }}
                  </td>
                  <td style="font-size:0.82rem">{{ f.fecha_factura | date:'dd/MM/yyyy' }}</td>
                  <td style="text-align:right;font-weight:600">
                    {{ f.valor_neto | currency:'COP':'symbol-narrow':'1.0-0' }}
                    @if (f.descuentos > 0) {
                      <div style="font-size:0.72rem;color:#94a3b8">-{{ f.descuentos | currency:'COP':'symbol-narrow':'1.0-0' }}</div>
                    }
                  </td>
                  <td style="text-align:center">
                    <x-etiqueta [value]="estadoLabel(f.estado)" [severity]="estadoSeverity(f.estado)" />
                    @if (f.glosada) {
                      <div style="font-size:0.68rem;color:#fbbf24;margin-top:2px">⚠ glosada</div>
                    }
                  </td>
                  <td style="text-align:center">
                    <span class="dur-badge">{{ f.total_detalles }}</span>
                  </td>
                  <td style="text-align:center;white-space:nowrap">
                    <p-button icon="pi pi-eye" [text]="true" size="small"
                      matTooltip="Ver detalle" (onClick)="abrirDetalle(f.id)" />
                    @if (f.estado === 'BORRADOR') {
                      <p-button icon="pi pi-send" [text]="true" size="small"
                        severity="success" matTooltip="Radicar"
                        (onClick)="abrirRadicar(f)" />
                      <p-button icon="pi pi-trash" [text]="true" size="small"
                        severity="danger" matTooltip="Eliminar"
                        (onClick)="eliminarFactura(f)" />
                    }
                    @if (f.estado === 'RADICADA') {
                      <p-button icon="pi pi-check" [text]="true" size="small"
                        severity="success" matTooltip="Marcar aceptada"
                        (onClick)="cambiarEstado(f, 'ACEPTADA')" />
                    }
                    @if (f.estado === 'ACEPTADA') {
                      <p-button icon="pi pi-dollar" [text]="true" size="small"
                        severity="success" matTooltip="Marcar pagada"
                        (onClick)="cambiarEstado(f, 'PAGADA')" />
                    }
                  </td>
                </tr>
              </ng-template>
            </p-table>
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
            <p-table [value]="contratos()" [rowHover]="true" styleClass="p-datatable-sm">
              <ng-template pTemplate="header">
                <tr>
                  <th>Nombre contrato</th>
                  <th>N° contrato</th>
                  <th>Entidad</th>
                  <th>Tarifa base</th>
                  <th>Vigencia</th>
                  <th style="text-align:center">Estado</th>
                  <th style="text-align:center">Acciones</th>
                </tr>
              </ng-template>
              <ng-template pTemplate="body" let-c>
                <tr>
                  <td><strong>{{ c.nombre }}</strong></td>
                  <td style="font-size:0.82rem;color:#94a3b8">{{ c.numero_contrato }}</td>
                  <td style="font-size:0.85rem">{{ c.entidad_nombre || '—' }}</td>
                  <td>
                    <x-etiqueta [value]="c.tipo_tarifa" severity="secondary" />
                    @if (c.porcentaje_incremento) {
                      <span style="font-size:0.75rem;color:#94a3b8;margin-left:4px">+{{ c.porcentaje_incremento }}%</span>
                    }
                  </td>
                  <td style="font-size:0.82rem">
                    {{ c.fecha_inicio | date:'dd/MM/yyyy' }} – {{ c.fecha_fin | date:'dd/MM/yyyy' }}
                  </td>
                  <td style="text-align:center">
                    <x-etiqueta [value]="c.activo ? 'Activo' : 'Inactivo'"
                      [severity]="c.activo ? 'success' : 'secondary'" />
                  </td>
                  <td style="text-align:center">
                    <p-button icon="pi pi-pencil" [text]="true" size="small"
                      matTooltip="Editar" (onClick)="abrirFormContrato(c)" />
                    <p-button [icon]="c.activo ? 'pi pi-ban' : 'pi pi-check'" [text]="true"
                      size="small" [severity]="c.activo ? 'warn' : 'success'"
                      [matTooltip]="c.activo ? 'Desactivar' : 'Activar'"
                      (onClick)="toggleContrato(c)" />
                  </td>
                </tr>
              </ng-template>
            </p-table>
          }
        }

        <!-- ─── TAB GLOSAS ─── -->
        @if (tab() === 'glosas') {
          <div class="filter-bar">
            <p-select [(ngModel)]="filtroEstadoGlosa" [options]="opcionesEstadoGlosa"
              optionLabel="label" optionValue="value" [showClear]="true"
              placeholder="Estado" style="width:160px" (onChange)="cargarGlosas()" />
          </div>

          @if (cargandoGlosa()) {
            <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
          } @else if (glosas().length === 0) {
            <div class="sin-resultados">
              <i class="pi pi-check-circle" style="font-size:2rem;opacity:0.4;color:#4ade80"></i>
              <p>No hay glosas con el filtro seleccionado.</p>
            </div>
          } @else {
            <p-table [value]="glosas()" [rowHover]="true" styleClass="p-datatable-sm">
              <ng-template pTemplate="header">
                <tr>
                  <th>Factura</th>
                  <th>Item glosado</th>
                  <th>Motivo</th>
                  <th style="text-align:right">Valor</th>
                  <th>Fecha</th>
                  <th style="text-align:center">Estado</th>
                  <th style="text-align:center">Acción</th>
                </tr>
              </ng-template>
              <ng-template pTemplate="body" let-g>
                <tr>
                  <td><strong class="num-factura">{{ g.numero_factura || '—' }}</strong></td>
                  <td style="font-size:0.85rem">{{ g.item_glosado }}</td>
                  <td style="font-size:0.82rem">{{ g.motivo }}</td>
                  <td style="text-align:right;font-weight:600;color:#f87171">
                    {{ g.valor_glosado | currency:'COP':'symbol-narrow':'1.0-0' }}
                  </td>
                  <td style="font-size:0.82rem">{{ g.fecha_glosa | date:'dd/MM/yyyy' }}</td>
                  <td style="text-align:center">
                    <x-etiqueta [value]="g.estado"
                      [severity]="g.estado === 'PENDIENTE' ? 'warn' : g.estado === 'RECHAZADA' ? 'danger' : 'success'" />
                  </td>
                  <td style="text-align:center">
                    @if (g.estado === 'PENDIENTE') {
                      <p-button icon="pi pi-reply" [text]="true" size="small"
                        matTooltip="Responder glosa" (onClick)="abrirResponderGlosa(g)" />
                    } @else {
                      <span style="font-size:0.75rem;color:#64748b" matTooltip="{{ g.respuesta }}">
                        {{ g.fecha_respuesta | date:'dd/MM/yy' }}
                      </span>
                    }
                  </td>
                </tr>
              </ng-template>
            </p-table>
          }
        }

      }

      <!-- ══════════════════ VISTA NUEVA FACTURA ══════════════════ -->
      @if (vista() === 'nueva') {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <p-button icon="pi pi-arrow-left" [text]="true" (onClick)="volverLista()" />
          <h2 class="page-title" style="margin:0">Nueva Factura</h2>
        </div>

        <div class="form-card">
          <div class="form-section">
            <div class="section-title">Datos de la factura</div>
            <div class="form-grid">
              <div class="field">
                <label class="field-label">Entidad / EPS *</label>
                <p-select [(ngModel)]="nFact.entidad_id" [options]="entidadesOpts()"
                  optionLabel="label" optionValue="value" [filter]="true"
                  placeholder="Seleccione entidad..."
                  class="w-full" style="width:100%"
                  (onChange)="onEntidadChange()" />
              </div>
              <div class="field">
                <label class="field-label">Contrato (opcional)</label>
                <p-select [(ngModel)]="nFact.contrato_id" [options]="contratosEntidad()"
                  optionLabel="label" optionValue="value" [filter]="true" [showClear]="true"
                  placeholder="Seleccione contrato..."
                  class="w-full" style="width:100%" />
              </div>
              <div class="field">
                <label class="field-label">Período inicio *</label>
                <input type="date" [(ngModel)]="nFact.periodo_inicio" class="w-full date-input" />
              </div>
              <div class="field">
                <label class="field-label">Período fin *</label>
                <input type="date" [(ngModel)]="nFact.periodo_fin" class="w-full date-input" />
              </div>
              <div class="field">
                <label class="field-label">Descuentos (COP)</label>
                <input pInputText type="number" [(ngModel)]="nFact.descuentos" class="w-full" placeholder="0" />
              </div>
              <div class="field">
                <label class="field-label">Observaciones</label>
                <input pInputText [(ngModel)]="nFact.observaciones" class="w-full" />
              </div>
            </div>
          </div>

          <mat-divider />

          <!-- Líneas de detalle -->
          <div class="form-section">
            <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem">
              <div class="section-title" style="margin-bottom:0">Servicios prestados</div>
              <p-button label="Agregar servicio" icon="pi pi-plus" size="small"
                [outlined]="true" (onClick)="agregarLineaTemporal()" />
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
                  <div class="detalle-row">
                    <input pInputText [(ngModel)]="lin.servicio" class="w-full" placeholder="Descripción servicio" />
                    <input pInputText [(ngModel)]="lin.codigo_cups" style="width:100px" placeholder="CUPS" />
                    <input pInputText type="number" [(ngModel)]="lin.cantidad"
                      style="width:70px;text-align:center" min="1" (input)="calcLine(lin)" />
                    <input pInputText type="number" [(ngModel)]="lin.valor_unitario"
                      style="width:130px;text-align:right" (input)="calcLine(lin)" />
                    <span class="total-linea">{{ ((lin.valor_unitario ?? 0) * (lin.cantidad ?? 0)) | currency:'COP':'symbol-narrow':'1.0-0' }}</span>
                    <p-button icon="pi pi-times" [text]="true" size="small" severity="danger"
                      (onClick)="quitarLinea($index)" />
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
            <p-button label="Cancelar" [text]="true" (onClick)="volverLista()" />
            <p-button label="Crear factura" icon="pi pi-check"
              [loading]="guardando()"
              [disabled]="!nFact.entidad_id || !nFact.periodo_inicio || !nFact.periodo_fin"
              (onClick)="crearFactura()" />
          </div>
        </div>
      }

      <!-- ══════════════════ VISTA DETALLE FACTURA ══════════════════ -->
      @if (vista() === 'detalle' && facturaDetalle()) {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.5rem">
          <p-button icon="pi pi-arrow-left" [text]="true" (onClick)="volverLista()" />
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
              <p-button label="Agregar" icon="pi pi-plus" size="small"
                [outlined]="true" (onClick)="abrirAgregarDetalle()" />
            }
          </div>

          @if (facturaDetalle()!.detalles.length > 0) {
            <p-table [value]="facturaDetalle()!.detalles" styleClass="p-datatable-sm">
              <ng-template pTemplate="header">
                <tr>
                  <th>Servicio</th>
                  <th>CUPS</th>
                  <th style="text-align:center">Cant.</th>
                  <th style="text-align:right">Valor unit.</th>
                  <th style="text-align:right">Total</th>
                  @if (facturaDetalle()!.estado === 'BORRADOR') {
                    <th></th>
                  }
                </tr>
              </ng-template>
              <ng-template pTemplate="body" let-d>
                <tr>
                  <td>
                    {{ d.servicio }}
                    @if (d.paciente_nombre) {
                      <div style="font-size:0.75rem;color:#94a3b8">{{ d.paciente_nombre }}</div>
                    }
                  </td>
                  <td style="font-size:0.82rem;color:#94a3b8">{{ d.codigo_cups }}</td>
                  <td style="text-align:center">{{ d.cantidad }}</td>
                  <td style="text-align:right;font-size:0.82rem">{{ d.valor_unitario | currency:'COP':'symbol-narrow':'1.0-0' }}</td>
                  <td style="text-align:right;font-weight:600">{{ d.valor_total | currency:'COP':'symbol-narrow':'1.0-0' }}</td>
                  @if (facturaDetalle()!.estado === 'BORRADOR') {
                    <td style="text-align:center">
                      <p-button icon="pi pi-times" [text]="true" size="small" severity="danger"
                        (onClick)="eliminarDetalle(d)" [loading]="accionando()" />
                    </td>
                  }
                </tr>
              </ng-template>
            </p-table>
          }

          <!-- Glosas -->
          @if (facturaDetalle()!.glosas.length > 0) {
            <mat-divider />
            <div class="section-title">Glosas ({{ facturaDetalle()!.glosas.length }})</div>
            <p-table [value]="facturaDetalle()!.glosas" styleClass="p-datatable-sm">
              <ng-template pTemplate="header">
                <tr>
                  <th>Item glosado</th>
                  <th>Motivo</th>
                  <th style="text-align:right">Valor</th>
                  <th style="text-align:center">Estado</th>
                  <th>Respuesta</th>
                </tr>
              </ng-template>
              <ng-template pTemplate="body" let-g>
                <tr>
                  <td>{{ g.item_glosado }}</td>
                  <td style="font-size:0.82rem">{{ g.motivo }}</td>
                  <td style="text-align:right;color:#f87171;font-weight:600">{{ g.valor_glosado | currency:'COP':'symbol-narrow':'1.0-0' }}</td>
                  <td style="text-align:center">
                    <x-etiqueta [value]="g.estado"
                      [severity]="g.estado === 'PENDIENTE' ? 'warn' : g.estado === 'RECHAZADA' ? 'danger' : 'success'" />
                  </td>
                  <td style="font-size:0.78rem;color:#94a3b8">{{ g.respuesta || '—' }}</td>
                </tr>
              </ng-template>
            </p-table>
          }

          <!-- Acciones -->
          <mat-divider />
          <div style="display:flex;gap:0.5rem;justify-content:flex-end;flex-wrap:wrap">
            @if (facturaDetalle()!.estado === 'BORRADOR') {
              <p-button label="Agregar glosa" icon="pi pi-exclamation-circle" [outlined]="true"
                severity="warn" size="small" (onClick)="abrirAgregarGlosa()" />
              <p-button label="Radicar" icon="pi pi-send"
                severity="success" (onClick)="abrirRadicar(facturaDetalle()!)" />
            }
            @if (facturaDetalle()!.estado === 'RADICADA') {
              <p-button label="Agregar glosa" icon="pi pi-exclamation-circle" [outlined]="true"
                severity="warn" size="small" (onClick)="abrirAgregarGlosa()" />
              <p-button label="Marcar aceptada" icon="pi pi-check"
                severity="success" [loading]="accionando()"
                (onClick)="cambiarEstado(facturaDetalle()!, 'ACEPTADA')" />
            }
            @if (facturaDetalle()!.estado === 'ACEPTADA') {
              <p-button label="Marcar pagada" icon="pi pi-dollar"
                severity="success" [loading]="accionando()"
                (onClick)="cambiarEstado(facturaDetalle()!, 'PAGADA')" />
            }
            @if (!['PAGADA','ANULADA'].includes(facturaDetalle()!.estado)) {
              <p-button label="Anular" icon="pi pi-ban" [outlined]="true"
                severity="danger" size="small" [loading]="accionando()"
                (onClick)="cambiarEstado(facturaDetalle()!, 'ANULADA')" />
            }
          </div>
        </div>
      }

    </div>

    <!-- ══ DIALOG RADICAR ══ -->
    <p-dialog header="Radicar Factura" [(visible)]="mostrarRadicar"
      [style]="{width:'440px'}" [modal]="true">
      @if (facturaRadicar()) {
        <div class="item-info-card" style="margin-bottom:1rem">
          <div>
            <div style="font-weight:600;color:#f1f5f9">{{ facturaRadicar()!.numero_factura }}</div>
            <div style="font-size:0.82rem;color:#94a3b8">{{ facturaRadicar()!.entidad_nombre }} — {{ facturaRadicar()!.valor_neto | currency:'COP':'symbol-narrow':'1.0-0' }}</div>
          </div>
        </div>
        <div class="form-grid" style="gap:0.875rem">
          <div class="field" style="grid-column:1/-1">
            <label class="field-label">Número de radicado *</label>
            <input pInputText [(ngModel)]="radNumero" class="w-full" placeholder="RAD-2026-XXXXX" />
          </div>
          <div class="field" style="grid-column:1/-1">
            <label class="field-label">Fecha de radicación</label>
            <input type="date" [(ngModel)]="radFecha" class="w-full date-input" />
          </div>
        </div>
      }
      <div class="dialog-footer">
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarRadicar = false" />
        <p-button label="Radicar factura" icon="pi pi-send"
          severity="success" [loading]="accionando()"
          [disabled]="!radNumero"
          (onClick)="confirmarRadicar()" />
      </div>
    </p-dialog>

    <!-- ══ DIALOG AGREGAR DETALLE ══ -->
    <p-dialog header="Agregar Servicio" [(visible)]="mostrarDetalleDlg"
      [style]="{width:'480px'}" [modal]="true">
      <div class="form-grid" style="gap:0.875rem">
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Descripción del servicio *</label>
          <input pInputText [(ngModel)]="newDet.servicio" class="w-full"
            placeholder="Ej: Consulta médica general - HC 2026-001" />
        </div>
        <div class="field">
          <label class="field-label">Código CUPS</label>
          <input pInputText [(ngModel)]="newDet.codigo_cups" class="w-full" placeholder="890201" />
        </div>
        <div class="field">
          <label class="field-label">Cantidad</label>
          <input pInputText type="number" [(ngModel)]="newDet.cantidad" class="w-full" min="1" />
        </div>
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Valor unitario (COP) *</label>
          <input pInputText type="number" [(ngModel)]="newDet.valor_unitario" class="w-full" placeholder="0" />
        </div>
      </div>
      <div class="dialog-footer">
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarDetalleDlg = false" />
        <p-button label="Agregar" icon="pi pi-check"
          [loading]="accionando()"
          [disabled]="!newDet.servicio || !newDet.valor_unitario"
          (onClick)="confirmarAgregarDetalle()" />
      </div>
    </p-dialog>

    <!-- ══ DIALOG AGREGAR GLOSA ══ -->
    <p-dialog header="Registrar Glosa" [(visible)]="mostrarGlosaDlg"
      [style]="{width:'480px'}" [modal]="true">
      <div class="form-grid" style="gap:0.875rem">
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Item glosado *</label>
          <input pInputText [(ngModel)]="newGlosa.item_glosado" class="w-full"
            placeholder="Ej: Consulta del 2026-01-05" />
        </div>
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Motivo de la glosa *</label>
          <textarea pTextarea [(ngModel)]="newGlosa.motivo" rows="2" class="w-full"
            placeholder="Falta autorización, documentación incompleta..."></textarea>
        </div>
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Valor glosado (COP) *</label>
          <input pInputText type="number" [(ngModel)]="newGlosa.valor_glosado" class="w-full" placeholder="0" />
        </div>
      </div>
      <div class="dialog-footer">
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarGlosaDlg = false" />
        <p-button label="Registrar glosa" icon="pi pi-exclamation-circle"
          severity="warn" [loading]="accionando()"
          [disabled]="!newGlosa.item_glosado || !newGlosa.motivo || !newGlosa.valor_glosado"
          (onClick)="confirmarGlosa()" />
      </div>
    </p-dialog>

    <!-- ══ DIALOG RESPONDER GLOSA ══ -->
    <p-dialog header="Responder Glosa" [(visible)]="mostrarRespGlosa"
      [style]="{width:'480px'}" [modal]="true">
      @if (glosaResponder()) {
        <div class="item-info-card" style="margin-bottom:1rem">
          <div>
            <div style="font-weight:600;color:#f1f5f9">{{ glosaResponder()!.item_glosado }}</div>
            <div style="font-size:0.82rem;color:#94a3b8">{{ glosaResponder()!.motivo }}</div>
            <div style="font-size:0.82rem;color:#f87171;margin-top:0.25rem">{{ glosaResponder()!.valor_glosado | currency:'COP':'symbol-narrow':'1.0-0' }}</div>
          </div>
        </div>
        <div class="field" style="margin-bottom:1rem">
          <label class="field-label">Respuesta / sustentación *</label>
          <textarea pTextarea [(ngModel)]="respGlosaTexto" rows="4" class="w-full"
            placeholder="Explique por qué acepta o rechaza la glosa..."></textarea>
        </div>
        <div style="display:flex;justify-content:flex-end;gap:0.5rem;padding-top:0.75rem;border-top:1px solid rgba(255,255,255,0.1)">
          <p-button label="Cancelar" [text]="true" (onClick)="mostrarRespGlosa = false" />
          <p-button label="Aceptar glosa" icon="pi pi-check"
            severity="success" [loading]="accionando()"
            [disabled]="!respGlosaTexto"
            (onClick)="confirmarRespuestaGlosa(true)" />
          <p-button label="Rechazar glosa" icon="pi pi-times"
            severity="danger" [loading]="accionando()"
            [disabled]="!respGlosaTexto"
            (onClick)="confirmarRespuestaGlosa(false)" />
        </div>
      }
    </p-dialog>

    <!-- ══ DIALOG CONTRATO ══ -->
    <p-dialog [header]="editContrato ? 'Editar Contrato' : 'Nuevo Contrato'"
      [(visible)]="mostrarContratoForm" [style]="{width:'560px'}" [modal]="true">
      <div class="form-grid" style="gap:0.875rem">
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Entidad / EPS *</label>
          <p-select [(ngModel)]="cForm.entidad_id" [options]="entidadesOpts()"
            optionLabel="label" optionValue="value" [filter]="true"
            placeholder="Seleccione entidad..."
            class="w-full" style="width:100%" />
        </div>
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Nombre del contrato *</label>
          <input pInputText [(ngModel)]="cForm.nombre" class="w-full"
            placeholder="Ej: Contrato SURA 2026" />
        </div>
        <div class="field">
          <label class="field-label">Número de contrato *</label>
          <input pInputText [(ngModel)]="cForm.numero_contrato" class="w-full" />
        </div>
        <div class="field">
          <label class="field-label">Tipo de tarifa</label>
          <p-select [(ngModel)]="cForm.tipo_tarifa" [options]="opcionesTarifa"
            optionLabel="label" optionValue="value"
            class="w-full" style="width:100%" />
        </div>
        <div class="field">
          <label class="field-label">Fecha inicio</label>
          <input type="date" [(ngModel)]="cForm.fecha_inicio" class="w-full date-input" />
        </div>
        <div class="field">
          <label class="field-label">Fecha fin</label>
          <input type="date" [(ngModel)]="cForm.fecha_fin" class="w-full date-input" />
        </div>
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">% de incremento sobre tarifa base</label>
          <input pInputText type="number" [(ngModel)]="cForm.porcentaje_incremento"
            class="w-full" placeholder="0" />
        </div>
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Observaciones</label>
          <textarea pTextarea [(ngModel)]="cForm.observaciones" rows="2" class="w-full"></textarea>
        </div>
      </div>
      <div class="dialog-footer">
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarContratoForm = false" />
        <p-button [label]="editContrato ? 'Guardar cambios' : 'Crear contrato'"
          icon="pi pi-check" [loading]="guardando()"
          [disabled]="!cForm.nombre || !cForm.numero_contrato || !cForm.entidad_id"
          (onClick)="guardarContrato()" />
      </div>
    </p-dialog>

  `,
  styles: [`
    .page-card { padding: 1.5rem; }
    .page-title { font-size: 1.3rem; font-weight: 700; color: #f1f5f9; margin: 0; }
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
  mostrarRadicar = false;
  facturaRadicar = signal<Factura | null>(null);
  radNumero = '';
  radFecha = '';

  // Dialog detalle (agregar línea)
  mostrarDetalleDlg = false;
  newDet: Partial<DetalleCreate> = { codigo_cups: '890201', cantidad: 1 };

  // Dialog glosa
  mostrarGlosaDlg = false;
  newGlosa: Partial<{ item_glosado: string; motivo: string; valor_glosado: number }> = {};
  glosaTargetId: number | null = null;

  // Dialog responder glosa
  mostrarRespGlosa = false;
  glosaResponder = signal<Glosa | null>(null);
  respGlosaTexto = '';

  // Dialog contrato
  mostrarContratoForm = false;
  editContrato: Contrato | null = null;
  cForm: Partial<ContratoCreate> = { tipo_tarifa: 'CUPS', porcentaje_incremento: 0 };

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

  readonly opcionesTarifa = [
    { label: 'CUPS', value: 'CUPS' },
    { label: 'ISS', value: 'ISS' },
    { label: 'SOAT', value: 'SOAT' },
    { label: 'Personalizada', value: 'PERSONALIZADA' },
  ];

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
    this.facturaRadicar.set(f);
    this.radNumero = '';
    this.radFecha = new Date().toISOString().split('T')[0];
    this.mostrarRadicar = true;
  }

  confirmarRadicar(): void {
    const f = this.facturaRadicar();
    if (!f || !this.radNumero) return;
    this.accionando.set(true);
    this.svc.radicarFactura(f.id, this.radNumero, this.radFecha).subscribe({
      next: updated => {
        this.accionando.set(false);
        this.mostrarRadicar = false;
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
    this.newDet = { codigo_cups: '890201', cantidad: 1 };
    this.mostrarDetalleDlg = true;
  }

  confirmarAgregarDetalle(): void {
    const f = this.facturaDetalle();
    if (!f || !this.newDet.servicio || !this.newDet.valor_unitario) return;
    this.accionando.set(true);
    this.svc.agregarDetalle(f.id, {
      servicio: this.newDet.servicio!,
      codigo_cups: this.newDet.codigo_cups || '890201',
      cantidad: Number(this.newDet.cantidad) || 1,
      valor_unitario: Number(this.newDet.valor_unitario),
    }).subscribe({
      next: updated => {
        this.accionando.set(false);
        this.mostrarDetalleDlg = false;
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
    this.newGlosa = {};
    this.glosaTargetId = this.facturaDetalle()?.id ?? null;
    this.mostrarGlosaDlg = true;
  }

  confirmarGlosa(): void {
    const id = this.glosaTargetId;
    if (!id || !this.newGlosa.item_glosado || !this.newGlosa.motivo || !this.newGlosa.valor_glosado) return;
    this.accionando.set(true);
    this.svc.agregarGlosa(id, {
      item_glosado: this.newGlosa.item_glosado!,
      motivo: this.newGlosa.motivo!,
      valor_glosado: Number(this.newGlosa.valor_glosado),
    }).subscribe({
      next: () => {
        this.accionando.set(false);
        this.mostrarGlosaDlg = false;
        this.ok('Glosa registrada');
        if (this.vista() === 'detalle' && this.facturaDetalle()) {
          this.abrirDetalle(this.facturaDetalle()!.id);
        }
        this.cargarStats();
      },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error al registrar glosa'); },
    });
  }

  abrirResponderGlosa(g: Glosa): void {
    this.glosaResponder.set(g);
    this.respGlosaTexto = '';
    this.mostrarRespGlosa = true;
  }

  confirmarRespuestaGlosa(aceptar: boolean): void {
    const g = this.glosaResponder();
    if (!g || !this.respGlosaTexto) return;
    this.accionando.set(true);
    this.svc.responderGlosa(g.id, this.respGlosaTexto, aceptar).subscribe({
      next: () => {
        this.accionando.set(false);
        this.mostrarRespGlosa = false;
        this.ok(aceptar ? 'Glosa aceptada' : 'Glosa rechazada');
        this.cargarGlosas();
        this.cargarStats();
      },
      error: err => { this.accionando.set(false); this.err(err?.error?.detail ?? 'Error al responder'); },
    });
  }

  // ── Contratos ────────────────────────────────────

  abrirFormContrato(c: Contrato | null): void {
    this.editContrato = c;
    this.cForm = c
      ? { entidad_id: c.entidad_id, nombre: c.nombre, numero_contrato: c.numero_contrato,
          fecha_inicio: c.fecha_inicio, fecha_fin: c.fecha_fin, tipo_tarifa: c.tipo_tarifa,
          porcentaje_incremento: c.porcentaje_incremento, observaciones: c.observaciones }
      : { tipo_tarifa: 'CUPS', porcentaje_incremento: 0 };
    this.mostrarContratoForm = true;
  }

  guardarContrato(): void {
    if (!this.cForm.nombre || !this.cForm.numero_contrato || !this.cForm.entidad_id) return;
    this.guardando.set(true);
    const obs = this.editContrato
      ? this.svc.actualizarContrato(this.editContrato.id, this.cForm as ContratoCreate)
      : this.svc.crearContrato(this.cForm as ContratoCreate);
    obs.subscribe({
      next: () => {
        this.guardando.set(false);
        this.mostrarContratoForm = false;
        this.ok(this.editContrato ? 'Contrato actualizado' : 'Contrato creado');
        this.cargarContratos();
      },
      error: err => { this.guardando.set(false); this.err(err?.error?.detail ?? 'Error al guardar contrato'); },
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
