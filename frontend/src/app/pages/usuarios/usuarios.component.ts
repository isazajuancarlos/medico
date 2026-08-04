// SPDX-License-Identifier: LicenseRef-Propietario
import { Component, Inject, OnInit, ViewChild, computed, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatPaginator, MatPaginatorModule } from '@angular/material/paginator';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatSelectModule } from '@angular/material/select';
import { MatTableDataSource, MatTableModule } from '@angular/material/table';
import { MatTooltipModule } from '@angular/material/tooltip';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { Avisos } from '../../shared/ui/avisos.service';
import { Usuario, UsuariosService } from '../../services/usuarios.service';

type TipoDoc = { codigo: string; nombre: string };

/**
 * El documento de identidad de un profesional (P11/P12 del RIPS).
 *
 * Los tipos SE PIDEN al servidor, no se escriben aquí: son los ocho que la
 * Resolución 948 admite para quien REALIZA la atención, y salen del maestro del
 * validador. Una lista en el navegador y otra en `muv.rs` divergen en cuanto el
 * maestro cambie, y la del navegador es la que ve el médico.
 */
@Component({
  selector: 'app-usuario-documento',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule,
    MatFormFieldModule, MatInputModule, MatSelectModule,
  ],
  template: `
    <h2 mat-dialog-title>Documento de {{ datos.usuario.nombre }}</h2>
    <mat-dialog-content>
      <p class="ayuda">
        El RIPS exige el documento de quien atiende en los cinco archivos. Sin él
        la atención se reporta incompleta.
      </p>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Tipo</mat-label>
        <mat-select [(ngModel)]="tipo">
          <mat-option [value]="null">— sin documento —</mat-option>
          @for (t of datos.tipos; track t.codigo) {
            <mat-option [value]="t.codigo">{{ t.codigo }} — {{ t.nombre }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Número</mat-label>
        <input matInput [(ngModel)]="numero" [disabled]="!tipo" />
      </mat-form-field>
      @if (!tipo && numero) {
        <p class="aviso">Elija el tipo, o borre el número: medio documento no identifica a nadie.</p>
      }
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!valido()" [mat-dialog-close]="resultado()">
        Guardar
      </button>
    </mat-dialog-actions>
  `,
  styles: [`
    mat-form-field { width: 100%; margin-bottom: .75rem; }
    .ayuda { color: rgba(0,0,0,.6); font-size: .85rem; margin: 0 0 1rem; }
    .aviso { color: #b45309; font-size: .85rem; margin: .25rem 0 0; }
  `],
})
export class UsuarioDocumentoComponent {
  tipo: string | null = null;
  numero = '';

  constructor(
    @Inject(MAT_DIALOG_DATA) public datos: { usuario: Usuario; tipos: TipoDoc[] },
  ) {
    // Se abre con lo que YA tiene: un diálogo de corrección que arranca vacío
    // invita a reescribir desde cero un dato que estaba bien.
    this.tipo = datos.usuario.tipo_documento;
    this.numero = datos.usuario.numero_documento ?? '';
  }

  /** O los dos, o ninguno. Vaciar los dos BORRA el documento, y es válido. */
  valido(): boolean {
    const n = this.numero.trim();
    return (!this.tipo && !n) || (!!this.tipo && !!n);
  }

  resultado(): { tipo: string | null; numero: string | null } {
    const n = this.numero.trim();
    return this.tipo && n ? { tipo: this.tipo, numero: n } : { tipo: null, numero: null };
  }
}

/**
 * La planta del prestador: quién puede entrar al sistema, y el documento con
 * el que firma lo que atiende.
 *
 * # Por qué esta pantalla existe aparte de «Administración»
 *
 * `/admin` existe y tiene su pestaña de usuarios, pero habla ÚNICAMENTE con
 * `/api/admin`, que hoy es el backend Python: contra el binario da 404. Su
 * formulario pide además `username`, `especialidad_id`, `registro_medico`,
 * `registro_rethus` y `cuenta_bloqueada`, y ninguno de esos campos existe en la
 * tabla `usuario` de Rust —la especialidad y el registro llegan con el maestro
 * de talento humano, y el bloqueo no existe porque en su lugar hay un freno que
 * se agota solo—.
 *
 * Esperar al port completo de `admin` dejaba P11/P12 vacío para TODOS los
 * profesionales del sistema, que es un campo que el RIPS exige. Esta pantalla
 * hace lo que el binario ya sabe hacer, y lo hace con las rutas que ya existen:
 * cuando `admin` se porte, esto es su pestaña de usuarios, no trabajo tirado.
 */
