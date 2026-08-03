import { Component, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MensajeComponent } from '../../shared/ui/mensaje.component';
import { HttpClient, HttpParams } from '@angular/common/http';

@Component({
  selector: 'app-rips',
  standalone: true,
  imports: [
    CommonModule, FormsModule, MatButtonModule, MatDatepickerModule,
    MatFormFieldModule, MatInputModule, MatProgressSpinnerModule, MensajeComponent,
  ],
  template: `
    <div class="page-card">
      <h2 class="page-title">Generación de RIPS</h2>
      <p style="color: #64748b; margin-bottom: 1.5rem;">
        Generación de archivos RIPS en formato JSON según Resolución 2275/2023
      </p>

      <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; max-width: 500px; margin-bottom: 1.5rem;">
        <mat-form-field appearance="outline">
          <mat-label>Fecha Inicio</mat-label>
          <input matInput [matDatepicker]="calIni" [(ngModel)]="fechaInicio" />
          <mat-datepicker-toggle matIconSuffix [for]="calIni" />
          <mat-datepicker #calIni />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Fecha Fin</mat-label>
          <input matInput [matDatepicker]="calFin" [(ngModel)]="fechaFin" />
          <mat-datepicker-toggle matIconSuffix [for]="calFin" />
          <mat-datepicker #calFin />
        </mat-form-field>
      </div>

      <button
        matButton="filled"
        [disabled]="!fechaInicio || !fechaFin || loading()"
        (click)="generarRips()"
      >
        @if (loading()) {
          <mat-spinner diameter="18" />
        } @else {
          <i class="pi pi-download"></i>
        }
        Generar RIPS
      </button>

      @if (error()) {
        <x-mensaje severity="error" [text]="error()!" />
      }
      @if (success()) {
        <x-mensaje severity="success" text="RIPS generado exitosamente. Descargando..." />
      }
    </div>
  `,
  styles: [`
    x-mensaje { display: block; margin-top: 1rem; }
    button i { margin-right: 0.4rem; }
    mat-spinner { display: inline-block; margin-right: 0.4rem; }
  `],
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
      // Los dos backends nombran distinto el motivo: FastAPI lo manda en
      // `detail` y el binario de Rust en `mensaje`. Mientras convivan hay que
      // mirar los dos, o el aviso que explica QUÉ pasó —qué documentos faltan,
      // qué historia no tiene valor de servicio— se pierde y el usuario ve un
      // «Error generando RIPS» que no le dice nada.
      error: (err: { error?: { mensaje?: string; detail?: string } }) => {
        this.loading.set(false);
        this.error.set(err.error?.mensaje || err.error?.detail || 'Error generando RIPS');
      },
    });
  }
}
