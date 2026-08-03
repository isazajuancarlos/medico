// SPDX-License-Identifier: LicenseRef-Propietario
import { describe, expect, it } from 'vitest';

/**
 * Que todo estado que la plantilla LEE y alguien escribe en ASÍNCRONO sea una
 * señal.
 *
 * Nace de #256, la adopción de OnPush. Desde Angular 22 OnPush es el defecto, y
 * un componente OnPush **solo se vuelve a pintar** cuando algo lo marca sucio:
 * una señal que cambió, un evento de su propia vista, o un `markForCheck()` a
 * mano. Un campo llano escrito desde un `subscribe` no marca nada.
 *
 * El fallo que eso produce no se parece a un fallo: la petición responde, el
 * campo cambia, la memoria tiene el dato correcto — y la pantalla sigue
 * mostrando lo anterior. Nada lanza, nada se pone rojo, `ng build` pasa, el
 * banco de humo monta la pantalla igual (monta con la pantalla vacía, que es
 * antes de que llegue la respuesta) y el guardián de las directivas mudas mira
 * otra cosa. Es el mismo silencio que ya costó caro con `matTooltip` sin su
 * módulo, con la edición de bloques de la agenda y con la fuente de Material.
 *
 * # La regla, en una frase
 *
 * *Si la plantilla lo lee y algo asíncrono lo escribe, tiene que ser señal.*
 *
 * No admite «en cualquier otro caso» (directiva 23). En particular NO es
 * excepción que el `subscribe` lo dispare un clic: el clic marca la vista sucia
 * en ese instante, y la escritura llega después, cuando la detección ya pasó.
 *
 * # Lo que NO se marca, y por qué
 *
 * - Escrituras síncronas desde un manejador de la vista (`(click)`, `ngModel`):
 *   el propio evento marca la vista sucia. Por eso solo se miran los cuerpos de
 *   `subscribe`, `setTimeout`, `then` y `requestAnimationFrame`.
 * - Campos que la plantilla no lee: da igual cómo se escriban.
 *
 * # Por qué se leen las EXPRESIONES y no la plantilla entera
 *
 * Buscar el nombre del campo en el texto de la plantilla da falsos positivos
 * absurdos: un campo llamado `form` aparece dentro de `mat-form-field` en toda
 * pantalla con un formulario. Aquí se extraen solo los sitios donde Angular
 * evalúa código —interpolaciones, enlaces, eventos y el control de flujo— y se
 * busca ahí. Una alarma que resulta falsa dos veces es una que a la tercera
 * nadie mira.
 */

