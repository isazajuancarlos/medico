import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable, map, of, switchMap } from 'rxjs';
import { Patient, PatientCreate, TipoUsuario, ZonaResidencia } from '../models/patient.models';
import { PaginatedResponse } from '../models/common.models';

/** La forma que devuelve el `medico-api` (Rust). Nombres del dominio médico
 *  colombiano; el adaptador los traduce al modelo del frontend. */
interface PacienteRust {
  id: number;
  tipo_documento: string;
  numero_documento: string;
  nombre1: string;
  nombre2: string | null;
  apellido1: string;
  apellido2: string | null;
  fecha_nacimiento: string;
  sexo: string;
  cod_municipio: string | null;
  celular: string | null;
  email: string | null;
  direccion: string | null;
  tipo_usuario: string | null;
  cod_pais: string | null;
  zona: string | null;
  entidad_id: number | null;
  institucion_id: number;
}

/**
 * Acceso a pacientes contra el `medico-api`. Este servicio es un ADAPTADOR: el
 * componente y el formulario siguen hablando en el modelo `Patient`
 * (`identificacion`, `tipo_id`…) mientras el Rust habla en el suyo
 * (`numero_documento`, `tipo_documento`…). La traducción vive aquí, en un solo
 * sitio, para no repuntar cada pantalla.
 *
 * El Rust devuelve la lista completa acotada por institución; la paginación y la
 * búsqueda se hacen en el cliente (una lista por IPS es acotada). La paginación
 * de servidor llegará cuando algún prestador la necesite.
 */
@Injectable({ providedIn: 'root' })
export class PacienteService {
  private readonly API = '/api/pacientes';
  private http = inject(HttpClient);

  private aPatient(r: PacienteRust): Patient {
    return {
      id: r.id,
      identificacion: r.numero_documento,
      tipo_id: r.tipo_documento,
      nombre1: r.nombre1,
      nombre2: r.nombre2 ?? undefined,
      apellido1: r.apellido1,
      apellido2: r.apellido2 ?? undefined,
      fecha_nacimiento: r.fecha_nacimiento,
      sexo: r.sexo,
      // Blanco = «no capturado», no un valor por defecto inventado.
      celular: r.celular ?? '',
      email: r.email ?? undefined,
      direccion: r.direccion ?? undefined,
      tipo_usuario: (r.tipo_usuario ?? '') as TipoUsuario,
      cod_municipio: r.cod_municipio ?? '',
      cod_pais: r.cod_pais ?? '',
      zona: (r.zona ?? '') as ZonaResidencia,
      entidad_id: r.entidad_id ?? undefined,
      institucion_id: r.institucion_id,
    };
  }

  private aRust(c: Partial<PatientCreate>): Record<string, unknown> {
    return {
      tipo_documento: c.tipo_id,
      numero_documento: c.identificacion,
      nombre1: c.nombre1,
      nombre2: c.nombre2 ?? null,
      apellido1: c.apellido1,
      apellido2: c.apellido2 ?? null,
      fecha_nacimiento: c.fecha_nacimiento,
      sexo: c.sexo,
      cod_municipio: c.cod_municipio ?? null,
      celular: c.celular ?? null,
      email: c.email ?? null,
      direccion: c.direccion ?? null,
      tipo_usuario: c.tipo_usuario ?? null,
      cod_pais: c.cod_pais ?? null,
      zona: c.zona ?? null,
      entidad_id: c.entidad_id ?? null,
    };
  }

  private coincide(p: Patient, t: string): boolean {
    const texto = `${p.nombre1} ${p.nombre2 ?? ''} ${p.apellido1} ${p.apellido2 ?? ''} ${p.identificacion}`;
    return texto.toLowerCase().includes(t.toLowerCase());
  }

  list(page = 1, size = 20, q?: string): Observable<PaginatedResponse<Patient>> {
    return this.http.get<PacienteRust[]>(this.API).pipe(
      map((rs) => {
        let items = rs.map((r) => this.aPatient(r));
        if (q) items = items.filter((p) => this.coincide(p, q));
        const total = items.length;
        const inicio = (page - 1) * size;
        return { total, items: items.slice(inicio, inicio + size), page, size };
      }),
    );
  }

  buscar(q: string): Observable<Patient[]> {
    return this.http.get<PacienteRust[]>(this.API).pipe(
      map((rs) => rs.map((r) => this.aPatient(r)).filter((p) => this.coincide(p, q))),
    );
  }

  get(id: number): Observable<Patient> {
    return this.http.get<PacienteRust>(`${this.API}/${id}`).pipe(map((r) => this.aPatient(r)));
  }

  create(data: PatientCreate): Observable<Patient> {
    return this.http
      .post<{ id: number }>(this.API, this.aRust(data))
      .pipe(switchMap((r) => this.get(r.id)));
  }

  update(id: number, data: Partial<PatientCreate>): Observable<Patient> {
    return this.http.put(`${this.API}/${id}`, this.aRust(data)).pipe(switchMap(() => this.get(id)));
  }

  /** Las historias del paciente aún no están portadas al medico-api; la pantalla
   *  que las usa sigue en el backend anterior. Devuelve vacío para no romper. */
  getHistorias(_id: number): Observable<unknown[]> {
    return of([]);
  }
}
