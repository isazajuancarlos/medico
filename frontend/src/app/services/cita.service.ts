import { Injectable, inject } from '@angular/core';
import { HttpClient, HttpParams } from '@angular/common/http';
import { Observable, map } from 'rxjs';
import { Appointment, AppointmentCreate, EstadoCita, SlotDisponible, MedicoDisponible, TipoCita, TipoCitaApi } from '../models/appointment.models';
import { PaginatedResponse } from '../models/common.models';

/** La cita como la devuelve el `medico-api` (Rust), con nombres resueltos. */
interface CitaRust {
  id: number;
  paciente_id: number;
  paciente_nombre: string;
  medico_id: number | null;
  medico_nombre: string | null;
  fecha: string;
  hora: string;
  estado: string;
  motivo: string | null;
  /** null = no se registró. NO es 'Primera Vez'. */
  tipo: TipoCitaApi | null;
  /** null = no se registró. NO son 30 minutos. */
  duracion_minutos: number | null;
  solicitada_en: string | null;
  institucion_id: number;
}

const A_ANGULAR: Record<string, EstadoCita> = {
  PROGRAMADA: 'Pendiente',
  ATENDIDA: 'Atendida',
  CANCELADA: 'Cancelada',
  NO_ASISTIO: 'No Asistió',
};

/** El tipo se traduce en las dos direcciones: la columna guarda el vocabulario
 *  de la base y la pantalla enseña el del usuario. Guardar el texto de la
 *  interfaz en la columna es lo que dejó muerta la pantalla de laboratorio. */
const TIPO_A_ANGULAR: Record<TipoCitaApi, TipoCita> = {
  PRIMERA_VEZ: 'Primera Vez',
  CONTROL: 'Control / Revisión',
  URGENCIA: 'Urgencia',
};

const TIPO_A_API: Record<TipoCita, TipoCitaApi> = {
  'Primera Vez': 'PRIMERA_VEZ',
  'Control / Revisión': 'CONTROL',
  Urgencia: 'URGENCIA',
};

/**
 * Citas y agenda contra el `medico-api`. Los dos subsistemas están portados
 * (#237): turnos, agendas mensuales, bloques, bloqueos e indicadores.
 */
@Injectable({ providedIn: 'root' })
export class CitaService {
  private readonly API = '/api/citas';
  private readonly AGENDA_API = '/api/agenda';
  private http = inject(HttpClient);

  private aAppointment(c: CitaRust): Appointment {
    return {
      id: c.id,
      paciente_id: c.paciente_id,
      medico_id: c.medico_id ?? 0,
      fecha: c.fecha,
      hora: c.hora,
      // Sin dato registrado se deja VACÍO. Antes iba 'Primera Vez' fijo, que en
      // la pantalla no se distinguía de algo que alguien hubiera escrito.
      tipo: c.tipo ? TIPO_A_ANGULAR[c.tipo] : undefined,
      estado: A_ANGULAR[c.estado] ?? 'Pendiente',
      motivo: c.motivo ?? undefined,
      duracion_minutos: c.duracion_minutos ?? undefined,
      solicitada_en: c.solicitada_en ?? undefined,
      paciente_nombre: c.paciente_nombre,
      medico_nombre: c.medico_nombre ?? undefined,
    };
  }

  list(page = 1, size = 20, fecha?: string, medicoId?: number, estado?: string): Observable<PaginatedResponse<Appointment>> {
    let params = new HttpParams();
    if (fecha) params = params.set('fecha', fecha);
    return this.http.get<CitaRust[]>(this.API, { params }).pipe(
      map((cs) => {
        let items = cs.map((c) => this.aAppointment(c));
        if (medicoId) items = items.filter((a) => a.medico_id === medicoId);
        if (estado) items = items.filter((a) => a.estado === estado);
        const total = items.length;
        const inicio = (page - 1) * size;
        return { total, items: items.slice(inicio, inicio + size), page, size };
      }),
    );
  }

  get(id: number): Observable<Appointment | undefined> {
    return this.http.get<CitaRust[]>(this.API).pipe(
      map((cs) => cs.map((c) => this.aAppointment(c)).find((a) => a.id === id)),
    );
  }

