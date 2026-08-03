import { Component, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';

/**
 * A dónde llega quien intenta entrar a un módulo que no puede usar.
 *
 * Distingue los dos motivos, porque llevan a acciones distintas: si está
 * contratado y aún no existe, el usuario espera; si no está en su plan, hay
 * que hablar con quien contrata. Un único "no disponible" para ambos casos
 * manda a todo el mundo a soporte.
 */
@Component({
  selector: 'app-modulo-no-disponible',
  standalone: true,
  imports: [CommonModule, RouterLink],
  template: `
    <div class="aviso">
      <i class="pi" [class.pi-clock]="pendiente" [class.pi-lock]="!pendiente"></i>

      @if (pendiente) {
        <h2>Módulo en construcción</h2>
        <p>
          <strong>{{ nombre }}</strong> está incluido en el plan de su institución,
          pero todavía no está disponible.
        </p>
        <p class="detalle">No se ha perdido información: el módulo aún no existe.</p>
      } @else {
        <h2>Módulo no incluido</h2>
        <p>
          <strong>{{ nombre }}</strong> no hace parte del plan contratado por su
          institución.
        </p>
        <p class="detalle">
          Si necesita este módulo, consúltelo con el administrador de su institución.
        </p>
      }

      <a routerLink="/dashboard" class="volver">Volver al inicio</a>
    </div>
  `,
  styles: [`
    .aviso {
      max-width: 460px; margin: 4rem auto; padding: 2.5rem 2rem;
      text-align: center; background: white; border-radius: 12px;
      border: 1px solid #e2e8f0;
    }
    .aviso i { font-size: 2.5rem; color: #94a3b8; margin-bottom: 1rem; display: block; }
    h2 { margin: 0 0 0.75rem; font-size: 1.25rem; color: #1e293b; }
    p { color: #475569; margin: 0 0 0.5rem; }
    .detalle { font-size: 0.875rem; color: #64748b; }
    .volver {
      display: inline-block; margin-top: 1.5rem; padding: 0.55rem 1.25rem;
      background: #3b82f6; color: white; border-radius: 6px; text-decoration: none;
      font-size: 0.875rem;
    }
  `],
})
export class ModuloNoDisponibleComponent {
  private ruta = inject(ActivatedRoute);

  readonly nombre = this.legible(this.ruta.snapshot.queryParamMap.get('modulo'));
  readonly pendiente = this.ruta.snapshot.queryParamMap.get('estado') === 'pendiente';

  /** BANCO_SANGRE -> "Banco sangre" */
  private legible(modulo: string | null): string {
    if (!modulo) return 'El módulo solicitado';
    const texto = modulo.replace(/_/g, ' ').toLowerCase();
    return texto.charAt(0).toUpperCase() + texto.slice(1);
  }
}
