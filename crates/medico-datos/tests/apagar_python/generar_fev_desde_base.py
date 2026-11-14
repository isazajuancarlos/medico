# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Juan Carlos Isaza
#
# El lado PYTHON de la medición de extremo a extremo del FEV-RIPS (#598).
#
# Siembra una base SQLite con el ESQUEMA de SQLModel a partir del mismo `spec`
# JSON que la prueba de Rust usa para sembrar el suyo, genera el documento
# FEV-RIPS leyendo esa base con `rips_completo` + `fev_rips`, y lo imprime como
# JSON COMPACTO por `stdout` —el mismo formato con el que Rust serializa el
# suyo—, para que la prueba de Rust compare las dos CADENAS.
#
# Por qué en Python y no en Rust (directiva 38): su único trabajo es ejecutar el
# sistema Python de verdad —su ORM, sus consultas, su fusión—, que es justo lo
# que se está midiendo. Un puerto a Rust mediría otra cosa. Es el mismo motivo
# por el que `fev_rips_paridad_con_python.rs` invoca a `python3`.
#
# LA EQUIVALENCIA es lo único delicado (ficha #598): los dos esquemas divergen
# —el documento del paciente es `tipo_documento`/`numero_documento` en Rust y
# `tipo_id`/`identificacion` en Python; el destino de urgencias es «DOMICILIO»
# en Rust y «Domicilio» en Python—, así que «sembrar lo mismo» significa sembrar
# el valor SEMÁNTICAMENTE equivalente en la representación de CADA sistema. El
# `spec` es NEUTRAL; aquí se traduce a lo que Python guarda. Si la traducción
# está mal, el FEV sale distinto y la prueba de Rust se pone roja —el error no
# se esconde—.
#
# Uso:  generar_fev_desde_base.py <spec.json> <destino.db> <num_nota>
# Sale 0 e imprime el FEV en stdout; sale != 0 y explica en stderr si algo falta
# (un `DatosIncompletos` es un fallo de la SIEMBRA, no del generador, y hay que
# verlo, no tragarlo — directivas 20 y 30).

import json
import os
import sys
from datetime import date, datetime


def _fecha(texto: str) -> datetime:
    """El `spec` trae las fechas como «AAAA-MM-DD HH:MM:SS» (hora del prestador).

    La prueba de Rust fija la zona del prestador en UTC, así que su conversión
    a UTC es la identidad y guarda ESTE MISMO texto. Aquí se guarda el naive tal
    cual: los dos lados parten del mismo instante de pared, que es la única forma
    de que `fechaInicioAtencion` coincida byte a byte.
    """
    return datetime.strptime(texto, "%Y-%m-%d %H:%M:%S")


def _fecha_dia(texto: str) -> date:
    return datetime.strptime(texto, "%Y-%m-%d").date()


# El destino de urgencias: el `spec` lo trae en la convención de Rust
# (MAYÚSCULAS); Python lo guarda capitalizado y con tilde. Los dos mapean al
# MISMO código de salida del catálogo, así que el FEV coincide; lo que cambia es
# el texto de la columna, que es lo que «cada esquema» quiere decir.
DESTINO_A_PYTHON = {
    "DOMICILIO": "Domicilio",
    "FALLECIDO": "Fallecido",
    "HOSPITALIZACION": "Hospitalización",
    "REMISION": "Remisión",
    "OBSERVACION": "Observación",
    "FUGA": "Fuga",
}


