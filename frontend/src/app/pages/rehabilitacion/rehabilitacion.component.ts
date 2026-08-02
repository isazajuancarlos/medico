import { Component, OnInit, ViewChild, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatCardModule } from '@angular/material/card';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatPaginator, MatPaginatorModule } from '@angular/material/paginator';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatTableDataSource, MatTableModule } from '@angular/material/table';
import { MatTooltipModule } from '@angular/material/tooltip';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { Avisos } from '../../shared/ui/avisos.service';
import { RehabilitacionService, PlanRehabilitacion, SesionTerapia, PlanCreate, SesionCreate } from '../../services/rehabilitacion.service';

type Vista = 'lista' | 'nuevo' | 'detalle';

@Component({
  selector: 'app-rehabilitacion',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatCardModule, MatFormFieldModule, MatInputModule,
    MatPaginatorModule, MatProgressBarModule, MatSelectModule, MatTableModule,
    MatTooltipModule, EtiquetaComponent,
  ],
  template: `
<div class="p-4">

  <!-- LISTA -->
  <ng-container *ngIf="vista === 'lista'">
    <div class="mb-4">
      <h2 class="text-2xl font-bold text-gray-800 dark:text-gray-100">Rehabilitación y Terapias</h2>
      <p class="text-gray-500 text-sm">Planes de fisioterapia, terapia ocupacional, lenguaje y respiratoria</p>
    </div>

    <!-- Filtros + Botón -->
    <div class="mb-6 filtros">
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Tipo de terapia</mat-label>
        <mat-select [(ngModel)]="filtroTipo" (selectionChange)="cargarPlanes()">
          <mat-option value="">Todos los tipos</mat-option>
          <mat-option *ngFor="let t of tiposTerapia" [value]="t.value">{{ t.label }}</mat-option>
        </mat-select>
      </mat-form-field>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Estado</mat-label>
        <mat-select [(ngModel)]="filtroActivo" (selectionChange)="cargarPlanes()">
          <mat-option [value]="null">Todos</mat-option>
          <mat-option *ngFor="let o of opcionesActivo" [value]="o.value">{{ o.label }}</mat-option>
        </mat-select>
      </mat-form-field>
      <div style="display:flex; justify-content:flex-end;">
        <button matButton="filled" (click)="irANuevo()">
          <i class="pi pi-plus"></i> Nuevo plan
        </button>
      </div>
    </div>

    <div class="mt-4">
    <mat-progress-bar *ngIf="loading" mode="indeterminate" />
    <table mat-table [dataSource]="datosPlanes" class="tabla">
      <ng-container matColumnDef="paciente">
        <th mat-header-cell *matHeaderCellDef>Paciente</th>
        <td mat-cell *matCellDef="let p">
          <div class="font-semibold">{{ p.paciente_nombre }}</div>
          <div class="text-xs text-gray-400">{{ p.paciente_identificacion }}</div>
        </td>
      </ng-container>
      <ng-container matColumnDef="tipo">
        <th mat-header-cell *matHeaderCellDef>Tipo</th>
        <td mat-cell *matCellDef="let p">
          <span class="font-medium text-sm px-2 py-1 rounded" [ngClass]="tipoClass(p.tipo_terapia)">
            {{ p.tipo_terapia }}
          </span>
        </td>
      </ng-container>
      <ng-container matColumnDef="diagnostico">
        <th mat-header-cell *matHeaderCellDef>Diagnóstico funcional</th>
        <td mat-cell *matCellDef="let p" class="max-w-xs truncate text-sm">{{ p.diagnostico_funcional }}</td>
      </ng-container>
      <ng-container matColumnDef="progreso">
        <th mat-header-cell *matHeaderCellDef>Progreso</th>
        <td mat-cell *matCellDef="let p">
          <div class="barra">
            <mat-progress-bar mode="determinate" [value]="progresoPct(p)" />
            <span class="text-xs text-gray-500 whitespace-nowrap">{{ p.sesiones_realizadas }}/{{ p.sesiones_autorizadas }}</span>
          </div>
        </td>
      </ng-container>
      <ng-container matColumnDef="frecuencia">
        <th mat-header-cell *matHeaderCellDef>Frecuencia</th>
        <td mat-cell *matCellDef="let p" class="text-sm">{{ p.frecuencia }}</td>
      </ng-container>
      <ng-container matColumnDef="estado">
        <th mat-header-cell *matHeaderCellDef>Estado</th>
        <td mat-cell *matCellDef="let p">
          <x-etiqueta [value]="p.activo ? 'Activo' : 'Cerrado'"
            [severity]="p.activo ? 'success' : 'secondary'" />
        </td>
      </ng-container>
      <ng-container matColumnDef="acciones">
        <th mat-header-cell *matHeaderCellDef></th>
        <td mat-cell *matCellDef="let p">
          <button matIconButton matTooltip="Ver detalle" (click)="verDetalle(p)">
            <i class="pi pi-eye"></i>
          </button>
        </td>
      </ng-container>

      <tr mat-header-row *matHeaderRowDef="columnasPlanes"></tr>
      <tr mat-row *matRowDef="let fila; columns: columnasPlanes"></tr>
    </table>
    <!-- El vacío va fuera de la tabla: mat-table no hereda la plantilla de
         vacío de p-table. -->
    <p *ngIf="!loading && datosPlanes.data.length === 0" class="vacio">No hay planes registrados</p>
    <mat-paginator [pageSize]="15" [pageSizeOptions]="[10, 15, 25]" />
    </div>
  </ng-container>

  <!-- NUEVO PLAN -->
  <ng-container *ngIf="vista === 'nuevo'">
    <div class="flex items-center gap-3 mb-6">
      <button matIconButton (click)="vista = 'lista'"><i class="pi pi-arrow-left"></i></button>
      <div>
        <h2 class="text-2xl font-bold text-gray-800 dark:text-gray-100">Nuevo Plan de Rehabilitación</h2>
      </div>
    </div>

    <mat-card>
      <mat-card-content>
      <!-- Búsqueda de paciente -->
      <div class="mb-6">
        <div class="buscador">
          <mat-form-field appearance="outline" subscriptSizing="dynamic" class="crece">
            <mat-label>Paciente *</mat-label>
            <input matInput [(ngModel)]="busquedaPaciente" (keyup.enter)="buscarPaciente()"
              placeholder="Buscar por nombre o identificación..." />
          </mat-form-field>
          <button matButton="filled" [disabled]="buscando" (click)="buscarPaciente()">
            <i [class]="buscando ? 'pi pi-spin pi-spinner' : 'pi pi-search'"></i>
          </button>
        </div>
        <div *ngIf="resultadosPaciente.length" class="border rounded mt-2 bg-white dark:bg-gray-800 shadow max-h-48 overflow-y-auto">
          <div *ngFor="let pac of resultadosPaciente"
            class="px-3 py-2 cursor-pointer hover:bg-blue-50 dark:hover:bg-gray-700"
            (click)="seleccionarPaciente(pac)">
            <span class="font-medium">{{ pac.nombre_completo }}</span>
            <span class="text-xs text-gray-400 ml-2">{{ pac.identificacion }}</span>
          </div>
        </div>
        <div *ngIf="pacienteSeleccionado" class="mt-2 p-2 bg-blue-50 dark:bg-blue-900 rounded flex items-center gap-2">
          <i class="pi pi-user text-blue-500"></i>
          <span class="font-medium text-blue-700 dark:text-blue-200">{{ pacienteSeleccionado.nombre_completo }}</span>
          <span class="text-xs text-blue-500 ml-1">{{ pacienteSeleccionado.identificacion }}</span>
        </div>
      </div>

      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <mat-form-field appearance="outline">
          <mat-label>Tipo de terapia *</mat-label>
          <mat-select [(ngModel)]="f.tipo_terapia">
            <mat-option *ngFor="let t of tiposTerapia" [value]="t.value">{{ t.label }}</mat-option>
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Frecuencia *</mat-label>
          <input matInput [(ngModel)]="f.frecuencia" placeholder="Ej: 3 veces/semana" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Sesiones autorizadas *</mat-label>
          <input matInput type="number" min="1" max="100" [(ngModel)]="f.sesiones_autorizadas" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Duración sesión (min)</mat-label>
          <input matInput type="number" min="15" max="120" step="5" [(ngModel)]="f.duracion_sesion_min" />
        </mat-form-field>
        <mat-form-field appearance="outline" class="md:col-span-2">
          <mat-label>Diagnóstico funcional *</mat-label>
          <textarea matInput [(ngModel)]="f.diagnostico_funcional" rows="3"
            placeholder="Diagnóstico funcional del paciente..."></textarea>
        </mat-form-field>
        <mat-form-field appearance="outline" class="md:col-span-2">
          <mat-label>Objetivos terapéuticos *</mat-label>
          <textarea matInput [(ngModel)]="f.objetivos" rows="3"
            placeholder="Objetivos del plan de rehabilitación..."></textarea>
        </mat-form-field>
      </div>

      <div class="flex justify-end gap-3 mt-6">
        <button matButton (click)="vista = 'lista'">Cancelar</button>
        <button matButton="filled" [disabled]="guardando" (click)="guardarPlan()">
          <i [class]="guardando ? 'pi pi-spin pi-spinner' : 'pi pi-save'"></i> Crear plan
        </button>
      </div>
      </mat-card-content>
    </mat-card>
  </ng-container>

  <!-- DETALLE PLAN -->
  <ng-container *ngIf="vista === 'detalle' && planActual">
    <div class="flex items-center gap-3 mb-4">
      <button matIconButton (click)="vista = 'lista'"><i class="pi pi-arrow-left"></i></button>
      <div class="flex-1">
        <h2 class="text-2xl font-bold text-gray-800 dark:text-gray-100">Plan de Rehabilitación</h2>
        <p class="text-gray-500 text-sm">{{ planActual.paciente_nombre }} · {{ planActual.tipo_terapia }}</p>
      </div>
      <x-etiqueta [value]="planActual.activo ? 'Activo' : 'Cerrado'"
        [severity]="planActual.activo ? 'success' : 'secondary'" />
    </div>

    <!-- Info del plan -->
    <div class="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
      <div class="p-4 rounded-xl border border-blue-200 dark:border-blue-800 bg-blue-50 dark:bg-blue-900/20">
        <div class="text-xs text-blue-500 uppercase tracking-wide mb-1">Terapeuta</div>
        <div class="font-semibold text-gray-800 dark:text-gray-100 text-sm">{{ planActual.terapeuta_nombre }}</div>
      </div>
      <div class="p-4 rounded-xl border border-purple-200 dark:border-purple-800 bg-purple-50 dark:bg-purple-900/20">
        <div class="text-xs text-purple-500 uppercase tracking-wide mb-1">Frecuencia</div>
        <div class="font-semibold text-gray-800 dark:text-gray-100 text-sm">{{ planActual.frecuencia }}</div>
      </div>
      <div class="p-4 rounded-xl border border-green-200 dark:border-green-800 bg-green-50 dark:bg-green-900/20">
        <div class="text-xs text-green-500 uppercase tracking-wide mb-1">Sesiones</div>
        <div class="font-semibold text-gray-800 dark:text-gray-100">{{ planActual.sesiones_realizadas }} / {{ planActual.sesiones_autorizadas }}</div>
        <mat-progress-bar mode="determinate" [value]="progresoPct(planActual)" class="mt-1" />
      </div>
      <div class="p-4 rounded-xl border border-orange-200 dark:border-orange-800 bg-orange-50 dark:bg-orange-900/20">
        <div class="text-xs text-orange-500 uppercase tracking-wide mb-1">Duración sesión</div>
        <div class="font-semibold text-gray-800 dark:text-gray-100">{{ planActual.duracion_sesion_min }} min</div>
      </div>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
      <mat-card>
        <mat-card-header><mat-card-title>Diagnóstico funcional</mat-card-title></mat-card-header>
        <mat-card-content>
          <p class="text-sm text-gray-700 dark:text-gray-300 whitespace-pre-wrap">{{ planActual.diagnostico_funcional }}</p>
        </mat-card-content>
      </mat-card>
      <mat-card>
        <mat-card-header><mat-card-title>Objetivos terapéuticos</mat-card-title></mat-card-header>
        <mat-card-content>
          <p class="text-sm text-gray-700 dark:text-gray-300 whitespace-pre-wrap">{{ planActual.objetivos }}</p>
        </mat-card-content>
      </mat-card>
    </div>

    <!-- Sesiones -->
    <mat-card>
      <mat-card-header>
        <mat-card-title>Sesiones realizadas</mat-card-title>
        <button matButton="filled" *ngIf="planActual.activo" (click)="mostrarFormSesion = true">
          <i class="pi pi-plus"></i> Registrar sesión
        </button>
      </mat-card-header>
      <mat-card-content>

      <!-- Form nueva sesión -->
      <div *ngIf="mostrarFormSesion" class="mb-6 p-4 border rounded-xl bg-gray-50 dark:bg-gray-800">
        <h4 class="font-semibold mb-3">Sesión #{{ (planActual.sesiones_realizadas + 1) }}</h4>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <mat-form-field appearance="outline" class="md:col-span-2">
            <mat-label>Evaluación funcional *</mat-label>
            <textarea matInput [(ngModel)]="fs.evaluacion_funcional" rows="3"
              placeholder="Estado funcional del paciente en esta sesión..."></textarea>
          </mat-form-field>
          <mat-form-field appearance="outline">
            <mat-label>Dolor EVA (0-10)</mat-label>
            <input matInput type="number" min="0" max="10" [(ngModel)]="fs.dolor_eva" />
          </mat-form-field>
          <mat-form-field appearance="outline">
            <mat-label>Rango de movimiento</mat-label>
            <input matInput [(ngModel)]="fs.rango_movimiento" placeholder="Ej: Flexión 90°, Extensión 30°" />
          </mat-form-field>
          <mat-form-field appearance="outline" class="md:col-span-2">
            <mat-label>Técnicas aplicadas *</mat-label>
            <textarea matInput [(ngModel)]="fs.tecnicas_aplicadas" rows="2"
              placeholder="Técnicas y procedimientos realizados..."></textarea>
          </mat-form-field>
          <mat-form-field appearance="outline">
            <mat-label>Progreso</mat-label>
            <mat-select [(ngModel)]="fs.progreso">
              <mat-option *ngFor="let o of opcionesProgreso" [value]="o.value">{{ o.label }}</mat-option>
            </mat-select>
          </mat-form-field>
          <mat-form-field appearance="outline">
            <mat-label>Ejercicios en casa</mat-label>
            <input matInput [(ngModel)]="fs.ejercicios_domicilio" placeholder="Indicaciones para el hogar" />
          </mat-form-field>
          <mat-form-field appearance="outline" class="md:col-span-2">
            <mat-label>Observaciones</mat-label>
            <textarea matInput [(ngModel)]="fs.observaciones" rows="2"></textarea>
          </mat-form-field>
        </div>
        <div class="flex gap-3 justify-end mt-4">
          <button matButton (click)="cancelarSesion()">Cancelar</button>
          <button matButton="filled" [disabled]="guardandoSesion" (click)="guardarSesion()">
            <i [class]="guardandoSesion ? 'pi pi-spin pi-spinner' : 'pi pi-save'"></i> Guardar sesión
          </button>
        </div>
      </div>

      <mat-progress-bar *ngIf="loadingSesiones" mode="indeterminate" />
      <table mat-table [dataSource]="sesiones" class="tabla">
        <ng-container matColumnDef="numero">
          <th mat-header-cell *matHeaderCellDef style="width:60px">#</th>
          <td mat-cell *matCellDef="let s" class="text-center font-bold text-blue-600">{{ s.numero_sesion }}</td>
        </ng-container>
        <ng-container matColumnDef="fecha">
          <th mat-header-cell *matHeaderCellDef>Fecha</th>
          <td mat-cell *matCellDef="let s" class="text-sm">{{ s.fecha_sesion | date:'dd/MM/yyyy HH:mm' }}</td>
        </ng-container>
        <ng-container matColumnDef="evaluacion">
          <th mat-header-cell *matHeaderCellDef>Evaluación funcional</th>
          <td mat-cell *matCellDef="let s" class="text-sm max-w-xs">{{ s.evaluacion_funcional }}</td>
        </ng-container>
        <ng-container matColumnDef="tecnicas">
          <th mat-header-cell *matHeaderCellDef>Técnicas</th>
          <td mat-cell *matCellDef="let s" class="text-sm max-w-xs">{{ s.tecnicas_aplicadas }}</td>
        </ng-container>
        <ng-container matColumnDef="dolor">
          <th mat-header-cell *matHeaderCellDef>Dolor EVA</th>
          <td mat-cell *matCellDef="let s" class="text-center">
            <span *ngIf="s.dolor_eva !== null && s.dolor_eva !== undefined"
              class="px-2 py-1 rounded text-xs font-bold"
              [ngClass]="dolorClass(s.dolor_eva)">{{ s.dolor_eva }}</span>
          </td>
        </ng-container>
        <ng-container matColumnDef="progreso">
          <th mat-header-cell *matHeaderCellDef>Progreso</th>
          <td mat-cell *matCellDef="let s">
            <x-etiqueta *ngIf="s.progreso" [value]="s.progreso" [severity]="progresoSeverity(s.progreso)" />
          </td>
        </ng-container>

        <tr mat-header-row *matHeaderRowDef="columnasSesiones"></tr>
        <tr mat-row *matRowDef="let fila; columns: columnasSesiones"></tr>
      </table>
      <p *ngIf="!loadingSesiones && sesiones.length === 0" class="vacio">No hay sesiones registradas</p>
      </mat-card-content>
    </mat-card>

    <!-- Resultado final (si cerrado) -->
    <div *ngIf="!planActual.activo && planActual.resultado_final" class="mt-4 p-4 bg-gray-50 dark:bg-gray-800 rounded-xl border">
      <h4 class="font-semibold mb-2">Resultado Final</h4>
      <p class="text-sm text-gray-700 dark:text-gray-300 whitespace-pre-wrap">{{ planActual.resultado_final }}</p>
    </div>

    <!-- Cerrar plan -->
    <div *ngIf="planActual.activo && !mostrarCierre" class="mt-4 flex justify-end">
      <button matButton (click)="mostrarCierre = true">
        <i class="pi pi-check-circle"></i> Cerrar plan
      </button>
    </div>
    <div *ngIf="mostrarCierre" class="mt-4 p-4 border rounded-xl bg-yellow-50 dark:bg-yellow-900/20">
      <h4 class="font-semibold mb-3">Cerrar Plan de Rehabilitación</h4>
      <mat-form-field appearance="outline" class="w-full">
        <mat-label>Resultado final</mat-label>
        <textarea matInput [(ngModel)]="resultadoFinal" rows="3"
          placeholder="Resumen de resultados y estado final del paciente..."></textarea>
      </mat-form-field>
      <div class="flex gap-3 justify-end">
        <button matButton (click)="mostrarCierre = false">Cancelar</button>
        <button matButton="filled" class="peligro" [disabled]="guardando" (click)="cerrarPlan()">
          <i [class]="guardando ? 'pi pi-spin pi-spinner' : 'pi pi-lock'"></i> Confirmar cierre
        </button>
      </div>
    </div>
  </ng-container>
</div>
  `,
  changeDetection: ChangeDetectionStrategy.Eager,
  styles: [`
    .filtros { display:grid; grid-template-columns:auto auto 1fr; gap:.75rem; align-items:center; }
    .buscador { display:flex; gap:.5rem; align-items:flex-start; }
    .buscador .crece { flex:1; }
    .barra { display:flex; align-items:center; gap:.5rem; min-width:140px; }
    .barra mat-progress-bar { flex:1; }
    .tabla { width:100%; }
    .vacio { text-align:center; padding:2rem; color:#9ca3af; }
    mat-form-field { width:100%; }
    /* En el tema M3 el input color de un botón no pinta nada: lo destructivo va
       por CSS. Ver MIGRACION_MATERIAL.md. */
    .peligro { background:#dc2626; color:#fff; }
  `],
})
export class RehabilitacionComponent implements OnInit {
  vista: Vista = 'lista';
  readonly columnasPlanes = ['paciente', 'tipo', 'diagnostico', 'progreso', 'frecuencia', 'estado', 'acciones'];
  readonly columnasSesiones = ['numero', 'fecha', 'evaluacion', 'tecnicas', 'dolor', 'progreso'];
  readonly datosPlanes = new MatTableDataSource<PlanRehabilitacion>([]);