/** Las vías por las que un valor llega DESPUÉS de que la detección ya pasó. */
const VIAS_ASINCRONAS = [
  /\.subscribe\s*\(/g,
  /setTimeout\s*\(/g,
  /\.then\s*\(/g,
  /requestAnimationFrame\s*\(/g,
];

/** Desde el paréntesis de apertura, el bloque completo contando anidamiento. */
function bloqueEquilibrado(fuente: string, inicio: number, abre = '(', cierra = ')'): string {
  let profundidad = 0;
  for (let i = inicio; i < fuente.length; i++) {
    if (fuente[i] === abre) profundidad++;
    else if (fuente[i] === cierra) {
      profundidad--;
      if (profundidad === 0) return fuente.slice(inicio, i + 1);
    }
  }
  return fuente.slice(inicio);
}

/**
 * Los cuerpos de los métodos de la clase, por nombre.
 *
 * Hace falta porque **el alcance asíncrono no termina en el `subscribe`**: la
 * primera versión de este guardián solo miraba el cuerpo del callback y se dejó
 * fuera `historias`, donde el `subscribe` llama a `_loadAnt(...)` y es ESE
 * método el que escribe los cuatro objetos de antecedentes. Un guardián que
 * mira un solo nivel da un verde que no significa nada.
 */
function metodosDe(cuerpo: string): Map<string, string> {
  const metodos = new Map<string, string>();
  for (const m of cuerpo.matchAll(/(?:^|\n)\s{2}(?:private\s+|protected\s+|public\s+|async\s+)*(\w+)\s*\([^)]*\)\s*(?::\s*[^{;]+)?\{/g)) {
    const llave = cuerpo.indexOf('{', m.index! + m[0].length - 1);
    metodos.set(m[1], bloqueEquilibrado(cuerpo, llave, '{', '}'));
  }
  return metodos;
}

/**
 * Todo lo que puede ejecutarse DESPUÉS de la detección: los cuerpos de los
 * callbacks asíncronos, más —transitivamente— todo método que llamen.
 */
function alcanceAsincrono(cuerpo: string): string[] {
  const metodos = metodosDe(cuerpo);
  const trozos: string[] = [];
  const pendientes: string[] = [];

  for (const via of VIAS_ASINCRONAS) {
    for (const m of cuerpo.matchAll(via)) {
      pendientes.push(bloqueEquilibrado(cuerpo, m.index! + m[0].length - 1));
    }
  }

  const vistos = new Set<string>();
  while (pendientes.length) {
    const trozo = pendientes.pop()!;
    trozos.push(trozo);
    for (const llamada of trozo.matchAll(/this\.(\w+)\s*\(/g)) {
      const nombre = llamada[1];
      if (vistos.has(nombre)) continue;
      vistos.add(nombre);
      const m = metodos.get(nombre);
      if (m) pendientes.push(m);
    }
  }
  return trozos;
}

/** Los nombres declarados como señal (`signal`, `computed`, `toSignal`). */
function señalesDe(fuente: string): Set<string> {
  const nombres = new Set<string>();
  const patrones = [
    /(\w+)\s*=\s*(?:signal|computed)\s*[<(]/g,
    /readonly\s+(\w+)\s*=\s*(?:signal|computed|toSignal)/g,
  ];
  for (const p of patrones) {
    for (const m of fuente.matchAll(p)) nombres.add(m[1]);
  }
  return nombres;
}

/**
 * Solo los trozos de plantilla que Angular EVALÚA: interpolaciones, enlaces,
 * eventos y control de flujo. Ver el porqué en la cabecera.
 */
function expresionesDePlantilla(cuerpoDelComponente: string): string {
  const plantilla = /template:\s*`([\s\S]*?)`\s*,\s*(?:changeDetection|styles|standalone|selector|imports|providers|\})/
    .exec(cuerpoDelComponente);
  if (!plantilla) return '';
  const t = plantilla[1];
  const trozos: string[] = [];
  const fuentes = [
    /\{\{([\s\S]*?)\}\}/g,          // interpolación
    /\[[\w.$()-]+\]\s*=\s*"([^"]*)"/g, // enlace de propiedad y [(ngModel)]
    /\(([\w.$-]+)\)\s*=\s*"([^"]*)"/g, // evento
    /\*ng(?:If|For)\s*=\s*"([^"]*)"/g, // control de flujo antiguo
    /@(?:if|for|switch|case)\s*\(([^)]*)\)/g, // control de flujo nuevo
  ];
  for (const f of fuentes) {
    for (const m of t.matchAll(f)) trozos.push(m.slice(1).join(' '));
  }
  return trozos.join('\n');
}

const FUENTES = import.meta.glob('./**/*.ts', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

const ARCHIVOS = Object.entries(FUENTES).filter(([ruta]) => !ruta.endsWith('.spec.ts'));

interface Componente {
  archivo: string;
  nombre: string;
  /** Campos llanos escritos en asíncrono y leídos por la plantilla. */
  sospechosos: string[];
}

function componentesDe(archivo: string, fuente: string): Componente[] {
  const señales = señalesDe(fuente);
  const trozos = fuente.split('@Component(');
  return trozos.slice(1).map((cuerpo, i) => {
    const expresiones = expresionesDePlantilla(cuerpo);
    const sospechosos = new Set<string>();
    for (const bloque of alcanceAsincrono(cuerpo)) {
      for (const a of bloque.matchAll(/this\.(\w+)\s*=[^=]/g)) {
        const campo = a[1];
        if (señales.has(campo)) continue;
        // Leído por la plantilla: se busca el identificador en las
        // expresiones, no en el texto de la plantilla.
        const leido = new RegExp(`(?<![\\w.$])${campo}(?![\\w$])`).test(expresiones);
        if (leido) sospechosos.add(campo);
      }
    }
    const clase = /export class (\w+)/.exec(cuerpo);
    return {
      archivo,
      nombre: clase ? clase[1] : `${archivo}#${i + 1}`,
      sospechosos: [...sospechosos].sort(),
    };
  });
}

const COMPONENTES = ARCHIVOS.flatMap(([ruta, fuente]) => componentesDe(ruta, fuente));

describe('el estado que la plantilla lee es señal', () => {
  // Sin esta guardia, un recorrido que dejara de encontrar componentes pasaría
  // en verde sin comprobar nada. Misma guardia que las otras dos redes.
  it('encuentra los componentes del proyecto', () => {
    expect(COMPONENTES.length).toBeGreaterThan(50);
  });

  for (const c of COMPONENTES) {
    if (c.sospechosos.length === 0) continue;

    it(`${c.nombre} (${c.archivo}) no escribe estado de plantilla en asíncrono`, () => {
      expect(
        c.sospechosos,
        `estos campos los escribe un callback asíncrono y la plantilla los lee: ` +
          `${c.sospechosos.join(', ')}. Con OnPush —el defecto desde Angular 22— ` +
          'la respuesta llega, el campo cambia y la pantalla NO se repinta, sin ' +
          'que nada falle. Conviértalos en señal.',
      ).toEqual([]);
    });
  }
});
