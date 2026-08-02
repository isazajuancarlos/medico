/**
 * El banco de humo de TODAS las pantallas: que cada una **se monte**.
 *
 * Existe por la migración de PrimeNG a Angular Material (#255), y por un número:
 * de las 30 páginas de `pages/`, **tres** tenían prueba. Para las otras 27 la
 * única red era `ng build`, que dice que el TypeScript compila y no dice si la
 * pantalla se abre — cambiar un `<p-table>` por un `<table mat-table>` puede
 * dejar la tabla vacía sin que nada falle. Es la directiva 28 aplicada al
 * frontend: el compilador mide el códec, no el producto.
 *
 * **No enumera componentes: recorre `app.routes.ts`.** Una pantalla nueva entra
 * sola en el banco, y una que se borre sale sola. Un banco escrito a mano nace
 * completo y caduca en el primer commit; este no puede quedarse atrás porque su
 * fuente es la misma tabla de rutas que sirve la aplicación.
 *
 * Lo que afirma es deliberadamente poco y por eso no da falsos verdes: que el
 * componente se instancia, que un ciclo de detección de cambios no lanza, y que
 * deja algo en el DOM. Lo que NO afirma —que la tabla traiga las filas, que el
 * botón haga lo suyo— es trabajo de las pruebas de cada pantalla.
 *
 * Las peticiones se interceptan y **no se responden**: se comprueba el montaje
 * con la pantalla vacía, que es el estado en que de verdad se abre siempre —
 * primero se pinta, luego llegan los datos—. Por eso no hay `http.verify()`
 * aquí: dejar peticiones sin atender es el caso, no un descuido.
 */
import { TestBed } from '@angular/core/testing';
import { Type } from '@angular/core';
import { provideHttpClient, withXhr } from '@angular/common/http';
import { provideHttpClientTesting } from '@angular/common/http/testing';
import { provideRouter, Routes } from '@angular/router';
import { provideNoopAnimations } from '@angular/platform-browser/animations';
import { appConfig } from './app.config';
import { routes } from './app.routes';

/**
 * `AuthService` lee `localStorage` en su constructor y el entorno de pruebas no
 * lo trae. Se suple con un almacén de verdad —no con un doble que devuelve
 * `null`— porque varias pantallas deciden qué pintar según el rol del usuario:
 * sin identidad, media aplicación se montaría vacía y el banco pasaría sin
 * haber mirado nada.
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

/** Un superadmin: el rol que ve TODAS las pantallas. */
const IDENTIDAD = {
  id: 1,
  username: 'a@x.co',
  nombre_completo: 'Ana Ruiz',
  rol: 'superadmin',
  institucion_id: 1,
};

interface Pantalla {
  ruta: string;
  cargar: () => Promise<Type<unknown>>;
}

/** Aplana el árbol de rutas y se queda con las que traen un componente diferido. */
function pantallasDe(rs: Routes, prefijo = ''): Pantalla[] {
  const salida: Pantalla[] = [];
  for (const r of rs) {
    const ruta = [prefijo, r.path ?? ''].filter(Boolean).join('/');
    if (r.loadComponent) {
      salida.push({
        ruta: ruta || '(raíz)',
        cargar: () => Promise.resolve(r.loadComponent!()) as Promise<Type<unknown>>,
      });
    }
    if (r.children) salida.push(...pantallasDe(r.children, ruta));
  }
  return salida;
}

const PANTALLAS = pantallasDe(routes);

describe('todas las pantallas se montan', () => {
  beforeEach(() => {
    suplirAlmacenLocal();
    localStorage.setItem('current_user', JSON.stringify(IDENTIDAD));
    localStorage.setItem('modulos_habilitados', JSON.stringify(['*']));
  });

  afterEach(() => {
    localStorage.clear();
    TestBed.resetTestingModule();
  });

  /**
   * La guardia del propio banco: si `app.routes.ts` deja de exponer pantallas
   * —un refactor que cambie `loadComponent` por otra cosa—, todo lo de abajo se
   * volvería un bucle sobre cero elementos y pasaría en verde sin probar nada.
   * Este número se sube a mano cuando se añaden pantallas, a propósito.
   */
  it('el banco encuentra las pantallas de la aplicación', () => {
    expect(PANTALLAS.length).toBeGreaterThanOrEqual(30);
  });

  for (const p of PANTALLAS) {
    it(`monta /${p.ruta}`, async () => {
      const componente = await p.cargar();
      TestBed.configureTestingModule({
        imports: [componente],
        providers: [
          // **Los proveedores de la aplicación de verdad**, no una lista
          // paralela. Se descubrió montando `/rips`: el calendario de Material
          // exige un `DateAdapter` que sí está en `appConfig`, y el banco
          // fallaba por no tenerlo — o sea que cualquier proveedor global era un
          // punto ciego suyo, y una lista propia se desincroniza en el primer
          // cambio y empieza a probar una aplicación que no existe.
          ...appConfig.providers,
          // Y encima lo propio de una prueba. Van DESPUÉS a propósito: en
          // Angular gana el último, así que estos pisan a los de arriba.
          provideHttpClient(withXhr()),
          provideHttpClientTesting(),
          provideRouter([]),
          provideNoopAnimations(),
        ],
      });

      const fixture = TestBed.createComponent(componente);
      fixture.detectChanges();

      expect(fixture.componentInstance).toBeTruthy();
      expect((fixture.nativeElement as HTMLElement).innerHTML.length).toBeGreaterThan(0);
    });
  }
});
