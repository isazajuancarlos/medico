import { Component, OnInit, inject, signal, computed } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ActivatedRoute } from '@angular/router';
import { ButtonModule } from 'primeng/button';
import { InputTextModule } from 'primeng/inputtext';
import { TextareaModule } from 'primeng/textarea';
import { InputNumberModule } from 'primeng/inputnumber';
import { CheckboxModule } from 'primeng/checkbox';
import { StepperModule } from 'primeng/stepper';
import { CardModule } from 'primeng/card';
import { DividerModule } from 'primeng/divider';
import { TagModule } from 'primeng/tag';
import { ToastModule } from 'primeng/toast';
import { PanelModule } from 'primeng/panel';
import { TableModule } from 'primeng/table';
import { ConfirmDialogModule } from 'primeng/confirmdialog';
import { InputGroupModule } from 'primeng/inputgroup';
import { InputGroupAddonModule } from 'primeng/inputgroupaddon';
import { TabsModule } from 'primeng/tabs';
import { SelectModule } from 'primeng/select';
import { TooltipModule } from 'primeng/tooltip';
import { HistoriaService, Historia, PacienteInfo, MedicoInfo, Formula, FormulaCreate, MedicamentoInfo, Orden, OrdenCreate, ImagenDiagnostica, EspecialidadInfo,
         UnidadDispensacion, Finalidad, CausaMotivo } from '../../services/historia.service';
import { PacienteService } from '../../services/paciente.service';
import { Patient } from '../../models/patient.models';
import { MessageService, ConfirmationService } from 'primeng/api';
import { Subject, Observable } from 'rxjs';
import { debounceTime, distinctUntilChanged, switchMap, catchError } from 'rxjs/operators';
import { of } from 'rxjs';

type Vista = 'busqueda' | 'lista' | 'formulario';

interface AntFamiliares {
  hta: boolean; diabetes: boolean; coronarias: boolean; cancer: boolean;
  acv: boolean; tbc: boolean; asma: boolean; otros: string;
}

interface AntPersonales {
  hta: boolean; diabetes: boolean; coronarias: boolean; acv: boolean; tbc: boolean; asma: boolean;
  cancer_activo: boolean; cancer_desc: string;
  quirurgicos_activo: boolean; quirurgicos_desc: string;
  its_activo: boolean; its_desc: string;
  otros: string; habitos: string;
}

interface AntGineco {
  menarquia: number | null; ciclos_regulares: boolean; dismenorrea: boolean; metrorragia: boolean;
  planificacion: boolean; metodo_planificacion: string;
  citologia_vigente: boolean; fecha_citologia: string;
  lactancia_actual: boolean;
  embarazo_actual: boolean; semanas_gestacion: number | null;
  gestas: number | null; partos: number | null; cesareas: number | null;
  abortos: number | null; hijos_vivos: number | null; fum: string;
  obs_adicionales: string;
}

interface AntPrenatales {
  embarazo_a_termino: boolean; semanas_gestacion_nacer: number | null;
  peso_nacer: number | null; talla_nacer: number | null;
  apgar_1min: number | null; apgar_5min: number | null;
  parto_normal: boolean; parto_cesarea: boolean; parto_forceps: boolean;
  lactancia_materna: boolean; complicaciones: string;
}

function defaultAntFam(): AntFamiliares {
  return { hta: false, diabetes: false, coronarias: false, cancer: false, acv: false, tbc: false, asma: false, otros: '' };
}
function defaultAntPers(): AntPersonales {
  return {
    hta: false, diabetes: false, coronarias: false, acv: false, tbc: false, asma: false,
    cancer_activo: false, cancer_desc: '', quirurgicos_activo: false, quirurgicos_desc: '',
    its_activo: false, its_desc: '', otros: '', habitos: '',
  };
}
function defaultAntGineco(): AntGineco {
  return {
    menarquia: null, ciclos_regulares: false, dismenorrea: false, metrorragia: false,
    planificacion: false, metodo_planificacion: '', citologia_vigente: false, fecha_citologia: '',
    lactancia_actual: false, embarazo_actual: false, semanas_gestacion: null,
    gestas: null, partos: null, cesareas: null, abortos: null, hijos_vivos: null, fum: '',
    obs_adicionales: '',
  };
}
function defaultAntPrenatales(): AntPrenatales {
  return {
    embarazo_a_termino: false, semanas_gestacion_nacer: null, peso_nacer: null, talla_nacer: null,
    apgar_1min: null, apgar_5min: null, parto_normal: false, parto_cesarea: false, parto_forceps: false,
    lactancia_materna: false, complicaciones: '',
  };
}

