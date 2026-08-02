/**
 * El alta de pacientes: la puerta de entrada de todo lo demás.
 *
 * Sin paciente no hay historia, sin historia no hay RIPS y sin RIPS no hay
 * pago. Hasta ahora el botón "Nuevo Paciente" no tenía manejador, así que un
 * paciente solo entraba por el script de semillas.
 */
import { ComponentFixture, TestBed } from '@angular/core/testing';
import { provideHttpClient, withXhr } from '@angular/common/http';
import { provideHttpClientTesting } from '@angular/common/http/testing';
import { MAT_DIALOG_DATA, MatDialogRef } from '@angular/material/dialog';
import { provideNoopAnimations } from '@angular/platform-browser/animations';
import {
  PacienteFormComponent,
  faltantes,
  hoyISO,
  pacienteVacio,
} from './paciente-form.component';
import { Patient, PatientCreate } from '../../models/patient.models';

function completo(): PatientCreate {
  return {
    ...pacienteVacio(),
    identificacion: '1017234567',
    nombre1: 'Ana',
    apellido1: 'Restrepo',
    fecha_nacimiento: '1990-03-14',
    celular: '3001112233',
    cod_municipio: '05001',
  };
}

describe('qué falta para poder guardar', () => {
  it('un paciente completo no tiene faltantes', () => {
    expect(faltantes(completo())).toEqual([]);
  });

  it('exige lo que el RIPS necesita sí o sí', () => {
    const vacio = pacienteVacio();
    const fallos = faltantes(vacio).join(' ');

    for (const exigido of [
      'documento', 'primer nombre', 'primer apellido',
      'fecha de nacimiento', 'celular', 'municipio',
    ]) {
      expect(fallos).toContain(exigido);
    }
  });

  it('no exige lo que de verdad es opcional', () => {
    // Un consultorio pequeño no siempre tiene correo ni segundo apellido, y no
    // por eso puede quedarse sin registrar la atención.
    const p = completo();
    delete p.email;
    delete p.nombre2;
    delete p.apellido2;
    delete p.direccion;
    delete p.entidad_id;
    expect(faltantes(p)).toEqual([]);
  });

  it('un campo con solo espacios cuenta como vacío', () => {
    expect(faltantes({ ...completo(), nombre1: '   ' })).toContain('el primer nombre');
  });

  it('rechaza una fecha de nacimiento en el futuro', () => {
    // Un año mal digitado (2026 por 1926) descuadraría en silencio la
    // clasificación por edad de todos los informes epidemiológicos.
    const futuro = new Date();
    futuro.setFullYear(futuro.getFullYear() + 1);
    const p = { ...completo(), fecha_nacimiento: futuro.toISOString().slice(0, 10) };

    expect(faltantes(p).join(' ')).toContain('futuro');
  });

  it('acepta a alguien nacido hoy', () => {
    // Un recién nacido se registra el mismo día. El límite es "no futuro", no
    // "hace al menos un día".
    expect(faltantes({ ...completo(), fecha_nacimiento: hoyISO() })).toEqual([]);
  });

  it('enumera TODO lo que falta, no solo lo primero', () => {
    const p = { ...completo(), nombre1: '', celular: '' };
    expect(faltantes(p).length).toBe(2);
  });
});

describe('valores por defecto del paciente nuevo', () => {
  it('trae puesto lo que casi nunca cambia', () => {
    const p = pacienteVacio();
    expect(p.tipo_id).toBe('CC');
    expect(p.cod_pais).toBe('170');
    expect(p.zona).toBe('01');
    expect(p.tipo_usuario).toBe('01');
  });

  it('NO trae municipio: el que había era Medellín para todo el país', () => {
    // El modelo del backend tiene "05001" por defecto, y eso reportaba a todos
    // los pacientes como residentes de Medellín. Aquí se deja vacío para que
    // `faltantes()` obligue a escribirlo.
    expect(pacienteVacio().cod_municipio).toBe('');
  });
});

describe('el formulario', () => {
  let fixture: ComponentFixture<PacienteFormComponent>;
  let form: PacienteFormComponent;
  /** Con qué se cerró el diálogo. `undefined` = no se cerró. */
  let cerradoCon: Patient | null | undefined;

  /**
   * El formulario es contenido de un `MatDialog`, así que el paciente a editar
   * entra por `MAT_DIALOG_DATA` y el resultado sale por `MatDialogRef.close()`.
   * Aquí los dos van dobles: montar el diálogo de verdad probaría el overlay
   * del CDK, no las reglas de captura, que es lo que estas pruebas miran.
   */
  function montar(paciente: Patient | null): void {
    cerradoCon = undefined;
    TestBed.resetTestingModule();
    TestBed.configureTestingModule({
      imports: [PacienteFormComponent],
      providers: [
        provideHttpClient(withXhr()),
        provideHttpClientTesting(),
        provideNoopAnimations(),
        { provide: MAT_DIALOG_DATA, useValue: paciente },
        { provide: MatDialogRef, useValue: { close: (p?: Patient) => (cerradoCon = p ?? null) } },
      ],
    });
    fixture = TestBed.createComponent(PacienteFormComponent);
    form = fixture.componentInstance;
    form.ngOnInit();
  }

  it('en alta arranca en blanco y en modo nuevo', () => {
    montar(null);

    expect(form.modo).toBe('nuevo');
    expect(form.datos.identificacion).toBe('');
    expect(form.editable).toBe(true);
  });

  it('al editar carga el paciente y entra en modo edición', () => {
    montar({ id: 7, ...completo() } as never);

    expect(form.modo).toBe('edicion');
    expect(form.datos.identificacion).toBe('1017234567');
  });

  it('no guarda con campos faltantes, y dice cuáles', () => {
    montar(null);
    form.guardar();

    // No se cerró: un formulario incompleto no devuelve paciente, y sin
    // paciente la pantalla no recarga ni afirma un alta que no ocurrió.
    expect(cerradoCon).toBeUndefined();
    expect(form.error()).toContain('documento');
  });

  it('la edad se calcula para cazar una fecha mal escrita', () => {
    montar(null);
    form.datos = { ...completo(), fecha_nacimiento: '2000-01-01' };
    const esperada = new Date().getFullYear() - 2000;
    // Puede ser uno menos si aún no ha llegado el cumpleaños de este año.
    expect([`${esperada} años`, `${esperada - 1} años`]).toContain(form.edad());
  });

  it('sin fecha no inventa una edad', () => {
    montar(null);
    form.datos = { ...completo(), fecha_nacimiento: '' };
    expect(form.edad()).toBeNull();
  });
});
