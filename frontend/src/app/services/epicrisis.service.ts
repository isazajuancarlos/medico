import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

export interface Epicrisis {
  id: number;
  paciente_id: number;
  medico_tratante_id: number;
  fecha_ingreso: string;
  fecha_egreso: string;
  dias_estancia: number;
  diagnostico_ingreso: string;
  diagnostico_egreso: string;
  complicaciones?: string;
  resumen_evolucion: string;
  procedimientos_realizados: string;
  examenes_relevantes?: string;
  recomendaciones_egreso: string;
  medicamentos_egreso?: string;
  controles_programados?: string;
  pronostico: string;
  incapacidad_dias: number;
  firma_medico_path?: string;
  paciente_nombre?: string;
  paciente_identificacion?: string;
  medico_nombre?: string;
}

export interface EpicrisisCreate {
  paciente_id: number;
  fecha_ingreso: string;
  fecha_egreso: string;
  dias_estancia: number;
  diagnostico_ingreso: string;
  diagnostico_egreso: string;
  complicaciones?: string;
  resumen_evolucion: string;
  procedimientos_realizados: string;
  examenes_relevantes?: string;
  recomendaciones_egreso: string;
  medicamentos_egreso?: string;
  controles_programados?: string;
  pronostico: string;
  incapacidad_dias: number;
}

@Injectable({ providedIn: 'root' })
export class EpicrisisService {
  private readonly API = '/api/epicrisis';

  constructor(private http: HttpClient) {}

  list(pacienteId?: number): Observable<Epicrisis[]> {
    const params: Record<string, string> = {};
    if (pacienteId) params['paciente_id'] = String(pacienteId);
    return this.http.get<Epicrisis[]>(`${this.API}/`, { params });
  }

  get(id: number): Observable<Epicrisis> {
    return this.http.get<Epicrisis>(`${this.API}/${id}`);
  }

  create(data: EpicrisisCreate): Observable<Epicrisis> {
    return this.http.post<Epicrisis>(`${this.API}/`, data);
  }

  update(id: number, data: Partial<EpicrisisCreate>): Observable<Epicrisis> {
    return this.http.put<Epicrisis>(`${this.API}/${id}`, data);
  }

  delete(id: number): Observable<void> {
    return this.http.delete<void>(`${this.API}/${id}`);
  }

  searchPacientes(q: string): Observable<any[]> {
    return this.http.get<any[]>('/api/pacientes', { params: { search: q } });
  }
}
