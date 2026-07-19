/**
 * La máquina de estados del formulario.
 *
 * Lo que se fija aquí son reglas que en el original estaban repartidas entre
 * `activar_objetos` y `botones`, y que por estar en dos métodos se podían
 * desincronizar. Aquí no pueden.
 */
import {
  barraDe,
  camposEditables,
  hayCambiosPendientes,
  siguienteModo,
} from './estado-formulario';

describe('estado del formulario', () => {
  describe('qué se puede hacer en cada modo', () => {
    it('en consulta se crea, edita, borra y navega, pero no se guarda', () => {
      const b = barraDe('consulta');
      expect(b.nuevo).toBe(true);
      expect(b.editar).toBe(true);
      expect(b.eliminar).toBe(true);
      expect(b.navegar).toBe(true);
      expect(b.guardar).toBe(false);
      expect(b.deshacer).toBe(false);
    });

    it('editando solo se guarda o se deshace', () => {
      for (const modo of ['nuevo', 'edicion'] as const) {
        const b = barraDe(modo);
        expect(b.guardar).toBe(true);
        expect(b.deshacer).toBe(true);
        expect(b.nuevo).toBe(false);
        expect(b.editar).toBe(false);
        expect(b.eliminar).toBe(false);
      }
    });

    it('NO se navega con cambios sin guardar', () => {
      // En el original, moverse de registro en edición perdía los cambios en
      // silencio. En una historia clínica eso no es aceptable.
      expect(barraDe('nuevo').navegar).toBe(false);
      expect(barraDe('edicion').navegar).toBe(false);
    });
  });

  describe('editabilidad de los campos', () => {
    it('va siempre de la mano del modo, no por separado', () => {
      expect(camposEditables('consulta')).toBe(false);
      expect(camposEditables('nuevo')).toBe(true);
      expect(camposEditables('edicion')).toBe(true);
    });

    it('los campos son editables exactamente cuando se puede guardar', () => {
      // Es la invariante que el original podía romper al llamar a
      // activar_objetos y olvidar botones: campos abiertos sin poder guardar.
      for (const modo of ['consulta', 'nuevo', 'edicion'] as const) {
        expect(camposEditables(modo)).toBe(barraDe(modo).guardar);
      }
    });
  });

  describe('transiciones', () => {
    it('el ciclo normal de alta vuelve a consulta', () => {
      expect(siguienteModo('consulta', 'nuevo')).toBe('nuevo');
      expect(siguienteModo('nuevo', 'guardar')).toBe('consulta');
    });

    it('el ciclo de edición vuelve a consulta', () => {
      expect(siguienteModo('consulta', 'editar')).toBe('edicion');
      expect(siguienteModo('edicion', 'guardar')).toBe('consulta');
    });

    it('deshacer devuelve a consulta desde cualquier edición', () => {
      expect(siguienteModo('nuevo', 'deshacer')).toBe('consulta');
      expect(siguienteModo('edicion', 'deshacer')).toBe('consulta');
    });

    it('una acción imposible devuelve null en vez de reventar', () => {
      // Permite que un atajo de teclado pulsado en el modo equivocado
      // simplemente se ignore, como el botón deshabilitado del original.
      expect(siguienteModo('consulta', 'guardar')).toBeNull();
      expect(siguienteModo('nuevo', 'nuevo')).toBeNull();
      expect(siguienteModo('edicion', 'eliminar')).toBeNull();
    });

    it('no se puede eliminar mientras se edita', () => {
      expect(siguienteModo('nuevo', 'eliminar')).toBeNull();
      expect(siguienteModo('edicion', 'eliminar')).toBeNull();
    });
  });

  describe('salida con cambios', () => {
    it('avisa en los modos de edición y no en consulta', () => {
      expect(hayCambiosPendientes('consulta')).toBe(false);
      expect(hayCambiosPendientes('nuevo')).toBe(true);
      expect(hayCambiosPendientes('edicion')).toBe(true);
    });
  });

  describe('la barra devuelta es una copia', () => {
    it('modificarla no altera el estado de los demás formularios', () => {
      const a = barraDe('consulta');
      a.guardar = true;
      expect(barraDe('consulta').guardar).toBe(false);
    });
  });
});
