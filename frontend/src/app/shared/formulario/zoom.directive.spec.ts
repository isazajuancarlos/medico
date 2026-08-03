/**
 * El zoom por clic derecho: 828 usos en 31 de los 37 formularios del original.
 *
 * Es el comportamiento más extendido del sistema y el más fácil de descartar
 * por raro. Se prueba para que nadie lo quite sin darse cuenta.
 */
import { Component } from '@angular/core';
import { ComponentFixture, TestBed } from '@angular/core/testing';
import { ZoomDirective } from './zoom.directive';

@Component({
  standalone: true,
  imports: [ZoomDirective],
  template: `
    <span id="etiqueta" appZoom>{{ texto }}</span>
    <input id="campo" appZoom [value]="valor" />
    <span id="explicito" appZoom [zoomTexto]="explicito">se muestra otra cosa</span>
    <span id="vacio" appZoom></span>
  `,
})
class Anfitrion {
  texto = 'J449 — EPOC no especificada';
  valor = '1017234567';
  explicito = 'texto ampliado a propósito';
}

describe('zoom del formulario', () => {
  let fixture: ComponentFixture<Anfitrion>;
  let ampliados: string[];
  let escucha: (e: Event) => void;

  beforeEach(() => {
    TestBed.configureTestingModule({ imports: [Anfitrion] });
    fixture = TestBed.createComponent(Anfitrion);
    fixture.detectChanges();

    ampliados = [];
    escucha = (e: Event) => ampliados.push((e as CustomEvent).detail.texto);
    document.addEventListener('medico:zoom', escucha);
  });

  afterEach(() => document.removeEventListener('medico:zoom', escucha));

  function elemento(id: string): HTMLElement {
    return fixture.nativeElement.querySelector(`#${id}`);
  }

  function clicDerecho(id: string) {
    const evento = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    elemento(id).dispatchEvent(evento);
    return evento;
  }

  it('amplía el texto de una etiqueta', () => {
    clicDerecho('etiqueta');
    expect(ampliados).toEqual(['J449 — EPOC no especificada']);
  });

  it('amplía el valor de un campo, no su texto', () => {
    clicDerecho('campo');
    expect(ampliados).toEqual(['1017234567']);
  });

  it('zoomTexto manda sobre el contenido del elemento', () => {
    clicDerecho('explicito');
    expect(ampliados).toEqual(['texto ampliado a propósito']);
  });

  it('sustituye el menú del navegador: si no, aparecerían los dos', () => {
    expect(clicDerecho('etiqueta').defaultPrevented).toBe(true);
  });

  it('un elemento sin contenido no abre una ventana vacía', () => {
    clicDerecho('vacio');
    expect(ampliados).toEqual([]);
  });

  it('F2 hace lo mismo por teclado, que el original no tenía', () => {
    const evento = new KeyboardEvent('keydown', { key: 'F2', bubbles: true, cancelable: true });
    elemento('etiqueta').dispatchEvent(evento);

    expect(ampliados).toEqual(['J449 — EPOC no especificada']);
    expect(evento.defaultPrevented).toBe(true);
  });

  it('otras teclas no lo disparan', () => {
    for (const key of ['F1', 'Enter', 'z']) {
      elemento('etiqueta').dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
    }
    expect(ampliados).toEqual([]);
  });
});
