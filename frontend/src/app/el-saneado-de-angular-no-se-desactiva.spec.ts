// SPDX-License-Identifier: LicenseRef-Propietario
import { describe, expect, it } from 'vitest';

/**
 * Nadie desactiva el saneado de Angular, y el HTML se pinta en UN solo sitio.
 *
 * Sale de la revisión de seguridad del 2026-08-03. El diálogo de confirmación
 * (`shared/ui/confirmar.service.ts`) pinta su mensaje con `[innerHTML]`, y el
 * comentario que tenía encima decía que el texto «lo escribe la aplicación,
 * nunca el usuario ni el servidor». **Era falso**: nueve de los doce sitios que
 * preguntan interpolan dato del servidor, y uno de ellos es
 * `img.nombre_original`, el nombre de archivo que subió el cliente.
 *
 * Hoy no es explotable, y esa es exactamente la trampa: lo único que lo sostiene
 * es que Angular sanea la vinculación. O sea que el fallo no está a la vista, y
 * llega el día en que alguien quiere marcado que el saneador se come, ve el
 * comentario tranquilizador y «arregla» el problema con
 * `bypassSecurityTrustHtml`. Ese commit convierte un nombre de archivo subido en
 * XSS almacenado dentro de una historia clínica, y no hay nada que se ponga
 * rojo. Un comentario no es una salvaguarda; esto sí.
 *
 * Dos comprobaciones, y las dos son ausencias —por eso se leen los fuentes y no
 * se monta nada—:
 *
 *  1. **Cero llamadas a `bypassSecurityTrust*`** en todo el proyecto. Es la
 *     puerta explícita de salida del saneado; no hay uso legítimo previsto aquí,
 *     y el día que lo haya se discute en la revisión de ese cambio, que es justo
 *     lo que esta prueba obliga a hacer.
 *
 *  2. **`[innerHTML]` vive en un único archivo.** No para prohibirlo —el que hay
 *     viene de `<p-confirmDialog>`, que hacía lo mismo, y se conserva porque las
 *     preguntas llevan `<strong>`— sino para que la superficie no crezca en
 *     silencio: una segunda pieza que pinte HTML sería otra que auditar, y la
 *     segunda es la que nadie recuerda.
 */

// El fuente se lee con el glob de Vite, como el resto de los guardianes: así no
// hace falta declarar `@types/node` solo para esto.
const FUENTES = import.meta.glob('./**/*.ts', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>;

const ARCHIVOS = Object.entries(FUENTES).filter(([ruta]) => !ruta.endsWith('.spec.ts'));

/** El único que puede pintar HTML, y por la razón escrita en su propio archivo. */
const CON_INNERHTML_PERMITIDO = './shared/ui/confirmar.service.ts';

describe('el saneado de Angular no se desactiva', () => {
  it('nadie llama a bypassSecurityTrust*', () => {
    const culpables = ARCHIVOS.filter(([, fuente]) =>
      /bypassSecurityTrust(Html|Script|Url|ResourceUrl|Style)\s*\(/.test(fuente),
    ).map(([ruta]) => ruta);

    expect(
      culpables,
      'bypassSecurityTrust* apaga el saneado de Angular. Si de verdad hace falta, ' +
        'que entre por su propia revisión y no de rebote en otro cambio.',
    ).toEqual([]);
  });

  it('el HTML se pinta en un solo sitio', () => {
    const pintan = ARCHIVOS.filter(([, fuente]) => /\[innerHTML\]/.test(fuente)).map(
      ([ruta]) => ruta,
    );

    expect(
      pintan,
      'apareció otra pieza que pinta HTML. No está prohibido, pero la superficie ' +
        'que hay que auditar no crece sin que nadie lo mire.',
    ).toEqual([CON_INNERHTML_PERMITIDO]);
  });

  it('y el archivo que lo pinta avisa de que su mensaje trae dato del servidor', () => {
    // La comprobación de arriba se cumple sola mientras nadie añada nada; esta
    // es la que impide que se borre el aviso al reescribir el componente, que es
    // como volvería el comentario falso.
    const fuente = FUENTES[CON_INNERHTML_PERMITIDO];
    expect(fuente, 'el diálogo de confirmación cambió de sitio').toBeTypeOf('string');
    expect(fuente).toMatch(/NO ENVOLVER ESTO EN bypassSecurityTrustHtml/);
  });
});
