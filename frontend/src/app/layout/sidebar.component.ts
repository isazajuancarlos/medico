import { Component, inject } from '@angular/core';
import { RouterLink, RouterLinkActive } from '@angular/router';
import { CommonModule } from '@angular/common';
import { AuthService } from '../core/auth.service';
import { RolUsuario } from '../models/auth.models';

interface MenuItem {
  label: string;
  icon: string;
  route: string;
  roles?: RolUsuario[];
}

interface MenuSection {
  label: string;
  items: MenuItem[];
  roles?: RolUsuario[];
}

@Component({
  selector: 'app-sidebar',
  standalone: true,
  imports: [RouterLink, RouterLinkActive, CommonModule],
  template: `
    <div class="sidebar-logo">
      <div class="logo-icon">M</div>
      <div class="logo-text">
        <strong>MED-SYS Pro</strong>
        <small>Sistema EMR</small>
      </div>
    </div>

    <nav class="sidebar-nav">
      @for (section of visibleSections; track section.label) {
        <div class="nav-section">
          <div class="nav-section-title">{{ section.label }}</div>
          @for (item of visibleItems(section.items); track item.route) {
            <a [routerLink]="item.route" routerLinkActive="active" class="nav-item">
              <i [class]="'pi ' + item.icon"></i>
              <span>{{ item.label }}</span>
            </a>
          }
        </div>
      }
    </nav>
  `,
  styles: [`
    :host { display: block; height: 100%; background: #1e293b; color: white; }
    .sidebar-logo {
      display: flex; align-items: center; gap: 0.75rem;
      padding: 1.25rem 1rem; border-bottom: 1px solid rgba(255,255,255,0.1);
    }
    .logo-icon {
      width: 36px; height: 36px; background: #3b82f6; border-radius: 8px;
      display: flex; align-items: center; justify-content: center;
      font-weight: 700; font-size: 1.1rem;
    }
    .logo-text strong { display: block; font-size: 0.95rem; }
    .logo-text small { font-size: 0.7rem; color: rgba(255,255,255,0.5); }
    .sidebar-nav { padding: 0.5rem 0; }
    .nav-section { margin-bottom: 0.5rem; }
    .nav-section-title {
      padding: 0.5rem 1rem 0.25rem;
      font-size: 0.65rem; font-weight: 600; letter-spacing: 0.05em;
      color: rgba(255,255,255,0.4); text-transform: uppercase;
    }
    .nav-item {
      display: flex; align-items: center; gap: 0.75rem;
      padding: 0.6rem 1rem; color: rgba(255,255,255,0.75);
      text-decoration: none; font-size: 0.875rem;
      border-left: 3px solid transparent; transition: all 0.15s;
    }
    .nav-item:hover { background: rgba(255,255,255,0.05); color: white; }
    .nav-item.active { background: rgba(59,130,246,0.15); color: #93c5fd; border-left-color: #3b82f6; }
    .nav-item i { font-size: 0.9rem; width: 16px; text-align: center; }
  `]
})
export class SidebarComponent {
  private authService = inject(AuthService);

  private readonly allSections: MenuSection[] = [
    {
      label: 'Principal',
      items: [
        { label: 'Dashboard', icon: 'pi-home', route: '/dashboard' },
        { label: 'Pacientes', icon: 'pi-users', route: '/pacientes', roles: ['admin', 'medico', 'superadmin', 'enfermeria'] },
        { label: 'Citas', icon: 'pi-calendar', route: '/citas', roles: ['admin', 'medico', 'superadmin', 'enfermeria'] },
      ]
    },
    {
      label: 'Clínico',
      roles: ['admin', 'medico', 'superadmin', 'enfermeria', 'anestesiologo'],
      items: [
        { label: 'Historia Clínica', icon: 'pi-file-edit', route: '/historias', roles: ['admin', 'medico', 'superadmin'] },
        { label: 'Urgencias', icon: 'pi-exclamation-circle', route: '/urgencias' },
        { label: 'Hospitalización', icon: 'pi-building', route: '/hospitalizacion' },
        { label: 'Evoluciones', icon: 'pi-list', route: '/evoluciones' },
        { label: 'Interconsultas', icon: 'pi-share-alt', route: '/interconsultas' },
        { label: 'Epicrisis', icon: 'pi-file', route: '/epicrisis' },
      ]
    },
    {
      label: 'Especialidades',
      items: [
        { label: 'Laboratorio', icon: 'pi-chart-scatter', route: '/laboratorio', roles: ['laboratorista', 'bacteriologo', 'admin', 'medico', 'superadmin'] },
        { label: 'Imágenes', icon: 'pi-image', route: '/imagenes', roles: ['radiologo', 'admin', 'medico', 'superadmin'] },
        { label: 'Nutrición', icon: 'pi-heart', route: '/nutricion', roles: ['nutricionista', 'admin', 'superadmin'] },
        { label: 'Rehabilitación', icon: 'pi-refresh', route: '/rehabilitacion', roles: ['terapeuta', 'admin', 'superadmin'] },
        { label: 'Quirófano', icon: 'pi-wrench', route: '/quirofano', roles: ['anestesiologo', 'medico', 'admin', 'superadmin'] },
      ]
    },
    {
      label: 'Administrativo',
      items: [
        { label: 'Farmacia', icon: 'pi-box', route: '/farmacia', roles: ['regente_farmacia', 'admin', 'superadmin'] },
        { label: 'Facturación', icon: 'pi-money-bill', route: '/facturacion', roles: ['admin', 'superadmin'] },
        { label: 'RIPS', icon: 'pi-download', route: '/rips', roles: ['admin', 'superadmin'] },
        { label: 'Estadísticas', icon: 'pi-chart-bar', route: '/estadisticas', roles: ['admin', 'superadmin', 'medico'] },
      ]
    },
    {
      label: 'Configuración',
      roles: ['admin', 'superadmin'],
      items: [
        { label: 'Administración', icon: 'pi-cog', route: '/admin', roles: ['admin', 'superadmin'] },
        { label: 'SuperAdmin', icon: 'pi-shield', route: '/superadmin', roles: ['superadmin'] },
      ]
    }
  ];

  get visibleSections(): MenuSection[] {
    return this.allSections.filter(s => {
      if (!s.roles) return true;
      return this.authService.hasRole(...(s.roles as RolUsuario[]));
    });
  }

  visibleItems(items: MenuItem[]): MenuItem[] {
    return items.filter(item => {
      if (!item.roles) return true;
      return this.authService.hasRole(...(item.roles as RolUsuario[]));
    });
  }
}
