import { CommonModule } from '@angular/common';
import { Component, inject, OnInit, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { HttpClient, HttpParams } from '@angular/common/http';
import { ButtonModule } from 'primeng/button';
import { CheckboxModule } from 'primeng/checkbox';
import { MessageModule } from 'primeng/message';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { ZoomDirective } from '../../shared/formulario/zoom.directive';

/**
 * Contrarreferencia. Migrado de `modules/contrarreferencia.py` (NiceGUI).
 *
 * Muestra lo que el servidor dice que pasó con la remisión al registrarla. En
 * el original no pasaba nada —la remisión seguía pendiente— y el usuario no
 * tenía forma de saberlo.
 */

interface Contrarreferencia {
  id: number;
  fecha_creacion: string;
  paciente: string | null;
  diagnostico_final: string;
  recomendaciones: string | null;
  requiere_seguimiento: boolean;
  fecha_control_sugerida: string | null;
  recibida: boolean;
  fecha_recepcion: string | null;
  cierre: string | null;
}

@Component({
  selector: 'app-contrarreferencia',
  standalone: true,
  imports: [CommonModule, FormsModule, TableModule, ButtonModule, TagModule,
            MessageModule, CheckboxModule, ZoomDirective],
  template: `
    <div class="page-card">
      <div class="cabecera">
        <h2 class="page-title">Contrarreferencias</h2>
        <label class="filtro">
          <p-checkbox [(ngModel)]="soloPendientes" [binary]="true"
            (onChange)="cargar()" inputId="sp" />
          <span>Solo sin acusar recibo</span>
        </label>
      </div>

      @if (aviso(); as a) { <p-message severity="info" [text]="a" styleClass="w-full" /> }
      @if (error(); as e) { <p-message severity="error" [text]="e" styleClass="w-full" /> }

      <p-table [value]="filas()" [loading]="cargando()" [paginator]="true"
        [rows]="20" dataKey="id" styleClass="p-datatable-sm">
        <ng-template pTemplate="header">
          <tr>
            <th>Fecha</th><th>Paciente</th><th>Diagnóstico final</th>
            <th>Seguimiento</th><th>Recibida</th><th></th>
          </tr>
        </ng-template>
        <ng-template pTemplate="body" let-c>
          <tr>
            <td>{{ c.fecha_creacion | date: 'yyyy-MM-dd' }}</td>
            <td appZoom>{{ c.paciente }}</td>
            <td appZoom>{{ c.diagnostico_final }}</td>
            <td>
              @if (c.requiere_seguimiento) {
                <p-tag [value]="c.fecha_control_sugerida || 'Sí'" severity="warn" />
              } @else { <span class="tenue">—</span> }
            </td>
            <td>
              @if (c.recibida) {
                <p-tag [value]="(c.fecha_recepcion | date: 'yyyy-MM-dd') ?? 'Recibida'" severity="success" />
              } @else { <p-tag value="Pendiente" severity="warn" /> }
            </td>
            <td>
              @if (!c.recibida) {
                <p-button icon="pi pi-check" [text]="true" size="small"
                  pTooltip="Acusar recibo" (onClick)="recibir(c)" />
              }
            </td>
          </tr>
        </ng-template>
        <ng-template pTemplate="emptymessage">
          <tr><td colspan="6" class="vacio">No hay contrarreferencias.</td></tr>
        </ng-template>
      </p-table>
    </div>
  `,
  styles: [`
    .cabecera { display:flex; justify-content:space-between; align-items:center; margin-bottom:1rem; }
    .filtro { display:flex; align-items:center; gap:.5rem; font-size:.85rem; }
    .tenue { color:var(--text-color-secondary); }
    .vacio { text-align:center; padding:2rem; color:var(--text-color-secondary); }
  `],
})
export class ContrarreferenciaComponent implements OnInit {
  private http = inject(HttpClient);

  filas = signal<Contrarreferencia[]>([]);
  cargando = signal(false);
  aviso = signal<string | null>(null);
  error = signal<string | null>(null);
  soloPendientes = false;

  ngOnInit(): void { this.cargar(); }

  cargar(): void {
    this.cargando.set(true);
    let params = new HttpParams();
    if (this.soloPendientes) params = params.set('solo_pendientes', 'true');
    this.http.get<Contrarreferencia[]>('/api/contrarreferencia/', { params }).subscribe({
      next: (r) => { this.filas.set(r); this.cargando.set(false); },
      error: () => { this.filas.set([]); this.cargando.set(false); },
    });
  }

  recibir(c: Contrarreferencia): void {
    this.error.set(null);
    this.http.post(`/api/contrarreferencia/${c.id}/recibir`, {}).subscribe({
      next: () => { this.aviso.set('Acuse de recibo registrado.'); this.cargar(); },
      error: (e) => this.error.set(e?.error?.detail ?? 'No se pudo registrar el acuse.'),
    });
  }
}
