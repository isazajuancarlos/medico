"""El RIPS de una institución no puede contener pacientes de otra.

Las pruebas que sí generan un archivo pasan `permitir_supuestos=True`: aquí se
mide el AISLAMIENTO, y exigir además la historia completa mezclaría dos cosas
que fallan por motivos distintos. Lo contrario —que sin ese permiso no se
genere— se prueba en `test_rips_sin_supuestos.py`.

`aislamiento.py` cerró los 24 routers, pero este generador se le escapó: la
fuga no está en la consulta que arma el router, sino en un módulo que el router
llama. `routers/rips.py` invoca `exportar_rips_completo(fecha_inicio, fecha_fin)`
sin pasar `institucion_id`, y dentro cada generador hace

    if institucion_id:
        q = q.where(Paciente.institucion_id == institucion_id)

o sea: **sin institución, no filtra nada**. El administrador de cualquier
institución obtenía las atenciones de TODAS, etiquetadas con el NIT de la
primera fila de la tabla `institucion`, que es lo que devuelve el respaldo de
`_get_institucion`.

Son dos daños a la vez, y ninguno se nota mirando el archivo:

  - datos clínicos de terceros salen del sistema (Ley 1581, categoría especial);
  - la factura de A incluye atenciones de B, así que ni el cobro ni la glosa
    cuadran con la realidad de ninguno de los dos.

El filtro opcional es el problema de fondo: un parámetro que por omisión abre
todo es una fuga esperando a que alguien olvide pasarlo. Aquí se exige.
"""
import os
import sys
from datetime import date, datetime

import pytest

os.environ.setdefault("SECRET_KEY", "clave-de-prueba-con-mas-de-32-caracteres-ok")
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

RUTA_BD = "./test_rips_aislamiento.db"


@pytest.fixture
def bd(monkeypatch):
    """Dos instituciones, un paciente y una atención en cada una.

    Con su propio archivo: las pruebas del backend comparten una sola BD por
    accidente (tarea #79) y borrar la ajena deja treinta fallos que no tienen
    nada que ver con lo que se está probando.
    """
    from sqlmodel import Session, SQLModel, create_engine

    import database
    import rips_completo
    from models import (
        Historia, Institucion, Paciente, RolUsuario, Usuario,
    )

    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)
    motor = create_engine(f"sqlite:///{RUTA_BD}")
    SQLModel.metadata.create_all(motor)
    monkeypatch.setattr(database, "engine", motor)
    monkeypatch.setattr(rips_completo, "engine", motor)

    with Session(motor) as db:
        for i, nombre in [(1, "Consultorio Norte"), (2, "Clínica Sur")]:
            db.add(Institucion(
                id=i, nit=f"90000000{i}", razon_social=nombre,
                nombre_comercial=nombre, direccion="c", ciudad="Medellín",
                departamento="Antioquia", telefono="6", email=f"{i}@x.co",
                representante_legal="R", identificacion_representante="1",
                codigo_prestador=f"0500{i}"))
            db.add(Usuario(id=i, username=f"m{i}", hashed_password="h",
                           nombre_completo=f"Médico {i}", email=f"m{i}@x.co",
                           rol=RolUsuario.MEDICO, institucion_id=i, activo=True))
        for pid, inst in [(10, 1), (20, 2)]:
            db.add(Paciente(id=pid, institucion_id=inst, identificacion=f"CC{pid}",
                            nombre1="N", apellido1=f"Inst{inst}",
                            fecha_nacimiento=date(1990, 1, 1), sexo="M",
                            celular="3", cod_municipio="05001"))
            db.add(Historia(id=pid, paciente_id=pid, medico_id=inst,
                            codigo_historia=f"H{pid}", motivo_consulta="x",
                            enfermedad_actual="x", diagnostico_principal="J449",
                            plan_tratamiento="x",
                            fecha=datetime(2026, 6, 15, 9, 0)))
        db.commit()
    yield motor
    if os.path.exists(RUTA_BD):
        os.remove(RUTA_BD)


DESDE, HASTA = date(2026, 6, 1), date(2026, 6, 30)


def _documentos(ac):
    """Los documentos de identificación que aparecen en un archivo AC."""
    return {u["numDocumentoIdentificacion"] for u in ac["usuarios"]}


