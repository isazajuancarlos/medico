import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

export interface Entidad {
  id: number;
  nit: string;
  nombre: string;
  codigo_eps: string;
}

export interface Contrato {
  id: number;
  entidad_id: number;
  nombre: string;
  numero_contrato: string;
  fecha_inicio: string;
  fecha_fin: string;
  activo: boolean;
  tipo_tarifa: string;
  porcentaje_incremento: number;
  observaciones?: string;
  entidad_nombre?: string;
}

export interface ContratoCreate {
  entidad_id: number;
  nombre: string;
  numero_contrato: string;
  fecha_inicio: string;
  fecha_fin: string;
  tipo_tarifa: string;
  porcentaje_incremento: number;
  observaciones?: string;
  activo?: boolean;
}

export interface DetalleFactura {
  id: number;
  factura_id: number;
  servicio: string;
  codigo_cups: string;
  cantidad: number;
  valor_unitario: number;
  valor_total: number;
  paciente_id?: number;
  historia_id?: number;
  paciente_nombre?: string;
}

export interface DetalleCreate {
  servicio: string;
  codigo_cups: string;
  cantidad: number;
  valor_unitario: number;
  paciente_id?: number;
  historia_id?: number;
}

export interface Glosa {
  id: number;
  factura_id: number;
  item_glosado: string;
  motivo: string;
  valor_glosado: number;
  fecha_glosa: string;
  respuesta?: string;
  fecha_respuesta?: string;
  estado: string;
  numero_factura?: string;
}

export interface Factura {
  id: number;
  numero_factura: string;
  entidad_id: number;
  contrato_id?: number;
  fecha_factura: string;
  fecha_radicado?: string;
  num_radicado?: string;
  periodo_inicio: string;
  periodo_fin: string;
  valor_bruto: number;
  descuentos: number;
  valor_neto: number;
  estado: string;
  glosada: boolean;
  observaciones?: string;
  detalles: DetalleFactura[];
  glosas: Glosa[];
  entidad_nombre?: string;
  contrato_nombre?: string;
  total_detalles: number;
  glosas_pendientes: number;
}

export interface FacturaCreate {
  entidad_id: number;
  contrato_id?: number;
  periodo_inicio: string;
  periodo_fin: string;
  descuentos: number;
  observaciones?: string;
  detalles: DetalleCreate[];
}

export interface StatsFacturacion {
  total_facturas: number;
  borradores: number;
  radicadas: number;
  aceptadas: number;
  pagadas: number;
  glosadas: number;
  valor_total_radicado: number;
  valor_total_pagado: number;
  glosas_pendientes: number;
  valor_glosado_pendiente: number;
}

@Injectable({ providedIn: 'root' })
export class FacturacionService {
  private readonly API = '/api/facturacion';

  constructor(private http: HttpClient) {}

  getStats(): Observable<StatsFacturacion> {
    return this.http.get<StatsFacturacion>(`${this.API}/stats`);
  }

  listFacturas(params: Record<string, string> = {}): Observable<Factura[]> {
    return this.http.get<Factura[]>(`${this.API}/facturas`, { params });
  }

  getFactura(id: number): Observable<Factura> {
    return this.http.get<Factura>(`${this.API}/facturas/${id}`);
  }

  crearFactura(data: FacturaCreate): Observable<Factura> {
    return this.http.post<Factura>(`${this.API}/facturas`, data);
  }

  actualizarFactura(id: number, data: Partial<FacturaCreate>): Observable<Factura> {
    return this.http.put<Factura>(`${this.API}/facturas/${id}`, data);
  }

  eliminarFactura(id: number): Observable<void> {
    return this.http.delete<void>(`${this.API}/facturas/${id}`);
  }

  radicarFactura(id: number, num_radicado: string, fecha_radicado?: string): Observable<Factura> {
    return this.http.put<Factura>(`${this.API}/facturas/${id}/radicar`, { num_radicado, fecha_radicado });
  }

  cambiarEstado(id: number, estado: string): Observable<Factura> {
    return this.http.put<Factura>(`${this.API}/facturas/${id}/estado`, null, { params: { nuevo_estado: estado } });
  }

  agregarDetalle(facturaId: number, data: DetalleCreate): Observable<Factura> {
    return this.http.post<Factura>(`${this.API}/facturas/${facturaId}/detalles`, data);
  }

  eliminarDetalle(facturaId: number, detalleId: number): Observable<Factura> {
    return this.http.delete<Factura>(`${this.API}/facturas/${facturaId}/detalles/${detalleId}`);
  }

  agregarGlosa(facturaId: number, data: { item_glosado: string; motivo: string; valor_glosado: number }): Observable<Glosa> {
    return this.http.post<Glosa>(`${this.API}/facturas/${facturaId}/glosas`, data);
  }

  listGlosas(estado?: string): Observable<Glosa[]> {
    const params: Record<string, string> = {};
    if (estado) params['estado'] = estado;
    return this.http.get<Glosa[]>(`${this.API}/glosas`, { params });
  }

  responderGlosa(id: number, respuesta: string, aceptar: boolean): Observable<Glosa> {
    return this.http.put<Glosa>(`${this.API}/glosas/${id}/responder`, { respuesta, aceptar });
  }

  listContratos(activo?: boolean): Observable<Contrato[]> {
    const params: Record<string, string> = {};
    if (activo !== undefined) params['activo'] = String(activo);
    return this.http.get<Contrato[]>(`${this.API}/contratos`, { params });
  }

  crearContrato(data: ContratoCreate): Observable<Contrato> {
    return this.http.post<Contrato>(`${this.API}/contratos`, data);
  }

  actualizarContrato(id: number, data: Partial<ContratoCreate>): Observable<Contrato> {
    return this.http.put<Contrato>(`${this.API}/contratos/${id}`, data);
  }

  listEntidades(): Observable<Entidad[]> {
    return this.http.get<Entidad[]>(`${this.API}/entidades`);
  }
}
