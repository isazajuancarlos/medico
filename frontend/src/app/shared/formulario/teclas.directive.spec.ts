/**
 * Los atajos de teclado heredados del FoxPro.
 *
 * Lo que se protege aquí es la COSTUMBRE del usuario, que es lo primero que se
 * pierde en una migración y lo último que se perdona. En particular que Enter
 * BAJE al siguiente campo en vez de enviar: quien lleva años digitando «dato,
 * Enter, dato, Enter» guardaría la historia a medio llenar una y otra vez.
 */
import { Component, ChangeDetectionStrategy } from '@angular/core';
import { ComponentFixture, TestBed } from '@angular/core/testing';
import { TeclasDirective } from './teclas.directive';

@Component({
  standalone: true,
  imports: [TeclasDirective],
  changeDetection: ChangeDetectionStrategy.Eager,
  template: `
    <form
      appTeclas
      [enterAvanza]="enterAvanza"
      (confirmar)="confirmado = confirmado + 1"
      (cancelar)="cancelado = cancelado + 1"
    >
      <input id="a" />
      <input id="oculto" style="display:none" />
      <input id="b" />
      <textarea id="area"></textarea>
      <input id="c" />
      <button id="boton" type="button"></button>
    </form>
  `,
})
class Anfitrion {
  enterAvanza = true;
  confirmado = 0;
  cancelado = 0;
}

describe('atajos de teclado del formulario', () => {
  let fixture: ComponentFixture<Anfitrion>;
  let anfitrion: Anfitrion;

  beforeEach(() => {
    TestBed.configureTestingModule({ imports: [Anfitrion] });
    fixture = TestBed.createComponent(Anfitrion);
    anfitrion = fixture.componentInstance;
    fixture.detectChanges();
    // El foco solo se mueve de verdad si el elemento está en el documento.
    document.body.appendChild(fixture.nativeElement);
  });

  afterEach(() => fixture.nativeElement.remove());

  function campo(id: string): HTMLElement {
    return fixture.nativeElement.querySelector(`#${id}`);
  }

  function pulsar(id: string, tecla: string, mod: Partial<KeyboardEventInit> = {}) {
    const evento = new KeyboardEvent('keydown', { key: tecla, bubbles: true, cancelable: true, ...mod });
    campo(id).dispatchEvent(evento);
    return evento;
  }

  describe('Enter avanza, no envía', () => {
    it('lleva el foco al siguiente campo', () => {
      campo('a').focus();
      const evento = pulsar('a', 'Enter');

      expect(document.activeElement).toBe(campo('b'));
      expect(evento.defaultPrevented).toBe(true);
    });

    it('salta los campos ocultos', () => {
      // Entre «a» y «b» hay un input con display:none. Darle el foco dejaría al
      // usuario escribiendo en un campo que no ve. Se comprueba que «oculto» es
      // de verdad el siguiente en el DOM, para que la prueba no pueda pasar
      // simplemente porque el foco no se movió.
      expect(campo('a').nextElementSibling).toBe(campo('oculto'));

      campo('a').focus();
      pulsar('a', 'Enter');
      expect(document.activeElement).toBe(campo('b'));
    });

    it('no se mueve desde el último campo, en vez de dar la vuelta', () => {
      // Primero se comprueba que en este mismo formulario el foco SÍ avanza;
      // si no, «no se movió» no probaría nada.
      campo('b').focus();
      pulsar('b', 'Enter');
      expect(document.activeElement).not.toBe(campo('b'));

      campo('c').focus();
      pulsar('c', 'Enter');
      expect(document.activeElement).toBe(campo('c'));
    });

    it('en un textarea Enter es un salto de línea y se respeta', () => {
      campo('area').focus();
      const evento = pulsar('area', 'Enter');

      expect(evento.defaultPrevented).toBe(false);
      expect(document.activeElement).toBe(campo('area'));
    });

    it('en un botón Enter lo pulsa y se respeta', () => {
      const evento = pulsar('boton', 'Enter');
      expect(evento.defaultPrevented).toBe(false);
    });

    it('con enterAvanza en false se comporta como la web, para un buscador', () => {
      anfitrion.enterAvanza = false;
      fixture.detectChanges();

      campo('a').focus();
      const evento = pulsar('a', 'Enter');

      expect(evento.defaultPrevented).toBe(false);
      expect(document.activeElement).toBe(campo('a'));
    });

    it('avanzar NO confirma: son gestos distintos', () => {
      pulsar('a', 'Enter');
      expect(anfitrion.confirmado).toBe(0);
    });
  });

  describe('Ctrl+Enter confirma', () => {
    it('emite confirmar y no mueve el foco', () => {
      campo('a').focus();
      const evento = pulsar('a', 'Enter', { ctrlKey: true });

      expect(anfitrion.confirmado).toBe(1);
      expect(document.activeElement).toBe(campo('a'));
      expect(evento.defaultPrevented).toBe(true);
    });

    it('Cmd+Enter hace lo mismo, para quien trabaje en Mac', () => {
      pulsar('a', 'Enter', { metaKey: true });
      expect(anfitrion.confirmado).toBe(1);
    });

    it('confirma incluso desde un textarea, donde Enter solo salta línea', () => {
      pulsar('area', 'Enter', { ctrlKey: true });
      expect(anfitrion.confirmado).toBe(1);
    });
  });

  describe('Esc cancela', () => {
    it('emite cancelar en vez de cerrar por su cuenta', () => {
      // El original PREGUNTA antes de cerrar. La directiva solo avisa; quien
      // escuche decide, y así no puede perder una historia a medio escribir.
      const evento = pulsar('a', 'Escape');

      expect(anfitrion.cancelado).toBe(1);
      expect(evento.defaultPrevented).toBe(true);
    });

    it('cancelar no es confirmar', () => {
      pulsar('a', 'Escape');
      expect(anfitrion.confirmado).toBe(0);
    });
  });

  it('las demás teclas pasan sin que la directiva las toque', () => {
    for (const tecla of ['a', 'Tab', 'F5', 'ArrowDown']) {
      const evento = pulsar('a', tecla);
      expect(evento.defaultPrevented).toBe(false);
    }
    expect(anfitrion.confirmado).toBe(0);
    expect(anfitrion.cancelado).toBe(0);
  });
});
