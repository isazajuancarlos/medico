"""Que ningún campo del RIPS vuelva a tener un valor por defecto de conveniencia.

=== POR QUÉ ESTA PRUEBA Y NO UN NOT NULL ===

El diagnóstico intuitivo de este problema es «faltan restricciones NOT NULL en
la base». La capa es la correcta —una comprobación en Pydantic solo cubre la
API, y el `seed.py`, el importador o un UPDATE a mano la esquivan—, pero el
mecanismo aquí es justo el contrario.

Estos campos NUNCA estaban en NULL. El modelo decía:

    cod_consulta_cups: Optional[str] = Field(default="890201")

Con eso, TODA consulta se reportaba con el mismo procedimiento, se hubiera hecho
el que se hubiera hecho. Un NOT NULL no habría cambiado nada: la columna venía
llena. Llena de una constante.

Peor: `auditoria_rips` existe justamente para señalar las atenciones que se
reportarían con valores supuestos, y **no podía verlos**. Busca campos vacíos, y
el defecto impedía que ninguno lo estuviera. Habíamos construido una auditoría
ciega por diseño. Al quitar los defectos pasó de detectar 2 huecos de 6 a
detectar los 6.

La regla, entonces:

  - Lo ESTRUCTURAL —claves ajenas, fechas de creación, identificadores— va NOT
    NULL en la base. No puede faltar nunca y no depende de lo que se capture.
  - Lo que describe UNA OBSERVACIÓN —qué procedimiento se hizo, cuánto valió,
    dónde vive el paciente— NO lleva defecto y SÍ admite nulo. La ausencia tiene
    que poder representarse, verse y bloquear el informe en el momento adecuado.

Y admite nulo por una razón clínica, no técnica: la historia se abre cuando
entra el paciente y el CUPS se sabe cuando termina la atención. Un NOT NULL
obligaría a inventar un valor en el INSERT, que es exactamente cómo llegó ahí el
«890201».
"""
import os
import sys

import pytest

os.environ.setdefault("DATABASE_URL", "sqlite:///./test_aislamiento.db")
os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from auditoria_rips import CAMPOS, auditar  # noqa: E402
from models import Historia, Paciente  # noqa: E402

#: Campos que describen una observación concreta. Si tienen defecto, el sistema
#: reporta una constante como si fuera un dato.
SIN_DEFECTO = {
    Historia: [
        "cod_consulta_cups",          # qué procedimiento se hizo
        "modalidad_grupo_servicio",   # cómo se prestó
        "grupo_servicios",            # de qué grupo
        "concepto_recaudo",           # qué se recaudó
        "valor_servicio",             # cuánto valió
        "cod_servicio",
    ],
    Paciente: [
        "cod_municipio",              # dónde vive
    ],
}


def _defecto(modelo, campo):
    f = modelo.model_fields[campo]
    return f.default


@pytest.mark.parametrize(
    "modelo,campo",
    [(m, c) for m, campos in SIN_DEFECTO.items() for c in campos],
    ids=lambda x: x if isinstance(x, str) else x.__name__,
)
def test_no_tiene_valor_por_defecto(modelo, campo):
    valor = _defecto(modelo, campo)
    assert valor is None, (
        f"{modelo.__name__}.{campo} tiene el valor por defecto {valor!r}.\n"
        f"Eso hace que la columna nunca quede vacía y que se reporte esa "
        f"constante como si fuera un dato observado. `auditoria_rips` no puede "
        f"señalar lo que nunca está vacío: la auditoría se vuelve ciega.\n"
        f"Si el campo no se puede capturar todavía, déjelo en None y que la "
        f"auditoría bloquee el informe."
    )


def test_una_historia_recien_creada_no_es_reportable():
    """El control de todo lo anterior.

    Abrir una historia y no capturar nada más NO puede producir un RIPS válido.
    Si esta prueba empieza a pasar al revés, es que alguien repuso un defecto.
    """
    h = Historia(
        paciente_id=1, medico_id=1, codigo_historia="H",
        motivo_consulta="x", enfermedad_actual="x",
        diagnostico_principal="J449", plan_tratamiento="x",
    )
    h.id = 1

    informe = auditar([h])

    assert not informe.reportable_sin_supuestos
    assert len(informe.huecos) == len(CAMPOS), (
        "la auditoría no está viendo todos los campos sin capturar; "
        "probablemente alguno recuperó su valor por defecto"
    )


def test_la_auditoria_cubre_todo_lo_que_declaramos_sin_defecto():
    """Las dos listas tienen que hablar del mismo conjunto.

    Si se añade un campo a SIN_DEFECTO y no a `auditoria_rips.CAMPOS`, se queda
    sin defecto Y sin quien lo señale: peor que antes, porque ahora sale vacío
    en el archivo.
    """
    auditados = {atributo for atributo, _, _, _ in CAMPOS}
    declarados = set(SIN_DEFECTO[Historia])

    sin_auditar = declarados - auditados
    assert not sin_auditar, (
        f"sin defecto pero nadie los señala: {sorted(sin_auditar)}"
    )


def test_lo_estructural_SI_es_obligatorio():
    """El contrapeso: quitar defectos no es hacerlo todo opcional.

    Las claves ajenas y los datos que no dependen de lo que se capture siguen
    siendo obligatorios, y ahí sí corresponde la restricción en la base.
    """
    for campo in ("paciente_id", "medico_id", "codigo_historia",
                  "diagnostico_principal"):
        assert Historia.model_fields[campo].is_required(), (
            f"Historia.{campo} dejó de ser obligatorio"
        )

    for campo in ("identificacion", "nombre1", "apellido1",
                  "fecha_nacimiento", "sexo"):
        assert Paciente.model_fields[campo].is_required(), (
            f"Paciente.{campo} dejó de ser obligatorio"
        )
