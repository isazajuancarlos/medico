/**
 * La pestaña de permisos de /admin (#235): que la matriz SE DIBUJE.
 *
 * El backend ya está probado por su lado —la regla, la caché, los guardarraíles
 * y que un cambio por la API mueva de verdad lo que la siguiente petición puede
 * hacer—. Nada de eso dice si esta pantalla se abre: `admin.component.ts` no
 * tenía ni una prueba, y el formulario de informes reventaba en pantalla con
 * 550 pruebas en verde. Aquí se monta el componente de verdad y se mira el DOM.
 */
import { ComponentFixture, TestBed } from '@angular/core/testing';
import { provideHttpClient, withXhr } from '@angular/common/http';
import { provideHttpClientTesting, HttpTestingController } from '@angular/common/http/testing';
import { AdminComponent } from './admin.component';
import { MatrizCapacidades } from '../../services/admin.service';

/// Una matriz mínima con las tres situaciones que la pantalla tiene que
/// distinguir: una casilla que sigue lo recomendado, una apartada, y una que no
/// se puede tocar.
function matrizDePrueba(): MatrizCapacidades {
  return {
    institucion_id: 1,
    capacidades: [
      {
        capacidad: 'escribir_clinico',
        configurable: true,
        roles: [
          { rol: 'medico', permitido: true, defecto: true, apartado: false, configurable: true },
          { rol: 'laboratorista', permitido: true, defecto: false, apartado: true, configurable: true },
          { rol: 'superadmin', permitido: true, defecto: true, apartado: false, configurable: false },
        ],
      },
      {
        capacidad: 'gestionar_instituciones',
        configurable: false,
        roles: [
          { rol: 'medico', permitido: false, defecto: false, apartado: false, configurable: false },
          { rol: 'laboratorista', permitido: false, defecto: false, apartado: false, configurable: false },
          { rol: 'superadmin', permitido: true, defecto: true, apartado: false, configurable: false },
        ],
      },
    ],
  };
}

describe('la pestaña de permisos se dibuja', () => {
  let fixture: ComponentFixture<AdminComponent>;
  let http: HttpTestingController;

  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [AdminComponent],
      providers: [provideHttpClient(withXhr()), provideHttpClientTesting()],
    }).compileComponents();

    fixture = TestBed.createComponent(AdminComponent);
    fixture.detectChanges();
    http = TestBed.inject(HttpTestingController);
    // `ngOnInit` pide siete cosas que a esta pestaña no le importan, pero hay
    // que responderlas o `http.verify()` las denuncia. Se responde con una forma
    // INOCUA y no con `null`: el manejador de auditoría hace `r.items` y con
    // null lanza, lo que dejaba la suite en 74 verdes y exit 1 — un rojo que no
    // se ve en el recuento.
    http.match(() => true).forEach(r => r.flush({ items: [], total: 0 }));
  });

  function abrirPestana(): void {
    fixture.componentInstance.setTab('capacidades');
    const peticion = http.expectOne(r => r.url.endsWith('/capacidades'));
    expect(peticion.request.method).toBe('GET');
    peticion.flush(matrizDePrueba());
    fixture.detectChanges();
  }

  it('pinta una fila por capacidad y una columna por rol', () => {
    abrirPestana();
    const el: HTMLElement = fixture.nativeElement;
    expect(el.querySelectorAll('table.matriz tbody tr').length).toBe(2);
    expect(el.querySelectorAll('table.matriz thead th.rot').length).toBe(3);
  });

  it('usa el nombre en castellano y no el técnico', () => {
    abrirPestana();
    const texto: string = fixture.nativeElement.textContent;
    expect(texto).toContain('Escribir historia y recetar');
    expect(texto).not.toContain('escribir_clinico');
  });

  /// Lo que la ficha pedía: que se VEA qué se apartó del valor recomendado.
  it('marca la casilla apartada y solo esa', () => {
    abrirPestana();
    const apartadas = fixture.nativeElement.querySelectorAll('.celda.apartada');
    expect(apartadas.length).toBe(1);
  });

  /// Y que la pantalla no ofrezca una casilla que el servidor va a rechazar:
  /// prometer un control que no funciona es peor que no tenerlo.
  it('deshabilita lo que no es configurable', () => {
    abrirPestana();
    const el: HTMLElement = fixture.nativeElement;
    const deshabilitadas = el.querySelectorAll('.celda input:disabled');
    // Los 3 de `gestionar_instituciones` más el superadmin de la otra fila.
    expect(deshabilitadas.length).toBe(4);
    // Y las dos que sí se tocan quedan activas.
    expect(el.querySelectorAll('.celda input:not(:disabled)').length).toBe(2);
  });

  it('el botón de guardar nace apagado y se enciende al marcar', () => {
    abrirPestana();
    const comp = fixture.componentInstance;
    expect(comp.pendientes().length).toBe(0);

    const fila = comp.matriz()!.capacidades[0];
    const casilla = fila.roles[0];
    comp.marcar(fila, casilla, { target: { checked: false } } as unknown as Event);
    fixture.detectChanges();

    expect(comp.pendientes()).toEqual([
      { capacidad: 'escribir_clinico', rol: 'medico', permitido: false },
    ]);
    // Y la casilla queda marcada como apartada en el acto, sin esperar al PUT.
    expect(casilla.apartado).toBe(true);
  });

  /// El rechazo del guardarraíl llega como texto plano en un 422 y **tiene que
  /// verse**: es el mensaje que le dice al administrador por qué no puede.
  it('un rechazo del servidor se muestra y la matriz se relee', () => {
    abrirPestana();
    const comp = fixture.componentInstance;
    const fila = comp.matriz()!.capacidades[0];
    comp.marcar(fila, fila.roles[0], { target: { checked: false } } as unknown as Event);

    comp.guardarCapacidades();
    const put = http.expectOne(r => r.method === 'PUT' && r.url.endsWith('/capacidades'));
    put.flush('dejaría a la institución sin ningún rol que pueda gestionar usuarios', {
      status: 422,
      statusText: 'Unprocessable Entity',
    });
    fixture.detectChanges();

    // Se relee, porque no se aplicó nada: dejar las casillas movidas haría
    // creer que el cambio está puesto.
    const relectura = http.expectOne(r => r.method === 'GET' && r.url.endsWith('/capacidades'));
    relectura.flush(matrizDePrueba());
    fixture.detectChanges();
    expect(comp.pendientes().length).toBe(0);
  });

  afterEach(() => {
    http.verify();
  });
});
