// SPDX-License-Identifier: LicenseRef-Propietario
/**
 * La pantalla que pone el documento del profesional (P11/P12).
 *
 * El backend ya está probado —valida contra el maestro, aísla por institución
 * con 404, y rechaza con 409 el documento que ya es de otro—. Lo que se prueba
 * aquí es lo que la pantalla puede deshacer:
 *
 * 1. **que el fallo se VEA**. Es la misma asimetría de «cerrar sesión en todos
 *    los dispositivos»: si el servidor rechaza el documento y el diálogo se
 *    cierra en silencio, quien lo escribió se va convencido de haberlo
 *    guardado, y el RIPS sigue saliendo incompleto sin que nadie lo sepa. El
 *    mensaje del servidor dice CUÁL de las tres reglas se incumplió, así que se
 *    enseña ese y no uno genérico.
 * 2. **que la cuenta de «sin documento» sea derivada**, no una segunda copia
 *    que se olvida de actualizar justo después de guardar.
 * 3. **la regla de los dos campos**, que es la que impide dejar media identidad.
 */
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TestBed } from '@angular/core/testing';
import { provideHttpClient, withXhr } from '@angular/common/http';
import { provideHttpClientTesting, HttpTestingController } from '@angular/common/http/testing';
import { UsuariosService, Usuario } from '../../services/usuarios.service';
import { UsuarioDocumentoComponent } from './usuarios.component';

const CON: Usuario = {
  id: 1, nombre: 'Dra Ruiz', email: 'ruiz@x.co', rol: 'medico',
  institucion_id: 1, activo: true, tipo_documento: 'CC', numero_documento: '71234567',
};
const SIN: Usuario = {
  id: 2, nombre: 'Dr Paz', email: 'paz@x.co', rol: 'medico',
  institucion_id: 1, activo: true, tipo_documento: null, numero_documento: null,
};

describe('el documento del profesional, desde la pantalla', () => {
  let srv: UsuariosService;
  let http: HttpTestingController;

  beforeEach(() => {
    TestBed.configureTestingModule({
      providers: [provideHttpClient(withXhr()), provideHttpClientTesting()],
    });
    srv = TestBed.inject(UsuariosService);
    http = TestBed.inject(HttpTestingController);
  });

  afterEach(() => http.verify());

  it('el listado trae el documento, que es lo que permite ver a quién le falta', () => {
    let recibidos: Usuario[] = [];
    srv.listar().subscribe((u) => (recibidos = u));
    http.expectOne('/api/usuarios').flush([CON, SIN]);

    expect(recibidos.filter((u) => !u.numero_documento).map((u) => u.nombre)).toEqual(['Dr Paz']);
  });

  it('guardar manda los dos campos, y borrar manda dos nulos', () => {
    srv.fijarDocumento(2, 'CC', '80000000').subscribe();
    const a = http.expectOne('/api/usuarios/2/documento');
    expect(a.request.method).toBe('PUT');
    expect(a.request.body).toEqual({ tipo_documento: 'CC', numero_documento: '80000000' });
    a.flush(null);

    // Borrar es una operación legítima: mejor sin documento que con uno falso.
    srv.fijarDocumento(2, null, null).subscribe();
    const b = http.expectOne('/api/usuarios/2/documento');
    expect(b.request.body).toEqual({ tipo_documento: null, numero_documento: null });
    b.flush(null);
  });

  it('el rechazo del servidor llega al llamante y no se traga', () => {
    // 409: ese documento ya es de otro profesional. Si el servicio se lo comiera,
    // la pantalla cerraría el diálogo como si hubiera guardado.
    let fallo: unknown = null;
    let exito = false;
    srv.fijarDocumento(2, 'CC', '71234567').subscribe({
      next: () => (exito = true),
      error: (e) => (fallo = e),
    });
    http.expectOne('/api/usuarios/2/documento').flush(
      { detail: 'ese documento ya es de otro profesional.' },
      { status: 409, statusText: 'Conflict' },
    );

    expect(exito).toBe(false);
    expect(fallo).toBeTruthy();
    expect((fallo as { error: { detail: string } }).error.detail).toContain('ya es de otro');
  });

  it('los tipos se piden al servidor y no se escriben en el navegador', () => {
    srv.tiposDocumento().subscribe();
    const r = http.expectOne('/api/tipos-documento-profesional');
    expect(r.request.method).toBe('GET');
    r.flush([{ codigo: 'CC', nombre: 'CEDULA DE CIUDADANIA' }]);
  });
});

describe('el diálogo del documento', () => {
  function dialogo(usuario: Usuario): UsuarioDocumentoComponent {
    return new UsuarioDocumentoComponent({
      usuario,
      tipos: [{ codigo: 'CC', nombre: 'CEDULA DE CIUDADANIA' }],
    } as { usuario: Usuario; tipos: { codigo: string; nombre: string }[] });
  }

  it('se abre con lo que el profesional YA tiene', () => {
    const d = dialogo(CON);
    expect(d.tipo).toBe('CC');
    expect(d.numero).toBe('71234567');
  });

  it('o los dos campos o ninguno', () => {
    const d = dialogo(SIN);

    // Ninguno: válido, y significa «no lo tengo».
    expect(d.valido()).toBe(true);
    expect(d.resultado()).toEqual({ tipo: null, numero: null });

    // Solo el tipo: no.
    d.tipo = 'CC';
    expect(d.valido()).toBe(false);

    // Solo el número: tampoco.
    d.tipo = null;
    d.numero = '999';
    expect(d.valido()).toBe(false);

    // Los dos: sí.
    d.tipo = 'CC';
    expect(d.valido()).toBe(true);
    expect(d.resultado()).toEqual({ tipo: 'CC', numero: '999' });
  });

  it('un número de solo espacios no cuenta como número', () => {
    // Si contara, se guardaría un documento cuyo número es vacío: la forma
    // correcta sin el dato, que es lo que P11/P12 enseñó a no hacer.
    const d = dialogo(SIN);
    d.tipo = 'CC';
    d.numero = '   ';
    expect(d.valido()).toBe(false);
  });
});