  create(data: AppointmentCreate): Observable<Appointment> {
    const cuerpo = {
      paciente_id: data.paciente_id,
      medico_id: data.medico_id,
      fecha: data.fecha,
      hora: data.hora,
      motivo: data.motivo ?? null,
      tipo: TIPO_A_API[data.tipo],
      duracion_minutos: data.duracion_minutos,
      // Si el mostrador no la captura, la cita simplemente no cuenta para el
      // indicador de oportunidad — y el indicador dice cuántas quedaron fuera.
      solicitada_en: data.solicitada_en ?? null,
    };
    return this.http.post<{ id: number }>(this.API, cuerpo).pipe(
      map((r) => ({
        ...data,
        id: r.id,
        estado: 'Pendiente' as EstadoCita,
      })),
    );
  }

  cancel(id: number): Observable<any> {
    return this.http.put(`${this.API}/${id}/estado`, { estado: 'CANCELADA' });
  }

  /** Marca la cita como atendida. (La apertura de la historia clínica llegará
   *  cuando ese módulo se porte; por ahora, atender = registrar el estado.) */
  marcarAtendida(id: number): Observable<any> {
    return this.http.put(`${this.API}/${id}/estado`, { estado: 'ATENDIDA' });
  }

  /** Los profesionales a los que se puede agendar. Sale de `/api/agenda/medicos`,
   *  que filtra por los roles que ATIENDEN — al laboratorista no se le agenda una
   *  cita, se le manda una orden. */
  getMedicos(): Observable<MedicoDisponible[]> {
    return this.http.get<MedicoDisponible[]>(`${this.AGENDA_API}/medicos`);
  }

  // --- Agenda (#237) ---------------------------------------------------------

  getSlots(medicoId: number, fecha: string): Observable<SlotDisponible[]> {
    const params = new HttpParams().set('medico_id', medicoId).set('fecha', fecha);
    return this.http.get<SlotDisponible[]>(`${this.AGENDA_API}/turnos`, { params });
  }

  getAgendas(medicoId?: number, anio?: number, mes?: number): Observable<any[]> {
    let params = new HttpParams();
    if (medicoId) params = params.set('medico_id', medicoId);
    if (anio) params = params.set('anio', anio);
    if (mes) params = params.set('mes', mes);
    return this.http.get<any[]>(this.AGENDA_API, { params });
  }

  getAgenda(id: number): Observable<any> {
    return this.http.get<any>(`${this.AGENDA_API}/${id}`);
  }

  createAgenda(data: any): Observable<any> {
    return this.http.post<any>(this.AGENDA_API, data);
  }

  updateAgenda(id: number, data: any): Observable<any> {
    return this.http.put<any>(`${this.AGENDA_API}/${id}`, data);
  }

  deleteAgenda(id: number): Observable<any> {
    return this.http.delete(`${this.AGENDA_API}/${id}`);
  }

  addBloque(agendaId: number, data: any): Observable<any> {
    return this.http.post<any>(`${this.AGENDA_API}/${agendaId}/bloques`, data);
  }

  deleteBloque(agendaId: number, bloqueId: number): Observable<any> {
    return this.http.delete(`${this.AGENDA_API}/${agendaId}/bloques/${bloqueId}`);
  }

  getBloqueos(medicoId?: number, desde?: string, hasta?: string): Observable<any[]> {
    let params = new HttpParams();
    if (medicoId) params = params.set('medico_id', medicoId);
    if (desde) params = params.set('desde', desde);
    if (hasta) params = params.set('hasta', hasta);
    return this.http.get<any[]>(`${this.AGENDA_API}/bloqueos`, { params });
  }

  createBloqueo(data: any): Observable<any> {
    return this.http.post<any>(`${this.AGENDA_API}/bloqueos`, data);
  }

  deleteBloqueo(id: number): Observable<any> {
    return this.http.delete(`${this.AGENDA_API}/bloqueos/${id}`);
  }

  getIndicadoresOportunidad(medicoId?: number, desde?: string, hasta?: string): Observable<any> {
    let params = new HttpParams();
    if (medicoId) params = params.set('medico_id', medicoId);
    if (desde) params = params.set('desde', desde);
    if (hasta) params = params.set('hasta', hasta);
    return this.http.get<any>(`${this.AGENDA_API}/indicadores`, { params });
  }
}
