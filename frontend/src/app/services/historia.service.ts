import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';

export interface PacienteInfo {
  id: number;
  identificacion: string;
  tipo_id: string;
  nombre_completo: string;
  fecha_nacimiento: string;
  sexo: string;
  edad: number;
  celular?: string;
  entidad_nombre?: string;
  ant_familiares_struct?: Record<string, any>;
  ant_personales_struct?: Record<string, any>;
  ant_ginecologicos_struct?: Record<string, any>;
  ant_obstetricos_struct?: Record<string, any>;
}

export interface MedicoInfo {
  id: number;
  nombre_completo: string;
  especialidad?: string;
  registro_medico?: string;
}

export interface Historia {
  id: number;
  paciente_id: number;
  medico_id: number;
  cita_id?: number;
  codigo_historia?: string;
  motivo_consulta?: string;
  enfermedad_actual?: string;
  impresion_diagnostica?: string;
  revision_sistemas?: string;
  examen_fisico?: string;
  // Signos vitales
  peso?: number;
  talla?: number;          // stored in cm
  imc?: number;
  peso_ideal?: number;
  estado_nutricional?: string;
  pam?: string;
  pulso?: number;
  frecuencia_cardiaca?: number;
  frecuencia_respiratoria?: number;
  temperatura?: number;
  pabd?: number;
  pc?: number;
  tension_arterial?: string;
  // Examen físico por sistemas
  ef_cabeza?: string; ef_ojos?: string; ef_nariz?: string; ef_oidos?: string;
  ef_boca?: string; ef_faringe?: string; ef_cuello?: string; ef_pulmones?: string;
  ef_corazon?: string; ef_abdomen?: string; ef_gu?: string; ef_neurologico?: string;
  ef_extremidad?: string; ef_osteomuscular?: string; ef_piel?: string;
  ef_hallazgos?: string;
  // Diagnóstico y plan
  diagnostico_principal?: string;
  diagnostico_relacionado1?: string;
  plan_tratamiento?: string;
  finalidad?: string;
  causa_externa?: string;
  tipo_diagnostico?: string;
  datos_dinamicos?: Record<string, any>;
  genera_incapacidad?: boolean;
  dias_incapacidad?: number;
  fecha?: string;
  firmada: boolean;
  paciente?: PacienteInfo;
  medico?: MedicoInfo;
}

export interface PacienteContexto {
  paciente: PacienteInfo;
  medico?: MedicoInfo;
}

export interface AntecedentesUpdate {
  ant_familiares_struct?: Record<string, any>;
  ant_personales_struct?: Record<string, any>;
  ant_ginecologicos_struct?: Record<string, any>;
  ant_obstetricos_struct?: Record<string, any>;
}

export interface MedicamentoInfo {
  id: number;
  codigo_cum: string;
  nombre: string;
  concentracion: string;
  forma_farmaceutica: string;
  titular?: string;
}

export interface Formula {
  id: number;
  historia_id: number;
  medicamento: string;
  codigo_cum?: string;
  dosis: string;
  cantidad: number;
  unidad?: string;
  dias_tratamiento?: number;
  observaciones?: string;
  fecha_formula: string;
  dispensada: boolean;
}

export interface UnidadDispensacion {
  codigo: number;
  nombre: string;
}

export interface FormulaCreate {
  medicamento: string;
  codigo_cum?: string;
  dosis: string;
  cantidad: number;
  unidad?: string;
  /** Campo M15 del RIPS: el MUV exige que sea mayor que cero. */
  dias_tratamiento?: number;
  observaciones?: string;
}

export type FormulaUpdate = Partial<FormulaCreate>;

export interface EspecialidadInfo {
  id: number;
  nombre: string;
}

export interface Orden {
  id: number;
  historia_id: number;
  paciente_id?: number;
  medico_ordena_id?: number;
  fecha_orden: string;
  estado: string;
  tipo: string;
  codigo_cups?: string;
  descripcion: string;
  justificacion: string;
  especialidad_destino_id?: number;
  especialidad_nombre?: string;
}

export interface OrdenCreate {
  tipo: string;
  descripcion: string;
  justificacion: string;
  codigo_cups?: string;
  especialidad_destino_id?: number;
  institucion_destino_id?: number;
}

export interface ImagenDiagnostica {
  id: number;
  historia_id: number;
  ruta_archivo: string;
  nombre_original: string;
  descripcion: string;
  fecha_subida: string;
}

@Injectable({ providedIn: 'root' })
export class HistoriaService {
  private readonly API = '/api/historias';
  private readonly PACIENTES_API = '/api/pacientes';

