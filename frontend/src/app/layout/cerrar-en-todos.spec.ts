/**
 * El botón «cerrar sesión en todos los dispositivos» (M3, tarea #194).
 *
 * El backend ya estaba y ya está probado: sube el `token_version`, invalida toda
 * cookie emitida, y responde **500 en vez de 200 cuando la revocación no tocó
 * ninguna fila** — se niega a fingir un cierre que no ocurrió.
 *
 * Lo que se prueba aquí es que la pantalla no deshaga esa negativa. Un cierre
 * normal puede irse al login pase lo que pase, porque borrar la cookie de este
 * navegador ya cumple lo prometido. Este NO: lo prometido pasa en los otros
 * dispositivos, y si el servidor falla, mandar al login pintaría de «hecho» algo
 * que no se hizo — justo a quien lo pidió porque sospecha que alguien más está
 * dentro de su cuenta.
 */
import { vi } from 'vitest';
import { TestBed } from '@angular/core/testing';
import { provideHttpClient } from '@angular/common/http';
import { provideHttpClientTesting, HttpTestingController } from '@angular/common/http/testing';
import { provideRouter, Router } from '@angular/router';
import { Avisos } from '../shared/ui/avisos.service';
import { AuthService } from '../core/auth.service';

/**
 * El entorno de estas pruebas no trae `localStorage`, y `AuthService` lo lee en
 * su constructor — por eso este servicio no se había instanciado nunca en una
 * prueba, y por eso hace falta suplirlo aquí.
 *
 * Se suple con un almacén de verdad y no con `noop`: parte de lo que se
 * comprueba es que la copia local de la identidad SE BORRE al revocar y NO se
 * borre cuando la revocación falla. Con un doble que traga y devuelve `null`,
 * las dos afirmaciones pasarían solas.
 */
function suplirAlmacenLocal(): void {
  if (typeof globalThis.localStorage !== 'undefined') return;
  const almacen = new Map<string, string>();
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: {
      getItem: (k: string) => almacen.get(k) ?? null,
      setItem: (k: string, v: string) => void almacen.set(k, v),
      removeItem: (k: string) => void almacen.delete(k),
      clear: () => almacen.clear(),
      key: (i: number) => Array.from(almacen.keys())[i] ?? null,
      get length() {
        return almacen.size;
      },
    },
  });
}

