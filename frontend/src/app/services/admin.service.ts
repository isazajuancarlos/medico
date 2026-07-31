import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

export interface Institucion {
  id: number;
  nit: string;
  razon_social: string;
  nombre_comercial: string;
  codigo_habilitacion?: string;
  direccion: string;
  ciudad: string;
  departamento: string;
  telefono: string;
  email: string;
  representante_legal: string;
  identificacion_representante: string;
  codigo_prestador: string;
  tipo_prestador: string;
  color_primario: string;
  color_secundario: string;
  activa: boolean;
}

export interface ConfiguracionInstitucion {
  id: number;
  horario_apertura: string;
  horario_cierre: string;
  dias_atencion: string[];
  duracion_cita_default: number;
  tiempo_buffer: number;
  enviar_recordatorios: boolean;
  horas_anticipacion: number;
  smtp_host?: string;
  smtp_port?: number;
  smtp_usuario?: string;
  prefijo_factura: string;
  resolucion_facturacion?: string;
  rango_desde?: number;
  rango_hasta?: number;
  permitir_citas_online: boolean;
  requiere_autorizacion: boolean;
  dias_maximos_agendamiento: number;
}

export interface PoliticaSeguridad {
  id: number;
  longitud_minima_password: number;
  requiere_mayuscula: boolean;
  requiere_minuscula: boolean;
  requiere_numero: boolean;
  requiere_caracter_especial: boolean;
  dias_expiracion_password: number;
  historial_passwords: number;
  timeout_sesion_minutos: number;
  sesiones_simultaneas_permitidas: number;
  cerrar_otras_sesiones_al_login: boolean;
  max_intentos_login: number;
  tiempo_bloqueo_minutos: number;
  requiere_2fa: boolean;
  requiere_2fa_admin: boolean;
  metodo_2fa: string;
  registrar_todos_los_accesos: boolean;
  retener_logs_dias: number;
  notificar_login_nuevo_dispositivo: boolean;
  notificar_cambio_password: boolean;
  notificar_intentos_fallidos: boolean;
}

export interface UsuarioAdmin {
  id: number;
  username: string;
  nombre_completo: string;
  email?: string;
  rol: string;
  activo: boolean;
  cuenta_bloqueada: boolean;
  especialidad_id?: number;
  especialidad_nombre?: string;
  registro_medico?: string;
  registro_rethus?: string;
  requiere_cambio_password: boolean;
  ultimo_cambio_password?: string;
}

export interface UsuarioCreate {
  username: string;
  password: string;
  nombre_completo: string;
  email?: string;
  rol: string;
  especialidad_id?: number;
  registro_medico?: string;
  registro_rethus?: string;
}

export interface UsuarioUpdate {
  nombre_completo?: string;
  email?: string;
  rol?: string;
  activo?: boolean;
  especialidad_id?: number;
  registro_medico?: string;
  registro_rethus?: string;
  requiere_cambio_password?: boolean;
}

export interface ConfigCampo {
  id: number;
  etiqueta: string;
  tipo: string;
  opciones?: string;
  activo: boolean;
  orden: number;
}

export interface AuditoriaItem {
  id: number;
  fecha_hora: string;
  usuario_id: number;
  usuario_nombre: string;
  accion: string;
  tabla_afectada: string;
  registro_id: number;
  paciente_id?: number;
  ip_address?: string;
  exitoso: boolean;
}

export interface Especialidad {
  id: number;
  nombre: string;
}

export interface EntidadEPS {
  id: number;
  nit: string;
  nombre: string;
  codigo_eps: string;
}

@Injectable({ providedIn: 'root' })
export class AdminService {
  private readonly API = '/api/admin';

  constructor(private http: HttpClient) {}

  // Institución
  getInstitucion(): Observable<Institucion> {
    return this.http.get<Institucion>(`${this.API}/institucion`);
  }
  updateInstitucion(data: Partial<Institucion>): Observable<Institucion> {
    return this.http.put<Institucion>(`${this.API}/institucion`, data);
  }

  // Configuración
  getConfiguracion(): Observable<ConfiguracionInstitucion> {
    return this.http.get<ConfiguracionInstitucion>(`${this.API}/configuracion`);
  }
  updateConfiguracion(data: Partial<ConfiguracionInstitucion>): Observable<ConfiguracionInstitucion> {
    return this.http.put<ConfiguracionInstitucion>(`${this.API}/configuracion`, data);
  }

  // Política de seguridad
  getPolitica(): Observable<PoliticaSeguridad> {
    return this.http.get<PoliticaSeguridad>(`${this.API}/politica-seguridad`);
  }
  updatePolitica(data: Partial<PoliticaSeguridad>): Observable<PoliticaSeguridad> {
    return this.http.put<PoliticaSeguridad>(`${this.API}/politica-seguridad`, data);
  }

