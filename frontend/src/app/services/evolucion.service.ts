import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

export interface Evolucion {
  id: number;
  paciente_id: number;
  medico_id: number;
  fecha: string;
  subjetivo: string;
  objetivo: string;
  analisis: string;
  plan: string;
  historia_relacionada_id?: number;
  tension_arterial?: string;
  frecuencia_cardiaca?: number;
  temperatura?: number;
  peso?: number;
  paciente_nombre?: string;
  paciente_identificacion?: string;
  medico_nombre?: string;
}

export interface EvolucionCreate {
  paciente_id: number;
  subjetivo: string;
  objetivo: string;
  analisis: string;
  plan: string;
  historia_relacionada_id?: number;
  tension_arterial?: string;
  frecuencia_cardiaca?: number;
  temperatura?: number;
  peso?: number;
}

@Injectable({ providedIn: 'root' })
export class EvolucionService {
  private readonly API = '/api/evoluciones';

  constructor(private http: HttpClient) {}

  list(params: { paciente_id?: number; medico_id?: number; fecha_desde?: string; fecha_hasta?: string } = {}): Observable<Evolucion[]> {
    const p: Record<string, string> = {};
    if (params.paciente_id) p['paciente_id'] = String(params.paciente_id);
    if (params.medico_id) p['medico_id'] = String(params.medico_id);
    if (params.fecha_desde) p['fecha_desde'] = params.fecha_desde;
    if (params.fecha_hasta) p['fecha_hasta'] = params.fecha_hasta;
    return this.http.get<Evolucion[]>(`${this.API}/`, { params: p });
  }

  get(id: number): Observable<Evolucion> {
    return this.http.get<Evolucion>(`${this.API}/${id}`);
  }

  create(data: EvolucionCreate): Observable<Evolucion> {
    return this.http.post<Evolucion>(`${this.API}/`, data);
  }

  update(id: number, data: Partial<EvolucionCreate>): Observable<Evolucion> {
    return this.http.put<Evolucion>(`${this.API}/${id}`, data);
  }

  delete(id: number): Observable<void> {
    return this.http.delete<void>(`${this.API}/${id}`);
  }

  searchPacientes(q: string): Observable<any[]> {
    return this.http.get<any[]>('/api/pacientes', { params: { search: q } });
  }
}
