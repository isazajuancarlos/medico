import { Injectable } from '@angular/core';
import { HttpClient, HttpParams } from '@angular/common/http';
import { Observable } from 'rxjs';
import { Appointment, AppointmentCreate, SlotDisponible, MedicoDisponible } from '../models/appointment.models';
import { PaginatedResponse } from '../models/common.models';

@Injectable({ providedIn: 'root' })
export class CitaService {
  private readonly API = '/api/citas';
  private readonly AGENDA_API = '/api/agenda';

  constructor(private http: HttpClient) {}

  list(page = 1, size = 20, fecha?: string, medicoId?: number, estado?: string): Observable<PaginatedResponse<Appointment>> {
    let params = new HttpParams().set('page', page).set('size', size);
    if (fecha) params = params.set('fecha', fecha);
    if (medicoId) params = params.set('medico_id', medicoId);
    if (estado) params = params.set('estado', estado);
    return this.http.get<PaginatedResponse<Appointment>>(this.API, { params });
  }

  get(id: number): Observable<Appointment> {
    return this.http.get<Appointment>(`${this.API}/${id}`);
  }

  create(data: AppointmentCreate): Observable<Appointment> {
    return this.http.post<Appointment>(this.API, data);
  }

  update(id: number, data: Partial<Appointment>): Observable<Appointment> {
    return this.http.put<Appointment>(`${this.API}/${id}`, data);
  }

  cancel(id: number): Observable<unknown> {
    return this.http.delete(`${this.API}/${id}`);
  }

  atender(id: number): Observable<{ historia_id: number; paciente_id: number }> {
    return this.http.post<{ historia_id: number; paciente_id: number }>(`${this.API}/${id}/atender`, {});
  }

  getSlots(medicoId: number, fecha: string): Observable<SlotDisponible[]> {
    const params = new HttpParams().set('medico_id', medicoId).set('fecha', fecha);
    return this.http.get<SlotDisponible[]>(`${this.AGENDA_API}/slots`, { params });
  }

  getMedicos(): Observable<MedicoDisponible[]> {
    return this.http.get<MedicoDisponible[]>(`${this.AGENDA_API}/medicos`);
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
