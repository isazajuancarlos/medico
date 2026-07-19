import { Component, OnInit, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { HttpClient } from '@angular/common/http';
import { CardModule } from 'primeng/card';
import { AuthService } from '../../core/auth.service';
import { KPIs } from '../../models/common.models';

@Component({
  selector: 'app-dashboard',
  standalone: true,
  imports: [CommonModule, CardModule],
  template: `
    <div class="page-card">
      <h2 class="page-title">
        Bienvenido, {{ user()?.nombre_completo }}
      </h2>
      <p style="color: #64748b">{{ today }}</p>
    </div>

    @if (kpis) {
      <div class="kpi-grid">
        <div class="kpi-card">
          <div class="kpi-icon blue"><i class="pi pi-calendar"></i></div>
          <div class="kpi-data">
            <div class="kpi-value">{{ kpis.citas_hoy }}</div>
            <div class="kpi-label">Citas Hoy</div>
          </div>
        </div>
        <div class="kpi-card">
          <div class="kpi-icon green"><i class="pi pi-calendar-plus"></i></div>
          <div class="kpi-data">
            <div class="kpi-value">{{ kpis.citas_mes }}</div>
            <div class="kpi-label">Citas Este Mes</div>
          </div>
        </div>
        <div class="kpi-card">
          <div class="kpi-icon purple"><i class="pi pi-users"></i></div>
          <div class="kpi-data">
            <div class="kpi-value">{{ kpis.total_pacientes }}</div>
            <div class="kpi-label">Total Pacientes</div>
          </div>
        </div>
      </div>
    }
  `,
  styles: [`
    .kpi-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 1rem; }
    .kpi-card {
      background: white; border-radius: 12px; padding: 1.5rem;
      box-shadow: 0 1px 3px rgba(0,0,0,0.1); display: flex; align-items: center; gap: 1rem;
    }
    .kpi-icon {
      width: 52px; height: 52px; border-radius: 12px;
      display: flex; align-items: center; justify-content: center; flex-shrink: 0;
    }
    .kpi-icon i { font-size: 1.4rem; }
    .kpi-icon.blue { background: #dbeafe; color: #2563eb; }
    .kpi-icon.green { background: #dcfce7; color: #16a34a; }
    .kpi-icon.purple { background: #f3e8ff; color: #9333ea; }
    .kpi-value { font-size: 2rem; font-weight: 700; color: #1e293b; line-height: 1; }
    .kpi-label { font-size: 0.8rem; color: #64748b; margin-top: 0.25rem; }
  `]
})
export class DashboardComponent implements OnInit {
  private http = inject(HttpClient);
  authService = inject(AuthService);

  readonly user = this.authService.currentUser;
  kpis: KPIs | null = null;
  today = new Date().toLocaleDateString('es-CO', { weekday: 'long', year: 'numeric', month: 'long', day: 'numeric' });

  ngOnInit(): void {
    this.http.get<KPIs>('/api/estadisticas/kpis').subscribe({
      next: data => this.kpis = data,
      error: () => {},
    });
  }
}
