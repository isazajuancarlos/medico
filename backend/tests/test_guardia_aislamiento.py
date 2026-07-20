"""Guardia: que no se vuelva a colar un `db.get` sin aislamiento.

La corrección de `core/aislamiento.py` no sirve de nada si el próximo endpoint
se escribe copiando uno viejo. Una directiva en un documento no lo impide; esta
prueba sí.

Cubre los veinticuatro routers. La lista de deuda quedó vacía.

No se listan excepciones por línea a propósito. Una lista de líneas se
desactualiza en el primer refactor y acaba silenciando lo que debía vigilar.
En su lugar, cada `db.get` legítimo lleva su razón escrita al lado.
"""
import os
import re

RUTA_ROUTERS = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "routers"
)

#: Routers sin migrar. Llegó a tener diecinueve; hoy está VACÍA y todos los
#: routers están vigilados. Si alguna vez vuelve a llenarse, que sea una
#: decisión escrita y no un olvido.
PENDIENTES_DE_MIGRAR: set[str] = set()

#: Modelos sin dueño: traerlos por id no expone nada de nadie.
#: Se mantiene alineado con `core.aislamiento.COMPARTIDOS`.
SIN_DUENNO = {
    "Cie10", "Cups", "Medicamento", "Especialidad", "Entidad", "ConfigCampo",
    "BloqueHorario", "Cama", "ItemInventario", "Contrato", "TarifaContrato",
    "Institucion", "AuditoriaSuperAdmin", "HistorialPassword", "Token2FA",
}

PATRON = re.compile(r"db\.get\(\s*([A-Z]\w*)")

#: Marca para el `db.get` que sí es legítimo, con SU razón escrita al lado:
#:
#:     db.get(Paciente, historia.paciente_id)  # aislamiento: ya autorizado por el padre
#:
#: No es una etiqueta para callar el aviso: la razón es obligatoria y tiene que
#: decir algo. Los casos que aparecieron son tres, y cada uno se defiende
#: distinto —un dato colgado de un registro ya autorizado, un id que sale de una
#: consulta ya filtrada, y el refresh de sesión, donde todavía no hay usuario y
#: el id viene de un token firmado—. Una sola etiqueta para los tres los habría
#: vuelto indistinguibles.
MARCA = "# aislamiento:"
#: Longitud mínima de la razón. Sin esto, `# aislamiento:` a secas sería un
#: silenciador y volveríamos al punto de partida.
RAZON_MINIMA = 15


def _infracciones(archivo: str) -> list[tuple[int, str]]:
    ruta = os.path.join(RUTA_ROUTERS, archivo)
    with open(ruta, encoding="utf-8") as f:
        lineas = f.readlines()
    fuera = []
    for n, linea in enumerate(lineas, 1):
        if MARCA in linea:
            razon = linea.split(MARCA, 1)[1].strip()
            if len(razon) < RAZON_MINIMA:
                fuera.append((n, f"marca sin razón: «{razon}»"))
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
