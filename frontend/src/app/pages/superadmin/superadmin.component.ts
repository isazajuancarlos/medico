import { Component, Inject, OnInit, ViewChild, computed, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatButtonModule } from '@angular/material/button';
import { MatDatepickerModule } from '@angular/material/datepicker';
import {
  MAT_DIALOG_DATA,
  MatDialog,
  MatDialogModule,
} from '@angular/material/dialog';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatPaginator, MatPaginatorModule } from '@angular/material/paginator';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { MatTableDataSource, MatTableModule } from '@angular/material/table';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { Avisos } from '../../shared/ui/avisos.service';
import { SuperadminService, Institucion, UsuarioResumen } from '../../services/superadmin.service';

/** Alta de una IPS. Devuelve los dos datos, o nada. */
@Component({
  selector: 'app-superadmin-ips',
  standalone: true,
  imports: [FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatInputModule],
  template: `
    <h2 mat-dialog-title>Nueva IPS</h2>
    <mat-dialog-content>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Nombre *</mat-label>
        <input matInput [(ngModel)]="nombre" />
      </mat-form-field>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>NIT *</mat-label>
        <input matInput [(ngModel)]="nit" />
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!completo()" [mat-dialog-close]="resultado()">
        <i class="pi pi-check"></i> Crear
      </button>
    </mat-dialog-actions>
  `,
  styles: [`
    mat-dialog-content { display:flex; flex-direction:column; gap:1rem; padding-top:.5rem; }
    mat-form-field { width:100%; }
  `],
})
export class SuperadminIpsComponent {
  nombre = '';
  nit = '';
  completo(): boolean { return !!this.nombre.trim() && !!this.nit.trim(); }
  resultado(): { nombre: string; nit: string } | null {
    return this.completo() ? { nombre: this.nombre.trim(), nit: this.nit.trim() } : null;
  }
}

/** El plan de una IPS: cupo de usuarios y vencimiento opcional. */
@Component({
  selector: 'app-superadmin-licencia',
  standalone: true,
  imports: [
    FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule,
    MatInputModule, MatDatepickerModule,
  ],
  template: `
    <h2 mat-dialog-title>Licencia — {{ ips.nombre }}</h2>
    <mat-dialog-content>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Cupo de usuarios *</mat-label>
        <input matInput type="number" min="1" [(ngModel)]="maxUsuarios" />
      </mat-form-field>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Vence (opcional)</mat-label>
        <input matInput [matDatepicker]="cal" [(ngModel)]="vence" placeholder="Sin vencimiento" />
        <mat-datepicker-toggle matIconSuffix [for]="cal" />
        <mat-datepicker #cal />
      </mat-form-field>
      <small>
        Vacío = perpetua. Una licencia vencida bloquea crear usuarios, nunca la lectura.
      </small>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!completo()" [mat-dialog-close]="resultado()">
        <i class="pi pi-check"></i> Fijar plan
      </button>
    </mat-dialog-actions>
  `,
  styles: [`
    mat-dialog-content { display:flex; flex-direction:column; gap:1rem; padding-top:.5rem; }
    mat-form-field { width:100%; }
    small { color:#64748b; }
  `],
})
export class SuperadminLicenciaComponent {
  maxUsuarios: number | null = 10;
  vence: Date | null = null;

  constructor(@Inject(MAT_DIALOG_DATA) public ips: Institucion) {}

  completo(): boolean { return !!this.maxUsuarios && Number(this.maxUsuarios) >= 1; }

  resultado(): { max_usuarios: number; vence: string | null } | null {
    if (!this.completo()) return null;
    return {
      max_usuarios: Number(this.maxUsuarios),
      // El día LOCAL, no el UTC. El calendario entrega la medianoche local, y
      // `toISOString().slice(0,10)` —que es lo que había— la traduce a UTC: en
      // Colombia (UTC-5) sale el mismo día y no se nota, pero al este de
      // Greenwich sale el ANTERIOR. Una licencia que vence un día antes de lo
      // pactado bloquea el alta de usuarios sin que nadie sepa por qué, y el
      // día en que se descubra no habrá forma de reproducirlo desde Medellín.
      vence: this.vence ? diaLocal(this.vence) : null,
    };
  }
}

