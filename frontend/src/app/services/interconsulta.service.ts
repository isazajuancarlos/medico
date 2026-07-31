import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';
import { map, switchMap } from 'rxjs/operators';

// La interconsulta es un DOCUMENTO CLÍNICO de tipo INTERCONSULTA. Su estado
// (SOLICITADA→ACEPTADA→RESPONDIDA, o CANCELADA) y su respuesta viven en el
// `contenido`. Las transiciones leen el documento y reescriben SOLO el estado
// (y la respuesta), sin pisar el resto del contenido.

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

const TIPO = 'INTERCONSULTA';

@Injectable({ providedIn: 'root' })
export class InterconsultaService {
  private readonly API = '/api/documentos';

  constructor(private http: HttpClient) {}

  list(params: { estado?: string; prioridad?: string; solo_mias?: boolean; solo_para_mi?: boolean } = {}): Observable<Interconsulta[]> {
    // El estado y la prioridad viven en el contenido, así que se filtran en el
    // cliente sobre la worklist del tipo.
    return this.http.get<any[]>(this.API, { params: { tipo: TIPO } }).pipe(
      map(ds => {
        let r = ds.map(d => this.aInter(d));
        if (params.estado) r = r.filter(x => x.estado === params.estado);
        if (params.prioridad) r = r.filter(x => x.prioridad === params.prioridad);
        return r;
      }),
    );
  }

  get(id: number): Observable<Interconsulta> {
    return this.http.get<any>(`${this.API}/${id}`).pipe(map(d => this.aInter(d)));
  }

  create(data: InterconsultaCreate): Observable<{ id: number }> {
    const { paciente_id, ...resto } = data;
    return this.http.post<{ id: number }>(this.API, {
      tipo: TIPO,
      paciente_id,
      historia_id: null,
      titulo: '',
      contenido: { ...resto, estado: 'SOLICITADA' },
    });
  }

  aceptar(id: number): Observable<Interconsulta> {
    return this.transicion(id, { estado: 'ACEPTADA' });
  }

  responder(id: number, data: InterconsultaResponder): Observable<Interconsulta> {
    return this.transicion(id, {
      estado: 'RESPONDIDA',
      respuesta: data.respuesta,
      recomendaciones: data.recomendaciones ?? null,
      fecha_respuesta: new Date().toISOString(),
    });
  }

  cancelar(id: number): Observable<Interconsulta> {
    return this.transicion(id, { estado: 'CANCELADA' });
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

  listEspecialidades(): Observable<any[]> {
    return this.http.get<any[]>('/api/especialidades');
  }

  // ── Transición de estado: leer, fusionar el cambio, reescribir ───
  private transicion(id: number, cambios: Record<string, unknown>): Observable<Interconsulta> {
    return this.http.get<any>(`${this.API}/${id}`).pipe(
      switchMap(doc => this.http.put<any>(`${this.API}/${id}`, {
        historia_id: doc.historia_id ?? null,
        titulo: doc.titulo ?? '',
        contenido: { ...(doc.contenido ?? {}), ...cambios },
      })),
      map(d => this.aInter(d)),
    );
  }

  private aInter(d: any): Interconsulta {
    return {
      ...(d.contenido ?? {}),
      id: d.id,
      paciente_id: d.paciente_id,
      medico_solicita_id: d.medico_id,
      fecha_solicitud: d.fecha,
      paciente_nombre: d.paciente_nombre,
      paciente_identificacion: d.paciente_identificacion,
      medico_solicita_nombre: d.medico_nombre,
    } as Interconsulta;
  }
}
