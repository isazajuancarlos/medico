/**
 * Las reglas de la nota operatoria, probadas aparte del formulario. Lo que se
 * verifica es el comportamiento que el original `notas_operatorias.py` fijaba:
 * la duración que cruza medianoche, el estado obligatorio, y los avisos que
 * señalan sin bloquear.
 */
import { describe, expect, it } from 'vitest';
import { ESTADOS_SALIDA, duracion, estadoValido, revisar } from './nota-operatoria.dominio';

describe('nota operatoria — duración', () => {
  it('cuenta los minutos entre dos instantes', () => {
    expect(duracion('2026-07-29T08:00:00', '2026-07-29T09:30:00')).toEqual({ minutos: 90, horas: 1.5 });
  });

  it('aguanta que la cirugía cruce la medianoche', () => {
    // 23:40 → 01:20 son 100 minutos, no menos veintidós horas.
    expect(duracion('2026-07-29T23:40:00', '2026-07-30T01:20:00')?.minutos).toBe(100);
  });

  it('rechaza que el fin sea anterior al inicio', () => {
    expect(duracion('2026-07-29T09:00:00', '2026-07-29T08:00:00')).toBeNull();
  });

  it('rechaza una cirugía de cero minutos y las horas ilegibles', () => {
    expect(duracion('2026-07-29T09:00:00', '2026-07-29T09:00:00')).toBeNull();
    expect(duracion('no-es-fecha', '2026-07-29T09:00:00')).toBeNull();
  });
});

describe('nota operatoria — estado de salida (obligatorio, sin defecto)', () => {
  it('acepta los cuatro estados, normalizando mayúsculas y espacios', () => {
    for (const e of ESTADOS_SALIDA) {
      expect(estadoValido(e.toLowerCase())).toBe(e);
      expect(estadoValido(`  ${e}  `)).toBe(e);
    }
  });

  it('rechaza el vacío y lo desconocido (no hay «ESTABLE» de relleno)', () => {
    expect(estadoValido('')).toBeNull();
    expect(estadoValido(null)).toBeNull();
    expect(estadoValido('MEJORADO')).toBeNull();
  });
});

describe('nota operatoria — avisos (señalan sin bloquear)', () => {
  const base = {
    hora_inicio: '2026-07-29T08:00:00',
    hora_fin: '2026-07-29T09:00:00',
    sangrado_estimado_ml: 50,
    complicaciones: '',
    estado_paciente_salida: 'ESTABLE',
    firmada: true,
  };

  it('una nota rutinaria y completa no genera avisos', () => {
    expect(revisar(base)).toEqual([]);
  });

  it('avisa de una cirugía de más de 12 horas', () => {
    const r = revisar({ ...base, hora_fin: '2026-07-29T21:00:00' });
    expect(r.some(a => a.includes('horas de duración'))).toBe(true);
  });

  it('avisa si no se registró el sangrado, y si es alto', () => {
    expect(revisar({ ...base, sangrado_estimado_ml: null }).some(a => a.includes('No se registró el sangrado'))).toBe(true);
    expect(revisar({ ...base, sangrado_estimado_ml: 1500 }).some(a => a.includes('1500 ml'))).toBe(true);
  });

  it('avisa de complicaciones y de un estado distinto de ESTABLE', () => {
    expect(revisar({ ...base, complicaciones: 'hipotensión' }).some(a => a.includes('complicaciones'))).toBe(true);
    expect(revisar({ ...base, estado_paciente_salida: 'CRITICO' }).some(a => a.includes('CRITICO'))).toBe(true);
  });

  it('avisa de la firma que falta', () => {
    expect(revisar({ ...base, firmada: false }).some(a => a.includes('firma del cirujano'))).toBe(true);
  });
});
