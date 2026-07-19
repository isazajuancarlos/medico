import { Directive, ElementRef, HostListener, inject, Input } from '@angular/core';

/**
 * Clic derecho sobre un campo o etiqueta: amplía su contenido.
 *
 * Es el comportamiento MÁS EXTENDIDO del sistema original: 828 usos en 31 de
 * los 37 formularios, invocando `medico1.zoomx(...)`. Está en casi todos los
 * controles, incluidas las etiquetas.
 *
 * No es un adorno. En un consultorio se lee la pantalla a distancia, con luz
 * mala y a veces con presbicia, y un código CIE-10 o una cédula mal leídos
 * tienen consecuencias clínicas. Quitarlo "porque es raro" sería la clase de
 * pérdida silenciosa que hace que un sistema migrado se sienta peor que el
 * viejo aunque tenga más funciones.
 *
 * Se conserva el gesto original —clic derecho— porque está en la costumbre del
 * usuario, y se añade una alternativa accesible por teclado, que el original no
 * tenía.
 *
 *     <span appZoom>{{ paciente.identificacion }}</span>
 *     <input appZoom [zoomTexto]="historia.diagnostico" />
 */
@Directive({
  selector: '[appZoom]',
  standalone: true,
})
export class ZoomDirective {
  /** Texto a ampliar. Si no se da, se toma del propio elemento. */
  @Input() zoomTexto?: string;

  private readonly el = inject(ElementRef<HTMLElement>);

  @HostListener('contextmenu', ['$event'])
  alClicDerecho(evento: MouseEvent): void {
    evento.preventDefault();
    this.ampliar();
  }

  /** Alternativa por teclado: el original solo tenía el gesto de ratón. */
  @HostListener('keydown', ['$event'])
  alTeclado(evento: KeyboardEvent): void {
    if (evento.key === 'F2') {
      evento.preventDefault();
      this.ampliar();
    }
  }

  private textoActual(): string {
    if (this.zoomTexto !== undefined) {
      return this.zoomTexto;
    }
    const nodo = this.el.nativeElement as HTMLElement & { value?: string };
    return (nodo.value ?? nodo.textContent ?? '').trim();
  }

  private ampliar(): void {
    const texto = this.textoActual();
    if (!texto) {
      return;
    }
    document.dispatchEvent(
      new CustomEvent('medico:zoom', { detail: { texto } }),
    );
  }
}