@Component({
  selector: 'app-historias',
  standalone: true,
  providers: [MessageService, ConfirmationService],
  imports: [
    CommonModule, FormsModule,
    ButtonModule, InputTextModule, TextareaModule, InputNumberModule,
    CheckboxModule, StepperModule, CardModule, DividerModule, TagModule,
    ToastModule, PanelModule, TableModule, ConfirmDialogModule,
    InputGroupModule, InputGroupAddonModule, TabsModule,
    SelectModule, TooltipModule,
  ],
  template: `
    <div class="page-card">

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA 1: BÚSQUEDA DE PACIENTE             -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'busqueda') {
        <h2 class="page-title">Historias Clínicas</h2>
        <p class="search-hint">Busque al paciente por nombre o número de documento para ver o crear su historia clínica.</p>

        <div class="search-box">
          <p-inputgroup>
            <p-inputgroup-addon><i class="pi pi-search"></i></p-inputgroup-addon>
            <input pInputText
              [(ngModel)]="terminoBusqueda"
              placeholder="Nombre o documento del paciente..."
              (keydown.enter)="buscarPacientes()"
              class="search-input" />
            <p-button label="Buscar" [loading]="buscando()" (onClick)="buscarPacientes()" />
          </p-inputgroup>
        </div>

        @if (resultadosBusqueda().length > 0) {
          <div class="resultados">
            <p class="resultados-titulo">{{ resultadosBusqueda().length }} paciente(s) encontrado(s)</p>
            @for (p of resultadosBusqueda(); track p.id) {
              <div class="resultado-item" (click)="seleccionarPaciente(p)">
                <div class="resultado-nombre">{{ nombreCompleto(p) }}</div>
                <div class="resultado-detalle">
                  <span>{{ p.tipo_id }}: {{ p.identificacion }}</span>
                  <span class="sep">·</span>
                  <span>{{ p.sexo === 'M' ? 'Masculino' : 'Femenino' }}</span>
                  @if (p.edad) { <span class="sep">·</span><span>{{ p.edad }} años</span> }
                </div>
              </div>
            }
          </div>
        }

        @if (sinResultados()) {
          <div class="sin-resultados">
            <i class="pi pi-inbox" style="font-size:2rem;opacity:0.4"></i>
            <p>No se encontraron pacientes con ese criterio de búsqueda.</p>
          </div>
        }
      }

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA 2: LISTA DE HISTORIAS DEL PACIENTE  -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'lista') {
        <!-- Cabecera paciente -->
        @if (paciente()) {
          <div class="info-header">
            <div class="info-block paciente-block">
              <div class="info-label">Paciente</div>
              <div class="info-name">{{ paciente()!.nombre_completo }}</div>
              <div class="info-row">
                <span>{{ paciente()!.tipo_id }}: <strong>{{ paciente()!.identificacion }}</strong></span>
                <span class="sep">|</span>
                <span>{{ paciente()!.sexo === 'M' ? 'Masculino' : 'Femenino' }}</span>
                <span class="sep">|</span>
                <span>{{ paciente()!.edad }} años</span>
                <span class="sep">|</span>
                <span>{{ paciente()!.fecha_nacimiento | date:'dd/MM/yyyy' }}</span>
              </div>
              @if (paciente()!.entidad_nombre) {
                <div class="info-row">EPS: <strong>{{ paciente()!.entidad_nombre }}</strong></div>
              }
            </div>
          </div>
        }

        <div class="lista-header">
          <h2 class="page-title" style="margin-bottom:0">Historias Clínicas</h2>
          <div style="display:flex;gap:0.5rem">
            <p-button label="Nueva Historia" icon="pi pi-plus" (onClick)="nuevaHistoria()" />
            <p-button label="Volver" icon="pi pi-arrow-left" [text]="true" (onClick)="volverBusqueda()" />
          </div>
        </div>

        @if (cargandoHistorias()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando historias...</div>
        } @else if (historiasPaciente().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-file-o" style="font-size:2rem;opacity:0.4"></i>
            <p>Este paciente no tiene historias clínicas registradas.</p>
            <p-button label="Crear Primera Historia" icon="pi pi-plus" (onClick)="nuevaHistoria()" />
          </div>
        } @else {
          <p-table [value]="historiasPaciente()" [rowHover]="true" styleClass="p-datatable-sm">
            <ng-template pTemplate="header">
              <tr>
                <th>Código</th>
                <th>Fecha</th>
                <th>Motivo de Consulta</th>
                <th>Diagnóstico</th>
                <th>Médico</th>
                <th style="text-align:center">Estado</th>
                <th style="text-align:center">Acciones</th>
              </tr>
            </ng-template>
            <ng-template pTemplate="body" let-h>
              <tr>
                <td><strong>{{ h.codigo_historia }}</strong></td>
                <td>{{ h.fecha | date:'dd/MM/yyyy' }}</td>
                <td class="col-truncate">{{ h.motivo_consulta }}</td>
                <td class="col-truncate">{{ h.diagnostico_principal }}</td>
                <td>{{ h.medico?.nombre_completo }}</td>
                <td style="text-align:center">
                  @if (h.firmada) {
                    <p-tag value="Firmada" severity="success" icon="pi pi-lock" />
                  } @else {
                    <p-tag value="Borrador" severity="warn" icon="pi pi-pencil" />
                  }
                </td>
                <td style="text-align:center">
                  <div style="display:flex;gap:0.4rem;justify-content:center">
                    <p-button icon="pi pi-eye" [text]="true" size="small"
                      pTooltip="Ver / Editar" (onClick)="editarHistoria(h)" />
                    @if (!h.firmada) {
                      <p-button icon="pi pi-trash" [text]="true" size="small" severity="danger"
                        pTooltip="Eliminar" (onClick)="confirmarEliminar(h)" />
                    }
                  </div>
                </td>
              </tr>
            </ng-template>
          </p-table>
        }
      }

      <!-- ═══════════════════════════════════════════ -->
      <!--  VISTA 3: FORMULARIO DE HISTORIA           -->
      <!-- ═══════════════════════════════════════════ -->
      @if (vista() === 'formulario') {

        <!-- Cabecera paciente + médico -->
        @if (paciente()) {
          <div class="info-header">
            <div class="info-block paciente-block">
              <div class="info-label">Paciente</div>
              <div class="info-name">{{ paciente()!.nombre_completo }}</div>
              <div class="info-row">
                <span>{{ paciente()!.tipo_id }}: <strong>{{ paciente()!.identificacion }}</strong></span>
                <span class="sep">|</span>
                <span>{{ paciente()!.sexo === 'M' ? 'Masculino' : 'Femenino' }}</span>
                <span class="sep">|</span>
                <span>{{ paciente()!.edad }} años</span>
                <span class="sep">|</span>
                <span>{{ paciente()!.fecha_nacimiento | date:'dd/MM/yyyy' }}</span>
              </div>
              @if (paciente()!.entidad_nombre) {
                <div class="info-row">EPS: <strong>{{ paciente()!.entidad_nombre }}</strong></div>
              }
              @if (paciente()!.celular) {
                <div class="info-row">Tel: {{ paciente()!.celular }}</div>
              }
            </div>
            @if (medico()) {
              <div class="info-block medico-block">
                <div class="info-label">Médico tratante</div>
                <div class="info-name">{{ medico()!.nombre_completo }}</div>
                @if (medico()!.especialidad) {
                  <div class="info-row">{{ medico()!.especialidad }}</div>
                }
                @if (medico()!.registro_medico) {
                  <div class="info-row">Reg. médico: {{ medico()!.registro_medico }}</div>
                }
              </div>
            }
            @if (historia()?.codigo_historia) {
              <div class="info-block codigo-block">
                <div class="info-label">Historia</div>
                <div class="info-name">{{ historia()!.codigo_historia }}</div>
                @if (historia()?.fecha) {
                  <div class="info-row">{{ historia()!.fecha | date:'dd/MM/yyyy HH:mm' }}</div>
                }
              </div>
            }
          </div>
        }

        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:1.5rem">
          <div style="display:flex;align-items:center;gap:0.75rem">
            <p-button icon="pi pi-arrow-left" [text]="true" (onClick)="volverLista()" pTooltip="Volver a la lista" />
            <h2 class="page-title" style="margin-bottom:0">
              {{ historia()?.id ? 'Editar Historia' : 'Nueva Historia' }}
            </h2>
          </div>
          @if (historia()?.id) {
            <p-button label="Firmar Historia" icon="pi pi-lock"
              [loading]="signing()" [disabled]="!!historia()?.firmada"
              (onClick)="firmarHistoria()" />
          }
        </div>

        <p-stepper [value]="activeStep()">
          <p-step-list>
            <p-step [value]="1">Antecedentes</p-step>
            <p-step [value]="2">Examen Físico</p-step>
            <p-step [value]="3">Diagnóstico</p-step>
            <p-step [value]="4">Plan</p-step>
            <p-step [value]="5">Fórmula Médica</p-step>
            <p-step [value]="6">Órdenes</p-step>
            <p-step [value]="7">Imágenes</p-step>
          </p-step-list>

          <p-step-panels>

            <!-- PASO 1: ANTECEDENTES -->
            <p-step-panel [value]="1">
              <ng-template pTemplate="content" let-activateCallback="activateCallback">

                <p-panel header="Antecedentes Familiares" [toggleable]="true" styleClass="ant-panel">
                  <div class="check-grid">
                    <div class="chk"><p-checkbox [(ngModel)]="antFam.hta" [binary]="true" inputId="ff_hta" /><label for="ff_hta">HTA</label></div>
                    <div class="chk"><p-checkbox [(ngModel)]="antFam.diabetes" [binary]="true" inputId="ff_dia" /><label for="ff_dia">Diabetes</label></div>
                    <div class="chk"><p-checkbox [(ngModel)]="antFam.coronarias" [binary]="true" inputId="ff_cor" /><label for="ff_cor">Coronarias</label></div>
                    <div class="chk"><p-checkbox [(ngModel)]="antFam.cancer" [binary]="true" inputId="ff_can" /><label for="ff_can">Cáncer</label></div>
                    <div class="chk"><p-checkbox [(ngModel)]="antFam.acv" [binary]="true" inputId="ff_acv" /><label for="ff_acv">ACV</label></div>
                    <div class="chk"><p-checkbox [(ngModel)]="antFam.tbc" [binary]="true" inputId="ff_tbc" /><label for="ff_tbc">TBC</label></div>
                    <div class="chk"><p-checkbox [(ngModel)]="antFam.asma" [binary]="true" inputId="ff_asm" /><label for="ff_asm">Asma</label></div>
                  </div>
                  <div class="field" style="margin-top:0.75rem">
                    <label>Otros</label>
                    <input pInputText [(ngModel)]="antFam.otros" class="w-full" placeholder="Describa otros antecedentes familiares..." />
                  </div>
                </p-panel>

                <p-panel header="Antecedentes Personales" [toggleable]="true" styleClass="ant-panel">
                  <div class="check-grid">
                    <div class="chk"><p-checkbox [(ngModel)]="antPers.hta" [binary]="true" inputId="fp_hta" /><label for="fp_hta">HTA</label></div>
                    <div class="chk"><p-checkbox [(ngModel)]="antPers.diabetes" [binary]="true" inputId="fp_dia" /><label for="fp_dia">Diabetes</label></div>
                    <div class="chk"><p-checkbox [(ngModel)]="antPers.coronarias" [binary]="true" inputId="fp_cor" /><label for="fp_cor">Coronarias</label></div>
                    <div class="chk"><p-checkbox [(ngModel)]="antPers.acv" [binary]="true" inputId="fp_acv" /><label for="fp_acv">ACV</label></div>
                    <div class="chk"><p-checkbox [(ngModel)]="antPers.tbc" [binary]="true" inputId="fp_tbc" /><label for="fp_tbc">TBC</label></div>
                    <div class="chk"><p-checkbox [(ngModel)]="antPers.asma" [binary]="true" inputId="fp_asm" /><label for="fp_asm">Asma</label></div>
                  </div>
                  <p-divider />
                  <div class="cond-row">
                    <div class="chk"><p-checkbox [(ngModel)]="antPers.cancer_activo" [binary]="true" inputId="fp_can" /><label for="fp_can">Cáncer</label></div>
                    @if (antPers.cancer_activo) {
                      <input pInputText [(ngModel)]="antPers.cancer_desc" class="cond-input" placeholder="Tipo / descripción..." />
                    }
                  </div>
                  <div class="cond-row">
                    <div class="chk"><p-checkbox [(ngModel)]="antPers.quirurgicos_activo" [binary]="true" inputId="fp_qui" /><label for="fp_qui">Quirúrgicos</label></div>
                    @if (antPers.quirurgicos_activo) {
                      <input pInputText [(ngModel)]="antPers.quirurgicos_desc" class="cond-input" placeholder="Cirugías / descripción..." />
                    }
                  </div>
                  <div class="cond-row">
                    <div class="chk"><p-checkbox [(ngModel)]="antPers.its_activo" [binary]="true" inputId="fp_its" /><label for="fp_its">ITS</label></div>
                    @if (antPers.its_activo) {
                      <input pInputText [(ngModel)]="antPers.its_desc" class="cond-input" placeholder="Descripción..." />
                    }
                  </div>
                  <p-divider />
                  <div class="form-grid">
                    <div class="field">
                      <label>Otros antecedentes</label>
                      <input pInputText [(ngModel)]="antPers.otros" class="w-full" placeholder="Alergias, farmacológicos, tóxicos..." />
                    </div>
                    <div class="field">
                      <label>Hábitos</label>
                      <input pInputText [(ngModel)]="antPers.habitos" class="w-full" placeholder="Tabaco, alcohol, ejercicio..." />
                    </div>
                  </div>
                </p-panel>

                @if (esMujer()) {
                  <p-panel header="Antecedentes Ginecobstétricos" [toggleable]="true" styleClass="ant-panel">
                    <div class="form-grid">
                      <div class="field">
                        <label>Menarquia (edad)</label>
                        <p-inputNumber [(ngModel)]="antGineco.menarquia" [min]="0" [max]="25" class="w-full" />
                      </div>
                      <div class="field">
                        <label>FUM</label>
                        <input pInputText type="date" [(ngModel)]="antGineco.fum" class="w-full" />
                      </div>
                    </div>
                    <div class="check-grid" style="margin-bottom:0.75rem">
                      <div class="chk"><p-checkbox [(ngModel)]="antGineco.ciclos_regulares" [binary]="true" inputId="fg_cic" /><label for="fg_cic">Ciclos regulares</label></div>
                      <div class="chk"><p-checkbox [(ngModel)]="antGineco.dismenorrea" [binary]="true" inputId="fg_dis" /><label for="fg_dis">Dismenorrea</label></div>
                      <div class="chk"><p-checkbox [(ngModel)]="antGineco.metrorragia" [binary]="true" inputId="fg_met" /><label for="fg_met">Metrorragia</label></div>
                      <div class="chk"><p-checkbox [(ngModel)]="antGineco.citologia_vigente" [binary]="true" inputId="fg_cit" /><label for="fg_cit">Citología vigente</label></div>
                      <div class="chk"><p-checkbox [(ngModel)]="antGineco.lactancia_actual" [binary]="true" inputId="fg_lac" /><label for="fg_lac">Lactancia actual</label></div>
                    </div>
                    <div class="cond-row">
                      <div class="chk"><p-checkbox [(ngModel)]="antGineco.planificacion" [binary]="true" inputId="fg_pla" /><label for="fg_pla">Planificación familiar</label></div>
                      @if (antGineco.planificacion) {
                        <input pInputText [(ngModel)]="antGineco.metodo_planificacion" class="cond-input" placeholder="Método..." />
                      }
                    </div>
                    @if (antGineco.citologia_vigente) {
                      <div class="field" style="margin-top:0.5rem">
                        <label>Fecha citología</label>
                        <input pInputText type="date" [(ngModel)]="antGineco.fecha_citologia" style="width:200px" />
                      </div>
                    }
                    <div class="cond-row" style="margin-top:0.5rem">
                      <div class="chk"><p-checkbox [(ngModel)]="antGineco.embarazo_actual" [binary]="true" inputId="fg_emb" /><label for="fg_emb">Embarazo actual</label></div>
                      @if (antGineco.embarazo_actual) {
                        <p-inputNumber [(ngModel)]="antGineco.semanas_gestacion" [min]="1" [max]="42"
                          placeholder="Semanas" class="cond-input" />
                      }
                    </div>
                    <p-divider />
                    <div class="obstetrico-grid">
                      <div class="field"><label>G (Gestas)</label><p-inputNumber [(ngModel)]="antGineco.gestas" [min]="0" class="w-full" /></div>
                      <div class="field"><label>P (Partos)</label><p-inputNumber [(ngModel)]="antGineco.partos" [min]="0" class="w-full" /></div>
                      <div class="field"><label>C (Cesáreas)</label><p-inputNumber [(ngModel)]="antGineco.cesareas" [min]="0" class="w-full" /></div>
                      <div class="field"><label>A (Abortos)</label><p-inputNumber [(ngModel)]="antGineco.abortos" [min]="0" class="w-full" /></div>
                      <div class="field"><label>Hijos vivos</label><p-inputNumber [(ngModel)]="antGineco.hijos_vivos" [min]="0" class="w-full" /></div>
                    </div>
                    <div class="field" style="margin-top:0.75rem">
                      <label>Observaciones</label>
                      <textarea pTextarea [(ngModel)]="antGineco.obs_adicionales" rows="2" class="w-full"></textarea>
                    </div>
                  </p-panel>

                  <p-panel header="Antecedentes Prenatales" [toggleable]="true" styleClass="ant-panel">
                    <div class="check-grid" style="margin-bottom:0.75rem">
                      <div class="chk"><p-checkbox [(ngModel)]="antPrenatales.embarazo_a_termino" [binary]="true" inputId="fp_ter" /><label for="fp_ter">Embarazo a término</label></div>
                      <div class="chk"><p-checkbox [(ngModel)]="antPrenatales.parto_normal" [binary]="true" inputId="fp_pno" /><label for="fp_pno">Parto normal</label></div>
                      <div class="chk"><p-checkbox [(ngModel)]="antPrenatales.parto_cesarea" [binary]="true" inputId="fp_pce" /><label for="fp_pce">Cesárea</label></div>
                      <div class="chk"><p-checkbox [(ngModel)]="antPrenatales.parto_forceps" [binary]="true" inputId="fp_pfo" /><label for="fp_pfo">Fórceps</label></div>
                      <div class="chk"><p-checkbox [(ngModel)]="antPrenatales.lactancia_materna" [binary]="true" inputId="fp_lma" /><label for="fp_lma">Lactancia materna</label></div>
                    </div>
                    <div class="form-grid">
                      <div class="field">
                        <label>Semanas de gestación al nacer</label>
                        <p-inputNumber [(ngModel)]="antPrenatales.semanas_gestacion_nacer" [min]="20" [max]="44" class="w-full" />
                      </div>
                      <div class="field">
                        <label>Peso al nacer (kg)</label>
                        <p-inputNumber [(ngModel)]="antPrenatales.peso_nacer" [minFractionDigits]="1" [maxFractionDigits]="3" class="w-full" />
                      </div>
                      <div class="field">
                        <label>Talla al nacer (cm)</label>
                        <p-inputNumber [(ngModel)]="antPrenatales.talla_nacer" [minFractionDigits]="1" class="w-full" />
                      </div>
                      <div class="field">
                        <label>APGAR al minuto</label>
                        <p-inputNumber [(ngModel)]="antPrenatales.apgar_1min" [min]="0" [max]="10" class="w-full" />
                      </div>
                      <div class="field">
                        <label>APGAR 5 minutos</label>
                        <p-inputNumber [(ngModel)]="antPrenatales.apgar_5min" [min]="0" [max]="10" class="w-full" />
                      </div>
                    </div>
                    <div class="field" style="margin-top:0.5rem">
                      <label>Complicaciones perinatales</label>
                      <textarea pTextarea [(ngModel)]="antPrenatales.complicaciones" rows="2" class="w-full" placeholder="Describa si aplica..."></textarea>
                    </div>
                  </p-panel>
                }

                <div class="step-actions">
                  <p-button label="Siguiente" icon="pi pi-chevron-right" iconPos="right" (onClick)="activateCallback(2)" />
                </div>
              </ng-template>
            </p-step-panel>

            <!-- PASO 2: EXAMEN FÍSICO (Signos Vitales + Examen por Sistemas) -->
            <p-step-panel [value]="2">
              <ng-template pTemplate="content" let-activateCallback="activateCallback">
                <p-tabs value="sv">
                  <p-tablist>
                    <p-tab value="sv">Signos Vitales</p-tab>
                    <p-tab value="ef">Examen por Sistemas</p-tab>
                  </p-tablist>
                  <p-tabpanels>

                    <!-- TAB 1: SIGNOS VITALES -->
                    <p-tabpanel value="sv">
                      <div class="sv-grid">

                        <div class="field">
                          <label>Perímetro abdominal (cm)</label>
                          <p-inputNumber [(ngModel)]="form.pabd" [minFractionDigits]="1" class="w-full" placeholder="pabd" />
                        </div>
                        <div class="field">
                          <label>Presión arterial media</label>
                          <input pInputText [(ngModel)]="form.pam" class="w-full" placeholder="pam (mmHg)" />
                        </div>
                        <div class="field">
                          <label>Temperatura corporal (°C)</label>
                          <p-inputNumber [(ngModel)]="form.temperatura" [minFractionDigits]="1" class="w-full" placeholder="tc" />
                        </div>
                        <div class="field">
                          <label>Pulso / min</label>
                          <p-inputNumber [(ngModel)]="form.pulso" [min]="0" class="w-full" placeholder="p/m" />
                        </div>
                        <div class="field">
                          <label>Frec. respiratoria</label>
                          <p-inputNumber [(ngModel)]="form.frecuencia_respiratoria" [min]="0" class="w-full" placeholder="fr" />
                        </div>
                        <div class="field">
                          <label>Frec. cardíaca / min</label>
                          <p-inputNumber [(ngModel)]="form.frecuencia_cardiaca" [min]="0" class="w-full" placeholder="fc/m" />
                        </div>
                        <div class="field">
                          <label>Perímetro cefálico (cm)</label>
                          <p-inputNumber [(ngModel)]="form.pc" [minFractionDigits]="1" class="w-full" placeholder="pc" />
                        </div>
                        <div class="field">
                          <label>Peso (kg)</label>
                          <p-inputNumber [(ngModel)]="form.peso" [minFractionDigits]="1" [maxFractionDigits]="2"
                            class="w-full" placeholder="peso/k" (ngModelChange)="calcularMetrics()" />
                        </div>
                        <div class="field">
                          <label>Talla (m)</label>
                          <p-inputNumber [(ngModel)]="tallaMts" [minFractionDigits]="2" [maxFractionDigits]="2"
                            [min]="0" [max]="3" class="w-full" placeholder="talla/m"
                            (ngModelChange)="onTallaChange($event)" />
                        </div>

                        <!-- CALCULADOS -->
                        <div class="field sv-calc">
                          <label>Peso ideal (kg) <span class="calc-badge">calculo</span></label>
                          <div class="calc-value">{{ form.peso_ideal != null ? (form.peso_ideal | number:'1.1-1') + ' kg' : '—' }}</div>
                        </div>
                        <div class="field sv-calc">
                          <label>Talla ideal (m) <span class="calc-badge">calculo</span></label>
                          <div class="calc-value">{{ tallaIdeal() != null ? (tallaIdeal()! | number:'1.2-2') + ' m' : '—' }}</div>
                        </div>
                        <div class="field sv-calc">
                          <label>IMC <span class="calc-badge">calculo</span></label>
                          <div class="calc-value">
                            @if (form.imc) {
                              {{ form.imc | number:'1.1-1' }}
                              <p-tag [value]="imcLabel()" [severity]="imcSeverity()" style="margin-left:0.5rem" />
                            } @else { — }
                          </div>
                        </div>
                        <div class="field sv-calc" style="grid-column:1/-1">
                          <label>Estado nutricional <span class="calc-badge">calculo</span></label>
                          <div class="calc-value">{{ form.estado_nutricional || '—' }}</div>
                        </div>

                      </div>
                    </p-tabpanel>

                    <!-- TAB 2: EXAMEN POR SISTEMAS -->
                    <p-tabpanel value="ef">
                      <p style="font-size:0.82rem;color:#94a3b8;margin:0 0 1rem">
                        Marque los sistemas con hallazgos anormales e ingrese la descripción. Los no marcados se guardan como <strong>Normal</strong>.
                      </p>
                      <div class="ef-sistemas-grid">
                        @for (item of efItems; track item.key) {
                          <div class="ef-row">
                            <div class="ef-check-label">
                              <p-checkbox [(ngModel)]="efState[item.key].anormal" [binary]="true"
                                [inputId]="'ef_' + item.key" (ngModelChange)="onEfCheck(item.key)" />
                              <label [for]="'ef_' + item.key"
                                [class.ef-anormal]="efState[item.key].anormal">{{ item.label }}</label>
                            </div>
                            @if (efState[item.key].anormal) {
                              <input pInputText [(ngModel)]="efState[item.key].texto" class="ef-text-input"
                                [placeholder]="'Hallazgo en ' + item.label.toLowerCase() + '...'" />
                            } @else {
                              <span class="ef-normal-badge">Normal</span>
                            }
                          </div>
                        }
                      </div>
                      <div class="field" style="margin-top:1.25rem">
                        <label>Hallazgos generales</label>
                        <textarea pTextarea [(ngModel)]="form.ef_hallazgos" rows="4" class="w-full"
                          placeholder="Observaciones generales del examen físico..."></textarea>
                      </div>
                    </p-tabpanel>

                  </p-tabpanels>
                </p-tabs>

                <div class="step-actions">
                  <p-button label="Anterior" icon="pi pi-chevron-left" [text]="true" (onClick)="activateCallback(1)" />
                  <p-button label="Siguiente" icon="pi pi-chevron-right" iconPos="right" (onClick)="onNextExamen(activateCallback)" />
                </div>
              </ng-template>
            </p-step-panel>

            <!-- PASO 3: DIAGNÓSTICO CIE-10 -->
            <p-step-panel [value]="3">
              <ng-template pTemplate="content" let-activateCallback="activateCallback">
                <div class="form-grid">
                  <div class="field">
                    <label>Código CIE-10 principal</label>
                    <input pInputText [(ngModel)]="form.diagnostico_principal" class="w-full" placeholder="Ej: J06.9" />
                  </div>
                  <div class="field">
                    <label>Código CIE-10 relacionado</label>
                    <input pInputText [(ngModel)]="form.diagnostico_relacionado1" class="w-full" placeholder="Código adicional" />
                  </div>
                  <!--
                    Finalidad y causa: los dos van al RIPS y hasta el 2026-08-02
                    NO se podían elegir. El sistema los rellenaba —«10» y «13»—
                    con códigos que además eran de la tabla anterior a la
                    Resolución 948 y que el validador rechaza. Sin este campo, el
                    dato lo afirmaba el sistema en nombre del médico.
                    Van sin valor inicial a propósito: vacío es «falta», y el
                    generador del RIPS lo pide al cerrar el mes.
                  -->
                  <div class="field">
                    <label>Finalidad de la atención</label>
                    <p-select [(ngModel)]="form.finalidad" [options]="finalidades()"
                      optionLabel="nombre" optionValue="codigo" [filter]="true" filterBy="nombre"
                      placeholder="Para qué se atendió (Resolución 948)" class="w-full"
                      appendTo="body" [showClear]="true" />
                  </div>
                  <div class="field">
                    <label>Causa o motivo de la atención</label>
                    <p-select [(ngModel)]="form.causa_externa" [options]="causasMotivo()"
                      optionLabel="nombre" optionValue="codigo" [filter]="true" filterBy="nombre"
                      placeholder="Por qué se atendió (Resolución 948)" class="w-full"
                      appendTo="body" [showClear]="true" />
                  </div>
                </div>
                <div class="step-actions">
                  <p-button label="Anterior" icon="pi pi-chevron-left" [text]="true" (onClick)="activateCallback(2)" />
                  <p-button label="Siguiente" icon="pi pi-chevron-right" iconPos="right" (onClick)="activateCallback(4)" />
                </div>
              </ng-template>
            </p-step-panel>

            <!-- PASO 4: PLAN -->
            <p-step-panel [value]="4">
              <ng-template pTemplate="content" let-activateCallback="activateCallback">
                <div class="field-full">
                  <label>Plan de Tratamiento</label>
                  <textarea pTextarea [(ngModel)]="form.plan_tratamiento" rows="8" class="w-full"
                    placeholder="Plan de manejo y tratamiento..."></textarea>
                </div>
                <div class="step-actions">
                  <p-button label="Anterior" icon="pi pi-chevron-left" [text]="true" (onClick)="activateCallback(3)" />
                  <p-button label="Guardar y continuar" icon="pi pi-save"
                    [loading]="saving()" (onClick)="guardarYContinuar(activateCallback)" />
                </div>
              </ng-template>
            </p-step-panel>

            <!-- PASO 5: FÓRMULA MÉDICA -->
            <p-step-panel [value]="5">
              <ng-template pTemplate="content" let-activateCallback="activateCallback">

                @if (!historia()?.id) {
                  <div class="formula-aviso">
                    <i class="pi pi-info-circle"></i>
                    Guarda primero la historia clínica para poder agregar fórmulas médicas.
                  </div>
                } @else {

                  <!-- Botón nueva fórmula -->
                  @if (!mostrarFormFormula()) {
                    <div style="margin-bottom:1rem">
                      <p-button label="Agregar medicamento" icon="pi pi-plus" (onClick)="abrirFormFormula()" />
                    </div>
                  }

                  <!-- Formulario de nueva/edición de fórmula -->
                  @if (mostrarFormFormula()) {
                    <div class="formula-form">
                      <h4 style="margin:0 0 1rem;color:#f1f5f9">
                        {{ formulaEditando() ? 'Editar medicamento' : 'Nuevo medicamento' }}
                      </h4>

                      <!-- Búsqueda de medicamento -->
                      <div class="field" style="margin-bottom:1rem">
                        <label>Buscar en catálogo INVIMA <span style="color:#94a3b8;font-weight:400">(o ingrese manualmente)</span></label>
                        <div class="med-search-wrap">
                          <p-inputgroup>
                            <p-inputgroup-addon><i class="pi pi-search"></i></p-inputgroup-addon>
                            <input pInputText
                              [(ngModel)]="termMed"
                              (ngModelChange)="onTermMedChange($event)"
                              placeholder="Nombre o código CUM..."
                              class="w-full"
                              autocomplete="off" />
                            @if (buscandoMed()) {
                              <p-inputgroup-addon><i class="pi pi-spin pi-spinner"></i></p-inputgroup-addon>
                            }
                          </p-inputgroup>
                          @if (resultadosMed().length > 0) {
                            <div class="med-dropdown">
                              @for (m of resultadosMed(); track m.id) {
                                <div class="med-option" (click)="seleccionarMedicamento(m)">
                                  <span class="med-nombre">{{ m.nombre }}</span>
                                  <span class="med-detalle">{{ m.concentracion }} · {{ m.forma_farmaceutica }}</span>
                                  <span class="med-cum">CUM: {{ m.codigo_cum }}</span>
                                </div>
                              }
                            </div>
                          }
                        </div>
                      </div>

                      <!-- Campos del formulario -->
                      <div class="form-grid">
                        <div class="field" style="grid-column:1/-1">
                          <label>Medicamento *</label>
                          <input pInputText [(ngModel)]="fForm.medicamento" class="w-full"
                            placeholder="Nombre del medicamento (manual o desde búsqueda)" />
                        </div>
                        <div class="field">
                          <label>Código CUM</label>
                          <input pInputText [(ngModel)]="fForm.codigo_cum" class="w-full"
                            placeholder="Auto desde búsqueda o manual" />
                        </div>
                        <div class="field">
                          <label>Dosis *</label>
                          <input pInputText [(ngModel)]="fForm.dosis" class="w-full"
                            placeholder="Ej: 500mg cada 8h por 7 días" />
                        </div>
                        <div class="field">
                          <label>Cantidad *</label>
                          <p-inputNumber [(ngModel)]="fForm.cantidad" [min]="1" class="w-full" />
                        </div>
                        <div class="field">
                          <label>Unidad *</label>
                          <p-select [(ngModel)]="fForm.unidad" [options]="unidades()"
                            optionLabel="nombre" optionValue="nombre"
                            placeholder="Elija la presentación" class="w-full"
                            appendTo="body" />
                          <small class="text-color-secondary">
                            El RIPS la pide como código. Si la presentación no
                            está en la lista, queda anotada al cerrar el mes.
                          </small>
                        </div>
                        <div class="field">
                          <label>Días de tratamiento *</label>
                          <p-inputNumber [(ngModel)]="fForm.dias_tratamiento" [min]="1"
                            class="w-full" placeholder="Ej: 7" />
                          <small class="text-color-secondary">
                            Obligatorio en el RIPS y mayor que cero. No se deduce
                            de la dosis.
                          </small>
                        </div>
                        <div class="field" style="grid-column:1/-1">
                          <label>Observaciones</label>
                          <textarea pTextarea [(ngModel)]="fForm.observaciones" rows="2" class="w-full"
                            placeholder="Indicaciones adicionales para el paciente o farmacia..."></textarea>
                        </div>
                      </div>

                      <div class="step-actions">
                        <p-button label="Cancelar" [text]="true" (onClick)="cerrarFormFormula()" />
                        <p-button
                          [label]="formulaEditando() ? 'Actualizar' : 'Agregar'"
                          icon="pi pi-check"
                          [loading]="savingFormula()"
                          (onClick)="guardarFormula()" />
                      </div>
                    </div>
                  }

                  <!-- Tabla de fórmulas existentes -->
                  @if (formulas().length > 0) {
                    <p-table [value]="formulas()" styleClass="p-datatable-sm" style="margin-top:1rem">
                      <ng-template pTemplate="header">
                        <tr>
                          <th>Medicamento</th>
                          <th>Dosis</th>
                          <th style="text-align:center">Cantidad</th>
                          <th>Unidad</th>
                          <th>Observaciones</th>
                          <th style="text-align:center">Estado</th>
                          <th style="text-align:center">Acciones</th>
                        </tr>
                      </ng-template>
                      <ng-template pTemplate="body" let-f>
                        <tr>
                          <td>
                            <strong>{{ f.medicamento }}</strong>
                            @if (f.codigo_cum) {
                              <div style="font-size:0.75rem;color:#94a3b8">CUM: {{ f.codigo_cum }}</div>
                            }
                          </td>
                          <td>{{ f.dosis }}</td>
                          <td style="text-align:center">{{ f.cantidad }}</td>
                          <td>{{ f.unidad }}</td>
                          <td class="col-truncate">{{ f.observaciones }}</td>
                          <td style="text-align:center">
                            @if (f.dispensada) {
                              <p-tag value="Dispensada" severity="success" icon="pi pi-check" />
                            } @else {
                              <p-tag value="Pendiente" severity="warn" icon="pi pi-clock" />
                            }
                          </td>
                          <td style="text-align:center">
                            @if (!f.dispensada && !historia()?.firmada) {
                              <div style="display:flex;gap:0.4rem;justify-content:center">
                                <p-button icon="pi pi-pencil" [text]="true" size="small"
                                  pTooltip="Editar" (onClick)="editarFormula(f)" />
                                <p-button icon="pi pi-trash" [text]="true" size="small" severity="danger"
                                  pTooltip="Eliminar" (onClick)="confirmarEliminarFormula(f)" />
                              </div>
                            }
                          </td>
                        </tr>
                      </ng-template>
                    </p-table>
                  } @else if (!mostrarFormFormula()) {
                    <div class="sin-resultados" style="padding:2rem">
                      <i class="pi pi-file-o" style="font-size:1.5rem;opacity:0.4"></i>
                      <p style="margin:0">No hay medicamentos en esta fórmula médica.</p>
                    </div>
                  }

                }

                <div class="step-actions" style="margin-top:1.5rem">
                  <p-button label="Anterior" icon="pi pi-chevron-left" [text]="true" (onClick)="activateCallback(4)" />
                  @if (historia()?.id) {
                    <p-button label="Siguiente" icon="pi pi-chevron-right" iconPos="right" (onClick)="activateCallback(6)" />
                  }
                </div>
              </ng-template>
            </p-step-panel>

            <!-- PASO 6: ÓRDENES MÉDICAS -->
            <p-step-panel [value]="6">
              <ng-template pTemplate="content" let-activateCallback="activateCallback">

                @if (!historia()?.id) {
                  <div class="formula-aviso">
                    <i class="pi pi-info-circle"></i>
                    Guarda primero la historia clínica para poder agregar órdenes médicas.
                  </div>
                } @else {

                  @if (!mostrarFormOrden()) {
                    <div style="margin-bottom:1rem">
                      <p-button label="Agregar orden" icon="pi pi-plus" (onClick)="abrirFormOrden()" />
                    </div>
                  }

                  @if (mostrarFormOrden()) {
                    <div class="formula-form">
                      <h4 style="margin:0 0 1rem;color:#f1f5f9">Nueva orden médica</h4>
                      <div class="form-grid">
                        <div class="field">
                          <label>Tipo de orden *</label>
                          <p-select
                            [(ngModel)]="oForm.tipo"
                            [options]="tiposOrden"
                            optionLabel="label"
                            optionValue="value"
                            placeholder="Seleccione tipo"
                            class="w-full" />
                        </div>
                        <div class="field">
                          <label>Código CUPS</label>
                          <input pInputText [(ngModel)]="oForm.codigo_cups" class="w-full" placeholder="Opcional" />
                        </div>
                        <div class="field" style="grid-column:1/-1">
                          <label>Descripción *</label>
                          <input pInputText [(ngModel)]="oForm.descripcion" class="w-full"
                            placeholder="Ej: Hemograma completo, Rayos X de tórax..." />
                        </div>
                        <div class="field" style="grid-column:1/-1">
                          <label>Justificación clínica *</label>
                          <textarea pTextarea [(ngModel)]="oForm.justificacion" rows="3" class="w-full"
                            placeholder="Motivo clínico de la orden..."></textarea>
                        </div>
                        @if (oForm.tipo === 'Remisión Especialista') {
                          <div class="field" style="grid-column:1/-1">
                            <label>Especialidad destino</label>
                            <p-select
                              [(ngModel)]="oForm.especialidad_destino_id"
                              [options]="especialidades()"
                              optionLabel="nombre"
                              optionValue="id"
                              placeholder="Seleccione especialidad"
                              [filter]="true"
                              filterPlaceholder="Buscar..."
                              class="w-full" />
                          </div>
                        }
                      </div>
                      <div class="step-actions">
                        <p-button label="Cancelar" [text]="true" (onClick)="cerrarFormOrden()" />
                        <p-button label="Agregar" icon="pi pi-check"
                          [loading]="savingOrden()" (onClick)="guardarOrden()" />
                      </div>
                    </div>
                  }

                  @if (ordenes().length > 0) {
                    <p-table [value]="ordenes()" styleClass="p-datatable-sm" style="margin-top:1rem">
                      <ng-template pTemplate="header">
                        <tr>
                          <th>Tipo</th>
                          <th>Descripción</th>
                          <th>Especialidad / CUPS</th>
                          <th>Justificación</th>
                          <th style="text-align:center">Estado</th>
                          <th style="text-align:center">Acciones</th>
                        </tr>
                      </ng-template>
                      <ng-template pTemplate="body" let-o>
                        <tr>
                          <td><p-tag [value]="o.tipo" [severity]="ordenSeverity(o.tipo)" /></td>
                          <td><strong>{{ o.descripcion }}</strong></td>
                          <td class="col-truncate">
                            @if (o.especialidad_nombre) { {{ o.especialidad_nombre }} }
                            @else if (o.codigo_cups) { CUPS: {{ o.codigo_cups }} }
                            @else { — }
                          </td>
                          <td class="col-truncate">{{ o.justificacion }}</td>
                          <td style="text-align:center">
                            <p-tag [value]="o.estado" [severity]="estadoOrdenSeverity(o.estado)" />
                          </td>
                          <td style="text-align:center">
                            @if (!historia()?.firmada) {
                              <p-button icon="pi pi-trash" [text]="true" size="small" severity="danger"
                                pTooltip="Eliminar" (onClick)="confirmarEliminarOrden(o)" />
                            }
                          </td>
                        </tr>
                      </ng-template>
                    </p-table>
                  } @else if (!mostrarFormOrden()) {
                    <div class="sin-resultados" style="padding:2rem">
                      <i class="pi pi-clipboard" style="font-size:1.5rem;opacity:0.4"></i>
                      <p style="margin:0">No hay órdenes médicas en esta historia.</p>
                    </div>
                  }

                }

                <div class="step-actions" style="margin-top:1.5rem">
                  <p-button label="Anterior" icon="pi pi-chevron-left" [text]="true" (onClick)="activateCallback(5)" />
                  @if (historia()?.id) {
                    <p-button label="Siguiente" icon="pi pi-chevron-right" iconPos="right" (onClick)="activateCallback(7)" />
                  }
                </div>
              </ng-template>
            </p-step-panel>

            <!-- PASO 7: IMÁGENES DIAGNÓSTICAS -->
            <p-step-panel [value]="7">
              <ng-template pTemplate="content" let-activateCallback="activateCallback">

                @if (!historia()?.id) {
                  <div class="formula-aviso">
                    <i class="pi pi-info-circle"></i>
                    Guarda primero la historia clínica para poder adjuntar imágenes.
                  </div>
                } @else {

                  <div class="imagen-upload-area">
                    <div class="form-grid">
                      <div class="field">
                        <label>Archivo (imagen, PDF, etc.)</label>
                        <div style="display:flex;gap:0.5rem;align-items:center">
                          <input #fileInput type="file" style="display:none"
                            accept="image/*,.pdf,.dcm"
                            (change)="onFileSelected($event)" />
                          <p-button label="Seleccionar archivo" icon="pi pi-upload" [text]="true"
                            (onClick)="fileInput.click()" />
                          @if (imagenArchivoSeleccionado) {
                            <span style="font-size:0.85rem;color:#94a3b8">{{ imagenArchivoSeleccionado.name }}</span>
                          }
                        </div>
                      </div>
                      <div class="field">
                        <label>Descripción</label>
                        <input pInputText [(ngModel)]="imagenDescripcion" class="w-full"
                          placeholder="Ej: Rx tórax PA, ECG, foto lesión..." />
                      </div>
                    </div>
                    <p-button label="Subir imagen" icon="pi pi-cloud-upload"
                      [loading]="uploadingImagen()"
                      [disabled]="!imagenArchivoSeleccionado"
                      (onClick)="subirImagen()" />
                  </div>

                  @if (imagenes().length > 0) {
                    <p-table [value]="imagenes()" styleClass="p-datatable-sm" style="margin-top:1.5rem">
                      <ng-template pTemplate="header">
                        <tr>
                          <th>Archivo</th>
                          <th>Descripción</th>
                          <th>Fecha</th>
                          <th style="text-align:center">Acciones</th>
                        </tr>
                      </ng-template>
                      <ng-template pTemplate="body" let-img>
                        <tr>
                          <td>
                            <a [href]="urlImagen(img.id)" target="_blank" style="color:#818cf8">
                              <i class="pi pi-file" style="margin-right:0.4rem"></i>{{ img.nombre_original }}
                            </a>
                          </td>
                          <td>{{ img.descripcion }}</td>
                          <td>{{ img.fecha_subida | date:'dd/MM/yyyy HH:mm' }}</td>
                          <td style="text-align:center">
                            @if (!historia()?.firmada) {
                              <p-button icon="pi pi-trash" [text]="true" size="small" severity="danger"
                                pTooltip="Eliminar" (onClick)="confirmarEliminarImagen(img)" />
                            }
                          </td>
                        </tr>
                      </ng-template>
                    </p-table>
                  } @else {
                    <div class="sin-resultados" style="padding:2rem">
                      <i class="pi pi-images" style="font-size:1.5rem;opacity:0.4"></i>
                      <p style="margin:0">No hay imágenes adjuntas en esta historia.</p>
                    </div>
                  }

                }

                <div class="step-actions" style="margin-top:1.5rem">
                  <p-button label="Anterior" icon="pi pi-chevron-left" [text]="true" (onClick)="activateCallback(6)" />
                  @if (historia()?.id) {
                    <p-button label="Firmar Historia" icon="pi pi-lock"
                      [loading]="signing()" [disabled]="!!historia()?.firmada"
                      (onClick)="firmarHistoria()" />
                  }
                </div>
              </ng-template>
            </p-step-panel>

          </p-step-panels>
        </p-stepper>
      }

    </div>
    <p-toast />
    <p-confirmDialog />
  `,
  styles: [`
    /* ── Búsqueda ── */
    .search-hint { color: #94a3b8; margin-bottom: 1.5rem; font-size: 0.9rem; }
    .search-box { max-width: 600px; margin-bottom: 1.5rem; }
    .search-input { flex: 1; }
    .resultados { max-width: 700px; }
    .resultados-titulo { font-size: 0.8rem; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 0.5rem; }
    .resultado-item {
      padding: 0.85rem 1rem; border-radius: 8px; cursor: pointer;
      border: 1px solid rgba(255,255,255,0.08); margin-bottom: 0.5rem;
      background: rgba(255,255,255,0.03); transition: background 0.15s;
    }
    .resultado-item:hover { background: rgba(255,255,255,0.07); border-color: rgba(99,102,241,0.5); }
    .resultado-nombre { font-weight: 600; color: #f1f5f9; margin-bottom: 0.25rem; }
    .resultado-detalle { font-size: 0.82rem; color: #94a3b8; display: flex; gap: 0.4rem; flex-wrap: wrap; }
    .sin-resultados { text-align: center; padding: 3rem 1rem; color: #64748b; display: flex; flex-direction: column; align-items: center; gap: 0.75rem; }
    .cargando { padding: 2rem; text-align: center; color: #64748b; display: flex; align-items: center; justify-content: center; gap: 0.5rem; }

    /* ── Lista ── */
    .lista-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem; }
    .col-truncate { max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

    /* ── Cabecera paciente/médico ── */
    .info-header {
      display: flex; gap: 1.5rem; flex-wrap: wrap;
      background: #0f172a; border: 1px solid rgba(255,255,255,0.1); border-radius: 8px;
      padding: 1rem 1.25rem; margin-bottom: 1.5rem;
    }
    .info-block { display: flex; flex-direction: column; gap: 0.2rem; }
    .info-label { font-size: 0.7rem; font-weight: 600; text-transform: uppercase; color: #94a3b8; letter-spacing: 0.05em; }
    .info-name { font-size: 1rem; font-weight: 600; color: #f1f5f9; }
    .info-row { font-size: 0.82rem; color: #cbd5e1; display: flex; gap: 0.4rem; flex-wrap: wrap; align-items: center; }
    .sep { opacity: 0.4; }
    .paciente-block { flex: 2; min-width: 220px; }
    .medico-block { flex: 1.5; min-width: 180px; border-left: 1px solid rgba(255,255,255,0.1); padding-left: 1.25rem; }
    .codigo-block { flex: 0 0 auto; border-left: 1px solid rgba(255,255,255,0.1); padding-left: 1.25rem; }

    /* ── Grids ── */
    .form-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; margin-bottom: 1rem; }
    .obstetrico-grid { display: grid; grid-template-columns: repeat(5, 1fr); gap: 0.75rem; }
    .field { display: flex; flex-direction: column; gap: 0.4rem; }
    .field label, .field-full label { font-weight: 500; font-size: 0.875rem; color: #e2e8f0; }
    .field-full { grid-column: 1/-1; display: flex; flex-direction: column; gap: 0.4rem; }

    /* ── Checkboxes ── */
    .check-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 0.6rem 1rem; }
    .chk { display: flex; align-items: center; gap: 0.5rem; }
    .chk label { color: #f1f5f9; cursor: pointer; font-size: 0.875rem; font-weight: 400; }
    .cond-row { display: flex; align-items: center; gap: 1rem; margin-bottom: 0.5rem; min-height: 2rem; }
    .cond-input { flex: 1; }

    /* ── Paneles antecedentes ── */
    .ant-panel { margin-bottom: 1rem; }

    /* ── IMC ── */
    .imc-value { display: flex; align-items: center; gap: 0.6rem; font-size: 1.1rem; font-weight: 600; color: #f1f5f9; }

    /* ── Step actions ── */
    .step-actions { display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1.5rem; padding-top: 1rem; border-top: 1px solid rgba(255,255,255,0.1); }

    ::ng-deep textarea { resize: vertical; }
    ::ng-deep .ant-panel .p-panel-content { padding: 1rem; }

    /* ── Signos Vitales ── */
    .sv-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
      gap: 1rem;
      padding: 1rem 0;
    }
    .sv-calc { background: rgba(99,102,241,0.06); border-radius: 6px; padding: 0.6rem 0.8rem; }
    .calc-badge {
      display: inline-block; font-size: 0.65rem; font-weight: 600; letter-spacing: 0.05em;
      text-transform: uppercase; color: #818cf8; background: rgba(99,102,241,0.15);
      border-radius: 4px; padding: 0.1rem 0.4rem; margin-left: 0.4rem;
    }
    .calc-value { font-size: 1.1rem; font-weight: 600; color: #f1f5f9; margin-top: 0.25rem; display: flex; align-items: center; }

    /* ── Examen por sistemas ── */
    .ef-sistemas-grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
      gap: 0.6rem;
    }
    .ef-row {
      display: flex; align-items: center; gap: 0.75rem;
      padding: 0.5rem 0.6rem; border-radius: 6px;
      border: 1px solid rgba(255,255,255,0.06);
      background: rgba(255,255,255,0.02);
    }
    .ef-check-label { display: flex; align-items: center; gap: 0.5rem; min-width: 120px; flex-shrink: 0; }
    .ef-check-label label { cursor: pointer; font-size: 0.875rem; color: #cbd5e1; white-space: nowrap; }
    .ef-check-label label.ef-anormal { color: #fbbf24; font-weight: 600; }
    .ef-text-input { flex: 1; font-size: 0.82rem; }
    .ef-normal-badge { font-size: 0.75rem; color: #4ade80; opacity: 0.7; white-space: nowrap; }

    /* ── Fórmula médica ── */
    .formula-aviso {
      display: flex; align-items: center; gap: 0.6rem;
      background: rgba(99,102,241,0.1); border: 1px solid rgba(99,102,241,0.3);
      border-radius: 8px; padding: 1rem 1.25rem; color: #a5b4fc; font-size: 0.9rem;
    }
    .formula-form {
      background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08);
      border-radius: 8px; padding: 1.25rem; margin-bottom: 1rem;
    }
    .med-search-wrap { position: relative; }
    .med-dropdown {
      position: absolute; top: 100%; left: 0; right: 0; z-index: 100;
      background: #1e293b; border: 1px solid rgba(255,255,255,0.15); border-radius: 6px;
      box-shadow: 0 8px 24px rgba(0,0,0,0.4); max-height: 260px; overflow-y: auto;
    }
    .med-option {
      display: flex; flex-direction: column; gap: 0.15rem;
      padding: 0.6rem 0.85rem; cursor: pointer; border-bottom: 1px solid rgba(255,255,255,0.06);
      transition: background 0.12s;
    }
    .med-option:last-child { border-bottom: none; }
    .med-option:hover { background: rgba(99,102,241,0.15); }
    .med-nombre { font-weight: 600; color: #f1f5f9; font-size: 0.875rem; }
    .med-detalle { color: #94a3b8; font-size: 0.78rem; }
    .med-cum { color: #64748b; font-size: 0.72rem; }

    /* ── Imágenes diagnósticas ── */
    .imagen-upload-area {
      background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08);
      border-radius: 8px; padding: 1.25rem; margin-bottom: 1rem;
    }
  `]
})
export class HistoriasComponent implements OnInit {
  private route = inject(ActivatedRoute);
  private svc = inject(HistoriaService);
  private pacienteSvc = inject(PacienteService);
  private messageService = inject(MessageService);
  private confirmationService = inject(ConfirmationService);

