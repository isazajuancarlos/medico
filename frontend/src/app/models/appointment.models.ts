export type EstadoCita = 'Pendiente' | 'Confirmada' | 'Atendida' | 'Cancelada' | 'No Asistió';
export type TipoCita = 'Primera Vez' | 'Control / Revisión' | 'Urgencia';

/** El vocabulario con el que el tipo viaja a la base, igual que `estado`.
 *  La pantalla traduce; la columna no guarda texto de interfaz. */
export type TipoCitaApi = 'PRIMERA_VEZ' | 'CONTROL' | 'URGENCIA';

export interface Appointment {
  id: number;
  paciente_id: number;
  medico_id: number;
  fecha: string;
  hora: string;
  /** `undefined` = NO SE REGISTRÓ. Hasta la migración 0035 el servicio rellenaba
   *  'Primera Vez' para todas, y en la pantalla eso era indistinguible de un
   *  dato real. Se muestra vacío, no se inventa. */
  tipo?: TipoCita;
  estado: EstadoCita;
  motivo?: string;
  /** `undefined` = no se registró. Antes se rellenaba con 30. */
  duracion_minutos?: number;
  /** Cuándo la pidió el paciente: es lo que mide la oportunidad (Res. 256/2016). */
  solicitada_en?: string;
  paciente_nombre?: string;
  medico_nombre?: string;
}

export interface AppointmentCreate {
  paciente_id: number;
  medico_id: number;
  fecha: string;
  hora: string;
  tipo: TipoCita;
  motivo?: string;
  duracion_minutos: number;
  solicitada_en?: string;
}

/** Por qué un turno no está libre, tal como lo manda el binario. */
export type MotivoTurno =
  | { clase: 'bloqueado'; detalle: string }
  | { clase: 'cita_agendada' }
  | { clase: 'cita_de_duracion_desconocida' };

export interface SlotDisponible {
  hora: string;
  disponible: boolean;
  motivo?: MotivoTurno;
}

export interface MedicoDisponible {
  id: number;
  nombre: string;
  rol: string;
  especialidad?: string;
}
