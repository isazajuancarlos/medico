/**
 * La máquina de estados de los formularios, migrada del sistema en FoxPro.
 *
 * En el original esto vivía en dos métodos del objeto compartido `medico1`:
 * `activar_objetos` (qué campos aceptan escritura) y `botones` (qué botones se
 * ven). Los dos se llamaban SIEMPRE JUNTOS —72 veces cada uno, en los mismos
 * sitios— porque en realidad son una sola cosa: el modo en que está el
 * formulario.
 *
 * Aquí se unifican, que es lo que el original habría hecho de poder. Un solo
 * estado gobierna la editabilidad y la barra de botones, así que no pueden
 * quedar desincronizados: el fallo clásico de esa pareja de métodos era
 * habilitar los campos y olvidar conmutar los botones, dejando al usuario
 * escribiendo en un formulario que no podía guardar.
 */

/** Los tres modos del original: consulta, alta y edición. */
export type ModoFormulario = 'consulta' | 'nuevo' | 'edicion';

/** Acciones de la barra CRUD (wznew, wzedit, wzdelete, wzsave, wzundo). */
export type AccionCrud = 'nuevo' | 'editar' | 'eliminar' | 'guardar' | 'deshacer';

export interface EstadoBarra {
  nuevo: boolean;
  editar: boolean;
  eliminar: boolean;
  guardar: boolean;
  deshacer: boolean;
  /** Navegación primero/anterior/siguiente/ultimo del original. */
  navegar: boolean;
}

/**
 * Qué botones se habilitan en cada modo.
 *
 * Consulta: se puede crear, editar, borrar y navegar; no hay nada que guardar.
 * Nuevo y edición: solo guardar o deshacer, y la navegación se bloquea — moverse
 * de registro con cambios sin guardar los perdía en silencio, y eso en una
 * historia clínica no es aceptable.
 */
const BARRA: Record<ModoFormulario, EstadoBarra> = {
  consulta: { nuevo: true, editar: true, eliminar: true, guardar: false, deshacer: false, navegar: true },
  nuevo:    { nuevo: false, editar: false, eliminar: false, guardar: true, deshacer: true, navegar: false },
  edicion:  { nuevo: false, editar: false, eliminar: false, guardar: true, deshacer: true, navegar: false },
};

export function barraDe(modo: ModoFormulario): EstadoBarra {
  return { ...BARRA[modo] };
}

/** En consulta los campos son de solo lectura; en los demás modos, editables. */
export function camposEditables(modo: ModoFormulario): boolean {
  return modo !== 'consulta';
}

/** Si hay cambios sin guardar que se perderían al salir. */
export function hayCambiosPendientes(modo: ModoFormulario): boolean {
  return modo !== 'consulta';
}

/**
 * Transición válida ante una acción, o `null` si la acción no aplica.
 *
 * Devolver `null` en vez de lanzar permite que la interfaz simplemente ignore
 * un atajo de teclado pulsado en el modo equivocado, que es lo que hacía el
 * original al tener el botón deshabilitado.
 */
export function siguienteModo(
  modo: ModoFormulario,
  accion: AccionCrud,
): ModoFormulario | null {
  const permitida = barraDe(modo);
  switch (accion) {
    case 'nuevo':
      return permitida.nuevo ? 'nuevo' : null;
    case 'editar':
      return permitida.editar ? 'edicion' : null;
    case 'eliminar':
      return permitida.eliminar ? 'consulta' : null;
    case 'guardar':
    case 'deshacer':
      return permitida.guardar ? 'consulta' : null;
    default:
      return null;
  }
}
