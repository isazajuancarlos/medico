import { Component, OnInit, computed, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { TextareaModule } from 'primeng/textarea';
import { TableModule } from 'primeng/table';
import { TagModule } from 'primeng/tag';
import { ToastModule } from 'primeng/toast';
import { SelectModule } from 'primeng/select';
import { TooltipModule } from 'primeng/tooltip';
import { DialogModule } from 'primeng/dialog';
import { MessageService } from 'primeng/api';
import {
  AmbulanciasService, Ambulancia, AmbulanciaCreate, Traslado, TrasladoCreate,
} from '../../services/ambulancias.service';
import { PacienteService } from '../../services/paciente.service';

type Tab = 'parque' | 'traslados';

/**
 * Transporte asistencial.
 *
 * La pantalla NO decide cuándo una ambulancia puede salir: lo decide el
 * servidor, y aquí se muestra el motivo que devuelve. Lo que sí hace es no
 * ofrecer lo que se sabe que va a fallar —una ambulancia ocupada, con el SOAT
 * vencido o dada de baja no aparece en el selector de despacho—, porque hacer
 * que el usuario descubra la regla chocándose con un 409 es enseñársela dos
 * veces.
 *
 * El SOAT tiene TRES estados y no dos: vigente, vencido y **sin registrar**. El
 * tercero no se pinta como los otros dos ni se calla: no saber no es estar al
 * día, y tampoco es motivo para no despachar una urgencia.
 */
@Component({
  selector: 'app-ambulancias',
  standalone: true,
  providers: [MessageService],
  imports: [
    CommonModule, FormsModule, ButtonModule, InputTextModule, TextareaModule,
    TableModule, TagModule, ToastModule, SelectModule, TooltipModule, DialogModule,
  ],
  template: `
    <div class="page-card">
      <div class="list-header">
        <h2 class="page-title">Ambulancias y traslados</h2>
        <div style="display:flex;gap:0.5rem">
          <p-button label="Despachar" icon="pi pi-send" severity="success"
            [disabled]="disponibles().length === 0" (onClick)="abrirDespacho()" />
          @if (tab() === 'parque') {
            <p-button label="Nueva ambulancia" icon="pi pi-plus" (onClick)="abrirCrear()" />
          }
        </div>
      </div>

      <div class="stats-row">
        <div class="stat-card stat-green">
          <div class="stat-value">{{ disponibles().length }}</div>
          <div class="stat-label">Disponibles ahora</div>
        </div>
        <div class="stat-card">
          <div class="stat-value">{{ enCurso().length }}</div>
          <div class="stat-label">Traslados en curso</div>
        </div>
        <div class="stat-card stat-red">
          <div class="stat-value">{{ conSoatVencido().length }}</div>
          <div class="stat-label">SOAT vencido</div>
        </div>
        <div class="stat-card stat-yellow">
          <div class="stat-value">{{ sinSoatRegistrado().length }}</div>
          <div class="stat-label">SOAT sin registrar</div>
        </div>
      </div>

      @if (conSoatVencido().length || sinSoatRegistrado().length) {
        <div class="aviso">
          <i class="pi pi-exclamation-triangle"></i>
          <span>
            @if (conSoatVencido().length) {
              <strong>{{ conSoatVencido().length }}</strong> ambulancia(s) no pueden circular: SOAT vencido.
            }
            @if (sinSoatRegistrado().length) {
              <strong>{{ sinSoatRegistrado().length }}</strong> sin SOAT registrado — el sistema no
              puede afirmar que estén al día, y por eso no lo afirma.
            }
          </span>
        </div>
      }

      <div class="tab-bar">
        <button class="tab-btn" [class.active]="tab() === 'parque'" (click)="tab.set('parque')">
          <i class="pi pi-truck"></i> Parque
        </button>
        <button class="tab-btn" [class.active]="tab() === 'traslados'" (click)="cambiarATraslados()">
          <i class="pi pi-map-marker"></i> Traslados
          @if (enCurso().length) { <span class="badge-count">{{ enCurso().length }}</span> }
        </button>
      </div>

      @if (tab() === 'parque') {
        <div class="filter-bar">
          <input pInputText [(ngModel)]="busq" placeholder="Placa o número interno..."
            (input)="cargar()" class="search-input" />
          <p-select [(ngModel)]="filtroEstado" [options]="opcionesEstado"
            optionLabel="label" optionValue="value" [showClear]="true"
            placeholder="Estado" style="width:180px" (onChange)="cargar()" />
        </div>

        @if (cargando()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (ambulancias().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-truck" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay ambulancias registradas.</p>
          </div>
        } @else {
          <p-table [value]="ambulancias()" [rowHover]="true" styleClass="p-datatable-sm">
            <ng-template pTemplate="header">
              <tr>
                <th>Placa</th>
                <th style="text-align:center">Tipo</th>
                <th>Vehículo</th>
                <th style="text-align:center">Estado</th>
                <th>SOAT</th>
                <th style="width:120px"></th>
              </tr>
            </ng-template>
            <ng-template pTemplate="body" let-a>
              <tr [style.opacity]="a.activa ? 1 : 0.55">
                <td>
                  <strong>{{ a.placa }}</strong>
                  @if (a.numero_interno) {
                    <div style="font-size:0.75rem;color:#94a3b8">{{ a.numero_interno }}</div>
                  }
                </td>
                <td style="text-align:center">
                  <p-tag [value]="a.tipo" [severity]="a.tipo === 'TAM' ? 'warn' : 'info'"
                    [pTooltip]="a.tipo === 'TAM' ? 'Transporte asistencial medicalizado' : 'Transporte asistencial básico'" />
                </td>
                <td style="font-size:0.82rem;color:#94a3b8">
                  {{ a.marca || '—' }} {{ a.modelo || '' }}
                </td>
                <td style="text-align:center">
                  <p-tag [value]="a.activa ? a.estado : 'Dada de baja'"
                    [severity]="colorEstado(a)" />
                  @if (a.traslado_en_curso) {
                    <div style="font-size:0.7rem;color:#64748b">traslado #{{ a.traslado_en_curso }}</div>
                  }
                </td>
                <td style="font-size:0.82rem">
                  @if (a.soat_sin_registrar) {
                    <span class="soat-desconocido">sin registrar</span>
                  } @else if (a.soat_vencido) {
                    <span class="soat-vencido">venció {{ a.vence_soat }}</span>
                  } @else if (a.soat_por_vencer) {
                    <span class="soat-pronto">vence {{ a.vence_soat }}</span>
                  } @else {
                    <span style="color:#94a3b8">{{ a.vence_soat }}</span>
                  }
                </td>
                <td style="text-align:right">
                  <p-button icon="pi pi-pencil" [text]="true" size="small"
                    pTooltip="Editar" (onClick)="abrirEditar(a)" />
                  @if (!a.traslado_en_curso) {
                    <p-button [icon]="a.activa ? 'pi pi-ban' : 'pi pi-check'" [text]="true" size="small"
                      [severity]="a.activa ? 'warn' : 'success'"
                      [pTooltip]="a.activa ? 'Dar de baja' : 'Reactivar'"
                      (onClick)="alternarBaja(a)" />
                  }
                </td>
              </tr>
            </ng-template>
          </p-table>
        }
      }

      @if (tab() === 'traslados') {
        @if (cargandoTraslados()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando...</div>
        } @else if (traslados().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-map-marker" style="font-size:2rem;opacity:0.4"></i>
            <p>No hay traslados registrados.</p>
          </div>
        } @else {
          <p-table [value]="traslados()" [rowHover]="true" styleClass="p-datatable-sm">
            <ng-template pTemplate="header">
              <tr>
                <th>Paciente</th>
                <th>Ambulancia</th>
                <th style="text-align:center">Tipo</th>
                <th>Ruta</th>
                <th style="text-align:center">Estado</th>
                <th>Salida / Llegada</th>
                <th style="width:120px"></th>
              </tr>
            </ng-template>
            <ng-template pTemplate="body" let-t>
              <tr>
                <td>
                  <strong>{{ t.paciente_nombre || '—' }}</strong>
                  <div style="font-size:0.75rem;color:#94a3b8">{{ t.paciente_identificacion }}</div>
                </td>
                <td style="font-size:0.82rem">
                  {{ t.ambulancia_placa }}
                  <div style="font-size:0.72rem;color:#64748b">{{ t.ambulancia_tipo }}</div>
                </td>
                <td style="text-align:center">
                  <p-tag [value]="t.tipo_traslado" severity="secondary" />
                </td>
                <td style="font-size:0.82rem">
                  {{ t.origen }} <i class="pi pi-arrow-right" style="font-size:0.7rem;opacity:0.5"></i> {{ t.destino }}
                  <div style="font-size:0.72rem;color:#64748b">{{ t.motivo }}</div>
                </td>
                <td style="text-align:center">
                  <p-tag [value]="t.estado" [severity]="colorTraslado(t.estado)" />
                </td>
                <td style="font-size:0.78rem">
                  {{ t.fecha_salida | date:'dd/MM HH:mm' }}
                  <div style="color:#64748b">
                    <!-- Un cancelado no llegó a ninguna parte, y el guion lo dice
                         en vez de dejar el hueco en blanco. -->
                    {{ t.fecha_llegada ? (t.fecha_llegada | date:'dd/MM HH:mm') : '—' }}
                  </div>
                </td>
                <td style="text-align:right">
                  @if (t.estado === 'En Curso') {
                    <p-button icon="pi pi-check" [text]="true" size="small" severity="success"
                      pTooltip="Finalizar" (onClick)="finalizar(t)" />
                    <p-button icon="pi pi-times" [text]="true" size="small" severity="danger"
                      pTooltip="Cancelar" (onClick)="cancelar(t)" />
                  }
                </td>
              </tr>
            </ng-template>
          </p-table>
        }
      }
    </div>

    <!-- ══ DIALOG AMBULANCIA ══ -->
    <p-dialog [header]="editando ? 'Editar ambulancia' : 'Nueva ambulancia'"
      [(visible)]="mostrarForm" [style]="{width:'620px'}" [modal]="true">
      <div class="form-grid">
        <div class="field">
          <label class="field-label">Placa *</label>
          <input pInputText [(ngModel)]="aForm.placa" class="w-full" placeholder="WXY123" />
        </div>
        <div class="field">
          <label class="field-label">Tipo *</label>
          <p-select [(ngModel)]="aForm.tipo" [options]="opcionesTipo"
            optionLabel="label" optionValue="value" class="w-full" style="width:100%" />
        </div>
        <div class="field">
          <label class="field-label">Número interno</label>
          <input pInputText [(ngModel)]="aForm.numero_interno" class="w-full" />
        </div>
        <div class="field">
          <label class="field-label">Marca</label>
          <input pInputText [(ngModel)]="aForm.marca" class="w-full" />
        </div>
        <div class="field">
          <label class="field-label">Modelo</label>
          <input pInputText [(ngModel)]="aForm.modelo" class="w-full" />
        </div>
        <div class="field">
          <label class="field-label">Vence SOAT</label>
          <input type="date" [(ngModel)]="aForm.vence_soat" class="w-full date-input" />
        </div>
        <div class="field">
          <label class="field-label">Vence tecnomecánica</label>
          <input type="date" [(ngModel)]="aForm.vence_tecnomecanica" class="w-full date-input" />
        </div>
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Observaciones</label>
          <textarea pTextarea [(ngModel)]="aForm.observaciones" rows="2" class="w-full"></textarea>
        </div>
      </div>
      <div class="dialog-footer">
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarForm = false" />
        <p-button [label]="editando ? 'Guardar' : 'Crear'" icon="pi pi-check"
          [loading]="guardando()" [disabled]="!aForm.placa" (onClick)="guardar()" />
      </div>
    </p-dialog>

    <!-- ══ DIALOG DESPACHO ══ -->
    <p-dialog header="Despachar ambulancia" [(visible)]="mostrarDespacho"
      [style]="{width:'620px'}" [modal]="true">
      <div class="form-grid">
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Ambulancia *</label>
          <p-select [(ngModel)]="tForm.ambulancia_id" [options]="opcionesDespacho()"
            optionLabel="label" optionValue="value" [filter]="true"
            placeholder="Solo se ofrecen las que pueden salir ahora"
            class="w-full" style="width:100%" />
        </div>
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Paciente *</label>
          <p-select [(ngModel)]="tForm.paciente_id" [options]="opcionesPaciente()"
            optionLabel="label" optionValue="value" [filter]="true"
            placeholder="Buscar por nombre o documento..."
            class="w-full" style="width:100%" />
        </div>
        <div class="field">
          <label class="field-label">Tipo de traslado *</label>
          <p-select [(ngModel)]="tForm.tipo_traslado" [options]="opcionesTipoTraslado"
            optionLabel="label" optionValue="value" class="w-full" style="width:100%" />
        </div>
        <div class="field">
          <label class="field-label">Conductor</label>
          <input pInputText [(ngModel)]="tForm.conductor" class="w-full" />
        </div>
        <div class="field">
          <label class="field-label">Origen *</label>
          <input pInputText [(ngModel)]="tForm.origen" class="w-full"
            placeholder="Vía Las Palmas km 12" />
        </div>
        <div class="field">
          <label class="field-label">Destino *</label>
          <input pInputText [(ngModel)]="tForm.destino" class="w-full"
            placeholder="Urgencias sede norte" />
        </div>
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Motivo *</label>
          <input pInputText [(ngModel)]="tForm.motivo" class="w-full"
            placeholder="Accidente de tránsito" />
        </div>
        <div class="field" style="grid-column:1/-1">
          <label class="field-label">Personal asistencial</label>
          <input pInputText [(ngModel)]="tForm.personal_asistencial" class="w-full" />
        </div>
      </div>
      <div class="dialog-footer">
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarDespacho = false" />
        <p-button label="Despachar" icon="pi pi-send" severity="success"
          [loading]="despachando()" [disabled]="!despachoCompleto()" (onClick)="despachar()" />
      </div>
    </p-dialog>

    <p-toast />
  `,
  styles: [`
    .page-card { padding: 1.5rem; }
    .page-title { font-size: 1.3rem; font-weight: 700; color: #f1f5f9; margin: 0; }
    .list-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.25rem; }
    .stats-row { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 0.75rem; margin-bottom: 1rem; }
    .stat-card { background: #1e293b; border: 1px solid #334155; border-radius: 8px; padding: 0.85rem 1rem; }
    .stat-value { font-size: 1.5rem; font-weight: 700; color: #f1f5f9; }
    .stat-label { font-size: 0.72rem; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.03em; }
    .stat-green .stat-value { color: #4ade80; }
    .stat-red .stat-value { color: #f87171; }
    .stat-yellow .stat-value { color: #fbbf24; }
    .aviso { display: flex; gap: 0.6rem; align-items: center; background: rgba(251,191,36,0.08);
      border: 1px solid rgba(251,191,36,0.3); color: #fcd34d; border-radius: 8px;
      padding: 0.65rem 0.9rem; font-size: 0.82rem; margin-bottom: 1rem; }
    .tab-bar { display: flex; gap: 0.25rem; border-bottom: 1px solid #334155; margin-bottom: 1rem; }
    .tab-btn { background: none; border: none; color: #94a3b8; padding: 0.6rem 1rem;
      cursor: pointer; font-size: 0.875rem; border-bottom: 2px solid transparent;
      display: flex; align-items: center; gap: 0.4rem; }
    .tab-btn.active { color: #93c5fd; border-bottom-color: #3b82f6; }
    .badge-count { background: #3b82f6; color: white; border-radius: 10px; padding: 0 0.4rem; font-size: 0.7rem; }
    .filter-bar { display: flex; gap: 0.6rem; margin-bottom: 1rem; flex-wrap: wrap; }
    .search-input { width: 260px; }
    .cargando { padding: 3rem; text-align: center; color: #64748b; }
    .sin-resultados { text-align: center; padding: 3rem 1rem; color: #64748b;
      display: flex; flex-direction: column; align-items: center; gap: 0.75rem; }
    .form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 0.875rem; }
    .field-label { display: block; font-size: 0.78rem; color: #94a3b8; margin-bottom: 0.25rem; }
    .w-full { width: 100%; }
    .date-input { background: #0f172a; border: 1px solid #334155; color: #e2e8f0;
      border-radius: 6px; padding: 0.5rem; }
    .dialog-footer { display: flex; justify-content: flex-end; gap: 0.5rem; margin-top: 1.25rem; }
    /* Los tres estados del SOAT se ven distintos a propósito: «sin registrar» no
       se pinta como «vigente» ni como «vencido», porque no es ninguno de los dos. */
    .soat-vencido { color: #f87171; font-weight: 600; }
    .soat-pronto { color: #fbbf24; }
    .soat-desconocido { color: #94a3b8; font-style: italic; }
  `],
})
export class AmbulanciasComponent implements OnInit {
  private svc = inject(AmbulanciasService);
  private pacientes = inject(PacienteService);
  private toast = inject(MessageService);

  tab = signal<Tab>('parque');
  ambulancias = signal<Ambulancia[]>([]);
  traslados = signal<Traslado[]>([]);
  cargando = signal(false);
  cargandoTraslados = signal(false);
  busq = '';
  filtroEstado: string | null = null;

  mostrarForm = false;
  editando: Ambulancia | null = null;
  guardando = signal(false);
  aForm: Partial<AmbulanciaCreate> = { tipo: 'TAB' };

  mostrarDespacho = false;
  despachando = signal(false);
  tForm: Partial<TrasladoCreate> = { tipo_traslado: 'Primario' };
  private listaPacientes = signal<{ label: string; value: number }[]>([]);

  readonly opcionesTipo = [
    { label: 'TAB — básico', value: 'TAB' },
    { label: 'TAM — medicalizado', value: 'TAM' },
  ];
  readonly opcionesTipoTraslado = [
    { label: 'Primario — desde el sitio del evento', value: 'Primario' },
    { label: 'Secundario — entre prestadores', value: 'Secundario' },
  ];
  readonly opcionesEstado = [
    { label: 'Disponible', value: 'Disponible' },
    { label: 'En Traslado', value: 'En Traslado' },
    { label: 'En Mantenimiento', value: 'En Mantenimiento' },
    { label: 'Fuera de Servicio', value: 'Fuera de Servicio' },
  ];

  /** Las que el servidor dejaría salir ahora mismo. Es la misma regla que aplica
   *  `ambulancia::despachar`, escrita aquí para no ofrecer lo que va a fallar. */
  disponibles = computed(() =>
    this.ambulancias().filter(
      a => a.activa && a.estado === 'Disponible' && !a.traslado_en_curso && !a.soat_vencido,
    ),
  );
  conSoatVencido = computed(() => this.ambulancias().filter(a => a.activa && a.soat_vencido));
  sinSoatRegistrado = computed(() => this.ambulancias().filter(a => a.activa && a.soat_sin_registrar));
  enCurso = computed(() => this.traslados().filter(t => t.estado === 'En Curso'));

  opcionesDespacho = computed(() =>
    this.disponibles().map(a => ({
      label: `${a.placa} — ${a.tipo}${a.soat_sin_registrar ? ' (SOAT sin registrar)' : ''}`,
      value: a.id,
    })),
  );
  opcionesPaciente = computed(() => this.listaPacientes());

  despachoCompleto = computed(() =>
    !!(this.tForm.ambulancia_id && this.tForm.paciente_id && this.tForm.origen
       && this.tForm.destino && this.tForm.motivo),
  );

  ngOnInit(): void {
    this.cargar();
    this.cargarTraslados();
  }

  cargar(): void {
    this.cargando.set(true);
    this.svc.listar(this.busq || undefined, this.filtroEstado ?? undefined).subscribe({
      next: a => { this.ambulancias.set(a); this.cargando.set(false); },
      error: () => this.cargando.set(false),
    });
  }

  cambiarATraslados(): void {
    this.tab.set('traslados');
    this.cargarTraslados();
  }

  cargarTraslados(): void {
    this.cargandoTraslados.set(true);
    this.svc.listarTraslados().subscribe({
      next: t => { this.traslados.set(t); this.cargandoTraslados.set(false); },
      error: () => this.cargandoTraslados.set(false),
    });
  }

  colorEstado(a: Ambulancia): 'success' | 'info' | 'warn' | 'danger' | 'secondary' {
    if (!a.activa) return 'secondary';
    if (a.estado === 'Disponible') return 'success';
    if (a.estado === 'En Traslado') return 'info';
    if (a.estado === 'En Mantenimiento') return 'warn';
    return 'danger';
  }

  colorTraslado(estado: string): 'success' | 'info' | 'danger' | 'secondary' {
    if (estado === 'Finalizado') return 'success';
    if (estado === 'En Curso') return 'info';
    if (estado === 'Cancelado') return 'danger';
    return 'secondary';
  }

  abrirCrear(): void {
    this.editando = null;
    this.aForm = { tipo: 'TAB' };
    this.mostrarForm = true;
  }

  abrirEditar(a: Ambulancia): void {
    this.editando = a;
    this.aForm = {
      placa: a.placa, tipo: a.tipo, numero_interno: a.numero_interno,
      marca: a.marca, modelo: a.modelo, vence_soat: a.vence_soat,
      vence_tecnomecanica: a.vence_tecnomecanica, observaciones: a.observaciones,
    };
    this.mostrarForm = true;
  }

  guardar(): void {
    if (!this.aForm.placa) return;
    this.guardando.set(true);
    const editando = this.editando;
    const peticion = editando
      ? this.svc.actualizar(editando.id, this.aForm)
      : this.svc.crear(this.aForm as AmbulanciaCreate);
    peticion.subscribe({
      next: () => {
        this.guardando.set(false);
        this.mostrarForm = false;
        this.ok(editando ? 'Ambulancia actualizada' : 'Ambulancia registrada', this.aForm.placa!);
        this.cargar();
      },
      error: err => { this.guardando.set(false); this.err(err?.error?.detail ?? 'No se pudo guardar'); },
    });
  }

  alternarBaja(a: Ambulancia): void {
    this.svc.actualizar(a.id, { activa: !a.activa }).subscribe({
      next: () => { this.ok(a.activa ? 'Dada de baja' : 'Reactivada', a.placa); this.cargar(); },
      error: err => this.err(err?.error?.detail ?? 'No se pudo cambiar el estado'),
    });
  }

  abrirDespacho(): void {
    this.tForm = { tipo_traslado: 'Primario' };
    if (this.listaPacientes().length === 0) {
      this.pacientes.list(1, 500).subscribe({
        next: r => this.listaPacientes.set(
          r.items.map(p => ({
            label: `${p.nombre1} ${p.apellido1} — ${p.identificacion}`,
            value: p.id,
          })),
        ),
      });
    }
    this.mostrarDespacho = true;
  }

  despachar(): void {
    if (!this.despachoCompleto()) return;
    this.despachando.set(true);
    this.svc.despachar(this.tForm as TrasladoCreate).subscribe({
      next: t => {
        this.despachando.set(false);
        this.mostrarDespacho = false;
        this.ok('Ambulancia despachada', `${t.ambulancia_placa} → ${t.destino}`);
        this.cargar();
        this.cargarTraslados();
      },
      error: err => {
        this.despachando.set(false);
        this.err(err?.error?.detail ?? 'No se pudo despachar');
      },
    });
  }

  finalizar(t: Traslado): void {
    this.svc.finalizar(t.id).subscribe({
      next: () => { this.ok('Traslado finalizado', `${t.ambulancia_placa} disponible`); this.refrescar(); },
      error: err => this.err(err?.error?.detail ?? 'No se pudo finalizar'),
    });
  }

  cancelar(t: Traslado): void {
    this.svc.cancelar(t.id).subscribe({
      next: () => { this.ok('Traslado cancelado', 'La ambulancia queda disponible'); this.refrescar(); },
      error: err => this.err(err?.error?.detail ?? 'No se pudo cancelar'),
    });
  }

  private refrescar(): void {
    this.cargar();
    this.cargarTraslados();
  }

  private ok(summary: string, detail: string): void {
    this.toast.add({ severity: 'success', summary, detail, life: 3000 });
  }

  private err(detail: string): void {
    this.toast.add({ severity: 'error', summary: 'No se pudo', detail, life: 6000 });
  }
}
