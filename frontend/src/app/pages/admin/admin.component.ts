import { Component, Inject, OnInit, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MAT_DIALOG_DATA, MatDialog, MatDialogModule } from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatSlideToggleModule } from '@angular/material/slide-toggle';
import { MatTableModule } from '@angular/material/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatDividerModule } from '@angular/material/divider';
import { Avisos } from '../../shared/ui/avisos.service';
import {
  AdminService, Institucion, ConfiguracionInstitucion, PoliticaSeguridad,
  UsuarioAdmin, ConfigCampo, AuditoriaItem, Especialidad, EntidadEPS,
  MatrizCapacidades, FilaCapacidad, CasillaCapacidad, CambioCapacidad,
} from '../../services/admin.service';

type Tab = 'institucion' | 'configuracion' | 'usuarios' | 'seguridad' | 'campos' | 'auditoria' | 'catalogos' | 'capacidades';
type CatTab = 'especialidades' | 'entidades';

const ROLES = [
  { value: 'medico', label: 'Médico' },
  { value: 'enfermeria', label: 'Enfermería' },
  { value: 'auxiliar_enfermeria', label: 'Auxiliar Enfermería' },
  { value: 'admin', label: 'Administrador' },
  { value: 'regente_farmacia', label: 'Regente Farmacia' },
  { value: 'laboratorista', label: 'Laboratorista' },
  { value: 'radiologo', label: 'Radiólogo' },
  { value: 'anestesiologo', label: 'Anestesiólogo' },
  { value: 'terapeuta', label: 'Terapeuta' },
  { value: 'nutricionista', label: 'Nutricionista' },
  { value: 'psicologo', label: 'Psicólogo' },
  { value: 'trabajo_social', label: 'Trabajo Social' },
  { value: 'bacteriologo', label: 'Bacteriólogo' },
  { value: 'paciente', label: 'Paciente' },
];

/// Los nombres de las capacidades para la pantalla. Solo es traducción: la lista
/// de qué capacidades existen viene del servidor con la matriz, así que una que
/// falte aquí se pinta con su nombre técnico en vez de desaparecer.
const CAPACIDADES: Record<string, string> = {
  firmar_clinico: 'Firmar historia o documento',
  borrar_clinico: 'Borrar historia o documento',
  escribir_clinico: 'Escribir historia y recetar',
  escribir_paciente: 'Registrar paciente y antecedentes',
  atender_urgencias_hospitalizacion: 'Urgencias y hospitalización',
  escribir_nutricion: 'Nutrición',
  escribir_rehabilitacion: 'Rehabilitación',
  gestionar_remision: 'Remisiones y contrarreferencia',
  escribir_laboratorio: 'Laboratorio',
  escribir_imagenologia: 'Imágenes diagnósticas',
  gestionar_farmacia: 'Farmacia y dispensación',
  gestionar_ambulancias: 'Ambulancias y traslados',
  gestionar_usuarios: 'Gestionar usuarios y permisos',
  gestionar_instituciones: 'Crear instituciones',
};

const TIPOS_CAMPO = [
  { value: 'Texto', label: 'Texto' },
  { value: 'Número', label: 'Número' },
  { value: 'Si/No', label: 'Si/No' },
  { value: 'Lista Opciones', label: 'Lista Opciones' },
];

const DIAS_SEMANA = [
  { key: 'L', label: 'Lun' }, { key: 'M', label: 'Mar' }, { key: 'X', label: 'Mié' },
  { key: 'J', label: 'Jue' }, { key: 'V', label: 'Vie' }, { key: 'S', label: 'Sáb' },
  { key: 'D', label: 'Dom' },
];


/** Alta y edición de un usuario. Devuelve el formulario, o nada. */
@Component({
  selector: 'app-admin-usuario',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatSelectModule,
  ],
  template: `
    <h2 mat-dialog-title>{{ editando ? 'Editar usuario' : 'Nuevo usuario' }}</h2>
    <mat-dialog-content>
      <div class="rejilla">
        <mat-form-field appearance="outline">
          <mat-label>Nombre completo *</mat-label>
          <input matInput [(ngModel)]="f.nombre_completo" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Username *</mat-label>
          <input matInput [(ngModel)]="f.username" [disabled]="editando" />
        </mat-form-field>
        @if (!editando) {
          <mat-form-field appearance="outline">
            <mat-label>Contraseña *</mat-label>
            <input matInput type="password" [(ngModel)]="f.password" />
          </mat-form-field>
        }
        <mat-form-field appearance="outline">
          <mat-label>Email</mat-label>
          <input matInput type="email" [(ngModel)]="f.email" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Rol *</mat-label>
          <mat-select [(ngModel)]="f.rol">
            @for (r of roles; track r.value) { <mat-option [value]="r.value">{{ r.label }}</mat-option> }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Especialidad</mat-label>
          <mat-select [(ngModel)]="f.especialidad_id">
            <mat-option [value]="null">— Sin especialidad —</mat-option>
            @for (e of especialidades; track e.id) { <mat-option [value]="e.id">{{ e.nombre }}</mat-option> }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Reg. Médico / Tarjeta Prof.</mat-label>
          <input matInput [(ngModel)]="f.registro_medico" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Registro RETHUS</mat-label>
          <input matInput [(ngModel)]="f.registro_rethus" />
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!completo()" [mat-dialog-close]="resultado()">
        <i class="pi pi-save"></i> Guardar
      </button>
    </mat-dialog-actions>
  `,
  styles: [`
    .rejilla { display:grid; grid-template-columns:1fr 1fr; gap:.875rem; padding-top:.5rem;
      min-width:min(32rem, 85vw); }
    mat-form-field { width:100%; }
  `],
})
export class AdminUsuarioComponent {
  readonly roles: { value: string; label: string }[];
  readonly especialidades: { id: number; nombre: string }[];
  readonly editando: boolean;
  f: any;

  constructor(
    @Inject(MAT_DIALOG_DATA)
    datos: {
      usuario: UsuarioAdmin | null;
      roles: { value: string; label: string }[];
      especialidades: { id: number; nombre: string }[];
    },
  ) {
    this.roles = datos.roles;
    this.especialidades = datos.especialidades;
    this.editando = !!datos.usuario;
    this.f = datos.usuario
      ? { ...datos.usuario, password: '' }
      : { username: '', password: '', nombre_completo: '', email: '', rol: 'medico',
          especialidad_id: null, registro_medico: '', registro_rethus: '' };
  }

  completo(): boolean { return !!this.f.nombre_completo && !!this.f.rol; }

  resultado(): any | null { return this.completo() ? this.f : null; }
}

/**
 * Restablecer la contraseña de un usuario.
 *
 * El mínimo de seis caracteres se comprueba aquí y deja el botón inerte: antes
 * se dejaba pulsar y el aviso llegaba después, que es enseñar la regla
 * chocándose con ella.
 */
@Component({
  selector: 'app-admin-reset-clave',
  standalone: true,
  imports: [FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatInputModule],
  template: `
    <h2 mat-dialog-title>Restablecer contraseña</h2>
    <mat-dialog-content>
      <p class="quien">Usuario: <strong>{{ u.nombre_completo }}</strong></p>
      <mat-form-field appearance="outline">
        <mat-label>Nueva contraseña (mín. 6 caracteres)</mat-label>
        <input matInput type="password" [(ngModel)]="clave" />
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" class="aviso" [disabled]="clave.length < 6"
        [mat-dialog-close]="clave">
        <i class="pi pi-key"></i> Restablecer
      </button>
    </mat-dialog-actions>
  `,
  styles: [`
    mat-dialog-content { min-width:min(22rem, 85vw); }
    .quien { color:#64748b; margin-bottom:1rem; }
    mat-form-field { width:100%; }
    .aviso { background:#f59e0b; color:#0f172a; }
  `],
})
export class AdminResetClaveComponent {
  clave = '';
  constructor(@Inject(MAT_DIALOG_DATA) public u: UsuarioAdmin) {}
}

