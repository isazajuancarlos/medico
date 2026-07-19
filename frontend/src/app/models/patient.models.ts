export type TipoUsuario = '01' | '02' | '03' | '04' | '05';
export type ZonaResidencia = '01' | '02';

export interface Patient {
  id: number;
  identificacion: string;
  tipo_id: string;
  nombre1: string;
  nombre2?: string;
  apellido1: string;
  apellido2?: string;
  fecha_nacimiento: string;
  sexo: string;
  celular: string;
  email?: string;
  direccion?: string;
  tipo_usuario: TipoUsuario;
  cod_municipio: string;
  cod_pais: string;
  zona: ZonaResidencia;
  entidad_id?: number;
  institucion_id?: number;
  edad?: number;
}

export interface PatientCreate {
  identificacion: string;
  tipo_id: string;
  nombre1: string;
  nombre2?: string;
  apellido1: string;
  apellido2?: string;
  fecha_nacimiento: string;
  sexo: string;
  celular: string;
  email?: string;
  direccion?: string;
  tipo_usuario: TipoUsuario;
  cod_municipio: string;
  cod_pais: string;
  zona: ZonaResidencia;
  entidad_id?: number;
}
