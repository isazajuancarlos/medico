"""Resuelve el valor de un servicio, para que nadie lo escriba a mano.

Hoy el prestador GENERA el archivo y después EDITA EL JSON para poner el valor
de cada servicio. Eso es trabajo manual sobre un archivo de miles de líneas,
justo el sitio donde un error no se ve y llega como glosa.

El valor ya existe en el sistema: `Contrato` -> `TarifaContrato`, por código
CUPS. Lo que faltaba era usarlo.

=== LA DECISIÓN QUE MÁS IMPORTA: SE CONGELA AL PRESTAR EL SERVICIO ===

El valor se resuelve UNA VEZ, cuando se registra la atención, y se guarda en la
historia. NO se consulta la tarifa al generar el informe.

Es la diferencia entre un sistema que se puede auditar y uno que no. Las tarifas
cambian —cada año, y a mitad de año si se renegocia el contrato—. Si el informe
de marzo se generara con la tarifa de hoy, dos ejecuciones del mismo periodo
darían cifras distintas, y ninguna correspondería a lo que se pactó cuando se
atendió al paciente. Congelarlo es también lo que permite responder una glosa:
el valor reportado es el que estaba vigente ese día, y queda constancia.

=== CUANDO NO HAY TARIFA, NO SE INVENTA ===

Si no hay contrato vigente o el CUPS no está en él, devuelve None. El generador
lo dejará vacío y `auditoria_rips.auditar()` lo señalará antes de enviar. Poner
un cero —que es lo que hacía el sistema— produce un archivo que pasa la
validación de estructura y no le paga a nadie.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date
from typing import Any, Optional


@dataclass(frozen=True)
class ValorServicio:
    """Lo que se cobra por una atención, con su origen."""

    valor: Optional[int]
    pago_moderador: int = 0
    #: De qué contrato salió. Sirve para explicar una glosa.
    contrato: Optional[str] = None
    #: Por qué no se pudo resolver, si no se pudo.
    motivo: Optional[str] = None

    @property
    def resuelto(self) -> bool:
        return self.valor is not None


def _contratos_vigentes(db, entidad_id: Optional[int], cuando: date):
    """Contratos activos de la entidad en esa fecha."""
    from sqlmodel import select

    from models import Contrato

    if entidad_id is None:
        return []
    consulta = select(Contrato).where(
        Contrato.entidad_id == entidad_id,
        Contrato.activo == True,  # noqa: E712 — SQLModel necesita la comparación
        Contrato.fecha_inicio <= cuando,
        Contrato.fecha_fin >= cuando,
    )
    return list(db.exec(consulta).all())


def resolver(
    db,
    *,
    cups: Optional[str],
    entidad_id: Optional[int],
    fecha: Optional[date] = None,
) -> ValorServicio:
    """Valor del servicio según el contrato vigente el día de la atención.

    `fecha` es la de la ATENCIÓN, no la de hoy: un informe rehecho el año que
    viene debe dar el mismo número.
    """
    from sqlmodel import select

    from models import TarifaContrato

    cuando = fecha or date.today()

    if not cups:
        return ValorServicio(None, motivo="la atención no tiene código CUPS")

    contratos = _contratos_vigentes(db, entidad_id, cuando)
    if not contratos:
        return ValorServicio(
            None,
            motivo=f"no hay contrato vigente al {cuando.isoformat()} para esa entidad",
        )

    for contrato in contratos:
        tarifa = db.exec(
            select(TarifaContrato).where(
                TarifaContrato.contrato_id == contrato.id,
                TarifaContrato.codigo_servicio == cups,
            )
        ).first()
        if tarifa is None:
            continue

        base = float(tarifa.valor_tarifa)
        # El incremento pactado sobre la tarifa base (ISS, SOAT…). Un contrato
        # «ISS + 30 %» guarda la tarifa ISS y el 30 aparte.
        base *= 1 + (contrato.porcentaje_incremento or 0) / 100.0
        valor = int(round(base))
        copago = int(round(valor * (tarifa.copago_porcentaje or 0) / 100.0))

        return ValorServicio(
            valor=valor,
            pago_moderador=copago,
            contrato=contrato.numero_contrato,
        )

    return ValorServicio(
        None,
        motivo=f"el CUPS {cups} no está en ningún contrato vigente",
    )


def aplicar_a_historia(db, historia: Any, *, forzar: bool = False) -> ValorServicio:
    """Resuelve y guarda el valor en la historia, sin hacer commit.

    Por defecto NO pisa un valor ya puesto: si alguien lo capturó a mano o se
    corrigió tras una glosa, esa decisión gana sobre la tarifa. `forzar` sirve
    para recalcular a propósito.

    El commit lo hace quien tiene la sesión, como en el resto del sistema.
    """
    if historia.valor_servicio and not forzar:
        return ValorServicio(
            valor=historia.valor_servicio,
            pago_moderador=historia.valor_pago_moderador or 0,
            motivo="ya tenía valor; no se sobrescribe",
        )

    paciente = getattr(historia, "paciente", None)
    entidad_id = getattr(paciente, "entidad_id", None) if paciente else None
    fecha = getattr(historia, "fecha", None)
    if hasattr(fecha, "date"):
        fecha = fecha.date()

    resultado = resolver(
        db,
        cups=getattr(historia, "cod_consulta_cups", None),
        entidad_id=entidad_id,
        fecha=fecha,
    )

    if resultado.resuelto:
        historia.valor_servicio = resultado.valor
        historia.valor_pago_moderador = resultado.pago_moderador

    return resultado