  // ── Estado de vistas ──
  vista = signal<Vista>('busqueda');

  // ── Búsqueda ──
  terminoBusqueda = '';
  buscando = signal(false);
  resultadosBusqueda = signal<Patient[]>([]);
  sinResultados = signal(false);

  // ── Lista de historias ──
  historiasPaciente = signal<Historia[]>([]);
  cargandoHistorias = signal(false);

  // ── Formulario historia ──
  historia = signal<Historia | null>(null);
  paciente = signal<PacienteInfo | null>(null);
  medico = signal<MedicoInfo | null>(null);
  activeStep = signal(1);
  saving = signal(false);
  signing = signal(false);

  form: Partial<Historia> = {};
  antFam = defaultAntFam();
  antPers = defaultAntPers();
  antGineco = defaultAntGineco();
  antPrenatales = defaultAntPrenatales();

  // ── Examen físico por sistemas ──
  readonly efItems = [
    { key: 'ef_cabeza', label: 'Cabeza' },
    { key: 'ef_ojos', label: 'Ojos' },
    { key: 'ef_nariz', label: 'Nariz' },
    { key: 'ef_oidos', label: 'Oídos' },
    { key: 'ef_boca', label: 'Boca' },
    { key: 'ef_faringe', label: 'Faringe' },
    { key: 'ef_cuello', label: 'Cuello' },
    { key: 'ef_pulmones', label: 'Pulmones' },
    { key: 'ef_corazon', label: 'Corazón' },
    { key: 'ef_abdomen', label: 'Abdomen' },
    { key: 'ef_gu', label: 'G.U.' },
    { key: 'ef_neurologico', label: 'Nervioso' },
    { key: 'ef_extremidad', label: 'Extremidad' },
    { key: 'ef_osteomuscular', label: 'Osteomuscular' },
    { key: 'ef_piel', label: 'Piel' },
  ] as const;
  efState: Record<string, { anormal: boolean; texto: string }> = {};
  tallaMts: number | null = null;

