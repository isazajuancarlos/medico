import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

export interface PlanRehabilitacion {
  id: number;
  paciente_id: number;
  terapeuta_id: number;
  fecha_creacion: string;
  diagnostico_funcional: string;
  objetivos: string;
  sesiones_autorizadas: number;
  sesiones_realizadas: number;
  tipo_terapia: string;
  frecuencia: string;
  duracion_sesion_min: number;
  activo: boolean;
  fecha_cierre?: string;
  resultado_final?: string;
  paciente_nombre?: string;
  paciente_identificacion?: string;
  terapeuta_nombre?: string;
}

export interface SesionTerapia {
  id: number;
  plan_id: number;
  paciente_id: number;
  terapeuta_id: number;
  fecha_sesion: string;
  numero_sesion: number;
  evaluacion_funcional: string;
  dolor_eva?: number;
  rango_movimiento?: string;
  tecnicas_aplicadas: string;
  ejercicios_domicilio?: string;
  progreso?: string;
  observaciones?: string;
  terapeuta_nombre?: string;
}

export interface PlanCreate {
  paciente_id: number;
  diagnostico_funcional: string;
  objetivos: string;
  sesiones_autorizadas: number;
  tipo_terapia: string;
  frecuencia: string;
  duracion_sesion_min: number;
}

export interface SesionCreate {
  evaluacion_funcional: string;
  dolor_eva?: number;
  rango_movimiento?: string;
  tecnicas_aplicadas: string;
  ejercicios_domicilio?: string;
  progreso?: string;
  observaciones?: string;
}

@Injectable({ providedIn: 'root' })
export class RehabilitacionService {
  private readonly API = '/api/rehabilitacion';

  constructor(private http: HttpClient) {}

  listPlanes(pacienteId?: number, tipoTerapia?: string, activo?: boolean): Observable<PlanRehabilitacion[]> {
    const params: Record<string, string> = {};
    if (pacienteId) params['paciente_id'] = String(pacienteId);
    if (tipoTerapia) params['tipo_terapia'] = tipoTerapia;
    if (activo !== undefined) params['activo'] = String(activo);
    return this.http.get<PlanRehabilitacion[]>(`${this.API}/planes`, { params });
  }

  getPlan(id: number): Observable<PlanRehabilitacion> {
    return this.http.get<PlanRehabilitacion>(`${this.API}/planes/${id}`);
  }

  createPlan(data: PlanCreate): Observable<PlanRehabilitacion> {
    return this.http.post<PlanRehabilitacion>(`${this.API}/planes`, data);
  }

  updatePlan(id: number, data: Partial<PlanCreate> & { activo?: boolean; resultado_final?: string; fecha_cierre?: string }): Observable<PlanRehabilitacion> {
    return this.http.put<PlanRehabilitacion>(`${this.API}/planes/${id}`, data);
  }

  listSesiones(planId: number): Observable<SesionTerapia[]> {
    return this.http.get<SesionTerapia[]>(`${this.API}/planes/${planId}/sesiones`);
  }

  createSesion(planId: number, data: SesionCreate): Observable<SesionTerapia> {
    return this.http.post<SesionTerapia>(`${this.API}/planes/${planId}/sesiones`, data);
  }

  updateSesion(sesionId: number, data: Partial<SesionCreate>): Observable<SesionTerapia> {
    return this.http.put<SesionTerapia>(`${this.API}/sesiones/${sesionId}`, data);
  }

  searchPacientes(q: string): Observable<any[]> {
    return this.http.get<any[]>('/api/pacientes', { params: { search: q } });
  }
}
