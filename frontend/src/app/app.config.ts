import { ApplicationConfig, provideBrowserGlobalErrorListeners, provideZoneChangeDetection } from '@angular/core';
import { provideRouter, withComponentInputBinding } from '@angular/router';
import { provideHttpClient, withInterceptors, withXhr } from '@angular/common/http';
import { provideAnimationsAsync } from '@angular/platform-browser/animations/async';
import { MAT_DATE_LOCALE, provideNativeDateAdapter } from '@angular/material/core';
import { routes } from './app.routes';
import { authInterceptor } from './core/auth.interceptor';

export const appConfig: ApplicationConfig = {
  providers: [
    provideBrowserGlobalErrorListeners(),
    provideZoneChangeDetection({ eventCoalescing: true }),
    provideHttpClient(withXhr(), withInterceptors([authInterceptor])),
    provideAnimationsAsync(),
    provideRouter(routes, withComponentInputBinding()),
    // El calendario de Material, una sola vez para las cinco pantallas que lo
    // usan. El locale importa: sin él escribe las fechas en el formato de
    // Estados Unidos —mes/día/año—, y «03/04» leído al revés en una historia
    // clínica es un mes de diferencia.
    provideNativeDateAdapter(),
    { provide: MAT_DATE_LOCALE, useValue: 'es-CO' },
    // Ni `MessageService` ni `ConfirmationService`: `Avisos` y `Confirmar` son
    // `providedIn: 'root'`, así que no se declaran aquí ni en cada pantalla
    // —eran 21 `providers: [MessageService]` locales, uno por componente—.
    // `providePrimeNG` se fue con la última pantalla que usaba PrimeNG
    // (2026-08-02). De la librería solo queda `primeicons`, que es una fuente
    // de iconos MIT y no tiene el problema de licencia de `primeng`.
  ],
};
