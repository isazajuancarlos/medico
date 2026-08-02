import { Component, OnInit, Inject, ViewChild, inject, signal, ChangeDetectionStrategy } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { MatAutocompleteModule } from '@angular/material/autocomplete';
import { MatButtonModule } from '@angular/material/button';
import { MatDatepickerModule } from '@angular/material/datepicker';
import { MAT_DIALOG_DATA, MatDialog, MatDialogModule } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatPaginator, MatPaginatorModule } from '@angular/material/paginator';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatTableDataSource, MatTableModule } from '@angular/material/table';
import { MatTooltipModule } from '@angular/material/tooltip';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { Avisos } from '../../shared/ui/avisos.service';
import { diaLocal } from '../../shared/fecha';
import { CitaService } from '../../services/cita.service';
import { PacienteService } from '../../services/paciente.service';
import { Appointment, AppointmentCreate, MedicoDisponible } from '../../models/appointment.models';

/** Un paciente tal como lo propone el buscador del diálogo. */
interface PacienteSugerido {
  id: number;
  label: string;
  identificacion: string;
}

/**
 * Alta de una cita. Devuelve la cita completa por afterClosed(), o nada.
 *
 * El formulario vive DENTRO del diálogo —es el patrón (b) de
 * MIGRACION_MATERIAL.md—: con p-dialog los ocho campos eran de la pantalla
 * anfitriona y openNew() no hacía más que limpiarlos. Ahora el componente nace
 * vacío y muere con el diálogo, así que cerrar por Escape no deja media cita
 * esperando al siguiente paciente.
 */
@Component({
  selector: 'app-cita-nueva',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatSelectModule, MatDatepickerModule, MatAutocompleteModule,
  ],
  template: `
    <h2 mat-dialog-title>Nueva cita</h2>
    <mat-dialog-content>
      <div class="rejilla">
        <mat-form-field appearance="outline" subscriptSizing="dynamic" class="ancho">
          <mat-label>Paciente *</mat-label>
          <input matInput [matAutocomplete]="ac" [(ngModel)]="paciente"
            (ngModelChange)="buscar($event)"
            placeholder="Buscar por nombre o identificación…" />
          <mat-autocomplete #ac="matAutocomplete" [displayWith]="mostrar">
            @for (p of sugerencias(); track p.id) {
              <mat-option [value]="p">
                {{ p.label }} <small class="tenue">{{ p.identificacion }}</small>
              </mat-option>
            }
          </mat-autocomplete>
        </mat-form-field>

        <mat-form-field appearance="outline" subscriptSizing="dynamic">
          <mat-label>Médico *</mat-label>
          <mat-select [(ngModel)]="medicoId">
            @for (m of medicos; track m.id) {
              <mat-option [value]="m.id">{{ m.nombre }}</mat-option>
            }
          </mat-select>
        </mat-form-field>

        <mat-form-field appearance="outline" subscriptSizing="dynamic">
          <mat-label>Tipo de cita *</mat-label>
          <mat-select [(ngModel)]="tipo">
            @for (t of tipos; track t) {
              <mat-option [value]="t">{{ t }}</mat-option>
            }
          </mat-select>
        </mat-form-field>

        <mat-form-field appearance="outline" subscriptSizing="dynamic">
          <mat-label>Fecha *</mat-label>
          <input matInput [matDatepicker]="cal" [(ngModel)]="fecha" [min]="hoy" />
          <mat-datepicker-toggle matIconSuffix [for]="cal" />
          <mat-datepicker #cal />
        </mat-form-field>

        <mat-form-field appearance="outline" subscriptSizing="dynamic">
          <mat-label>Duración</mat-label>
          <mat-select [(ngModel)]="duracion">
            @for (d of duraciones; track d) {
              <mat-option [value]="d">{{ d }} min</mat-option>
            }
          </mat-select>
        </mat-form-field>

        <!-- Hora a mano: la agenda por turnos del servidor todavía no está
             portada al medico-api. -->
        <mat-form-field appearance="outline" subscriptSizing="dynamic">
          <mat-label>Hora *</mat-label>
          <input matInput type="time" [(ngModel)]="hora" />
        </mat-form-field>

        <mat-form-field appearance="outline" subscriptSizing="dynamic" class="ancho">
          <mat-label>Motivo de consulta</mat-label>
          <input matInput [(ngModel)]="motivo" placeholder="Motivo de la visita" />
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!completo()" [mat-dialog-close]="resultado()">
        <i class="pi pi-check"></i> Agendar cita
      </button>
    </mat-dialog-actions>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .rejilla {
      display:grid; grid-template-columns:1fr 1fr; gap:1rem; padding-top:.5rem;
      min-width:min(34rem, 80vw);
    }
    .ancho { grid-column:1/-1; }
    .tenue { color:#64748b; }
    mat-form-field { width:100%; }
  `],
})
export class CitaNuevaComponent {
  private pacienteSvc = inject(PacienteService);

