import { Component, OnInit, computed, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { TableModule } from 'primeng/table';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { PasswordModule } from 'primeng/password';
import { DialogModule } from 'primeng/dialog';
import { DatePickerModule } from 'primeng/datepicker';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { Avisos } from '../../shared/ui/avisos.service';
import { SuperadminService, Institucion, UsuarioResumen } from '../../services/superadmin.service';

@Component({
  selector: 'app-superadmin',
  standalone: true,
  imports: [
    CommonModule, FormsModule, TableModule, ButtonModule, InputTextModule,
    PasswordModule, DialogModule, DatePickerModule, EtiquetaComponent,
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
    <p-button label="Nueva IPS" icon="pi pi-plus" size="small" (onClick)="abrirIps()" />
  </div>

  <p-table [value]="instituciones()" [loading]="cargando()" dataKey="id" styleClass="p-datatable-sm"
           [paginator]="instituciones().length > 10" [rows]="10">
    <ng-template pTemplate="header">
      <tr><th>IPS</th><th>NIT</th><th>Estado</th><th style="width:1%">Acciones</th></tr>
    </ng-template>
    <ng-template pTemplate="body" let-i>
      <tr>
        <td>{{ i.nombre }}</td>
        <td>{{ i.nit }}</td>
        <td><x-etiqueta [value]="i.activa ? 'Activa' : 'Inactiva'" [severity]="i.activa ? 'success' : 'secondary'" /></td>
        <td style="white-space:nowrap">
          <p-button label="Licencia" icon="pi pi-id-card" [text]="true" size="small" (onClick)="abrirLicencia(i)" />
          <p-button label="Crear admin" icon="pi pi-user-plus" [text]="true" size="small" severity="secondary" (onClick)="abrirAdmin(i)" />
        </td>
      </tr>
    </ng-template>
    <ng-template pTemplate="emptymessage">
      <tr><td colspan="4" style="text-align:center;padding:2rem;color:#64748b">Aún no hay instituciones. Cree la primera.</td></tr>
    </ng-template>
  </p-table>

  <h3 style="margin:2rem 0 .5rem">Usuarios</h3>
  <p-table [value]="usuarios()" dataKey="id" styleClass="p-datatable-sm" [paginator]="usuarios().length > 10" [rows]="10">
    <ng-template pTemplate="header">
      <tr><th>Nombre</th><th>Correo</th><th>Rol</th><th>Institución</th><th>Estado</th></tr>
    </ng-template>
    <ng-template pTemplate="body" let-u>
      <tr>
        <td>{{ u.nombre }}</td>
        <td>{{ u.email }}</td>
        <td>{{ u.rol }}</td>
        <td>{{ nombreInstitucion(u.institucion_id) }}</td>
        <td><x-etiqueta [value]="u.activo ? 'Activo' : 'Inactivo'" [severity]="u.activo ? 'success' : 'secondary'" /></td>
      </tr>
    </ng-template>
    <ng-template pTemplate="emptymessage">
      <tr><td colspan="5" style="text-align:center;padding:1.5rem;color:#64748b">Sin usuarios.</td></tr>
    </ng-template>
  </p-table>
</div>

<p-dialog [(visible)]="dlgIps" header="Nueva IPS" [modal]="true" [style]="{width:'420px'}">
  <div class="campo"><label>Nombre *</label><input pInputText [(ngModel)]="ips.nombre" style="width:100%" /></div>
  <div class="campo"><label>NIT *</label><input pInputText [(ngModel)]="ips.nit" style="width:100%" /></div>
  <ng-template pTemplate="footer">
    <p-button label="Cancelar" [text]="true" severity="secondary" (onClick)="dlgIps=false" />
    <p-button label="Crear" icon="pi pi-check" [loading]="guardando()" [disabled]="!ips.nombre || !ips.nit" (onClick)="crearIps()" />
  </ng-template>
</p-dialog>

<p-dialog [(visible)]="dlgLic" [header]="'Licencia — ' + (licSel?.nombre || '')" [modal]="true" [style]="{width:'440px'}">
  <div class="campo"><label>Cupo de usuarios *</label><input pInputText type="number" min="1" [(ngModel)]="lic.max_usuarios" style="width:100%" /></div>
  <div class="campo">
    <label>Vence (opcional)</label>
    <p-datepicker [(ngModel)]="lic.vence" dateFormat="yy-mm-dd" [showButtonBar]="true" placeholder="Sin vencimiento" [style]="{width:'100%'}" appendTo="body" />
    <small style="color:#64748b">Vacío = perpetua. Una licencia vencida bloquea crear usuarios, nunca la lectura.</small>
  </div>
  <ng-template pTemplate="footer">
    <p-button label="Cancelar" [text]="true" severity="secondary" (onClick)="dlgLic=false" />
    <p-button label="Fijar plan" icon="pi pi-check" [loading]="guardando()" [disabled]="!lic.max_usuarios || lic.max_usuarios < 1" (onClick)="fijarLic()" />
  </ng-template>
</p-dialog>

<p-dialog [(visible)]="dlgAdmin" [header]="'Nuevo administrador — ' + (adminSel?.nombre || '')" [modal]="true" [style]="{width:'460px'}">
  <div class="campo"><label>Nombre *</label><input pInputText [(ngModel)]="admin.nombre" style="width:100%" /></div>
  <div class="campo"><label>Correo *</label><input pInputText type="email" [(ngModel)]="admin.email" style="width:100%" /></div>
  <div class="campo"><label>Contraseña *</label><p-password [(ngModel)]="admin.clave" [feedback]="false" [toggleMask]="true" [style]="{width:'100%'}" inputStyleClass="w-full" /></div>
  <ng-template pTemplate="footer">
    <p-button label="Cancelar" [text]="true" severity="secondary" (onClick)="dlgAdmin=false" />
    <p-button label="Crear admin" icon="pi pi-check" [loading]="guardando()" [disabled]="!admin.nombre || !admin.email || !admin.clave" (onClick)="crearAdmin()" />
  </ng-template>
</p-dialog>
  `,
  styles: [`
    .fila-titulo { display:flex; justify-content:space-between; align-items:center; margin-bottom:1rem }
    .campo { margin-bottom:1rem }
    .campo label { display:block; font-size:.875rem; font-weight:500; color:#374151; margin-bottom:.3rem }
  `],
})
export class SuperadminComponent implements OnInit {
  private svc = inject(SuperadminService);
  private msg = inject(Avisos);

  instituciones = signal<Institucion[]>([]);
  usuarios = signal<UsuarioResumen[]>([]);
  cargando = signal(true);
  guardando = signal(false);

  private nombrePorId = computed(() => {
    const m = new Map<number, string>();
    for (const i of this.instituciones()) m.set(i.id, i.nombre);
    return m;
  });

  dlgIps = false;
  ips = { nombre: '', nit: '' };

  dlgLic = false;
  licSel: Institucion | null = null;
  lic: { max_usuarios: number | null; vence: Date | null } = { max_usuarios: 10, vence: null };

  dlgAdmin = false;
  adminSel: Institucion | null = null;
  admin = { nombre: '', email: '', clave: '' };

  ngOnInit(): void {
    this.cargar();
  }

  nombreInstitucion(id: number | null): string {
    return id ? (this.nombrePorId().get(id) ?? '—') : 'Sistema';
  }

  private cargar(): void {
    this.cargando.set(true);
    this.svc.listInstituciones().subscribe({
      next: (is) => { this.instituciones.set(is); this.cargando.set(false); },
      error: () => this.cargando.set(false),
    });
    this.svc.listUsuarios().subscribe({ next: (us) => this.usuarios.set(us), error: () => {} });
  }

  abrirIps(): void {
    this.ips = { nombre: '', nit: '' };
    this.dlgIps = true;
  }

  crearIps(): void {
    this.guardando.set(true);
    this.svc.crearInstitucion(this.ips.nombre.trim(), this.ips.nit.trim()).subscribe({
      next: () => {
        this.guardando.set(false);
        this.dlgIps = false;
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
    this.licSel = i;
    this.lic = { max_usuarios: 10, vence: null };
    this.dlgLic = true;
  }

  fijarLic(): void {
    if (!this.licSel || !this.lic.max_usuarios) return;
    this.guardando.set(true);
    const vence = this.lic.vence ? this.lic.vence.toISOString().slice(0, 10) : null;
    this.svc.fijarLicencia(this.licSel.id, Number(this.lic.max_usuarios), vence).subscribe({
      next: () => {
        this.guardando.set(false);
        this.dlgLic = false;
        this.msg.add({ severity: 'success', summary: 'Plan fijado' });
      },
      error: () => {
        this.guardando.set(false);
        this.msg.add({ severity: 'error', summary: 'Error', detail: 'No se pudo fijar el plan.' });
      },
    });
  }

  abrirAdmin(i: Institucion): void {
    this.adminSel = i;
    this.admin = { nombre: '', email: '', clave: '' };
    this.dlgAdmin = true;
  }

  crearAdmin(): void {
    if (!this.adminSel) return;
    this.guardando.set(true);
    this.svc.crearUsuario({
      nombre: this.admin.nombre.trim(),
      email: this.admin.email.trim(),
      clave: this.admin.clave,
      rol: 'admin',
      institucion_id: this.adminSel.id,
    }).subscribe({
      next: () => {
        this.guardando.set(false);
        this.dlgAdmin = false;
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
