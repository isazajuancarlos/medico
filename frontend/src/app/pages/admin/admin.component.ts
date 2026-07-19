import { Component, OnInit, inject, signal } from '@angular/core';
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
import { DividerModule } from 'primeng/divider';
import { ToggleSwitchModule } from 'primeng/toggleswitch';
import { MessageService } from 'primeng/api';
import {
  AdminService, Institucion, ConfiguracionInstitucion, PoliticaSeguridad,
  UsuarioAdmin, ConfigCampo, AuditoriaItem, Especialidad, EntidadEPS,
} from '../../services/admin.service';

type Tab = 'institucion' | 'configuracion' | 'usuarios' | 'seguridad' | 'campos' | 'auditoria' | 'catalogos';
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

@Component({
  selector: 'app-admin',
  standalone: true,
  providers: [MessageService],
  imports: [
    CommonModule, FormsModule,
    ButtonModule, InputTextModule, TextareaModule, TableModule,
    TagModule, ToastModule, SelectModule, TooltipModule, DialogModule,
    DividerModule, ToggleSwitchModule,
  ],
  template: `
    <p-toast />
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
              <div class="field-group">
                <label>Razón Social *</label>
                <input pInputText [(ngModel)]="institucion()!.razon_social" />
              </div>
              <div class="field-group">
                <label>Nombre Comercial *</label>
                <input pInputText [(ngModel)]="institucion()!.nombre_comercial" />
              </div>
              <div class="field-group">
                <label>NIT</label>
                <input pInputText [ngModel]="institucion()!.nit" disabled />
              </div>
              <div class="field-group">
                <label>Código Habilitación (REPS)</label>
                <input pInputText [(ngModel)]="institucion()!.codigo_habilitacion" />
              </div>
              <div class="field-group">
                <label>Código Prestador RIPS</label>
                <input pInputText [(ngModel)]="institucion()!.codigo_prestador" />
              </div>
              <div class="field-group">
                <label>Tipo Prestador</label>
                <p-select [(ngModel)]="institucion()!.tipo_prestador"
                  [options]="['IPS','CLINICA','HOSPITAL','CONSULTORIO','LABORATORIO']"
                  placeholder="Seleccionar" />
              </div>
            </div>
            <p-divider />
            <h3 class="section-title">Ubicación y Contacto</h3>
            <div class="form-grid">
              <div class="field-group" style="grid-column:span 2">
                <label>Dirección</label>
                <input pInputText [(ngModel)]="institucion()!.direccion" class="w-full" />
              </div>
              <div class="field-group">
                <label>Ciudad</label>
                <input pInputText [(ngModel)]="institucion()!.ciudad" />
              </div>
              <div class="field-group">
                <label>Departamento</label>
                <input pInputText [(ngModel)]="institucion()!.departamento" />
              </div>
              <div class="field-group">
                <label>Teléfono</label>
                <input pInputText [(ngModel)]="institucion()!.telefono" />
              </div>
              <div class="field-group">
                <label>Email institucional</label>
                <input pInputText type="email" [(ngModel)]="institucion()!.email" />
              </div>
            </div>
            <p-divider />
            <h3 class="section-title">Representante Legal</h3>
            <div class="form-grid">
              <div class="field-group">
                <label>Nombre</label>
                <input pInputText [(ngModel)]="institucion()!.representante_legal" />
              </div>
              <div class="field-group">
                <label>Identificación</label>
                <input pInputText [(ngModel)]="institucion()!.identificacion_representante" />
              </div>
            </div>
            <p-divider />
            <h3 class="section-title">Branding</h3>
            <div class="form-grid">
              <div class="field-group">
                <label>Color Primario</label>
                <div style="display:flex;gap:.5rem;align-items:center">
                  <input type="color" [(ngModel)]="institucion()!.color_primario"
                    style="height:38px;width:60px;border:1px solid #e2e8f0;border-radius:6px;cursor:pointer" />
                  <input pInputText [(ngModel)]="institucion()!.color_primario" style="flex:1" />
                </div>
              </div>
              <div class="field-group">
                <label>Color Secundario</label>
                <div style="display:flex;gap:.5rem;align-items:center">
                  <input type="color" [(ngModel)]="institucion()!.color_secundario"
                    style="height:38px;width:60px;border:1px solid #e2e8f0;border-radius:6px;cursor:pointer" />
                  <input pInputText [(ngModel)]="institucion()!.color_secundario" style="flex:1" />
                </div>
              </div>
            </div>
            <div class="form-actions">
              <p-button label="Guardar cambios" icon="pi pi-save" (onClick)="guardarInstitucion()" [loading]="guardando()" />
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
              <div class="field-group">
                <label>Hora apertura</label>
                <input pInputText type="time" [(ngModel)]="config()!.horario_apertura" />
              </div>
              <div class="field-group">
                <label>Hora cierre</label>
                <input pInputText type="time" [(ngModel)]="config()!.horario_cierre" />
              </div>
              <div class="field-group">
                <label>Duración cita (min)</label>
                <input pInputText type="number" [(ngModel)]="config()!.duracion_cita_default" min="5" max="120" />
              </div>
              <div class="field-group">
                <label>Buffer entre citas (min)</label>
                <input pInputText type="number" [(ngModel)]="config()!.tiempo_buffer" min="0" max="60" />
              </div>
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
              <div class="field-group">
                <label>Días máx. agendamiento</label>
                <input pInputText type="number" [(ngModel)]="config()!.dias_maximos_agendamiento" min="1" />
              </div>
            </div>
            <p-divider />
            <h3 class="section-title">Facturación</h3>
            <div class="form-grid">
              <div class="field-group">
                <label>Prefijo factura</label>
                <input pInputText [(ngModel)]="config()!.prefijo_factura" maxlength="10" />
              </div>
              <div class="field-group">
                <label>Resolución DIAN</label>
                <input pInputText [(ngModel)]="config()!.resolucion_facturacion" />
              </div>
              <div class="field-group">
                <label>Rango desde</label>
                <input pInputText type="number" [(ngModel)]="config()!.rango_desde" />
              </div>
              <div class="field-group">
                <label>Rango hasta</label>
                <input pInputText type="number" [(ngModel)]="config()!.rango_hasta" />
              </div>
            </div>
            <p-divider />
            <h3 class="section-title">Opciones del Sistema</h3>
            <div class="toggle-grid">
              <div class="toggle-item">
                <p-toggleswitch [(ngModel)]="config()!.permitir_citas_online" />
                <span>Permitir citas en línea</span>
              </div>
              <div class="toggle-item">
                <p-toggleswitch [(ngModel)]="config()!.requiere_autorizacion" />
                <span>Requerir autorización EPS</span>
              </div>
              <div class="toggle-item">
                <p-toggleswitch [(ngModel)]="config()!.enviar_recordatorios" />
                <span>Enviar recordatorios de citas</span>
              </div>
            </div>
            @if (config()!.enviar_recordatorios) {
              <div class="form-grid" style="margin-top:1rem">
                <div class="field-group">
                  <label>Horas anticipación recordatorio</label>
                  <input pInputText type="number" [(ngModel)]="config()!.horas_anticipacion" min="1" max="72" />
                </div>
                <div class="field-group">
                  <label>SMTP Host</label>
                  <input pInputText [(ngModel)]="config()!.smtp_host" />
                </div>
                <div class="field-group">
                  <label>SMTP Puerto</label>
                  <input pInputText type="number" [(ngModel)]="config()!.smtp_port" />
                </div>
                <div class="field-group">
                  <label>SMTP Usuario</label>
                  <input pInputText [(ngModel)]="config()!.smtp_usuario" />
                </div>
              </div>
            }
            <div class="form-actions">
              <p-button label="Guardar configuración" icon="pi pi-save" (onClick)="guardarConfig()" [loading]="guardando()" />
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
            <span class="p-input-icon-left">
              <i class="pi pi-search"></i>
              <input pInputText [(ngModel)]="busqUsuario" placeholder="Buscar..." (input)="cargarUsuarios()"
                style="padding-left:2rem" />
            </span>
            <p-select [(ngModel)]="filtroRolUsuario"
              [options]="[{value:'',label:'Todos los roles'},...ROLES]"
              optionLabel="label" optionValue="value"
              (onChange)="cargarUsuarios()" style="width:180px" />
          </div>
          <p-button label="Nuevo usuario" icon="pi pi-plus" (onClick)="abrirFormUsuario(null)" />
        </div>
        <p-table [value]="usuarios()" [loading]="cargando()">
          <ng-template #header>
            <tr>
              <th>Nombre</th>
              <th>Username</th>
              <th>Rol</th>
              <th>Especialidad</th>
              <th style="text-align:center">Estado</th>
              <th style="width:180px"></th>
            </tr>
          </ng-template>
          <ng-template #body let-u>
            <tr [class.row-inactivo]="!u.activo">
              <td>
                {{ u.nombre_completo }}
                @if (u.cuenta_bloqueada) {
                  <p-tag value="Bloqueado" severity="danger" style="margin-left:.5rem" />
                }
                @if (u.requiere_cambio_password) {
                  <p-tag value="Cambiar clave" severity="warn" style="margin-left:.25rem" />
                }
              </td>
              <td><code style="font-size:.82rem">{{ u.username }}</code></td>
              <td style="font-size:.85rem">{{ rolLabel(u.rol) }}</td>
              <td style="font-size:.82rem;color:#64748b">{{ u.especialidad_nombre || '—' }}</td>
              <td style="text-align:center">
                <p-tag [value]="u.activo ? 'Activo' : 'Inactivo'" [severity]="u.activo ? 'success' : 'secondary'" />
              </td>
              <td>
                <div style="display:flex;gap:.25rem;justify-content:flex-end">
                  <p-button icon="pi pi-pencil" [text]="true" size="small" pTooltip="Editar" (onClick)="abrirFormUsuario(u)" />
                  @if (u.cuenta_bloqueada) {
                    <p-button icon="pi pi-unlock" [text]="true" size="small" severity="warn" pTooltip="Desbloquear" (onClick)="desbloquear(u)" />
                  }
                  <p-button icon="pi pi-key" [text]="true" size="small" pTooltip="Reset contraseña" (onClick)="abrirResetPassword(u)" />
                  <p-button icon="pi pi-ban" [text]="true" size="small" severity="danger" pTooltip="Desactivar" (onClick)="desactivarUsuario(u)" />
                </div>
              </td>
            </tr>
          </ng-template>
          <ng-template #emptymessage>
            <tr><td colspan="6" style="text-align:center;padding:2rem;color:#94a3b8">Sin usuarios</td></tr>
          </ng-template>
        </p-table>
      }

      <!-- ══════════════════ SEGURIDAD ══════════════════ -->
      @if (tab() === 'seguridad') {
        @if (politica()) {
          <div class="form-section">
            <h3 class="section-title">Política de Contraseñas</h3>
            <div class="form-grid">
              <div class="field-group">
                <label>Longitud mínima</label>
                <input pInputText type="number" [(ngModel)]="politica()!.longitud_minima_password" min="6" max="32" />
              </div>
              <div class="field-group">
                <label>Días expiración (0 = nunca)</label>
                <input pInputText type="number" [(ngModel)]="politica()!.dias_expiracion_password" min="0" />
              </div>
              <div class="field-group">
                <label>Historial (no reutilizar últimas N)</label>
                <input pInputText type="number" [(ngModel)]="politica()!.historial_passwords" min="0" max="24" />
              </div>
            </div>
            <div class="toggle-grid" style="margin-top:.75rem">
              <div class="toggle-item">
                <p-toggleswitch [(ngModel)]="politica()!.requiere_mayuscula" />
                <span>Requiere mayúscula</span>
              </div>
              <div class="toggle-item">
                <p-toggleswitch [(ngModel)]="politica()!.requiere_minuscula" />
                <span>Requiere minúscula</span>
              </div>
              <div class="toggle-item">
                <p-toggleswitch [(ngModel)]="politica()!.requiere_numero" />
                <span>Requiere número</span>
              </div>
              <div class="toggle-item">
                <p-toggleswitch [(ngModel)]="politica()!.requiere_caracter_especial" />
                <span>Requiere carácter especial</span>
              </div>
            </div>
            <p-divider />
            <h3 class="section-title">Sesiones</h3>
            <div class="form-grid">
              <div class="field-group">
                <label>Timeout sesión (min)</label>
                <input pInputText type="number" [(ngModel)]="politica()!.timeout_sesion_minutos" min="5" />
              </div>
              <div class="field-group">
                <label>Sesiones simultáneas permitidas</label>
                <input pInputText type="number" [(ngModel)]="politica()!.sesiones_simultaneas_permitidas" min="1" />
              </div>
            </div>
            <div class="toggle-grid" style="margin-top:.75rem">
              <div class="toggle-item">
                <p-toggleswitch [(ngModel)]="politica()!.cerrar_otras_sesiones_al_login" />
                <span>Cerrar otras sesiones al iniciar</span>
              </div>
            </div>
            <p-divider />
            <h3 class="section-title">Intentos de Login y Bloqueo</h3>
            <div class="form-grid">
              <div class="field-group">
                <label>Máximo intentos fallidos</label>
                <input pInputText type="number" [(ngModel)]="politica()!.max_intentos_login" min="3" max="20" />
              </div>
              <div class="field-group">
                <label>Tiempo bloqueo (min)</label>
                <input pInputText type="number" [(ngModel)]="politica()!.tiempo_bloqueo_minutos" min="1" />
              </div>
            </div>
            <p-divider />
            <h3 class="section-title">Autenticación de Dos Factores (2FA)</h3>
            <div class="toggle-grid">
              <div class="toggle-item">
                <p-toggleswitch [(ngModel)]="politica()!.requiere_2fa" />
                <span>Requerir 2FA para todos los usuarios</span>
              </div>
              <div class="toggle-item">
                <p-toggleswitch [(ngModel)]="politica()!.requiere_2fa_admin" />
                <span>Requerir 2FA para administradores</span>
              </div>
            </div>
            @if (politica()!.requiere_2fa || politica()!.requiere_2fa_admin) {
              <div class="form-grid" style="margin-top:1rem">
                <div class="field-group">
                  <label>Método 2FA</label>
                  <p-select [(ngModel)]="politica()!.metodo_2fa"
                    [options]="[{v:'EMAIL',l:'Correo electrónico'},{v:'SMS',l:'SMS'},{v:'TOTP',l:'App autenticadora (TOTP)'}]"
                    optionLabel="l" optionValue="v" />
                </div>
              </div>
            }
            <p-divider />
            <h3 class="section-title">Auditoría y Notificaciones</h3>
            <div class="form-grid">
              <div class="field-group">
                <label>Retener logs (días)</label>
                <input pInputText type="number" [(ngModel)]="politica()!.retener_logs_dias" min="30" />
              </div>
            </div>
            <div class="toggle-grid" style="margin-top:.75rem">
              <div class="toggle-item">
                <p-toggleswitch [(ngModel)]="politica()!.registrar_todos_los_accesos" />
                <span>Registrar todos los accesos</span>
              </div>
              <div class="toggle-item">
                <p-toggleswitch [(ngModel)]="politica()!.notificar_login_nuevo_dispositivo" />
                <span>Notificar login desde nuevo dispositivo</span>
              </div>
              <div class="toggle-item">
                <p-toggleswitch [(ngModel)]="politica()!.notificar_cambio_password" />
                <span>Notificar cambio de contraseña</span>
              </div>
              <div class="toggle-item">
                <p-toggleswitch [(ngModel)]="politica()!.notificar_intentos_fallidos" />
                <span>Notificar intentos fallidos</span>
              </div>
            </div>
            <div class="form-actions">
              <p-button label="Guardar política" icon="pi pi-save" (onClick)="guardarPolitica()" [loading]="guardando()" />
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
          <p-button label="Nuevo campo" icon="pi pi-plus" (onClick)="abrirFormCampo(null)" />
        </div>
        <p-table [value]="campos()" [loading]="cargando()">
          <ng-template #header>
            <tr>
              <th style="width:60px">Orden</th>
              <th>Etiqueta</th>
              <th style="width:150px">Tipo</th>
              <th>Opciones</th>
              <th style="width:90px;text-align:center">Activo</th>
              <th style="width:100px"></th>
            </tr>
          </ng-template>
          <ng-template #body let-c>
            <tr [class.row-inactivo]="!c.activo">
              <td style="text-align:center;color:#94a3b8">{{ c.orden }}</td>
              <td><strong>{{ c.etiqueta }}</strong></td>
              <td><p-tag [value]="c.tipo" severity="info" /></td>
              <td style="font-size:.82rem;color:#64748b">{{ c.opciones || '—' }}</td>
              <td style="text-align:center">
                <p-toggleswitch [ngModel]="c.activo" (ngModelChange)="toggleCampo(c, $event)" />
              </td>
              <td>
                <div style="display:flex;gap:.25rem;justify-content:flex-end">
                  <p-button icon="pi pi-pencil" [text]="true" size="small" (onClick)="abrirFormCampo(c)" />
                  <p-button icon="pi pi-trash" [text]="true" size="small" severity="danger" (onClick)="eliminarCampo(c)" />
                </div>
              </td>
            </tr>
          </ng-template>
          <ng-template #emptymessage>
            <tr><td colspan="6" style="text-align:center;padding:2rem;color:#94a3b8">Sin campos configurados</td></tr>
          </ng-template>
        </p-table>
      }

      <!-- ══════════════════ AUDITORÍA ══════════════════ -->
      @if (tab() === 'auditoria') {
        <div class="list-header" style="margin-bottom:1rem;flex-wrap:wrap;gap:.5rem">
          <div style="display:flex;gap:.5rem;align-items:center;flex-wrap:wrap">
            <input type="date" [(ngModel)]="audFechaInicio" class="p-inputtext p-component" style="height:2.25rem" />
            <input type="date" [(ngModel)]="audFechaFin" class="p-inputtext p-component" style="height:2.25rem" />
            <input pInputText [(ngModel)]="audAccion" placeholder="Filtrar acción..." style="width:180px" />
            <p-button label="Buscar" icon="pi pi-search" size="small" (onClick)="cargarAuditoria()" />
          </div>
          <span style="font-size:.85rem;color:#64748b">{{ audTotal() }} registros</span>
        </div>
        <p-table [value]="auditoria()" [loading]="cargando()">
          <ng-template #header>
            <tr>
              <th style="width:155px">Fecha/Hora</th>
              <th>Usuario</th>
              <th>Acción</th>
              <th>Tabla</th>
              <th style="width:80px;text-align:center">Reg.</th>
              <th style="width:120px">IP</th>
              <th style="width:70px;text-align:center">OK</th>
            </tr>
          </ng-template>
          <ng-template #body let-a>
            <tr>
              <td style="font-size:.8rem;color:#64748b">{{ a.fecha_hora | date:'dd/MM/yy HH:mm' }}</td>
              <td style="font-size:.85rem">{{ a.usuario_nombre }}</td>
              <td><code style="font-size:.78rem;background:#f1f5f9;padding:.1rem .3rem;border-radius:3px">{{ a.accion }}</code></td>
              <td style="font-size:.82rem;color:#64748b">{{ a.tabla_afectada }}</td>
              <td style="text-align:center;font-size:.82rem">{{ a.registro_id }}</td>
              <td style="font-size:.78rem;color:#94a3b8">{{ a.ip_address || '—' }}</td>
              <td style="text-align:center">
                <p-tag [value]="a.exitoso ? 'OK' : 'Err'" [severity]="a.exitoso ? 'success' : 'danger'" />
              </td>
            </tr>
          </ng-template>
          <ng-template #emptymessage>
            <tr><td colspan="7" style="text-align:center;padding:2rem;color:#94a3b8">Sin registros</td></tr>
          </ng-template>
        </p-table>
        @if (audPages() > 1) {
          <div style="display:flex;justify-content:center;align-items:center;gap:.5rem;margin-top:1rem">
            <p-button [disabled]="audPage() === 1" icon="pi pi-angle-left" [text]="true" (onClick)="audPrev()" />
            <span style="font-size:.85rem">Pág {{ audPage() }} / {{ audPages() }}</span>
            <p-button [disabled]="audPage() >= audPages()" icon="pi pi-angle-right" [text]="true" (onClick)="audNext()" />
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
              <input pInputText [(ngModel)]="nuevaEspecialidad" placeholder="Nueva especialidad..."
                style="width:220px" (keyup.enter)="crearEspecialidad()" />
              <p-button label="Agregar" icon="pi pi-plus" size="small" (onClick)="crearEspecialidad()" />
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
            <span class="p-input-icon-left">
              <i class="pi pi-search"></i>
              <input pInputText [(ngModel)]="busqEntidad" placeholder="Buscar EPS..."
                (input)="cargarEntidades()" style="padding-left:2rem;width:260px" />
            </span>
            <p-button label="Nueva entidad" icon="pi pi-plus" (onClick)="abrirFormEntidad(null)" />
          </div>
          <p-table [value]="entidades()" [loading]="cargando()">
            <ng-template #header>
              <tr>
                <th>Nombre</th>
                <th style="width:140px">NIT</th>
                <th style="width:120px">Código EPS</th>
                <th style="width:90px"></th>
              </tr>
            </ng-template>
            <ng-template #body let-e>
              <tr>
                <td>{{ e.nombre }}</td>
                <td style="font-size:.85rem;color:#64748b">{{ e.nit }}</td>
                <td><code style="font-size:.82rem">{{ e.codigo_eps }}</code></td>
                <td>
                  <div style="display:flex;gap:.25rem;justify-content:flex-end">
                    <p-button icon="pi pi-pencil" [text]="true" size="small" (onClick)="abrirFormEntidad(e)" />
                    <p-button icon="pi pi-trash" [text]="true" size="small" severity="danger" (onClick)="eliminarEntidad(e)" />
                  </div>
                </td>
              </tr>
            </ng-template>
            <ng-template #emptymessage>
              <tr><td colspan="4" style="text-align:center;padding:2rem;color:#94a3b8">Sin entidades</td></tr>
            </ng-template>
          </p-table>
        }
      }

    </div>

    <!-- ══ DIALOG: Usuario ══ -->
    <p-dialog [(visible)]="mostrarFormUsuario"
      [header]="editUsuario ? 'Editar usuario' : 'Nuevo usuario'"
      [modal]="true" [style]="{width:'540px'}" [draggable]="false">
      @if (formUsuario) {
        <div class="form-grid dialog-form">
          <div class="field-group">
            <label>Nombre completo *</label>
            <input pInputText [(ngModel)]="formUsuario.nombre_completo" class="w-full" />
          </div>
          <div class="field-group">
            <label>Username *</label>
            <input pInputText [(ngModel)]="formUsuario.username" [disabled]="!!editUsuario" class="w-full" />
          </div>
          @if (!editUsuario) {
            <div class="field-group">
              <label>Contraseña *</label>
              <input pInputText type="password" [(ngModel)]="formUsuario.password" class="w-full" />
            </div>
          }
          <div class="field-group">
            <label>Email</label>
            <input pInputText type="email" [(ngModel)]="formUsuario.email" class="w-full" />
          </div>
          <div class="field-group">
            <label>Rol *</label>
            <p-select [(ngModel)]="formUsuario.rol" [options]="ROLES"
              optionLabel="label" optionValue="value" class="w-full" />
          </div>
          <div class="field-group">
            <label>Especialidad</label>
            <p-select [(ngModel)]="formUsuario.especialidad_id"
              [options]="[{id:null,nombre:'— Sin especialidad —'},...especialidades()]"
              optionLabel="nombre" optionValue="id" class="w-full" />
          </div>
          <div class="field-group">
            <label>Reg. Médico / Tarjeta Prof.</label>
            <input pInputText [(ngModel)]="formUsuario.registro_medico" class="w-full" />
          </div>
          <div class="field-group">
            <label>Registro RETHUS</label>
            <input pInputText [(ngModel)]="formUsuario.registro_rethus" class="w-full" />
          </div>
        </div>
      }
      <ng-template #footer>
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarFormUsuario = false" />
        <p-button label="Guardar" icon="pi pi-save" (onClick)="guardarUsuario()" [loading]="guardando()" />
      </ng-template>
    </p-dialog>

    <!-- ══ DIALOG: Reset Password ══ -->
    <p-dialog [(visible)]="mostrarResetPass" header="Restablecer contraseña"
      [modal]="true" [style]="{width:'380px'}" [draggable]="false">
      <p style="color:#64748b;margin-bottom:1rem">
        Usuario: <strong>{{ resetUsuario?.nombre_completo }}</strong>
      </p>
      <div class="field-group">
        <label>Nueva contraseña (mín. 6 caracteres)</label>
        <input pInputText type="password" [(ngModel)]="nuevaPassword" class="w-full" />
      </div>
      <ng-template #footer>
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarResetPass = false" />
        <p-button label="Restablecer" icon="pi pi-key" severity="warn"
          (onClick)="confirmarReset()" [loading]="guardando()" />
      </ng-template>
    </p-dialog>

    <!-- ══ DIALOG: Campo HC ══ -->
    <p-dialog [(visible)]="mostrarFormCampo"
      [header]="editCampo ? 'Editar campo' : 'Nuevo campo'"
      [modal]="true" [style]="{width:'420px'}" [draggable]="false">
      @if (formCampo) {
        <div class="form-grid dialog-form">
          <div class="field-group" style="grid-column:span 2">
            <label>Etiqueta *</label>
            <input pInputText [(ngModel)]="formCampo.etiqueta" class="w-full" />
          </div>
          <div class="field-group">
            <label>Tipo *</label>
            <p-select [(ngModel)]="formCampo.tipo" [options]="tiposCampo"
              optionLabel="label" optionValue="value" class="w-full" />
          </div>
          <div class="field-group">
            <label>Orden</label>
            <input pInputText type="number" [(ngModel)]="formCampo.orden" class="w-full" />
          </div>
          @if (formCampo.tipo === 'Lista Opciones') {
            <div class="field-group" style="grid-column:span 2">
              <label>Opciones (separadas por coma)</label>
              <input pInputText [(ngModel)]="formCampo.opciones" class="w-full"
                placeholder="Opción 1, Opción 2, Opción 3" />
            </div>
          }
        </div>
      }
      <ng-template #footer>
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarFormCampo = false" />
        <p-button label="Guardar" icon="pi pi-save" (onClick)="guardarCampo()" [loading]="guardando()" />
      </ng-template>
    </p-dialog>

    <!-- ══ DIALOG: Entidad ══ -->
    <p-dialog [(visible)]="mostrarFormEntidad"
      [header]="editEntidad ? 'Editar entidad' : 'Nueva entidad'"
      [modal]="true" [style]="{width:'420px'}" [draggable]="false">
      @if (formEntidad) {
        <div class="form-grid dialog-form">
          <div class="field-group" style="grid-column:span 2">
            <label>Nombre *</label>
            <input pInputText [(ngModel)]="formEntidad.nombre" class="w-full" />
          </div>
          <div class="field-group">
            <label>NIT *</label>
            <input pInputText [(ngModel)]="formEntidad.nit" [disabled]="!!editEntidad" class="w-full" />
          </div>
          <div class="field-group">
            <label>Código EPS *</label>
            <input pInputText [(ngModel)]="formEntidad.codigo_eps" class="w-full" />
          </div>
        </div>
      }
      <ng-template #footer>
        <p-button label="Cancelar" [text]="true" (onClick)="mostrarFormEntidad = false" />
        <p-button label="Guardar" icon="pi pi-save" (onClick)="guardarEntidad()" [loading]="guardando()" />
      </ng-template>
    </p-dialog>
  `,
  styles: [`
    .list-header { display:flex; justify-content:space-between; align-items:center; margin-bottom:1rem; flex-wrap:wrap; gap:.5rem }
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
  `],
})
export class AdminComponent implements OnInit {
  private svc = inject(AdminService);
  private msg = inject(MessageService);

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
  mostrarFormUsuario = false;
  editUsuario: UsuarioAdmin | null = null;
  formUsuario: any = null;
  mostrarResetPass = false;
  resetUsuario: UsuarioAdmin | null = null;
  nuevaPassword = '';

