import { CommonModule } from '@angular/common';
import { Component, EventEmitter, inject, Input, OnChanges, Output, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { HttpClient } from '@angular/common/http';
import { ButtonModule } from 'primeng/button';
import { DatePickerModule } from 'primeng/datepicker';
import { DialogModule } from 'primeng/dialog';
import { InputTextModule } from 'primeng/inputtext';
import { MessageModule } from 'primeng/message';
import { SelectModule } from 'primeng/select';
import { BarraCrudComponent } from '../../shared/formulario/barra-crud.component';
import { AccionCrud, ModoFormulario, camposEditables } from '../../shared/formulario/estado-formulario';
import { TeclasDirective } from '../../shared/formulario/teclas.directive';
import { ZoomDirective } from '../../shared/formulario/zoom.directive';
import { PacienteService } from '../../services/paciente.service';
import { Patient, PatientCreate } from '../../models/patient.models';

/**
 * Alta y edición de pacientes: el `pacientes.scx` del original.
 *
 * Hasta ahora el botón "Nuevo Paciente" existía y no hacía nada —tres botones
 * en la pantalla y ningún manejador—, así que un paciente solo podía entrar al
 * sistema por el script de semillas. Todo lo demás cuelga de aquí: sin paciente
 * no hay historia, sin historia no hay RIPS y sin RIPS no hay pago.
 *
 * Usa el marco compartido de formularios en vez de repetirlo: la barra CRUD
 * gobierna qué se puede hacer, Enter avanza al siguiente campo como en el
 * FoxPro, y el clic derecho amplía cualquier campo.
 */

/** Tipos de documento de la Resolución 2275/2023, en el orden de uso real. */
export const TIPOS_DOCUMENTO = [
  { valor: 'CC', etiqueta: 'CC — Cédula de ciudadanía' },
  { valor: 'TI', etiqueta: 'TI — Tarjeta de identidad' },
  { valor: 'RC', etiqueta: 'RC — Registro civil' },
  { valor: 'CE', etiqueta: 'CE — Cédula de extranjería' },
  { valor: 'PA', etiqueta: 'PA — Pasaporte' },
  { valor: 'PE', etiqueta: 'PE — Permiso especial de permanencia' },
  { valor: 'PT', etiqueta: 'PT — Permiso por protección temporal' },
  { valor: 'CN', etiqueta: 'CN — Certificado de nacido vivo' },
  { valor: 'AS', etiqueta: 'AS — Adulto sin identificar' },
  { valor: 'MS', etiqueta: 'MS — Menor sin identificar' },
];

export const TIPOS_USUARIO = [
  { valor: '01', etiqueta: 'Contributivo' },
  { valor: '02', etiqueta: 'Subsidiado' },
  { valor: '03', etiqueta: 'Vinculado' },
  { valor: '04', etiqueta: 'Particular' },
  { valor: '05', etiqueta: 'Otro' },
];

interface Entidad {
  id: number;
  nombre: string;
}

/** Un paciente en blanco, con los valores que casi nunca cambian ya puestos. */
export function pacienteVacio(): PatientCreate {
  return {
    identificacion: '',
    tipo_id: 'CC',
    nombre1: '',
    apellido1: '',
    fecha_nacimiento: '',
    sexo: 'M',
    celular: '',
    tipo_usuario: '01',
    cod_municipio: '',
    cod_pais: '170',
    zona: '01',
  };
}

/**
 * Qué falta para poder guardar.
 *
 * Se valida aquí y no solo en el servidor porque el mensaje llega antes y
 * señala el campo. Pero no sustituye al servidor: esto es comodidad, no
 * garantía.
 */
export function faltantes(p: PatientCreate): string[] {
  const fallos: string[] = [];
  if (!p.identificacion?.trim()) fallos.push('el número de documento');
  if (!p.nombre1?.trim()) fallos.push('el primer nombre');
  if (!p.apellido1?.trim()) fallos.push('el primer apellido');
  if (!p.fecha_nacimiento) fallos.push('la fecha de nacimiento');
  if (!p.celular?.trim()) fallos.push('el celular');
  if (!p.cod_municipio?.trim()) fallos.push('el municipio de residencia');

  // La fecha de nacimiento manda en la clasificación por edad de todos los
  // informes epidemiológicos: una fecha futura los descuadra en silencio.
  if (p.fecha_nacimiento && p.fecha_nacimiento > hoyISO()) {
    fallos.push('una fecha de nacimiento que no esté en el futuro');
  }
  return fallos;
}

export function hoyISO(): string {
  return new Date().toISOString().slice(0, 10);
}

@Component({
  selector: 'app-paciente-form',
  standalone: true,
  imports: [
    CommonModule, FormsModule, DialogModule, InputTextModule, SelectModule,
    DatePickerModule, ButtonModule, MessageModule, BarraCrudComponent,
    TeclasDirective, ZoomDirective,
  ],
  template: `
    <p-dialog
      [visible]="abierto"
      (visibleChange)="alCerrarDialogo($event)"
      [modal]="true"
      [style]="{ width: '52rem', maxWidth: '95vw' }"
      [header]="modo === 'nuevo' ? 'Nuevo paciente' : 'Paciente ' + (datos.identificacion || '')"
    >
      <form appTeclas (confirmar)="guardar()" (cancelar)="pedirCierre()">
        <app-barra-crud
          [modo]="modo"
          [mostrarNavegacion]="false"
          (accion)="alAccion($event)"
        />

        @if (error()) {
          <p-message severity="error" styleClass="w-full" [text]="error()!" />
        }

        <div class="rejilla">
          <div class="campo">
            <label for="tipo_id">Tipo de documento</label>
            <p-select
              inputId="tipo_id" [options]="tiposDocumento" optionLabel="etiqueta"
              optionValue="valor" [(ngModel)]="datos.tipo_id" name="tipo_id"
              [disabled]="!editable" appendTo="body" />
          </div>

          <div class="campo">
            <label for="identificacion">Número de documento</label>
            <input pInputText id="identificacion" name="identificacion" appZoom
              [(ngModel)]="datos.identificacion" [disabled]="!editable" />
          </div>

          <div class="campo">
            <label for="nombre1">Primer nombre</label>
            <input pInputText id="nombre1" name="nombre1" appZoom
              [(ngModel)]="datos.nombre1" [disabled]="!editable" />
          </div>

          <div class="campo">
            <label for="nombre2">Segundo nombre</label>
            <input pInputText id="nombre2" name="nombre2" appZoom
              [(ngModel)]="datos.nombre2" [disabled]="!editable" />
          </div>

          <div class="campo">
            <label for="apellido1">Primer apellido</label>
            <input pInputText id="apellido1" name="apellido1" appZoom
              [(ngModel)]="datos.apellido1" [disabled]="!editable" />
          </div>

          <div class="campo">
            <label for="apellido2">Segundo apellido</label>
            <input pInputText id="apellido2" name="apellido2" appZoom
              [(ngModel)]="datos.apellido2" [disabled]="!editable" />
          </div>

          <div class="campo">
            <label for="fecha_nacimiento">Fecha de nacimiento</label>
            <input pInputText type="date" id="fecha_nacimiento" name="fecha_nacimiento"
              [max]="hoy" [(ngModel)]="datos.fecha_nacimiento" [disabled]="!editable" />
            @if (edad(); as e) {
              <small class="edad">{{ e }}</small>
            }
          </div>

          <div class="campo">
            <label for="sexo">Sexo</label>
            <p-select
              inputId="sexo" [options]="sexos" optionLabel="etiqueta" optionValue="valor"
              [(ngModel)]="datos.sexo" name="sexo" [disabled]="!editable" appendTo="body" />
          </div>

          <div class="campo">
            <label for="celular">Celular</label>
            <input pInputText id="celular" name="celular" appZoom
              [(ngModel)]="datos.celular" [disabled]="!editable" />
          </div>

          <div class="campo">
            <label for="email">Correo</label>
            <input pInputText id="email" name="email" appZoom
              [(ngModel)]="datos.email" [disabled]="!editable" />
          </div>

          <div class="campo ancho">
            <label for="direccion">Dirección</label>
            <input pInputText id="direccion" name="direccion" appZoom
              [(ngModel)]="datos.direccion" [disabled]="!editable" />
          </div>

          <div class="campo">
            <label for="cod_municipio">Municipio (DIVIPOLA)</label>
            <input pInputText id="cod_municipio" name="cod_municipio" appZoom
              placeholder="05001" [(ngModel)]="datos.cod_municipio" [disabled]="!editable" />
          </div>

          <div class="campo">
            <label for="zona">Zona</label>
            <p-select
              inputId="zona" [options]="zonas" optionLabel="etiqueta" optionValue="valor"
              [(ngModel)]="datos.zona" name="zona" [disabled]="!editable" appendTo="body" />
          </div>

          <div class="campo">
            <label for="tipo_usuario">Tipo de usuario</label>
            <p-select
              inputId="tipo_usuario" [options]="tiposUsuario" optionLabel="etiqueta"
              optionValue="valor" [(ngModel)]="datos.tipo_usuario" name="tipo_usuario"
              [disabled]="!editable" appendTo="body" />
          </div>

          <div class="campo">
            <label for="entidad_id">Entidad responsable</label>
            <p-select
              inputId="entidad_id" [options]="entidades()" optionLabel="nombre"
              optionValue="id" [(ngModel)]="datos.entidad_id" name="entidad_id"
              [showClear]="true" placeholder="Ninguna"
              [disabled]="!editable" appendTo="body" />
          </div>
        </div>
      </form>
    </p-dialog>
  `,
  styles: [`
    .rejilla {
      display: grid; grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
      gap: .85rem; margin-top: 1rem;
    }
    .campo { display: flex; flex-direction: column; gap: .3rem; }
    .campo.ancho { grid-column: 1 / -1; }
    label { font-size: .8rem; font-weight: 600; color: var(--text-color-secondary); }
    .edad { color: var(--primary-color); font-size: .75rem; }
  `],
})
export class PacienteFormComponent implements OnChanges {
  private svc = inject(PacienteService);
  private http = inject(HttpClient);

  @Input() abierto = false;
  /** Paciente a editar, o `null` para dar de alta uno nuevo. */
  @Input() paciente: Patient | null = null;

  @Output() abiertoChange = new EventEmitter<boolean>();
  @Output() guardado = new EventEmitter<Patient>();

  readonly tiposDocumento = TIPOS_DOCUMENTO;
  readonly tiposUsuario = TIPOS_USUARIO;
  readonly sexos = [
    { valor: 'M', etiqueta: 'Masculino' },
    { valor: 'F', etiqueta: 'Femenino' },
  ];
  readonly zonas = [
    { valor: '01', etiqueta: 'Urbana' },
    { valor: '02', etiqueta: 'Rural' },
  ];
  readonly hoy = hoyISO();

  modo: ModoFormulario = 'nuevo';
  datos: PatientCreate = pacienteVacio();
  error = signal<string | null>(null);
  entidades = signal<Entidad[]>([]);

  get editable(): boolean {
    return camposEditables(this.modo);
  }

  ngOnChanges(): void {
    if (!this.abierto) {
      return;
    }
    this.error.set(null);
    this.modo = this.paciente ? 'edicion' : 'nuevo';
    this.datos = this.paciente
      ? { ...pacienteVacio(), ...this.paciente }
      : pacienteVacio();
    if (this.entidades().length === 0) {
      this.http.get<Entidad[]>('/api/admin/entidades').subscribe({
        next: (e) => this.entidades.set(e),
        // Sin entidades el formulario sigue sirviendo: un particular no tiene.
        error: () => this.entidades.set([]),
      });
    }
  }

  /** Edad en años, para cazar de inmediato una fecha mal digitada. */
  edad(): string | null {
    if (!this.datos.fecha_nacimiento) {
      return null;
    }
    const nacimiento = new Date(this.datos.fecha_nacimiento + 'T00:00:00');
    if (Number.isNaN(nacimiento.getTime())) {
      return null;
    }
    const hoy = new Date();
    let años = hoy.getFullYear() - nacimiento.getFullYear();
    const antes =
      hoy.getMonth() < nacimiento.getMonth() ||
      (hoy.getMonth() === nacimiento.getMonth() && hoy.getDate() < nacimiento.getDate());
    if (antes) años--;
    return años >= 0 ? `${años} años` : null;
  }

  alAccion(accion: AccionCrud): void {
    if (accion === 'guardar') this.guardar();
    if (accion === 'deshacer') this.pedirCierre();
  }

  guardar(): void {
    const fallan = faltantes(this.datos);
    if (fallan.length > 0) {
      this.error.set(`Falta ${fallan.join(', ')}.`);
      return;
    }
    this.error.set(null);

    const alTerminar = {
      next: (p: Patient) => {
        this.guardado.emit(p);
        this.cerrar();
      },
      error: (e: { error?: { detail?: string } }) =>
        this.error.set(e?.error?.detail ?? 'No se pudo guardar el paciente.'),
    };

    if (this.paciente) {
      this.svc.update(this.paciente.id, this.datos).subscribe(alTerminar);
    } else {
      this.svc.create(this.datos).subscribe(alTerminar);
    }
  }

  /**
   * Cerrar con cambios pendientes tiene que confirmarse.
   *
   * Es el comportamiento del original: Esc PREGUNTA antes de cerrar. Sin eso,
   * una tecla a destiempo borra un formulario recién llenado.
   */
  pedirCierre(): void {
    if (this.modo !== 'consulta' && !confirm('¿Cerrar sin guardar los cambios?')) {
      return;
    }
    this.cerrar();
  }

  alCerrarDialogo(visible: boolean): void {
    if (!visible) {
      this.pedirCierre();
    }
  }

  private cerrar(): void {
    this.abierto = false;
    this.abiertoChange.emit(false);
  }
}