  /** Por setter: la tabla vive dentro de un *ngIf de vista, así que el
   *  paginador aparece y desaparece. Ver MIGRACION_MATERIAL.md. */
  @ViewChild(MatPaginator)
  set paginador(p: MatPaginator | undefined) { if (p) this.datosPlanes.paginator = p; }

  sesiones: SesionTerapia[] = [];
  planActual: PlanRehabilitacion | null = null;
  loading = false;
  loadingSesiones = false;
  guardando = false;
  guardandoSesion = false;
  mostrarFormSesion = false;
  mostrarCierre = false;
  resultadoFinal = '';

  filtroTipo = '';
  filtroActivo: boolean | null = null;

  busquedaPaciente = '';
  buscando = false;
  resultadosPaciente: any[] = [];
  pacienteSeleccionado: any = null;

  tiposTerapia = [
    { label: 'Fisioterapia', value: 'FISICA' },
    { label: 'Terapia Ocupacional', value: 'OCUPACIONAL' },
    { label: 'Terapia de Lenguaje', value: 'LENGUAJE' },
    { label: 'Terapia Respiratoria', value: 'RESPIRATORIA' },
  ];

  opcionesActivo = [
    { label: 'Activos', value: true },
    { label: 'Cerrados', value: false },
  ];

  opcionesProgreso = [
    { label: 'Mejoría', value: 'MEJORIA' },
    { label: 'Estable', value: 'ESTABLE' },
    { label: 'Deterioro', value: 'DETERIORO' },
  ];

