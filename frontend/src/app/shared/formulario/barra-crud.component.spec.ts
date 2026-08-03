/**
 * La barra CRUD: los cinco botones que en el original había que recablear en
 * cada uno de los 18 formularios que la tenían.
 *
 * Lo que se prueba no es que se dibujen botones, sino que la barra NO deje
 * ejecutar una acción imposible en el modo actual —el equivalente al botón
 * inerte del original— y que lo que emite coincida con lo que muestra.
 */
import { ComponentFixture, TestBed } from '@angular/core/testing';
import { BarraCrudComponent } from './barra-crud.component';
import { AccionCrud, ModoFormulario, barraDe } from './estado-formulario';

describe('barra CRUD', () => {
  let fixture: ComponentFixture<BarraCrudComponent>;
  let barra: BarraCrudComponent;
  let acciones: AccionCrud[];
  let modos: ModoFormulario[];

  beforeEach(() => {
    TestBed.configureTestingModule({ imports: [BarraCrudComponent] });
    fixture = TestBed.createComponent(BarraCrudComponent);
    barra = fixture.componentInstance;

    acciones = [];
    modos = [];
    barra.accion.subscribe((a) => acciones.push(a));
    barra.modoCambia.subscribe((m) => modos.push(m));
  });

  /**
   * Con `setInput`, no asignando la propiedad: es lo que hace un padre al
   * enlazar `[modo]`, y es lo único que marca sucio un componente OnPush —el
   * defecto desde Angular 22—. Asignando a pelo, `detectChanges()` no repinta
   * y la prueba compara contra lo que había antes.
   */
  function enModo(modo: ModoFormulario) {
    fixture.componentRef.setInput('modo', modo);
    fixture.detectChanges();
  }

  describe('una acción imposible no se ejecuta', () => {
    it('no se guarda desde consulta', () => {
      enModo('consulta');
      barra.ejecutar('guardar');

      expect(acciones).toEqual([]);
      expect(modos).toEqual([]);
    });

    it('no se elimina mientras se edita', () => {
      for (const modo of ['nuevo', 'edicion'] as const) {
        enModo(modo);
        barra.ejecutar('eliminar');
      }
      expect(acciones).toEqual([]);
    });

    it('el modo tampoco cambia por dentro', () => {
      enModo('consulta');
      barra.ejecutar('deshacer');
      expect(barra.modo).toBe('consulta');
    });
  });

  describe('las acciones válidas emiten acción y destino', () => {
    it('nuevo desde consulta abre el alta', () => {
      enModo('consulta');
      barra.ejecutar('nuevo');

      expect(acciones).toEqual(['nuevo']);
      expect(modos).toEqual(['nuevo']);
    });

    it('guardar desde edición vuelve a consulta', () => {
      enModo('edicion');
      barra.ejecutar('guardar');

      expect(acciones).toEqual(['guardar']);
      expect(modos).toEqual(['consulta']);
    });

    it('la barra NO se cambia el modo a sí misma: lo hace el anfitrión', () => {
      // Si se lo cambiara sola, el formulario y la barra podrían discrepar,
      // que es exactamente el fallo que tenía el original entre
      // `activar_objetos` y `botones`.
      enModo('consulta');
      barra.ejecutar('nuevo');
      expect(barra.modo).toBe('consulta');
    });
  });

  describe('lo que se muestra es lo que se puede hacer', () => {
    it('la barra dibujada coincide con el estado del modo', () => {
      for (const modo of ['consulta', 'nuevo', 'edicion'] as const) {
        enModo(modo);
        expect(barra.barra()).toEqual(barraDe(modo));
      }
    });

    it('cada acción habilitada es ejecutable, y ninguna deshabilitada lo es', () => {
      const todas: AccionCrud[] = ['nuevo', 'editar', 'eliminar', 'guardar', 'deshacer'];

      for (const modo of ['consulta', 'nuevo', 'edicion'] as const) {
        for (const accion of todas) {
          enModo(modo);
          acciones = [];
          barra.accion.subscribe((a: AccionCrud) => acciones.push(a));

          barra.ejecutar(accion);
          const habilitada = barra.barra()[accion];
          expect(acciones.length > 0).toBe(habilitada);
        }
      }
    });
  });

  describe('navegación', () => {
    it('los formularios de un solo registro pueden ocultarla', () => {
      barra.mostrarNavegacion = false;
      enModo('consulta');
      expect(fixture.nativeElement.querySelectorAll('.pi-angle-right').length).toBe(0);
    });

    it('se muestra por defecto', () => {
      enModo('consulta');
      expect(fixture.nativeElement.querySelectorAll('.pi-angle-right').length).toBe(1);
    });

    it('avisa cuando hay cambios sin guardar', () => {
      enModo('edicion');
      expect(fixture.nativeElement.textContent).toContain('Editando');

      enModo('nuevo');
      expect(fixture.nativeElement.textContent).toContain('sin guardar');

      enModo('consulta');
      expect(fixture.nativeElement.textContent).not.toContain('Editando');
    });
  });
});
