import { inject } from '@angular/core';
import { CanActivateFn, ActivatedRouteSnapshot, Router } from '@angular/router';
import { AuthService } from './auth.service';
import { RolUsuario } from '../models/auth.models';

export const roleGuard: CanActivateFn = (route: ActivatedRouteSnapshot) => {
  const authService = inject(AuthService);
  const router = inject(Router);
  const allowedRoles = route.data['roles'] as RolUsuario[];

  if (!allowedRoles || allowedRoles.length === 0) return true;

  if (authService.hasRole(...allowedRoles)) return true;
  return router.createUrlTree(['/dashboard']);
};
