// SPDX-License-Identifier: LicenseRef-Propietario
import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

/**
 * La planta del prestador, contra las rutas que el binario SÍ sirve.
 *
 * No se cuelga de `admin.service.ts` a propósito: aquel habla con `/api/admin`,
 * que hoy es Python y da 404 contra el binario, y su `UsuarioAdmin` pide
 * `username`, `especialidad_id`, `registro_medico`, `registro_rethus` y
 * `cuenta_bloqueada` — campos que la tabla `usuario` de Rust no tiene (la
 * especialidad y el registro llegan con el maestro de talento humano; el
 * bloqueo no existe porque en su lugar hay un FRENO que se agota solo).
 * Mezclarlos habría hecho que esta pantalla dependiera de un port que no está.
 */
export interface Usuario {
  id: number;
  nombre: string;
  email: string;
  rol: string;
  institucion_id: number | null;
  activo: boolean;
  /** P11/P12. `null` es la respuesta que hay que poder ver de un vistazo. */
  tipo_documento: string | null;
  numero_documento: string | null;
}

export interface UsuarioNuevo {
  nombre: string;
  email: string;
  clave: string;
  rol: string;
  /** P11/P12. Los dos o ninguno; lo valida el servidor contra el maestro. */
  tipo_documento?: string | null;
  numero_documento?: string | null;
}

@Injectable({ providedIn: 'root' })
export class UsuariosService {
  private http = inject(HttpClient);

  listar(): Observable<Usuario[]> {
    return this.http.get<Usuario[]>('/api/usuarios');
  }

  crear(u: UsuarioNuevo): Observable<{ id: number }> {
    return this.http.post<{ id: number }>('/api/usuarios', u);
  }

  /**
   * Pone o corrige el documento de identidad del profesional.
   *
   * Enviar `null` en los dos campos lo BORRA, y es una operación legítima:
   * corregir un dedazo dejando la fila sin documento es mejor que dejarla con
   * uno falso — el RIPS anota la falta y el cierre del mes la enseña, que es
   * justo lo que se quiere. `204` sin cuerpo.
   */
  fijarDocumento(
    id: number,
    tipo_documento: string | null,
    numero_documento: string | null,
  ): Observable<void> {
    return this.http.put<void>(`/api/usuarios/${id}/documento`, {
      tipo_documento,
      numero_documento,
    });
  }

  /** Los ocho tipos que la 948 admite para quien REALIZA la atención. */
  tiposDocumento(): Observable<{ codigo: string; nombre: string }[]> {
    return this.http.get<{ codigo: string; nombre: string }[]>('/api/tipos-documento-profesional');
  }
}
