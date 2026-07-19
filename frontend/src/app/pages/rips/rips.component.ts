import { Component, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { CardModule } from 'primeng/card';
import { DatePickerModule } from 'primeng/datepicker';
import { MessageModule } from 'primeng/message';
import { HttpClient, HttpParams } from '@angular/common/http';

@Component({
  selector: 'app-rips',
  standalone: true,
  imports: [CommonModule, FormsModule, ButtonModule, CardModule, DatePickerModule, MessageModule],
  template: `
    <div class="page-card">
      <h2 class="page-title">Generación de RIPS</h2>
      <p style="color: #64748b; margin-bottom: 1.5rem;">
        Generación de archivos RIPS en formato JSON según Resolución 2275/2023
      </p>

      <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; max-width: 500px; margin-bottom: 1.5rem;">
        <div>
          <label style="display:block; font-weight:500; margin-bottom:0.4rem;">Fecha Inicio</label>
          <p-datepicker [(ngModel)]="fechaInicio" dateFormat="yy-mm-dd" [showIcon]="true" styleClass="w-full" />
        </div>
        <div>
          <label style="display:block; font-weight:500; margin-bottom:0.4rem;">Fecha Fin</label>
          <p-datepicker [(ngModel)]="fechaFin" dateFormat="yy-mm-dd" [showIcon]="true" styleClass="w-full" />
        </div>
      </div>

      <p-button
        label="Generar RIPS"
        icon="pi pi-download"
        [loading]="loading()"
        [disabled]="!fechaInicio || !fechaFin"
        (onClick)="generarRips()"
      />

      @if (error()) {
        <p-message severity="error" [text]="error()!" styleClass="w-full mt-3" />
      }
      @if (success()) {
        <p-message severity="success" text="RIPS generado exitosamente. Descargando..." styleClass="w-full mt-3" />
      }
    </div>
  `,
})
export class RipsComponent {
  private http = inject(HttpClient);
  fechaInicio: Date | null = null;
  fechaFin: Date | null = null;
  loading = signal(false);
  error = signal<string | null>(null);
  success = signal(false);

  generarRips(): void {
    if (!this.fechaInicio || !this.fechaFin) return;
    this.loading.set(true);
    this.error.set(null);
    this.success.set(false);

    const fmt = (d: Date) => d.toISOString().split('T')[0];
    const params = new HttpParams()
      .set('inicio', fmt(this.fechaInicio))
      .set('fin', fmt(this.fechaFin));

    this.http.get('/api/rips', { params }).subscribe({
      next: data => {
        this.loading.set(false);
        this.success.set(true);
        const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `RIPS_${fmt(this.fechaInicio!)}_${fmt(this.fechaFin!)}.json`;
        a.click();
        URL.revokeObjectURL(url);
      },
      error: (err: { error?: { detail?: string } }) => {
        this.loading.set(false);
        this.error.set(err.error?.detail || 'Error generando RIPS');
      },
    });
  }
}
