import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';
import { map } from 'rxjs/operators';

// La evolución es un DOCUMENTO CLÍNICO de tipo EVOLUCION: sus campos SOAP y los
// signos vitales viven en el `contenido` JSON del documento. Este servicio es un
// adaptador que presenta la forma plana que espera la pantalla y traduce a la
// espina genérica /api/documentos por debajo.

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
  firmada?: boolean;
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

const TIPO = 'EVOLUCION';

@Injectable({ providedIn: 'root' })
export class EvolucionService {
  private readonly API = '/api/documentos';

  constructor(private http: HttpClient) {}

  list(params: { paciente_id?: number; medico_id?: number; fecha_desde?: string; fecha_hasta?: string } = {}): Observable<Evolucion[]> {
    const p: Record<string, string> = { tipo: TIPO };
    if (params.paciente_id) p['paciente_id'] = String(params.paciente_id);
    // La worklist trae la vista ligera (con nombres, sin contenido): basta para
    // la tabla. El detalle SOAP se pide con get() al abrir una.
    return this.http.get<any[]>(this.API, { params: p }).pipe(
      map(ds => ds.map(d => this.deVista(d))),
    );
  }

  get(id: number): Observable<Evolucion> {
    return this.http.get<any>(`${this.API}/${id}`).pipe(map(d => this.deDocumento(d)));
  }

  create(data: EvolucionCreate): Observable<{ id: number }> {
    return this.http.post<{ id: number }>(this.API, {
      tipo: TIPO,
      paciente_id: data.paciente_id,
      historia_id: data.historia_relacionada_id ?? null,
      titulo: '',
      contenido: this.contenido(data),
    });
  }

  update(id: number, data: Partial<EvolucionCreate>): Observable<Evolucion> {
    return this.http.put<any>(`${this.API}/${id}`, {
      historia_id: data.historia_relacionada_id ?? null,
      titulo: '',
      contenido: this.contenido(data),
    }).pipe(map(d => this.deDocumento(d)));
  }

  delete(id: number): Observable<void> {
    return this.http.delete<void>(`${this.API}/${id}`);
  }

  searchPacientes(q: string): Observable<any[]> {
    // El backend acota por ámbito; aquí se compone el nombre para el selector.
    return this.http.get<any[]>('/api/pacientes').pipe(
      map(ps => ps.map(p => ({
        ...p,
        nombre_completo: [p.nombre1, p.nombre2, p.apellido1, p.apellido2].filter(Boolean).join(' '),
        identificacion: p.numero_documento,
      }))),
    );
  }

  // ── Traducción documento ⇄ evolución ────────────────────────────
  private contenido(d: Partial<EvolucionCreate>): Record<string, unknown> {
    return {
      subjetivo: d.subjetivo ?? '',
      objetivo: d.objetivo ?? '',
      analisis: d.analisis ?? '',
      plan: d.plan ?? '',
      tension_arterial: d.tension_arterial ?? null,
      frecuencia_cardiaca: d.frecuencia_cardiaca ?? null,
      temperatura: d.temperatura ?? null,
      peso: d.peso ?? null,
    };
  }

  private deDocumento(d: any): Evolucion {
    const c = d.contenido ?? {};
    return {
      id: d.id, paciente_id: d.paciente_id, medico_id: d.medico_id, fecha: d.fecha,
      subjetivo: c.subjetivo ?? '', objetivo: c.objetivo ?? '',
      analisis: c.analisis ?? '', plan: c.plan ?? '',
      historia_relacionada_id: d.historia_id ?? undefined,
      tension_arterial: c.tension_arterial ?? undefined,
      frecuencia_cardiaca: c.frecuencia_cardiaca ?? undefined,
      temperatura: c.temperatura ?? undefined,
      peso: c.peso ?? undefined,
      firmada: d.firmada,
    };
  }

  private deVista(d: any): Evolucion {
    return {
      id: d.id, paciente_id: d.paciente_id, medico_id: d.medico_id, fecha: d.fecha,
      subjetivo: '', objetivo: '', analisis: '', plan: '',
      firmada: d.firmada,
      paciente_nombre: d.paciente_nombre,
      paciente_identificacion: d.paciente_identificacion,
      medico_nombre: d.medico_nombre,
    };
  }
}