  f: PlanCreate = this.emptyPlan();
  fs: SesionCreate = this.emptySesion();

  constructor(private svc: RehabilitacionService, private msg: Avisos) {}

  ngOnInit() { this.cargarPlanes(); }

  emptyPlan(): PlanCreate {
    return { paciente_id: 0, diagnostico_funcional: '', objetivos: '', sesiones_autorizadas: 10, tipo_terapia: 'FISICA', frecuencia: '3 veces/semana', duracion_sesion_min: 45 };
  }

  emptySesion(): SesionCreate {
    return { evaluacion_funcional: '', tecnicas_aplicadas: '', dolor_eva: undefined, rango_movimiento: '', progreso: '', ejercicios_domicilio: '', observaciones: '' };
  }

  cargarPlanes() {
    this.loading = true;
    const tipoParam = this.filtroTipo || undefined;
    const activoParam = this.filtroActivo !== null ? this.filtroActivo : undefined;
    this.svc.listPlanes(undefined, tipoParam, activoParam).subscribe({
      next: p => { this.datosPlanes.data = p; this.loading = false; },
      error: () => { this.loading = false; },
    });
  }

  irANuevo() {
    this.f = this.emptyPlan();
    this.pacienteSeleccionado = null;
    this.busquedaPaciente = '';
    this.resultadosPaciente = [];
    this.vista = 'nuevo';
  }

