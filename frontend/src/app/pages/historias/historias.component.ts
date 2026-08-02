import { Component, OnInit, inject, signal, computed } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ActivatedRoute } from '@angular/router';
import { MatButtonModule } from '@angular/material/button';
import { MatCardModule } from '@angular/material/card';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatExpansionModule } from '@angular/material/expansion';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatInputModule } from '@angular/material/input';
import { MatSelectModule } from '@angular/material/select';
import { MatStepperModule } from '@angular/material/stepper';
import { MatTableModule } from '@angular/material/table';
import { MatTabsModule } from '@angular/material/tabs';
import { MatDividerModule } from '@angular/material/divider';
import { EtiquetaComponent } from '../../shared/ui/etiqueta.component';
import { MatTooltipModule } from '@angular/material/tooltip';
import { HistoriaService, Historia, PacienteInfo, MedicoInfo, Formula, FormulaCreate, MedicamentoInfo, Orden, OrdenCreate, ImagenDiagnostica, EspecialidadInfo,
         UnidadDispensacion, Finalidad, CausaMotivo } from '../../services/historia.service';
import { PacienteService } from '../../services/paciente.service';
import { Patient } from '../../models/patient.models';
import { Avisos } from '../../shared/ui/avisos.service';
import { Confirmar } from '../../shared/ui/confirmar.service';
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
  imports: [
    CommonModule, FormsModule,
    MatButtonModule, MatCardModule, MatCheckboxModule, MatExpansionModule,
    MatFormFieldModule, MatInputModule, MatSelectModule, MatStepperModule,
    MatTableModule, MatTabsModule,
    MatDividerModule, MatTooltipModule, EtiquetaComponent,
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
          <div class="buscador">
            <mat-form-field appearance="outline" subscriptSizing="dynamic" class="search-input">
              <mat-label>Paciente</mat-label>
              <input matInput [(ngModel)]="terminoBusqueda"
                placeholder="Nombre o documento del paciente..."
                (keydown.enter)="buscarPacientes()" />
            </mat-form-field>
            <button matButton="filled" [disabled]="buscando()" (click)="buscarPacientes()">
              <i [class]="buscando() ? 'pi pi-spin pi-spinner' : 'pi pi-search'"></i> Buscar
            </button>
          </div>
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
            <button matButton="filled" (click)="nuevaHistoria()"><i class="pi pi-plus"></i> Nueva Historia</button>
            <button matButton (click)="volverBusqueda()"><i class="pi pi-arrow-left"></i> Volver</button>
          </div>
        </div>

        @if (cargandoHistorias()) {
          <div class="cargando"><i class="pi pi-spin pi-spinner"></i> Cargando historias...</div>
        } @else if (historiasPaciente().length === 0) {
          <div class="sin-resultados">
            <i class="pi pi-file-o" style="font-size:2rem;opacity:0.4"></i>
            <p>Este paciente no tiene historias clínicas registradas.</p>
            <button matButton="filled" (click)="nuevaHistoria()"><i class="pi pi-plus"></i> Crear Primera Historia</button>
          </div>
        } @else {
          <table mat-table [dataSource]="historiasPaciente()" class="tabla">
            <ng-container matColumnDef="codigo">
              <th mat-header-cell *matHeaderCellDef>Código</th>
              <td mat-cell *matCellDef="let h"><strong>{{ h.codigo_historia }}</strong></td>
            </ng-container>
            <ng-container matColumnDef="fecha">
              <th mat-header-cell *matHeaderCellDef>Fecha</th>
              <td mat-cell *matCellDef="let h">{{ h.fecha | date:'dd/MM/yyyy' }}</td>
            </ng-container>
            <ng-container matColumnDef="motivo">
              <th mat-header-cell *matHeaderCellDef>Motivo de Consulta</th>
              <td mat-cell *matCellDef="let h" class="col-truncate">{{ h.motivo_consulta }}</td>
            </ng-container>
            <ng-container matColumnDef="diagnostico">
              <th mat-header-cell *matHeaderCellDef>Diagnóstico</th>
              <td mat-cell *matCellDef="let h" class="col-truncate">{{ h.diagnostico_principal }}</td>
            </ng-container>
            <ng-container matColumnDef="medico">
              <th mat-header-cell *matHeaderCellDef>Médico</th>
              <td mat-cell *matCellDef="let h">{{ h.medico?.nombre_completo }}</td>
            </ng-container>
            <ng-container matColumnDef="estado">
              <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
              <td mat-cell *matCellDef="let h" class="centro">
                @if (h.firmada) {
                  <x-etiqueta value="Firmada" severity="success" icon="pi pi-lock" />
                } @else {
                  <x-etiqueta value="Borrador" severity="warn" icon="pi pi-pencil" />
                }
              </td>
            </ng-container>
            <ng-container matColumnDef="acciones">
              <th mat-header-cell *matHeaderCellDef class="centro">Acciones</th>
              <td mat-cell *matCellDef="let h" class="centro">
                <div style="display:flex;gap:0.4rem;justify-content:center">
                  <button matIconButton matTooltip="Ver / Editar" (click)="editarHistoria(h)">
                    <i class="pi pi-eye"></i>
                  </button>
                  @if (!h.firmada) {
                    <button matIconButton class="peligro" matTooltip="Eliminar" (click)="confirmarEliminar(h)">
                      <i class="pi pi-trash"></i>
                    </button>
                  }
                </div>
              </td>
            </ng-container>

            <tr mat-header-row *matHeaderRowDef="columnasHistorias"></tr>
            <tr mat-row *matRowDef="let fila; columns: columnasHistorias"></tr>
          </table>
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
            <button matIconButton matTooltip="Volver a la lista" (click)="volverLista()"><i class="pi pi-arrow-left"></i></button>
            <h2 class="page-title" style="margin-bottom:0">
              {{ historia()?.id ? 'Editar Historia' : 'Nueva Historia' }}
            </h2>
          </div>
          @if (historia()?.id) {
            <button matButton="filled" [disabled]="!!historia()?.firmada || signing()" (click)="firmarHistoria()"><i [class]="signing() ? 'pi pi-spin pi-spinner' : 'pi pi-lock'"></i> Firmar Historia</button>
          }
        </div>

        <!-- El paso activo vive en la señal, no en el componente: lo mueven
             tanto la cabecera del stepper como los botones de Siguiente y
             Anterior, y hay código (guardarYContinuar) que salta de paso. -->
        <mat-stepper [selectedIndex]="activeStep() - 1"
          (selectionChange)="activeStep.set($event.selectedIndex + 1)">

            <!-- PASO 1: ANTECEDENTES -->
            <mat-step label="Antecedentes">

                <mat-expansion-panel expanded class="ant-panel">
                  <mat-expansion-panel-header>
                    <mat-panel-title>Antecedentes familiares</mat-panel-title>
                  </mat-expansion-panel-header>
                  <div class="check-grid">
                    <div class="chk"><mat-checkbox [(ngModel)]="antFam.hta">HTA</mat-checkbox></div>
                    <div class="chk"><mat-checkbox [(ngModel)]="antFam.diabetes">Diabetes</mat-checkbox></div>
                    <div class="chk"><mat-checkbox [(ngModel)]="antFam.coronarias">Coronarias</mat-checkbox></div>
                    <div class="chk"><mat-checkbox [(ngModel)]="antFam.cancer">Cáncer</mat-checkbox></div>
                    <div class="chk"><mat-checkbox [(ngModel)]="antFam.acv">ACV</mat-checkbox></div>
                    <div class="chk"><mat-checkbox [(ngModel)]="antFam.tbc">TBC</mat-checkbox></div>
                    <div class="chk"><mat-checkbox [(ngModel)]="antFam.asma">Asma</mat-checkbox></div>
                  </div>
                  <mat-form-field appearance="outline" style="margin-top:0.75rem">
                        <mat-label>Otros</mat-label>
                        <input matInput [(ngModel)]="antFam.otros" class="w-full" placeholder="Describa otros antecedentes familiares..." />
                      </mat-form-field>
                </mat-expansion-panel>

                <mat-expansion-panel expanded class="ant-panel">
                  <mat-expansion-panel-header>
                    <mat-panel-title>Antecedentes personales</mat-panel-title>
                  </mat-expansion-panel-header>
                  <div class="check-grid">
                    <div class="chk"><mat-checkbox [(ngModel)]="antPers.hta">HTA</mat-checkbox></div>
                    <div class="chk"><mat-checkbox [(ngModel)]="antPers.diabetes">Diabetes</mat-checkbox></div>
                    <div class="chk"><mat-checkbox [(ngModel)]="antPers.coronarias">Coronarias</mat-checkbox></div>
                    <div class="chk"><mat-checkbox [(ngModel)]="antPers.acv">ACV</mat-checkbox></div>
                    <div class="chk"><mat-checkbox [(ngModel)]="antPers.tbc">TBC</mat-checkbox></div>
                    <div class="chk"><mat-checkbox [(ngModel)]="antPers.asma">Asma</mat-checkbox></div>
                  </div>
                  <mat-divider />
                  <div class="cond-row">
                    <div class="chk"><mat-checkbox [(ngModel)]="antPers.cancer_activo">Cáncer</mat-checkbox></div>
                    @if (antPers.cancer_activo) {
                      <input [(ngModel)]="antPers.cancer_desc" class="cond-input" placeholder="Tipo / descripción..." />
                    }
                  </div>
                  <div class="cond-row">
                    <div class="chk"><mat-checkbox [(ngModel)]="antPers.quirurgicos_activo">Quirúrgicos</mat-checkbox></div>
                    @if (antPers.quirurgicos_activo) {
                      <input [(ngModel)]="antPers.quirurgicos_desc" class="cond-input" placeholder="Cirugías / descripción..." />
                    }
                  </div>
                  <div class="cond-row">
                    <div class="chk"><mat-checkbox [(ngModel)]="antPers.its_activo">ITS</mat-checkbox></div>
                    @if (antPers.its_activo) {
                      <input [(ngModel)]="antPers.its_desc" class="cond-input" placeholder="Descripción..." />
                    }
                  </div>
                  <mat-divider />
                  <div class="form-grid">
                    <mat-form-field appearance="outline">
                        <mat-label>Otros antecedentes</mat-label>
                        <input matInput [(ngModel)]="antPers.otros" class="w-full" placeholder="Alergias, farmacológicos, tóxicos..." />
                      </mat-form-field>
                    <mat-form-field appearance="outline">
                        <mat-label>Hábitos</mat-label>
                        <input matInput [(ngModel)]="antPers.habitos" class="w-full" placeholder="Tabaco, alcohol, ejercicio..." />
                      </mat-form-field>
                  </div>
                </mat-expansion-panel>

                @if (esMujer()) {
                  <mat-expansion-panel expanded class="ant-panel">
                  <mat-expansion-panel-header>
                    <mat-panel-title>Antecedentes ginecobstétricos</mat-panel-title>
                  </mat-expansion-panel-header>
                    <div class="form-grid">
                      <mat-form-field appearance="outline">
                        <mat-label>Menarquia (edad)</mat-label>
                        <input matInput [(ngModel)]="antGineco.menarquia" type="number" min="0" max="25" />
                      </mat-form-field>
                      <mat-form-field appearance="outline">
                        <mat-label>FUM</mat-label>
                        <input matInput type="date" [(ngModel)]="antGineco.fum" class="w-full" />
                      </mat-form-field>
                    </div>
                    <div class="check-grid" style="margin-bottom:0.75rem">
                      <div class="chk"><mat-checkbox [(ngModel)]="antGineco.ciclos_regulares">Ciclos regulares</mat-checkbox></div>
                      <div class="chk"><mat-checkbox [(ngModel)]="antGineco.dismenorrea">Dismenorrea</mat-checkbox></div>
                      <div class="chk"><mat-checkbox [(ngModel)]="antGineco.metrorragia">Metrorragia</mat-checkbox></div>
                      <div class="chk"><mat-checkbox [(ngModel)]="antGineco.citologia_vigente">Citología vigente</mat-checkbox></div>
                      <div class="chk"><mat-checkbox [(ngModel)]="antGineco.lactancia_actual">Lactancia actual</mat-checkbox></div>
                    </div>
                    <div class="cond-row">
                      <div class="chk"><mat-checkbox [(ngModel)]="antGineco.planificacion">Planificación familiar</mat-checkbox></div>
                      @if (antGineco.planificacion) {
                        <input [(ngModel)]="antGineco.metodo_planificacion" class="cond-input" placeholder="Método..." />
                      }
                    </div>
                    @if (antGineco.citologia_vigente) {
                      <mat-form-field appearance="outline" style="margin-top:0.5rem">
                        <mat-label>Fecha citología</mat-label>
                        <input matInput type="date" [(ngModel)]="antGineco.fecha_citologia" style="width:200px" />
                      </mat-form-field>
                    }
                    <div class="cond-row" style="margin-top:0.5rem">
                      <div class="chk"><mat-checkbox [(ngModel)]="antGineco.embarazo_actual">Embarazo actual</mat-checkbox></div>
                      @if (antGineco.embarazo_actual) {
                        <input [(ngModel)]="antGineco.semanas_gestacion" type="number" min="1" max="42"
                          placeholder="Semanas" class="cond-input" />
                      }
                    </div>
                    <mat-divider />
                    <div class="obstetrico-grid">
                      <mat-form-field appearance="outline">
                        <mat-label>G (Gestas)</mat-label>
                        <input matInput [(ngModel)]="antGineco.gestas" type="number" min="0" />
                      </mat-form-field>
                      <mat-form-field appearance="outline">
                        <mat-label>P (Partos)</mat-label>
                        <input matInput [(ngModel)]="antGineco.partos" type="number" min="0" />
                      </mat-form-field>
                      <mat-form-field appearance="outline">
                        <mat-label>C (Cesáreas)</mat-label>
                        <input matInput [(ngModel)]="antGineco.cesareas" type="number" min="0" />
                      </mat-form-field>
                      <mat-form-field appearance="outline">
                        <mat-label>A (Abortos)</mat-label>
                        <input matInput [(ngModel)]="antGineco.abortos" type="number" min="0" />
                      </mat-form-field>
                      <mat-form-field appearance="outline">
                        <mat-label>Hijos vivos</mat-label>
                        <input matInput [(ngModel)]="antGineco.hijos_vivos" type="number" min="0" />
                      </mat-form-field>
                    </div>
                    <mat-form-field appearance="outline" style="margin-top:0.75rem">
                        <mat-label>Observaciones</mat-label>
                        <textarea matInput [(ngModel)]="antGineco.obs_adicionales" rows="2" class="w-full"></textarea>
                      </mat-form-field>
                  </mat-expansion-panel>

                  <mat-expansion-panel expanded class="ant-panel">
                    <mat-expansion-panel-header>
                      <mat-panel-title>Antecedentes prenatales</mat-panel-title>
                    </mat-expansion-panel-header>
                    <div class="check-grid" style="margin-bottom:0.75rem">
                      <div class="chk"><mat-checkbox [(ngModel)]="antPrenatales.embarazo_a_termino">Embarazo a término</mat-checkbox></div>
                      <div class="chk"><mat-checkbox [(ngModel)]="antPrenatales.parto_normal">Parto normal</mat-checkbox></div>
                      <div class="chk"><mat-checkbox [(ngModel)]="antPrenatales.parto_cesarea">Cesárea</mat-checkbox></div>
                      <div class="chk"><mat-checkbox [(ngModel)]="antPrenatales.parto_forceps">Fórceps</mat-checkbox></div>
                      <div class="chk"><mat-checkbox [(ngModel)]="antPrenatales.lactancia_materna">Lactancia materna</mat-checkbox></div>
                    </div>
                    <div class="form-grid">
                      <mat-form-field appearance="outline">
                        <mat-label>Semanas de gestación al nacer</mat-label>
                        <input matInput [(ngModel)]="antPrenatales.semanas_gestacion_nacer" type="number" min="20" max="44" />
                      </mat-form-field>
                      <mat-form-field appearance="outline">
                        <mat-label>Peso al nacer (kg)</mat-label>
                        <input matInput [(ngModel)]="antPrenatales.peso_nacer" type="number" step="any" />
                      </mat-form-field>
                      <mat-form-field appearance="outline">
                        <mat-label>Talla al nacer (cm)</mat-label>
                        <input matInput [(ngModel)]="antPrenatales.talla_nacer" type="number" step="any" />
                      </mat-form-field>
                      <mat-form-field appearance="outline">
                        <mat-label>APGAR al minuto</mat-label>
                        <input matInput [(ngModel)]="antPrenatales.apgar_1min" type="number" min="0" max="10" />
                      </mat-form-field>
                      <mat-form-field appearance="outline">
                        <mat-label>APGAR 5 minutos</mat-label>
                        <input matInput [(ngModel)]="antPrenatales.apgar_5min" type="number" min="0" max="10" />
                      </mat-form-field>
                    </div>
                    <mat-form-field appearance="outline" style="margin-top:0.5rem">
                        <mat-label>Complicaciones perinatales</mat-label>
                        <textarea matInput [(ngModel)]="antPrenatales.complicaciones" rows="2" class="w-full" placeholder="Describa si aplica..."></textarea>
                      </mat-form-field>
                  </mat-expansion-panel>
                }

                <div class="step-actions">
                  <button matButton="filled" (click)="irAPaso(2)">Siguiente <i class="pi pi-chevron-right"></i></button>
                </div>
            </mat-step>

            <!-- PASO 2: EXAMEN FÍSICO (Signos Vitales + Examen por Sistemas) -->
            <mat-step label="Examen Físico">
                <mat-tab-group>
                    <!-- TAB 1: SIGNOS VITALES -->
                    <mat-tab label="Signos Vitales">
                      <div class="sv-grid">

                        <mat-form-field appearance="outline">
                        <mat-label>Perímetro abdominal (cm)</mat-label>
                        <input matInput [(ngModel)]="form.pabd" type="number" step="any" />
                      </mat-form-field>
                        <mat-form-field appearance="outline">
                        <mat-label>Presión arterial media</mat-label>
                        <input matInput [(ngModel)]="form.pam" class="w-full" placeholder="pam (mmHg)" />
                      </mat-form-field>
                        <mat-form-field appearance="outline">
                        <mat-label>Temperatura corporal (°C)</mat-label>
                        <input matInput [(ngModel)]="form.temperatura" type="number" step="any" />
                      </mat-form-field>
                        <mat-form-field appearance="outline">
                        <mat-label>Pulso / min</mat-label>
                        <input matInput [(ngModel)]="form.pulso" type="number" min="0" />
                      </mat-form-field>
                        <mat-form-field appearance="outline">
                        <mat-label>Frec. respiratoria</mat-label>
                        <input matInput [(ngModel)]="form.frecuencia_respiratoria" type="number" min="0" />
                      </mat-form-field>
                        <mat-form-field appearance="outline">
                        <mat-label>Frec. cardíaca / min</mat-label>
                        <input matInput [(ngModel)]="form.frecuencia_cardiaca" type="number" min="0" />
                      </mat-form-field>
                        <mat-form-field appearance="outline">
                        <mat-label>Perímetro cefálico (cm)</mat-label>
                        <input matInput [(ngModel)]="form.pc" type="number" step="any" />
                      </mat-form-field>
                        <mat-form-field appearance="outline">
                        <mat-label>Peso (kg)</mat-label>
                        <input matInput [(ngModel)]="form.peso" type="number" step="any" />
                      </mat-form-field>
                        <mat-form-field appearance="outline">
                        <mat-label>Talla (m)</mat-label>
                        <input matInput [(ngModel)]="tallaMts" type="number" min="0" max="3" step="any" />
                      </mat-form-field>

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
                              <x-etiqueta [value]="imcLabel()" [severity]="imcSeverity()" style="margin-left:0.5rem" />
                            } @else { — }
                          </div>
                        </div>
                        <div class="field sv-calc" style="grid-column:1/-1">
                          <label>Estado nutricional <span class="calc-badge">calculo</span></label>
                          <div class="calc-value">{{ form.estado_nutricional || '—' }}</div>
                        </div>

                      </div>
                    </mat-tab>

                    <!-- TAB 2: EXAMEN POR SISTEMAS -->
                    <mat-tab label="Examen por Sistemas">
                      <p style="font-size:0.82rem;color:#94a3b8;margin:0 0 1rem">
                        Marque los sistemas con hallazgos anormales e ingrese la descripción. Los no marcados se guardan como <strong>Normal</strong>.
                      </p>
                      <div class="ef-sistemas-grid">
                        @for (item of efItems; track item.key) {
                          <div class="ef-row">
                            <div class="ef-check-label">
                              <mat-checkbox [(ngModel)]="efState[item.key].anormal"
                                (ngModelChange)="onEfCheck(item.key)"
                                [class.ef-anormal]="efState[item.key].anormal">{{ item.label }}</mat-checkbox>
                            </div>
                            @if (efState[item.key].anormal) {
                              <input [(ngModel)]="efState[item.key].texto" class="ef-text-input"
                                [placeholder]="'Hallazgo en ' + item.label.toLowerCase() + '...'" />
                            } @else {
                              <span class="ef-normal-badge">Normal</span>
                            }
                          </div>
                        }
                      </div>
                      <mat-form-field appearance="outline" style="margin-top:1.25rem">
                        <mat-label>Hallazgos generales</mat-label>
                        <textarea matInput [(ngModel)]="form.ef_hallazgos" rows="4" class="w-full"
                          placeholder="Observaciones generales del examen físico..."></textarea>
                      </mat-form-field>
                    </mat-tab>
                </mat-tab-group>

                <div class="step-actions">
                  <button matButton (click)="irAPaso(1)"><i class="pi pi-chevron-left"></i> Anterior</button>
                  <button matButton="filled" (click)="onNextExamen()">Siguiente <i class="pi pi-chevron-right"></i></button>
                </div>
            </mat-step>

            <!-- PASO 3: DIAGNÓSTICO CIE-10 -->
            <mat-step label="Diagnóstico">
                <div class="form-grid">
                  <mat-form-field appearance="outline">
                        <mat-label>Código CIE-10 principal</mat-label>
                        <input matInput [(ngModel)]="form.diagnostico_principal" class="w-full" placeholder="Ej: J06.9" />
                      </mat-form-field>
                  <mat-form-field appearance="outline">
                        <mat-label>Código CIE-10 relacionado</mat-label>
                        <input matInput [(ngModel)]="form.diagnostico_relacionado1" class="w-full" placeholder="Código adicional" />
                      </mat-form-field>
                  <!--
                    Finalidad y causa: los dos van al RIPS y hasta el 2026-08-02
                    NO se podían elegir. El sistema los rellenaba —«10» y «13»—
                    con códigos que además eran de la tabla anterior a la
                    Resolución 948 y que el validador rechaza. Sin este campo, el
                    dato lo afirmaba el sistema en nombre del médico.
                    Van sin valor inicial a propósito: vacío es «falta», y el
                    generador del RIPS lo pide al cerrar el mes.
                  -->
                  <mat-form-field appearance="outline">
                    <mat-label>Finalidad de la atención</mat-label>
                    <mat-select [(ngModel)]="form.finalidad"
                      placeholder="Para qué se atendió (Resolución 948)">
                      <!-- Sin opción por defecto y a propósito: hasta el
                           2026-08-02 este campo salía con un código de la tabla
                           ANTERIOR a la 948, puesto por el sistema en nombre del
                           médico. Vacío es «nadie lo dijo», que es la verdad. -->
                      <mat-option [value]="null">— Sin elegir —</mat-option>
                      @for (f of finalidades(); track f.codigo) {
                        <mat-option [value]="f.codigo">{{ f.nombre }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>
                  <mat-form-field appearance="outline">
                    <mat-label>Causa o motivo de la atención</mat-label>
                    <mat-select [(ngModel)]="form.causa_externa"
                      placeholder="Por qué se atendió (Resolución 948)">
                      <mat-option [value]="null">— Sin elegir —</mat-option>
                      @for (c of causasMotivo(); track c.codigo) {
                        <mat-option [value]="c.codigo">{{ c.nombre }}</mat-option>
                      }
                    </mat-select>
                  </mat-form-field>
                </div>
                <div class="step-actions">
                  <button matButton (click)="irAPaso(2)"><i class="pi pi-chevron-left"></i> Anterior</button>
                  <button matButton="filled" (click)="irAPaso(4)">Siguiente <i class="pi pi-chevron-right"></i></button>
                </div>
            </mat-step>

            <!-- PASO 4: PLAN -->
            <mat-step label="Plan">
                <mat-form-field appearance="outline" class="ancho">
                  <mat-label>Plan de tratamiento</mat-label>
                  <textarea matInput [(ngModel)]="form.plan_tratamiento" rows="8"
                    placeholder="Plan de manejo y tratamiento..."></textarea>
                </mat-form-field>
                <div class="step-actions">
                  <button matButton (click)="irAPaso(3)"><i class="pi pi-chevron-left"></i> Anterior</button>
                  <button matButton="filled" [disabled]="saving()" (click)="guardarYContinuar()"><i [class]="saving() ? 'pi pi-spin pi-spinner' : 'pi pi-save'"></i> Guardar y continuar</button>
                </div>
            </mat-step>

            <!-- PASO 5: FÓRMULA MÉDICA -->
            <mat-step label="Fórmula Médica">

                @if (!historia()?.id) {
                  <div class="formula-aviso">
                    <i class="pi pi-info-circle"></i>
                    Guarda primero la historia clínica para poder agregar fórmulas médicas.
                  </div>
                } @else {

                  <!-- Botón nueva fórmula -->
                  @if (!mostrarFormFormula()) {
                    <div style="margin-bottom:1rem">
                      <button matButton="filled" (click)="abrirFormFormula()"><i class="pi pi-plus"></i> Agregar medicamento</button>
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
                          <mat-form-field appearance="outline" subscriptSizing="dynamic" class="ancho">
                            <mat-label>Medicamento</mat-label>
                            <input matInput [(ngModel)]="termMed"
                              (ngModelChange)="onTermMedChange($event)"
                              placeholder="Nombre o código CUM..."
                              autocomplete="off" />
                            @if (buscandoMed()) {
                              <i class="pi pi-spin pi-spinner" matSuffix></i>
                            } @else {
                              <i class="pi pi-search" matSuffix></i>
                            }
                          </mat-form-field>
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
                        <mat-form-field appearance="outline" style="grid-column:1/-1">
                        <mat-label>Medicamento *</mat-label>
                        <input matInput [(ngModel)]="fForm.medicamento" class="w-full"
                            placeholder="Nombre del medicamento (manual o desde búsqueda)" />
                      </mat-form-field>
                        <mat-form-field appearance="outline">
                        <mat-label>Código CUM</mat-label>
                        <input matInput [(ngModel)]="fForm.codigo_cum" class="w-full"
                            placeholder="Auto desde búsqueda o manual" />
                      </mat-form-field>
                        <mat-form-field appearance="outline">
                        <mat-label>Dosis *</mat-label>
                        <input matInput [(ngModel)]="fForm.dosis" class="w-full"
                            placeholder="Ej: 500mg cada 8h por 7 días" />
                      </mat-form-field>
                        <mat-form-field appearance="outline">
                        <mat-label>Cantidad *</mat-label>
                        <input matInput [(ngModel)]="fForm.cantidad" type="number" min="1" />
                      </mat-form-field>
                        <div class="campo-con-nota">
                          <mat-form-field appearance="outline">
                            <mat-label>Unidad *</mat-label>
                            <mat-select [(ngModel)]="fForm.unidad" placeholder="Elija la presentación">
                              @for (u of unidades(); track u.nombre) {
                                <mat-option [value]="u.nombre">{{ u.nombre }}</mat-option>
                              }
                            </mat-select>
                          </mat-form-field>
                          <small class="text-color-secondary">
                            El RIPS la pide como código. Si la presentación no
                            está en la lista, queda anotada al cerrar el mes.
                          </small>
                        </div>
                        <mat-form-field appearance="outline">
                        <mat-label>Días de tratamiento *</mat-label>
                        <input matInput [(ngModel)]="fForm.dias_tratamiento" type="number" min="1" />
                      </mat-form-field>
                        <mat-form-field appearance="outline" style="grid-column:1/-1">
                        <mat-label>Observaciones</mat-label>
                        <textarea matInput [(ngModel)]="fForm.observaciones" rows="2" class="w-full"
                            placeholder="Indicaciones adicionales para el paciente o farmacia..."></textarea>
                      </mat-form-field>
                      </div>

                      <div class="step-actions">
                        <button matButton (click)="cerrarFormFormula()">Cancelar</button>
                        <button matIconButton [disabled]="savingFormula()" (click)="guardarFormula()"><i [class]="savingFormula() ? 'pi pi-spin pi-spinner' : 'pi pi-check'"></i></button>
                      </div>
                    </div>
                  }

                  <!-- Tabla de fórmulas existentes -->
                  @if (formulas().length > 0) {
                    <table mat-table [dataSource]="formulas()" class="tabla" style="margin-top:1rem">
                      <ng-container matColumnDef="medicamento">
                        <th mat-header-cell *matHeaderCellDef>Medicamento</th>
                        <td mat-cell *matCellDef="let f">
                          <strong>{{ f.medicamento }}</strong>
                          @if (f.codigo_cum) {
                            <div style="font-size:0.75rem;color:#94a3b8">CUM: {{ f.codigo_cum }}</div>
                          }
                        </td>
                      </ng-container>
                      <ng-container matColumnDef="dosis">
                        <th mat-header-cell *matHeaderCellDef>Dosis</th>
                        <td mat-cell *matCellDef="let f">{{ f.dosis }}</td>
                      </ng-container>
                      <ng-container matColumnDef="cantidad">
                        <th mat-header-cell *matHeaderCellDef class="centro">Cantidad</th>
                        <td mat-cell *matCellDef="let f" class="centro">{{ f.cantidad }}</td>
                      </ng-container>
                      <ng-container matColumnDef="unidad">
                        <th mat-header-cell *matHeaderCellDef>Unidad</th>
                        <td mat-cell *matCellDef="let f">{{ f.unidad }}</td>
                      </ng-container>
                      <ng-container matColumnDef="observaciones">
                        <th mat-header-cell *matHeaderCellDef>Observaciones</th>
                        <td mat-cell *matCellDef="let f" class="col-truncate">{{ f.observaciones }}</td>
                      </ng-container>
                      <ng-container matColumnDef="estado">
                        <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
                        <td mat-cell *matCellDef="let f" class="centro">
                          @if (f.dispensada) {
                            <x-etiqueta value="Dispensada" severity="success" icon="pi pi-check" />
                          } @else {
                            <x-etiqueta value="Pendiente" severity="warn" icon="pi pi-clock" />
                          }
                        </td>
                      </ng-container>
                      <ng-container matColumnDef="acciones">
                        <th mat-header-cell *matHeaderCellDef class="centro">Acciones</th>
                        <td mat-cell *matCellDef="let f" class="centro">
                          @if (!f.dispensada && !historia()?.firmada) {
                            <div style="display:flex;gap:0.4rem;justify-content:center">
                              <button matIconButton matTooltip="Editar" (click)="editarFormula(f)">
                                <i class="pi pi-pencil"></i>
                              </button>
                              <button matIconButton class="peligro" matTooltip="Eliminar"
                                (click)="confirmarEliminarFormula(f)">
                                <i class="pi pi-trash"></i>
                              </button>
                            </div>
                          }
                        </td>
                      </ng-container>

                      <tr mat-header-row *matHeaderRowDef="columnasFormulas"></tr>
                      <tr mat-row *matRowDef="let fila; columns: columnasFormulas"></tr>
                    </table>
                  } @else if (!mostrarFormFormula()) {
                    <div class="sin-resultados" style="padding:2rem">
                      <i class="pi pi-file-o" style="font-size:1.5rem;opacity:0.4"></i>
                      <p style="margin:0">No hay medicamentos en esta fórmula médica.</p>
                    </div>
                  }

                }

                <div class="step-actions" style="margin-top:1.5rem">
                  <button matButton (click)="irAPaso(4)"><i class="pi pi-chevron-left"></i> Anterior</button>
                  @if (historia()?.id) {
                    <button matButton="filled" (click)="irAPaso(6)">Siguiente <i class="pi pi-chevron-right"></i></button>
                  }
                </div>
            </mat-step>

            <!-- PASO 6: ÓRDENES MÉDICAS -->
            <mat-step label="Órdenes">

                @if (!historia()?.id) {
                  <div class="formula-aviso">
                    <i class="pi pi-info-circle"></i>
                    Guarda primero la historia clínica para poder agregar órdenes médicas.
                  </div>
                } @else {

                  @if (!mostrarFormOrden()) {
                    <div style="margin-bottom:1rem">
                      <button matButton="filled" (click)="abrirFormOrden()"><i class="pi pi-plus"></i> Agregar orden</button>
                    </div>
                  }

                  @if (mostrarFormOrden()) {
                    <div class="formula-form">
                      <h4 style="margin:0 0 1rem;color:#f1f5f9">Nueva orden médica</h4>
                      <div class="form-grid">
                        <mat-form-field appearance="outline">
                          <mat-label>Tipo de orden *</mat-label>
                          <mat-select [(ngModel)]="oForm.tipo">
                            @for (t of tiposOrden; track t.value) {
                              <mat-option [value]="t.value">{{ t.label }}</mat-option>
                            }
                          </mat-select>
                        </mat-form-field>
                        <mat-form-field appearance="outline">
                        <mat-label>Código CUPS</mat-label>
                        <input matInput [(ngModel)]="oForm.codigo_cups" class="w-full" placeholder="Opcional" />
                      </mat-form-field>
                        <mat-form-field appearance="outline" style="grid-column:1/-1">
                        <mat-label>Descripción *</mat-label>
                        <input matInput [(ngModel)]="oForm.descripcion" class="w-full"
                            placeholder="Ej: Hemograma completo, Rayos X de tórax..." />
                      </mat-form-field>
                        <mat-form-field appearance="outline" style="grid-column:1/-1">
                        <mat-label>Justificación clínica *</mat-label>
                        <textarea matInput [(ngModel)]="oForm.justificacion" rows="3" class="w-full"
                            placeholder="Motivo clínico de la orden..."></textarea>
                      </mat-form-field>
                        @if (oForm.tipo === 'Remisión Especialista') {
                          <mat-form-field appearance="outline" style="grid-column:1/-1">
                            <mat-label>Especialidad destino</mat-label>
                            <mat-select [(ngModel)]="oForm.especialidad_destino_id">
                              @for (e of especialidades(); track e.id) {
                                <mat-option [value]="e.id">{{ e.nombre }}</mat-option>
                              }
                            </mat-select>
                          </mat-form-field>
                        }
                      </div>
                      <div class="step-actions">
                        <button matButton (click)="cerrarFormOrden()">Cancelar</button>
                        <button matButton="filled" [disabled]="savingOrden()" (click)="guardarOrden()"><i [class]="savingOrden() ? 'pi pi-spin pi-spinner' : 'pi pi-check'"></i> Agregar</button>
                      </div>
                    </div>
                  }

                  @if (ordenes().length > 0) {
                    <table mat-table [dataSource]="ordenes()" class="tabla" style="margin-top:1rem">
                      <ng-container matColumnDef="tipo">
                        <th mat-header-cell *matHeaderCellDef>Tipo</th>
                        <td mat-cell *matCellDef="let o">
                          <x-etiqueta [value]="o.tipo" [severity]="ordenSeverity(o.tipo)" />
                        </td>
                      </ng-container>
                      <ng-container matColumnDef="descripcion">
                        <th mat-header-cell *matHeaderCellDef>Descripción</th>
                        <td mat-cell *matCellDef="let o"><strong>{{ o.descripcion }}</strong></td>
                      </ng-container>
                      <ng-container matColumnDef="destino">
                        <th mat-header-cell *matHeaderCellDef>Especialidad / CUPS</th>
                        <td mat-cell *matCellDef="let o" class="col-truncate">
                          @if (o.especialidad_nombre) { {{ o.especialidad_nombre }} }
                          @else if (o.codigo_cups) { CUPS: {{ o.codigo_cups }} }
                          @else { — }
                        </td>
                      </ng-container>
                      <ng-container matColumnDef="justificacion">
                        <th mat-header-cell *matHeaderCellDef>Justificación</th>
                        <td mat-cell *matCellDef="let o" class="col-truncate">{{ o.justificacion }}</td>
                      </ng-container>
                      <ng-container matColumnDef="estado">
                        <th mat-header-cell *matHeaderCellDef class="centro">Estado</th>
                        <td mat-cell *matCellDef="let o" class="centro">
                          <x-etiqueta [value]="o.estado" [severity]="estadoOrdenSeverity(o.estado)" />
                        </td>
                      </ng-container>
                      <ng-container matColumnDef="acciones">
                        <th mat-header-cell *matHeaderCellDef class="centro">Acciones</th>
                        <td mat-cell *matCellDef="let o" class="centro">
                          @if (!historia()?.firmada) {
                            <button matIconButton class="peligro" matTooltip="Eliminar"
                              (click)="confirmarEliminarOrden(o)">
                              <i class="pi pi-trash"></i>
                            </button>
                          }
                        </td>
                      </ng-container>

                      <tr mat-header-row *matHeaderRowDef="columnasOrdenes"></tr>
                      <tr mat-row *matRowDef="let fila; columns: columnasOrdenes"></tr>
                    </table>
                  } @else if (!mostrarFormOrden()) {
                    <div class="sin-resultados" style="padding:2rem">
                      <i class="pi pi-clipboard" style="font-size:1.5rem;opacity:0.4"></i>
                      <p style="margin:0">No hay órdenes médicas en esta historia.</p>
                    </div>
                  }

                }

                <div class="step-actions" style="margin-top:1.5rem">
                  <button matButton (click)="irAPaso(5)"><i class="pi pi-chevron-left"></i> Anterior</button>
                  @if (historia()?.id) {
                    <button matButton="filled" (click)="irAPaso(7)">Siguiente <i class="pi pi-chevron-right"></i></button>
                  }
                </div>
            </mat-step>

            <!-- PASO 7: IMÁGENES DIAGNÓSTICAS -->
            <mat-step label="Imágenes">

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
                          <button matButton (click)="fileInput.click()"><i class="pi pi-upload"></i> Seleccionar archivo</button>
                          @if (imagenArchivoSeleccionado) {
                            <span style="font-size:0.85rem;color:#94a3b8">{{ imagenArchivoSeleccionado.name }}</span>
                          }
                        </div>
                      </div>
                      <mat-form-field appearance="outline">
                        <mat-label>Descripción</mat-label>
                        <input matInput [(ngModel)]="imagenDescripcion" class="w-full"
                          placeholder="Ej: Rx tórax PA, ECG, foto lesión..." />
                      </mat-form-field>
                    </div>
                    <button matButton="filled" [disabled]="!imagenArchivoSeleccionado || uploadingImagen()" (click)="subirImagen()"><i [class]="uploadingImagen() ? 'pi pi-spin pi-spinner' : 'pi pi-cloud-upload'"></i> Subir imagen</button>
                  </div>

                  @if (imagenes().length > 0) {
                    <table mat-table [dataSource]="imagenes()" class="tabla" style="margin-top:1.5rem">
                      <ng-container matColumnDef="archivo">
                        <th mat-header-cell *matHeaderCellDef>Archivo</th>
                        <td mat-cell *matCellDef="let img">
                          <a [href]="urlImagen(img.id)" target="_blank" style="color:#818cf8">
                            <i class="pi pi-file" style="margin-right:0.4rem"></i>{{ img.nombre_original }}
                          </a>
                        </td>
                      </ng-container>
                      <ng-container matColumnDef="descripcion">
                        <th mat-header-cell *matHeaderCellDef>Descripción</th>
                        <td mat-cell *matCellDef="let img">{{ img.descripcion }}</td>
                      </ng-container>
                      <ng-container matColumnDef="fecha">
                        <th mat-header-cell *matHeaderCellDef>Fecha</th>
                        <td mat-cell *matCellDef="let img">{{ img.fecha_subida | date:'dd/MM/yyyy HH:mm' }}</td>
                      </ng-container>
                      <ng-container matColumnDef="acciones">
                        <th mat-header-cell *matHeaderCellDef class="centro">Acciones</th>
                        <td mat-cell *matCellDef="let img" class="centro">
                          @if (!historia()?.firmada) {
                            <button matIconButton class="peligro" matTooltip="Eliminar"
                              (click)="confirmarEliminarImagen(img)">
                              <i class="pi pi-trash"></i>
                            </button>
                          }
                        </td>
                      </ng-container>

                      <tr mat-header-row *matHeaderRowDef="columnasImagenes"></tr>
                      <tr mat-row *matRowDef="let fila; columns: columnasImagenes"></tr>
                    </table>
                  } @else {
                    <div class="sin-resultados" style="padding:2rem">
                      <i class="pi pi-images" style="font-size:1.5rem;opacity:0.4"></i>
                      <p style="margin:0">No hay imágenes adjuntas en esta historia.</p>
                    </div>
                  }

                }

                <div class="step-actions" style="margin-top:1.5rem">
                  <button matButton (click)="irAPaso(6)"><i class="pi pi-chevron-left"></i> Anterior</button>
                  @if (historia()?.id) {
                    <button matButton="filled" [disabled]="!!historia()?.firmada || signing()" (click)="firmarHistoria()"><i [class]="signing() ? 'pi pi-spin pi-spinner' : 'pi pi-lock'"></i> Firmar Historia</button>
                  }
                </div>
            </mat-step>

        </mat-stepper>
      }

    </div>
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
  private messageService = inject(Avisos);
  private confirmationService = inject(Confirmar);

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

  /** El orden de las columnas lo fija esta lista, no la plantilla. */
  readonly columnasHistorias = ['codigo', 'fecha', 'motivo', 'diagnostico', 'medico',
                                'estado', 'acciones'];
  readonly columnasFormulas = ['medicamento', 'dosis', 'cantidad', 'unidad',
                               'observaciones', 'estado', 'acciones'];
  readonly columnasOrdenes = ['tipo', 'descripcion', 'destino', 'justificacion',
                              'estado', 'acciones'];
  readonly columnasImagenes = ['archivo', 'descripcion', 'fecha', 'acciones'];

  /** Mover el paso activo. La señal es la única fuente: la cabecera del stepper
   *  la escribe por (selectionChange) y la lee por [selectedIndex]. */
  irAPaso(n: number): void { this.activeStep.set(n); }
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
      peligroso: true,
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

  onNextExamen(): void {
    // Aplica el estado de checkboxes al form antes de avanzar
    for (const item of this.efItems) {
      const s = this.efState[item.key];
      (this.form as any)[item.key] = s?.anormal ? (s.texto.trim() || 'Anormal') : 'Normal';
    }
    this.irAPaso(3);
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

  guardarYContinuar(): void {
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
            this.irAPaso(5);
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
      peligroso: true,
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
      peligroso: true,
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
      peligroso: true,
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
