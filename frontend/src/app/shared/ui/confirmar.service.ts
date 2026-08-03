import { Component, Inject, Injectable, inject } from '@angular/core';
import { MatButtonModule } from '@angular/material/button';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';

/** Lo que se pregunta. `message` admite HTML porque las que ya existen lo usan. */
export interface Confirmacion {
  header: string;
  message: string;
  icon?: string;
  acceptLabel?: string;
  rejectLabel?: string;
  /** Pinta el botón de aceptar como destructivo. */
  peligroso?: boolean;
  accept: () => void;
  reject?: () => void;
}

@Component({
  selector: 'x-confirmar-dialogo',
  standalone: true,
  imports: [MatDialogModule, MatButtonModule],
  template: `
    <h2 mat-dialog-title>
      @if (datos.icon) {
        <i [class]="datos.icon" style="margin-right:.5rem"></i>
      }
      {{ datos.header }}
    </h2>
    <mat-dialog-content>
      <!-- OJO: "message" SÍ trae dato del servidor. Nueve de los doce sitios que
           preguntan interpolan algo que no escribimos nosotros —el nombre del
           paciente, el del medicamento, y en historias el nombre de archivo que
           subió el cliente—, así que la frase que había aquí antes («el texto lo
           escribe la aplicación, nunca el usuario ni el servidor») era falsa. Se
           conserva el innerHTML porque las preguntas ya existentes llevan
           <strong> y esto viene de p-confirmDialog, que hacía lo mismo; lo que
           lo hace seguro es que Angular sanea la vinculación.

           NO ENVOLVER ESTO EN bypassSecurityTrustHtml. Es la única pieza del
           proyecto que pinta HTML, y el saneado es lo único que la sostiene:
           quitarlo convierte el nombre de un archivo subido en XSS almacenado.
           Si algún día hace falta marcado que el saneador se come, la salida es
           partir el campo en texto llano + marcado constante, no confiar. -->
      <div [innerHTML]="datos.message"></div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton (click)="cerrar(false)">{{ datos.rejectLabel ?? 'Cancelar' }}</button>
      <button
        matButton="filled"
        [style.background]="datos.peligroso ? '#dc2626' : null"
        [style.color]="datos.peligroso ? '#fff' : null"
        (click)="cerrar(true)"
      >
        {{ datos.acceptLabel ?? 'Aceptar' }}
      </button>
    </mat-dialog-actions>
  `,
})
export class ConfirmarDialogoComponent {
  private ref = inject<MatDialogRef<ConfirmarDialogoComponent, boolean>>(MatDialogRef);
  constructor(@Inject(MAT_DIALOG_DATA) public datos: Confirmacion) {}
  cerrar(v: boolean) {
    this.ref.close(v);
  }
}

/**
 * La confirmación de una acción — lo que en PrimeNG eran `ConfirmationService` y
 * `<p-confirmDialog>`, aquí sobre `MatDialog`.
 *
 * Conserva la firma `confirm({ header, message, accept })` por la misma razón
 * que [`Avisos`]: las nueve pantallas que la usan no se reescriben en el mismo
 * diff en que se cambia de librería.
 *
 * **El diálogo no se puede cerrar por fuera** (`disableClose`). Las nueve
 * preguntas que existen son destructivas —anular una historia, cerrar la sesión
 * en todos los dispositivos—: un clic fuera del cuadro no es una respuesta, y
 * dejar que valga como «cancelar» está bien hasta que un día valga como otra
 * cosa. Aquí se responde con un botón.
 */
@Injectable({ providedIn: 'root' })
export class Confirmar {
  private dialogo = inject(MatDialog);

  confirm(c: Confirmacion): void {
    this.dialogo
      .open(ConfirmarDialogoComponent, {
        data: c,
        width: '440px',
        disableClose: true,
        autoFocus: 'dialog',
      })
      .afterClosed()
      .subscribe((acepto) => (acepto ? c.accept() : c.reject?.()));
  }
}
