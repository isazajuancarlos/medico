export type RolUsuario =
  | 'superadmin'
  | 'admin'
  | 'medico'
  | 'paciente'
  | 'enfermeria'
  | 'auxiliar_enfermeria'
  | 'regente_farmacia'
  | 'laboratorista'
  | 'radiologo'
  | 'anestesiologo'
  | 'terapeuta'
  | 'nutricionista'
  | 'psicologo'
  | 'trabajo_social'
  | 'bacteriologo';

export interface CurrentUser {
  id: number;
  username: string;
  nombre_completo: string;
  rol: RolUsuario;
  institucion_id: number | null;
  email: string | null;
}

/** Lo que devuelve el `medico-api` en `GET /api/yo`. La identidad viene del
 *  servidor, no de un token que el JavaScript pueda leer. */
export interface YoResponse {
  id: number;
  nombre: string;
  email: string;
  rol: RolUsuario;
  institucion_id: number | null;
}

export interface LoginRequest {
  email: string;
  clave: string;
}
