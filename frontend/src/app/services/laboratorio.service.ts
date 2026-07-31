import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

export interface OrdenLab {
  id: number;
  historia_id: number;
  paciente_id?: number;
  medico_ordena_id?: number;
  fecha_orden: string;
  estado: string;
  tipo: string;
  codigo_cups?: string;
  descripcion: string;
  justificacion: string;
  paciente_nombre?: string;
  paciente_identificacion?: string;
  medico_nombre?: string;
  tiene_resultado: boolean;
}

export interface ParametroResultado {
  valor: string;
  unidad: string;
  ref_min?: string;
  ref_max?: string;
  critico: boolean;
}

export interface ResultadoLab {
  id: number;
  orden_id: number;
  paciente_id?: number;
  bioanalista_id: number;
  bioanalista_nombre?: string;
  fecha_resultado: string;
  resultados_json: Record<string, ParametroResultado>;
  observaciones?: string;
  /**
   * Verdadero si quien procesó lo declaró **o** si algún parámetro viene marcado
   * como crítico. Lo decide el servidor tomando la disyunción: un potasio de 7,2
   * no puede pasar por normal porque nadie se acordó de marcar la casilla.
   */
  valores_criticos: boolean;
  validado: boolean;
  fecha_validacion?: string;
  /** Quién validó. Una afirmación sin autor no acredita nada (Ley 527/1999). */
  validado_por_id?: number;
}

export interface ResultadoCreate {
  resultados_json: Record<string, ParametroResultado>;
  observaciones?: string;
  valores_criticos: boolean;
}

export interface PerfilLab {
  id: number;
  nombre: string;
  descripcion?: string;
  examenes: Array<{ nombre: string; unidad: string; ref_min?: string; ref_max?: string }>;
  activo: boolean;
}

export interface PerfilCreate {
  nombre: string;
  descripcion?: string;
  examenes: Array<{ nombre: string; unidad: string; ref_min?: string; ref_max?: string }>;
}

@Injectable({ providedIn: 'root' })
export class LaboratorioService {
  private readonly API = '/api/laboratorio';

  constructor(private http: HttpClient) {}

  // ── Órdenes ──────────────────────────────────────────────────────
  listOrdenes(estado?: string, pacienteId?: number): Observable<OrdenLab[]> {
    const params: Record<string, string> = {};
    if (estado) params['estado'] = estado;
    if (pacienteId) params['paciente_id'] = String(pacienteId);
    return this.http.get<OrdenLab[]>(`${this.API}/ordenes`, { params });
  }

  getOrden(ordenId: number): Observable<OrdenLab> {
    return this.http.get<OrdenLab>(`${this.API}/ordenes/${ordenId}`);
  }

  updateEstado(ordenId: number, estado: string): Observable<OrdenLab> {
    return this.http.put<OrdenLab>(`${this.API}/ordenes/${ordenId}/estado`, { estado });
  }

  // ── Resultados ───────────────────────────────────────────────────
  getResultado(ordenId: number): Observable<ResultadoLab> {
    return this.http.get<ResultadoLab>(`${this.API}/ordenes/${ordenId}/resultado`);
  }

  createResultado(ordenId: number, data: ResultadoCreate): Observable<ResultadoLab> {
    return this.http.post<ResultadoLab>(`${this.API}/ordenes/${ordenId}/resultado`, data);
  }

  updateResultado(ordenId: number, data: Partial<ResultadoCreate>): Observable<ResultadoLab> {
    return this.http.put<ResultadoLab>(`${this.API}/ordenes/${ordenId}/resultado`, data);
  }

  validarResultado(ordenId: number): Observable<ResultadoLab> {
    return this.http.post<ResultadoLab>(`${this.API}/ordenes/${ordenId}/resultado/validar`, {});
  }

  // ── Historial paciente ───────────────────────────────────────────
  getResultadosPaciente(pacienteId: number): Observable<ResultadoLab[]> {
    return this.http.get<ResultadoLab[]>(`${this.API}/paciente/${pacienteId}/resultados`);
  }

  // ── Perfiles ─────────────────────────────────────────────────────
  listPerfiles(): Observable<PerfilLab[]> {
    return this.http.get<PerfilLab[]>(`${this.API}/perfiles`);
  }

  createPerfil(data: PerfilCreate): Observable<PerfilLab> {
    return this.http.post<PerfilLab>(`${this.API}/perfiles`, data);
  }

  deletePerfil(perfilId: number): Observable<void> {
    return this.http.delete<void>(`${this.API}/perfiles/${perfilId}`);
  }
}
