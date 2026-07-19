export type EstadoCita = 'Pendiente' | 'Confirmada' | 'Atendida' | 'Cancelada' | 'No Asistió';
export type TipoCita = 'Primera Vez' | 'Control / Revisión' | 'Urgencia';

export interface Appointment {
  id: number;
  paciente_id: number;
  medico_id: number;
  fecha: string;
  hora: string;
  tipo: TipoCita;
  estado: EstadoCita;
  motivo?: string;
  duracion_minutos: number;
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
}

export interface SlotDisponible {
  hora: string;
  disponible: boolean;
  motivo_no_disponible?: string;
}

export interface MedicoDisponible {
  id: number;
  nombre: string;
  rol: string;
  especialidad?: string;
}