/** Un campo adicional de la historia clínica. */
@Component({
  selector: 'app-admin-campo',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatSelectModule,
  ],
  template: `
    <h2 mat-dialog-title>{{ editando ? 'Editar campo' : 'Nuevo campo' }}</h2>
    <mat-dialog-content>
      <div class="rejilla">
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Etiqueta *</mat-label>
          <input matInput [(ngModel)]="f.etiqueta" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Tipo *</mat-label>
          <mat-select [(ngModel)]="f.tipo">
            @for (t of tipos; track t.value) { <mat-option [value]="t.value">{{ t.label }}</mat-option> }
          </mat-select>
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Orden</mat-label>
          <input matInput type="number" [(ngModel)]="f.orden" />
        </mat-form-field>
        @if (f.tipo === 'Lista Opciones') {
          <mat-form-field appearance="outline" class="ancho">
            <mat-label>Opciones (separadas por coma)</mat-label>
            <input matInput [(ngModel)]="f.opciones" placeholder="Opción 1, Opción 2, Opción 3" />
          </mat-form-field>
        }
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!f.etiqueta" [mat-dialog-close]="resultado()">
        <i class="pi pi-save"></i> Guardar
      </button>
    </mat-dialog-actions>
  `,
  styles: [`
    .rejilla { display:grid; grid-template-columns:1fr 1fr; gap:.875rem; padding-top:.5rem;
      min-width:min(26rem, 85vw); }
    .ancho { grid-column:1/-1; }
    mat-form-field { width:100%; }
  `],
})
export class AdminCampoComponent {
  readonly tipos: { value: string; label: string }[];
  readonly editando: boolean;
  f: any;

  constructor(
    @Inject(MAT_DIALOG_DATA)
    datos: { campo: ConfigCampo | null; tipos: { value: string; label: string }[] },
  ) {
    this.tipos = datos.tipos;
    this.editando = !!datos.campo;
    this.f = datos.campo ? { ...datos.campo } : { etiqueta: '', tipo: 'Texto', opciones: '', orden: 0 };
  }

  resultado(): any | null { return this.f.etiqueta ? this.f : null; }
}

/** Una entidad responsable de pago (EPS). */
@Component({
  selector: 'app-admin-entidad',
  standalone: true,
  imports: [FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatInputModule],
  template: `
    <h2 mat-dialog-title>{{ editando ? 'Editar entidad' : 'Nueva entidad' }}</h2>
    <mat-dialog-content>
      <div class="rejilla">
        <mat-form-field appearance="outline" class="ancho">
          <mat-label>Nombre *</mat-label>
          <input matInput [(ngModel)]="f.nombre" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>NIT *</mat-label>
          <input matInput [(ngModel)]="f.nit" [disabled]="editando" />
        </mat-form-field>
        <mat-form-field appearance="outline">
          <mat-label>Código EPS *</mat-label>
          <input matInput [(ngModel)]="f.codigo_eps" />
        </mat-form-field>
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!completo()" [mat-dialog-close]="resultado()">
        <i class="pi pi-save"></i> Guardar
      </button>
    </mat-dialog-actions>
  `,
  styles: [`
    .rejilla { display:grid; grid-template-columns:1fr 1fr; gap:.875rem; padding-top:.5rem;
      min-width:min(26rem, 85vw); }
    .ancho { grid-column:1/-1; }
    mat-form-field { width:100%; }
  `],
})
export class AdminEntidadComponent {
  readonly editando: boolean;
  f: any;

  constructor(@Inject(MAT_DIALOG_DATA) e: EntidadEPS | null) {
    this.editando = !!e;
    this.f = e ? { ...e } : { nit: '', nombre: '', codigo_eps: '' };
  }

  completo(): boolean { return !!this.f.nombre && !!this.f.nit && !!this.f.codigo_eps; }

  resultado(): any | null { return this.completo() ? this.f : null; }
}

