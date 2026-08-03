import { Component, Input } from '@angular/core';

/**
 * El aviso EN LA PÁGINA —lo que en PrimeNG era `<p-message>` (7 usos)—, que no
 * es lo mismo que un [`Avisos`]: aquel se va solo a los pocos segundos, este se
 * queda mientras la condición dure. Los siete sitios donde se usa dicen cosas
 * como «este consentimiento está revocado», que no puede desaparecer sola.
 *
 * Material no lo trae: tiene `mat-error`, atado a un `mat-form-field`, y las
 * barras de la Snackbar, que son temporales. Se hace propio (directiva 26).
 *
 * Admite contenido proyectado, porque los usos actuales meten texto con
 * `<strong>` dentro.
 */
@Component({
  selector: 'x-mensaje',
  standalone: true,
  template: `
    <div class="x-mensaje" [class]="'tono-' + severity">
      @if (icono()) {
        <i [class]="icono()"></i>
      }
      <div class="cuerpo">
        @if (text) {
          {{ text }}
        }
        <ng-content />
      </div>
    </div>
  `,
  styles: [
    `
      .x-mensaje {
        display: flex;
        gap: 0.6rem;
        align-items: flex-start;
        padding: 0.75rem 1rem;
        border-radius: 8px;
        border-left: 4px solid;
        font-size: 0.875rem;
        line-height: 1.5;
      }
      .cuerpo {
        flex: 1;
      }
      .tono-success {
        background: #f0fdf4;
        border-color: #16a34a;
        color: #14532d;
      }
      .tono-info {
        background: #eff6ff;
        border-color: #2563eb;
        color: #1e3a8a;
      }
      .tono-warn {
        background: #fffbeb;
        border-color: #d97706;
        color: #78350f;
      }
      .tono-error {
        background: #fef2f2;
        border-color: #dc2626;
        color: #7f1d1d;
      }
    `,
  ],
})
export class MensajeComponent {
  @Input() severity: 'success' | 'info' | 'warn' | 'error' = 'info';
  /** Texto simple. Para contenido con marcado, se proyecta como hijo. */
  @Input() text?: string;

  icono(): string {
    return {
      success: 'pi pi-check-circle',
      info: 'pi pi-info-circle',
      warn: 'pi pi-exclamation-triangle',
      error: 'pi pi-times-circle',
    }[this.severity];
  }
}
