import { Injectable } from '@angular/core';
import { HttpClient, HttpParams } from '@angular/common/http';
import { Observable } from 'rxjs';
import { Patient, PatientCreate } from '../models/patient.models';
import { PaginatedResponse } from '../models/common.models';

@Injectable({ providedIn: 'root' })
export class PacienteService {
  private readonly API = '/api/pacientes';

  constructor(private http: HttpClient) {}

  list(page = 1, size = 20, q?: string): Observable<PaginatedResponse<Patient>> {
    let params = new HttpParams().set('page', page).set('size', size);
    if (q) params = params.set('q', q);
    return this.http.get<PaginatedResponse<Patient>>(this.API, { params });
  }

  buscar(q: string): Observable<Patient[]> {
    return this.http.get<Patient[]>(`${this.API}/buscar`, { params: { q } });
  }

  get(id: number): Observable<Patient> {
    return this.http.get<Patient>(`${this.API}/${id}`);
  }

  create(data: PatientCreate): Observable<Patient> {
    return this.http.post<Patient>(this.API, data);
  }

  update(id: number, data: Partial<PatientCreate>): Observable<Patient> {
    return this.http.put<Patient>(`${this.API}/${id}`, data);
  }

  getHistorias(id: number): Observable<unknown[]> {
    return this.http.get<unknown[]>(`${this.API}/${id}/historias`);
  }
}
