import { CommonModule } from '@angular/common';
import { Component, Inject, inject, OnInit, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { HttpClient } from '@angular/common/http';
import {
  MAT_DIALOG_DATA,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MensajeComponent } from '../../shared/ui/mensaje.component';
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
 *
 * Se abre con `MatDialog`, no con un `[abierto]` que gobernaba el padre: el
 * paciente a editar entra por `MAT_DIALOG_DATA` —`null` para dar de alta— y el
 * guardado sale por `afterClosed()`. `disableClose` está puesto a propósito,
 * porque cerrar con cambios pendientes tiene que preguntar.
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
    CommonModule, FormsModule, MatDialogModule, MatFormFieldModule,
    MatInputModule, MatSelectModule, MensajeComponent, BarraCrudComponent,
    TeclasDirective, ZoomDirective,
  ],
  template: `
    <h2 mat-dialog-title>
      {{ modo === 'nuevo' ? 'Nuevo paciente' : 'Paciente ' + (datos.identificacion || '') }}
    </h2>
    <mat-dialog-content>
      <form appTeclas (confirmar)="guardar()" (cancelar)="pedirCierre()">
        <app-barra-crud
          [modo]="modo"
          [mostrarNavegacion]="false"
          (accion)="alAccion($event)"
        />

        @if (error()) {
          <x-mensaje severity="error" [text]="error()!" />
        }

        <div class="rejilla">
          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Tipo de documento</mat-label>
            <mat-select [(ngModel)]="datos.tipo_id" name="tipo_id" [disabled]="!editable">
              @for (t of tiposDocumento; track t.valor) {
                <mat-option [value]="t.valor">{{ t.etiqueta }}</mat-option>
              }
            </mat-select>
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Número de documento</mat-label>
            <input matInput name="identificacion" appZoom
              [(ngModel)]="datos.identificacion" [disabled]="!editable" />
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Primer nombre</mat-label>
            <input matInput name="nombre1" appZoom
              [(ngModel)]="datos.nombre1" [disabled]="!editable" />
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Segundo nombre</mat-label>
            <input matInput name="nombre2" appZoom
              [(ngModel)]="datos.nombre2" [disabled]="!editable" />
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Primer apellido</mat-label>
            <input matInput name="apellido1" appZoom
              [(ngModel)]="datos.apellido1" [disabled]="!editable" />
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Segundo apellido</mat-label>
            <input matInput name="apellido2" appZoom
              [(ngModel)]="datos.apellido2" [disabled]="!editable" />
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Fecha de nacimiento</mat-label>
            <input matInput type="date" name="fecha_nacimiento"
              [max]="hoy" [(ngModel)]="datos.fecha_nacimiento" [disabled]="!editable" />
            @if (edad(); as e) {
              <mat-hint class="edad">{{ e }}</mat-hint>
            }
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Sexo</mat-label>
            <mat-select [(ngModel)]="datos.sexo" name="sexo" [disabled]="!editable">
              @for (s of sexos; track s.valor) {
                <mat-option [value]="s.valor">{{ s.etiqueta }}</mat-option>
              }
            </mat-select>
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Celular</mat-label>
            <input matInput name="celular" appZoom
              [(ngModel)]="datos.celular" [disabled]="!editable" />
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Correo</mat-label>
            <input matInput name="email" appZoom
              [(ngModel)]="datos.email" [disabled]="!editable" />
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic" class="ancho">
            <mat-label>Dirección</mat-label>
            <input matInput name="direccion" appZoom
              [(ngModel)]="datos.direccion" [disabled]="!editable" />
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Municipio (DIVIPOLA)</mat-label>
            <input matInput name="cod_municipio" appZoom
              placeholder="05001" [(ngModel)]="datos.cod_municipio" [disabled]="!editable" />
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Zona</mat-label>
            <mat-select [(ngModel)]="datos.zona" name="zona" [disabled]="!editable">
              @for (z of zonas; track z.valor) {
                <mat-option [value]="z.valor">{{ z.etiqueta }}</mat-option>
              }
            </mat-select>
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Tipo de usuario</mat-label>
            <mat-select [(ngModel)]="datos.tipo_usuario" name="tipo_usuario"
              [disabled]="!editable">
              @for (t of tiposUsuario; track t.valor) {
                <mat-option [value]="t.valor">{{ t.etiqueta }}</mat-option>
              }
            </mat-select>
          </mat-form-field>

          <mat-form-field appearance="outline" subscriptSizing="dynamic">
            <mat-label>Entidad responsable</mat-label>
            <mat-select [(ngModel)]="datos.entidad_id" name="entidad_id"
              [disabled]="!editable">
              <!-- La opción vacía hace de [showClear] de PrimeNG: un particular
                   no tiene entidad, y sin ella no habría forma de quitarla. -->
              <mat-option [value]="null">Ninguna</mat-option>
              @for (e of entidades(); track e.id) {
                <mat-option [value]="e.id">{{ e.nombre }}</mat-option>
              }
            </mat-select>
          </mat-form-field>
        </div>
      </form>
    </mat-dialog-content>
  `,
  styles: [`
    .rejilla {
      display: grid; grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
      gap: .85rem; margin-top: 1rem;
    }
    .ancho { grid-column: 1 / -1; }
    x-mensaje { display: block; margin-top: 1rem; }
    .edad { color: var(--primary-color); }
  `],
})
export class PacienteFormComponent implements OnInit {
  private svc = inject(PacienteService);
  private http = inject(HttpClient);
  private ref = inject<MatDialogRef<PacienteFormComponent, Patient>>(MatDialogRef);

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

  /** El paciente a editar, o `null` para dar de alta uno nuevo. */
  constructor(@Inject(MAT_DIALOG_DATA) public paciente: Patient | null) {}

  get editable(): boolean {
    return camposEditables(this.modo);
  }

  ngOnInit(): void {
    this.error.set(null);
    this.modo = this.paciente ? 'edicion' : 'nuevo';
    this.datos = this.paciente
      ? { ...pacienteVacio(), ...this.paciente }
      : pacienteVacio();
    this.http.get<Entidad[]>('/api/admin/entidades').subscribe({
      next: (e) => this.entidades.set(e),
      // Sin entidades el formulario sigue sirviendo: un particular no tiene.
      error: () => this.entidades.set([]),
    });
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
      // El paciente guardado ES el valor de retorno del diálogo: la pantalla
      // recarga cuando le llega uno, y no cuando el diálogo se cierra.
      next: (p: Patient) => this.ref.close(p),
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
    // Sin argumento: cerrar sin guardar no devuelve paciente, y la pantalla no
    // recarga. Devolver algo aquí sería afirmar un alta que no ocurrió.
    this.ref.close();
  }
}
