import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';
import { map } from 'rxjs/operators';

// La nota operatoria es un DOCUMENTO CLÍNICO de tipo NOTA_OPERATORIA: sus campos
// propios viven en el `contenido` JSON de la espina genérica /api/documentos.
// Este servicio traduce la forma plana que espera la pantalla a ese documento,
// igual que epicrisis e interconsulta. El TIEMPO quirúrgico NO viaja: se deriva
// de las horas en el cliente (ver nota-operatoria.dominio), para no guardar tres
// campos para dos datos.

export interface NotaOperatoria {
  id: number;
  paciente_id: number;
  cirujano_id: number;
  fecha: string;
  procedimiento_realizado: string;
  diagnostico_preoperatorio: string;
  diagnostico_postoperatorio: string;
  hallazgos?: string;
  hora_inicio: string;
  hora_fin: string;
  sangrado_estimado_ml: number | null;
  complicaciones?: string;
  estado_paciente_salida: string;
  firmada: boolean;
  paciente_nombre?: string;
  paciente_identificacion?: string;
  cirujano_nombre?: string;
}

export interface NotaOperatoriaCreate {
  paciente_id: number;
  procedimiento_realizado: string;
  diagnostico_preoperatorio: string;
  diagnostico_postoperatorio: string;
  hallazgos?: string;
  hora_inicio: string;
  hora_fin: string;
  sangrado_estimado_ml: number | null;
  complicaciones?: string;
  estado_paciente_salida: string;
}

const TIPO = 'NOTA_OPERATORIA';

@Injectable({ providedIn: 'root' })
export class NotaOperatoriaService {
  private http = inject(HttpClient);
  private readonly API = '/api/documentos';

  list(pacienteId?: number): Observable<NotaOperatoria[]> {
    const params: Record<string, string> = { tipo: TIPO };
    if (pacienteId) params['paciente_id'] = String(pacienteId);
    return this.http.get<any[]>(this.API, { params }).pipe(map(ds => ds.map(d => this.aNota(d))));
  }

  get(id: number): Observable<NotaOperatoria> {
    return this.http.get<any>(`${this.API}/${id}`).pipe(map(d => this.aNota(d)));
  }

  create(data: NotaOperatoriaCreate): Observable<{ id: number }> {
    return this.http.post<{ id: number }>(this.API, this.aDocumento(data));
  }

  update(id: number, data: Partial<NotaOperatoriaCreate>): Observable<NotaOperatoria> {
    return this.http.put<any>(`${this.API}/${id}`, this.aDocumento(data)).pipe(map(d => this.aNota(d)));
  }

  delete(id: number): Observable<void> {
    return this.http.delete<void>(`${this.API}/${id}`);
  }

  firmar(id: number): Observable<void> {
    return this.http.post<void>(`${this.API}/${id}/firmar`, {});
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

  // ── Traducción documento ⇄ nota operatoria ──────────────────────
  private aDocumento(data: Partial<NotaOperatoriaCreate>) {
    const { paciente_id, ...contenido } = data;
    return { tipo: TIPO, paciente_id, historia_id: null, titulo: '', contenido };
  }

  private aNota(d: any): NotaOperatoria {
    return {
      ...(d.contenido ?? {}),
      id: d.id,
      paciente_id: d.paciente_id,
      cirujano_id: d.medico_id,
      fecha: d.fecha,
      firmada: !!d.firmada,
      paciente_nombre: d.paciente_nombre,
      paciente_identificacion: d.paciente_identificacion,
      cirujano_nombre: d.medico_nombre,
    } as NotaOperatoria;
  }
}