  /** Los médicos los trae la pantalla, que ya los había pedido para el filtro:
   *  pedirlos otra vez al abrir el diálogo sería una petición por cada alta. */
  readonly medicos: MedicoDisponible[];

  constructor(@Inject(MAT_DIALOG_DATA) datos: { medicos: MedicoDisponible[] }) {
    this.medicos = datos.medicos;
  }

  readonly tipos = ['Primera Vez', 'Control / Revisión', 'Urgencia'];
  readonly duraciones = [20, 30, 45, 60];
  readonly hoy = new Date();

  /**
   * Mientras se escribe es la cadena tecleada; al elegir una opción pasa a ser
   * el paciente. Esa distinción ES la validación: escribir un nombre a mano y
   * no elegirlo de la lista deja el diálogo incompleto, que era lo que hacía
   * forceSelection en el autocompletado de PrimeNG.
   */
  paciente: PacienteSugerido | string = '';
  sugerencias = signal<PacienteSugerido[]>([]);

  medicoId: number | null = null;
  tipo = 'Primera Vez';
  fecha: Date | null = null;
  duracion = 30;
  hora = '';
  motivo = '';

  readonly mostrar = (p: PacienteSugerido | string): string =>
    typeof p === 'string' ? p : (p?.label ?? '');

  buscar(valor: PacienteSugerido | string): void {
    if (typeof valor !== 'string') return;   // ya eligió: no hay nada que buscar
    const q = valor.trim();
    if (q.length < 2) { this.sugerencias.set([]); return; }
    this.pacienteSvc.buscar(q).subscribe({
      next: (items: any[]) => this.sugerencias.set(items.map((p) => ({
        id: p.id,
        label: `${p.nombre1} ${p.apellido1}`,
        identificacion: p.identificacion,
      }))),
      error: () => this.sugerencias.set([]),
    });
  }

  completo(): boolean {
    return typeof this.paciente === 'object' && !!this.medicoId && !!this.fecha && !!this.hora;
  }

  /** Lo que sale por afterClosed(). Null si falta algo: el botón ya está
   *  inerte, pero un diálogo capaz de cerrarse con media cita la agendaría sin
   *  paciente. */
  resultado(): AppointmentCreate | null {
    if (!this.completo()) return null;
    return {
      paciente_id: (this.paciente as PacienteSugerido).id,
      medico_id: this.medicoId!,
      fecha: diaLocal(this.fecha!),
      hora: this.hora,
      tipo: this.tipo as any,
      motivo: this.motivo,
      duracion_minutos: this.duracion,
    };
  }
}

