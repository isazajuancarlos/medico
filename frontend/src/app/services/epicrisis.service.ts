import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';
import { map } from 'rxjs/operators';

// La epicrisis es un DOCUMENTO CLÍNICO de tipo EPICRISIS: todos sus campos viven
// en el `contenido` JSON del documento. Este servicio adapta la forma plana que
// espera la pantalla a la espina genérica /api/documentos.

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

const TIPO = 'EPICRISIS';

@Injectable({ providedIn: 'root' })
export class EpicrisisService {
  private readonly API = '/api/documentos';

  constructor(private http: HttpClient) {}

  list(pacienteId?: number): Observable<Epicrisis[]> {
    const params: Record<string, string> = { tipo: TIPO };
    if (pacienteId) params['paciente_id'] = String(pacienteId);
    return this.http.get<any[]>(this.API, { params }).pipe(map(ds => ds.map(d => this.aEpicrisis(d))));
  }

  get(id: number): Observable<Epicrisis> {
    return this.http.get<any>(`${this.API}/${id}`).pipe(map(d => this.aEpicrisis(d)));
  }

  create(data: EpicrisisCreate): Observable<{ id: number }> {
    return this.http.post<{ id: number }>(this.API, this.aDocumento(data));
  }

  update(id: number, data: Partial<EpicrisisCreate>): Observable<Epicrisis> {
    return this.http.put<any>(`${this.API}/${id}`, this.aDocumento(data)).pipe(map(d => this.aEpicrisis(d)));
  }

  delete(id: number): Observable<void> {
    return this.http.delete<void>(`${this.API}/${id}`);
  }

  searchPacientes(q: string): Observable<any[]> {
    return this.http.get<any[]>('/api/pacientes').pipe(
      map(ps => ps.map(p => ({
        ...p,
        nombre_completo: [p.nombre1, p.nombre2, p.apellido1, p.apellido2].filter(Boolean).join(' '),
        identificacion: p.numero_documento,
      }))),
    );
  }

  // ── Traducción documento ⇄ epicrisis ────────────────────────────
  private aDocumento(data: Partial<EpicrisisCreate>) {
    const { paciente_id, ...contenido } = data;
    return { tipo: TIPO, paciente_id, historia_id: null, titulo: '', contenido };
  }

  private aEpicrisis(d: any): Epicrisis {
    // El contenido trae todos los campos de la epicrisis; lo de arriba, la
    // identidad y los nombres unidos de la worklist.
    return {
      ...(d.contenido ?? {}),
      id: d.id,
      paciente_id: d.paciente_id,
      medico_tratante_id: d.medico_id,
      paciente_nombre: d.paciente_nombre,
      paciente_identificacion: d.paciente_identificacion,
      medico_nombre: d.medico_nombre,
    } as Epicrisis;
  }
}
