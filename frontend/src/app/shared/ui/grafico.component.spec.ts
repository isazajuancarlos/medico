// SPDX-License-Identifier: LicenseRef-Propietario
import { TestBed } from '@angular/core/testing';
import { ChartData } from 'chart.js';
import { describe, expect, it } from 'vitest';
import { GraficoComponent, hayQueDibujar } from './grafico.component';

/**
 * Lo que se prueba aquí es **la decisión de dibujar**, no el dibujo.
 *
 * jsdom no da contexto 2D, así que un lienzo de chart.js no llega a existir en
 * pruebas — y montar uno de verdad probaría a chart.js, que ya viene probada.
 * Lo que sí es nuestro, y lo que puede estar mal, es cuándo aparece un gráfico:
 * el banco de humo abre `estadisticas` en la pestaña «resumen», donde no hay
 * ninguno, así que sin estas pruebas la pieza entera estaría sin red.
 */
describe('cuándo hay algo que dibujar', () => {
  const serie = (datos: number[]): ChartData => ({
    labels: datos.map((_, i) => `x${i}`),
    datasets: [{ data: datos }],
  });

  it('con datos, sí', () => {
    expect(hayQueDibujar(serie([3, 1, 4]))).toBe(true);
  });

  it('un solo valor basta: un consultorio con una atención tiene un gráfico', () => {
    expect(hayQueDibujar(serie([1]))).toBe(true);
  });

  it('un cero es un dato, no una ausencia', () => {
    // Cero citas canceladas en el mes es información, y la barra en cero es su
    // forma correcta. Confundirlo con «todavía no ha llegado» borraría del
    // panel justo el indicador bueno.
    expect(hayQueDibujar(serie([0, 0, 0]))).toBe(true);
  });

  it('sin datos, no', () => {
    expect(hayQueDibujar(null)).toBe(false);
    expect(hayQueDibujar(undefined)).toBe(false);
  });

  it('con `datasets` vacío, no', () => {
    expect(hayQueDibujar({ labels: [], datasets: [] })).toBe(false);
  });

  it('con una serie SIN puntos, tampoco', () => {
    // Es el estado real mientras la petición viaja: los `computed` de
    // `estadisticas` devuelven `{ labels: [], datasets: [{ data: [] }] }` sobre
    // un `?? {}`. Dibujarlo pinta ejes en cero, que en un panel de indicadores
    // se leen como «no hubo atenciones» — una afirmación que nadie hizo.
    expect(hayQueDibujar(serie([]))).toBe(false);
  });

  it('basta con que UNA serie traiga puntos', () => {
    expect(hayQueDibujar({
      labels: ['a'],
      datasets: [{ data: [] }, { data: [7] }],
    })).toBe(true);
  });
});

describe('el componente', () => {
  it('monta y no lanza aunque no haya contexto 2D', () => {
    // jsdom no lo da. Que la pantalla se caiga por no poder dibujar un gráfico
    // escondería los fallos que sí importan.
    const fixture = TestBed.createComponent(GraficoComponent);
    fixture.componentRef.setInput('type', 'doughnut');
    fixture.componentRef.setInput('data', {
      labels: ['a', 'b'],
      datasets: [{ data: [1, 2] }],
    });
    expect(() => fixture.detectChanges()).not.toThrow();
    expect(fixture.nativeElement.querySelector('canvas')).toBeTruthy();
  });

  it('respeta la altura pedida', () => {
    // `height` era un atributo de `p-chart` y se conserva. Sin alto explícito,
    // `maintainAspectRatio: false` deja el lienzo en cero y el gráfico no se ve.
    const fixture = TestBed.createComponent(GraficoComponent);
    fixture.componentRef.setInput('type', 'bar');
    fixture.componentRef.setInput('height', '420px');
    fixture.detectChanges();

    const caja = fixture.nativeElement.querySelector('.lienzo') as HTMLElement;
    expect(caja.style.height).toBe('420px');
  });
});
