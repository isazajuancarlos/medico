import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

export interface Resumen {
  fecha: string;
  total_pacientes: number;
  citas_hoy: number;
  citas_mes: number;
  citas_atendidas_mes: number;
  tasa_asistencia: number;
  consultas_mes: number;
  urgencias_activas: number;
  hospitalizados: number;
  total_camas: number;
  camas_disponibles: number;
  camas_ocupadas: number;
  tasa_ocupacion: number;
  facturas_pendientes: number;
  valor_pendiente: number;
}

export interface EstadisticasCitas {
  periodo: { inicio: string; fin: string };
  total: number;
  by_estado: Record<string, number>;
  by_tipo: Record<string, number>;
  tendencia: { fecha: string; cantidad: number }[];
  by_medico: { medico: string; cantidad: number }[];
}

export interface EstadisticasPacientes {
  total: number;
  by_sexo: Record<string, number>;
  by_grupo_edad: Record<string, number>;
  by_tipo_usuario: Record<string, number>;
  by_eps: { eps: string; cantidad: number }[];
  by_zona: Record<string, number>;
}

export interface EstadisticasDiagnosticos {
  periodo: { inicio: string; fin: string };
  diagnosticos: { codigo: string; descripcion: string; cantidad: number }[];
}

export interface Medico {
  medico_id: number;
  nombre: string;
  especialidad: string;
  citas_atendidas: number;
  historias_creadas: number;
}

export interface EstadisticasProduccion {
  periodo: { inicio: string; fin: string };
  medicos: Medico[];
}

export interface EstadisticasFinanciero {
  periodo: { inicio: string; fin: string };
  total_valor: number;
  by_estado: { estado: string; cantidad: number; valor: number }[];
  tendencia_mensual: { periodo: string; cantidad: number; valor: number }[];
  glosas: { estado: string; cantidad: number; valor: number }[];
  valor_glosado_pendiente: number;
}

export interface EstadisticasUrgencias {
  periodo: { inicio: string; fin: string };
  total: number;
  by_triage: Record<string, number>;
  by_destino: Record<string, number>;
  tiempo_espera_promedio_min: number;
}

@Injectable({ providedIn: 'root' })
export class EstadisticasService {
  private readonly API = '/api/estadisticas';

  constructor(private http: HttpClient) {}

  getResumen(): Observable<Resumen> {
    return this.http.get<Resumen>(`${this.API}/resumen`);
  }

  getCitas(fechaInicio?: string, fechaFin?: string): Observable<EstadisticasCitas> {
    const params: Record<string, string> = {};
    if (fechaInicio) params['fecha_inicio'] = fechaInicio;
    if (fechaFin) params['fecha_fin'] = fechaFin;
    return this.http.get<EstadisticasCitas>(`${this.API}/citas`, { params });
  }

  getPacientes(): Observable<EstadisticasPacientes> {
    return this.http.get<EstadisticasPacientes>(`${this.API}/pacientes`);
  }

  getDiagnosticos(fechaInicio?: string, fechaFin?: string): Observable<EstadisticasDiagnosticos> {
    const params: Record<string, string> = {};
    if (fechaInicio) params['fecha_inicio'] = fechaInicio;
    if (fechaFin) params['fecha_fin'] = fechaFin;
    return this.http.get<EstadisticasDiagnosticos>(`${this.API}/diagnosticos`, { params });
  }

  getProduccion(fechaInicio?: string, fechaFin?: string): Observable<EstadisticasProduccion> {
    const params: Record<string, string> = {};
    if (fechaInicio) params['fecha_inicio'] = fechaInicio;
    if (fechaFin) params['fecha_fin'] = fechaFin;
    return this.http.get<EstadisticasProduccion>(`${this.API}/produccion`, { params });
  }

  getFinanciero(fechaInicio?: string, fechaFin?: string): Observable<EstadisticasFinanciero> {
    const params: Record<string, string> = {};
    if (fechaInicio) params['fecha_inicio'] = fechaInicio;
    if (fechaFin) params['fecha_fin'] = fechaFin;
    return this.http.get<EstadisticasFinanciero>(`${this.API}/financiero`, { params });
  }

  getUrgencias(fechaInicio?: string, fechaFin?: string): Observable<EstadisticasUrgencias> {
    const params: Record<string, string> = {};
    if (fechaInicio) params['fecha_inicio'] = fechaInicio;
    if (fechaFin) params['fecha_fin'] = fechaFin;
    return this.http.get<EstadisticasUrgencias>(`${this.API}/urgencias`, { params });
  }
}
