import { Routes } from '@angular/router';
import { MainLayoutComponent } from './layout/main-layout.component';
import { authGuard } from './core/auth.guard';
import { roleGuard } from './core/role.guard';
import { moduloGuard } from './core/modulo.guard';

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
        path: 'modulo-no-disponible',
        loadComponent: () => import('./pages/modulo-no-disponible/modulo-no-disponible.component')
          .then(m => m.ModuloNoDisponibleComponent),
      },
      {
        path: 'dashboard',
        loadComponent: () => import('./pages/dashboard/dashboard.component').then(m => m.DashboardComponent),
      },
      {
        path: 'pacientes',
        loadComponent: () => import('./pages/pacientes/pacientes.component').then(m => m.PacientesComponent),
        canActivate: [roleGuard, moduloGuard('PACIENTES')],
        data: { roles: ['admin', 'medico', 'superadmin', 'enfermeria', 'auxiliar_enfermeria', 'psicologo'] },
      },
      {
        path: 'citas',
        loadComponent: () => import('./pages/citas/citas.component').then(m => m.CitasComponent),
        canActivate: [moduloGuard('CITAS')],
      },
      {
        path: 'historias',
        loadComponent: () => import('./pages/historias/historias.component').then(m => m.HistoriasComponent),
        canActivate: [roleGuard, moduloGuard('HISTORIA')],
        data: { roles: ['admin', 'medico', 'superadmin'] },
      },
      {
        path: 'urgencias',
        loadComponent: () => import('./pages/urgencias/urgencias.component').then(m => m.UrgenciasComponent),
        canActivate: [moduloGuard('URGENCIAS')],
      },
      {
        path: 'farmacia',
        loadComponent: () => import('./pages/farmacia/farmacia.component').then(m => m.FarmaciaComponent),
        canActivate: [roleGuard, moduloGuard('FARMACIA')],
        data: { roles: ['regente_farmacia', 'admin', 'superadmin'] },
      },
      {
        path: 'facturacion',
        loadComponent: () => import('./pages/facturacion/facturacion.component').then(m => m.FacturacionComponent),
        canActivate: [moduloGuard('FACTURACION')],
      },
      {
        path: 'hospitalizacion',
        loadComponent: () => import('./pages/hospitalizacion/hospitalizacion.component').then(m => m.HospitalizacionComponent),
        canActivate: [moduloGuard('HOSPITALIZACION')],
      },
      {
        // La planta del prestador contra las rutas que el binario YA sirve.
        // Aparte de `/admin` porque aquella habla solo con `/api/admin`, que
        // hoy es Python y da 404 contra el binario. Ver el comentario largo del
        // componente.
        path: 'usuarios',
        loadComponent: () => import('./pages/usuarios/usuarios.component').then(m => m.UsuariosComponent),
        canActivate: [roleGuard],
        data: { roles: ['admin', 'superadmin'] },
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
        canActivate: [roleGuard, moduloGuard('RIPS')],
        data: { roles: ['admin', 'superadmin'] },
      },
      // Placeholder routes for all other modules
      { path: 'evoluciones', loadComponent: () => import('./pages/evoluciones/evoluciones.component').then(m => m.EvolucionesComponent), canActivate: [moduloGuard('EVOLUCIONES')] },
      { path: 'consentimiento', loadComponent: () => import('./pages/consentimiento/consentimiento.component').then(m => m.ConsentimientoComponent), canActivate: [moduloGuard('CONSENTIMIENTO')] },
      { path: 'incapacidades', loadComponent: () => import('./pages/incapacidades/incapacidades.component').then(m => m.IncapacidadesComponent), canActivate: [moduloGuard('INCAPACIDADES')] },
      { path: 'remisiones', loadComponent: () => import('./pages/remisiones/remisiones.component').then(m => m.RemisionesComponent), canActivate: [moduloGuard('REMISIONES')] },
      { path: 'laboratorio', loadComponent: () => import('./pages/laboratorio/laboratorio.component').then(m => m.LaboratorioComponent), canActivate: [moduloGuard('LABORATORIO')] },
      { path: 'imagenes', loadComponent: () => import('./pages/imagenes/imagenes.component').then(m => m.ImagenesComponent), canActivate: [moduloGuard('IMAGENES')] },
      // Sin `moduloGuard`: AMBULANCIAS todavía no está en el catálogo de módulos
      // —que hoy solo sirve el backend Python— y ponerle un guardián de un módulo
      // inexistente lo haría invisible sin decir por qué. Cuando el catálogo se
      // porte, esta ruta lo lleva como las demás.
      { path: 'ambulancias', loadComponent: () => import('./pages/ambulancias/ambulancias.component').then(m => m.AmbulanciasComponent),
        canActivate: [roleGuard], data: { roles: ['admin', 'superadmin', 'medico', 'enfermeria', 'auxiliar_enfermeria'] } },
      { path: 'banco-sangre', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Banco de Sangre' } },
      { path: 'quirofano', loadComponent: () => import('./pages/quirofano/quirofano.component').then(m => m.QuirofanoComponent), canActivate: [moduloGuard('QUIROFANO')] },
      { path: 'nota-operatoria', loadComponent: () => import('./pages/nota-operatoria/nota-operatoria.component').then(m => m.NotaOperatoriaComponent), canActivate: [moduloGuard('NOTA_OPERATORIA')] },
      { path: 'preanestesia', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Pre-Anestesia' } },
      { path: 'recuperacion', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Recuperación (UCPA)' } },
      { path: 'interconsultas', loadComponent: () => import('./pages/interconsultas/interconsultas.component').then(m => m.InterconsultasComponent), canActivate: [moduloGuard('INTERCONSULTAS')] },
      { path: 'epicrisis', loadComponent: () => import('./pages/epicrisis/epicrisis.component').then(m => m.EpicrisisComponent), canActivate: [moduloGuard('EPICRISIS')] },
      { path: 'contrarreferencia', loadComponent: () => import('./pages/contrarreferencia/contrarreferencia.component').then(m => m.ContrarreferenciaComponent), canActivate: [moduloGuard('CONTRARREFERENCIA')] },
      { path: 'nutricion', loadComponent: () => import('./pages/nutricion/nutricion.component').then(m => m.NutricionComponent), canActivate: [moduloGuard('NUTRICION')] },
      { path: 'rehabilitacion', loadComponent: () => import('./pages/rehabilitacion/rehabilitacion.component').then(m => m.RehabilitacionComponent), canActivate: [moduloGuard('REHABILITACION')] },
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
      { path: 'estadisticas', loadComponent: () => import('./pages/estadisticas/estadisticas.component').then(m => m.EstadisticasComponent), canActivate: [moduloGuard('ESTADISTICAS')] },
      { path: 'agenda-medica', loadComponent: () => import('./pages/agenda-medica/agenda-medica.component').then(m => m.AgendaMedicaComponent) },
      { path: 'portal-citas', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Portal Paciente - Citas' } },
      { path: 'terminos', loadComponent: () => import('./pages/placeholder/placeholder.component').then(m => m.PlaceholderComponent), data: { title: 'Términos y Condiciones' } },
      { path: '**', redirectTo: '/dashboard' },
    ],
  },
];