@Component({
  selector: 'app-admin',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatFormFieldModule, MatInputModule, MatSelectModule,
    MatSlideToggleModule, MatTableModule,
    EtiquetaComponent, MatTooltipModule, MatDividerModule,
  ],
  template: `
    <div class="page-card">

      <div class="list-header">
        <h2 class="page-title">Administración</h2>
      </div>

      <!-- Tabs -->
      <div class="tab-bar">
        @for (t of tabs; track t.key) {
          <button class="tab-btn" [class.active]="tab() === t.key" (click)="setTab(t.key)">
            <i [class]="t.icon"></i> {{ t.label }}
          </button>
        }
      </div>

      <!-- ══════════════════ INSTITUCIÓN ══════════════════ -->
      @if (tab() === 'institucion') {
        @if (institucion()) {
          <div class="form-section">
            <h3 class="section-title">Datos Institucionales</h3>
            <div class="form-grid">
              <mat-form-field appearance="outline">
                <mat-label>Razón Social *</mat-label>
                <input matInput [(ngModel)]="institucion()!.razon_social" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Nombre Comercial *</mat-label>
                <input matInput [(ngModel)]="institucion()!.nombre_comercial" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>NIT</mat-label>
                <input matInput [ngModel]="institucion()!.nit" disabled />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Código Habilitación (REPS)</mat-label>
                <input matInput [(ngModel)]="institucion()!.codigo_habilitacion" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Código Prestador RIPS</mat-label>
                <input matInput [(ngModel)]="institucion()!.codigo_prestador" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Tipo Prestador</mat-label>
                <mat-select [(ngModel)]="institucion()!.tipo_prestador">
                  @for (t of tiposPrestador; track t) { <mat-option [value]="t">{{ t }}</mat-option> }
                </mat-select>
              </mat-form-field>
            </div>
            <mat-divider />
            <h3 class="section-title">Ubicación y Contacto</h3>
            <div class="form-grid">
              <mat-form-field appearance="outline" style="grid-column:span 2">
                <mat-label>Dirección</mat-label>
                <input matInput [(ngModel)]="institucion()!.direccion" class="w-full" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Ciudad</mat-label>
                <input matInput [(ngModel)]="institucion()!.ciudad" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Departamento</mat-label>
                <input matInput [(ngModel)]="institucion()!.departamento" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Teléfono</mat-label>
                <input matInput [(ngModel)]="institucion()!.telefono" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Email institucional</mat-label>
                <input matInput type="email" [(ngModel)]="institucion()!.email" />
              </mat-form-field>
            </div>
            <mat-divider />
            <h3 class="section-title">Representante Legal</h3>
            <div class="form-grid">
              <mat-form-field appearance="outline">
                <mat-label>Nombre</mat-label>
                <input matInput [(ngModel)]="institucion()!.representante_legal" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Identificación</mat-label>
                <input matInput [(ngModel)]="institucion()!.identificacion_representante" />
              </mat-form-field>
            </div>
            <mat-divider />
            <h3 class="section-title">Branding</h3>
            <div class="form-grid">
              <div class="color-fila">
                <input type="color" [(ngModel)]="institucion()!.color_primario" class="muestra" />
                <mat-form-field appearance="outline" style="flex:1">
                  <mat-label>Color primario</mat-label>
                  <input matInput [(ngModel)]="institucion()!.color_primario" />
                </mat-form-field>
              </div>
              <div class="color-fila">
                <input type="color" [(ngModel)]="institucion()!.color_secundario" class="muestra" />
                <mat-form-field appearance="outline" style="flex:1">
                  <mat-label>Color secundario</mat-label>
                  <input matInput [(ngModel)]="institucion()!.color_secundario" />
                </mat-form-field>
              </div>
            </div>
            <div class="form-actions">
              <button matButton="filled" [disabled]="guardando()" (click)="guardarInstitucion()">
                <i [class]="guardando() ? 'pi pi-spin pi-spinner' : 'pi pi-save'"></i> Guardar cambios
              </button>
            </div>
          </div>
        } @else {
          <div class="loading-state"><i class="pi pi-spin pi-spinner"></i></div>
        }
      }

      <!-- ══════════════════ CONFIGURACIÓN ══════════════════ -->
      @if (tab() === 'configuracion') {
        @if (config()) {
          <div class="form-section">
            <h3 class="section-title">Horario de Atención</h3>
            <div class="form-grid">
              <mat-form-field appearance="outline">
                <mat-label>Hora apertura</mat-label>
                <input matInput type="time" [(ngModel)]="config()!.horario_apertura" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Hora cierre</mat-label>
                <input matInput type="time" [(ngModel)]="config()!.horario_cierre" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Duración cita (min)</mat-label>
                <input matInput type="number" [(ngModel)]="config()!.duracion_cita_default" min="5" max="120" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Buffer entre citas (min)</mat-label>
                <input matInput type="number" [(ngModel)]="config()!.tiempo_buffer" min="0" max="60" />
              </mat-form-field>
              <div class="field-group" style="grid-column:span 2">
                <label>Días de atención</label>
                <div class="days-row">
                  @for (dia of diasSemana; track dia.key) {
                    <button class="day-btn"
                      [class.active]="config()!.dias_atencion?.includes(dia.key)"
                      (click)="toggleDia(dia.key)">{{ dia.label }}</button>
                  }
                </div>
              </div>
              <mat-form-field appearance="outline">
                <mat-label>Días máx. agendamiento</mat-label>
                <input matInput type="number" [(ngModel)]="config()!.dias_maximos_agendamiento" min="1" />
              </mat-form-field>
            </div>
            <mat-divider />
            <h3 class="section-title">Facturación</h3>
            <div class="form-grid">
              <mat-form-field appearance="outline">
                <mat-label>Prefijo factura</mat-label>
                <input matInput [(ngModel)]="config()!.prefijo_factura" maxlength="10" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Resolución DIAN</mat-label>
                <input matInput [(ngModel)]="config()!.resolucion_facturacion" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Rango desde</mat-label>
                <input matInput type="number" [(ngModel)]="config()!.rango_desde" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Rango hasta</mat-label>
                <input matInput type="number" [(ngModel)]="config()!.rango_hasta" />
              </mat-form-field>
            </div>
            <mat-divider />
            <h3 class="section-title">Opciones del Sistema</h3>
            <div class="toggle-grid">
              <div class="toggle-item">
                <mat-slide-toggle [(ngModel)]="config()!.permitir_citas_online" />
                <span>Permitir citas en línea</span>
              </div>
              <div class="toggle-item">
                <mat-slide-toggle [(ngModel)]="config()!.requiere_autorizacion" />
                <span>Requerir autorización EPS</span>
              </div>
              <div class="toggle-item">
                <mat-slide-toggle [(ngModel)]="config()!.enviar_recordatorios" />
                <span>Enviar recordatorios de citas</span>
              </div>
            </div>
            @if (config()!.enviar_recordatorios) {
              <div class="form-grid" style="margin-top:1rem">
                <mat-form-field appearance="outline">
                <mat-label>Horas anticipación recordatorio</mat-label>
                <input matInput type="number" [(ngModel)]="config()!.horas_anticipacion" min="1" max="72" />
              </mat-form-field>
                <mat-form-field appearance="outline">
                <mat-label>SMTP Host</mat-label>
                <input matInput [(ngModel)]="config()!.smtp_host" />
              </mat-form-field>
                <mat-form-field appearance="outline">
                <mat-label>SMTP Puerto</mat-label>
                <input matInput type="number" [(ngModel)]="config()!.smtp_port" />
              </mat-form-field>
                <mat-form-field appearance="outline">
                <mat-label>SMTP Usuario</mat-label>
                <input matInput [(ngModel)]="config()!.smtp_usuario" />
              </mat-form-field>
              </div>
            }
            <div class="form-actions">
              <button matButton="filled" [disabled]="guardando()" (click)="guardarConfig()">
                <i [class]="guardando() ? 'pi pi-spin pi-spinner' : 'pi pi-save'"></i> Guardar configuración
              </button>
            </div>
          </div>
        } @else {
          <div class="loading-state"><i class="pi pi-spin pi-spinner"></i></div>
        }
      }

      <!-- ══════════════════ USUARIOS ══════════════════ -->
      @if (tab() === 'usuarios') {
        <div class="list-header" style="margin-bottom:1rem">
          <div style="display:flex;gap:.5rem;align-items:center;flex-wrap:wrap">
            <mat-form-field appearance="outline" subscriptSizing="dynamic">
              <mat-label>Buscar</mat-label>
              <input matInput [(ngModel)]="busqUsuario" placeholder="Buscar..." (input)="cargarUsuarios()" />
            </mat-form-field>
            <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:180px">
              <mat-label>Rol</mat-label>
              <mat-select [(ngModel)]="filtroRolUsuario" (selectionChange)="cargarUsuarios()">
                <mat-option [value]="''">Todos los roles</mat-option>
                @for (r of ROLES; track r.value) {
                  <mat-option [value]="r.value">{{ r.label }}</mat-option>
                }
              </mat-select>
            </mat-form-field>
          </div>
          <button matButton="filled" (click)="abrirFormUsuario(null)">
            <i class="pi pi-plus"></i> Nuevo usuario
          </button>
        </div>
        <table mat-table [dataSource]="usuarios()" class="tabla">
          <ng-container matColumnDef="nombre">
            <th mat-header-cell *matHeaderCellDef>Nombre</th>
            <td mat-cell *matCellDef="let u" [class.row-inactivo]="!u.activo">
              {{ u.nombre_completo }}
              @if (u.cuenta_bloqueada) {
                <x-etiqueta value="Bloqueado" severity="danger" style="margin-left:.5rem" />
              }
              @if (u.requiere_cambio_password) {
                <x-etiqueta value="Cambiar clave" severity="warn" style="margin-left:.25rem" />
              }
            </td>
          </ng-container>
          <ng-container matColumnDef="username">
            <th mat-header-cell *matHeaderCellDef>Username</th>
            <td mat-cell *matCellDef="let u"><code style="font-size:.82rem">{{ u.username }}</code></td>
          </ng-container>
          <ng-container matColumnDef="rol">
            <th mat-header-cell *matHeaderCellDef>Rol</th>
            <td mat-cell *matCellDef="let u" style="font-size:.85rem">{{ rolLabel(u.rol) }}</td>
          </ng-container>
          <ng-container matColumnDef="especialidad">
            <th mat-header-cell *matHeaderCellDef>Especialidad</th>
            <td mat-cell *matCellDef="let u" style="font-size:.82rem;color:#64748b">
              {{ u.especialidad_nombre || '—' }}
            </td>
          </ng-container>
          <ng-container matColumnDef="estado">
            <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
            <td mat-cell *matCellDef="let u" class="centro">
              <x-etiqueta [value]="u.activo ? 'Activo' : 'Inactivo'"
                [severity]="u.activo ? 'success' : 'secondary'" />
            </td>
          </ng-container>
          <ng-container matColumnDef="acciones">
            <th mat-header-cell *matHeaderCellDef style="width:180px"></th>
            <td mat-cell *matCellDef="let u">
              <div style="display:flex;gap:.25rem;justify-content:flex-end">
                <button matIconButton matTooltip="Editar" (click)="abrirFormUsuario(u)">
                  <i class="pi pi-pencil"></i>
                </button>
                @if (u.cuenta_bloqueada) {
                  <button matIconButton class="aviso-color" matTooltip="Desbloquear" (click)="desbloquear(u)">
                    <i class="pi pi-unlock"></i>
                  </button>
                }
                <button matIconButton matTooltip="Reset contraseña" (click)="abrirResetPassword(u)">
                  <i class="pi pi-key"></i>
                </button>
                <button matIconButton class="peligro" matTooltip="Desactivar" (click)="desactivarUsuario(u)">
                  <i class="pi pi-ban"></i>
                </button>
              </div>
            </td>
          </ng-container>

          <tr mat-header-row *matHeaderRowDef="columnasUsuarios"></tr>
          <tr mat-row *matRowDef="let fila; columns: columnasUsuarios"></tr>
        </table>
        @if (!cargando() && usuarios().length === 0) {
          <p class="vacio">Sin usuarios</p>
        }
      }

      <!-- ══════════════════ SEGURIDAD ══════════════════ -->
      @if (tab() === 'seguridad') {
        @if (politica()) {
          <div class="form-section">
            <h3 class="section-title">Política de Contraseñas</h3>
            <div class="form-grid">
              <mat-form-field appearance="outline">
                <mat-label>Longitud mínima</mat-label>
                <input matInput type="number" [(ngModel)]="politica()!.longitud_minima_password" min="6" max="32" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Días expiración (0 = nunca)</mat-label>
                <input matInput type="number" [(ngModel)]="politica()!.dias_expiracion_password" min="0" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Historial (no reutilizar últimas N)</mat-label>
                <input matInput type="number" [(ngModel)]="politica()!.historial_passwords" min="0" max="24" />
              </mat-form-field>
            </div>
            <div class="toggle-grid" style="margin-top:.75rem">
              <div class="toggle-item">
                <mat-slide-toggle [(ngModel)]="politica()!.requiere_mayuscula" />
                <span>Requiere mayúscula</span>
              </div>
              <div class="toggle-item">
                <mat-slide-toggle [(ngModel)]="politica()!.requiere_minuscula" />
                <span>Requiere minúscula</span>
              </div>
              <div class="toggle-item">
                <mat-slide-toggle [(ngModel)]="politica()!.requiere_numero" />
                <span>Requiere número</span>
              </div>
              <div class="toggle-item">
                <mat-slide-toggle [(ngModel)]="politica()!.requiere_caracter_especial" />
                <span>Requiere carácter especial</span>
              </div>
            </div>
            <mat-divider />
            <h3 class="section-title">Sesiones</h3>
            <div class="form-grid">
              <mat-form-field appearance="outline">
                <mat-label>Timeout sesión (min)</mat-label>
                <input matInput type="number" [(ngModel)]="politica()!.timeout_sesion_minutos" min="5" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Sesiones simultáneas permitidas</mat-label>
                <input matInput type="number" [(ngModel)]="politica()!.sesiones_simultaneas_permitidas" min="1" />
              </mat-form-field>
            </div>
            <div class="toggle-grid" style="margin-top:.75rem">
              <div class="toggle-item">
                <mat-slide-toggle [(ngModel)]="politica()!.cerrar_otras_sesiones_al_login" />
                <span>Cerrar otras sesiones al iniciar</span>
              </div>
            </div>
            <mat-divider />
            <h3 class="section-title">Intentos de Login y Bloqueo</h3>
            <div class="form-grid">
              <mat-form-field appearance="outline">
                <mat-label>Máximo intentos fallidos</mat-label>
                <input matInput type="number" [(ngModel)]="politica()!.max_intentos_login" min="3" max="20" />
              </mat-form-field>
              <mat-form-field appearance="outline">
                <mat-label>Tiempo bloqueo (min)</mat-label>
                <input matInput type="number" [(ngModel)]="politica()!.tiempo_bloqueo_minutos" min="1" />
              </mat-form-field>
            </div>
            <mat-divider />
            <h3 class="section-title">Autenticación de Dos Factores (2FA)</h3>
            <div class="toggle-grid">
              <div class="toggle-item">
                <mat-slide-toggle [(ngModel)]="politica()!.requiere_2fa" />
                <span>Requerir 2FA para todos los usuarios</span>
              </div>
              <div class="toggle-item">
                <mat-slide-toggle [(ngModel)]="politica()!.requiere_2fa_admin" />
                <span>Requerir 2FA para administradores</span>
              </div>
            </div>
            @if (politica()!.requiere_2fa || politica()!.requiere_2fa_admin) {
              <div class="form-grid" style="margin-top:1rem">
                <mat-form-field appearance="outline">
                  <mat-label>Método 2FA</mat-label>
                  <mat-select [(ngModel)]="politica()!.metodo_2fa">
                    @for (m of metodos2fa; track m.value) {
                      <mat-option [value]="m.value">{{ m.label }}</mat-option>
                    }
                  </mat-select>
                </mat-form-field>
              </div>
            }
            <mat-divider />
            <h3 class="section-title">Auditoría y Notificaciones</h3>
            <div class="form-grid">
              <mat-form-field appearance="outline">
                <mat-label>Retener logs (días)</mat-label>
                <input matInput type="number" [(ngModel)]="politica()!.retener_logs_dias" min="30" />
              </mat-form-field>
            </div>
            <div class="toggle-grid" style="margin-top:.75rem">
              <div class="toggle-item">
                <mat-slide-toggle [(ngModel)]="politica()!.registrar_todos_los_accesos" />
                <span>Registrar todos los accesos</span>
              </div>
              <div class="toggle-item">
                <mat-slide-toggle [(ngModel)]="politica()!.notificar_login_nuevo_dispositivo" />
                <span>Notificar login desde nuevo dispositivo</span>
              </div>
              <div class="toggle-item">
                <mat-slide-toggle [(ngModel)]="politica()!.notificar_cambio_password" />
                <span>Notificar cambio de contraseña</span>
              </div>
              <div class="toggle-item">
                <mat-slide-toggle [(ngModel)]="politica()!.notificar_intentos_fallidos" />
                <span>Notificar intentos fallidos</span>
              </div>
            </div>
            <div class="form-actions">
              <button matButton="filled" [disabled]="guardando()" (click)="guardarPolitica()">
                <i [class]="guardando() ? 'pi pi-spin pi-spinner' : 'pi pi-save'"></i> Guardar política
              </button>
            </div>
          </div>
        } @else {
          <div class="loading-state"><i class="pi pi-spin pi-spinner"></i></div>
        }
      }

      <!-- ══════════════════ CAMPOS HC ══════════════════ -->
      @if (tab() === 'campos') {
        <div class="list-header" style="margin-bottom:1rem">
          <span style="color:#64748b;font-size:.9rem">Campos adicionales en la historia clínica</span>
          <button matButton="filled" (click)="abrirFormCampo(null)">
            <i class="pi pi-plus"></i> Nuevo campo
          </button>
        </div>
        <table mat-table [dataSource]="campos()" class="tabla">
          <ng-container matColumnDef="orden">
            <th mat-header-cell *matHeaderCellDef style="width:60px">Orden</th>
            <td mat-cell *matCellDef="let c" class="centro" style="color:#94a3b8">{{ c.orden }}</td>
          </ng-container>
          <ng-container matColumnDef="etiqueta">
            <th mat-header-cell *matHeaderCellDef>Etiqueta</th>
            <td mat-cell *matCellDef="let c"><strong>{{ c.etiqueta }}</strong></td>
          </ng-container>
          <ng-container matColumnDef="tipo">
            <th mat-header-cell *matHeaderCellDef style="width:150px">Tipo</th>
            <td mat-cell *matCellDef="let c"><x-etiqueta [value]="c.tipo" severity="info" /></td>
          </ng-container>
          <ng-container matColumnDef="opciones">
            <th mat-header-cell *matHeaderCellDef>Opciones</th>
            <td mat-cell *matCellDef="let c" style="font-size:.82rem;color:#64748b">{{ c.opciones || '—' }}</td>
          </ng-container>
          <ng-container matColumnDef="activo">
            <th mat-header-cell *matHeaderCellDef class="centro" style="width:90px">Activo</th>
            <td mat-cell *matCellDef="let c" class="centro">
              <mat-slide-toggle [ngModel]="c.activo" (ngModelChange)="toggleCampo(c, $event)" />
            </td>
          </ng-container>
          <ng-container matColumnDef="acciones">
            <th mat-header-cell *matHeaderCellDef style="width:100px"></th>
            <td mat-cell *matCellDef="let c">
              <div style="display:flex;gap:.25rem;justify-content:flex-end">
                <button matIconButton matTooltip="Editar" (click)="abrirFormCampo(c)">
                  <i class="pi pi-pencil"></i>
                </button>
                <button matIconButton class="peligro" matTooltip="Eliminar" (click)="eliminarCampo(c)">
                  <i class="pi pi-trash"></i>
                </button>
              </div>
            </td>
          </ng-container>

          <tr mat-header-row *matHeaderRowDef="columnasCampos"></tr>
          <tr mat-row *matRowDef="let fila; columns: columnasCampos"></tr>
        </table>
        @if (!cargando() && campos().length === 0) {
          <p class="vacio">Sin campos configurados</p>
        }
      }

      <!-- ══════════════════ AUDITORÍA ══════════════════ -->
      @if (tab() === 'auditoria') {
        <div class="list-header" style="margin-bottom:1rem;flex-wrap:wrap;gap:.5rem">
          <div style="display:flex;gap:.5rem;align-items:center;flex-wrap:wrap">
            <mat-form-field appearance="outline" subscriptSizing="dynamic">
              <mat-label>Desde</mat-label>
              <input matInput type="date" [(ngModel)]="audFechaInicio" />
            </mat-form-field>
            <mat-form-field appearance="outline" subscriptSizing="dynamic">
              <mat-label>Hasta</mat-label>
              <input matInput type="date" [(ngModel)]="audFechaFin" />
            </mat-form-field>
            <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:180px">
              <mat-label>Acción</mat-label>
              <input matInput [(ngModel)]="audAccion" placeholder="Filtrar acción..." />
            </mat-form-field>
            <button matButton="filled" (click)="cargarAuditoria()">
              <i class="pi pi-search"></i> Buscar
            </button>
          </div>
          <span style="font-size:.85rem;color:#64748b">{{ audTotal() }} registros</span>
        </div>
        <table mat-table [dataSource]="auditoria()" class="tabla">
          <ng-container matColumnDef="fecha">
            <th mat-header-cell *matHeaderCellDef style="width:155px">Fecha/Hora</th>
            <td mat-cell *matCellDef="let a" style="font-size:.8rem;color:#64748b">
              {{ a.fecha_hora | date:'dd/MM/yy HH:mm' }}
            </td>
          </ng-container>
          <ng-container matColumnDef="usuario">
            <th mat-header-cell *matHeaderCellDef>Usuario</th>
            <td mat-cell *matCellDef="let a" style="font-size:.85rem">{{ a.usuario_nombre }}</td>
          </ng-container>
          <ng-container matColumnDef="accion">
            <th mat-header-cell *matHeaderCellDef>Acción</th>
            <td mat-cell *matCellDef="let a">
              <code style="font-size:.78rem;background:#f1f5f9;padding:.1rem .3rem;border-radius:3px">{{ a.accion }}</code>
            </td>
          </ng-container>
          <ng-container matColumnDef="tabla">
            <th mat-header-cell *matHeaderCellDef>Tabla</th>
            <td mat-cell *matCellDef="let a" style="font-size:.82rem;color:#64748b">{{ a.tabla_afectada }}</td>
          </ng-container>
          <ng-container matColumnDef="registro">
            <th mat-header-cell *matHeaderCellDef class="centro" style="width:80px">Reg.</th>
            <td mat-cell *matCellDef="let a" class="centro" style="font-size:.82rem">{{ a.registro_id }}</td>
          </ng-container>
          <ng-container matColumnDef="ip">
            <th mat-header-cell *matHeaderCellDef style="width:120px">IP</th>
            <td mat-cell *matCellDef="let a" style="font-size:.78rem;color:#94a3b8">{{ a.ip_address || '—' }}</td>
          </ng-container>
          <ng-container matColumnDef="ok">
            <th mat-header-cell *matHeaderCellDef class="centro" style="width:70px">OK</th>
            <td mat-cell *matCellDef="let a" class="centro">
              <x-etiqueta [value]="a.exitoso ? 'OK' : 'Err'" [severity]="a.exitoso ? 'success' : 'danger'" />
            </td>
          </ng-container>

          <tr mat-header-row *matHeaderRowDef="columnasAuditoria"></tr>
          <tr mat-row *matRowDef="let fila; columns: columnasAuditoria"></tr>
        </table>
        @if (!cargando() && auditoria().length === 0) {
          <p class="vacio">Sin registros</p>
        }
        @if (audPages() > 1) {
          <div style="display:flex;justify-content:center;align-items:center;gap:.5rem;margin-top:1rem">
            <button matIconButton [disabled]="audPage() === 1" matTooltip="Anterior" (click)="audPrev()">
              <i class="pi pi-angle-left"></i>
            </button>
            <span style="font-size:.85rem">Pág {{ audPage() }} / {{ audPages() }}</span>
            <button matIconButton [disabled]="audPage() >= audPages()" matTooltip="Siguiente" (click)="audNext()">
              <i class="pi pi-angle-right"></i>
            </button>
          </div>
        }
      }

      <!-- ══════════════════ CATÁLOGOS ══════════════════ -->
      @if (tab() === 'catalogos') {
        <div class="tab-bar" style="margin-bottom:1rem">
          <button class="tab-btn" [class.active]="catTab() === 'especialidades'" (click)="catTab.set('especialidades')">
            <i class="pi pi-star"></i> Especialidades
          </button>
          <button class="tab-btn" [class.active]="catTab() === 'entidades'" (click)="catTab.set('entidades')">
            <i class="pi pi-building"></i> Entidades / EPS
          </button>
        </div>

        @if (catTab() === 'especialidades') {
          <div class="list-header" style="margin-bottom:1rem">
            <span style="color:#64748b;font-size:.9rem">{{ especialidades().length }} especialidades registradas</span>
            <div style="display:flex;gap:.5rem">
              <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:220px">
                <mat-label>Nueva especialidad</mat-label>
                <input matInput [(ngModel)]="nuevaEspecialidad" (keyup.enter)="crearEspecialidad()" />
              </mat-form-field>
              <button matButton="filled" (click)="crearEspecialidad()">
                <i class="pi pi-plus"></i> Agregar
              </button>
            </div>
          </div>
          <div class="chips-grid">
            @for (e of especialidades(); track e.id) {
              <div class="chip">
                <span>{{ e.nombre }}</span>
                <button class="chip-del" (click)="eliminarEspecialidad(e)"><i class="pi pi-times"></i></button>
              </div>
            }
            @if (especialidades().length === 0) {
              <span style="color:#94a3b8;font-size:.9rem">Sin especialidades</span>
            }
          </div>
        }

        @if (catTab() === 'entidades') {
          <div class="list-header" style="margin-bottom:1rem">
            <mat-form-field appearance="outline" subscriptSizing="dynamic" style="width:260px">
              <mat-label>Buscar EPS</mat-label>
              <input matInput [(ngModel)]="busqEntidad" (input)="cargarEntidades()" />
            </mat-form-field>
            <button matButton="filled" (click)="abrirFormEntidad(null)">
              <i class="pi pi-plus"></i> Nueva entidad
            </button>
          </div>
          <table mat-table [dataSource]="entidades()" class="tabla">
            <ng-container matColumnDef="nombre">
              <th mat-header-cell *matHeaderCellDef>Nombre</th>
              <td mat-cell *matCellDef="let e">{{ e.nombre }}</td>
            </ng-container>
            <ng-container matColumnDef="nit">
              <th mat-header-cell *matHeaderCellDef style="width:140px">NIT</th>
              <td mat-cell *matCellDef="let e" style="font-size:.85rem;color:#64748b">{{ e.nit }}</td>
            </ng-container>
            <ng-container matColumnDef="codigo">
              <th mat-header-cell *matHeaderCellDef style="width:120px">Código EPS</th>
              <td mat-cell *matCellDef="let e"><code style="font-size:.82rem">{{ e.codigo_eps }}</code></td>
            </ng-container>
            <ng-container matColumnDef="acciones">
              <th mat-header-cell *matHeaderCellDef style="width:90px"></th>
              <td mat-cell *matCellDef="let e">
                <div style="display:flex;gap:.25rem;justify-content:flex-end">
                  <button matIconButton matTooltip="Editar" (click)="abrirFormEntidad(e)">
                    <i class="pi pi-pencil"></i>
                  </button>
                  <button matIconButton class="peligro" matTooltip="Eliminar" (click)="eliminarEntidad(e)">
                    <i class="pi pi-trash"></i>
                  </button>
                </div>
              </td>
            </ng-container>

          <tr mat-header-row *matHeaderRowDef="columnasEntidades"></tr>
          <tr mat-row *matRowDef="let fila; columns: columnasEntidades"></tr>
        </table>
        @if (!cargando() && entidades().length === 0) {
          <p class="vacio">Sin entidades</p>
        }
        }
      }

      <!-- ══════════════════ PERMISOS (capacidad × rol) ══════════════════ -->
      @if (tab() === 'capacidades') {
        <div class="form-section">
          <h3 class="section-title">Quién puede hacer qué, en esta institución</h3>
          <p class="ayuda">
            Cada casilla marcada permite esa acción a ese rol. Lo que no se toca sigue
            el <strong>valor recomendado</strong>, que se mueve solo si el sistema lo
            actualiza; lo que se aparta queda fijo y se marca en ámbar.
          </p>

          @if (cargandoCaps()) {
            <div class="loading-state">Cargando…</div>
          } @else if (matriz()) {
            <div class="leyenda">
              <span class="lg"><span class="pt recomendado"></span> sigue lo recomendado</span>
              <span class="lg"><span class="pt apartado"></span> apartado por esta institución</span>
              <span class="lg"><span class="pt bloqueado"></span> no configurable</span>
            </div>

            <div class="matriz-scroll">
              <table class="matriz">
                <thead>
                  <tr>
                    <th class="esq">Acción</th>
                    @for (r of rolesMatriz(); track r) {
                      <th class="rot"><span>{{ etiquetaRol(r) }}</span></th>
                    }
                  </tr>
                </thead>
                <tbody>
                  @for (f of matriz()!.capacidades; track f.capacidad) {
                    <tr [class.fila-bloqueada]="!f.configurable">
                      <th class="cap">
                        {{ etiquetaCapacidad(f.capacidad) }}
                        @if (!f.configurable) {
                          <i class="pi pi-lock" matTooltip="Se gobierna por encima de la institución"></i>
                        }
                      </th>
                      @for (c of f.roles; track c.rol) {
                        <td
                          class="celda"
                          [class.apartada]="c.apartado"
                          [class.bloqueada]="!c.configurable"
                          [matTooltip]="explicar(f, c)">
                          <input
                            type="checkbox"
                            [checked]="c.permitido"
                            [disabled]="!c.configurable || guardando()"
                            (change)="marcar(f, c, $event)" />
                        </td>
                      }
                    </tr>
                  }
                </tbody>
              </table>
            </div>

            <div class="form-actions" style="display:flex;gap:.5rem;align-items:center;flex-wrap:wrap">
              <button matButton="filled" [disabled]="pendientes().length === 0 || guardando()"
                (click)="guardarCapacidades()">
                <i [class]="guardando() ? 'pi pi-spin pi-spinner' : 'pi pi-save'"></i> Guardar cambios
              </button>
              <button matButton [disabled]="pendientes().length === 0 || guardando()"
                (click)="cargarCapacidades()">Descartar</button>
              @if (seleccionApartada()) {
                <button matButton [disabled]="guardando()" (click)="devolverTodoAlDefecto()">
                  <i class="pi pi-undo"></i> Devolver lo apartado a lo recomendado
                </button>
              }
              @if (pendientes().length) {
                <span class="pendientes">{{ pendientes().length }} sin guardar</span>
              }
            </div>
          } @else {
            <div class="loading-state">No se pudo cargar la matriz.</div>
          }
        </div>
      }

    </div>

  `,
  styles: [`
    .list-header { display:flex; justify-content:space-between; align-items:center; margin-bottom:1rem; flex-wrap:wrap; gap:.5rem }
    .tabla { width:100% }
    .centro { text-align:center }
    .vacio { text-align:center; padding:2rem; color:#94a3b8 }
    mat-form-field { width:100% }
    .color-fila { display:flex; gap:.5rem; align-items:center }
    .muestra { height:38px; width:60px; border:1px solid #e2e8f0; border-radius:6px; cursor:pointer }
    /* En M3 el input color de un botón no pinta nada: el color va por CSS. */
    .peligro { color:#dc2626 }
    .aviso-color { color:#f59e0b }
    .page-title { font-size:1.4rem; font-weight:700; color:#1e293b; margin:0 }

    .tab-bar { display:flex; gap:.25rem; border-bottom:2px solid #e2e8f0; margin-bottom:1.5rem; overflow-x:auto }
    .tab-btn { background:none; border:none; padding:.55rem .9rem; cursor:pointer; font-size:.86rem; color:#64748b; border-bottom:2px solid transparent; margin-bottom:-2px; display:flex; align-items:center; gap:.4rem; white-space:nowrap }
    .tab-btn.active { color:#3b82f6; border-bottom-color:#3b82f6; font-weight:600 }
    .tab-btn:hover:not(.active) { color:#1e293b; background:#f8fafc }

    .form-section { max-width:900px }
    .section-title { font-size:1rem; font-weight:600; color:#374151; margin:0 0 1rem }
    .form-grid { display:grid; grid-template-columns:repeat(2,1fr); gap:1rem }
    .field-group { display:flex; flex-direction:column; gap:.35rem }
    .field-group label { font-size:.82rem; font-weight:500; color:#374151 }
    .w-full { width:100% }
    .form-actions { margin-top:1.5rem }

    .toggle-grid { display:grid; grid-template-columns:repeat(auto-fill,minmax(260px,1fr)); gap:.75rem }
    .toggle-item { display:flex; align-items:center; gap:.75rem; font-size:.88rem; color:#374151 }

    .days-row { display:flex; gap:.5rem; flex-wrap:wrap; margin-top:.25rem }
    .day-btn { padding:.35rem .7rem; border-radius:6px; border:1px solid #e2e8f0; background:white; cursor:pointer; font-size:.82rem; color:#64748b; transition:all .15s }
    .day-btn.active { background:#3b82f6; color:white; border-color:#3b82f6; font-weight:600 }

    .chips-grid { display:flex; flex-wrap:wrap; gap:.5rem; padding:.5rem 0 }
    .chip { display:flex; align-items:center; gap:.4rem; background:#eff6ff; border:1px solid #bfdbfe; border-radius:20px; padding:.3rem .75rem; font-size:.85rem; color:#1d4ed8 }
    .chip-del { background:none; border:none; cursor:pointer; color:#93c5fd; font-size:.75rem; padding:0; line-height:1 }
    .chip-del:hover { color:#ef4444 }

    .dialog-form { margin:.5rem 0 }
    .row-inactivo { opacity:.6 }
    .loading-state { text-align:center; padding:3rem; color:#94a3b8 }

    /* Matriz de permisos (#235) */
    .ayuda { font-size:.85rem; color:#64748b; margin:.25rem 0 1rem; max-width:60ch; line-height:1.5 }
    .leyenda { display:flex; gap:1.25rem; flex-wrap:wrap; margin-bottom:.75rem; font-size:.8rem; color:#64748b }
    .lg { display:flex; align-items:center; gap:.4rem }
    .pt { width:.8rem; height:.8rem; border-radius:3px; border:1px solid #cbd5e1; display:inline-block }
    .pt.recomendado { background:white }
    .pt.apartado { background:#fef3c7; border-color:#f59e0b }
    .pt.bloqueado { background:#f1f5f9; border-color:#e2e8f0 }

    /* La tabla se desborda a lo ancho con 15 roles: scroll DENTRO de su caja,
       nunca en la página. */
    .matriz-scroll { overflow-x:auto; border:1px solid #e2e8f0; border-radius:8px }
    .matriz { border-collapse:collapse; font-size:.82rem; min-width:100% }
    .matriz th.esq { text-align:left; padding:.5rem .75rem; background:#f8fafc; position:sticky; left:0; z-index:2 }
    .matriz th.cap { text-align:left; padding:.45rem .75rem; font-weight:500; color:#334155;
                     white-space:nowrap; background:white; position:sticky; left:0; z-index:1;
                     border-right:1px solid #e2e8f0 }
    .matriz th.cap i { color:#94a3b8; margin-left:.35rem; font-size:.75rem }
    .matriz th.rot { padding:.5rem .25rem; background:#f8fafc; vertical-align:bottom; height:8.5rem }
    .matriz th.rot span { writing-mode:vertical-rl; transform:rotate(180deg); white-space:nowrap;
                          font-weight:500; color:#475569 }
    .matriz tbody tr:nth-child(even) th.cap { background:#fcfdfe }
    .matriz tbody tr:hover th.cap, .matriz tbody tr:hover .celda { background:#f0f9ff }
    .celda { text-align:center; padding:.3rem; border-left:1px solid #f1f5f9; min-width:2.2rem }
    .celda.apartada { background:#fef3c7 }
    .celda.bloqueada { background:#f8fafc }
    .celda input { cursor:pointer; width:1rem; height:1rem }
    .celda input:disabled { cursor:not-allowed; opacity:.45 }
    .fila-bloqueada th.cap { color:#94a3b8 }
    .pendientes { font-size:.82rem; color:#b45309; font-weight:500 }
  `],
})
export class AdminComponent implements OnInit {
  private svc = inject(AdminService);
  private dialogo = inject(MatDialog);