  constructor(private http: HttpClient) {}

  create(data: Partial<Historia>): Observable<Historia> {
    return this.http.post<Historia>(this.API, data);
  }

  get(id: number): Observable<Historia> {
    return this.http.get<Historia>(`${this.API}/${id}`);
  }

  update(id: number, data: Partial<Historia>): Observable<Historia> {
    return this.http.put<Historia>(`${this.API}/${id}`, data);
  }

  firmar(id: number): Observable<unknown> {
    return this.http.post(`${this.API}/${id}/firmar`, {});
  }

  getPacienteContexto(pacienteId: number): Observable<PacienteContexto> {
    return this.http.get<PacienteContexto>(`${this.API}/paciente/${pacienteId}/contexto`);
  }

  listByPaciente(pacienteId: number): Observable<Historia[]> {
    return this.http.get<Historia[]>(`${this.API}/paciente/${pacienteId}`);
  }

  delete(id: number): Observable<void> {
    return this.http.delete<void>(`${this.API}/${id}`);
  }

  updateAntecedentes(pacienteId: number, data: AntecedentesUpdate): Observable<unknown> {
    return this.http.put(`${this.PACIENTES_API}/${pacienteId}/antecedentes`, data);
  }

  // ── Medicamentos ────────────────────────────────────────────────
  buscarMedicamentos(q: string): Observable<MedicamentoInfo[]> {
    return this.http.get<MedicamentoInfo[]>(`${this.API}/medicamentos/buscar`, { params: { q } });
  }

  // ── Fórmulas médicas ────────────────────────────────────────────
  listFormulas(historiaId: number): Observable<Formula[]> {
    return this.http.get<Formula[]>(`${this.API}/${historiaId}/formulas`);
  }

  /** Las presentaciones que el RIPS sabe traducir a su código M13.
   *  Se piden al backend y no se copian aquí: una lista en dos sitios se
   *  desincroniza, y de ese código dependen el valor unitario y la cantidad. */
  getUnidadesDispensacion(): Observable<UnidadDispensacion[]> {
    return this.http.get<UnidadDispensacion[]>(`${this.API}/unidades-dispensacion`);
  }

  createFormula(historiaId: number, data: FormulaCreate): Observable<Formula> {
    return this.http.post<Formula>(`${this.API}/${historiaId}/formulas`, data);
  }

  updateFormula(historiaId: number, formulaId: number, data: FormulaUpdate): Observable<Formula> {
    return this.http.put<Formula>(`${this.API}/${historiaId}/formulas/${formulaId}`, data);
  }

  deleteFormula(historiaId: number, formulaId: number): Observable<void> {
    return this.http.delete<void>(`${this.API}/${historiaId}/formulas/${formulaId}`);
  }

  // ── Especialidades ───────────────────────────────────────────────
  listEspecialidades(): Observable<EspecialidadInfo[]> {
    return this.http.get<EspecialidadInfo[]>(`${this.API}/especialidades`);
  }

  // ── Órdenes médicas ──────────────────────────────────────────────
  listOrdenes(historiaId: number): Observable<Orden[]> {
    return this.http.get<Orden[]>(`${this.API}/${historiaId}/ordenes`);
  }

  createOrden(historiaId: number, data: OrdenCreate): Observable<Orden> {
    return this.http.post<Orden>(`${this.API}/${historiaId}/ordenes`, data);
  }

  updateOrden(historiaId: number, ordenId: number, data: Partial<OrdenCreate & { estado: string }>): Observable<Orden> {
    return this.http.put<Orden>(`${this.API}/${historiaId}/ordenes/${ordenId}`, data);
  }

  deleteOrden(historiaId: number, ordenId: number): Observable<void> {
    return this.http.delete<void>(`${this.API}/${historiaId}/ordenes/${ordenId}`);
  }

  // ── Imágenes diagnósticas ────────────────────────────────────────
  listImagenes(historiaId: number): Observable<ImagenDiagnostica[]> {
    return this.http.get<ImagenDiagnostica[]>(`${this.API}/${historiaId}/imagenes-diagnosticas`);
  }

  uploadImagen(historiaId: number, file: File, descripcion: string): Observable<ImagenDiagnostica> {
    const formData = new FormData();
    formData.append('file', file);
    formData.append('descripcion', descripcion);
    return this.http.post<ImagenDiagnostica>(`${this.API}/${historiaId}/imagenes`, formData);
  }

  deleteImagen(historiaId: number, imagenId: number): Observable<void> {
    return this.http.delete<void>(`${this.API}/${historiaId}/imagenes-diagnosticas/${imagenId}`);
  }
}