  // ── Fórmulas médicas ──
  formulas = signal<Formula[]>([]);
  mostrarFormFormula = signal(false);
  formulaEditando = signal<Formula | null>(null);
  savingFormula = signal(false);
  termMed = '';
  buscandoMed = signal(false);
  resultadosMed = signal<MedicamentoInfo[]>([]);
  // Sin `unidad: 'tabletas'`: era un dato inventado con aspecto de dato. Vacío,
  // la falta se ve al cerrar el mes; relleno, se factura una presentación que
  // nadie eligió y de la que dependen el valor unitario y la cantidad.
  fForm: FormulaCreate = { medicamento: '', dosis: '', cantidad: 1 };
  readonly unidades = signal<UnidadDispensacion[]>([]);
  // Las dos tablas de la 948 que la historia tiene que ofrecer. Solo las que
  // aplican al archivo de CONSULTAS: unas cuantas finalidades son exclusivas de
  // procedimientos y ofrecerlas aquí produciría un RIPS rechazado.
  readonly finalidades = signal<Finalidad[]>([]);
  readonly causasMotivo = signal<CausaMotivo[]>([]);
  private medSearch$ = new Subject<string>();

  // ── Órdenes médicas ──
  ordenes = signal<Orden[]>([]);
  mostrarFormOrden = signal(false);
  savingOrden = signal(false);
  especialidades = signal<EspecialidadInfo[]>([]);
  oForm: OrdenCreate = { tipo: '', descripcion: '', justificacion: '' };
  readonly tiposOrden = [
    { label: 'Laboratorio', value: 'Laboratorio' },
    { label: 'Imagenología', value: 'Imagenología' },
    { label: 'Remisión Especialista', value: 'Remisión Especialista' },
    { label: 'Cirugía', value: 'Cirugía' },
    { label: 'Terapia', value: 'Terapia' },
    { label: 'Otro', value: 'Otro' },
  ];

