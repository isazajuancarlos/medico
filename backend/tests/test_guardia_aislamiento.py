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
#:
#: SE IMPORTA, NO SE COPIA, y esto tiene fecha: hasta el 2026-08-05 era una copia
#: a mano con el comentario «se mantiene alineado con
#: `core.aislamiento.COMPARTIDOS`». No se mantuvo. Ese día `Contrato` SALIÓ de
#: COMPARTIDOS —dejó de ser catálogo nacional y pasó a ser el acuerdo privado de
#: un prestador— y esta lista siguió listándolo, así que el guardián dio por
#: legítimo un `db.get(Contrato, …)` sobre un id del cuerpo de la petición. El
#: resultado fue una fuga real (el nombre del contrato del vecino) que este banco
#: existía para cazar y no cazó.
#:
#: Una directiva en prosa —«se mantiene alineado»— es una que se puede
#: desalinear. Importándola, deja de ser posible (directiva 1: no se añade un
#: mecanismo para sostener lo que hace otro; se quita el primero).
from core.aislamiento import COMPARTIDOS as SIN_DUENNO  # noqa: E402

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


def _infracciones_en(fuente: str) -> list[tuple[int, str]]:
    """La regla, sobre el TEXTO. `_infracciones` solo le pasa un archivo.

    === POR QUÉ VA SOBRE TOKENS Y NO SOBRE LÍNEAS ===

    Un `db.get(Modelo` escrito en PROSA —dentro del docstring que explica
    precisamente la fuga que ese `db.get` causó— no es una infracción, y el
    2026-08-05 este guardián se cazó a sí mismo dos veces por eso.

    El primer arreglo fue excluir las LÍNEAS tocadas por un literal de cadena. Y
    es el error clásico: tapó el falso positivo y abrió el falso negativo que
    comparte causa (directiva 23). Un docstring es una línea con una cadena
    dentro, sí — pero también lo es cualquier línea de código que lleve un
    literal. Medido sobre `backend/routers/` en el momento de escribir esto:
    **1 029 de 6 309 líneas dejaron de mirarse (16,3 %), 824 de ellas con código
    ejecutable**, y estas dos pasaban limpias:

        p = db.get(Paciente, {"k": d.paciente_id}["k"])
        h = db.get(Historia, payload.model_dump()["historia_id"])

    La segunda es la forma más natural de reescribir la fuga que este banco
    acababa de cazar.

    La regla correcta no necesita exclusión NINGUNA, y por eso el arreglo quita
    un mecanismo en vez de añadir otro (directiva 1): la secuencia de tokens
    `NAME db` · `OP .` · `NAME get` · `OP (` · `NAME Modelo` **no se emite jamás
    dentro de un `STRING`** —el contenido de una cadena es un solo token—, así
    que buscarla en el flujo de tokens excluye la prosa gratis y ve el código
    esté donde esté. De regalo: caza los dos `db.get` de una misma línea y el
    partido en varias, que la versión por líneas no veía.
    """
    import io
    import tokenize as _tk

    lineas = fuente.splitlines(keepends=True)
    try:
        todos = list(_tk.generate_tokens(io.StringIO(fuente).readline))
    except (_tk.TokenError, IndentationError, SyntaxError):
        # Si no se deja tokenizar, se mira ENTERO con la regex: la dirección
        # segura es avisar de más, nunca callarse (directiva 30).
        fuera = []
        for n, linea in enumerate(lineas, 1):
            m = PATRON.search(linea)
            if m and m.group(1) not in SIN_DUENNO:
                fuera.append((n, m.group(1)))
        return fuera

    fuera: list[tuple[int, str]] = []

    # La MARCA vive en un COMENTARIO, así que se recoge del flujo de tokens y no
    # con un `in` sobre la línea: `MARCA in linea` daba por marcada una línea que
    # llevara ese texto dentro de una cadena.
    marcadas: set[int] = set()
    for tok in todos:
        if tok.type == _tk.COMMENT and MARCA in tok.string:
            razon = tok.string.split(MARCA, 1)[1].strip()
            if len(razon) < RAZON_MINIMA:
                fuera.append((tok.start[0], f"marca sin razón: «{razon}»"))
            else:
                marcadas.add(tok.start[0])

    # Entre `db` y `Modelo` puede haber saltos de línea, sangrados y hasta un
    # comentario si la llamada va partida. Se descartan para que la secuencia se
    # reconozca igual escrita en una línea o en cuatro.
    ruido = {_tk.NL, _tk.NEWLINE, _tk.COMMENT, _tk.INDENT, _tk.DEDENT}
    toks = [t for t in todos if t.type not in ruido]

    for i in range(len(toks) - 4):
        a, b, c, d, e = toks[i : i + 5]
        if not (
            a.type == _tk.NAME and a.string == "db"
            and b.type == _tk.OP and b.string == "."
            and c.type == _tk.NAME and c.string == "get"
            and d.type == _tk.OP and d.string == "("
            and e.type == _tk.NAME and e.string[:1].isupper()
        ):
            continue
        if e.string in SIN_DUENNO:
            continue
        # La marca vale si está en cualquiera de las líneas que ocupa la llamada:
        # con `db.get(` partido, el comentario va donde quepa.
        if marcadas & set(range(a.start[0], e.end[0] + 1)):
            continue
        fuera.append((a.start[0], e.string))

    return sorted(fuera)


