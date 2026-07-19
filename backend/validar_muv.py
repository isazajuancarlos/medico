#!/usr/bin/env python3
"""Ciclo completo contra el MUV: generar -> autenticar -> cargar -> CUV.

    # 1. Solo generar y guardar el archivo, sin tocar la red
    python validar_muv.py --generar /tmp/prueba.json

    # 2. Ciclo completo contra el contenedor de PRUEBAS
    python validar_muv.py --enviar --url https://localhost:9443 --sin-verificar-tls

Las credenciales se leen del entorno y NUNCA de la línea de órdenes, que queda
en el historial del shell:

    MUV_TIPO_DOC=CC MUV_NUM_DOC=... MUV_CLAVE=... MUV_NIT=... MUV_TIPO_USUARIO=PIN

=== DOS LÍMITES QUE NO SON TÉCNICOS ===

**Las credenciales son del prestador.** MinSalud confirmó (caso 2026403888) que
el acceso está atado a una persona registrada en Mi Seguridad Social y SISPRO y
vinculada a la entidad. No existe credencial de integrador: este script se corre
con las de él, en su equipo o con su autorización.

**Los datos van SINTÉTICOS.** El ambiente de pruebas tiene controles más débiles
que producción y una finalidad distinta de aquella para la que se recogieron las
historias. Enviar pacientes reales ahí es lo que la Ley 1581 no quiere para datos
de salud. Por eso `--enviar` usa el generador sintético y hay que pasar
`--archivo` explícitamente para mandar otra cosa: que sea una decisión, no un
descuido.
"""

from __future__ import annotations

import argparse
import json
import os
import sys

from datos_sinteticos import paquete_sintetico
from fev_rips import archivo_rips, contar_atenciones, escribir_archivo
from muv_client import (
    Credenciales, MuvClient, MuvCredencialesInvalidas, MuvError, MuvNoDisponible,
    TransporteUrllib,
)


def credenciales_del_entorno() -> Credenciales:
    faltan = [v for v in ("MUV_TIPO_DOC", "MUV_NUM_DOC", "MUV_CLAVE", "MUV_NIT")
              if not os.getenv(v)]
    if faltan:
        raise SystemExit(
            f"faltan variables de entorno: {', '.join(faltan)}\n"
            f"No se pasan por la línea de órdenes: quedarían en el historial."
        )
    return Credenciales(
        tipo_documento=os.environ["MUV_TIPO_DOC"],
        numero_documento=os.environ["MUV_NUM_DOC"],
        clave=os.environ["MUV_CLAVE"],
        nit=os.environ["MUV_NIT"],
        tipo_usuario=os.getenv("MUV_TIPO_USUARIO") or None,
    )


def informe(resultado) -> int:
    """Imprime el resultado y devuelve el código de salida."""
    print()
    if resultado.aceptado:
        print(f"✅ ACEPTADO — CUV {resultado.cuv}")
        print(f"   proceso {resultado.proceso_id} · ambiente {resultado.ambiente}")
    else:
        print("❌ RECHAZADO — no hay CUV")

    if resultado.bloqueantes:
        print(f"\n{len(resultado.bloqueantes)} hallazgo(s) que IMPIDEN el CUV:")
        for h in resultado.bloqueantes:
            print(f"  [{h.codigo}] {h.descripcion}")
            if h.path_fuente:
                print(f"      en {h.path_fuente}")
            if h.observaciones:
                print(f"      {h.observaciones}")

    if resultado.notificaciones:
        print(f"\n{len(resultado.notificaciones)} notificación(es) — no bloquean, "
              f"pero desde el 1-jun-2026 varias reglas de este grupo pasaron a "
              f"causal de rechazo:")
        for h in resultado.notificaciones[:10]:
            print(f"  [{h.codigo}] {h.descripcion}")
            if h.path_fuente:
                print(f"      en {h.path_fuente}")
        if len(resultado.notificaciones) > 10:
            print(f"  ... y {len(resultado.notificaciones) - 10} más")

    campos = resultado.campos_a_corregir()
    if campos:
        print(f"\nCampos señalados ({len(campos)}):")
        for ruta, hs in sorted(campos.items())[:15]:
            print(f"  {ruta}  <- {', '.join(h.codigo for h in hs)}")

    return 0 if resultado.aceptado else 1


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--generar", metavar="RUTA",
                    help="genera un archivo sintético y sale, sin tocar la red")
    ap.add_argument("--enviar", action="store_true",
                    help="ciclo completo contra el contenedor")
    ap.add_argument("--archivo", metavar="RUTA",
                    help="enviar ESTE archivo en vez del sintético. Si lleva datos "
                         "reales, no debe ir al ambiente de pruebas.")
    ap.add_argument("--url", default="https://localhost:9443")
    ap.add_argument("--pacientes", type=int, default=10)
    ap.add_argument("--nota", default="PRUEBA1",
                    help="numNota: obligatorio y NO repetible entre envíos")
    ap.add_argument("--sin-verificar-tls", action="store_true",
                    help="para el certificado autofirmado del contenedor local. "
                         "Solo defendible porque la conexión no sale de la máquina.")
    args = ap.parse_args()

    if args.archivo:
        with open(args.archivo, encoding="utf-8-sig") as f:
            crudo = json.load(f)
        paquete = crudo if "rips" in crudo else {"rips": crudo, "xmlFevFile": ""}
    else:
        paquete = paquete_sintetico(args.pacientes, num_nota=args.nota)

    rips = archivo_rips(paquete)
    print(f"usuarios: {len(rips['usuarios'])}")
    print(f"atenciones: {
        {k: v for k, v in contar_atenciones(paquete).items() if v} }")
    print(f"tipoNota: {rips['tipoNota']} · numNota: {rips['numNota']}")

    if args.generar:
        escribir_archivo(paquete, args.generar)
        print(f"\n✅ escrito en {args.generar} (UTF-8 con BOM, como el conversor)")
        return 0

    if not args.enviar:
        ap.error("hay que indicar --generar o --enviar")

    if args.archivo:
        print("\n⚠️  Se enviará un archivo propio. Si contiene pacientes reales, "
              "NO debe ir al ambiente de pruebas.")

    cliente = MuvClient(
        args.url,
        TransporteUrllib(verificar_tls=not args.sin_verificar_tls),
    )
    try:
        cliente.login(credenciales_del_entorno())
        print("✅ autenticado")
        return informe(cliente.cargar(paquete))
    except MuvCredencialesInvalidas as e:
        print(f"\n❌ credenciales: {e}", file=sys.stderr)
        return 2
    except MuvNoDisponible as e:
        print(f"\n❌ el contenedor no respondió: {e}", file=sys.stderr)
        return 3
    except MuvError as e:
        print(f"\n❌ {e}", file=sys.stderr)
        return 4


if __name__ == "__main__":
    raise SystemExit(main())