  /** El orden de las columnas lo fija esta lista, no la plantilla. */
  readonly columnasUsuarios = ['nombre', 'username', 'rol', 'especialidad', 'estado', 'acciones'];
  readonly columnasCampos = ['orden', 'etiqueta', 'tipo', 'opciones', 'activo', 'acciones'];
  readonly columnasAuditoria = ['fecha', 'usuario', 'accion', 'tabla', 'registro', 'ip', 'ok'];
  readonly columnasEntidades = ['nombre', 'nit', 'codigo', 'acciones'];

  /** Los tipos de prestador con los que se habilita ante el REPS. */
  readonly tiposPrestador = ['IPS', 'CLINICA', 'HOSPITAL', 'CONSULTORIO', 'LABORATORIO'];

  readonly metodos2fa = [
    { value: 'EMAIL', label: 'Correo electrónico' },
    { value: 'SMS', label: 'SMS' },
    { value: 'TOTP', label: 'App autenticadora (TOTP)' },
  ];
  private msg = inject(Avisos);

  tab = signal<Tab>('institucion');
  catTab = signal<CatTab>('especialidades');

  institucion = signal<Institucion | null>(null);
  config = signal<ConfiguracionInstitucion | null>(null);
  politica = signal<PoliticaSeguridad | null>(null);
  usuarios = signal<UsuarioAdmin[]>([]);
  campos = signal<ConfigCampo[]>([]);
  auditoria = signal<AuditoriaItem[]>([]);
  audTotal = signal(0);
  audPage = signal(1);
  readonly audSize = 50;
  especialidades = signal<Especialidad[]>([]);
  entidades = signal<EntidadEPS[]>([]);

