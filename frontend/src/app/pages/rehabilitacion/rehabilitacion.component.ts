import { Component, OnInit } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { TableModule } from 'primeng/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { CardModule } from 'primeng/card';
import { InputTextModule } from 'primeng/inputtext';
import { TextareaModule } from 'primeng/textarea';
import { SelectModule } from 'primeng/select';
import { InputNumberModule } from 'primeng/inputnumber';
import { ProgressBarModule } from 'primeng/progressbar';
import { Avisos } from '../../shared/ui/avisos.service';
import { RehabilitacionService, PlanRehabilitacion, SesionTerapia, PlanCreate, SesionCreate } from '../../services/rehabilitacion.service';

type Vista = 'lista' | 'nuevo' | 'detalle';

@Component({
  selector: 'app-rehabilitacion',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    ButtonModule, TableModule, EtiquetaComponent, CardModule,
    InputTextModule, TextareaModule, SelectModule, InputNumberModule,
    ProgressBarModule,
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
    <div class="mb-6" style="display:grid; grid-template-columns: auto auto 1fr; gap: 0.75rem; align-items: center;">
      <p-select [options]="tiposTerapia" [(ngModel)]="filtroTipo" placeholder="Tipo de terapia"
        (onChange)="cargarPlanes()" [showClear]="true" />
      <p-select [options]="opcionesActivo" [(ngModel)]="filtroActivo" placeholder="Estado"
        (onChange)="cargarPlanes()" [showClear]="true" />
      <div style="display:flex; justify-content:flex-end;">
        <p-button label="Nuevo Plan" icon="pi pi-plus" (onClick)="irANuevo()" />
      </div>
    </div>

    <div class="mt-4">
    <p-table [value]="planes" [loading]="loading" [paginator]="true" [rows]="15"
      [rowsPerPageOptions]="[10,15,25]" stripedRows responsiveLayout="scroll">
      <ng-template #header>
        <tr>
          <th>Paciente</th>
          <th>Tipo</th>
          <th>Diagnóstico funcional</th>
          <th>Progreso</th>
          <th>Frecuencia</th>
          <th>Estado</th>
          <th></th>
        </tr>
      </ng-template>
      <ng-template #body let-p>
        <tr>
          <td>
            <div class="font-semibold">{{ p.paciente_nombre }}</div>
            <div class="text-xs text-gray-400">{{ p.paciente_identificacion }}</div>
          </td>
          <td>
            <span class="font-medium text-sm px-2 py-1 rounded"
              [ngClass]="tipoClass(p.tipo_terapia)">
              {{ p.tipo_terapia }}
            </span>
          </td>
          <td class="max-w-xs truncate text-sm">{{ p.diagnostico_funcional }}</td>
          <td class="min-w-[140px]">
            <div class="flex items-center gap-2">
              <p-progressBar [value]="progresoPct(p)" [showValue]="false"
                styleClass="h-2 flex-1" />
              <span class="text-xs text-gray-500 whitespace-nowrap">{{ p.sesiones_realizadas }}/{{ p.sesiones_autorizadas }}</span>
            </div>
          </td>
          <td class="text-sm">{{ p.frecuencia }}</td>
          <td>
            <x-etiqueta [value]="p.activo ? 'Activo' : 'Cerrado'"
              [severity]="p.activo ? 'success' : 'secondary'" />
          </td>
          <td>
            <p-button icon="pi pi-eye" [text]="true" size="small"
              (onClick)="verDetalle(p)" matTooltip="Ver detalle" />
          </td>
        </tr>
      </ng-template>
      <ng-template #empty>
        <tr><td colspan="7" class="text-center py-8 text-gray-400">No hay planes registrados</td></tr>
      </ng-template>
    </p-table>
    </div>
  </ng-container>

  <!-- NUEVO PLAN -->
  <ng-container *ngIf="vista === 'nuevo'">
    <div class="flex items-center gap-3 mb-6">
      <p-button icon="pi pi-arrow-left" [text]="true" (onClick)="vista = 'lista'" />
      <div>
        <h2 class="text-2xl font-bold text-gray-800 dark:text-gray-100">Nuevo Plan de Rehabilitación</h2>
      </div>
    </div>

    <p-card>
      <!-- Búsqueda de paciente -->
      <div class="mb-6">
        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Paciente *</label>
        <div class="flex gap-2">
          <input pInputText [(ngModel)]="busquedaPaciente" placeholder="Buscar por nombre o identificación..."
            class="flex-1" (keyup.enter)="buscarPaciente()" />
          <p-button icon="pi pi-search" (onClick)="buscarPaciente()" [loading]="buscando" />
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
        <div>
          <label class="block text-sm font-medium mb-1">Tipo de Terapia *</label>
          <p-select [options]="tiposTerapia" [(ngModel)]="f.tipo_terapia" placeholder="Seleccionar..." class="w-full" />
        </div>
        <div>
          <label class="block text-sm font-medium mb-1">Frecuencia *</label>
          <input pInputText [(ngModel)]="f.frecuencia" placeholder="Ej: 3 veces/semana" class="w-full" />
        </div>
        <div>
          <label class="block text-sm font-medium mb-1">Sesiones Autorizadas *</label>
          <p-inputNumber [(ngModel)]="f.sesiones_autorizadas" [min]="1" [max]="100" class="w-full" />
        </div>
        <div>
          <label class="block text-sm font-medium mb-1">Duración sesión (min)</label>
          <p-inputNumber [(ngModel)]="f.duracion_sesion_min" [min]="15" [max]="120" [step]="5" class="w-full" />
        </div>
        <div class="md:col-span-2">
          <label class="block text-sm font-medium mb-1">Diagnóstico Funcional *</label>
          <textarea pTextarea [(ngModel)]="f.diagnostico_funcional" rows="3" class="w-full"
            placeholder="Diagnóstico funcional del paciente..."></textarea>
        </div>
        <div class="md:col-span-2">
          <label class="block text-sm font-medium mb-1">Objetivos Terapéuticos *</label>
          <textarea pTextarea [(ngModel)]="f.objetivos" rows="3" class="w-full"
            placeholder="Objetivos del plan de rehabilitación..."></textarea>
        </div>
      </div>

      <div class="flex justify-end gap-3 mt-6">
        <p-button label="Cancelar" severity="secondary" (onClick)="vista = 'lista'" />
        <p-button label="Crear Plan" icon="pi pi-save" (onClick)="guardarPlan()" [loading]="guardando" />
      </div>
    </p-card>
  </ng-container>

  <!-- DETALLE PLAN -->
  <ng-container *ngIf="vista === 'detalle' && planActual">
    <div class="flex items-center gap-3 mb-4">
      <p-button icon="pi pi-arrow-left" [text]="true" (onClick)="vista = 'lista'" />
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
        <p-progressBar [value]="progresoPct(planActual)" [showValue]="false" styleClass="h-1 mt-1" />
      </div>
      <div class="p-4 rounded-xl border border-orange-200 dark:border-orange-800 bg-orange-50 dark:bg-orange-900/20">
        <div class="text-xs text-orange-500 uppercase tracking-wide mb-1">Duración sesión</div>
        <div class="font-semibold text-gray-800 dark:text-gray-100">{{ planActual.duracion_sesion_min }} min</div>
      </div>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
      <p-card header="Diagnóstico Funcional">
        <p class="text-sm text-gray-700 dark:text-gray-300 whitespace-pre-wrap">{{ planActual.diagnostico_funcional }}</p>
      </p-card>
      <p-card header="Objetivos Terapéuticos">
        <p class="text-sm text-gray-700 dark:text-gray-300 whitespace-pre-wrap">{{ planActual.objetivos }}</p>
      </p-card>
    </div>

    <!-- Sesiones -->
    <p-card>
      <ng-template #header>
        <div class="flex justify-between items-center p-4 pb-0">
          <h3 class="font-bold text-lg">Sesiones Realizadas</h3>
          <p-button *ngIf="planActual.activo" label="Registrar Sesión" icon="pi pi-plus"
            size="small" (onClick)="mostrarFormSesion = true" />
        </div>
      </ng-template>

      <!-- Form nueva sesión -->
      <div *ngIf="mostrarFormSesion" class="mb-6 p-4 border rounded-xl bg-gray-50 dark:bg-gray-800">
        <h4 class="font-semibold mb-3">Sesión #{{ (planActual.sesiones_realizadas + 1) }}</h4>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div class="md:col-span-2">
            <label class="block text-sm font-medium mb-1">Evaluación Funcional *</label>
            <textarea pTextarea [(ngModel)]="fs.evaluacion_funcional" rows="3" class="w-full"
              placeholder="Estado funcional del paciente en esta sesión..."></textarea>
          </div>
          <div>
            <label class="block text-sm font-medium mb-1">Dolor EVA (0-10)</label>
            <p-inputNumber [(ngModel)]="fs.dolor_eva" [min]="0" [max]="10" class="w-full" />
          </div>
          <div>
            <label class="block text-sm font-medium mb-1">Rango de Movimiento</label>
            <input pInputText [(ngModel)]="fs.rango_movimiento" class="w-full"
              placeholder="Ej: Flexión 90°, Extensión 30°" />
          </div>
          <div class="md:col-span-2">
            <label class="block text-sm font-medium mb-1">Técnicas Aplicadas *</label>
            <textarea pTextarea [(ngModel)]="fs.tecnicas_aplicadas" rows="2" class="w-full"
              placeholder="Técnicas y procedimientos realizados..."></textarea>
          </div>
          <div>
            <label class="block text-sm font-medium mb-1">Progreso</label>
            <p-select [options]="opcionesProgreso" [(ngModel)]="fs.progreso"
              placeholder="Seleccionar..." class="w-full" />
          </div>
          <div>
            <label class="block text-sm font-medium mb-1">Ejercicios en Casa</label>
            <input pInputText [(ngModel)]="fs.ejercicios_domicilio" class="w-full"
              placeholder="Indicaciones para el hogar" />
          </div>
          <div class="md:col-span-2">
            <label class="block text-sm font-medium mb-1">Observaciones</label>
            <textarea pTextarea [(ngModel)]="fs.observaciones" rows="2" class="w-full"></textarea>
          </div>
        </div>
        <div class="flex gap-3 justify-end mt-4">
          <p-button label="Cancelar" severity="secondary" size="small"
            (onClick)="cancelarSesion()" />
          <p-button label="Guardar Sesión" icon="pi pi-save" size="small"
            (onClick)="guardarSesion()" [loading]="guardandoSesion" />
        </div>
      </div>

      <p-table [value]="sesiones" [loading]="loadingSesiones" responsiveLayout="scroll">
        <ng-template #header>
          <tr>
            <th style="width:60px">#</th>
            <th>Fecha</th>
            <th>Evaluación funcional</th>
            <th>Técnicas</th>
            <th>Dolor EVA</th>
            <th>Progreso</th>
          </tr>
        </ng-template>
        <ng-template #body let-s>
          <tr>
            <td class="text-center font-bold text-blue-600">{{ s.numero_sesion }}</td>
            <td class="text-sm">{{ s.fecha_sesion | date:'dd/MM/yyyy HH:mm' }}</td>
            <td class="text-sm max-w-xs">{{ s.evaluacion_funcional }}</td>
            <td class="text-sm max-w-xs">{{ s.tecnicas_aplicadas }}</td>
            <td class="text-center">
              <span *ngIf="s.dolor_eva !== null && s.dolor_eva !== undefined"
                class="px-2 py-1 rounded text-xs font-bold"
                [ngClass]="dolorClass(s.dolor_eva)">{{ s.dolor_eva }}</span>
            </td>
            <td>
              <x-etiqueta *ngIf="s.progreso" [value]="s.progreso"
                [severity]="progresoSeverity(s.progreso)" />
            </td>
          </tr>
        </ng-template>
        <ng-template #empty>
          <tr><td colspan="6" class="text-center py-6 text-gray-400">No hay sesiones registradas</td></tr>
        </ng-template>
      </p-table>
    </p-card>

    <!-- Resultado final (si cerrado) -->
    <div *ngIf="!planActual.activo && planActual.resultado_final" class="mt-4 p-4 bg-gray-50 dark:bg-gray-800 rounded-xl border">
      <h4 class="font-semibold mb-2">Resultado Final</h4>
      <p class="text-sm text-gray-700 dark:text-gray-300 whitespace-pre-wrap">{{ planActual.resultado_final }}</p>
    </div>

    <!-- Cerrar plan -->
    <div *ngIf="planActual.activo && !mostrarCierre" class="mt-4 flex justify-end">
      <p-button label="Cerrar Plan" icon="pi pi-check-circle" severity="secondary"
        (onClick)="mostrarCierre = true" />
    </div>
    <div *ngIf="mostrarCierre" class="mt-4 p-4 border rounded-xl bg-yellow-50 dark:bg-yellow-900/20">
      <h4 class="font-semibold mb-3">Cerrar Plan de Rehabilitación</h4>
      <label class="block text-sm font-medium mb-1">Resultado Final</label>
      <textarea pTextarea [(ngModel)]="resultadoFinal" rows="3" class="w-full mb-3"
        placeholder="Resumen de resultados y estado final del paciente..."></textarea>
      <div class="flex gap-3 justify-end">
        <p-button label="Cancelar" severity="secondary" size="small" (onClick)="mostrarCierre = false" />
        <p-button label="Confirmar Cierre" icon="pi pi-lock" size="small"
          severity="warn" (onClick)="cerrarPlan()" [loading]="guardando" />
      </div>
    </div>
  </ng-container>
</div>
  `,
})
export class RehabilitacionComponent implements OnInit {
  vista: Vista = 'lista';
  planes: PlanRehabilitacion[] = [];
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
      next: p => { this.planes = p; this.loading = false; },
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
        this.planes = [p, ...this.planes];
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
        const idx = this.planes.findIndex(x => x.id === p.id);
        if (idx >= 0) this.planes[idx] = p;
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
