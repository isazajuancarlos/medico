import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

export interface ItemInventario {
  id: number;
  nombre: string;
  tipo: string;
  categoria?: string;
  presentacion?: string;
  lote: string;
  fecha_vencimiento?: string;
  registro_invima?: string;
  cantidad_actual: number;
  cantidad_minima: number;
  unidad_medida: string;
  ubicacion: string;
  costo_unitario: number;
  valor_total: number;
  requiere_refrigeracion: boolean;
  controlado: boolean;
  alerta_stock: boolean;
  alerta_vencimiento: boolean;
}

export interface ItemInventarioCreate {
  nombre: string;
  tipo: string;
  categoria?: string;
  presentacion?: string;
  lote?: string;
  fecha_vencimiento?: string;
  registro_invima?: string;
  cantidad_actual: number;
  cantidad_minima: number;
  unidad_medida: string;
  ubicacion: string;
  costo_unitario: number;
  requiere_refrigeracion: boolean;
  controlado: boolean;
}

export interface EntradaStock {
  cantidad: number;
  motivo: string;
  lote?: string;
  fecha_vencimiento?: string;
  costo_unitario?: number;
}

export interface FormulaPendiente {
  id: number;
  historia_id: number;
  medicamento: string;
  codigo_cum?: string;
  dosis: string;
  cantidad: number;
  unidad?: string;
  observaciones?: string;
  fecha_formula: string;
  dispensada: boolean;
  paciente_nombre?: string;
  paciente_id?: number;
  medico_nombre?: string;
}

export interface DispensarCreate {
  formula_id: number;
  item_inventario_id: number;
  cantidad_dispensada: number;
  lote_dispensado: string;
  observaciones?: string;
}

export interface Dispensacion {
  id: number;
  formula_id: number;
  item_inventario_id: number;
  paciente_id: number;
  regente_farmacia_id: number;
  fecha_dispensacion: string;
  cantidad_dispensada: number;
  lote_dispensado: string;
  observaciones?: string;
  paciente_nombre?: string;
  medicamento_nombre?: string;
  item_nombre?: string;
  regente_nombre?: string;
}

export interface Movimiento {
  id: number;
  item_inventario_id: number;
  usuario_id: number;
  fecha: string;
  tipo_movimiento: string;
  cantidad: number;
  motivo: string;
  paciente_id?: number;
  formula_id?: number;
  usuario_nombre?: string;
  item_nombre?: string;
}

export interface StatsInventario {
  total_items: number;
  stock_bajo: number;
  por_vencer: number;
  vencidos: number;
  formulas_pendientes: number;
  valor_total_inventario: number;
}

@Injectable({ providedIn: 'root' })
export class FarmaciaService {
  private readonly API = '/api/farmacia';

  constructor(private http: HttpClient) {}

  getStats(): Observable<StatsInventario> {
    return this.http.get<StatsInventario>(`${this.API}/stats`);
  }

  listInventario(params: Record<string, string | boolean> = {}): Observable<ItemInventario[]> {
    const p: Record<string, string> = {};
    for (const [k, v] of Object.entries(params)) {
      if (v !== undefined && v !== null && v !== '') p[k] = String(v);
    }
    return this.http.get<ItemInventario[]>(`${this.API}/inventario`, { params: p });
  }

  crearItem(data: ItemInventarioCreate): Observable<ItemInventario> {
    return this.http.post<ItemInventario>(`${this.API}/inventario`, data);
  }

  actualizarItem(id: number, data: Partial<ItemInventarioCreate>): Observable<ItemInventario> {
    return this.http.put<ItemInventario>(`${this.API}/inventario/${id}`, data);
  }

  eliminarItem(id: number): Observable<void> {
    return this.http.delete<void>(`${this.API}/inventario/${id}`);
  }

  entradaStock(id: number, data: EntradaStock): Observable<ItemInventario> {
    return this.http.post<ItemInventario>(`${this.API}/inventario/${id}/entrada`, data);
  }

  listFormulasPendientes(q?: string): Observable<FormulaPendiente[]> {
    const params: Record<string, string> = {};
    if (q) params['q'] = q;
    return this.http.get<FormulaPendiente[]>(`${this.API}/formulas-pendientes`, { params });
  }

  dispensar(data: DispensarCreate): Observable<Dispensacion> {
    return this.http.post<Dispensacion>(`${this.API}/dispensar`, data);
  }

  listDispensaciones(page = 1): Observable<Dispensacion[]> {
    return this.http.get<Dispensacion[]>(`${this.API}/dispensaciones`, { params: { page: String(page) } });
  }

  listMovimientos(itemId?: number, tipo?: string, page = 1): Observable<Movimiento[]> {
    const params: Record<string, string> = { page: String(page) };
    if (itemId) params['item_id'] = String(itemId);
    if (tipo) params['tipo'] = tipo;
    return this.http.get<Movimiento[]>(`${this.API}/movimientos`, { params });
  }
}
