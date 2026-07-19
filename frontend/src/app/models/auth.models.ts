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

export interface TokenResponse {
  access_token: string;
  refresh_token: string;
  token_type: string;
  user: CurrentUser;
}

export interface LoginRequest {
  username: string;
  password: string;
}
