/**
 * Lo que el navegador trae y jsdom no.
 *
 * `ResizeObserver` no existe en jsdom, y **sí** en cualquier navegador desde
 * 2020. No suplirlo no protege de nada: hace que una pantalla perfectamente
 * sana se reporte rota en las pruebas porque una librería de interfaz mide su
 * propio ancho. Lo destapó `agenda-medica` al montarse en el banco de humo, y
 * lo pedirá igual Angular Material —`mat-tab-group` y los observadores del CDK
 * lo usan—, así que no es un parche para PrimeNG: es el entorno.
 *
 * Lo que se suple es **inerte a propósito**: registra, no observa y no dispara
 * nada. Un doble que inventara redimensiones haría pasar por probado un camino
 * que en las pruebas no ocurre nunca (directiva 20: mejor la ausencia declarada
 * que un dato fabricado). Si algún día hay que probar una reacción a un cambio
 * de tamaño, esa prueba se monta su propio doble y lo dice.
 */
class ObservadorDeTamanoInerte implements ResizeObserver {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}

if (typeof globalThis.ResizeObserver === 'undefined') {
  globalThis.ResizeObserver = ObservadorDeTamanoInerte as unknown as typeof ResizeObserver;
}