  buscarPaciente() {
    if (!this.busquedaPaciente.trim()) return;
    this.buscando = true;
    this.svc.searchPacientes(this.busquedaPaciente).subscribe({
      next: r => { this.resultadosPaciente = r; this.buscando = false; },
      error: () => { this.buscando = false; },
    });
  }

  seleccionarPaciente(pac: any) {
    this.pacienteSeleccionado = pac;
    this.f.paciente_id = pac.id;
    this.resultadosPaciente = [];
  }

  guardarPlan() {
    if (!this.pacienteSeleccionado) { this.msg.add({ severity: 'warn', summary: 'Seleccione un paciente' }); return; }
    if (!this.f.diagnostico_funcional.trim() || !this.f.objetivos.trim()) {
      this.msg.add({ severity: 'warn', summary: 'Complete los campos obligatorios' }); return;
    }
    this.guardando = true;
    this.svc.createPlan(this.f).subscribe({
      next: p => {
        this.msg.add({ severity: 'success', summary: 'Plan creado exitosamente' });
        this.guardando = false;
        this.datosPlanes.data = [p, ...this.datosPlanes.data];
        this.vista = 'lista';
      },
      error: () => { this.guardando = false; this.msg.add({ severity: 'error', summary: 'Error al crear el plan' }); },
    });
  }

