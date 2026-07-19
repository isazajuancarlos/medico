import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

export interface ValoracionNutricional {
  id: number;
  paciente_id: number;
  nutricionista_id: number;
  fecha_valoracion: string;
  peso: number;
  talla: number;
  imc: number;
  circunferencia_brazo?: number;
  pliegue_tricipital?: number;
  screening_nutricional: string;
  puntaje_screening?: number;
  riesgo_nutricional: string;
  diagnostico_nutricional: string;
  requerimiento_calorico?: number;
  requerimiento_proteico?: number;
  albumina?: number;
  prealbumina?: number;
  observaciones?: string;
  hospitalizacion_id?: number;
  paciente_nombre?: string;
  paciente_identificacion?: string;
  nutricionista_nombre?: string;
}

export interface PlanAlimentacion {
  id: number;
  valoracion_id: number;
  paciente_id: number;
  nutricionista_id: number;
  fecha_creacion: string;
  tipo_dieta: string;
  calorias_totales: number;
  distribucion_calorica: Record<string, any>;
  menu_diario: Record<string, any>;
  restricciones?: string;
  suplementos?: string;
  via_alimentacion: string;
  activo: boolean;
  observaciones?: string;
  nutricionista_nombre?: string;
}

export interface ValoracionCreate {
  paciente_id: number;
  peso: number;
  talla: number;
  imc: number;
  circunferencia_brazo?: number;
  pliegue_tricipital?: number;
  screening_nutricional: string;
  puntaje_screening?: number;
  riesgo_nutricional: string;
  diagnostico_nutricional: string;
  requerimiento_calorico?: number;
  requerimiento_proteico?: number;
  albumina?: number;
  prealbumina?: number;
  observaciones?: string;
}

export interface PlanCreate {
  tipo_dieta: string;
  calorias_totales: number;
  distribucion_calorica: Record<string, any>;
  menu_diario: Record<string, any>;
  restricciones?: string;
  suplementos?: string;
  via_alimentacion: string;
  observaciones?: string;
}

@Injectable({ providedIn: 'root' })
export class NutricionService {
  private readonly API = '/api/nutricion';

  constructor(private http: HttpClient) {}

  listValoraciones(pacienteId?: number, riesgo?: string): Observable<ValoracionNutricional[]> {
    const params: Record<string, string> = {};
    if (pacienteId) params['paciente_id'] = String(pacienteId);
    if (riesgo) params['riesgo'] = riesgo;
    return this.http.get<ValoracionNutricional[]>(`${this.API}/valoraciones`, { params });
  }

  getValoracion(id: number): Observable<ValoracionNutricional> {
    return this.http.get<ValoracionNutricional>(`${this.API}/valoraciones/${id}`);
  }

  createValoracion(data: ValoracionCreate): Observable<ValoracionNutricional> {
    return this.http.post<ValoracionNutricional>(`${this.API}/valoraciones`, data);
  }

  updateValoracion(id: number, data: Partial<ValoracionCreate>): Observable<ValoracionNutricional> {
    return this.http.put<ValoracionNutricional>(`${this.API}/valoraciones/${id}`, data);
  }

  deleteValoracion(id: number): Observable<void> {
    return this.http.delete<void>(`${this.API}/valoraciones/${id}`);
  }

  getPlan(valoracionId: number): Observable<PlanAlimentacion> {
    return this.http.get<PlanAlimentacion>(`${this.API}/valoraciones/${valoracionId}/plan`);
  }

  createPlan(valoracionId: number, data: PlanCreate): Observable<PlanAlimentacion> {
    return this.http.post<PlanAlimentacion>(`${this.API}/valoraciones/${valoracionId}/plan`, data);
  }

  updatePlan(planId: number, data: Partial<PlanCreate>): Observable<PlanAlimentacion> {
    return this.http.put<PlanAlimentacion>(`${this.API}/planes/${planId}`, data);
  }

  searchPacientes(q: string): Observable<any[]> {
    return this.http.get<any[]>('/api/pacientes', { params: { search: q } });
  }
}
