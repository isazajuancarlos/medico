"""Guardia: que no se vuelva a colar un `db.get` sin aislamiento.

La corrección de `core/aislamiento.py` no sirve de nada si el próximo endpoint
se escribe copiando uno viejo. Una directiva en un documento no lo impide; esta
prueba sí.

DEUDA CONOCIDA: la corrección se aplicó primero donde más pesa —`pacientes` e
`historias`, que son los datos de categoría especial—. Los demás routers siguen
pendientes y están listados abajo. La lista solo puede ENCOGER: si alguien
migra un router, quita su nombre y la prueba lo protege desde entonces. Si
alguien añade un `db.get` en un router ya limpio, esta prueba falla.

No se listan excepciones por línea a propósito. Una lista de líneas se
desactualiza en el primer refactor y acaba silenciando lo que debía vigilar.
"""
import os
import re

RUTA_ROUTERS = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "routers"
)

#: Routers todavía sin migrar a `obtener()`. Quitar de aquí al migrarlos.
#: Ver la tarea "aislamiento por institución en los routers restantes".
PENDIENTES_DE_MIGRAR = {
    "admin.py", "agenda_medica.py", "auth.py", "citas.py", "epicrisis.py",
    "estadisticas.py", "evoluciones.py", "facturacion.py", "farmacia.py",
    "hospitalizacion.py", "imagenes_dx.py", "interconsultas.py",
    "laboratorio.py", "nutricion.py", "quirofano.py", "rehabilitacion.py",
    "superadmin.py", "urgencias.py", "usuarios.py",
}

#: Modelos sin dueño: traerlos por id no expone nada de nadie.
#: Se mantiene alineado con `core.aislamiento.COMPARTIDOS`.
SIN_DUENNO = {
    "Cie10", "Cups", "Medicamento", "Especialidad", "Entidad", "ConfigCampo",
    "BloqueHorario", "Cama", "ItemInventario", "Contrato", "TarifaContrato",
    "Institucion", "AuditoriaSuperAdmin", "HistorialPassword", "Token2FA",
}

PATRON = re.compile(r"db\.get\(\s*([A-Z]\w*)")

#: Marca para el `db.get` que sí es legítimo: el que trae un dato COLGADO de un
#: registro ya autorizado (la historia que acaba de pasar por `obtener()` trae
#: su propio paciente, su cita, su médico). Se escribe al final de la línea y
#: obliga a justificarlo una por una, en vez de callar el aviso en bloque.
MARCA = "# aislamiento: ya autorizado por el padre"


def _infracciones(archivo: str) -> list[tuple[int, str]]:
    ruta = os.path.join(RUTA_ROUTERS, archivo)
    with open(ruta, encoding="utf-8") as f:
        lineas = f.readlines()
    fuera = []
    for n, linea in enumerate(lineas, 1):
        if MARCA in linea:
            continue
        m = PATRON.search(linea)
        if m and m.group(1) not in SIN_DUENNO:
            fuera.append((n, m.group(1)))
    return fuera


def _routers():
    return sorted(
        f for f in os.listdir(RUTA_ROUTERS)
        if f.endswith(".py") and f != "__init__.py"
    )


def test_los_routers_migrados_no_usan_db_get_directo():
    sucios = {}
    for archivo in _routers():
        if archivo in PENDIENTES_DE_MIGRAR:
            continue
        fuera = _infracciones(archivo)
        if fuera:
            sucios[archivo] = fuera

    assert not sucios, (
        "estos routers ya estaban migrados y volvieron a traer un registro con "
        "dueño por `db.get` en vez de `core.aislamiento.obtener`:\n"
        + "\n".join(
            f"  routers/{a}:{n} → db.get({modelo}, …)"
            for a, casos in sucios.items() for n, modelo in casos
        )
        + "\n\nUse `obtener(db, Modelo, id, current_user)`: devuelve 404 cuando "
          "el registro es de otra institución."
    )


def test_la_lista_de_pendientes_no_miente():
    """Un router que ya está limpio no debe seguir en la lista de deuda: si se
    queda, deja de estar vigilado sin que nadie lo note."""
    existentes = set(_routers())
    fantasmas = PENDIENTES_DE_MIGRAR - existentes
    assert not fantasmas, f"routers que ya no existen en la lista: {fantasmas}"

    ya_limpios = {a for a in PENDIENTES_DE_MIGRAR if not _infracciones(a)}
    assert not ya_limpios, (
        f"estos routers ya no tienen `db.get` sin aislamiento; quítelos de "
        f"PENDIENTES_DE_MIGRAR para que queden protegidos: {sorted(ya_limpios)}"
    )


def test_pacientes_e_historias_si_estan_protegidos():
    """Lo que se corrigió hoy, fijado explícitamente."""
    for archivo in ("pacientes.py", "historias.py"):
        assert archivo not in PENDIENTES_DE_MIGRAR
        assert _infracciones(archivo) == [], (
            f"routers/{archivo} volvió a tener `db.get` sin aislamiento"
        )
