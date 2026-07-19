import { Injectable, computed, inject, signal } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable, tap } from 'rxjs';

/**
 * Qué módulos tiene contratados esta institución.
 *
 * Es la pieza que permite que el MISMO frontend sirva a un consultorio de
 * consulta general y a un hospital: el menú no es una lista fija, se arma con
 * lo que responde `GET /api/modulos`.
 *
 * Tres estados, y hay que distinguirlos porque significan cosas distintas:
 *   habilitado   contratado y funcionando        -> se muestra
 *   pendiente    contratado, aún sin construir   -> se muestra inerte, avisando
 *   no_incluido  no está en su plan              -> no se muestra
 *
 * Mostrar un módulo "pendiente" como si funcionara lleva al usuario a una
 * pantalla que responde 501. Ocultarlo del todo le esconde lo que ya pagó.
 */
export type EstadoModulo = 'habilitado' | 'pendiente' | 'no_incluido';

export interface ModuloEstado {
  modulo: string;
  estado: EstadoModulo;
}

export interface GrupoModulos {
  grupo: string;
  modulos: ModuloEstado[];
}

export interface CatalogoModulos {
  institucion_id: number | null;
  habilitados: string[];
  grupos: GrupoModulos[];
}

@Injectable({ providedIn: 'root' })
export class ModulosService {
  private readonly API = '/api/modulos';
  private http = inject(HttpClient);

  private _catalogo = signal<CatalogoModulos | null>(null);
  private _cargado = signal(false);

  readonly catalogo = this._catalogo.asReadonly();
  readonly cargado = this._cargado.asReadonly();

  /** Estado por módulo, para consultar sin recorrer los grupos. */
  private readonly estados = computed<Record<string, EstadoModulo>>(() => {
    const cat = this._catalogo();
    if (!cat) return {};
    const mapa: Record<string, EstadoModulo> = {};
    for (const g of cat.grupos) {
      for (const m of g.modulos) mapa[m.modulo] = m.estado;
    }
    return mapa;
  });

  cargar(): Observable<CatalogoModulos> {
    return this.http.get<CatalogoModulos>(`${this.API}/`).pipe(
      tap((cat) => {
        this._catalogo.set(cat);
        this._cargado.set(true);
      }),
    );
  }

  limpiar(): void {
    this._catalogo.set(null);
    this._cargado.set(false);
  }

  estado(modulo?: string): EstadoModulo {
    // Sin módulo declarado, el elemento no está sujeto a licencia (p. ej. el
    // panel de administración): se muestra siempre.
    if (!modulo) return 'habilitado';
    // Mientras no haya respuesta del servidor no se oculta nada: esconder el
    // menú entero por una petición lenta es peor que mostrarlo de más.
    if (!this._cargado()) return 'habilitado';
    return this.estados()[modulo] ?? 'no_incluido';
  }

  estaDisponible(modulo?: string): boolean {
    return this.estado(modulo) === 'habilitado';
  }

  estaVisible(modulo?: string): boolean {
    return this.estado(modulo) !== 'no_incluido';
  }
}
