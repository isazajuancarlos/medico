import { Routes } from '@angular/router';
import { MainLayoutComponent } from './layout/main-layout.component';
import { authGuard } from './core/auth.guard';
import { roleGuard } from './core/role.guard';

export const routes: Routes = [
  { path: '', redirectTo: '/login', pathMatch: 'full' },
  {
    path: 'login',
    loadComponent: () => import('./pages/login/login.component').then(m => m.LoginComponent),
  },
  {
    path: '',
    component: MainLayoutComponent,
    canActivate: [authGuard],
    children: [
      {
        path: 'dashboard',
        loadComponent: () => import('./pages/dashboard/dashboard.component').then(m => m.DashboardComponent),
      },
      {
        path: 'pacientes',
        loadComponent: () => import('./pages/pacientes/pacientes.component').then(m => m.PacientesComponent),
        canActivate: [roleGuard],
        data: { roles: ['admin', 'medico', 'superadmin', 'enfermeria', 'auxiliar_enfermeria', 'psicologo'] },
      },
      {
        path: 'citas',
        loadComponent: () => import('./pages/citas/citas.component').then(m => m.CitasComponent),
      },
      {
        path: 'historias',
        loadComponent: () => import('./pages/historias/historias.component').then(m => m.HistoriasComponent),
        canActivate: [roleGuard],
        data: { roles: ['admin', 'medico', 'superadmin'] },
      },
      {
        path: 'urgencias',
        loadComponent: () => import('./pages/urgencias/urgencias.component').then(m => m.UrgenciasComponent),
      },
      {
        path: 'farmacia',
        loadComponent: () => import('./pages/farmacia/farmacia.component').then(m => m.FarmaciaComponent),
        canActivate: [roleGuard],
        data: { roles: ['regente_farmacia', 'admin', 'superadmin'] },
      },
      {
        path: 'facturacion',
        loadComponent: () => import('./pages/facturacion/facturacion.component').then(m => m.FacturacionComponent),
      },
      {
        path: 'hospitalizacion',
        loadComponent: () => import('./pages/hospitalizacion/hospitalizacion.component').then(m => m.HospitalizacionComponent),
      },
      {
        path: 'admin',
        loadComponent: () => import('./pages/admin/admin.component').then(m => m.AdminComponent),
        canActivate: [roleGuard],
        data: { roles: ['admin', 'superadmin'] },
      },
      {
        path: 'superadmin',
        loadComponent: () => import('./pages/superadmin/superadmin.component').then(m => m.SuperadminComponent),
        canActivate: [roleGuard],
        data: { roles: ['superadmin'] },
      },
      {
        path: 'rips',
        loadComponent: () => import('./pages/rips/rips.component').then(m => m.RipsComponent),
        canActivate: [roleGuard],
        data: { roles: ['admin', 'superadmin'] },
      },
      // Placeholder routes for all other modules
      { path: 'evoluciones', loadComponent: () => import('./pages/evoluciones/evoluciones.component').then(m => m.EvolucionesComponent) },
      { path: 'consentimiento', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Consentimiento Informado' } },
      { path: 'incapacidades', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Incapacidades' } },
      { path: 'remisiones', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Remisiones' } },
      { path: 'laboratorio', loadComponent: () => import('./pages/laboratorio/laboratorio.component').then(m => m.LaboratorioComponent) },
      { path: 'imagenes', loadComponent: () => import('./pages/imagenes/imagenes.component').then(m => m.ImagenesComponent) },
      { path: 'banco-sangre', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Banco de Sangre' } },
      { path: 'quirofano', loadComponent: () => import('./pages/quirofano/quirofano.component').then(m => m.QuirofanoComponent) },
      { path: 'nota-operatoria', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Nota Operatoria' } },
      { path: 'preanestesia', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Pre-Anestesia' } },
      { path: 'recuperacion', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Recuperación (UCPA)' } },
      { path: 'interconsultas', loadComponent: () => import('./pages/interconsultas/interconsultas.component').then(m => m.InterconsultasComponent) },
      { path: 'epicrisis', loadComponent: () => import('./pages/epicrisis/epicrisis.component').then(m => m.EpicrisisComponent) },
      { path: 'contrarreferencia', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Contrarreferencia' } },
      { path: 'nutricion', loadComponent: () => import('./pages/nutricion/nutricion.component').then(m => m.NutricionComponent) },
      { path: 'rehabilitacion', loadComponent: () => import('./pages/rehabilitacion/rehabilitacion.component').then(m => m.RehabilitacionComponent) },
      { path: 'trabajo-social', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Trabajo Social' } },
      { path: 'telemedicina', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Telemedicina' } },
      { path: 'vacunacion', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Vacunación PAI' } },
      { path: 'prenatal', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Control Prenatal' } },
      { path: 'crecimiento-desarrollo', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Crecimiento y Desarrollo' } },
      { path: 'salud-reproductiva', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Salud Reproductiva' } },
      { path: 'certificado-defuncion', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Certificado de Defunción' } },
      { path: 'registro-nacimiento', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Registro de Nacimiento' } },
      { path: 'turnos', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Turnos' } },
      { path: 'comites', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Comités' } },
      { path: 'equipos-medicos', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Equipos Médicos' } },
      { path: 'indicadores', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Indicadores de Calidad' } },
      { path: 'infecciones', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Control de Infecciones' } },
      { path: 'estadisticas', loadComponent: () => import('./pages/estadisticas/estadisticas.component').then(m => m.EstadisticasComponent) },
      { path: 'agenda-medica', loadComponent: () => import('./pages/agenda-medica/agenda-medica.component').then(m => m.AgendaMedicaComponent) },
      { path: 'portal-citas', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Portal Paciente - Citas' } },
      { path: 'terminos', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Términos y Condiciones' } },
      { path: '**', redirectTo: '/dashboard' },
    ],
  },
];