  guardando = signal(false);
  cargando = signal(false);

  busqUsuario = '';
  filtroRolUsuario = '';




  audFechaInicio = '';
  audFechaFin = '';
  audAccion = '';

  nuevaEspecialidad = '';
  busqEntidad = '';


  readonly ROLES = ROLES;
  readonly diasSemana = DIAS_SEMANA;
  readonly tiposCampo = TIPOS_CAMPO;

  readonly tabs = [
    { key: 'institucion' as Tab, label: 'Institución', icon: 'pi pi-building' },
    { key: 'configuracion' as Tab, label: 'Configuración', icon: 'pi pi-cog' },
    { key: 'usuarios' as Tab, label: 'Usuarios', icon: 'pi pi-users' },
    { key: 'seguridad' as Tab, label: 'Seguridad', icon: 'pi pi-shield' },
    { key: 'campos' as Tab, label: 'Campos HC', icon: 'pi pi-list' },
    { key: 'auditoria' as Tab, label: 'Auditoría', icon: 'pi pi-history' },
    { key: 'catalogos' as Tab, label: 'Catálogos', icon: 'pi pi-database' },
    { key: 'capacidades' as Tab, label: 'Permisos', icon: 'pi pi-key' },
  ];

  audPages() { return Math.ceil(this.audTotal() / this.audSize) || 1; }

