import { Injectable, inject } from '@angular/core';
import { HttpClient, HttpParams } from '@angular/common/http';
import { Observable, map, of } from 'rxjs';
import { Appointment, AppointmentCreate, EstadoCita, SlotDisponible, MedicoDisponible } from '../models/appointment.models';
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
  institucion_id: number;
}

const A_ANGULAR: Record<string, EstadoCita> = {
  PROGRAMADA: 'Pendiente',
  ATENDIDA: 'Atendida',
  CANCELADA: 'Cancelada',
  NO_ASISTIO: 'No Asistió',
};

/**
 * Citas contra el `medico-api`. Las CITAS ya están portadas (listar, agendar con
 * alta manual, cambiar estado). La AGENDA (slots, bloques, bloqueos, indicadores)
 * es un subsistema aún no portado: esos métodos siguen apuntando al backend
 * anterior y la página de agenda seguirá contra él hasta que se porte.
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
      // El Rust no guarda tipo ni duración todavía; son valores de presentación.
      tipo: 'Primera Vez',
      estado: A_ANGULAR[c.estado] ?? 'Pendiente',
      motivo: c.motivo ?? undefined,
      duracion_minutos: 30,
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

  getMedicos(): Observable<MedicoDisponible[]> {
    return this.http.get<{ id: number; nombre: string }[]>('/api/medicos').pipe(
      map((ms) => ms.map((m) => ({ id: m.id, nombre: m.nombre, rol: 'medico' }))),
    );
  }

  // --- Agenda: subsistema aún no portado; sigue contra el backend anterior. ---

  getSlots(_medicoId: number, _fecha: string): Observable<SlotDisponible[]> {
    // El alta de citas es manual mientras la agenda no esté portada.
    return of([]);
  }

  getAgendas(medicoId?: number, año?: number, mes?: number): Observable<any[]> {
    let params = new HttpParams();
    if (medicoId) params = params.set('medico_id', medicoId);
    if (año) params = params.set('año', año);
    if (mes) params = params.set('mes', mes);
    return this.http.get<any[]>(`${this.AGENDA_API}/`, { params });
  }

  createAgenda(data: any): Observable<any> {
    return this.http.post<any>(`${this.AGENDA_API}/`, data);
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
    return this.http.get<any[]>(`${this.AGENDA_API}/bloqueos/lista`, { params });
  }

  createBloqueo(data: any): Observable<any> {
    return this.http.post<any>(`${this.AGENDA_API}/bloqueos/crear`, data);
  }

  deleteBloqueo(id: number): Observable<any> {
    return this.http.delete(`${this.AGENDA_API}/bloqueos/${id}`);
  }

  getIndicadoresOportunidad(medicoId?: number, desde?: string, hasta?: string): Observable<any> {
    let params = new HttpParams();
    if (medicoId) params = params.set('medico_id', medicoId);
    if (desde) params = params.set('desde', desde);
    if (hasta) params = params.set('hasta', hasta);
    return this.http.get<any>(`${this.AGENDA_API}/indicadores/oportunidad`, { params });
  }
}
