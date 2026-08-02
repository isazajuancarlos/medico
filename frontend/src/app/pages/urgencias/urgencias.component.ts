import { Component, OnInit, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog, MatDialogModule } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatTableModule } from '@angular/material/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { Avisos } from '../../shared/ui/avisos.service';
import {
  UrgenciasService, EntradaCola, NIVELES_TRIAGE, DESTINOS_SALIDA,
} from '../../services/urgencias.service';

/** Lo que el diálogo de triage devuelve al cerrarse aceptando. */
export interface TriageNuevo {
  paciente_id: number;
  triage: string;
  motivo_consulta: string;
}

/** Lo que devuelve el de egreso. */
export interface EgresoNuevo {
  destino: string;
  observaciones?: string;
}

/**
 * Registrar a un paciente en la cola de triage.
 *
 * La búsqueda vive DENTRO del diálogo, no en la pantalla: es un paso del
 * formulario, y dejarla fuera obligaba a limpiarla a mano cada vez que se abría
 * (`abrirRegistro()` reseteaba cinco campos de la clase anfitriona; ahora el
 * componente nace vacío y muere con el diálogo).
 */
@Component({
  selector: 'app-urgencias-triage',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatSelectModule,
  ],
  template: `
    <h2 mat-dialog-title>Registrar paciente en triage</h2>
    <mat-dialog-content>
      <div class="dlg">
        <div class="buscador">
          <mat-form-field appearance="outline" subscriptSizing="dynamic" class="crece">
            <mat-label>Paciente</mat-label>
            <input matInput [(ngModel)]="busqueda" placeholder="Nombre o identificación…"
              (keyup.enter)="buscar()" />
          </mat-form-field>
          <button matButton="filled" [disabled]="buscando()" (click)="buscar()">
            <i [class]="buscando() ? 'pi pi-spin pi-spinner' : 'pi pi-search'"></i>
          </button>
        </div>
        @if (pacientes().length) {
          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Seleccione…</mat-label>
            <mat-select [(ngModel)]="pacienteSel">
              @for (p of pacientes(); track p.id) {
                <mat-option [value]="p">{{ p.nombre_completo }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
        } @else if (buscado()) {
          <!-- Sin esta frase, buscar y no encontrar nada se ve igual que no
               haber buscado todavía. -->
          <p class="sin-resultado">Ningún paciente coincide con esa búsqueda.</p>
        }
        <mat-form-field appearance="outline" subscriptSizing="dynamic">
          <mat-label>Nivel de triage *</mat-label>
          <mat-select [(ngModel)]="triageSel">
            @for (n of niveles; track n.value) {
              <mat-option [value]="n.value">{{ n.label }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline" subscriptSizing="dynamic">
          <mat-label>Motivo de consulta *</mat-label>
          <textarea matInput [(ngModel)]="motivo" rows="3"
            placeholder="Motivo por el que consulta…"></textarea>
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!completo()" [mat-dialog-close]="resultado()">
        <i class="pi pi-check"></i> Registrar
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .dlg { display:flex; flex-direction:column; gap:1rem; padding-top:.5rem; }
    .buscador { display:flex; gap:.5rem; align-items:flex-start; }
    .buscador .crece { flex:1; }
    .sin-resultado { margin:0; font-size:.85rem; color:var(--text-color-secondary); }
    mat-form-field { width:100%; }
  `],
})
export class UrgenciasTriageComponent {
  private svc = inject(UrgenciasService);

  readonly niveles = NIVELES_TRIAGE;

  busqueda = '';
  buscando = signal(false);
  buscado = signal(false);
  pacientes = signal<{ id: number; nombre_completo: string; identificacion: string }[]>([]);
  pacienteSel: { id: number } | null = null;
  triageSel: string | null = null;
  motivo = '';