class TestLaFuga:
    def test_el_AC_de_una_institucion_solo_trae_SUS_pacientes(self, bd):
        import rips_completo
        ac = rips_completo.generar_archivo_ac(DESDE, HASTA, institucion_id=1,
                                                permitir_supuestos=True)
        assert _documentos(ac) == {"CC10"}, "se coló un paciente de otra institución"

    def test_y_el_de_la_otra_solo_los_suyos(self, bd):
        import rips_completo
        ac = rips_completo.generar_archivo_ac(DESDE, HASTA, institucion_id=2,
                                                permitir_supuestos=True)
        assert _documentos(ac) == {"CC20"}

    def test_el_NIT_del_archivo_es_el_de_la_institucion_pedida(self, bd):
        """Etiquetar el archivo con el NIT de otra institución convierte la fuga
        en una factura ajena."""
        import rips_completo
        assert rips_completo.generar_archivo_ac(
            DESDE, HASTA, institucion_id=2,
            permitir_supuestos=True)["numDocumentoIdObligado"] == "900000002"

    def test_OMITIR_la_institucion_ya_ni_compila(self, bd):
        """La primera barrera la pone el intérprete: sin valor por defecto, el
        llamado que la olvide —como hacía `routers/rips.py`— revienta en el
        acto en vez de devolver las atenciones de todas."""
        import rips_completo
        with pytest.raises(TypeError):
            rips_completo.generar_archivo_ac(DESDE, HASTA)

    def test_pasarla_VACIA_a_proposito_tampoco_pasa(self, bd):
        """La segunda barrera es para el valor que llega de fuera: un usuario
        sin institución asignada da `None`, y `None` no puede significar
        «todas»."""
        import rips_completo
        for vacio in (None, 0):
            with pytest.raises(rips_completo.InstitucionRequerida):
                rips_completo.generar_archivo_ac(DESDE, HASTA, institucion_id=vacio)

    def test_una_institucion_inexistente_tampoco_pasa(self, bd):
        """Sin esto, un id equivocado devolvería un archivo vacío con el NIT de
        la primera institución de la tabla: un RIPS de nadie, a nombre de otro."""
        import rips_completo
        with pytest.raises(rips_completo.InstitucionRequerida):
            rips_completo.generar_archivo_ac(DESDE, HASTA, institucion_id=99)


class TestTodosLosGeneradores:
    """No basta con arreglar el AC: son seis archivos y el que se olvide filtra."""

    @pytest.mark.parametrize("nombre", [
        "generar_archivo_ac", "generar_archivo_ap", "generar_archivo_am",
        "generar_archivo_au", "generar_archivo_ah", "generar_archivo_ct",
    ])
    def test_ninguno_genera_sin_institucion(self, bd, nombre):
        import rips_completo
        generador = getattr(rips_completo, nombre)
        with pytest.raises(TypeError):
            generador(DESDE, HASTA)
        with pytest.raises(rips_completo.InstitucionRequerida):
            generador(DESDE, HASTA, institucion_id=None)

    def test_el_CT_cuenta_SOLO_los_registros_de_la_institucion(self, bd):
        """El CT declara cuántos registros lleva el envío. Si contara sin
        filtrar mientras los otros cinco archivos sí filtran, el número no
        cuadraría con el contenido y el MUV rechaza el paquete entero.

        Se llama SIN `conteos` a propósito: por ahí es por donde el CT hace sus
        propias consultas, y es la rama que se quedó sin filtro.
        """
        import rips_completo
        ct = rips_completo.generar_archivo_ct(DESDE, HASTA, institucion_id=1)
        assert ct["control"]["registrosConsultas"] == 1, (
            "el CT está contando las atenciones de las dos instituciones")
        assert ct["control"]["numRegistros"] == 1

    def test_el_CT_cuadra_con_lo_que_traen_los_otros_archivos(self, bd):
        """La comprobación que de verdad importa: el conteo declarado y el
        contenido real tienen que salir del mismo universo."""
        import rips_completo
        paquete = rips_completo.exportar_rips_completo(DESDE, HASTA, institucion_id=1,
                                                       permitir_supuestos=True)
        reales = sum(
            len(u["servicios"].get(clave, []))
            for archivo, clave in (("AC", "consultas"), ("AP", "procedimientos"),
                                   ("AM", "medicamentos"), ("AU", "urgencias"),
                                   ("AH", "hospitalizacion"))
            for u in paquete[archivo]["usuarios"]
        )
        assert paquete["CT"]["control"]["numRegistros"] == reales

    def test_el_paquete_completo_tampoco(self, bd):
        import rips_completo
        with pytest.raises(TypeError):
            rips_completo.exportar_rips_completo(DESDE, HASTA)
        with pytest.raises(rips_completo.InstitucionRequerida):
            rips_completo.exportar_rips_completo(DESDE, HASTA, institucion_id=None)


class TestElRouterPasaLaInstitucion:
    def test_la_firma_ya_no_permite_omitirla(self):
        """La prueba de que no se puede volver a olvidar: si `institucion_id`
        vuelve a tener valor por defecto, esto falla."""
        import inspect

        import rips_completo
        for nombre in ("generar_archivo_ac", "generar_archivo_ap",
                       "generar_archivo_am", "generar_archivo_au",
                       "generar_archivo_ah", "generar_archivo_ct",
                       "exportar_rips_completo"):
            firma = inspect.signature(getattr(rips_completo, nombre))
            p = firma.parameters["institucion_id"]
            assert p.default is inspect.Parameter.empty, (
                f"{nombre} vuelve a permitir omitir la institución")

    def test_el_router_usa_la_del_usuario_autenticado(self):
        """Y no una que venga por parámetro, que dejaría pedir la de otro."""
        import re
        from pathlib import Path
        fuente = Path(__file__).resolve().parent.parent / "routers" / "rips.py"
        texto = fuente.read_text(encoding="utf-8")
        assert re.search(r"institucion_id\s*=\s*current_user\.institucion_id", texto), (
            "routers/rips.py no está pasando la institución del usuario")
