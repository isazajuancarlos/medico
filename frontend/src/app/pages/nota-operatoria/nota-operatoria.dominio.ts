/**
 * Reglas de la nota operatoria, migradas de `backend/notas_operatorias.py`.
 *
 * Puras y probadas aparte del formulario a propósito: son las reglas de `Valid`
 * del original, y si viven solo en la pantalla no se pueden verificar ni reusar.
 *
 * Dos decisiones que vienen del original y NO se tocan:
 *  - **El tiempo quirúrgico se DERIVA de las horas**, no se guarda por separado:
 *    tres campos para dos datos acaban discrepando.
 *  - **`estado_paciente_salida` es obligatorio**, sin defecto: «ESTABLE» de
 *    relleno es una afirmación clínica que nadie hizo.
 */

export const ESTADOS_SALIDA = ['ESTABLE', 'INESTABLE', 'CRITICO', 'FALLECIDO'] as const;
export type EstadoSalida = (typeof ESTADOS_SALIDA)[number];

export interface Duracion {
  minutos: number;
  horas: number;
}

/**
 * Minutos de cirugía a partir de las horas. `null` si las horas no describen un
 * intervalo posible (fin antes que inicio, o cero minutos). Las horas son
 * `datetime` completos, así que una cirugía que cruza la medianoche —23:40 a
 * 01:20— dura 100 minutos, no menos veintidós horas.
 */
export function duracion(inicioISO: string, finISO: string): Duracion | null {
  const i = Date.parse(inicioISO);
  const f = Date.parse(finISO);
  if (Number.isNaN(i) || Number.isNaN(f)) return null;
  const minutos = Math.floor((f - i) / 60000);
  if (minutos <= 0) return null;
  return { minutos, horas: Math.round((minutos / 60) * 100) / 100 };
}

/** «2 h 15 min» se lee mejor que «135». */
export function formatoDuracion(minutos: number): string {
  const h = Math.floor(minutos / 60);
  const m = minutos % 60;
  return h ? `${h} h ${m} min` : `${m} min`;
}

/** El estado normalizado si es uno de los válidos; `null` si falta o no lo es. */
export function estadoValido(estado: string | null | undefined): EstadoSalida | null {
  const n = (estado ?? '').trim().toUpperCase();
  return (ESTADOS_SALIDA as readonly string[]).includes(n) ? (n as EstadoSalida) : null;
}

export interface NotaParaRevisar {
  hora_inicio: string;
  hora_fin: string;
  sangrado_estimado_ml: number | null;
  complicaciones?: string | null;
  estado_paciente_salida?: string | null;
  firmada?: boolean;
}

/**
 * Señala lo que merece una segunda mirada, SIN impedir guardar: en cirugía lo
 * infrecuente pasa, y un sistema que no deja registrar lo que ocurrió obliga a
 * mentirle. (La discrepancia «tiempo escrito vs horas» del original no aplica:
 * aquí el tiempo se deriva, no se escribe, así que no hay dos valores que
 * disientan.)
 */
export function revisar(n: NotaParaRevisar): string[] {
  const avisos: string[] = [];

  const t = duracion(n.hora_inicio, n.hora_fin);
  if (t && t.minutos > 12 * 60) {
    avisos.push(`La cirugía figura con ${t.horas} horas de duración.`);
  }

  if (n.sangrado_estimado_ml == null) {
    avisos.push('No se registró el sangrado estimado.');
  } else if (n.sangrado_estimado_ml > 1000) {
    avisos.push(`Sangrado estimado de ${n.sangrado_estimado_ml} ml.`);
  }

  if (n.complicaciones && n.complicaciones.trim()) {
    avisos.push('La nota registra complicaciones.');
  }

  const est = (n.estado_paciente_salida ?? '').trim();
  if (est && est.toUpperCase() !== 'ESTABLE') {
    avisos.push(`El paciente salió «${est}» de quirófano.`);
  }

  if (n.firmada === false) {
    avisos.push('Falta la firma del cirujano.');
  }

  return avisos;
}
