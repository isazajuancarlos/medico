// SPDX-License-Identifier: LicenseRef-Propietario
import { describe, expect, it } from 'vitest';

/**
 * Todo campo de antecedentes tiene DÓNDE escribirse.
 *
 * # El fallo que este guardián existe para no repetir
 *
 * El 2026-08-03, contrastando la historia clínica contra el sistema vivo, se
 * añadieron doce campos que faltaban. Ocho llegaron a la pantalla y **tres se
 * quedaron a medias**: `hosp_otra`, `lactancia_meses` y
 * `hermanos_muertos_menores_5` entraron en la interfaz y en el objeto por
 * defecto, viajaron al servidor en cada guardado… y ninguna pantalla los
 * ofrecía. Un campo que nadie puede rellenar no es media característica: es
 * una que no existe, y encima parece hecha desde el código.
 *
 * Lo irónico es que el hueco se descubrió el MISMO día en el sistema original
 * —16 columnas de `enca_historia` sin un solo control que las escriba— y en el
 * nuestro —P11/P12, el documento del profesional, con columna y sin puerta—.
 * Es un fallo que se comete solo, porque el modelo y la plantilla se editan por
 * separado y nada los ata.
 *
 * # Por qué se lee el fuente y no se monta la pantalla
 *
 * Lo que hay que comprobar es una AUSENCIA, y una ausencia no se ve
 * ejecutando: el banco de humo monta `historias` igual de bien con el campo
 * dentro o fuera de la plantilla, porque monta con el formulario vacío. Es el
 * mismo argumento del guardián de las directivas de Material.
 *
 * # La regla, en una frase
 *
 * Si un campo está en una `interface Ant…`, tiene que aparecer al menos una vez
 * como `antAlgo().campo` en la plantilla del componente. No se comprueba CÓMO
 * se pinta —eso es diseño— sino que se pueda tocar.
 */

const FUENTE = import.meta.glob('./pages/historias/historias.component.ts', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

const ARCHIVO = Object.values(FUENTE)[0];

/** Los campos declarados en cada `interface Ant…`. */
function camposDeInterfaces(fuente: string): Map<string, string[]> {
  const out = new Map<string, string[]>();
  const re = /interface\s+(Ant\w+)\s*\{([\s\S]*?)\n\}/g;
  for (let m = re.exec(fuente); m; m = re.exec(fuente)) {
    const cuerpo = m[2]
      // Fuera los comentarios: llevan ejemplos con dos puntos que parecerían campos.
      .replace(/\/\/\/?[^\n]*/g, '')
      .replace(/\/\*[\s\S]*?\*\//g, '');
    const campos = [...cuerpo.matchAll(/(\w+)\s*:/g)].map((c) => c[1]);
    out.set(m[1], campos);
  }
  return out;
}

describe('todo campo de antecedentes se puede rellenar', () => {
  it('la búsqueda encuentra las cuatro interfaces y sus campos', () => {
    // Sin esto, un cambio de forma en el archivo dejaría el guardián mirando
    // un conjunto vacío y dando verde sin comprobar nada.
    const interfaces = camposDeInterfaces(ARCHIVO);
    expect(ARCHIVO, 'no se pudo leer historias.component.ts').toBeTypeOf('string');
    expect([...interfaces.keys()].sort()).toEqual([
      'AntFamiliares',
      'AntGineco',
      'AntPerinatales',
      'AntPersonales',
    ]);
    for (const [nombre, campos] of interfaces) {
      expect(campos.length, `${nombre} sin campos: la extracción se rompió`).toBeGreaterThan(5);
    }
  });

  it('cada campo aparece en la plantilla', () => {
    const huerfanos: string[] = [];
    for (const [nombre, campos] of camposDeInterfaces(ARCHIVO)) {
      for (const campo of campos) {
        // `antFam()`, `antPers()`, `antGineco()`, `antPerinatales()` — se acepta
        // cualquier señal, porque lo que importa es que el campo se toque, no
        // desde qué nombre.
        const usado = new RegExp(`ant\\w*\\(\\)\\.${campo}\\b`).test(ARCHIVO);
        if (!usado) huerfanos.push(`${nombre}.${campo}`);
      }
    }
    expect(
      huerfanos,
      'estos campos viajan al servidor en cada guardado y ninguna pantalla los ofrece: ' +
        'nadie puede rellenarlos, así que el dato sale siempre vacío y parece que el ' +
        'paciente no lo tiene.',
    ).toEqual([]);
  });
});
