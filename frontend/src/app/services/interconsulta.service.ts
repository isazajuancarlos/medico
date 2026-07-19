import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

export interface Interconsulta {
  id: number;
  paciente_id: number;
  hospitalizacion_id?: number;
  medico_solicita_id: number;
  especialidad_solicitada_id: number;
  medico_responde_id?: number;
  prioridad: string;
  estado: string;
  motivo: string;
  historia_clinica_resumen?: string;
  respuesta?: string;
  recomendaciones?: string;
  fecha_solicitud: string;
  fecha_respuesta?: string;
  paciente_nombre?: string;
  paciente_identificacion?: string;
  medico_solicita_nombre?: string;
  medico_responde_nombre?: string;
  especialidad_nombre?: string;
}

export interface InterconsultaCreate {
  paciente_id: number;
  especialidad_solicitada_id: number;
  prioridad: string;
  motivo: string;
  historia_clinica_resumen?: string;
  hospitalizacion_id?: number;
}

export interface InterconsultaResponder {
  respuesta: string;
  recomendaciones?: string;
}

@Injectable({ providedIn: 'root' })
export class InterconsultaService {
  private readonly API = '/api/interconsultas';

  constructor(private http: HttpClient) {}

  list(params: { estado?: string; prioridad?: string; solo_mias?: boolean; solo_para_mi?: boolean } = {}): Observable<Interconsulta[]> {
    const p: Record<string, string> = {};
    if (params.estado) p['estado'] = params.estado;
    if (params.prioridad) p['prioridad'] = params.prioridad;
    if (params.solo_mias) p['solo_mias'] = 'true';
    if (params.solo_para_mi) p['solo_para_mi'] = 'true';
    return this.http.get<Interconsulta[]>(`${this.API}/`, { params: p });
  }

  get(id: number): Observable<Interconsulta> {
    return this.http.get<Interconsulta>(`${this.API}/${id}`);
  }

  create(data: InterconsultaCreate): Observable<Interconsulta> {
    return this.http.post<Interconsulta>(`${this.API}/`, data);
  }

  aceptar(id: number): Observable<Interconsulta> {
    return this.http.put<Interconsulta>(`${this.API}/${id}/aceptar`, {});
  }

  responder(id: number, data: InterconsultaResponder): Observable<Interconsulta> {
    return this.http.put<Interconsulta>(`${this.API}/${id}/responder`, data);
  }

  cancelar(id: number): Observable<Interconsulta> {
    return this.http.put<Interconsulta>(`${this.API}/${id}/cancelar`, {});
  }

  searchPacientes(q: string): Observable<any[]> {
    return this.http.get<any[]>('/api/pacientes', { params: { search: q } });
  }

  listEspecialidades(): Observable<any[]> {
    return this.http.get<any[]>('/api/historias/especialidades');
  }
}