  buscar(): void {
    if (!this.busqueda.trim()) return;
    this.buscando.set(true);
    this.svc.searchPacientes().subscribe({
      next: (ps) => {
        const q = this.busqueda.trim().toLowerCase();
        this.pacientes.set(ps.filter((p) =>
          p.nombre_completo.toLowerCase().includes(q) || String(p.identificacion).includes(q)));
        this.buscando.set(false);
        this.buscado.set(true);
      },
      error: () => { this.buscando.set(false); this.buscado.set(true); this.pacientes.set([]); },
    });
  }

  completo(): boolean {
    return !!this.pacienteSel && !!this.triageSel && !!this.motivo.trim();
  }

  /**
   * El valor que sale por `afterClosed()`. Devuelve `null` cuando falta algo:
   * el botón ya está inerte, pero un diálogo que pudiera cerrarse devolviendo
   * un triage a medias sería un paciente en cola sin nivel.
   */
  resultado(): TriageNuevo | null {
    if (!this.completo()) return null;
    return {
      paciente_id: this.pacienteSel!.id,
      triage: this.triageSel!,
      motivo_consulta: this.motivo.trim(),
    };
  }
}

/** El egreso. Mismo patrón: devuelve el dato, o nada. */
@Component({
  selector: 'app-urgencias-egreso',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatSelectModule,
  ],
  template: `
    <h2 mat-dialog-title>Registrar egreso</h2>
    <mat-dialog-content>
      <div class="dlg">
        <mat-form-field appearance="outline" subscriptSizing="dynamic">
          <mat-label>Destino de salida *</mat-label>
          <mat-select [(ngModel)]="destinoSel">
            @for (d of destinos; track d.value) {
              <mat-option [value]="d.value">{{ d.label }}</mat-option>
            }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline" subscriptSizing="dynamic">
          <mat-label>Observaciones</mat-label>
          <textarea matInput [(ngModel)]="obs" rows="2" placeholder="Opcional…"></textarea>
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!destinoSel" [mat-dialog-close]="resultado()">
        <i class="pi pi-check"></i> Egresar
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .dlg { display:flex; flex-direction:column; gap:1rem; padding-top:.5rem; }
    mat-form-field { width:100%; }
  `],
})
export class UrgenciasEgresoComponent {
  readonly destinos = DESTINOS_SALIDA;
  destinoSel: string | null = null;
  obs = '';

  resultado(): EgresoNuevo | null {
    if (!this.destinoSel) return null;
    return { destino: this.destinoSel, observaciones: this.obs.trim() || undefined };
  }
}

/**
 * Urgencias — cola de triage. Backend Rust propio (/api/urgencias). La cola
 * llega YA ordenada por prioridad desde el servidor (rojo primero); aquí solo se
 * pinta, se registra un triage nuevo y se egresa.
 */
