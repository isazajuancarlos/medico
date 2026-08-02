import { Injectable, inject, signal, computed } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Router } from '@angular/router';
import { Observable, map, switchMap, tap, catchError, throwError } from 'rxjs';
import { CurrentUser, YoResponse, LoginRequest, RolUsuario } from '../models/auth.models';
import { ModulosService } from './modulos.service';

/**
 * Sesión contra el `medico-api` (Rust): cookie firmada `HttpOnly`, no un token
 * en `localStorage`. El login no devuelve token; la identidad se lee de
 * `GET /api/yo`, y la cookie —que el JavaScript no puede leer— es la verdad.
 * En `localStorage` solo queda una copia de la identidad para pintar al instante;
 * si la cookie no vale, el servidor responde 401 y se limpia.
 */
@Injectable({ providedIn: 'root' })
export class AuthService {
  private _currentUser = signal<CurrentUser | null>(null);
  readonly currentUser = this._currentUser.asReadonly();
  readonly isLoggedIn = computed(() => this._currentUser() !== null);

  private modulos = inject(ModulosService);

  constructor(private http: HttpClient, private router: Router) {
    const cache = localStorage.getItem('current_user');
    if (cache) this._currentUser.set(JSON.parse(cache));
    // La cookie es la verdad: se valida contra el servidor al arrancar.
    this.http.get<YoResponse>('/api/yo', { withCredentials: true }).subscribe({
      next: (yo) => {
        this.aplicar(yo);
        this.cargarModulos();
      },
      error: () => this.cerrarLocal(),
    });
  }

  private aplicar(yo: YoResponse): CurrentUser {
    const user: CurrentUser = {
      id: yo.id,
      username: yo.email,
      nombre_completo: yo.nombre,
      rol: yo.rol,
      institucion_id: yo.institucion_id,
      email: yo.email,
    };
    this._currentUser.set(user);
    localStorage.setItem('current_user', JSON.stringify(user));
    return user;
  }

  private cargarModulos(): void {
    this.modulos.cargar().subscribe({
      // Un fallo aquí NO deja al usuario sin menú: "sin catálogo" es "todo
      // visible", y el backend sigue respondiendo 403 en lo que no corresponda.
      error: () => console.warn('No se pudo cargar el catálogo de módulos'),
    });
  }

  login(credentials: LoginRequest): Observable<CurrentUser> {
    return this.http.post('/api/acceso', credentials, { withCredentials: true }).pipe(
      switchMap(() => this.http.get<YoResponse>('/api/yo', { withCredentials: true })),
      map((yo) => this.aplicar(yo)),
      tap(() => this.cargarModulos()),
    );
  }

  logout(): Observable<unknown> {
    return this.http.post('/api/salir', {}, { withCredentials: true }).pipe(
      tap(() => this.cerrarLocal()),
      catchError(() => {
        this.cerrarLocal();
        return throwError(() => new Error('Logout error'));
      }),
    );
  }

  /**
   * Cierra la sesión en TODOS los dispositivos (M3, tarea #194).
   *
   * El servidor sube el `token_version` del usuario autenticado, con lo que toda
   * cookie ya emitida —incluida la de este navegador— deja de valer en la
   * siguiente petición. Actúa sobre quien está autenticado, nunca sobre un id
   * que venga de aquí.
   *
   * **Si falla, NO se limpia la sesión local**, al revés que `logout()`. Esa
   * diferencia es el punto entero de este método: en un cierre normal, borrar la
   * cookie de este navegador ya cumple lo que se prometió aunque el servidor no
   * conteste. Aquí lo prometido es cerrar en los OTROS dispositivos, y eso solo
   * lo puede hacer el servidor. Limpiar aquí y mandar al login pintaría de
   * «hecho» un cierre que no ocurrió — y quien lo pide suele estar pidiéndolo
   * porque cree que alguien más tiene su sesión.
   *
   * El 401 es el único caso que sí limpia, y lo hace el interceptor: si la
   * cookie ya no vale, la sesión de este navegador está cerrada de todos modos.
   */
  logoutTodos(): Observable<unknown> {
    return this.http.post('/api/salir-todo', {}, { withCredentials: true }).pipe(
      tap(() => this.cerrarLocal()),
    );
  }

  /** Limpia la sesión en el navegador, sin llamar al servidor. La usa el
   *  interceptor cuando el servidor responde 401. */
  cerrarLocal(): void {
    this.modulos.limpiar();
    this._currentUser.set(null);
    localStorage.removeItem('current_user');
  }

  hasRole(...roles: RolUsuario[]): boolean {
    const user = this._currentUser();
    return user !== null && roles.includes(user.rol);
  }

  getDefaultRoute(): string {
    const user = this._currentUser();
    if (!user) return '/login';
    const routes: Record<RolUsuario, string> = {
      superadmin: '/superadmin',
      admin: '/citas',
      medico: '/citas',
      enfermeria: '/urgencias',
      anestesiologo: '/preanestesia',
      laboratorista: '/laboratorio',
      bacteriologo: '/laboratorio',
      radiologo: '/imagenes',
      terapeuta: '/rehabilitacion',
      nutricionista: '/nutricion',
      trabajo_social: '/trabajo-social',
      psicologo: '/pacientes',
      auxiliar_enfermeria: '/pacientes',
      regente_farmacia: '/farmacia',
      paciente: '/portal-citas',
    };
    return routes[user.rol] || '/dashboard';
  }
}