  // ── Imágenes diagnósticas ──
  imagenes = signal<ImagenDiagnostica[]>([]);
  uploadingImagen = signal(false);
  imagenArchivoSeleccionado: File | null = null;
  imagenDescripcion = '';

  esMujer = computed(() => {
    const s = this.paciente()?.sexo ?? '';
    return s === 'F' || s.toLowerCase() === 'femenino' || s === 'f';
  });

  ngOnInit(): void {
    // Las presentaciones vienen del backend, que es donde vive el mapa a los
    // códigos del RIPS. Si falla, la lista queda vacía y el campo se ve vacío:
    // preferible a ofrecer opciones que quizá ya no se sepan traducir.
    this.svc.getUnidadesDispensacion()
      .pipe(catchError(() => of([] as UnidadDispensacion[])))
      .subscribe(u => this.unidades.set(u));

    // Mismo criterio que arriba: si el catálogo no llega, la lista queda vacía
    // y el campo se ve vacío. Lo que no se hace es preseleccionar una opción
    // «razonable», que es exactamente como llegamos a reportar «10» y «13» en
    // toda atención sin que nadie lo hubiera elegido.
    this.svc.getFinalidades()
      .pipe(catchError(() => of([] as Finalidad[])))
      .subscribe(f => this.finalidades.set(f.filter(x => x.aplica_ac)));
    this.svc.getCausasMotivo()
      .pipe(catchError(() => of([] as CausaMotivo[])))
      .subscribe(c => this.causasMotivo.set(c.filter(x => x.aplica_ac)));

    // Búsqueda de medicamentos con debounce
    this.medSearch$.pipe(
      debounceTime(350),
      distinctUntilChanged(),
      switchMap(q => {
        if (q.length < 2) { this.buscandoMed.set(false); return of([]); }
        this.buscandoMed.set(true);
        return this.svc.buscarMedicamentos(q).pipe(catchError(() => of([])));
      }),
    ).subscribe(res => {
      this.resultadosMed.set(res);
      this.buscandoMed.set(false);
    });

    const historiaId = this.route.snapshot.queryParamMap.get('historia_id');
    const pacienteId = this.route.snapshot.queryParamMap.get('paciente_id');
    const citaId = this.route.snapshot.queryParamMap.get('cita_id');

    if (historiaId) {
      // Viene con historia específica (p.ej. desde citas) → abrir formulario directo
      this.svc.get(Number(historiaId)).subscribe(h => {
        this.historia.set(h);
        this.form = { ...h };
        if (h.paciente) { this.paciente.set(h.paciente); this._loadAnt(h.paciente); }
        if (h.medico) this.medico.set(h.medico);
        this.tallaMts = h.talla ? Math.round(h.talla / 100 * 100) / 100 : null;
        this._initEfState();
        this._cargarFormulas(h.id);
        this._cargarOrdenes(h.id);
        this._cargarImagenes(h.id);
        this._cargarEspecialidades();
        this.vista.set('formulario');
      });
    } else if (pacienteId) {
      // Viene con paciente (p.ej. desde citas para nueva HC) → abrir lista o formulario
      if (citaId) {
        // Con cita: cargar contexto y abrir formulario nuevo
        this.form.paciente_id = Number(pacienteId);
        this.form.cita_id = Number(citaId);
        this.svc.getPacienteContexto(Number(pacienteId)).subscribe(ctx => {
          this.paciente.set(ctx.paciente);
          if (ctx.medico) this.medico.set(ctx.medico);
          this._loadAnt(ctx.paciente);
          this.vista.set('formulario');
        });
      } else {
        // Sin cita: cargar contexto y mostrar lista de historias
        this.svc.getPacienteContexto(Number(pacienteId)).subscribe(ctx => {
          this.paciente.set(ctx.paciente);
          if (ctx.medico) this.medico.set(ctx.medico);
          this._loadAnt(ctx.paciente);
          this._cargarHistoriasPaciente(Number(pacienteId));
          this.vista.set('lista');
        });
      }
    }
    // Sin params → vista de búsqueda (por defecto)
  }

