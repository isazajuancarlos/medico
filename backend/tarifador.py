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


def _tarifa_propia(db, institucion_id: Optional[int], cups: str, cuando: date):
    """La lista de precios de la institución, vigente ese día."""
    from sqlmodel import select, or_

    from models import TarifaInstitucion

    if institucion_id is None:
        return None
    return db.exec(
        select(TarifaInstitucion).where(
            TarifaInstitucion.institucion_id == institucion_id,
            TarifaInstitucion.codigo_servicio == cups,
            TarifaInstitucion.vigente_desde <= cuando,
            or_(TarifaInstitucion.vigente_hasta == None,  # noqa: E711
                TarifaInstitucion.vigente_hasta >= cuando),
        )
    ).first()


def resolver(
    db,
    *,
    cups: Optional[str],
    entidad_id: Optional[int] = None,
    institucion_id: Optional[int] = None,
    fecha: Optional[date] = None,
) -> ValorServicio:
    """Valor del servicio vigente el día de la atención.

    Busca en este orden, del acuerdo más específico al más general:

      1. El CONTRATO de la entidad del paciente, si lo hay. Es lo que aplica en
         una IPS que negocia con EPS.
      2. La LISTA DE PRECIOS DE LA INSTITUCIÓN. Es lo que aplica en un
         consultorio pequeño, que no negocia contratos y tiene tarifa única.
      3. Nada: devuelve el motivo y deja que la auditoría lo señale.

    El orden importa: si hay contrato, manda el contrato, porque es lo pactado.
    La tarifa propia entra cuando no hay contrato o cuando el CUPS no está en él.

    `fecha` es la de la ATENCIÓN, no la de hoy: un informe rehecho el año que
    viene debe dar el mismo número.
    """
    from sqlmodel import select

    from models import TarifaContrato

    cuando = fecha or date.today()

    if not cups:
        return ValorServicio(None, motivo="la atención no tiene código CUPS")

    contratos = _contratos_vigentes(db, entidad_id, cuando)

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

    # Sin contrato aplicable: la tarifa propia de la institución.
    propia = _tarifa_propia(db, institucion_id, cups, cuando)
    if propia is not None:
        valor = int(round(float(propia.valor_tarifa)))
        return ValorServicio(
            valor=valor,
            pago_moderador=int(round(valor * (propia.copago_porcentaje or 0) / 100.0)),
            contrato="tarifa propia",
        )

    if contratos:
        motivo = (
            f"el CUPS {cups} no está en ningún contrato vigente "
            f"ni en la tarifa propia de la institución"
        )
    else:
        motivo = (
            f"no hay tarifa para el CUPS {cups} al {cuando.isoformat()}: "
            f"ni contrato de la entidad, ni lista de precios de la institución"
        )
    return ValorServicio(None, motivo=motivo)


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
    institucion_id = getattr(paciente, "institucion_id", None) if paciente else None
    fecha = getattr(historia, "fecha", None)
    if hasattr(fecha, "date"):
        fecha = fecha.date()

    resultado = resolver(
        db,
        cups=getattr(historia, "cod_consulta_cups", None),
        entidad_id=entidad_id,
        institucion_id=institucion_id,
        fecha=fecha,
    )

    if resultado.resuelto:
        historia.valor_servicio = resultado.valor
        historia.valor_pago_moderador = resultado.pago_moderador

    return resultado
