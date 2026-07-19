import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';

import { ModulosService } from './modulos.service';

/**
 * Impide entrar a un módulo que la institución no contrató.
 *
 * Ocultarlo del menú no basta: la ruta se alcanza escribiendo la URL. El
 * backend ya responde 403, así que sin este guardián el usuario llegaría a una
 * pantalla que falla en pedazos en vez de a un mensaje claro.
 *
 * Uso en app.routes.ts:
 *   { path: 'quirofano', canActivate: [moduloGuard('QUIROFANO')], ... }
 */
export function moduloGuard(modulo: string): CanActivateFn {
  return () => {
    const modulos = inject(ModulosService);
    const router = inject(Router);

    const estado = modulos.estado(modulo);
    if (estado === 'habilitado') return true;

    // 'pendiente' y 'no_incluido' llevan al mismo sitio pero con distinto
    // motivo: uno se contrató y aún no existe, el otro no está en el plan.
    return router.createUrlTree(['/modulo-no-disponible'], {
      queryParams: { modulo, estado },
    });
  };
}