@Component({
  selector: 'app-citas',
  standalone: true,
  imports: [
    FormsModule, MatTableModule, MatPaginatorModule, MatButtonModule,
    MatFormFieldModule, MatInputModule, MatSelectModule, MatDatepickerModule,
    MatProgressBarModule, MatTooltipModule, EtiquetaComponent,
  ],
  template: `
<div class="page-card">
  <div class="cabecera">
    <h2 class="page-title">Agenda de Citas</h2>
    <button matButton="filled" (click)="nueva()">
      <i class="pi pi-plus"></i> Nueva cita
    </button>
  </div>

  <div class="filtros">
    <mat-form-field appearance="outline" subscriptSizing="dynamic">
      <mat-label>Filtrar por fecha</mat-label>
      <input matInput [matDatepicker]="cal" [(ngModel)]="filtroFecha" (dateChange)="cargar()" />
      @if (filtroFecha) {
        <button matIconButton matSuffix matTooltip="Quitar el filtro"
          (click)="filtroFecha = null; cargar()">
          <i class="pi pi-times"></i>
        </button>
      }
      <mat-datepicker-toggle matIconSuffix [for]="cal" />
      <mat-datepicker #cal />
    </mat-form-field>

    <mat-form-field appearance="outline" subscriptSizing="dynamic">
      <mat-label>Médico</mat-label>
      <mat-select [(ngModel)]="filtroMedico" (selectionChange)="cargar()">
        <mat-option [value]="null">Todos los médicos</mat-option>
        @for (m of medicos(); track m.id) {
          <mat-option [value]="m.id">{{ m.nombre }}</mat-option>
        }
      </mat-select>
    </mat-form-field>

    <mat-form-field appearance="outline" subscriptSizing="dynamic">
      <mat-label>Estado</mat-label>
      <mat-select [(ngModel)]="filtroEstado" (selectionChange)="cargar()">
        <mat-option [value]="null">Todos los estados</mat-option>
        @for (e of estados; track e) {
          <mat-option [value]="e">{{ e }}</mat-option>
        }
      </mat-select>
    </mat-form-field>
  </div>

  @if (cargando()) { <mat-progress-bar mode="indeterminate" /> }

  <table mat-table [dataSource]="datos" class="tabla">
    <ng-container matColumnDef="fecha">
      <th mat-header-cell *matHeaderCellDef>Fecha</th>
      <td mat-cell *matCellDef="let c">{{ c.fecha }}</td>
    </ng-container>
    <ng-container matColumnDef="hora">
      <th mat-header-cell *matHeaderCellDef>Hora</th>
      <td mat-cell *matCellDef="let c">{{ c.hora }}</td>
    </ng-container>
    <ng-container matColumnDef="paciente">
      <th mat-header-cell *matHeaderCellDef>Paciente</th>
      <td mat-cell *matCellDef="let c">{{ c.paciente_nombre || c.paciente_id }}</td>
    </ng-container>
    <ng-container matColumnDef="medico">
      <th mat-header-cell *matHeaderCellDef>Médico</th>
      <td mat-cell *matCellDef="let c">{{ c.medico_nombre || c.medico_id }}</td>
    </ng-container>
    <ng-container matColumnDef="tipo">
      <th mat-header-cell *matHeaderCellDef>Tipo</th>
      <!-- «—» y no una celda vacía: la cita anterior a la migración 0035 no
           trae tipo, y un hueco se lee como un fallo de la pantalla. -->
      <td mat-cell *matCellDef="let c">{{ c.tipo || '—' }}</td>
    </ng-container>
    <ng-container matColumnDef="motivo">
      <th mat-header-cell *matHeaderCellDef>Motivo</th>
      <td mat-cell *matCellDef="let c">{{ c.motivo }}</td>
    </ng-container>
    <ng-container matColumnDef="estado">
      <th mat-header-cell *matHeaderCellDef>Estado</th>
      <td mat-cell *matCellDef="let c">
        <x-etiqueta [value]="c.estado" [severity]="estadoSeverity(c.estado)" />
      </td>
    </ng-container>
    <ng-container matColumnDef="acciones">
      <th mat-header-cell *matHeaderCellDef>Acciones</th>
      <td mat-cell *matCellDef="let c">
        @if (c.estado === 'Pendiente' || c.estado === 'Confirmada') {
          <button matButton (click)="atender(c)" [disabled]="atendiendo() === c.id">
            <i [class]="atendiendo() === c.id ? 'pi pi-spin pi-spinner' : 'pi pi-play'"></i>
            Atender
          </button>
          <button matIconButton class="peligro" matTooltip="Cancelar cita" (click)="cancelar(c)">
            <i class="pi pi-times"></i>
          </button>
        }
      </td>
    </ng-container>

    <tr mat-header-row *matHeaderRowDef="columnas"></tr>
    <tr mat-row *matRowDef="let fila; columns: columnas"></tr>
  </table>

  <!-- El «no hay nada» va fuera de la tabla: mat-table no hereda la plantilla
       de vacío de p-table, y una tabla sin filas y sin frase se lee como
       «todavía está cargando». -->
  @if (!cargando() && datos.data.length === 0) {
    <p class="vacio">No hay citas para los filtros seleccionados.</p>
  }
  <mat-paginator [pageSize]="20" [pageSizeOptions]="[10, 20, 50]" />
</div>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .cabecera { display:flex; justify-content:space-between; align-items:center; margin-bottom:1rem; }
    .cabecera .page-title { margin-bottom:0; }
    .filtros { display:flex; gap:.75rem; flex-wrap:wrap; margin-bottom:1rem; }
    .tabla { width:100%; }
    .vacio { text-align:center; padding:2rem; color:#64748b; }
    .peligro { color:#dc2626; }
  `],
})
export class CitasComponent implements OnInit {
  private svc = inject(CitaService);
  private dialogo = inject(MatDialog);
  private msg = inject(Avisos);

