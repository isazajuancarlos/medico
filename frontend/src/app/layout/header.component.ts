import { Component, inject } from '@angular/core';
import { Router } from '@angular/router';
import { CommonModule } from '@angular/common';
import { AuthService } from '../core/auth.service';
import { ButtonModule } from 'primeng/button';
import { AvatarModule } from 'primeng/avatar';
import { TooltipModule } from 'primeng/tooltip';

@Component({
  selector: 'app-header',
  standalone: true,
  imports: [CommonModule, ButtonModule, AvatarModule, TooltipModule],
  template: `
    <div class="header-left">
      <span class="page-breadcrumb">MED-SYS Pro</span>
    </div>
    <div class="header-right">
      @if (user()) {
        <div class="user-info">
          <p-avatar
            [label]="userInitial()"
            shape="circle"
            styleClass="mr-2"
          />
          <div class="user-details">
            <span class="user-name">{{ user()!.nombre_completo }}</span>
            <span class="user-role">{{ user()!.rol }}</span>
          </div>
        </div>
        <p-button
          icon="pi pi-sign-out"
          [text]="true"
          severity="secondary"
          (onClick)="logout()"
          pTooltip="Cerrar sesión"
        />
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
    .user-details { display: flex; flex-direction: column; }
    .user-name { font-size: 0.875rem; font-weight: 600; color: #1e293b; }
    .user-role { font-size: 0.75rem; color: #64748b; text-transform: capitalize; }
  `]
})
export class HeaderComponent {
  private authService = inject(AuthService);
  private router = inject(Router);

  readonly user = this.authService.currentUser;

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
}