  verDetalle(p: PlanRehabilitacion) {
    this.planActual = p;
    this.mostrarFormSesion = false;
    this.mostrarCierre = false;
    this.resultadoFinal = '';
    this.fs = this.emptySesion();
    this.cargarSesiones(p.id);
    this.vista = 'detalle';
  }

  cargarSesiones(planId: number) {
    this.loadingSesiones = true;
    this.svc.listSesiones(planId).subscribe({
      next: s => { this.sesiones = s; this.loadingSesiones = false; },
      error: () => { this.loadingSesiones = false; },
    });
  }

  guardarSesion() {
    if (!this.fs.evaluacion_funcional.trim() || !this.fs.tecnicas_aplicadas.trim()) {
      this.msg.add({ severity: 'warn', summary: 'Complete los campos obligatorios' }); return;
    }
    this.guardandoSesion = true;
    this.svc.createSesion(this.planActual!.id, this.fs).subscribe({
      next: s => {
        this.sesiones = [...this.sesiones, s];
        if (this.planActual) {
          this.planActual.sesiones_realizadas++;
          if (this.planActual.sesiones_realizadas >= this.planActual.sesiones_autorizadas) {
            this.planActual.activo = false;
          }
        }
        this.fs = this.emptySesion();
        this.mostrarFormSesion = false;
        this.guardandoSesion = false;
        this.msg.add({ severity: 'success', summary: 'Sesión registrada' });
      },
      error: () => { this.guardandoSesion = false; this.msg.add({ severity: 'error', summary: 'Error al guardar la sesión' }); },
    });
  }