  // ── Búsqueda ──
  buscarPacientes(): void {
    const q = this.terminoBusqueda.trim();
    if (!q) return;
    this.buscando.set(true);
    this.sinResultados.set(false);
    this.resultadosBusqueda.set([]);
    this.pacienteSvc.buscar(q).subscribe({
      next: res => {
        this.buscando.set(false);
        this.resultadosBusqueda.set(res);
        this.sinResultados.set(res.length === 0);
      },
      error: () => {
        this.buscando.set(false);
        this.messageService.add({ severity: 'error', summary: 'Error', detail: 'Error al buscar pacientes' });
      },
    });
  }

  nombreCompleto(p: Patient): string {
    return [p.nombre1, p.nombre2, p.apellido1, p.apellido2].filter(Boolean).join(' ');
  }

  seleccionarPaciente(p: Patient): void {
    this.svc.getPacienteContexto(p.id).subscribe(ctx => {
      this.paciente.set(ctx.paciente);
      if (ctx.medico) this.medico.set(ctx.medico);
      this._loadAnt(ctx.paciente);
      this._cargarHistoriasPaciente(p.id);
      this.vista.set('lista');
    });
  }

  // ── Lista de historias ──
  private _cargarHistoriasPaciente(pacienteId: number): void {
    this.cargandoHistorias.set(true);
    this.svc.listByPaciente(pacienteId).subscribe({
      next: hs => {
        this.historiasPaciente.set(hs);
        this.cargandoHistorias.set(false);
      },
      error: () => {
        this.cargandoHistorias.set(false);
        this.messageService.add({ severity: 'error', summary: 'Error', detail: 'Error al cargar historias' });
      },
    });
  }

