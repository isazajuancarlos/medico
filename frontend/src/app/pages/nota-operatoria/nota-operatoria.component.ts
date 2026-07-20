import { CommonModule } from '@angular/common';
import { Component, inject, OnInit, signal } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { MessageModule } from 'primeng/message';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { ZoomDirective } from '../../shared/formulario/zoom.directive';

/**
 * Nota operatoria. Migrado de `modules/nota_operatoria.py` (NiceGUI).
 *
 * El tiempo quirúrgico que se muestra viene calculado del servidor, no de
 * `tiempo_quirurgico_min`: el modelo guarda tres campos para dos datos y
 * escritos por separado acaban discrepando.
 *
 * Los avisos se muestran pero no bloquean nada. En cirugía lo infrecuente pasa,
 * y un sistema que no deja registrar lo que de verdad ocurrió obliga a mentirle.
 */

interface Nota {
  id: number;
  fecha_creacion: string;
  paciente: string | null;
  cirujano: string | null;
  procedimiento_realizado: string;
  diagnostico_postoperatorio: string;
  hora_inicio: string;
  hora_fin: string;
  tiempo_quirurgico_min: number;
  estado_paciente_salida: string;
  avisos: string[];
}

@Component({
  selector: 'app-nota-operatoria',
  standalone: true,
  imports: [CommonModule, TableModule, TagModule, MessageModule, ZoomDirective],
  template: `
    <div class="page-card">
      <h2 class="page-title">Notas operatorias</h2>

      <p-table [value]="filas()" [loading]="cargando()" [paginator]="true"
        [rows]="20" dataKey="id" styleClass="p-datatable-sm">
        <ng-template pTemplate="header">
          <tr>
            <th>Fecha</th><th>Paciente</th><th>Procedimiento</th>
            <th>Duración</th><th>Salida</th><th>Cirujano</th><th>Revisar</th>
          </tr>
        </ng-template>
        <ng-template pTemplate="body" let-n>
          <tr>
            <td>{{ n.fecha_creacion | date: 'yyyy-MM-dd' }}</td>
            <td appZoom>{{ n.paciente }}</td>
            <td appZoom>{{ n.procedimiento_realizado }}</td>
            <td>{{ duracion(n.tiempo_quirurgico_min) }}</td>
            <td>
              <p-tag [value]="n.estado_paciente_salida"
                [severity]="n.estado_paciente_salida === 'ESTABLE' ? 'success' : 'danger'" />
            </td>
            <td>{{ n.cirujano }}</td>
            <td>
              @if (n.avisos.length) {
                <ul class="avisos">
                  @for (a of n.avisos; track a) { <li>{{ a }}</li> }
                </ul>
              } @else { <span class="tenue">—</span> }
            </td>
          </tr>
        </ng-template>
        <ng-template pTemplate="emptymessage">
          <tr><td colspan="7" class="vacio">No hay notas operatorias.</td></tr>
        </ng-template>
      </p-table>
    </div>
  `,
  styles: [`
    .avisos { margin:0; padding-left:1rem; font-size:.78rem; color:var(--orange-600); }
    .tenue { color:var(--text-color-secondary); }
    .vacio { text-align:center; padding:2rem; color:var(--text-color-secondary); }
  `],
})
export class NotaOperatoriaComponent implements OnInit {
  private http = inject(HttpClient);

  filas = signal<Nota[]>([]);
  cargando = signal(false);

  ngOnInit(): void {
    this.cargando.set(true);
    this.http.get<Nota[]>('/api/nota-operatoria/').subscribe({
      next: (r) => { this.filas.set(r); this.cargando.set(false); },
      error: () => { this.filas.set([]); this.cargando.set(false); },
    });
  }

  /** «2 h 15 min» se lee mejor que «135». */
  duracion(minutos: number): string {
    const h = Math.floor(minutos / 60);
    const m = minutos % 60;
    return h ? `${h} h ${m} min` : `${m} min`;
  }
}
