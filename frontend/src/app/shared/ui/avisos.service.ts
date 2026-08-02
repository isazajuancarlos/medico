import { Injectable, inject } from '@angular/core';
import { MatSnackBar } from '@angular/material/snack-bar';

export type Severidad = 'success' | 'info' | 'warn' | 'error';

/** Lo que se muestra: `summary` es el titular y `detail` la explicación. */
export interface Aviso {
  severity: Severidad;
  summary: string;
  detail?: string;
}

/**
 * Los avisos de la aplicación — lo que en PrimeNG eran `MessageService` y
 * `<p-toast>`, aquí sobre `MatSnackBar`.
 *
 * **Conserva la firma `add({ severity, summary, detail })` a propósito**, y no
 * por comodidad: son 80 llamadas en 21 pantallas. Cambiar de librería y de
 * vocabulario en el mismo diff mezcla dos cambios, y si algo se rompe no se
 * sabe cuál fue. Renombrar a `exito`/`error` es una pasada aparte, mecánica,
 * cuando esta esté verde. El servicio es propio: PrimeNG no queda debajo.
 *
 * Un error dura el doble y trae botón de cerrar. Un aviso de error que se va
 * solo a los tres segundos es un aviso que el usuario no llegó a leer, y en una
 * pantalla clínica eso significa creer que algo se guardó.
 */
@Injectable({ providedIn: 'root' })
export class Avisos {
  private barra = inject(MatSnackBar);

  add(aviso: Aviso): void {
    const grave = aviso.severity === 'error';
    const texto = aviso.detail ? `${aviso.summary}: ${aviso.detail}` : aviso.summary;
    this.barra.open(texto, grave ? 'Cerrar' : '', {
      duration: grave ? 8000 : 4000,
      horizontalPosition: 'right',
      verticalPosition: 'top',
      panelClass: ['x-aviso', `x-aviso-${aviso.severity}`],
    });
  }

  exito(summary: string, detail?: string) {
    this.add({ severity: 'success', summary, detail });
  }
  info(summary: string, detail?: string) {
    this.add({ severity: 'info', summary, detail });
  }
  advertencia(summary: string, detail?: string) {
    this.add({ severity: 'warn', summary, detail });
  }
  error(summary: string, detail?: string) {
    this.add({ severity: 'error', summary, detail });
  }
}