@Component({
  selector: 'app-urgencias',
  standalone: true,
  imports: [
    CommonModule, MatTableModule, MatButtonModule, MatProgressBarModule,
    EtiquetaComponent,
  ],
  template: `
    <div class="page-card">
      <div class="list-header">
        <h2 class="page-title">Urgencias — Cola de triage</h2>
        <button matButton="filled" (click)="abrirRegistro()">
          <i class="pi pi-plus"></i> Registrar paciente
        </button>
      </div>

      <!-- También mientras se guarda: el diálogo ya se cerró y la petición sigue
           viva, así que sin esto la pantalla se queda igual y parece que el
           registro no llegó a hacerse. Con p-dialog eso lo tapaba el
           [loading] de su botón, que ya no existe. -->
      @if (cargando() || guardando()) { <mat-progress-bar mode="indeterminate" /> }

      <table mat-table [dataSource]="cola()">
        <ng-container matColumnDef="triage">
          <th mat-header-cell *matHeaderCellDef>Triage</th>
          <td mat-cell *matCellDef="let p">
            <x-etiqueta [value]="p.triage_label" [severity]="severidad(p.triage)" />
          </td>
        </ng-container>
        <ng-container matColumnDef="paciente">
          <th mat-header-cell *matHeaderCellDef>Paciente</th>
          <td mat-cell *matCellDef="let p">{{ p.paciente_id }}</td>
        </ng-container>
        <ng-container matColumnDef="motivo">
          <th mat-header-cell *matHeaderCellDef>Motivo</th>
          <td mat-cell *matCellDef="let p">{{ p.motivo_consulta }}</td>
        </ng-container>
        <ng-container matColumnDef="ingreso">
          <th mat-header-cell *matHeaderCellDef>Ingreso</th>
          <td mat-cell *matCellDef="let p">{{ p.fecha_ingreso | date:'HH:mm' }}</td>
        </ng-container>
        <ng-container matColumnDef="accion">
          <th mat-header-cell *matHeaderCellDef class="centro">Acción</th>
          <td mat-cell *matCellDef="let p" class="centro">
            <button matButton (click)="abrirEgreso(p)">
              <i class="pi pi-sign-out"></i> Egresar
            </button>
          </td>
        </ng-container>

        <tr mat-header-row *matHeaderRowDef="columnas"></tr>
        <tr mat-row *matRowDef="let fila; columns: columnas"></tr>
      </table>

      <!-- La cola llega ordenada del servidor y NO se pagina: en urgencias el
           orden es la prioridad, y una segunda página escondería al rojo. -->
      @if (!cargando() && cola().length === 0) {
        <p class="vacio">Cola de urgencias vacía</p>
      }
    </div>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .list-header { display:flex; justify-content:space-between; align-items:center; margin-bottom:1.25rem; }
    .list-header .page-title { margin-bottom:0; }
    table { width:100%; }
    .centro { text-align:center; }
    .vacio { text-align:center; padding:2rem; color:#64748b; }
  `],
})
export class UrgenciasComponent implements OnInit {
  private svc = inject(UrgenciasService);
  private messages = inject(Avisos);
  private dialogo = inject(MatDialog);

  readonly columnas = ['triage', 'paciente', 'motivo', 'ingreso', 'accion'];

  cola = signal<EntradaCola[]>([]);
  cargando = signal(true);
  guardando = signal(false);

  ngOnInit(): void { this.cargar(); }

  cargar(): void {
    this.cargando.set(true);
    this.svc.cola().subscribe({
      next: c => { this.cola.set(c); this.cargando.set(false); },
      error: () => { this.cargando.set(false); this.err('No se pudo cargar la cola'); },
    });
  }

  severidad(clave: string): 'danger' | 'warn' | 'success' | 'info' | 'secondary' {
    switch (clave) {
      case 'TRIAGE_1': return 'danger';
      case 'TRIAGE_2': return 'warn';
      case 'TRIAGE_3': return 'warn';
      case 'TRIAGE_4': return 'success';
      case 'TRIAGE_5': return 'info';
      default: return 'secondary';
    }
  }

  abrirRegistro(): void {
    this.dialogo
      .open(UrgenciasTriageComponent, { width: '32rem', maxWidth: '95vw' })
      .afterClosed()
      .subscribe((t?: TriageNuevo | null) => { if (t) this.registrar(t); });
  }

  private registrar(t: TriageNuevo): void {
    this.guardando.set(true);
    this.svc.registrar(t).subscribe({
      next: () => { this.guardando.set(false); this.ok('Paciente en cola'); this.cargar(); },
      error: () => { this.guardando.set(false); this.err('No se pudo registrar'); },
    });
  }

  abrirEgreso(p: EntradaCola): void {
    this.dialogo
      .open(UrgenciasEgresoComponent, { width: '28rem', maxWidth: '95vw' })
      .afterClosed()
      .subscribe((e?: EgresoNuevo | null) => { if (e) this.egresar(p, e); });
  }

  private egresar(p: EntradaCola, e: EgresoNuevo): void {
    this.guardando.set(true);
    this.svc.egreso(p.id, e.destino, e.observaciones).subscribe({
      next: () => { this.guardando.set(false); this.ok('Egreso registrado'); this.cargar(); },
      error: () => { this.guardando.set(false); this.err('No se pudo registrar el egreso'); },
    });
  }

  private ok(detail: string): void { this.messages.add({ severity: 'success', summary: 'Listo', detail }); }
  private err(detail: string): void { this.messages.add({ severity: 'error', summary: 'Error', detail }); }
}