  // Usuarios
  listUsuarios(params: Record<string, string> = {}): Observable<UsuarioAdmin[]> {
    return this.http.get<UsuarioAdmin[]>(`${this.API}/usuarios`, { params });
  }
  createUsuario(data: UsuarioCreate): Observable<any> {
    return this.http.post(`${this.API}/usuarios`, data);
  }
  updateUsuario(id: number, data: UsuarioUpdate): Observable<any> {
    return this.http.put(`${this.API}/usuarios/${id}`, data);
  }
  deleteUsuario(id: number): Observable<any> {
    return this.http.delete(`${this.API}/usuarios/${id}`);
  }
  desbloquearUsuario(id: number): Observable<any> {
    return this.http.post(`${this.API}/usuarios/${id}/desbloquear`, {});
  }
  resetPassword(id: number, nueva_password: string): Observable<any> {
    return this.http.post(`${this.API}/usuarios/${id}/reset-password`, { nueva_password });
  }

  // Campos dinámicos
  listCampos(): Observable<ConfigCampo[]> {
    return this.http.get<ConfigCampo[]>(`${this.API}/campos-dinamicos`);
  }
  createCampo(data: Partial<ConfigCampo>): Observable<ConfigCampo> {
    return this.http.post<ConfigCampo>(`${this.API}/campos-dinamicos`, data);
  }
  updateCampo(id: number, data: Partial<ConfigCampo>): Observable<ConfigCampo> {
    return this.http.put<ConfigCampo>(`${this.API}/campos-dinamicos/${id}`, data);
  }
  deleteCampo(id: number): Observable<any> {
    return this.http.delete(`${this.API}/campos-dinamicos/${id}`);
  }

  // Auditoría
  listAuditoria(params: Record<string, string> = {}): Observable<{ total: number; items: AuditoriaItem[] }> {
    return this.http.get<{ total: number; items: AuditoriaItem[] }>(`${this.API}/auditoria`, { params });
  }

  // Especialidades
  listEspecialidades(): Observable<Especialidad[]> {
    return this.http.get<Especialidad[]>(`${this.API}/especialidades`);
  }
  createEspecialidad(nombre: string): Observable<Especialidad> {
    return this.http.post<Especialidad>(`${this.API}/especialidades`, { nombre });
  }
  deleteEspecialidad(id: number): Observable<any> {
    return this.http.delete(`${this.API}/especialidades/${id}`);
  }

  // Entidades (EPS)
  listEntidades(q?: string): Observable<EntidadEPS[]> {
    const params: Record<string, string> = {};
    if (q) params['q'] = q;
    return this.http.get<EntidadEPS[]>(`${this.API}/entidades`, { params });
  }
  createEntidad(data: Omit<EntidadEPS, 'id'>): Observable<EntidadEPS> {
    return this.http.post<EntidadEPS>(`${this.API}/entidades`, data);
  }
  updateEntidad(id: number, data: Partial<EntidadEPS>): Observable<EntidadEPS> {
    return this.http.put<EntidadEPS>(`${this.API}/entidades/${id}`, data);
  }
  deleteEntidad(id: number): Observable<any> {
    return this.http.delete(`${this.API}/entidades/${id}`);
  }

  // Matriz de capacidades por institución (#235)
  getCapacidades(): Observable<MatrizCapacidades> {
    return this.http.get<MatrizCapacidades>(`${this.API}/capacidades`);
  }
  updateCapacidades(cambios: CambioCapacidad[]): Observable<MatrizCapacidades> {
    return this.http.put<MatrizCapacidades>(`${this.API}/capacidades`, { cambios });
  }
}

/// Una casilla de la matriz, tal como la sirve el servidor.
///
/// `permitido` es la respuesta EFECTIVA y `defecto` lo que diría la regla
/// nacional; la pantalla solo los compara para pintar. **No recalcula el
/// defecto por su cuenta a propósito**: tenerlo escrito aquí y en Rust es cómo
/// las dos copias se desincronizan, y la del navegador es la que no manda.
export interface CasillaCapacidad {
  rol: string;
  permitido: boolean;
  defecto: boolean;
  /// Hay una fila guardada. No es lo mismo que `permitido !== defecto`: se
  /// puede haber guardado una casilla que hoy coincide con el defecto, y esa
  /// deja de seguirlo si el defecto cambia. Por eso existe «devolver al
  /// defecto» como acción aparte.
  apartado: boolean;
  configurable: boolean;
}

export interface FilaCapacidad {
  capacidad: string;
  configurable: boolean;
  roles: CasillaCapacidad[];
}

export interface MatrizCapacidades {
  institucion_id: number;
  capacidades: FilaCapacidad[];
}

/// `permitido: null` devuelve la casilla al defecto (borra la excepción).
export interface CambioCapacidad {
  capacidad: string;
  rol: string;
  permitido: boolean | null;
}
