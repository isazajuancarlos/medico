import { Component, inject } from '@angular/core';
import { FormBuilder, ReactiveFormsModule, Validators } from '@angular/forms';
import { Router } from '@angular/router';
import { CommonModule } from '@angular/common';
import { AuthService } from '../../core/auth.service';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { PasswordModule } from 'primeng/password';
import { CardModule } from 'primeng/card';
import { MessageModule } from 'primeng/message';
@Component({
  selector: 'app-login',
  standalone: true,
  imports: [
    CommonModule, ReactiveFormsModule, ButtonModule, InputTextModule,
    PasswordModule, CardModule, MessageModule,
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
          <div class="field">
            <label for="username">Usuario</label>
            <input
              id="username"
              pInputText
              formControlName="username"
              placeholder="Ingrese su usuario"
              class="w-full"
            />
          </div>

          <div class="field">
            <label for="password">Contraseña</label>
            <p-password
              id="password"
              formControlName="password"
              placeholder="Ingrese su contraseña"
              [feedback]="false"
              [toggleMask]="true"
              styleClass="w-full"
            />
          </div>

          @if (errorMessage) {
            <p-message severity="error" [text]="errorMessage" styleClass="w-full mb-3" />
          }

          <p-button
            type="submit"
            label="Iniciar Sesión"
            icon="pi pi-sign-in"
            [loading]="loading"
            [disabled]="form.invalid"
            styleClass="w-full"
          />
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
    .field { margin-bottom: 1.25rem; }
    .field label { display: block; font-weight: 500; margin-bottom: 0.4rem; color: #374151; font-size: 0.875rem; }
    ::ng-deep .field input, ::ng-deep .field .p-password { width: 100%; }
  `]
})
export class LoginComponent {
  private fb = inject(FormBuilder);
  private authService = inject(AuthService);
  private router = inject(Router);

  form = this.fb.group({
    username: ['', [Validators.required]],
    password: ['', [Validators.required]],
  });

  loading = false;
  errorMessage = '';

  onSubmit(): void {
    if (this.form.invalid) return;
    this.loading = true;
    this.errorMessage = '';

    const { username, password } = this.form.value;
    this.authService.login({ username: username!, password: password! }).subscribe({
      next: () => {
        this.router.navigate([this.authService.getDefaultRoute()]);
      },
      error: (err: { error?: { detail?: string } }) => {
        this.loading = false;
        this.errorMessage = err.error?.detail || 'Error al iniciar sesión. Verifique sus credenciales.';
      },
    });
  }
}
