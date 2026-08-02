import {
  AfterViewInit,
  Component,
  ElementRef,
  Input,
  OnChanges,
  OnDestroy,
  ViewChild,
} from '@angular/core';
import { Chart, ChartData, ChartOptions, ChartType, registerables } from 'chart.js';

// `p-chart` registraba todo por su cuenta al cargarse. Aquí se hace explícito y
// una sola vez: sin registrar, chart.js falla con «"doughnut" is not a
// registered controller», que no dice nada de lo que falta.
Chart.register(...registerables);

/**
 * Si hay algo que dibujar.
 *
 * Va aparte y exportada porque es **la regla**, no un detalle: un
 * `datasets: []` —o un `datasets: [{ data: [] }]`, que es lo que devuelven los
 * `computed` de `estadisticas` mientras la petición viaja— no es un gráfico
 * vacío, es la espera. Dibujarlo pinta unos ejes en cero, y unos ejes en cero
 * en un panel de indicadores se leen como «no hubo atenciones en el periodo»,
 * que es una afirmación que nadie hizo (directiva 20).
 */
export function hayQueDibujar(data: ChartData | null | undefined): boolean {
  if (!data?.datasets?.length) {
    return false;
  }
  return data.datasets.some((s) => (s.data?.length ?? 0) > 0);
}

/**
 * El gráfico —lo que en PrimeNG era `<p-chart>` (15 usos, todos en
 * `estadisticas`)—, aquí directamente sobre `chart.js`.
 *
 * Material no trae gráficos, y `chart.js` **ya era dependencia**: `p-chart` no
 * era más que una envoltura de Angular alrededor de ella. Quitarla no añade un
 * paquete; quita uno.
 *
 * Conserva la firma de `p-chart` a propósito —`type`, `[data]`, `[options]`,
 * `height`— por la misma razón que [`Avisos`] y [`Confirmar`]: cambiar de
 * librería y de vocabulario en el mismo diff mezcla dos cambios, y si algo se
 * rompe no se sabe cuál fue.
 *
 * **Sin datos no dibuja nada**, que es el estado en el que la pantalla se abre
 * siempre: las estadísticas llegan por HTTP. Un gráfico vacío inventado
 * —ejes con cero— se lee como «no hubo atenciones», que es una afirmación.
 */
@Component({
  selector: 'x-grafico',
  standalone: true,
  template: `
    <div class="lienzo" [style.height]="height">
      <canvas #lienzo></canvas>
    </div>
  `,
  styles: [`
    .lienzo { position: relative; width: 100%; }
    canvas { display: block; }
  `],
})
export class GraficoComponent implements AfterViewInit, OnChanges, OnDestroy {
  @Input({ required: true }) type!: ChartType;
  @Input() data: ChartData | null = null;
  /**
   * `ChartOptions<any>` y no `ChartOptions` a secas **porque el componente es
   * polimórfico**: `indexAxis` solo existe en las opciones de barras y
   * `elements.line` en las de líneas, así que una firma cerrada rechazaría la
   * mitad de los quince gráficos. La comprobación de verdad va donde se
   * escriben —`readonly hbarOpts: ChartOptions<'bar'>`—, que es donde alguien
   * teclearía `position: 'abajo'`.
   */
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  @Input() options: ChartOptions<any> = {};
  @Input() height = '260px';

  @ViewChild('lienzo') lienzo?: ElementRef<HTMLCanvasElement>;

  private grafico?: Chart;

  ngAfterViewInit(): void {
    this.pintar();
  }

  ngOnChanges(): void {
    this.pintar();
  }

  ngOnDestroy(): void {
    this.grafico?.destroy();
  }

  /**
   * Actualiza en sitio si ya existe. Destruir y recrear en cada cambio de datos
   * reinicia la animación y pierde el estado de la leyenda —qué series ocultó
   * quien mira—, y estas pantallas refrescan al cambiar el rango de fechas.
   */
  private pintar(): void {
    const canvas = this.lienzo?.nativeElement;
    if (!canvas || !hayQueDibujar(this.data)) {
      return;
    }

    if (this.grafico) {
      this.grafico.data = this.data!;
      this.grafico.options = this.options;
      this.grafico.update();
      return;
    }

    // En jsdom no hay contexto 2D, y chart.js lanza al no poder tomarlo. No se
    // dibuja y ya: un gráfico que no se ve en las pruebas no es un fallo, y
    // tumbar el montaje de la pantalla por esto escondería los que sí lo son.
    if (!canvas.getContext('2d')) {
      return;
    }

    this.grafico = new Chart(canvas, {
      type: this.type,
      data: this.data!,
      options: this.options,
    });
  }
}