  ngOnInit(): void {
    this.cargarInstitucion();
    this.cargarConfig();
    this.cargarPolitica();
    this.cargarUsuarios();
    this.cargarCampos();
    this.cargarEspecialidades();
    this.cargarEntidades();
    const hoy = new Date();
    this.audFechaInicio = new Date(hoy.getFullYear(), hoy.getMonth(), 1).toISOString().slice(0, 10);
    this.audFechaFin = hoy.toISOString().slice(0, 10);
    this.cargarAuditoria();
  }

  setTab(t: Tab): void {
    this.tab.set(t);
    // La matriz se pide al entrar y no en ngOnInit: es la única pestaña cuya
    // carga cuesta una consulta que la mayoría de las visitas a /admin no usa.
    if (t === 'capacidades' && !this.matriz()) this.cargarCapacidades();
  }

  // ── Permisos: matriz capacidad × rol (#235) ───────────────────────────────

  matriz = signal<MatrizCapacidades | null>(null);
  cargandoCaps = signal(false);
  /// Los cambios que el administrador ha marcado y aún no ha guardado, indexados
  /// por `capacidad|rol`. `null` = devolver al defecto.
  private cambios = signal<Map<string, boolean | null>>(new Map());

  pendientes(): CambioCapacidad[] {
    return [...this.cambios().entries()].map(([clave, permitido]) => {
      const [capacidad, rol] = clave.split('|');
      return { capacidad, rol, permitido };
    });
  }

