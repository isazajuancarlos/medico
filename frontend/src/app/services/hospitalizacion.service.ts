import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

export interface Cama {
  id: number;
  numero: string;
  piso: string;
  servicio: string;
  estado: string;
  tiene_oxigeno: boolean;
  tiene_monitor: boolean;
  es_aislamiento: boolean;
}

export interface Hospitalizacion {
  id: number;
  paciente_id: number;
  cama_id: number;
  medico_tratante_id: number;
  fecha_ingreso: string;
  fecha_egreso?: string;
  diagnostico_ingreso: string;
  via_ingreso: string;
  estado: string;
  motivo_ingreso?: string;
  observaciones_ingreso?: string;
  diagnostico_egreso?: string;
  resumen_clinico?: string;
  paciente_nombre?: string;
  paciente_identificacion?: string;
  cama_numero?: string;
  cama_piso?: string;
  cama_servicio?: string;
  medico_nombre?: string;
}

export interface MedicoBasic {
  id: number;
  nombre_completo: string;
}

export interface IngresoCreate {
  paciente_id: number;
  cama_id: number;
  medico_id: number;
  diagnostico_ingreso: string;
  via_ingreso: string;
  motivo_ingreso?: string;
  observaciones_ingreso?: string;
}

export interface EgresoUpdate {
  tipo_egreso: string;
  diagnostico_egreso?: string;
  resumen_clinico?: string;
}

@Injectable({ providedIn: 'root' })
export class HospitalizacionService {
  private readonly API = '/api/hospitalizacion';

  constructor(private http: HttpClient) {}

  listHospitalizaciones(estado?: string): Observable<Hospitalizacion[]> {
    const params: Record<string, string> = {};
    if (estado) params['estado'] = estado;
    return this.http.get<Hospitalizacion[]>(`${this.API}`, { params });
  }

  listCamas(estado?: string): Observable<Cama[]> {
    const params: Record<string, string> = {};
    if (estado) params['estado'] = estado;
    return this.http.get<Cama[]>(`${this.API}/camas`, { params });
  }

  listMedicos(): Observable<MedicoBasic[]> {
    return this.http.get<MedicoBasic[]>(`${this.API}/medicos`);
  }

  searchPacientes(q: string): Observable<any[]> {
    return this.http.get<any[]>('/api/pacientes', { params: { search: q } });
  }

  ingresar(data: IngresoCreate): Observable<Hospitalizacion> {
    return this.http.post<Hospitalizacion>(`${this.API}/ingresar`, data);
  }

  egreso(hospId: number, data: EgresoUpdate): Observable<Hospitalizacion> {
    return this.http.put<Hospitalizacion>(`${this.API}/${hospId}/egreso`, data);
  }
}