  cancelarSesion() {
    this.mostrarFormSesion = false;
    this.fs = this.emptySesion();
  }

  cerrarPlan() {
    this.guardando = true;
    this.svc.updatePlan(this.planActual!.id, { activo: false, resultado_final: this.resultadoFinal }).subscribe({
      next: p => {
        this.planActual = p;
        this.mostrarCierre = false;
        this.guardando = false;
        this.msg.add({ severity: 'success', summary: 'Plan cerrado correctamente' });
        const idx = this.datosPlanes.data.findIndex(x => x.id === p.id);
        if (idx >= 0) {
          const copia = [...this.datosPlanes.data];
          copia[idx] = p;
          this.datosPlanes.data = copia;
        }
      },
      error: () => { this.guardando = false; this.msg.add({ severity: 'error', summary: 'Error al cerrar el plan' }); },
    });
  }

  progresoPct(p: PlanRehabilitacion): number {
    return p.sesiones_autorizadas ? Math.round((p.sesiones_realizadas / p.sesiones_autorizadas) * 100) : 0;
  }

  tipoClass(tipo: string): string {
    const m: Record<string, string> = {
      FISICA: 'bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-200',
      OCUPACIONAL: 'bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-200',
      LENGUAJE: 'bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-200',
      RESPIRATORIA: 'bg-teal-100 text-teal-700 dark:bg-teal-900 dark:text-teal-200',
    };
    return m[tipo] ?? 'bg-gray-100 text-gray-700';
  }

  dolorClass(eva: number): string {
    if (eva <= 3) return 'bg-green-100 text-green-700';
    if (eva <= 6) return 'bg-yellow-100 text-yellow-800';
    return 'bg-red-100 text-red-700';
  }

  progresoSeverity(p: string): 'success' | 'warn' | 'danger' | 'secondary' {
    const m: Record<string, any> = { MEJORIA: 'success', ESTABLE: 'warn', DETERIORO: 'danger' };
    return m[p] ?? 'secondary';
  }
}