  mostrarFormCampo = false;
  editCampo: ConfigCampo | null = null;
  formCampo: any = null;

  audFechaInicio = '';
  audFechaFin = '';
  audAccion = '';

  nuevaEspecialidad = '';
  busqEntidad = '';
  mostrarFormEntidad = false;
  editEntidad: EntidadEPS | null = null;
  formEntidad: any = null;

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

  setTab(t: Tab): void { this.tab.set(t); }

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
    this.editUsuario = u;
    this.formUsuario = u
      ? { ...u, password: '' }
      : { username: '', password: '', nombre_completo: '', email: '', rol: 'medico', especialidad_id: null, registro_medico: '', registro_rethus: '' };
    this.mostrarFormUsuario = true;
  }
  guardarUsuario() {
    if (!this.formUsuario?.nombre_completo || !this.formUsuario.rol) { this.err('Complete nombre y rol'); return; }
    this.guardando.set(true);
    const obs = this.editUsuario
      ? this.svc.updateUsuario(this.editUsuario.id, this.formUsuario)
      : this.svc.createUsuario(this.formUsuario);
    obs.subscribe({
      next: () => { this.guardando.set(false); this.mostrarFormUsuario = false; this.ok(this.editUsuario ? 'Usuario actualizado' : 'Usuario creado'); this.cargarUsuarios(); },
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
    this.resetUsuario = u; this.nuevaPassword = ''; this.mostrarResetPass = true;
  }
  confirmarReset() {
    if (!this.nuevaPassword || this.nuevaPassword.length < 6) { this.err('Mínimo 6 caracteres'); return; }
    this.guardando.set(true);
    this.svc.resetPassword(this.resetUsuario!.id, this.nuevaPassword).subscribe({
      next: (r: any) => { this.guardando.set(false); this.mostrarResetPass = false; this.ok(r.mensaje); },
      error: e => { this.guardando.set(false); this.err(e?.error?.detail ?? 'Error'); },
    });
  }

  // ── Campos HC ─────────────────────────────────────────────────────────────
  private cargarCampos() {
    this.svc.listCampos().subscribe({ next: r => this.campos.set(r), error: () => {} });
  }
  abrirFormCampo(c: ConfigCampo | null) {
    this.editCampo = c;
    this.formCampo = c ? { ...c } : { etiqueta: '', tipo: 'Texto', opciones: '', orden: 0 };
    this.mostrarFormCampo = true;
  }
  guardarCampo() {
    if (!this.formCampo?.etiqueta) { this.err('Ingrese la etiqueta'); return; }
    this.guardando.set(true);
    const obs = this.editCampo
      ? this.svc.updateCampo(this.editCampo.id, this.formCampo)
      : this.svc.createCampo(this.formCampo);
    obs.subscribe({
      next: () => { this.guardando.set(false); this.mostrarFormCampo = false; this.ok('Campo guardado'); this.cargarCampos(); },
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
    this.editEntidad = e;
    this.formEntidad = e ? { ...e } : { nit: '', nombre: '', codigo_eps: '' };
    this.mostrarFormEntidad = true;
  }
  guardarEntidad() {
    if (!this.formEntidad?.nombre || !this.formEntidad.nit || !this.formEntidad.codigo_eps) {
      this.err('Complete todos los campos'); return;
    }
    this.guardando.set(true);
    const obs = this.editEntidad
      ? this.svc.updateEntidad(this.editEntidad.id, this.formEntidad)
      : this.svc.createEntidad(this.formEntidad);
    obs.subscribe({
      next: () => { this.guardando.set(false); this.mostrarFormEntidad = false; this.ok('Guardado'); this.cargarEntidades(); },
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