/** El primer administrador de una IPS recién dada de alta. */
@Component({
  selector: 'app-superadmin-admin',
  standalone: true,
  imports: [FormsModule, MatDialogModule, MatButtonModule, MatFormFieldModule, MatInputModule],
  template: `
    <h2 mat-dialog-title>Nuevo administrador — {{ ips.nombre }}</h2>
    <mat-dialog-content>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Nombre *</mat-label>
        <input matInput [(ngModel)]="nombre" />
      </mat-form-field>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Correo *</mat-label>
        <input matInput type="email" [(ngModel)]="email" autocomplete="username" />
      </mat-form-field>
      <mat-form-field appearance="outline" subscriptSizing="dynamic">
        <mat-label>Contraseña *</mat-label>
        <input matInput [type]="verClave ? 'text' : 'password'" [(ngModel)]="clave"
          autocomplete="new-password" />
        <button matIconButton matSuffix type="button" (click)="verClave = !verClave"
          [attr.aria-label]="verClave ? 'Ocultar contraseña' : 'Mostrar contraseña'"
          [attr.aria-pressed]="verClave">
          <i class="pi" [class.pi-eye]="!verClave" [class.pi-eye-slash]="verClave"></i>
        </button>
      </mat-form-field>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button matButton mat-dialog-close>Cancelar</button>
      <button matButton="filled" [disabled]="!completo()" [mat-dialog-close]="resultado()">
        <i class="pi pi-check"></i> Crear admin
      </button>
    </mat-dialog-actions>
  `,
  styles: [`
    mat-dialog-content { display:flex; flex-direction:column; gap:1rem; padding-top:.5rem; }
    mat-form-field { width:100%; }
  `],
})
export class SuperadminAdminComponent {
  nombre = '';
  email = '';
  clave = '';
  verClave = false;

  constructor(@Inject(MAT_DIALOG_DATA) public ips: Institucion) {}

  completo(): boolean {
    return !!this.nombre.trim() && !!this.email.trim() && !!this.clave;
  }

  resultado(): { nombre: string; email: string; clave: string } | null {
    if (!this.completo()) return null;
    return { nombre: this.nombre.trim(), email: this.email.trim(), clave: this.clave };
  }
}

/** `2026-08-02` en la zona del navegador. Ver el comentario de arriba. */
function diaLocal(d: Date): string {
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  return `${d.getFullYear()}-${mm}-${dd}`;
}

