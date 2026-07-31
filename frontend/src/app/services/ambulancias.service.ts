import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

/**
 * Transporte asistencial: el parque de ambulancias y sus traslados.
 *
 * Las dos modalidades son las que habilita un prestador colombiano — TAB
 * (básico) y TAM (medicalizado) — y no hay una tercera. Un traslado es PRIMARIO
 * cuando lleva al paciente desde donde ocurrió el evento hasta el prestador, y
 * SECUNDARIO cuando va de un prestador a otro; el segundo es el que se engancha
 * con una remisión.
 */
export type TipoAmbulancia = 'TAB' | 'TAM';
export type EstadoAmbulancia = 'Disponible' | 'En Traslado' | 'En Mantenimiento' | 'Fuera de Servicio';
export type TipoTraslado = 'Primario' | 'Secundario';
export type EstadoTraslado = 'En Curso' | 'Finalizado' | 'Cancelado';

export interface Ambulancia {
  id: number;
  placa: string;
  tipo: TipoAmbulancia;
  numero_interno?: string;
  marca?: string;
  modelo?: string;
  estado: EstadoAmbulancia;
  vence_soat?: string;
  vence_tecnomecanica?: string;
  observaciones?: string;
  activa: boolean;
  /** Venció: el servidor NO deja despacharla. */
  soat_vencido: boolean;
  /** Vence dentro del mes. Avisa; no impide nada. */
  soat_por_vencer: boolean;
  /**
   * No está registrado. **No significa que esté al día**: significa que no se
   * sabe. No bloquea el despacho —un campo sin llenar no puede volverse una
   * demora asistencial— pero se muestra.
   */
  soat_sin_registrar: boolean;
  tecnomecanica_vencida: boolean;
  /** El traslado que la tiene ocupada, si lo hay. */
  traslado_en_curso?: number;
}

export type AmbulanciaCreate = Omit<
  Ambulancia,
  'id' | 'estado' | 'activa' | 'soat_vencido' | 'soat_por_vencer' | 'soat_sin_registrar'
  | 'tecnomecanica_vencida' | 'traslado_en_curso'
>;
export type AmbulanciaUpdate = Partial<AmbulanciaCreate> & {
  estado?: Exclude<EstadoAmbulancia, 'En Traslado'>;
  activa?: boolean;
};

export interface Traslado {
  id: number;
  ambulancia_id: number;
  ambulancia_placa?: string;
  ambulancia_tipo?: TipoAmbulancia;
  paciente_id: number;
  paciente_nombre?: string;
  paciente_identificacion?: string;
  remision_id?: number;
  tipo_traslado: TipoTraslado;
  origen: string;
  destino: string;
  motivo: string;
  estado: EstadoTraslado;
  fecha_salida: string;
  /** Nula mientras está en curso, y **también** en los cancelados: un traslado
   *  cancelado no llegó a ninguna parte. */
  fecha_llegada?: string;
  conductor?: string;
  personal_asistencial?: string;
  registrado_por_id: number;
  registrado_por_nombre?: string;
  observaciones?: string;
}

export interface TrasladoCreate {
  ambulancia_id: number;
  paciente_id: number;
  remision_id?: number;
  tipo_traslado?: TipoTraslado;
  origen: string;
  destino: string;
  motivo: string;
  conductor?: string;
  personal_asistencial?: string;
  observaciones?: string;
}

export interface Cierre {
  fecha_llegada?: string;
  observaciones?: string;
}

@Injectable({ providedIn: 'root' })
export class AmbulanciasService {
  private readonly API = '/api/ambulancias';
  private http = inject(HttpClient);

  listar(q?: string, estado?: string, soloActivas = false): Observable<Ambulancia[]> {
    const params: Record<string, string> = {};
    if (q) params['q'] = q;
    if (estado) params['estado'] = estado;
    if (soloActivas) params['activa'] = 'true';
    return this.http.get<Ambulancia[]>(this.API, { params });
  }

  crear(data: AmbulanciaCreate): Observable<Ambulancia> {
    return this.http.post<Ambulancia>(this.API, data);
  }

  actualizar(id: number, data: AmbulanciaUpdate): Observable<Ambulancia> {
    return this.http.put<Ambulancia>(`${this.API}/${id}`, data);
  }

  eliminar(id: number): Observable<void> {
    return this.http.delete<void>(`${this.API}/${id}`);
  }

  listarTraslados(estado?: string, ambulanciaId?: number): Observable<Traslado[]> {
    const params: Record<string, string> = {};
    if (estado) params['estado'] = estado;
    if (ambulanciaId) params['ambulancia_id'] = String(ambulanciaId);
    return this.http.get<Traslado[]>(`${this.API}/traslados`, { params });
  }

  despachar(data: TrasladoCreate): Observable<Traslado> {
    return this.http.post<Traslado>(`${this.API}/traslados`, data);
  }

  finalizar(id: number, c: Cierre = {}): Observable<Traslado> {
    return this.http.post<Traslado>(`${this.API}/traslados/${id}/finalizar`, c);
  }

  cancelar(id: number, c: Cierre = {}): Observable<Traslado> {
    return this.http.post<Traslado>(`${this.API}/traslados/${id}/cancelar`, c);
  }
}
