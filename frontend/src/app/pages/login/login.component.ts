import { Component, inject, signal } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { Router } from '@angular/router';
import { CommonModule } from '@angular/common';
import { AuthService } from '../../core/auth.service';
import { MatButtonModule } from '@angular/material/button';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatIconButton } from '@angular/material/button';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MensajeComponent } from '../../shared/ui/mensaje.component';
@Component({
  selector: 'app-login',
  standalone: true,
  imports: [
    CommonModule, ReactiveFormsModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatIconButton, MatProgressSpinnerModule, MensajeComponent,
  ],
  template: `
    <div class="login-wrapper">
      <div class="login-box">
        <div class="login-logo">
          <div class="logo-circle">M</div>
          <h1>MED-SYS Pro</h1>
          <p>Sistema de Gestión de Práctica Médica</p>
        </div>

        <form [formGroup]="form" (ngSubmit)="onSubmit()">
          <mat-form-field appearance="outline" class="w-full">
            <mat-label>Correo</mat-label>
            <input matInput id="email" type="email" formControlName="email"
                   placeholder="correo@institucion.co" autocomplete="username" />
          </mat-form-field>

          <mat-form-field appearance="outline" class="w-full">
            <mat-label>Contraseña</mat-label>
            <input matInput id="clave" [type]="verClave ? 'text' : 'password'"
                   formControlName="clave" placeholder="Ingrese su contraseña"
                   autocomplete="current-password" />
            <!-- El ojo lo traía p-password con toggleMask; Material no lo trae,
                 y sin él no hay forma de comprobar lo que se tecleó en un
                 teclado de consultorio. Son cuatro líneas. -->
            <button matIconButton matSuffix type="button" (click)="verClave = !verClave"
                    [attr.aria-label]="verClave ? 'Ocultar contraseña' : 'Mostrar contraseña'"
                    [attr.aria-pressed]="verClave">
              <i class="pi" [class.pi-eye]="!verClave" [class.pi-eye-slash]="verClave"></i>
            </button>
          </mat-form-field>

          @if (errorMessage()) {
            <x-mensaje severity="error" [text]="errorMessage()" />
          }

          <button matButton="filled" type="submit" class="w-full boton-entrar"
                  [disabled]="form.invalid || cargando()">
            @if (cargando()) {
              <mat-spinner diameter="18" />
            } @else {
              <i class="pi pi-sign-in"></i>
            }
            Iniciar Sesión
          </button>
        </form>
      </div>
    </div>
  `,
  styles: [`
    .login-wrapper {
      min-height: 100vh; background: linear-gradient(135deg, #1e293b 0%, #0f172a 100%);
      display: flex; align-items: center; justify-content: center; padding: 1rem;
    }
    .login-box {
      background: white; border-radius: 16px; padding: 2.5rem;
      width: 100%; max-width: 420px; box-shadow: 0 25px 50px rgba(0,0,0,0.3);
    }
    .login-logo { text-align: center; margin-bottom: 2rem; }
    .logo-circle {
      width: 60px; height: 60px; background: #3b82f6; border-radius: 16px;
      display: flex; align-items: center; justify-content: center;
      font-weight: 700; font-size: 1.5rem; color: white; margin: 0 auto 1rem;
    }
    .login-logo h1 { font-size: 1.5rem; font-weight: 700; color: #1e293b; margin: 0; }
    .login-logo p { color: #64748b; font-size: 0.875rem; margin: 0.25rem 0 0; }
    .w-full { width: 100%; }
    .boton-entrar { margin-top: 0.5rem; display: flex; gap: 0.5rem; align-items: center; justify-content: center; }
    x-mensaje { display: block; margin-bottom: 1rem; }
  `]
})
export class LoginComponent {
  private fb = inject(FormBuilder);
  private authService = inject(AuthService);
  private router = inject(Router);

  form = this.fb.group({
    email: ['', [Validators.required, Validators.email]],
    clave: ['', [Validators.required]],
  });

  /** Señales, no campos llanos: las escribe el `subscribe` del acceso, o sea
   *  DESPUÉS de que la detección de cambios haya pasado. Con OnPush un campo
   *  llano dejaría el mensaje de error sin pintar y el botón girando para
   *  siempre, sin que nada fallara. Lo vigila
   *  `estado-que-la-plantilla-lee-es-señal.spec.ts`. */
  readonly cargando = signal(false);
  readonly errorMessage = signal('');
  verClave = false;

  onSubmit(): void {
    if (this.form.invalid) return;
    this.cargando.set(true);
    this.errorMessage.set('');

    const { email, clave } = this.form.value;
    this.authService.login({ email: email!, clave: clave! }).subscribe({
      next: () => {
        this.router.navigate([this.authService.getDefaultRoute()]);
      },
      error: (err: { error?: { detail?: string } }) => {
        this.cargando.set(false);
        this.errorMessage.set(err.error?.detail || 'Error al iniciar sesión. Verifique sus credenciales.');
      },
    });
  }
}