@Component({
  selector: 'app-superadmin',
  standalone: true,
  imports: [
    CommonModule, MatTableModule, MatPaginatorModule, MatButtonModule,
    MatProgressBarModule, EtiquetaComponent,
  ],
  template: `
<div class="page-card">
  <div class="fila-titulo">
    <div>
      <h2 class="page-title" style="margin:0">Gestión multi-institucional</h2>
      <p style="color:#64748b;margin:.25rem 0 0;font-size:.9rem">
        Dé de alta una IPS, fije su plan y cree su primer administrador.
      </p>
    </div>
    <button matButton="filled" (click)="abrirIps()">
      <i class="pi pi-plus"></i> Nueva IPS
    </button>
  </div>

  @if (cargando() || guardando()) { <mat-progress-bar mode="indeterminate" /> }

  <table mat-table [dataSource]="ips">
    <ng-container matColumnDef="nombre">
      <th mat-header-cell *matHeaderCellDef>IPS</th>
      <td mat-cell *matCellDef="let i">{{ i.nombre }}</td>
    </ng-container>
    <ng-container matColumnDef="nit">
      <th mat-header-cell *matHeaderCellDef>NIT</th>
      <td mat-cell *matCellDef="let i">{{ i.nit }}</td>
    </ng-container>
    <ng-container matColumnDef="estado">
      <th mat-header-cell *matHeaderCellDef>Estado</th>
      <td mat-cell *matCellDef="let i">
        <x-etiqueta [value]="i.activa ? 'Activa' : 'Inactiva'"
          [severity]="i.activa ? 'success' : 'secondary'" />
      </td>
    </ng-container>
    <ng-container matColumnDef="acciones">
      <th mat-header-cell *matHeaderCellDef>Acciones</th>
      <td mat-cell *matCellDef="let i" class="acciones">
        <button matButton (click)="abrirLicencia(i)">
          <i class="pi pi-id-card"></i> Licencia
        </button>
        <button matButton (click)="abrirAdmin(i)">
          <i class="pi pi-user-plus"></i> Crear admin
        </button>
      </td>
    </ng-container>

    <tr mat-header-row *matHeaderRowDef="colIps"></tr>
    <tr mat-row *matRowDef="let fila; columns: colIps"></tr>
  </table>

  @if (!cargando() && ips.data.length === 0) {
    <p class="vacio">Aún no hay instituciones. Cree la primera.</p>
  }
  <mat-paginator #pagIps [pageSize]="10" [pageSizeOptions]="[10, 25, 50]" />

  <h3 style="margin:2rem 0 .5rem">Usuarios</h3>

  <table mat-table [dataSource]="usuarios">
    <ng-container matColumnDef="nombre">
      <th mat-header-cell *matHeaderCellDef>Nombre</th>
      <td mat-cell *matCellDef="let u">{{ u.nombre }}</td>
    </ng-container>
    <ng-container matColumnDef="correo">
      <th mat-header-cell *matHeaderCellDef>Correo</th>
      <td mat-cell *matCellDef="let u">{{ u.email }}</td>
    </ng-container>
    <ng-container matColumnDef="rol">
      <th mat-header-cell *matHeaderCellDef>Rol</th>
      <td mat-cell *matCellDef="let u">{{ u.rol }}</td>
    </ng-container>
    <ng-container matColumnDef="institucion">
      <th mat-header-cell *matHeaderCellDef>Institución</th>
      <td mat-cell *matCellDef="let u">{{ nombreInstitucion(u.institucion_id) }}</td>
    </ng-container>
    <ng-container matColumnDef="estado">
      <th mat-header-cell *matHeaderCellDef>Estado</th>
      <td mat-cell *matCellDef="let u">
        <x-etiqueta [value]="u.activo ? 'Activo' : 'Inactivo'"
          [severity]="u.activo ? 'success' : 'secondary'" />
      </td>
    </ng-container>

    <tr mat-header-row *matHeaderRowDef="colUsuarios"></tr>
    <tr mat-row *matRowDef="let fila; columns: colUsuarios"></tr>
  </table>

  @if (usuarios.data.length === 0) { <p class="vacio">Sin usuarios.</p> }
  <mat-paginator #pagUsuarios [pageSize]="10" [pageSizeOptions]="[10, 25, 50]" />
</div>
  `,
  styles: [`
    .fila-titulo { display:flex; justify-content:space-between; align-items:center; margin-bottom:1rem }
    table { width:100%; }
    .acciones { white-space:nowrap; }
    .vacio { text-align:center; padding:2rem; color:#64748b; }
  `],
})
export class SuperadminComponent implements OnInit {
  private svc = inject(SuperadminService);
  private msg = inject(Avisos);
  private dialogo = inject(MatDialog);

  readonly colIps = ['nombre', 'nit', 'estado', 'acciones'];
  readonly colUsuarios = ['nombre', 'correo', 'rol', 'institucion', 'estado'];

  readonly ips = new MatTableDataSource<Institucion>([]);
  readonly usuarios = new MatTableDataSource<UsuarioResumen>([]);

  // Dos tablas y dos paginadores en la misma pantalla, así que se enganchan por
  // referencia de plantilla: un `@ViewChild(MatPaginator)` a secas devuelve el
  // primero y dejaría el segundo mudo sin que nada falle.
  @ViewChild('pagIps')
  set paginadorIps(p: MatPaginator | undefined) { if (p) this.ips.paginator = p; }

  @ViewChild('pagUsuarios')
  set paginadorUsuarios(p: MatPaginator | undefined) { if (p) this.usuarios.paginator = p; }

  cargando = signal(true);
  guardando = signal(false);

