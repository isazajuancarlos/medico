import { ApplicationConfig, provideBrowserGlobalErrorListeners, provideZoneChangeDetection } from '@angular/core';
import { provideRouter, withComponentInputBinding } from '@angular/router';
import { provideHttpClient, withInterceptors } from '@angular/common/http';
import { provideAnimationsAsync } from '@angular/platform-browser/animations/async';
import { MAT_DATE_LOCALE, provideNativeDateAdapter } from '@angular/material/core';
import { providePrimeNG } from 'primeng/config';
import Aura from '@primeuix/themes/aura';
import { routes } from './app.routes';
import { authInterceptor } from './core/auth.interceptor';

export const appConfig: ApplicationConfig = {
  providers: [
    provideBrowserGlobalErrorListeners(),
    provideZoneChangeDetection({ eventCoalescing: true }),
    provideHttpClient(withInterceptors([authInterceptor])),
    provideAnimationsAsync(),
    provideRouter(routes, withComponentInputBinding()),
    // El calendario de Material, una sola vez para las cinco pantallas que lo
    // usan. El locale importa: sin él escribe las fechas en el formato de
    // Estados Unidos —mes/día/año—, y «03/04» leído al revés en una historia
    // clínica es un mes de diferencia.
    provideNativeDateAdapter(),
    { provide: MAT_DATE_LOCALE, useValue: 'es-CO' },
    providePrimeNG({
      theme: { preset: Aura, options: { darkModeSelector: '.p-dark' } },
      ripple: true,
    }),
    // Ni `MessageService` ni `ConfirmationService`: `Avisos` y `Confirmar` son
    // `providedIn: 'root'`, así que no se declaran aquí ni en cada pantalla
    // —eran 21 `providers: [MessageService]` locales, uno por componente—.
    // `providePrimeNG` se queda mientras alguna pantalla siga usando PrimeNG;
    // se va con la última.
  ],
};
