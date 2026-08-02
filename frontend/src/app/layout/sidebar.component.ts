import { Component, inject, ChangeDetectionStrategy } from '@angular/core';
import { RouterLink, RouterLinkActive } from '@angular/router';
import { CommonModule } from '@angular/common';
import { AuthService } from '../core/auth.service';
import { ModulosService } from '../core/modulos.service';
import { RolUsuario } from '../models/auth.models';

interface MenuItem {
  label: string;
  icon: string;
  route: string;
  roles?: RolUsuario[];
  /**
   * Módulo del catálogo que habilita esta opción. Sin él, la opción no está
   * sujeta a licencia (dashboard, administración) y se muestra siempre.
   */
  modulo?: string;
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
            @if (disponible(item)) {
              <a [routerLink]="item.route" routerLinkActive="active" class="nav-item">
                <i [class]="'pi ' + item.icon"></i>
                <span>{{ item.label }}</span>
              </a>
            } @else {
              <!-- Contratado pero aún sin construir. Se muestra para no
                   esconder lo que el cliente ya pagó, pero NO se puede entrar:
                   la pantalla respondería 501. -->
              <span class="nav-item pendiente" title="Módulo contratado, aún no disponible">
                <i [class]="'pi ' + item.icon"></i>
                <span>{{ item.label }}</span>
                <small class="badge">pronto</small>
              </span>
            }
          }
        </div>
      }
    </nav>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
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
    .nav-item.pendiente { color: rgba(255,255,255,0.35); cursor: default; }
    .nav-item.pendiente:hover { background: transparent; color: rgba(255,255,255,0.35); }
    .nav-item .badge {
      margin-left: auto; font-size: 0.6rem; text-transform: uppercase;
      background: rgba(255,255,255,0.08); padding: 0.1rem 0.35rem; border-radius: 4px;
    }
  `]
})
export class SidebarComponent {
  private authService = inject(AuthService);
  private modulos = inject(ModulosService);

  private readonly allSections: MenuSection[] = [
    {
      label: 'Principal',
      items: [
        { label: 'Dashboard', icon: 'pi-home', route: '/dashboard' },
        { label: 'Pacientes', icon: 'pi-users', route: '/pacientes', modulo: 'PACIENTES', roles: ['admin', 'medico', 'superadmin', 'enfermeria'] },
        { label: 'Citas', icon: 'pi-calendar', route: '/citas', modulo: 'CITAS', roles: ['admin', 'medico', 'superadmin', 'enfermeria'] },
      ]
    },
    {
      label: 'Clínico',
      roles: ['admin', 'medico', 'superadmin', 'enfermeria', 'anestesiologo'],
      items: [
        { label: 'Historia Clínica', icon: 'pi-file-edit', route: '/historias', modulo: 'HISTORIA', roles: ['admin', 'medico', 'superadmin'] },
        { label: 'Urgencias', icon: 'pi-exclamation-circle', route: '/urgencias', modulo: 'URGENCIAS' },
        { label: 'Hospitalización', icon: 'pi-building', route: '/hospitalizacion', modulo: 'HOSPITALIZACION' },
        // Sin `modulo`: ver la nota de `app.routes.ts`. Va en Clínico y no en
        // Administrativo porque quien despacha una ambulancia está atendiendo una
        // urgencia, no llevando una flota.
        { label: 'Ambulancias', icon: 'pi-truck', route: '/ambulancias', roles: ['admin', 'superadmin', 'medico', 'enfermeria', 'auxiliar_enfermeria'] },
        { label: 'Evoluciones', icon: 'pi-list', route: '/evoluciones', modulo: 'EVOLUCIONES' },
        { label: 'Interconsultas', icon: 'pi-share-alt', route: '/interconsultas', modulo: 'INTERCONSULTAS' },
        { label: 'Epicrisis', icon: 'pi-file', route: '/epicrisis', modulo: 'EPICRISIS' },
      ]
    },
    {
      label: 'Especialidades',
      items: [
        { label: 'Laboratorio', icon: 'pi-chart-scatter', route: '/laboratorio', modulo: 'LABORATORIO', roles: ['laboratorista', 'bacteriologo', 'admin', 'medico', 'superadmin'] },
        { label: 'Imágenes', icon: 'pi-image', route: '/imagenes', modulo: 'IMAGENES', roles: ['radiologo', 'admin', 'medico', 'superadmin'] },
        { label: 'Nutrición', icon: 'pi-heart', route: '/nutricion', modulo: 'NUTRICION', roles: ['nutricionista', 'admin', 'superadmin'] },
        { label: 'Rehabilitación', icon: 'pi-refresh', route: '/rehabilitacion', modulo: 'REHABILITACION', roles: ['terapeuta', 'admin', 'superadmin'] },
        { label: 'Quirófano', icon: 'pi-wrench', route: '/quirofano', modulo: 'QUIROFANO', roles: ['anestesiologo', 'medico', 'admin', 'superadmin'] },
      ]
    },
    {
      label: 'Administrativo',
      items: [
        { label: 'Farmacia', icon: 'pi-box', route: '/farmacia', modulo: 'FARMACIA', roles: ['regente_farmacia', 'admin', 'superadmin'] },
        { label: 'Facturación', icon: 'pi-money-bill', route: '/facturacion', modulo: 'FACTURACION', roles: ['admin', 'superadmin'] },
        { label: 'RIPS', icon: 'pi-download', route: '/rips', modulo: 'RIPS', roles: ['admin', 'superadmin'] },
        { label: 'Estadísticas', icon: 'pi-chart-bar', route: '/estadisticas', modulo: 'ESTADISTICAS', roles: ['admin', 'superadmin', 'medico'] },
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
      if (s.roles && !this.authService.hasRole(...(s.roles as RolUsuario[]))) {
        return false;
      }
      // Una sección cuyas opciones se filtraron todas no debe dejar el título
      // huérfano ("Quirúrgico" sin nada debajo).
      return this.visibleItems(s.items).length > 0;
    });
  }

  visibleItems(items: MenuItem[]): MenuItem[] {
    return items.filter(item => {
      // Dos filtros independientes: QUIÉN es el usuario y QUÉ contrató la
      // institución. Un médico no ve facturación aunque la IPS la tenga; y
      // nadie ve quirófano si el consultorio no lo contrató.
      if (item.roles && !this.authService.hasRole(...(item.roles as RolUsuario[]))) {
        return false;
      }
      return this.modulos.estaVisible(item.modulo);
    });
  }

  /** Contratado Y construido: se puede navegar. */
  disponible(item: MenuItem): boolean {
    return this.modulos.estaDisponible(item.modulo);
  }
}