  private nombrePorId = computed(() => {
    const m = new Map<number, string>();
    for (const i of this.listaIps()) m.set(i.id, i.nombre);
    return m;
  });
  private listaIps = signal<Institucion[]>([]);

  ngOnInit(): void {
    this.cargar();
  }

  nombreInstitucion(id: number | null): string {
    return id ? (this.nombrePorId().get(id) ?? '—') : 'Sistema';
  }

  private cargar(): void {
    this.cargando.set(true);
    this.svc.listInstituciones().subscribe({
      next: (is) => { this.ips.data = is; this.listaIps.set(is); this.cargando.set(false); },
      error: () => this.cargando.set(false),
    });
    this.svc.listUsuarios().subscribe({ next: (us) => (this.usuarios.data = us), error: () => {} });
  }

  abrirIps(): void {
    this.dialogo
      .open(SuperadminIpsComponent, { width: '420px', maxWidth: '95vw' })
      .afterClosed()
      .subscribe((d?: { nombre: string; nit: string } | null) => { if (d) this.crearIps(d); });
  }

  private crearIps(d: { nombre: string; nit: string }): void {
    this.guardando.set(true);
    this.svc.crearInstitucion(d.nombre, d.nit).subscribe({
      next: () => {
        this.guardando.set(false);
        this.msg.add({ severity: 'success', summary: 'IPS creada' });
        this.cargar();
      },
      error: (e) => {
        this.guardando.set(false);
        const detalle = e?.status === 409 ? 'Ya existe una IPS con ese NIT.' : 'No se pudo crear la IPS.';
        this.msg.add({ severity: 'error', summary: 'Error', detail: detalle });
      },
    });
  }

  abrirLicencia(i: Institucion): void {
    this.dialogo
      .open(SuperadminLicenciaComponent, { data: i, width: '440px', maxWidth: '95vw' })
      .afterClosed()
      .subscribe((d?: { max_usuarios: number; vence: string | null } | null) => {
        if (d) this.fijarLicencia(i, d);
      });
  }

  private fijarLicencia(i: Institucion, d: { max_usuarios: number; vence: string | null }): void {
    this.guardando.set(true);
    this.svc.fijarLicencia(i.id, d.max_usuarios, d.vence).subscribe({
      next: () => {
        this.guardando.set(false);
        this.msg.add({ severity: 'success', summary: 'Plan fijado' });
      },
      error: () => {
        this.guardando.set(false);
        this.msg.add({ severity: 'error', summary: 'Error', detail: 'No se pudo fijar el plan.' });
      },
    });
  }

  abrirAdmin(i: Institucion): void {
    this.dialogo
      .open(SuperadminAdminComponent, { data: i, width: '460px', maxWidth: '95vw' })
      .afterClosed()
      .subscribe((d?: { nombre: string; email: string; clave: string } | null) => {
        if (d) this.crearAdmin(i, d);
      });
  }

  private crearAdmin(i: Institucion, d: { nombre: string; email: string; clave: string }): void {
    this.guardando.set(true);
    this.svc.crearUsuario({ ...d, rol: 'admin', institucion_id: i.id }).subscribe({
      next: () => {
        this.guardando.set(false);
        this.msg.add({ severity: 'success', summary: 'Administrador creado' });
        this.cargar();
      },
      error: (e) => {
        this.guardando.set(false);
        this.msg.add({ severity: 'error', summary: 'No se pudo crear', detail: this.motivo(e) });
      },
    });
  }

  /** Traduce el bloqueo del backend a un aviso accionable. */
  private motivo(e: { status?: number; error?: { motivo?: string } }): string {
    if (e?.status === 409) return 'El correo ya está registrado.';
    if (e?.status === 402) {
      const m: Record<string, string> = {
        sin_licencia: 'La IPS no tiene un plan activo. Fije la licencia primero.',
        licencia_vencida: 'La licencia de la IPS está vencida. Renuévela.',
        cupo_lleno: 'La IPS alcanzó el cupo de usuarios de su plan.',
      };
      return m[e.error?.motivo ?? ''] ?? 'El plan de la IPS no permite más usuarios.';
    }
    return 'No se pudo crear el usuario.';
  }
}
