export interface PaginatedResponse<T> {
  total: number;
  items: T[];
  page: number;
  size: number;
}

export interface MsgResponse {
  message: string;
}

export interface KPIs {
  citas_hoy: number;
  citas_mes: number;
  total_pacientes: number;
  fecha: string;
}
