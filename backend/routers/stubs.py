"""
Stub routers para módulos en desarrollo.
Retornan listas vacías o mensaje 501 Not Implemented.
"""
from fastapi import APIRouter
from schemas.common import MsgResponse

def make_stub_router(prefix: str, tag: str) -> APIRouter:
    router = APIRouter(prefix=f"/{prefix}", tags=[tag])

    @router.get("/", response_model=list)
    def list_items():
        return []

    return router


evoluciones_router = make_stub_router("evoluciones", "evoluciones")
consentimiento_router = make_stub_router("consentimiento", "consentimiento")
incapacidades_router = make_stub_router("incapacidades", "incapacidades")
remisiones_router = make_stub_router("remisiones", "remisiones")
laboratorio_router = make_stub_router("laboratorio", "laboratorio")
imagenes_router = make_stub_router("imagenes", "imagenes")
banco_sangre_router = make_stub_router("banco-sangre", "banco_sangre")
quirofano_router = make_stub_router("quirofano", "quirofano")
nota_operatoria_router = make_stub_router("nota-operatoria", "nota_operatoria")
preanestesia_router = make_stub_router("preanestesia", "preanestesia")
recuperacion_router = make_stub_router("recuperacion", "recuperacion")
interconsultas_router = make_stub_router("interconsultas", "interconsultas")
epicrisis_router = make_stub_router("epicrisis", "epicrisis")
contrarreferencia_router = make_stub_router("contrarreferencia", "contrarreferencia")
nutricion_router = make_stub_router("nutricion", "nutricion")
rehabilitacion_router = make_stub_router("rehabilitacion", "rehabilitacion")
trabajo_social_router = make_stub_router("trabajo-social", "trabajo_social")
telemedicina_router = make_stub_router("telemedicina", "telemedicina")
vacunacion_router = make_stub_router("vacunacion", "vacunacion")
prenatal_router = make_stub_router("prenatal", "prenatal")
crecimiento_desarrollo_router = make_stub_router("crecimiento-desarrollo", "crecimiento_desarrollo")
salud_reproductiva_router = make_stub_router("salud-reproductiva", "salud_reproductiva")
cert_defuncion_router = make_stub_router("certificado-defuncion", "cert_defuncion")
registro_nacimiento_router = make_stub_router("registro-nacimiento", "registro_nacimiento")
turnos_router = make_stub_router("turnos", "turnos")
comites_router = make_stub_router("comites", "comites")
equipos_medicos_router = make_stub_router("equipos-medicos", "equipos_medicos")
indicadores_router = make_stub_router("indicadores", "indicadores")
infecciones_router = make_stub_router("infecciones", "infecciones")
mar_router = make_stub_router("mar", "mar")
kardex_router = make_stub_router("kardex", "kardex")
censo_router = make_stub_router("censo", "censo")
planes_cuidado_router = make_stub_router("planes-cuidado", "planes_cuidado")
portal_paciente_router = make_stub_router("portal", "portal_paciente")
