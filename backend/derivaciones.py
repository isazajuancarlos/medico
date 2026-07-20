"""Datos que no hay que pedir porque ya están en otro sitio.

Es la contrapartida de haber quitado los defectos. Cada uno que se quitó hizo la
captura más lenta, y bajo la presión de tiempo de una consulta lo lento se
rodea: se escribe cualquier cosa con tal de cerrar la nota. Se cambiaría un dato
falso por uno basura, con más fricción.

La salida no es exigir más sino pedir menos: **derivar lo que ya se puede
saber**. Un valor derivado cuesta cero segundos al clínico, es reproducible y
—esto es lo que lo separa de un defecto— queda marcado como `DERIVADO`, así que
cualquiera puede ver de dónde salió y desconfiar de él si desconfía del origen.

Cada función devuelve `None` cuando no puede derivar. No aproxima: derivar mal
es exactamente el problema que se está resolviendo.
"""

from __future__ import annotations

from typing import Any, Optional

import divipola
import procedencia as proc


def municipio_desde_texto(ciudad: Optional[str],
                          departamento: Optional[str] = None) -> Optional[str]:
    """Código DIVIPOLA a partir del nombre de la ciudad que ya se capturó.

    `Paciente` guarda `ciudad_residencia` y `departamento_residencia` como texto
    libre desde el sistema viejo. Si el nombre identifica un municipio sin
    ambigüedad, no hay por qué pedir el código.

    Si hay más de un municipio con ese nombre —hay varios «San Juan»— devuelve
    `None` y que lo resuelva quien captura: elegir el primero sería inventar.
    """
    if not (ciudad or "").strip():
        return None

    candidatos = divipola.buscar_por_nombre(ciudad, limite=50)
    if departamento:
        deptos = {c: n for c, n in divipola.departamentos().items()}
        codigos = [c for c, n in deptos.items()
                   if n.lower() == departamento.strip().lower()]
        if codigos:
            candidatos = [m for m in candidatos if m.departamento == codigos[0]]

    exactos = [m for m in candidatos
               if m.nombre.strip().lower() == ciudad.strip().lower()]
    if len(exactos) == 1:
        return exactos[0].codigo
    if len(candidatos) == 1:
        return candidatos[0].codigo
    return None


def completar_municipio(paciente: Any) -> Optional[str]:
    """Deriva y marca el municipio del paciente, si se puede.

    No pisa lo que ya esté: un dato confirmado por una persona manda sobre uno
    derivado, siempre.
    """
    if proc.tiene_valor(paciente, "cod_municipio"):
        return None
    codigo = municipio_desde_texto(
        getattr(paciente, "ciudad_residencia", None),
        getattr(paciente, "departamento_residencia", None),
    )
    if codigo is None:
        return None
    paciente.cod_municipio = codigo
    proc.marcar(paciente, "cod_municipio", proc.Procedencia.DERIVADO)
    return codigo