  /// Los roles, en el orden en que los devuelve el servidor. Se toman de la
  /// primera fila en vez de de la constante ROLES de este archivo: el servidor
  /// es quien sabe qué roles existen, y una segunda lista aquí se quedaría atrás
  /// el día que se añada uno.
  rolesMatriz(): string[] {
    return this.matriz()?.capacidades[0]?.roles.map(c => c.rol) ?? [];
  }

  seleccionApartada(): boolean {
    return (this.matriz()?.capacidades ?? []).some(f => f.roles.some(c => c.apartado));
  }

  cargarCapacidades(): void {
    this.cargandoCaps.set(true);
    this.cambios.set(new Map());
    this.svc.getCapacidades().subscribe({
      next: m => { this.matriz.set(m); this.cargandoCaps.set(false); },
      error: () => { this.cargandoCaps.set(false); this.err('No se pudo cargar la matriz de permisos'); },
    });
  }

  /// Marca una casilla en local. No se envía hasta «Guardar cambios» porque el
  /// servidor valida el LOTE contra el estado resultante: quitarle la gestión de
  /// usuarios a un rol es válido si el mismo lote se la da a otro, y enviar cada
  /// casilla por separado haría que el orden de los clics decidiera si el cambio
  /// pasa.
  marcar(f: FilaCapacidad, c: CasillaCapacidad, ev: Event): void {
    const valor = (ev.target as HTMLInputElement).checked;
    // Optimista sobre la copia local, para que la casilla responda al clic; el
    // servidor devuelve la matriz real al guardar y esto se sustituye.
    c.permitido = valor;
    c.apartado = valor !== c.defecto;
    const m = new Map(this.cambios());
    m.set(`${f.capacidad}|${c.rol}`, valor);
    this.cambios.set(m);
  }

  guardarCapacidades(): void {
    const cambios = this.pendientes();
    if (!cambios.length) return;
    this.guardando.set(true);
    this.svc.updateCapacidades(cambios).subscribe({
      next: m => {
        this.matriz.set(m);
        this.cambios.set(new Map());
        this.guardando.set(false);
        this.ok('Permisos actualizados');
      },
      error: e => {
        this.guardando.set(false);
        // El 422 trae el motivo del guardarraíl en texto plano; es el mensaje
        // que el administrador necesita («dejaría a la institución sin ningún
        // rol que pueda gestionar usuarios»), así que se muestra tal cual.
        this.err(typeof e?.error === 'string' ? e.error : (e?.error?.detail ?? 'No se pudo guardar'));
        // Y se relee, porque no se aplicó nada: dejar la pantalla con las
        // casillas movidas haría creer que el cambio está puesto.
        this.cargarCapacidades();
      },
    });
  }

  /// Borra TODAS las excepciones y devuelve la institución al defecto entero.
  devolverTodoAlDefecto(): void {
    const cambios: CambioCapacidad[] = [];
    for (const f of this.matriz()?.capacidades ?? []) {
      for (const c of f.roles) {
        if (c.apartado && c.configurable) {
          cambios.push({ capacidad: f.capacidad, rol: c.rol, permitido: null });
        }
      }
    }
    if (!cambios.length) return;
    this.guardando.set(true);
    this.svc.updateCapacidades(cambios).subscribe({
      next: m => {
        this.matriz.set(m);
        this.cambios.set(new Map());
        this.guardando.set(false);
        this.ok('Todo vuelve a lo recomendado');
      },
      error: e => {
        this.guardando.set(false);
        this.err(typeof e?.error === 'string' ? e.error : 'No se pudo devolver al valor recomendado');
        this.cargarCapacidades();
      },
    });
  }

  explicar(f: FilaCapacidad, c: CasillaCapacidad): string {
    if (!f.configurable) return 'Esta acción se gobierna por encima de la institución';
    if (!c.configurable) return 'Este rol no se configura por institución';
    const base = c.defecto ? 'recomendado: permitido' : 'recomendado: no permitido';
    return c.apartado ? `Apartado por esta institución (${base})` : base;
  }

  etiquetaRol(rol: string): string {
    return ROLES.find(r => r.value === rol)?.label
      ?? (rol === 'superadmin' ? 'Superadmin' : rol);
  }

  etiquetaCapacidad(cap: string): string {
    return CAPACIDADES[cap] ?? cap;
  }

  // ── Institución ───────────────────────────────────────────────────────────
  private cargarInstitucion() {
    this.svc.getInstitucion().subscribe({ next: r => this.institucion.set(r), error: () => {} });
  }
  guardarInstitucion() {
    if (!this.institucion()) return;
    this.guardando.set(true);
    this.svc.updateInstitucion(this.institucion()!).subscribe({
      next: r => { this.institucion.set(r); this.guardando.set(false); this.ok('Institución actualizada'); },
      error: e => { this.guardando.set(false); this.err(e?.error?.detail ?? 'Error'); },
    });
  }

  // ── Configuración ─────────────────────────────────────────────────────────
  private cargarConfig() {
    this.svc.getConfiguracion().subscribe({ next: r => this.config.set(r), error: () => {} });
  }
  guardarConfig() {
    if (!this.config()) return;
    this.guardando.set(true);
    this.svc.updateConfiguracion(this.config()!).subscribe({
      next: r => { this.config.set(r); this.guardando.set(false); this.ok('Configuración guardada'); },
      error: e => { this.guardando.set(false); this.err(e?.error?.detail ?? 'Error'); },
    });
  }
  toggleDia(dia: string) {
    if (!this.config()) return;
    const dias = [...(this.config()!.dias_atencion ?? [])];
    const idx = dias.indexOf(dia);
    if (idx >= 0) dias.splice(idx, 1); else dias.push(dia);
    this.config.update(c => c ? { ...c, dias_atencion: dias } : c);
  }

