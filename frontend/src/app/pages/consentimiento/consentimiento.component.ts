import { CommonModule } from '@angular/common';
import { Component, inject, OnInit, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { HttpClient, HttpParams } from '@angular/common/http';
import { ButtonModule } from 'primeng/button';
import { CheckboxModule } from 'primeng/checkbox';
import { DialogModule } from 'primeng/dialog';
import { InputTextModule } from 'primeng/inputtext';
import { MessageModule } from 'primeng/message';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { TextareaModule } from 'primeng/textarea';
import { ZoomDirective } from '../../shared/formulario/zoom.directive';

/**
 * Consentimiento informado. Migrado de `modules/consentimiento.py` (NiceGUI).
 *
 * La pantalla NO decide si un consentimiento vale: lo pregunta. El servidor
 * devuelve `valido` y la lista de lo que falta, y aquí solo se muestra. La
 * regla —qué hace que un consentimiento sea informado— vive en un solo sitio,
 * porque es la que hay que poder defender ante un juez y no puede tener dos
 * versiones.
 */

interface Consentimiento {
  id: number;
  fecha_firma: string;
  tipo_procedimiento: string;
  descripcion_procedimiento: string;
  paciente: string | null;
  medico: string | null;
  paciente_comprende: boolean;
  acepta_procedimiento: boolean;
  revocado: boolean;
  fecha_revocacion: string | null;
  motivo_revocacion: string | null;
  valido: boolean;
  faltantes: string[];
  resumen: string;
}

@Component({
  selector: 'app-consentimiento',
  standalone: true,
  imports: [
    CommonModule, FormsModule, TableModule, ButtonModule, InputTextModule,
    TagModule, DialogModule, MessageModule, CheckboxModule, TextareaModule,
    ZoomDirective,
  ],
  template: `
    <div class="page-card">
      <div class="cabecera">
        <h2 class="page-title">Consentimiento informado</h2>
        <label class="solo-validos">
          <p-checkbox [(ngModel)]="soloValidos" [binary]="true"
            (onChange)="cargar()" inputId="sv" />
          <span>Solo los completos</span>
        </label>
      </div>

      @if (error(); as e) { <p-message severity="error" [text]="e" styleClass="w-full" /> }

      <p-table [value]="filas()" [loading]="cargando()" [paginator]="true"
        [rows]="20" dataKey="id" styleClass="p-datatable-sm">
        <ng-template pTemplate="header">
          <tr>
            <th>Fecha</th><th>Paciente</th><th>Procedimiento</th>
            <th>Estado</th><th>Médico</th><th></th>
          </tr>
        </ng-template>
        <ng-template pTemplate="body" let-c>
          <tr [class.revocado]="c.revocado">
            <td>{{ c.fecha_firma | date: 'yyyy-MM-dd' }}</td>
            <td appZoom>{{ c.paciente }}</td>
            <td appZoom>{{ c.tipo_procedimiento }}</td>
            <td>
              @if (c.revocado) {
                <p-tag value="Revocado" severity="danger" />
              } @else if (c.valido) {
                <p-tag value="Completo" severity="success" />
              } @else {
                <p-tag value="Incompleto" severity="warn" />
              }
            </td>
            <td>{{ c.medico }}</td>
            <td class="acciones">
              <p-button icon="pi pi-eye" [text]="true" size="small"
                pTooltip="Ver" (onClick)="ver(c)" />
              @if (!c.revocado) {
                <p-button icon="pi pi-ban" [text]="true" size="small" severity="danger"
                  pTooltip="Registrar revocación" (onClick)="pedirRevocacion(c)" />
              }
            </td>
          </tr>
        </ng-template>
        <ng-template pTemplate="emptymessage">
          <tr><td colspan="6" class="vacio">No hay consentimientos registrados.</td></tr>
        </ng-template>
      </p-table>
    </div>

    <p-dialog [(visible)]="detalleAbierto" [modal]="true" header="Consentimiento"
      [style]="{ width: '38rem', maxWidth: '95vw' }">
      @if (detalle(); as c) {
        @if (!c.valido) {
          <p-message [severity]="c.revocado ? 'error' : 'warn'" styleClass="w-full mb-3"
            [text]="c.resumen" />
        }
        <dl class="detalle">
          <dt>Paciente</dt><dd>{{ c.paciente }}</dd>
          <dt>Procedimiento</dt><dd>{{ c.tipo_procedimiento }}</dd>
          <dt>Descripción</dt><dd>{{ c.descripcion_procedimiento }}</dd>
          <dt>Comprendió</dt><dd>{{ c.paciente_comprende ? 'Sí' : 'No' }}</dd>
          <dt>Aceptó</dt><dd>{{ c.acepta_procedimiento ? 'Sí' : 'No' }}</dd>
          @if (c.revocado) {
            <dt>Revocado el</dt><dd>{{ c.fecha_revocacion | date: 'yyyy-MM-dd HH:mm' }}</dd>
            <dt>Manifestó</dt><dd>{{ c.motivo_revocacion }}</dd>
          }
          <dt>Médico</dt><dd>{{ c.medico }}</dd>
        </dl>
      }
    </p-dialog>

    <p-dialog [(visible)]="revocacionAbierta" [modal]="true"
      header="Registrar revocación" [style]="{ width: '32rem', maxWidth: '95vw' }">
      <p class="aviso">
        El paciente puede retirar su consentimiento en cualquier momento y sin
        dar razones. Escriba lo que manifestó, solo para que quede constancia.
      </p>
      <textarea pTextarea [(ngModel)]="motivo" rows="3" class="w-full"
        placeholder="Lo que dijo el paciente"></textarea>
      <ng-template pTemplate="footer">
        <p-button label="Cancelar" [text]="true" (onClick)="revocacionAbierta = false" />
        <p-button label="Registrar revocación" severity="danger"
          [disabled]="!motivo.trim()" (onClick)="revocar()" />
      </ng-template>
    </p-dialog>
  `,
  styles: [`
    .cabecera { display:flex; justify-content:space-between; align-items:center; margin-bottom:1rem; }
    .solo-validos { display:flex; align-items:center; gap:.5rem; font-size:.85rem; }
    tr.revocado { opacity:.65; }
    .acciones { white-space:nowrap; }
    .vacio { text-align:center; padding:2rem; color:var(--text-color-secondary); }
    .detalle { display:grid; grid-template-columns:auto 1fr; gap:.4rem 1rem; margin:0; }
    .detalle dt { font-weight:600; color:var(--text-color-secondary); font-size:.85rem; }
    .detalle dd { margin:0; }
    .aviso { font-size:.85rem; color:var(--text-color-secondary); margin:0 0 .75rem; }
  `],
})
export class ConsentimientoComponent implements OnInit {
  private http = inject(HttpClient);

  filas = signal<Consentimiento[]>([]);
  detalle = signal<Consentimiento | null>(null);
  cargando = signal(false);
  error = signal<string | null>(null);

  soloValidos = false;
  detalleAbierto = false;
  revocacionAbierta = false;
  motivo = '';
  private enRevocacion: Consentimiento | null = null;

  ngOnInit(): void { this.cargar(); }

  cargar(): void {
    this.cargando.set(true);
    let params = new HttpParams();
    if (this.soloValidos) params = params.set('solo_validos', 'true');
    this.http.get<Consentimiento[]>('/api/consentimiento/', { params }).subscribe({
      next: (r) => { this.filas.set(r); this.cargando.set(false); },
      error: () => { this.filas.set([]); this.cargando.set(false); },
    });
  }

  ver(c: Consentimiento): void {
    this.detalle.set(c);
    this.detalleAbierto = true;
  }

  pedirRevocacion(c: Consentimiento): void {
    this.enRevocacion = c;
    this.motivo = '';
    this.revocacionAbierta = true;
  }

  revocar(): void {
    if (!this.enRevocacion || !this.motivo.trim()) return;
    this.http.post<Consentimiento>(
      `/api/consentimiento/${this.enRevocacion.id}/revocar`,
      { motivo: this.motivo.trim() },
    ).subscribe({
      next: () => { this.revocacionAbierta = false; this.cargar(); },
      error: (e) => this.error.set(e?.error?.detail ?? 'No se pudo registrar la revocación.'),
    });
  }
}
