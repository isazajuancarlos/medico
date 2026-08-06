import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable, map } from 'rxjs';

export interface Sala {
  id: number;
  nombre: string;
  tipo: string;
  estado: string;
  equipamiento: Record<string, unknown>;
  institucion_id?: number;
}

export interface Programacion {
  id: number;
  quirofano_id: number;
  paciente_id: number;
  cirujano_id: number;
  anestesiologo_id?: number;
  fecha_programada: string;
  duracion_estimada_min: number;
  procedimiento: string;
  codigo_cups?: string;
  tipo_anestesia?: string;
  estado: string;
  hora_inicio_real?: string;
  hora_fin_real?: string;
  complicaciones?: string;
  nota_quirurgica?: string;
  hallazgos_quirurgicos?: string;
  sangrado_estimado_ml?: number;
  especimenes_patologia?: string;
  ayuno_verificado: boolean;
  labs_prequirurgicos: boolean;
  valoracion_preanestesica: boolean;
  hemoclasificacion?: string;
  reserva_sangre: boolean;
  // Enriched
  paciente_nombre?: string;
  paciente_identificacion?: string;
  quirofano_nombre?: string;
  cirujano_nombre?: string;
  anestesiologo_nombre?: string;
}

export interface ProgramacionCreate {
  quirofano_id: number;
  paciente_id: number;
  cirujano_id: number;
  anestesiologo_id?: number;
  fecha_programada: string;
  duracion_estimada_min: number;
  procedimiento: string;
  codigo_cups?: string;
  tipo_anestesia?: string;
}

export interface ChecklistUpdate {
  ayuno_verificado?: boolean;
  labs_prequirurgicos?: boolean;
  valoracion_preanestesica?: boolean;
  hemoclasificacion?: string;
  reserva_sangre?: boolean;
}

export interface CompletarCirugia {
  nota_quirurgica?: string;
  hallazgos_quirurgicos?: string;
  sangrado_estimado_ml?: number;
  especimenes_patologia?: string;
  complicaciones?: string;
}

export interface MedicoBasic {
  id: number;
  nombre_completo: string;
  rol?: string;
}

@Injectable({ providedIn: 'root' })
export class QuirofanoService {
  private readonly API = '/api/quirofano';

  constructor(private http: HttpClient) {}

  listSalas(): Observable<Sala[]> {
    return this.http.get<Sala[]>(`${this.API}/salas`);
  }

  listProgramaciones(estado?: string): Observable<Programacion[]> {
    const params: Record<string, string> = {};
    if (estado) params['estado'] = estado;
    return this.http.get<Programacion[]>(`${this.API}/programaciones`, { params });
  }

  getProgramacion(id: number): Observable<Programacion> {
    return this.http.get<Programacion>(`${this.API}/programaciones/${id}`);
  }

  listMedicos(): Observable<MedicoBasic[]> {
    return this.http.get<MedicoBasic[]>(`${this.API}/medicos`);
  }

  crear(data: ProgramacionCreate): Observable<Programacion> {
    return this.http.post<Programacion>(`${this.API}/programaciones`, data);
  }

  updateChecklist(id: number, data: ChecklistUpdate): Observable<Programacion> {
    return this.http.put<Programacion>(`${this.API}/programaciones/${id}/checklist`, data);
  }

  iniciar(id: number): Observable<Programacion> {
    return this.http.put<Programacion>(`${this.API}/programaciones/${id}/iniciar`, {});
  }

  completar(id: number, data: CompletarCirugia): Observable<Programacion> {
    return this.http.put<Programacion>(`${this.API}/programaciones/${id}/completar`, data);
  }

  cancelar(id: number, motivo?: string): Observable<Programacion> {
    return this.http.put<Programacion>(`${this.API}/programaciones/${id}/cancelar`, { motivo });
  }

  /**
   * El binario Rust sirve el paciente en crudo (`nombre1`, `numero_documento`),
   * y el selector pinta `nombre_completo` e `identificacion`. Sin este `map` la
   * lista sale con « — » en cada fila: no falla, no avisa, y no se puede elegir
   * a nadie.
   *
   * Es el mismo derivado que ya hacen epicrisis, evoluciones, interconsultas y
   * nota operatoria; este servicio se había quedado sin él.
   */
  searchPacientes(q: string): Observable<any[]> {
    return this.http.get<any[]>('/api/pacientes', { params: { search: q } }).pipe(
      map(ps => ps.map(p => ({
        ...p,
        nombre_completo: [p.nombre1, p.nombre2, p.apellido1, p.apellido2].filter(Boolean).join(' '),
        identificacion: p.numero_documento,
      }))),
    );
  }
}
