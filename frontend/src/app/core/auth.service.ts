import { Injectable, inject, signal, computed } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Router } from '@angular/router';
import { Observable, tap, catchError, throwError } from 'rxjs';
import { CurrentUser, TokenResponse, LoginRequest, RolUsuario } from '../models/auth.models';
import { ModulosService } from './modulos.service';

@Injectable({ providedIn: 'root' })
export class AuthService {
  private readonly API = '/api/auth';
  private _currentUser = signal<CurrentUser | null>(null);
  private _accessToken = signal<string | null>(null);

  readonly currentUser = this._currentUser.asReadonly();
  readonly isLoggedIn = computed(() => this._currentUser() !== null);

  // Los módulos contratados definen qué ve el usuario. Se cargan junto con la
  // sesión: si se cargaran al pintar el menú, el primer render mostraría
  // opciones que la institución no tiene.
  private modulos = inject(ModulosService);

  constructor(private http: HttpClient, private router: Router) {
    this.loadFromStorage();
    if (this._currentUser()) {
      this.cargarModulos();
    }
  }

  private cargarModulos(): void {
    this.modulos.cargar().subscribe({
      // Un fallo aquí NO debe dejar al usuario sin menú: el servicio trata
      // "sin catálogo" como "todo visible", y el backend sigue respondiendo
      // 403 en lo que no corresponda. Se pierde el filtro, no la seguridad.
      error: () => console.warn('No se pudo cargar el catálogo de módulos'),
    });
  }

  private loadFromStorage(): void {
    const token = localStorage.getItem('access_token');
    const user = localStorage.getItem('current_user');
    if (token && user) {
      this._accessToken.set(token);
      this._currentUser.set(JSON.parse(user));
    }
  }

  getToken(): string | null {
    return this._accessToken();
  }

  login(credentials: LoginRequest): Observable<TokenResponse> {
    return this.http.post<TokenResponse>(`${this.API}/login`, credentials, { withCredentials: true }).pipe(
      tap(response => {
        this._accessToken.set(response.access_token);
        this._currentUser.set(response.user);
        localStorage.setItem('access_token', response.access_token);
        localStorage.setItem('current_user', JSON.stringify(response.user));
        this.cargarModulos();
      })
    );
  }

  logout(): Observable<unknown> {
    return this.http.post(`${this.API}/logout`, {}, { withCredentials: true }).pipe(
      tap(() => this.clearSession()),
      catchError(() => {
        this.clearSession();
        return throwError(() => new Error('Logout error'));
      })
    );
  }

  refreshToken(): Observable<{ access_token: string }> {
    return this.http.post<{ access_token: string }>(`${this.API}/refresh`, {}, { withCredentials: true }).pipe(
      tap(response => {
        this._accessToken.set(response.access_token);
        localStorage.setItem('access_token', response.access_token);
      }),
      catchError(err => {
        this.clearSession();
        this.router.navigate(['/login']);
        return throwError(() => err);
      })
    );
  }

  private clearSession(): void {
    this.modulos.limpiar();
    this._accessToken.set(null);
    this._currentUser.set(null);
    localStorage.removeItem('access_token');
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