  // ── Política ──────────────────────────────────────────────────────────────
  private cargarPolitica() {
    this.svc.getPolitica().subscribe({ next: r => this.politica.set(r), error: () => {} });
  }
  guardarPolitica() {
    if (!this.politica()) return;
    this.guardando.set(true);
    this.svc.updatePolitica(this.politica()!).subscribe({
      next: r => { this.politica.set(r); this.guardando.set(false); this.ok('Política guardada'); },
      error: e => { this.guardando.set(false); this.err(e?.error?.detail ?? 'Error'); },
    });
  }

  // ── Usuarios ──────────────────────────────────────────────────────────────
  cargarUsuarios() {
    this.cargando.set(true);
    const params: Record<string, string> = {};
    if (this.busqUsuario) params['q'] = this.busqUsuario;
    if (this.filtroRolUsuario) params['rol'] = this.filtroRolUsuario;
    this.svc.listUsuarios(params).subscribe({
      next: r => { this.usuarios.set(r); this.cargando.set(false); },
      error: () => this.cargando.set(false),
    });
  }
  abrirFormUsuario(u: UsuarioAdmin | null) {
    this.dialogo
      .open(AdminUsuarioComponent, {
        data: { usuario: u, roles: this.ROLES, especialidades: this.especialidades() },
        maxWidth: '95vw',
      })
      .afterClosed()
      .subscribe((datos?: any) => { if (datos) this.guardarUsuario(u, datos); });
  }

  private guardarUsuario(editando: UsuarioAdmin | null, datos: any) {
    this.guardando.set(true);
    const obs = editando
      ? this.svc.updateUsuario(editando.id, datos)
      : this.svc.createUsuario(datos);
    obs.subscribe({
      next: () => {
        this.guardando.set(false);
        this.ok(editando ? 'Usuario actualizado' : 'Usuario creado');
        this.cargarUsuarios();
      },
      error: e => { this.guardando.set(false); this.err(e?.error?.detail ?? 'Error'); },
    });
  }
  desactivarUsuario(u: UsuarioAdmin) {
    if (!confirm(`¿Desactivar a ${u.nombre_completo}?`)) return;
    this.svc.deleteUsuario(u.id).subscribe({
      next: () => { this.ok('Usuario desactivado'); this.cargarUsuarios(); },
      error: e => this.err(e?.error?.detail ?? 'Error'),
    });
  }
  desbloquear(u: UsuarioAdmin) {
    this.svc.desbloquearUsuario(u.id).subscribe({
      next: () => { this.ok('Cuenta desbloqueada'); this.cargarUsuarios(); },
      error: e => this.err(e?.error?.detail ?? 'Error'),
    });
  }
  abrirResetPassword(u: UsuarioAdmin) {
    this.dialogo
      .open(AdminResetClaveComponent, { data: u, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((clave?: string) => { if (clave) this.restablecerClave(u, clave); });
  }

  private restablecerClave(u: UsuarioAdmin, clave: string) {
    this.guardando.set(true);
    this.svc.resetPassword(u.id, clave).subscribe({
      next: (r: any) => { this.guardando.set(false); this.ok(r.mensaje); },
      error: e => { this.guardando.set(false); this.err(e?.error?.detail ?? 'Error'); },
    });
  }

  // ── Campos HC ─────────────────────────────────────────────────────────────
  private cargarCampos() {
    this.svc.listCampos().subscribe({ next: r => this.campos.set(r), error: () => {} });
  }
  abrirFormCampo(c: ConfigCampo | null) {
    this.dialogo
      .open(AdminCampoComponent, { data: { campo: c, tipos: this.tiposCampo }, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((datos?: any) => { if (datos) this.guardarCampo(c, datos); });
  }

  private guardarCampo(editando: ConfigCampo | null, datos: any) {
    this.guardando.set(true);
    const obs = editando
      ? this.svc.updateCampo(editando.id, datos)
      : this.svc.createCampo(datos);
    obs.subscribe({
      next: () => { this.guardando.set(false); this.ok('Campo guardado'); this.cargarCampos(); },
      error: e => { this.guardando.set(false); this.err(e?.error?.detail ?? 'Error'); },
    });
  }
  toggleCampo(c: ConfigCampo, activo: boolean) {
    this.svc.updateCampo(c.id, { activo }).subscribe({
      next: r => this.campos.update(arr => arr.map(x => x.id === c.id ? r : x)),
      error: e => this.err(e?.error?.detail ?? 'Error'),
    });
  }
  eliminarCampo(c: ConfigCampo) {
    if (!confirm(`¿Eliminar campo "${c.etiqueta}"?`)) return;
    this.svc.deleteCampo(c.id).subscribe({
      next: () => { this.ok('Campo eliminado'); this.cargarCampos(); },
      error: e => this.err(e?.error?.detail ?? 'Error'),
    });
  }

  // ── Auditoría ─────────────────────────────────────────────────────────────
  cargarAuditoria() {
    this.cargando.set(true);
    const params: Record<string, string> = { page: String(this.audPage()), size: String(this.audSize) };
    if (this.audFechaInicio) params['fecha_inicio'] = this.audFechaInicio;
    if (this.audFechaFin) params['fecha_fin'] = this.audFechaFin;
    if (this.audAccion) params['accion'] = this.audAccion;
    this.svc.listAuditoria(params).subscribe({
      next: r => { this.auditoria.set(r.items); this.audTotal.set(r.total); this.cargando.set(false); },
      error: () => this.cargando.set(false),
    });
  }
  audPrev() { this.audPage.update(p => p - 1); this.cargarAuditoria(); }
  audNext() { this.audPage.update(p => p + 1); this.cargarAuditoria(); }

  // ── Especialidades ────────────────────────────────────────────────────────
  private cargarEspecialidades() {
    this.svc.listEspecialidades().subscribe({ next: r => this.especialidades.set(r), error: () => {} });
  }
  crearEspecialidad() {
    if (!this.nuevaEspecialidad.trim()) return;
    this.svc.createEspecialidad(this.nuevaEspecialidad.trim()).subscribe({
      next: () => { this.ok('Especialidad creada'); this.nuevaEspecialidad = ''; this.cargarEspecialidades(); },
      error: e => this.err(e?.error?.detail ?? 'Error'),
    });
  }
  eliminarEspecialidad(e: Especialidad) {
    if (!confirm(`¿Eliminar "${e.nombre}"?`)) return;
    this.svc.deleteEspecialidad(e.id).subscribe({
      next: () => { this.ok('Eliminada'); this.cargarEspecialidades(); },
      error: err => this.err(err?.error?.detail ?? 'Error'),
    });
  }

  // ── Entidades ─────────────────────────────────────────────────────────────
  cargarEntidades() {
    this.svc.listEntidades(this.busqEntidad || undefined).subscribe({
      next: r => this.entidades.set(r), error: () => {},
    });
  }
  abrirFormEntidad(e: EntidadEPS | null) {
    this.dialogo
      .open(AdminEntidadComponent, { data: e, maxWidth: '95vw' })
      .afterClosed()
      .subscribe((datos?: any) => { if (datos) this.guardarEntidad(e, datos); });
  }

  private guardarEntidad(editando: EntidadEPS | null, datos: any) {
    this.guardando.set(true);
    const obs = editando
      ? this.svc.updateEntidad(editando.id, datos)
      : this.svc.createEntidad(datos);
    obs.subscribe({
      next: () => { this.guardando.set(false); this.ok('Guardado'); this.cargarEntidades(); },
      error: e => { this.guardando.set(false); this.err(e?.error?.detail ?? 'Error'); },
    });
  }
  eliminarEntidad(e: EntidadEPS) {
    if (!confirm(`¿Eliminar "${e.nombre}"?`)) return;
    this.svc.deleteEntidad(e.id).subscribe({
      next: () => { this.ok('Eliminada'); this.cargarEntidades(); },
      error: err => this.err(err?.error?.detail ?? 'Error'),
    });
  }

  // ── Helpers ───────────────────────────────────────────────────────────────
  rolLabel(rol: string): string { return ROLES.find(r => r.value === rol)?.label ?? rol; }
  private ok(d: string) { this.msg.add({ severity: 'success', summary: 'OK', detail: d, life: 3000 }); }
  private err(d: string) { this.msg.add({ severity: 'error', summary: 'Error', detail: d, life: 5000 }); }
}
