import { Component, inject, signal } from '@angular/core';
import { Router } from '@angular/router';
import { CommonModule } from '@angular/common';
import { AuthService } from '../core/auth.service';
import { MatButtonModule } from '@angular/material/button';
import { MatTooltipModule } from '@angular/material/tooltip';
import { Avisos } from '../shared/ui/avisos.service';
import { Confirmar } from '../shared/ui/confirmar.service';

/**
 * La cabecera, con las DOS formas de cerrar sesión.
 *
 * Son acciones distintas y por eso son dos botones y no un icono con sorpresa:
 *
 * - **Cerrar sesión** borra la cookie de este navegador. Es reversible en un
 *   segundo y no afecta a nadie más.
 * - **Cerrar en todos los dispositivos** sube el `token_version` del usuario en
 *   el servidor, y con eso caduca toda cookie ya emitida — el móvil, el
 *   consultorio, y el equipo de quien se hubiera quedado dentro. Se pide
 *   confirmación porque cierra sesiones que no están a la vista.
 */
@Component({
  selector: 'app-header',
  standalone: true,
  imports: [CommonModule, MatButtonModule, MatTooltipModule],
  template: `
    <div class="header-left">
      <span class="page-breadcrumb">MED-SYS Pro</span>
    </div>
    <div class="header-right">
      @if (user()) {
        <div class="user-info">
          <!-- El avatar de PrimeNG era una inicial dentro de un círculo.
               Material no trae ese componente y traerlo entero por esto sería
               desproporcionado: es un span. -->
          <span class="avatar" aria-hidden="true">{{ userInitial() }}</span>
          <div class="user-details">
            <span class="user-name">{{ user()!.nombre_completo }}</span>
            <span class="user-role">{{ user()!.rol }}</span>
          </div>
        </div>
        <!-- Material no tiene el input de carga que tenía p-button, así que va a
             mano: el icono pasa a girar y el botón queda inerte. Las dos cosas,
             no solo la segunda — un botón que se apaga sin decir por qué se lee
             como averiado, y este cierra sesiones que no están a la vista. -->
        <button
          matIconButton
          [disabled]="cerrandoTodo()"
          (click)="confirmarCerrarTodo()"
          matTooltip="Cerrar sesión en todos los dispositivos"
          matTooltipPosition="below"
          aria-label="Cerrar sesión en todos los dispositivos"
        >
          <i [class]="cerrandoTodo() ? 'pi pi-spin pi-spinner' : 'pi pi-lock'"></i>
        </button>
        <button
          matIconButton
          (click)="logout()"
          matTooltip="Cerrar sesión"
          matTooltipPosition="below"
          aria-label="Cerrar sesión"
        >
          <i class="pi pi-sign-out"></i>
        </button>
      }
    </div>
  `,
  styles: [`
    :host {
      display: flex; align-items: center; justify-content: space-between;
      width: 100%; padding: 0 1.5rem;
    }
    .header-left .page-breadcrumb { font-weight: 600; color: #1e293b; }
    .header-right { display: flex; align-items: center; gap: 1rem; }
    .user-info { display: flex; align-items: center; gap: 0.75rem; }
    .avatar {
      display: inline-flex; align-items: center; justify-content: center;
      width: 2rem; height: 2rem; border-radius: 50%;
      background: #e2e8f0; color: #1e293b;
      font-size: 0.875rem; font-weight: 600;
    }
    .user-details { display: flex; flex-direction: column; }
    .user-name { font-size: 0.875rem; font-weight: 600; color: #1e293b; }
    .user-role { font-size: 0.75rem; color: #64748b; text-transform: capitalize; }
  `]
})
export class HeaderComponent {
  private authService = inject(AuthService);
  private router = inject(Router);
  private confirmationService = inject(Confirmar);
  private messageService = inject(Avisos);

  readonly user = this.authService.currentUser;
  readonly cerrandoTodo = signal(false);

  userInitial(): string {
    const u = this.user();
    return u ? u.nombre_completo.charAt(0).toUpperCase() : '?';
  }

  logout(): void {
    this.authService.logout().subscribe({
      next: () => this.router.navigate(['/login']),
      error: () => this.router.navigate(['/login']),
    });
  }

  confirmarCerrarTodo(): void {
    this.confirmationService.confirm({
      header: 'Cerrar sesión en todos los dispositivos',
      // Dice lo que va a pasar, incluido lo que le pasa a QUIEN lo pide: sin esa
      // frase, quien lo pulsa desde el consultorio no espera que le cierre esta
      // pantalla también.
      message:
        'Se cerrará la sesión en <strong>todos</strong> los dispositivos donde ' +
        'haya entrado con esta cuenta, incluido este. Tendrá que volver a entrar ' +
        'con su contraseña.<br><br>Úselo si cree que alguien más tiene acceso a ' +
        'su cuenta.',
      icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Cerrar en todos',
      rejectLabel: 'Cancelar',
      peligroso: true,
      accept: () => this.cerrarTodo(),
    });
  }

  private cerrarTodo(): void {
    this.cerrandoTodo.set(true);
    this.authService.logoutTodos().subscribe({
      next: () => {
        this.cerrandoTodo.set(false);
        this.router.navigate(['/login']);
      },
      // **No se navega al login.** El servidor responde 500 cuando la revocación
      // no ocurrió, y él mismo se niega a fingir un cierre que no hizo; mandar
      // aquí al login lo desharía — la pantalla diría «listo» mientras las otras
      // sesiones siguen abiertas. Quien pide esto suele sospechar que alguien
      // más está dentro, así que un falso «hecho» es peor que el error.
      //
      // El 401 no llega hasta aquí: lo atrapa el interceptor, que limpia y
      // redirige, y con la cookie ya inválida esta sesión está cerrada igual.
      error: () => {
        this.cerrandoTodo.set(false);
        this.messageService.add({
          severity: 'error',
          summary: 'No se cerraron las sesiones',
          detail:
            'El servidor no pudo revocarlas, así que las otras sesiones siguen ' +
            'abiertas. Vuelva a intentarlo; si sigue fallando, cambie la contraseña.',
          life: 10000,
        });
      },
    });
  }
}