  nuevaHistoria(): void {
    const pacienteId = this.paciente()?.id;
    if (!pacienteId) return;
    this.historia.set(null);
    this.form = { paciente_id: pacienteId };
    this.tallaMts = null;
    this._initEfState();
    this.activeStep.set(1);
    this.vista.set('formulario');
  }

  editarHistoria(h: Historia): void {
    this.historia.set(h);
    this.form = { ...h };
    if (h.paciente) this._loadAnt(h.paciente);
    this.tallaMts = h.talla ? Math.round(h.talla / 100 * 100) / 100 : null;
    this._initEfState();
    this.activeStep.set(1);
    this.formulas.set([]);
    this.ordenes.set([]);
    this.imagenes.set([]);
    this.cerrarFormFormula();
    this.cerrarFormOrden();
    this._cargarFormulas(h.id);
    this._cargarOrdenes(h.id);
    this._cargarImagenes(h.id);
    this._cargarEspecialidades();
    this.vista.set('formulario');
  }

  confirmarEliminar(h: Historia): void {
    this.confirmationService.confirm({
      message: `¿Eliminar la historia <strong>${h.codigo_historia}</strong>? Esta acción no se puede deshacer.`,
      header: 'Confirmar eliminación',
      icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Eliminar',
      rejectLabel: 'Cancelar',
      acceptButtonStyleClass: 'p-button-danger',
      accept: () => this._eliminarHistoria(h),
    });
  }

  private _eliminarHistoria(h: Historia): void {
    this.svc.delete(h.id).subscribe({
      next: () => {
        this.historiasPaciente.update(hs => hs.filter(x => x.id !== h.id));
        this.messageService.add({ severity: 'success', summary: 'Eliminada', detail: 'Historia eliminada correctamente' });
      },
      error: (err) => {
        const detail = err?.error?.detail ?? 'Error al eliminar la historia';
        this.messageService.add({ severity: 'error', summary: 'Error', detail });
      },
    });
  }

  // ── Navegación ──
  volverBusqueda(): void {
    this.resultadosBusqueda.set([]);
    this.sinResultados.set(false);
    this.terminoBusqueda = '';
    this.paciente.set(null);
    this.historiasPaciente.set([]);
    this.vista.set('busqueda');
  }

  volverLista(): void {
    const pacienteId = this.paciente()?.id;
    if (pacienteId) {
      this._cargarHistoriasPaciente(pacienteId);
    }
    this.historia.set(null);
    this.form = {};
    this.activeStep.set(1);
    this.formulas.set([]);
    this.ordenes.set([]);
    this.imagenes.set([]);
    this.cerrarFormFormula();
    this.cerrarFormOrden();
    this.imagenArchivoSeleccionado = null;
    this.imagenDescripcion = '';
    this.vista.set('lista');
  }

  // ── Formulario ──
  private _loadAnt(p: PacienteInfo): void {
    if (p.ant_familiares) this.antFam = { ...defaultAntFam(), ...p.ant_familiares } as AntFamiliares;
    if (p.ant_personales) this.antPers = { ...defaultAntPers(), ...p.ant_personales } as AntPersonales;
    if (p.ant_ginecologicos) this.antGineco = { ...defaultAntGineco(), ...p.ant_ginecologicos } as AntGineco;
    if (p.ant_obstetricos) this.antPrenatales = { ...defaultAntPrenatales(), ...p.ant_obstetricos } as AntPrenatales;
  }

  // ── Examen físico — checkboxes ───────────────────────────────
  private _initEfState(): void {
    for (const item of this.efItems) {
      const val = (this.form as any)[item.key] as string | undefined;
      const anormal = !!val && val !== 'Normal';
      this.efState[item.key] = { anormal, texto: anormal ? val! : '' };
    }
  }

  onEfCheck(key: string): void {
    if (!this.efState[key].anormal) this.efState[key].texto = '';
  }

  onNextExamen(activateCallback: (n: number) => void): void {
    // Aplica el estado de checkboxes al form antes de avanzar
    for (const item of this.efItems) {
      const s = this.efState[item.key];
      (this.form as any)[item.key] = s?.anormal ? (s.texto.trim() || 'Anormal') : 'Normal';
    }
    activateCallback(3);
  }

  // ── Signos vitales — cálculos ────────────────────────────────
  onTallaChange(m: number | null): void {
    this.tallaMts = m;
    this.form.talla = m ? Math.round(m * 100 * 10) / 10 : undefined;
    this.calcularMetrics();
  }

  calcularMetrics(): void {
    const m = this.tallaMts ?? (this.form.talla ? this.form.talla / 100 : null);
    const kg = this.form.peso ?? null;

    if (m && m > 0) {
      if (kg && kg > 0) {
        this.form.imc = Math.round(kg / (m * m) * 10) / 10;
        this.form.peso_ideal = Math.round(22 * m * m * 10) / 10;
        this.form.estado_nutricional = this._estadoNutricional(this.form.imc);
      }
    }
  }

  tallaIdeal = computed<number | null>(() => {
    const kg = this.form.peso ?? null;
    if (!kg || kg <= 0) return null;
    return Math.round(Math.sqrt(kg / 22) * 100) / 100;
  });

  imcLabel(): string {
    const v = this.form.imc ?? 0;
    if (v < 16) return 'Desnutrición severa';
    if (v < 17) return 'Desnutrición moderada';
    if (v < 18.5) return 'Bajo peso';
    if (v < 25) return 'Normal';
    if (v < 30) return 'Sobrepeso';
    if (v < 35) return 'Obesidad I';
    if (v < 40) return 'Obesidad II';
    return 'Obesidad III';
  }

  imcSeverity(): 'warn' | 'success' | 'danger' {
    const v = this.form.imc ?? 0;
    if (v < 18.5 || (v >= 25 && v < 30)) return 'warn';
    if (v >= 30) return 'danger';
    return 'success';
  }

  private _estadoNutricional(imc: number): string {
    if (imc < 16) return 'Desnutrición severa';
    if (imc < 17) return 'Desnutrición moderada';
    if (imc < 18.5) return 'Bajo peso';
    if (imc < 25) return 'Normal';
    if (imc < 30) return 'Sobrepeso';
    if (imc < 35) return 'Obesidad grado I';
    if (imc < 40) return 'Obesidad grado II';
    return 'Obesidad grado III (mórbida)';
  }

  private _antPayload() {
    return {
      ant_familiares: { ...this.antFam },
      ant_personales: {
        hta: this.antPers.hta, diabetes: this.antPers.diabetes,
        coronarias: this.antPers.coronarias, acv: this.antPers.acv,
        tbc: this.antPers.tbc, asma: this.antPers.asma,
        cancer: this.antPers.cancer_activo ? (this.antPers.cancer_desc || 'Sí') : 'no',
        quirurgicos: this.antPers.quirurgicos_activo ? (this.antPers.quirurgicos_desc || 'Sí') : 'no',
        its: this.antPers.its_activo ? (this.antPers.its_desc || 'Sí') : 'no',
        otros: this.antPers.otros, habitos: this.antPers.habitos,
      },
      ...(this.esMujer() ? {
        ant_ginecologicos: { ...this.antGineco },
        ant_obstetricos: { ...this.antPrenatales },
      } : {}),
    };
  }

  guardarYContinuar(activateCallback: (step: number) => void): void {
    const pacienteId = this.form.paciente_id ?? this.historia()?.paciente_id;
    if (!pacienteId) return;
    this.saving.set(true);

    this.svc.updateAntecedentes(pacienteId, this._antPayload()).subscribe({
      next: () => {
        if (!this.historia()?.id) {
          // Al crear, el formulario debe llevar el paciente y el médico que
          // atiende (el del contexto); la API los exige y no los inventa.
          this.form.paciente_id = pacienteId;
          if (this.medico()) this.form.medico_id = this.medico()!.id;
        }
        const obs = this.historia()?.id
          ? this.svc.update(this.historia()!.id, this.form)
          : this.svc.create(this.form);
        obs.subscribe({
          next: h => {
            this.historia.set(h);
            if (h.paciente) this.paciente.set(h.paciente);
            if (h.medico) this.medico.set(h.medico);
            this.saving.set(false);
            this.messageService.add({ severity: 'success', summary: 'Guardado', detail: 'Historia guardada exitosamente' });
            this._cargarFormulas(h.id);
            this._cargarOrdenes(h.id);
            this._cargarImagenes(h.id);
            this._cargarEspecialidades();
            activateCallback(5);
          },
          error: () => {
            this.saving.set(false);
            this.messageService.add({ severity: 'error', summary: 'Error', detail: 'Error al guardar la historia' });
          },
        });
      },
      error: () => {
        this.saving.set(false);
        this.messageService.add({ severity: 'error', summary: 'Error', detail: 'Error al guardar los antecedentes' });
      },
    });
  }

  guardar(): void {
    const pacienteId = this.form.paciente_id ?? this.historia()?.paciente_id;
    if (!pacienteId) return;
    this.saving.set(true);

    this.svc.updateAntecedentes(pacienteId, this._antPayload()).subscribe({
      next: () => {
        if (!this.historia()?.id) {
          // Al crear, el formulario debe llevar el paciente y el médico que
          // atiende (el del contexto); la API los exige y no los inventa.
          this.form.paciente_id = pacienteId;
          if (this.medico()) this.form.medico_id = this.medico()!.id;
        }
        const obs = this.historia()?.id
          ? this.svc.update(this.historia()!.id, this.form)
          : this.svc.create(this.form);
        obs.subscribe({
          next: h => {
            this.historia.set(h);
            if (h.paciente) this.paciente.set(h.paciente);
            if (h.medico) this.medico.set(h.medico);
            this.saving.set(false);
            this.messageService.add({ severity: 'success', summary: 'Guardado', detail: 'Historia guardada exitosamente' });
          },
          error: () => {
            this.saving.set(false);
            this.messageService.add({ severity: 'error', summary: 'Error', detail: 'Error al guardar la historia' });
          },
        });
      },
      error: () => {
        this.saving.set(false);
        this.messageService.add({ severity: 'error', summary: 'Error', detail: 'Error al guardar los antecedentes' });
      },
    });
  }

