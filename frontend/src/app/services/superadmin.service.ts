import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

export interface Institucion {
  id: number;
  nombre: string;
  nit: string;
  activa: boolean;
}

export interface UsuarioResumen {
  id: number;
  nombre: string;
  email: string;
  rol: string;
  institucion_id: number | null;
  activo: boolean;
}

export interface AdminNuevo {
  nombre: string;
  email: string;
  clave: string;
  rol: string;
  institucion_id: number | null;
}

/**
 * El plano de superadministración contra el `medico-api`: dar de alta IPS, fijar
 * su plan (licencia) y crear su primer administrador. Es el onboarding de un
 * cliente del sistema; solo el superadmin llega aquí (lo protege el roleGuard).
 */
@Injectable({ providedIn: 'root' })
export class SuperadminService {
  private http = inject(HttpClient);

  listInstituciones(): Observable<Institucion[]> {
    return this.http.get<Institucion[]>('/api/instituciones');
  }

  crearInstitucion(nombre: string, nit: string): Observable<{ id: number }> {
    return this.http.post<{ id: number }>('/api/instituciones', { nombre, nit });
  }

  fijarLicencia(id: number, max_usuarios: number, vence: string | null): Observable<unknown> {
    return this.http.put(`/api/instituciones/${id}/licencia`, { max_usuarios, vence });
  }

  listUsuarios(): Observable<UsuarioResumen[]> {
    return this.http.get<UsuarioResumen[]>('/api/usuarios');
  }

  crearUsuario(u: AdminNuevo): Observable<{ id: number }> {
    return this.http.post<{ id: number }>('/api/usuarios', u);
  }
}
