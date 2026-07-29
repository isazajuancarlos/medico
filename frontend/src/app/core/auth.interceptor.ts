import { HttpInterceptorFn, HttpErrorResponse } from '@angular/common/http';
import { inject } from '@angular/core';
import { catchError, throwError } from 'rxjs';
import { AuthService } from './auth.service';
import { Router } from '@angular/router';

/**
 * La sesión es una cookie: viaja sola con `withCredentials`, no hay token que
 * adjuntar. Ante un 401 (que no sea el propio login), se limpia la sesión local
 * y se manda a `/login` — sin `refresh`, que el `medico-api` no ofrece.
 */
export const authInterceptor: HttpInterceptorFn = (req, next) => {
  const authService = inject(AuthService);
  const router = inject(Router);

  const conCookie = req.clone({ withCredentials: true });

  return next(conCookie).pipe(
    catchError((error: HttpErrorResponse) => {
      const esLogin = req.url.includes('/api/acceso') || req.url.includes('/api/yo');
      if (error.status === 401 && !esLogin) {
        authService.cerrarLocal();
        router.navigate(['/login']);
      }
      return throwError(() => error);
    }),
  );
};