describe('cerrar sesión en todos los dispositivos', () => {
  let auth: AuthService;
  let http: HttpTestingController;

  beforeEach(() => {
    suplirAlmacenLocal();
    localStorage.setItem(
      'current_user',
      JSON.stringify({ id: 1, username: 'a@x.co', nombre_completo: 'Ana', rol: 'medico' }),
    );
    TestBed.configureTestingModule({
      providers: [
        provideHttpClient(),
        provideHttpClientTesting(),
        provideRouter([]),
        Avisos,
      ],
    });
    auth = TestBed.inject(AuthService);
    http = TestBed.inject(HttpTestingController);
    // El constructor del servicio valida la cookie contra `/api/yo`. Se responde
    // con una identidad para que la sesión quede montada, como en la vida real.
    http.expectOne('/api/yo').flush({
      id: 1,
      nombre: 'Ana',
      email: 'a@x.co',
      rol: 'medico',
      institucion_id: 1,
    });
    http.match(() => true).forEach(r => r.flush({}));
  });

  afterEach(() => {
    http.verify();
    localStorage.removeItem('current_user');
  });

  it('llama al endpoint de revocación, y no al de salir', () => {
    auth.logoutTodos().subscribe();
    const p = http.expectOne('/api/salir-todo');
    expect(p.request.method).toBe('POST');
    // No manda ningún id: el servidor revoca las del usuario AUTENTICADO. Si la
    // pantalla enviara uno, cualquiera podría cerrar las sesiones de otro.
    expect(p.request.body).toEqual({});
    p.flush({ ok: true });
  });

  it('al lograrse, deja la sesión local limpia', () => {
    auth.logoutTodos().subscribe();
    http.expectOne('/api/salir-todo').flush({ ok: true });
    expect(auth.currentUser()).toBeNull();
    expect(localStorage.getItem('current_user')).toBeNull();
  });

  it('si el servidor falla, NO limpia la sesión local', () => {
    // La regla entera. Limpiar aquí haría que la pantalla se comportara como si
    // hubiera funcionado.
    let hubo_error = false;
    auth.logoutTodos().subscribe({ error: () => (hubo_error = true) });
    http.expectOne('/api/salir-todo').flush(
      { mensaje: 'no se pudo revocar' },
      { status: 500, statusText: 'Internal Server Error' },
    );
    expect(hubo_error).toBe(true);
    expect(auth.currentUser()).not.toBeNull();
    expect(localStorage.getItem('current_user')).not.toBeNull();
  });

  it('el error llega al llamante en vez de tragarse', () => {
    // Sin esto la cabecera no tendría de qué enterarse, y la única señal del
    // fallo sería que no pasa nada al pulsar.
    let recibido: number | null = null;
    auth.logoutTodos().subscribe({ error: (e: { status: number }) => (recibido = e.status) });
    http.expectOne('/api/salir-todo').flush({}, { status: 500, statusText: 'x' });
    expect(recibido).toBe(500);
  });

  it('el cierre NORMAL sí limpia aunque el servidor falle', () => {
    // El contraste que hace que la prueba anterior signifique algo: no es que se
    // limpie o no según el humor, es que las dos acciones prometen cosas
    // distintas. Aquí lo prometido —cerrar ESTE navegador— se cumple igual.
    auth.logoutTodos().subscribe({ error: () => undefined });
    http.expectOne('/api/salir-todo').flush({}, { status: 500, statusText: 'x' });
    expect(auth.currentUser()).not.toBeNull();

    auth.logout().subscribe({ error: () => undefined });
    http.expectOne('/api/salir').flush({}, { status: 500, statusText: 'x' });
    expect(auth.currentUser()).toBeNull();
  });

  it('la cabecera no navega al login cuando la revocación falla', async () => {
    const { HeaderComponent } = await import('./header.component');
    const fixture = TestBed.createComponent(HeaderComponent);
    const router = TestBed.inject(Router);
    const navegaciones: readonly unknown[][] = [];
    vi.spyOn(router, 'navigate').mockImplementation((c: readonly unknown[]) => {
      (navegaciones as unknown[][]).push(c as unknown[]);
      return Promise.resolve(true);
    });
    fixture.detectChanges();

    // Se llama al camino que el diálogo confirma, sin el diálogo: lo que se
    // prueba es la consecuencia, no PrimeNG.
    (fixture.componentInstance as unknown as { cerrarTodo(): void }).cerrarTodo();
    http.expectOne('/api/salir-todo').flush({}, { status: 500, statusText: 'x' });

    expect(navegaciones.length).toBe(0);
    expect(fixture.componentInstance.cerrandoTodo()).toBe(false);
  });

  it('la cabecera sí navega al login cuando la revocación funciona', async () => {
    const { HeaderComponent } = await import('./header.component');
    const fixture = TestBed.createComponent(HeaderComponent);
    const router = TestBed.inject(Router);
    const navegaciones: readonly unknown[][] = [];
    vi.spyOn(router, 'navigate').mockImplementation((c: readonly unknown[]) => {
      (navegaciones as unknown[][]).push(c as unknown[]);
      return Promise.resolve(true);
    });
    fixture.detectChanges();

    (fixture.componentInstance as unknown as { cerrarTodo(): void }).cerrarTodo();
    http.expectOne('/api/salir-todo').flush({ ok: true });

    expect(navegaciones).toEqual([['/login']]);
  });

  it('la cabecera ofrece los dos botones, y se distinguen', async () => {
    const { HeaderComponent } = await import('./header.component');
    const fixture = TestBed.createComponent(HeaderComponent);
    fixture.detectChanges();
    const el: HTMLElement = fixture.nativeElement;
    const etiquetas = Array.from(el.querySelectorAll('button')).map(b =>
      b.getAttribute('aria-label'),
    );
    expect(etiquetas).toContain('Cerrar sesión');
    expect(etiquetas).toContain('Cerrar sesión en todos los dispositivos');
  });
});