@Component({
  selector: 'app-usuarios',
  standalone: true,
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatDialogModule, MatFormFieldModule, MatInputModule,
    MatPaginatorModule, MatProgressBarModule, MatSelectModule, MatTableModule,
    MatTooltipModule, EtiquetaComponent,
  ],
  template: `
    <div class="page-card">
      <div class="cabecera">
        <h1>Usuarios</h1>
        <div class="acciones">
          <span class="resumen">
            {{ sinDocumento() }} de {{ total() }} sin documento
          </span>
          <button matButton="filled" (click)="abrirAlta()">
            <i class="pi pi-plus"></i> Nuevo usuario
          </button>
        </div>
      </div>

      @if (cargando()) { <mat-progress-bar mode="indeterminate" /> }

      @if (sinDocumento() > 0) {
        <p class="aviso-rips">
          El RIPS exige el documento de quien atiende. Las atenciones de los
          {{ sinDocumento() }} profesionales sin documento se reportan incompletas.
        </p>
      }

      <table mat-table [dataSource]="tabla" class="w-full">
        <ng-container matColumnDef="nombre">
          <th mat-header-cell *matHeaderCellDef>Nombre</th>
          <td mat-cell *matCellDef="let u">{{ u.nombre }}</td>
        </ng-container>
        <ng-container matColumnDef="email">
          <th mat-header-cell *matHeaderCellDef>Correo</th>
          <td mat-cell *matCellDef="let u">{{ u.email }}</td>
        </ng-container>
        <ng-container matColumnDef="rol">
          <th mat-header-cell *matHeaderCellDef>Rol</th>
          <td mat-cell *matCellDef="let u">{{ u.rol }}</td>
        </ng-container>
        <ng-container matColumnDef="documento">
          <th mat-header-cell *matHeaderCellDef>Documento (RIPS)</th>
          <td mat-cell *matCellDef="let u">
            @if (u.numero_documento) {
              {{ u.tipo_documento }} {{ u.numero_documento }}
            } @else {
              <x-etiqueta value="Sin documento" severity="warn" />
            }
          </td>
        </ng-container>
        <ng-container matColumnDef="estado">
          <th mat-header-cell *matHeaderCellDef>Estado</th>
          <td mat-cell *matCellDef="let u">
            <x-etiqueta [value]="u.activo ? 'Activo' : 'Inactivo'"
                        [severity]="u.activo ? 'success' : 'secondary'" />
          </td>
        </ng-container>
        <ng-container matColumnDef="acciones">
          <th mat-header-cell *matHeaderCellDef></th>
          <td mat-cell *matCellDef="let u">
            <button matIconButton (click)="abrirDocumento(u)"
                    matTooltip="Documento del profesional (RIPS)">
              <i class="pi pi-id-card"></i>
            </button>
          </td>
        </ng-container>
        <tr mat-header-row *matHeaderRowDef="columnas"></tr>
        <tr mat-row *matRowDef="let fila; columns: columnas"></tr>
      </table>
      <mat-paginator [pageSizeOptions]="[10, 25, 50]" showFirstLastButtons />
    </div>
  `,
  styles: [`
    .page-card { padding: 1.25rem; }
    .cabecera { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
    .cabecera h1 { margin: 0; font-size: 1.4rem; }
    .acciones { display: flex; gap: 1rem; align-items: center; }
    .resumen { color: rgba(0,0,0,.6); font-size: .9rem; }
    .aviso-rips {
      background: #fef3c7; border-left: 4px solid #d97706; color: #78350f;
      padding: .75rem 1rem; margin: 0 0 1rem; border-radius: 4px; font-size: .9rem;
    }
    .w-full { width: 100%; }
  `],
})
export class UsuariosComponent implements OnInit {
  private srv = inject(UsuariosService);
  private dialogo = inject(MatDialog);
  private avisos = inject(Avisos);

  readonly columnas = ['nombre', 'email', 'rol', 'documento', 'estado', 'acciones'];
  readonly tabla = new MatTableDataSource<Usuario>([]);
  @ViewChild(MatPaginator) set paginador(p: MatPaginator) {
    if (p) this.tabla.paginator = p;
  }

  // Señales: la plantilla las lee y las escribe un `subscribe`. Con OnPush, un
  // campo llano no repintaría nada y la pantalla se quedaría con lo anterior.
  readonly usuarios = signal<Usuario[]>([]);
  readonly tipos = signal<TipoDoc[]>([]);
  readonly cargando = signal(false);
  readonly total = computed(() => this.usuarios().length);
  /**
   * Cuántos NO tienen documento: el número que dice si el RIPS va completo.
   *
   * Derivado con `computed` y no guardado en una señal aparte: dos fuentes para
   * el mismo hecho se desincronizan en el primer camino que actualice una y
   * olvide la otra, y aquí el que olvidara sería el de después de guardar.
   */
  readonly sinDocumento = computed(
    () => this.usuarios().filter((u) => !u.numero_documento).length,
  );

