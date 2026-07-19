import { Directive, EventEmitter, HostListener, Input, Output } from '@angular/core';

/**
 * Los atajos de teclado del sistema original, tal como están en la costumbre
 * del usuario.
 *
 * Extraídos de los manejadores `KeyPress` del FoxPro:
 *
 *   Esc          cierra el formulario sin guardar
 *   Ctrl+Enter   confirma (equivale al botón Aceptar) — 89 usos en 22 formularios
 *   Enter        BAJA AL SIGUIENTE CAMPO, no envía
 *
 * El tercero es el que más importa y el más fácil de perder. En el original,
 * `KEYBOARD '{DNARROW}'` convertía Enter en "siguiente", porque así se digita
 * en serie: el usuario teclea un dato, Enter, otro dato, Enter. En la web Enter
 * ENVÍA el formulario por defecto, así que si no se intercepta, quien lleve
 * años usando el sistema va a guardar a medio llenar una y otra vez.
 *
 * Se conserva el comportamiento original y se deja Ctrl+Enter como la forma
 * explícita de confirmar, que también es la del original.
 *
 *     <form appTeclas (confirmar)="guardar()" (cancelar)="cerrar()">
 */
@Directive({
  selector: '[appTeclas]',
  standalone: true,
})
export class TeclasDirective {
  /** Poner en false donde Enter sí deba enviar (un buscador, por ejemplo). */
  @Input() enterAvanza = true;

  @Output() confirmar = new EventEmitter<void>();
  @Output() cancelar = new EventEmitter<void>();

  @HostListener('keydown', ['$event'])
  alPulsar(evento: KeyboardEvent): void {
    if (evento.key === 'Escape') {
      evento.preventDefault();
      // El original PREGUNTA antes de cerrar («¿Está seguro de cerrar el
      // formulario?»), y solo entonces deshace y sale. No es un adorno: Esc
      // está al lado de las teclas de uso normal y sin confirmación se pierde
      // una historia a medio escribir. Quien escuche `cancelar` debe confirmar.
      this.cancelar.emit();
      return;
    }

    if (evento.key === 'Enter' && (evento.ctrlKey || evento.metaKey)) {
      evento.preventDefault();
      this.confirmar.emit();
      return;
    }

    if (evento.key === 'Enter' && this.enterAvanza) {
      const destino = evento.target as HTMLElement;
      // En un textarea Enter es un salto de línea legítimo, y en un botón es
      // pulsarlo: ahí no se toca.
      if (destino instanceof HTMLTextAreaElement || destino instanceof HTMLButtonElement) {
        return;
      }
      evento.preventDefault();
      this.avanzarFoco(destino);
    }
  }

  /** Mueve el foco al siguiente control enfocable, como el `{DNARROW}` original. */
  private avanzarFoco(desde: HTMLElement): void {
    const formulario = desde.closest('form, [appTeclas]');
    if (!formulario) {
      return;
    }
    const enfocables = Array.from(
      formulario.querySelectorAll<HTMLElement>(
        'input:not([disabled]):not([type=hidden]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ),
    ).filter((e) => e.offsetParent !== null);

    const i = enfocables.indexOf(desde);
    if (i >= 0 && i < enfocables.length - 1) {
      enfocables[i + 1].focus();
    }
  }
}