def main() -> int:
    if len(sys.argv) != 4:
        print(f"uso: {sys.argv[0]} <spec.json> <destino.db> <num_nota>", file=sys.stderr)
        return 2
    spec_path, db_path, num_nota = sys.argv[1], sys.argv[2], sys.argv[3]

    with open(spec_path, encoding="utf-8") as f:
        spec = json.load(f)

    # El motor se fija al importar `database`, así que la variable va ANTES de
    # cualquier import del backend. Es lo que hace `conftest.py`.
    os.environ["DATABASE_URL"] = f"sqlite:///{db_path}"

    backend = os.path.join(os.path.dirname(__file__), "..", "..", "..", "..", "backend")
    sys.path.insert(0, os.path.abspath(backend))

    import models  # noqa: F401 — registra las tablas para `create_all`
    from database import engine, init_db
    from sqlmodel import Session

    init_db()

    inst = spec["institucion"]
    prof = spec["profesional"]

    with Session(engine) as s:
        institucion = models.Institucion(
            id=1,
            nit=inst["nit"],
            codigo_habilitacion=inst.get("codigo_habilitacion"),
            codigo_prestador=inst.get("codigo_prestador", inst["nit"]),
            # El resto son NOT NULL del esquema Python y NO tocan el RIPS: relleno
            # explícito, no un dato que se afirme (directiva 20 — no salen al MUV).
            razon_social="IPS de prueba",
            nombre_comercial="IPS de prueba",
            direccion="Calle 1",
            ciudad="Medellín",
            departamento="Antioquia",
            telefono="6040000000",
            email="ips@ejemplo.test",
            representante_legal="Rep Legal",
            identificacion_representante="00000000",
        )
        s.add(institucion)

        medico = models.Usuario(
            id=prof["id"],
            username=f"medico{prof['id']}",
            hashed_password="x",
            nombre_completo="Médico de prueba",
            tipo_documento=prof["tipo_documento"],
            numero_documento=prof["numero_documento"],
            institucion_id=1,
        )
        s.add(medico)

        enfermera = models.Usuario(
            id=spec["enfermera"]["id"],
            username=f"enfermera{spec['enfermera']['id']}",
            hashed_password="x",
            nombre_completo="Enfermera de triage",
            institucion_id=1,
        )
        s.add(enfermera)

        cama = spec["cama"]
        s.add(models.Cama(id=cama["id"], numero=cama["numero"], piso=cama["piso"],
                          servicio=cama["servicio"]))

        for p in spec["pacientes"]:
            s.add(models.Paciente(
                id=p["ref"],
                # El documento del paciente vive en columnas DISTINTAS que en Rust.
                tipo_id=p["tipo_documento"],
                identificacion=p["numero_documento"],
                nombre1="Paciente",
                apellido1=f"Numero{p['ref']}",
                celular="3000000000",
                fecha_nacimiento=_fecha_dia(p["fecha_nacimiento"]),
                sexo=p["sexo"],
                institucion_id=1,
                tipo_usuario=p["tipo_usuario"],
                cod_pais=p["cod_pais"],
                cod_municipio=p["cod_municipio"],
                zona=p["zona"],
                cod_pais_origen=p["cod_pais_origen"],
            ))

        # Las historias se insertan en el ORDEN del `spec`, que Rust construye por
        # (fecha, id) ascendente. Las cinco consultas de `rips_completo` NO llevan
        # `order_by`, así que SQLite las devuelve por rowid = orden de inserción;
        # es lo que hace que la numeración de `consecutivo` coincida con la de
        # Rust. Si este orden se rompe, la prueba se pone roja, que es lo correcto.
        for h in spec["historias"]:
            historia = models.Historia(
                id=h["id"],
                codigo_historia=f"HC-{h['id']}",
                paciente_id=h["paciente_ref"],
                medico_id=prof["id"],
                fecha=_fecha(h["fecha"]),
                motivo_consulta="Motivo",
                enfermedad_actual="Enfermedad",
                plan_tratamiento="Plan",
                diagnostico_principal=h["diagnostico_principal"],
                diagnostico_relacionado1=h.get("diagnostico_relacionado1"),
                cod_consulta_cups=h["cod_consulta_cups"],
                modalidad_grupo_servicio=h["modalidad_grupo_servicio"],
                grupo_servicios=h["grupo_servicios"],
                cod_servicio=h["cod_servicio"],
                finalidad=h["finalidad"],
                causa_externa=h["causa_externa"],
                tipo_diagnostico=h["tipo_diagnostico"],
                valor_servicio=h["valor_servicio"],
                concepto_recaudo=h["concepto_recaudo"],
                genera_incapacidad=h["genera_incapacidad"],
            )
            s.add(historia)

            for o in h.get("ordenes", []):
                s.add(models.Orden(
                    historia_id=h["id"],
                    tipo=o["tipo"],
                    descripcion="Orden",
                    justificacion="Justificación",
                    codigo_cups=o["codigo_cups"],
                    via_ingreso_servicio=o.get("via_ingreso_servicio"),
                    grupo_servicios=o.get("grupo_servicios"),
                    cod_servicio=o.get("cod_servicio"),
                    valor_servicio=o["valor_servicio"],
                    concepto_recaudo=o["concepto_recaudo"],
                ))

            for m in h.get("formulas", []):
                s.add(models.Formula(
                    historia_id=h["id"],
                    medicamento=m["medicamento"],
                    dosis="1 cada 8h",
                    cantidad=m["cantidad"],
                    codigo_cum=m["codigo_cum"],
                    tipo_medicamento=m["tipo_medicamento"],
                    unidad=m["unidad"],
                    dias_tratamiento=m["dias_tratamiento"],
                    valor_unitario=m["valor_unitario"],
                    valor_servicio=m["valor_servicio"],
                    concepto_recaudo=m["concepto_recaudo"],
                ))

        for u in spec.get("urgencias", []):
            s.add(models.AtencionUrgencias(
                id=u["id"],
                paciente_id=u["paciente_ref"],
                triage="Amarillo - Urgente",
                enfermera_triage_id=spec["enfermera"]["id"],
                motivo_consulta_triage="Motivo urgencias",
                fecha_ingreso=_fecha(u["fecha_ingreso"]),
                fecha_egreso=_fecha(u["fecha_egreso"]),
                causa_motivo=u["causa_motivo"],
                diagnostico_ingreso=u["diagnostico_ingreso"],
                diagnostico_egreso=u.get("diagnostico_egreso"),
                destino_salida=DESTINO_A_PYTHON[u["destino_salida"]],
                diagnostico_causa_muerte=u.get("diagnostico_causa_muerte"),
                valor_servicio=u["valor_servicio"],
                concepto_recaudo=u["concepto_recaudo"],
            ))

        for hosp in spec.get("hospitalizaciones", []):
            s.add(models.Hospitalizacion(
                id=hosp["id"],
                paciente_id=hosp["paciente_ref"],
                cama_id=cama["id"],
                medico_tratante_id=prof["id"],
                fecha_ingreso=_fecha(hosp["fecha_ingreso"]),
                fecha_egreso=_fecha(hosp["fecha_egreso"]),
                diagnostico_ingreso=hosp["diagnostico_ingreso"],
                diagnostico_egreso=hosp.get("diagnostico_egreso"),
                via_ingreso=hosp["via_ingreso"],
                estado=hosp["estado"],
                causa_motivo=hosp["causa_motivo"],
                valor_servicio=hosp["valor_servicio"],
                concepto_recaudo=hosp["concepto_recaudo"],
            ))

        s.commit()

    # Generar el FEV leyendo la base recién sembrada.
    import fev_rips

    periodo = spec["periodo"]
    paquete = fev_rips.generar_paquete(
        _fecha_dia(periodo["inicio"]),
        _fecha_dia(periodo["fin"]),
        1,
        num_nota=num_nota,
    )
    # Compacto y sin escapar los no-ASCII: el MISMO formato que el `compacto` de
    # la prueba de Rust (`serde_json::to_string`).
    sys.stdout.write(json.dumps(paquete, ensure_ascii=False, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    sys.exit(main())
