import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

export interface OrdenImagen {
  id: number;
  historia_id: number;
  paciente_id?: number;
  medico_ordena_id?: number;
  fecha_orden: string;
  estado: string;
  tipo: string;
  codigo_cups?: string;
  descripcion: string;
  justificacion: string;
  paciente_nombre?: string;
  paciente_identificacion?: string;
  medico_nombre?: string;
  tiene_resultado: boolean;
}

export interface InformeImagen {
  id: number;
  orden_id: number;
  paciente_id?: number;
  radiologo_id: number;
  radiologo_nombre?: string;
  fecha_resultado: string;
  tecnica: string;
  hallazgos: string;
  impresion_diagnostica: string;
  recomendaciones?: string;
  clasificacion?: string;
  /** Rutas que sirven las imágenes adjuntas, ya listas para un `src`. */
  imagenes_paths: string[];
  firmado: boolean;
  fecha_firma?: string;
  /** Quién firmó. Firmado es de solo lectura: para corregir se emite otro. */
  firmado_por_id?: number;
}

export interface InformeCreate {
  tecnica: string;
  hallazgos: string;
  impresion_diagnostica: string;
  recomendaciones?: string;
  clasificacion?: string;
}

@Injectable({ providedIn: 'root' })
export class ImagenesService {
  private readonly API = '/api/imagenes';

  constructor(private http: HttpClient) {}

  listOrdenes(estado?: string, pacienteId?: number): Observable<OrdenImagen[]> {
    const params: Record<string, string> = {};
    if (estado) params['estado'] = estado;
    if (pacienteId) params['paciente_id'] = String(pacienteId);
    return this.http.get<OrdenImagen[]>(`${this.API}/ordenes`, { params });
  }

  getOrden(ordenId: number): Observable<OrdenImagen> {
    return this.http.get<OrdenImagen>(`${this.API}/ordenes/${ordenId}`);
  }

  updateEstado(ordenId: number, estado: string): Observable<OrdenImagen> {
    return this.http.put<OrdenImagen>(`${this.API}/ordenes/${ordenId}/estado`, { estado });
  }

  getInforme(ordenId: number): Observable<InformeImagen> {
    return this.http.get<InformeImagen>(`${this.API}/ordenes/${ordenId}/informe`);
  }

  createInforme(ordenId: number, data: InformeCreate): Observable<InformeImagen> {
    return this.http.post<InformeImagen>(`${this.API}/ordenes/${ordenId}/informe`, data);
  }

  updateInforme(ordenId: number, data: Partial<InformeCreate>): Observable<InformeImagen> {
    return this.http.put<InformeImagen>(`${this.API}/ordenes/${ordenId}/informe`, data);
  }

  firmarInforme(ordenId: number): Observable<InformeImagen> {
    return this.http.post<InformeImagen>(`${this.API}/ordenes/${ordenId}/informe/firmar`, {});
  }

  uploadImagen(ordenId: number, file: File): Observable<InformeImagen> {
    const formData = new FormData();
    formData.append('file', file);
    return this.http.post<InformeImagen>(`${this.API}/ordenes/${ordenId}/informe/imagenes`, formData);
  }

  getInformesPaciente(pacienteId: number): Observable<InformeImagen[]> {
    return this.http.get<InformeImagen[]>(`${this.API}/paciente/${pacienteId}/informes`);
  }
}