  ngOnInit(): void {
    this.cargar();
    this.srv.tiposDocumento().subscribe({
      next: (t) => this.tipos.set(t),
      // Sin los tipos NO se puede ofrecer el selector, y escribirlos aquí sería
      // la segunda lista que este proyecto no tiene. Se avisa y se deja el
      // diálogo cerrado.
      error: () => this.avisos.error('No se pudo cargar la lista de tipos de documento.'),
    });
  }

  private cargar(): void {
    this.cargando.set(true);
    this.srv.listar().subscribe({
      next: (us) => {
        this.usuarios.set(us);
        this.tabla.data = us;
        this.cargando.set(false);
      },
      error: () => {
        this.cargando.set(false);
        this.avisos.error('No se pudieron cargar los usuarios.');
      },
    });
  }

  abrirDocumento(u: Usuario): void {
    if (!this.tipos().length) {
      this.avisos.error('No hay lista de tipos de documento; no se puede editar sin ella.');
      return;
    }
    this.dialogo
      .open(UsuarioDocumentoComponent, {
        data: { usuario: u, tipos: this.tipos() },
        width: '460px',
      })
      .afterClosed()
      .subscribe((r?: { tipo: string | null; numero: string | null }) => {
        if (!r) return;
        this.srv.fijarDocumento(u.id, r.tipo, r.numero).subscribe({
          next: () => {
            this.avisos.exito(r.tipo ? 'Documento guardado.' : 'Documento borrado.');
            this.cargar();
          },
          // El error se ENSEÑA y no se traga: si el servidor rechazó el tipo o
          // el documento ya es de otro profesional, cerrar el diálogo en
          // silencio dejaría a quien lo escribió creyendo que se guardó.
          error: (e) =>
            this.avisos.error(e?.error?.detail ?? 'No se pudo guardar el documento.'),
        });
      });
  }

  abrirAlta(): void {
    this.dialogo
      .open(UsuarioAltaComponent, { data: { tipos: this.tipos() }, width: '460px' })
      .afterClosed()
      .subscribe((nuevo) => {
        if (!nuevo) return;
        this.srv.crear(nuevo).subscribe({
          next: () => {
            this.avisos.exito('Usuario creado.');
            this.cargar();
          },
          error: (e) => this.avisos.error(e?.error?.detail ?? 'No se pudo crear el usuario.'),
        });
      });
  }
}

/** Alta de un usuario, con su documento opcional desde el primer momento. */
@Component({
  selector: 'app-usuario-alta',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule,
    MatFormFieldModule, MatInputModule, MatSelectModule,
  ],
  template: `
    <h2 mat-dialog-title>Nuevo usuario</h2>
    <mat-dialog-content>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Nombre *</mat-label>
        <input matInput [(ngModel)]="nombre" />
      </mat-form-field>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Correo *</mat-label>
        <input matInput type="email" [(ngModel)]="email" />
      </mat-form-field>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Clave *</mat-label>
        <input matInput type="password" [(ngModel)]="clave" />
      </mat-form-field>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Rol *</mat-label>
        <mat-select [(ngModel)]="rol">
          @for (r of roles; track r) {
            <mat-option [value]="r">{{ r }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Tipo de documento</mat-label>
        <mat-select [(ngModel)]="tipo">
          <mat-option [value]="null">— después —</mat-option>
          @for (t of datos.tipos; track t.codigo) {
            <mat-option [value]="t.codigo">{{ t.codigo }} — {{ t.nombre }}</mat-option>
          }
        </mat-select>
      </mat-form-field>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Número de documento</mat-label>
        <input matInput [(ngModel)]="numero" [disabled]="!tipo" />
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!valido()" [mat-dialog-close]="resultado()">
        Crear
      </button>
    </mat-dialog-actions>
  `,
  styles: [`mat-form-field { width: 100%; margin-bottom: .75rem; }`],
})
export class UsuarioAltaComponent {
  nombre = '';
  email = '';
  clave = '';
  rol = 'medico';
  tipo: string | null = null;
  numero = '';

  /** Los roles del dominio. El servidor rechaza cualquier otro. */
  readonly roles = [
    'admin', 'medico', 'enfermeria', 'auxiliar_enfermeria', 'regente_farmacia',
    'laboratorista', 'radiologo', 'anestesiologo', 'terapeuta', 'nutricionista',
    'psicologo', 'trabajo_social', 'bacteriologo',
  ];

  constructor(@Inject(MAT_DIALOG_DATA) public datos: { tipos: TipoDoc[] }) {}

  valido(): boolean {
    const doc = (!this.tipo && !this.numero.trim()) || (!!this.tipo && !!this.numero.trim());
    return !!this.nombre.trim() && !!this.email.trim() && this.clave.length >= 8 && doc;
  }

  resultado() {
    const n = this.numero.trim();
    return {
      nombre: this.nombre.trim(),
      email: this.email.trim(),
      clave: this.clave,
      rol: this.rol,
      tipo_documento: this.tipo && n ? this.tipo : null,
      numero_documento: this.tipo && n ? n : null,
    };
  }
}