  firmarHistoria(): void {
    if (!this.historia()?.id) return;
    this.signing.set(true);
    this.svc.firmar(this.historia()!.id).subscribe({
      next: () => {
        this.historia.update(h => h ? { ...h, firmada: true } : h);
        this.signing.set(false);
        this.messageService.add({ severity: 'success', summary: 'Firmada', detail: 'Historia firmada digitalmente' });
      },
      error: () => {
        this.signing.set(false);
        this.messageService.add({ severity: 'error', summary: 'Error', detail: 'Error al firmar la historia' });
      },
    });
  }

  // ── Fórmulas médicas ──────────────────────────────────────────

  private _cargarFormulas(historiaId: number): void {
    this.svc.listFormulas(historiaId).subscribe(fs => this.formulas.set(fs));
  }

  private _resetFForm(): void {
    this.fForm = { medicamento: '', dosis: '', cantidad: 1 };
    this.termMed = '';
    this.resultadosMed.set([]);
    this.formulaEditando.set(null);
  }

  abrirFormFormula(): void {
    this._resetFForm();
    this.mostrarFormFormula.set(true);
  }

  cerrarFormFormula(): void {
    this._resetFForm();
    this.mostrarFormFormula.set(false);
  }

  onTermMedChange(q: string): void {
    if (q.length < 2) { this.resultadosMed.set([]); return; }
    this.medSearch$.next(q);
  }

  seleccionarMedicamento(m: MedicamentoInfo): void {
    this.fForm.medicamento = `${m.nombre} ${m.concentracion} ${m.forma_farmaceutica}`.trim();
    this.fForm.codigo_cum = m.codigo_cum;
    this.termMed = m.nombre;
    this.resultadosMed.set([]);
  }

  editarFormula(f: Formula): void {
    this.formulaEditando.set(f);
    this.fForm = {
      medicamento: f.medicamento,
      codigo_cum: f.codigo_cum,
      dosis: f.dosis,
      cantidad: f.cantidad,
      unidad: f.unidad ?? '',
      dias_tratamiento: f.dias_tratamiento,
      observaciones: f.observaciones,
    };
    this.termMed = f.medicamento;
    this.resultadosMed.set([]);
    this.mostrarFormFormula.set(true);
  }

  guardarFormula(): void {
    if (!this.fForm.medicamento || !this.fForm.dosis || !this.fForm.cantidad) {
      this.messageService.add({ severity: 'warn', summary: 'Campos requeridos', detail: 'Medicamento, dosis y cantidad son obligatorios' });
      return;
    }
    const historiaId = this.historia()!.id;
    this.savingFormula.set(true);

    // Crear devuelve el id y actualizar 204: tipos distintos que aquí no se usan
    // (se recarga la lista), por eso el observable se toma como `unknown`.
    const obs: Observable<unknown> = this.formulaEditando()
      ? this.svc.updateFormula(historiaId, this.formulaEditando()!.id, this.fForm)
      : this.svc.createFormula(historiaId, this.fForm);

    obs.subscribe({
      next: () => {
        // Se recarga la lista para traer la fila completa tal como la guardó el
        // servidor (crear devuelve solo el id; actualizar, 204 sin cuerpo).
        this.svc.listFormulas(historiaId).subscribe({ next: fs => this.formulas.set(fs) });
        this.savingFormula.set(false);
        this.cerrarFormFormula();
        this.messageService.add({ severity: 'success', summary: 'Guardado', detail: 'Medicamento guardado en la fórmula' });
      },
      error: err => {
        this.savingFormula.set(false);
        const detail = err?.error?.detail ?? 'Error al guardar el medicamento';
        this.messageService.add({ severity: 'error', summary: 'Error', detail });
      },
    });
  }

  confirmarEliminarFormula(f: Formula): void {
    this.confirmationService.confirm({
      message: `¿Eliminar <strong>${f.medicamento}</strong> de la fórmula?`,
      header: 'Confirmar eliminación',
      icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Eliminar',
      rejectLabel: 'Cancelar',
      acceptButtonStyleClass: 'p-button-danger',
      accept: () => {
        this.svc.deleteFormula(this.historia()!.id, f.id).subscribe({
          next: () => {
            this.formulas.update(fs => fs.filter(x => x.id !== f.id));
            this.messageService.add({ severity: 'success', summary: 'Eliminado', detail: 'Medicamento eliminado de la fórmula' });
          },
          error: err => {
            const detail = err?.error?.detail ?? 'Error al eliminar el medicamento';
            this.messageService.add({ severity: 'error', summary: 'Error', detail });
          },
        });
      },
    });
  }

  // ── Especialidades ───────────────────────────────────────────────
  private _cargarEspecialidades(): void {
    if (this.especialidades().length > 0) return;
    this.svc.listEspecialidades().subscribe({ next: es => this.especialidades.set(es) });
  }

  // ── Órdenes médicas ──────────────────────────────────────────────
  private _cargarOrdenes(historiaId: number): void {
    this.svc.listOrdenes(historiaId).subscribe({ next: os => this.ordenes.set(os) });
  }

  private _resetOForm(): void {
    this.oForm = { tipo: '', descripcion: '', justificacion: '' };
  }

  abrirFormOrden(): void {
    this._resetOForm();
    this.mostrarFormOrden.set(true);
  }

  cerrarFormOrden(): void {
    this._resetOForm();
    this.mostrarFormOrden.set(false);
  }

  guardarOrden(): void {
    if (!this.oForm.tipo || !this.oForm.descripcion || !this.oForm.justificacion) {
      this.messageService.add({ severity: 'warn', summary: 'Campos requeridos', detail: 'Tipo, descripción y justificación son obligatorios' });
      return;
    }
    this.savingOrden.set(true);
    const historiaId = this.historia()!.id;
    this.svc.createOrden(historiaId, this.oForm).subscribe({
      next: () => {
        // Se recarga la lista: crear devuelve solo el id, no la fila completa.
        this.svc.listOrdenes(historiaId).subscribe({ next: os => this.ordenes.set(os) });
        this.cerrarFormOrden();
        this.savingOrden.set(false);
        this.messageService.add({ severity: 'success', summary: 'Orden creada', detail: 'Orden agregada' });
      },
      error: err => {
        this.savingOrden.set(false);
        const detail = err?.error?.detail ?? 'Error al crear la orden';
        this.messageService.add({ severity: 'error', summary: 'Error', detail });
      },
    });
  }

  confirmarEliminarOrden(o: Orden): void {
    this.confirmationService.confirm({
      message: `¿Eliminar la orden de <strong>${o.tipo}</strong>?`,
      header: 'Confirmar eliminación',
      icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Eliminar',
      rejectLabel: 'Cancelar',
      acceptButtonStyleClass: 'p-button-danger',
      accept: () => {
        this.svc.deleteOrden(this.historia()!.id, o.id).subscribe({
          next: () => {
            this.ordenes.update(os => os.filter(x => x.id !== o.id));
            this.messageService.add({ severity: 'success', summary: 'Eliminada', detail: 'Orden eliminada' });
          },
          error: err => {
            const detail = err?.error?.detail ?? 'Error al eliminar la orden';
            this.messageService.add({ severity: 'error', summary: 'Error', detail });
          },
        });
      },
    });
  }

  ordenSeverity(tipo: string): 'info' | 'success' | 'warn' | 'danger' | 'secondary' {
    if (tipo === 'Laboratorio') return 'info';
    if (tipo === 'Imagenología') return 'secondary';
    if (tipo === 'Remisión Especialista') return 'warn';
    if (tipo === 'Cirugía') return 'danger';
    return 'success';
  }

  estadoOrdenSeverity(estado: string): 'info' | 'success' | 'warn' | 'danger' | 'secondary' {
    if (estado === 'Completada') return 'success';
    if (estado === 'Cancelada') return 'danger';
    if (estado === 'En Proceso') return 'info';
    return 'warn';
  }

  // ── Imágenes diagnósticas ────────────────────────────────────────
  private _cargarImagenes(historiaId: number): void {
    this.svc.listImagenes(historiaId).subscribe({ next: imgs => this.imagenes.set(imgs) });
  }

  /** URL para ver o descargar el archivo. El navegador manda la cookie de
   *  sesión, así que solo lo sirve a quien puede ver la historia. */
  urlImagen(imagenId: number): string {
    return this.svc.urlArchivo(imagenId);
  }

  onFileSelected(event: Event): void {
    const input = event.target as HTMLInputElement;
    this.imagenArchivoSeleccionado = input.files?.[0] ?? null;
  }

  subirImagen(): void {
    if (!this.imagenArchivoSeleccionado || !this.historia()?.id) return;
    this.uploadingImagen.set(true);
    this.svc.uploadImagen(
      this.historia()!.id,
      this.imagenArchivoSeleccionado,
      this.imagenDescripcion || this.imagenArchivoSeleccionado.name,
    ).subscribe({
      next: () => {
        // La subida devuelve solo el id; se recarga la lista para traer la fila
        // completa (mime, tamaño, fecha) tal como la guardó el servidor.
        this._cargarImagenes(this.historia()!.id);
        this.imagenArchivoSeleccionado = null;
        this.imagenDescripcion = '';
        this.uploadingImagen.set(false);
        this.messageService.add({ severity: 'success', summary: 'Subida', detail: 'Imagen adjuntada correctamente' });
      },
      error: err => {
        this.uploadingImagen.set(false);
        const detail = err?.error?.detail ?? 'Error al subir la imagen';
        this.messageService.add({ severity: 'error', summary: 'Error', detail });
      },
    });
  }

  confirmarEliminarImagen(img: ImagenDiagnostica): void {
    this.confirmationService.confirm({
      message: `¿Eliminar <strong>${img.nombre_original}</strong>?`,
      header: 'Confirmar eliminación',
      icon: 'pi pi-exclamation-triangle',
      acceptLabel: 'Eliminar',
      rejectLabel: 'Cancelar',
      acceptButtonStyleClass: 'p-button-danger',
      accept: () => {
        this.svc.deleteImagen(img.id).subscribe({
          next: () => {
            this.imagenes.update(imgs => imgs.filter(x => x.id !== img.id));
            this.messageService.add({ severity: 'success', summary: 'Eliminada', detail: 'Imagen eliminada' });
          },
          error: err => {
            const detail = err?.error?.detail ?? 'Error al eliminar la imagen';
            this.messageService.add({ severity: 'error', summary: 'Error', detail });
          },
        });
      },
    });
  }
}
