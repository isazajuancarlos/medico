import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';
import { map } from 'rxjs/operators';

// Urgencias tiene backend Rust propio (no la espina de documentos): la cola de
// triage, el registro y el egreso. El triage viaja como CLAVE (TRIAGE_1..5) para
// que el orden de prioridad lo fije el servidor; la etiqueta la trae aparte.

export interface EntradaCola {
  id: number;
  paciente_id: number;
  triage: string;        // clave TRIAGE_1..5
  triage_label: string;  // etiqueta humana
  motivo_consulta: string;
  fecha_ingreso: string;
  estado: string;
}

export interface TriageNuevo {
  paciente_id: number;
  triage: string;
  motivo_consulta: string;
  signos_vitales?: Record<string, unknown>;
}

/** Niveles de triage, de más a menos urgente (el orden es la prioridad). */
export const NIVELES_TRIAGE = [
  { value: 'TRIAGE_1', label: 'Rojo - Resucitación' },
  { value: 'TRIAGE_2', label: 'Naranja - Emergencia' },
  { value: 'TRIAGE_3', label: 'Amarillo - Urgente' },
  { value: 'TRIAGE_4', label: 'Verde - Menos Urgente' },
  { value: 'TRIAGE_5', label: 'Azul - No Urgente' },
];

/** Destinos de salida válidos (deben coincidir con el CHECK del backend). */
export const DESTINOS_SALIDA = [
  { value: 'DOMICILIO', label: 'Domicilio' },
  { value: 'HOSPITALIZACION', label: 'Hospitalización' },
  { value: 'REMISION', label: 'Remisión' },
  { value: 'OBSERVACION', label: 'Observación' },
  { value: 'FUGA', label: 'Fuga' },
  { value: 'FALLECIDO', label: 'Fallecido' },
];

@Injectable({ providedIn: 'root' })
export class UrgenciasService {
  private http = inject(HttpClient);
  private readonly API = '/api/urgencias';

  cola(): Observable<EntradaCola[]> {
    return this.http.get<EntradaCola[]>(`${this.API}/cola`);
  }

  registrar(t: TriageNuevo): Observable<{ id: number }> {
    return this.http.post<{ id: number }>(`${this.API}/triage`, t);
  }

  egreso(id: number, destino: string, observaciones?: string): Observable<void> {
    return this.http.put<void>(`${this.API}/${id}/egreso`, { destino, observaciones });
  }

  searchPacientes(): Observable<any[]> {
    return this.http.get<any[]>('/api/pacientes').pipe(
      map(ps => ps.map(p => ({
        ...p,
        nombre_completo: [p.nombre1, p.nombre2, p.apellido1, p.apellido2].filter(Boolean).join(' '),
        identificacion: p.numero_documento,
      }))),
    );
  }
}