  /** El orden de las columnas lo fija esta lista, no la plantilla. */
  readonly columnas = ['fecha', 'hora', 'paciente', 'medico', 'tipo', 'motivo', 'estado', 'acciones'];
  readonly datos = new MatTableDataSource<Appointment>([]);

  /** Por setter: la tabla convive con un @if de carga, así que el paginador
   *  aparece y desaparece. Ver MIGRACION_MATERIAL.md. */
  @ViewChild(MatPaginator)
  set paginador(p: MatPaginator | undefined) { if (p) this.datos.paginator = p; }

  medicos = signal<MedicoDisponible[]>([]);
  cargando = signal(true);
  atendiendo = signal<number | null>(null);

  readonly estados = ['Pendiente', 'Confirmada', 'Atendida', 'Cancelada', 'No Asistió'];

  filtroFecha: Date | null = null;
  filtroMedico: number | null = null;
  filtroEstado: string | null = null;

  ngOnInit(): void {
    this.cargar();
    this.svc.getMedicos().subscribe(m => this.medicos.set(m));
  }

  cargar(): void {
    this.cargando.set(true);
    const fecha = this.filtroFecha ? diaLocal(this.filtroFecha) : undefined;
    this.svc.list(1, 100, fecha, this.filtroMedico ?? undefined, this.filtroEstado ?? undefined)
      .subscribe({
        next: r => { this.datos.data = r.items; this.cargando.set(false); },
        error: () => this.cargando.set(false),
      });
  }

  nueva(): void {
    this.dialogo
      .open(CitaNuevaComponent, { data: { medicos: this.medicos() }, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((cita?: AppointmentCreate) => { if (cita) this.guardar(cita); });
  }

  private guardar(cita: AppointmentCreate): void {
    this.svc.create(cita).subscribe({
      next: () => {
        this.msg.add({ severity: 'success', summary: 'Cita agendada', detail: 'La cita fue registrada exitosamente' });
        this.cargar();
      },
      error: (err) => {
        const detail = err?.error?.detail ?? 'No se pudo agendar la cita';
        this.msg.add({ severity: 'error', summary: 'Error', detail });
      },
    });
  }

  atender(cita: Appointment): void {
    this.atendiendo.set(cita.id);
    // La apertura de la historia clínica llegará cuando ese módulo se porte; por
    // ahora, atender = registrar el estado ATENDIDA.
    this.svc.marcarAtendida(cita.id).subscribe({
      next: () => {
        this.atendiendo.set(null);
        this.msg.add({ severity: 'success', summary: 'Cita atendida' });
        this.cargar();
      },
      error: () => this.atendiendo.set(null),
    });
  }

  cancelar(cita: Appointment): void {
    this.svc.cancel(cita.id).subscribe({
      next: () => {
        this.msg.add({ severity: 'info', summary: 'Cita cancelada' });
        this.cargar();
      },
    });
  }

  estadoSeverity(estado: string): 'info' | 'success' | 'warn' | 'danger' | 'secondary' {
    const map: Record<string, any> = {
      'Pendiente': 'warn', 'Confirmada': 'info',
      'Atendida': 'success', 'Cancelada': 'danger', 'No Asistió': 'secondary',
    };
    return map[estado] ?? 'secondary';
  }
}
