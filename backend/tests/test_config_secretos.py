"""Que el sistema NO arranque sin secretos de verdad.

`core/config.py` decía `os.getenv("SECRET_KEY", "changeme")`. Con eso, un
despliegue sin `.env` —un contenedor nuevo, una variable mal escrita, un `.env`
borrado por creerlo un sobrante— levantaba igual y firmaba los tokens de sesión
con la cadena `changeme`, que está publicada en este repositorio.

Con la clave de firma, un token se FABRICA. Se emite uno de ADMIN de cualquier
institución y el aislamiento de `core/aislamiento.py` deja de proteger nada: el
intruso no se salta el filtro, entra por la puerta con una credencial propia.

Estas pruebas llaman a la comprobación directamente, con el entorno que quieren.
No pueden hacerlo reimportando el módulo, porque `config` se evalúa una sola vez
por proceso y el resultado quedaría fijado por quien lo importara primero.
"""
import os
import sys
from pathlib import Path

import pytest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")

from core.config import (  # noqa: E402
    CONOCIDOS,
    LARGO_MINIMO,
    ConfiguracionInsegura,
    _exigir,
)


@pytest.fixture
def entorno(monkeypatch):
    def poner(valor):
        if valor is None:
            monkeypatch.delenv("UNA_CLAVE", raising=False)
        else:
            monkeypatch.setenv("UNA_CLAVE", valor)
    return poner


class TestFaltaElSecreto:
    def test_sin_la_variable_no_arranca(self, entorno):
        entorno(None)
        with pytest.raises(ConfiguracionInsegura) as e:
            _exigir("UNA_CLAVE")
        assert "UNA_CLAVE" in str(e.value)

    def test_una_cadena_vacia_cuenta_como_ausente(self, entorno):
        entorno("")
        with pytest.raises(ConfiguracionInsegura):
            _exigir("UNA_CLAVE")

    def test_solo_espacios_tambien(self, entorno):
        entorno("     ")
        with pytest.raises(ConfiguracionInsegura):
            _exigir("UNA_CLAVE")

    def test_el_mensaje_trae_el_comando_para_generarla(self, entorno):
        """Un error que dice qué hacer se arregla; uno que solo se queja invita
        a poner cualquier cosa para salir del paso."""
        entorno(None)
        with pytest.raises(ConfiguracionInsegura) as e:
            _exigir("UNA_CLAVE")
        assert ".env" in str(e.value)
        assert "UNA_CLAVE=" in str(e.value)

    def test_el_mensaje_explica_por_que_no_hay_defecto(self, entorno):
        entorno(None)
        with pytest.raises(ConfiguracionInsegura) as e:
            _exigir("UNA_CLAVE")
        assert "por defecto" in str(e.value)


class TestValoresConocidos:
    @pytest.mark.parametrize("valor", sorted(CONOCIDOS))
    def test_ninguno_de_los_de_relleno_pasa(self, entorno, valor):
        entorno(valor)
        with pytest.raises(ConfiguracionInsegura) as e:
            _exigir("UNA_CLAVE", minimo=1)
        assert "repositorio" in str(e.value)

    def test_changeme_es_uno_de_ellos(self):
        """Era el defecto real de este archivo hasta hoy."""
        assert "changeme" in CONOCIDOS

    def test_no_se_escapa_por_mayusculas(self, entorno):
        entorno("ChangeMe")
        with pytest.raises(ConfiguracionInsegura):
            _exigir("UNA_CLAVE", minimo=1)


class TestLargo:
    def test_una_clave_corta_no_pasa(self, entorno):
        entorno("a" * (LARGO_MINIMO - 1))
        with pytest.raises(ConfiguracionInsegura) as e:
            _exigir("UNA_CLAVE")
        assert "fuerza bruta" in str(e.value)

    def test_justo_en_el_minimo_si_pasa(self, entorno):
        clave = "a" * LARGO_MINIMO
        entorno(clave)
        assert _exigir("UNA_CLAVE") == clave

    def test_el_minimo_no_es_simbolico(self):
        """Con menos de 32, quien capture un token saca la clave sin conexión."""
        assert LARGO_MINIMO >= 32


class TestOperacionNormal:
    """La restricción se prueba contra el uso legítimo, no solo contra el ataque.
    Una comprobación que estorba al que trabaja bien termina desactivada."""

    def test_una_clave_generada_de_verdad_pasa(self, entorno):
        import secrets
        clave = secrets.token_urlsafe(48)
        entorno(clave)
        assert _exigir("UNA_CLAVE") == clave

    def test_se_le_quitan_los_espacios_de_los_bordes(self, entorno):
        """Copiar y pegar de un gestor de contraseñas arrastra un salto de línea.
        Eso no debe tumbar el arranque."""
        clave = "x" * 40
        entorno(f"  {clave}\n")
        assert _exigir("UNA_CLAVE") == clave

    def test_la_clave_de_las_pruebas_pasa(self):
        """Si esto fallara, la comprobación estaría rompiendo la suite entera."""
        assert len(os.environ["SECRET_KEY"]) >= LARGO_MINIMO
        assert os.environ["SECRET_KEY"].lower() not in CONOCIDOS


def test_el_modulo_real_quedo_con_un_secreto_valido():
    """Cierra el círculo: no solo la función, el módulo que se importa."""
    from core.config import JWT_SECRET_KEY, SECRET_KEY

    for nombre, valor in (("SECRET_KEY", SECRET_KEY), ("JWT_SECRET_KEY", JWT_SECRET_KEY)):
        assert valor, f"{nombre} vacía"
        assert valor.lower() not in CONOCIDOS, f"{nombre} es un valor de relleno"
        assert len(valor) >= LARGO_MINIMO, f"{nombre} demasiado corta"


def test_el_ejemplo_no_entrega_un_secreto_que_sirva():
    """`.env.example` NO puede entregar un valor que ESTE validador acepte.

    Es la gemela de `el_ejemplo_no_entrega_un_secreto_que_sirva` de
    `crates/medico-api/src/config.rs`, y existe porque tener solo aquella dejaba
    a medias la regla: el 2026-07-31 el valor que el ejemplo publicaba se añadió
    a la lista negra del binario Rust y NO a `CONOCIDOS`, de modo que un
    despliegue Python que hubiera copiado el ejemplo seguía firmando sus JWT con
    una clave que está en el historial de git. Con ella se fabrica un token de
    superadmin y el aislamiento por institución deja de proteger nada.

    `CONOCIDOS` es una lista negra, o sea una categoría abierta. La regla que sí
    discrimina es esta: el archivo que se invita a copiar se comprueba contra el
    validador del arranque, así que cualquier valor plausible que alguien escriba
    ahí «para que se vea el formato» pone la prueba en rojo.
    """
    ejemplo = Path(__file__).resolve().parent.parent.parent / ".env.example"
    assert ejemplo.exists(), f"no se encontró {ejemplo}"

    valor = None
    for linea in ejemplo.read_text(encoding="utf-8").splitlines():
        if linea.strip().startswith("SECRET_KEY="):
            valor = linea.strip().split("=", 1)[1]
            break
    assert valor is not None, ".env.example tiene que seguir documentando SECRET_KEY"

    with pytest.raises(ConfiguracionInsegura):
        os.environ["UNA_CLAVE_DE_EJEMPLO"] = valor
        try:
            _exigir("UNA_CLAVE_DE_EJEMPLO")
        finally:
            os.environ.pop("UNA_CLAVE_DE_EJEMPLO", None)