def _infracciones(archivo: str) -> list[tuple[int, str]]:
    ruta = os.path.join(RUTA_ROUTERS, archivo)
    with open(ruta, encoding="utf-8") as f:
        return _infracciones_en(f.read())


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


# === EL GUARDIÁN, VIGILADO ===
#
# Los tres casos de arriba miran los routers REALES, y hoy están limpios. Eso
# significa que los tres salen verdes tanto con la regla buena como con una
# regla CIEGA: no discriminan nada sobre el guardián mismo (directiva 33). Un
# banco así ya falló una vez —el 2026-08-05, cuando el arreglo del falso
# positivo abrió el falso negativo que compartía causa y nadie se puso rojo—,
# así que la regla se prueba aquí contra fuente sintética, con los DOS lados.
#
# `_infracciones_en` existe para esto: es la costura por donde entra el texto
# sin pasar por disco, y es la MISMA función que usa `_infracciones`, así que
# un caso rojo aquí falla por la vía real y no por el arnés.

CASOS_DE_LA_REGLA = [
    # (nombre, fuente, ¿debe señalarlo?)
    #
    # --- Los que TIENEN que verse ---
    ("fuga desnuda", "p = db.get(Paciente, pid)", True),
    # Estos dos son la REGRESIÓN de este arreglo. La versión anterior excluía
    # la línea entera si un literal de cadena la tocaba, y como los dos llevan
    # comillas, los dos pasaban limpios: `[]` sobre dos fugas de verdad.
    ("fuga con literal en la misma línea",
     'p = db.get(Paciente, {"k": d.paciente_id}["k"])', True),
    ("fuga tras model_dump()",
     'h = db.get(Historia, payload.model_dump()["historia_id"])', True),
    # La llamada partida no la veía NINGUNA de las dos versiones por líneas.
    ("fuga partida en varias líneas", "p = db.get(\n    Paciente,\n    pid,\n)", True),
    ("dos fugas en una sola línea",
     "a, b = db.get(Paciente, x), db.get(Historia, y)", True),
    ("marca sin razón suficiente", "p = db.get(Paciente, pid)  # aislamiento: ya", True),
    #
    # --- Los que TIENEN que no verse ---
    # El falso positivo original: el guardián se cazó a sí mismo señalando la
    # prosa del docstring que explicaba la fuga.
    ("prosa dentro de un docstring",
     'def f():\n    """Explica la fuga que causaba db.get(Paciente, x)."""\n    return 1',
     False),
    ("el texto de la marca dentro de una cadena",
     'msg = "# aislamiento: esto es un mensaje, no una marca"', False),
    ("marca con razón de verdad",
     "p = db.get(Paciente, pid)  # aislamiento: lo acota el router padre", False),
    ("modelo sin dueño", "c = db.get(%s, cod)" % sorted(SIN_DUENNO)[0], False),
    ("otro objeto que se llama parecido", "x = self.db.get_cached(Paciente, pid)", False),
]


def test_la_regla_discrimina_los_dos_lados():
    """El caso rojo Y el verde. Con solo el rojo pasa igual un instrumento que
    diga que sí a todo; con solo el verde, uno ciego."""
    fallos = []
    for nombre, fuente, debe_verse in CASOS_DE_LA_REGLA:
        se_ve = bool(_infracciones_en(fuente))
        if se_ve != debe_verse:
            esperado = "señalarlo" if debe_verse else "dejarlo pasar"
            fallos.append(f"  «{nombre}»: debía {esperado} y no lo hizo")

    assert not fallos, (
        "la regla del guardián dejó de discriminar:\n" + "\n".join(fallos)
        + "\n\nSi lo que falla son los casos que DEBEN verse, el guardián está "
          "ciego y los routers limpios de arriba no lo prueban: salen verdes "
          "igual (directiva 33)."
    )


def test_el_guardian_no_se_apoya_en_excluir_lineas():
    """El falso negativo tenía UNA causa: excluir de la mirada las líneas que
    llevaban un literal. Se fija que ninguna exclusión así vuelva a entrar,
    porque el síntoma —dos fugas invisibles— no se ve en los routers de hoy."""
    con_comillas = 'p = db.get(Paciente, "el literal que antes lo escondía")'
    assert _infracciones_en(con_comillas) == [(1, "Paciente")], (
        "una llamada con un literal de cadena en la misma línea volvió a "
        "quedar fuera de la mirada del guardián"
    )
