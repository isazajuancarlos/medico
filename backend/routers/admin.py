from datetime import date, datetime
from typing import Optional, List
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlmodel import Session, select
from pydantic import BaseModel
from models import (
    Usuario, Institucion, ConfiguracionInstitucion, PoliticaSeguridad,
    ConfigCampo, AuditoriaAcceso, Especialidad, Entidad,
    RolUsuario, TipoCampo,
)
from core.deps import get_db, get_current_active_user, require_role
from core.aislamiento import obtener
import bcrypt

router = APIRouter(prefix="/admin", tags=["admin"])

ADMIN_ROLES = (RolUsuario.ADMIN, RolUsuario.SUPERADMIN)


# ─────────────────────────────────────────────────────────────────────────────
# SCHEMAS
# ─────────────────────────────────────────────────────────────────────────────

class InstitucionUpdate(BaseModel):
    razon_social: Optional[str] = None
    nombre_comercial: Optional[str] = None
    codigo_habilitacion: Optional[str] = None
    direccion: Optional[str] = None
    ciudad: Optional[str] = None
    departamento: Optional[str] = None
    telefono: Optional[str] = None
    email: Optional[str] = None
    representante_legal: Optional[str] = None
    identificacion_representante: Optional[str] = None
    codigo_prestador: Optional[str] = None
    tipo_prestador: Optional[str] = None
    color_primario: Optional[str] = None
    color_secundario: Optional[str] = None


class ConfiguracionUpdate(BaseModel):
    horario_apertura: Optional[str] = None
    horario_cierre: Optional[str] = None
    dias_atencion: Optional[List[str]] = None
    duracion_cita_default: Optional[int] = None
    tiempo_buffer: Optional[int] = None
    enviar_recordatorios: Optional[bool] = None
    horas_anticipacion: Optional[int] = None
    smtp_host: Optional[str] = None
    smtp_port: Optional[int] = None
    smtp_usuario: Optional[str] = None
    prefijo_factura: Optional[str] = None
    resolucion_facturacion: Optional[str] = None
    rango_desde: Optional[int] = None
    rango_hasta: Optional[int] = None
    permitir_citas_online: Optional[bool] = None
    requiere_autorizacion: Optional[bool] = None
    dias_maximos_agendamiento: Optional[int] = None


class PoliticaUpdate(BaseModel):
    longitud_minima_password: Optional[int] = None
    requiere_mayuscula: Optional[bool] = None
    requiere_minuscula: Optional[bool] = None
    requiere_numero: Optional[bool] = None
    requiere_caracter_especial: Optional[bool] = None
    dias_expiracion_password: Optional[int] = None
    historial_passwords: Optional[int] = None
    timeout_sesion_minutos: Optional[int] = None
    sesiones_simultaneas_permitidas: Optional[int] = None
    cerrar_otras_sesiones_al_login: Optional[bool] = None
    max_intentos_login: Optional[int] = None
    tiempo_bloqueo_minutos: Optional[int] = None
    requiere_2fa: Optional[bool] = None
    requiere_2fa_admin: Optional[bool] = None
    metodo_2fa: Optional[str] = None
    registrar_todos_los_accesos: Optional[bool] = None
    retener_logs_dias: Optional[int] = None
    notificar_login_nuevo_dispositivo: Optional[bool] = None
    notificar_cambio_password: Optional[bool] = None
    notificar_intentos_fallidos: Optional[bool] = None


class CampoCreate(BaseModel):
    etiqueta: str
    tipo: TipoCampo
    opciones: Optional[str] = None
    orden: int = 0


class CampoUpdate(BaseModel):
    etiqueta: Optional[str] = None
    tipo: Optional[TipoCampo] = None
    opciones: Optional[str] = None
    activo: Optional[bool] = None
    orden: Optional[int] = None


class EspecialidadCreate(BaseModel):
    nombre: str


class EntidadCreate(BaseModel):
    nit: str
    nombre: str
    codigo_eps: str


class EntidadUpdate(BaseModel):
    nombre: Optional[str] = None
    codigo_eps: Optional[str] = None


class ResetPasswordPayload(BaseModel):
    nueva_password: str


class UsuarioCreateAdmin(BaseModel):
    username: str
    password: str
    nombre_completo: str
    email: Optional[str] = None
    rol: RolUsuario = RolUsuario.MEDICO
    especialidad_id: Optional[int] = None
    registro_medico: Optional[str] = None
    registro_rethus: Optional[str] = None


class UsuarioUpdateAdmin(BaseModel):
    nombre_completo: Optional[str] = None
    email: Optional[str] = None
    rol: Optional[RolUsuario] = None
    activo: Optional[bool] = None
    especialidad_id: Optional[int] = None
    registro_medico: Optional[str] = None
    registro_rethus: Optional[str] = None
    requiere_cambio_password: Optional[bool] = None


# ─────────────────────────────────────────────────────────────────────────────
# INSTITUCIÓN
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/institucion")
def get_institucion(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    if not current_user.institucion_id:
        raise HTTPException(status_code=404, detail="Sin institución asignada")
    inst = db.get(Institucion, current_user.institucion_id)  # aislamiento: ya autorizado por el padre
    if not inst:
        raise HTTPException(status_code=404, detail="Institución no encontrada")
    return inst


@router.put("/institucion")
def update_institucion(
    payload: InstitucionUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    if not current_user.institucion_id:
        raise HTTPException(status_code=404, detail="Sin institución asignada")
    inst = db.get(Institucion, current_user.institucion_id)  # aislamiento: ya autorizado por el padre
    if not inst:
        raise HTTPException(status_code=404, detail="Institución no encontrada")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(inst, field, value)
    db.add(inst)
    db.commit()
    db.refresh(inst)
    return inst


# ─────────────────────────────────────────────────────────────────────────────
# CONFIGURACIÓN DE INSTITUCIÓN
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/configuracion")
def get_configuracion(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    if not current_user.institucion_id:
        raise HTTPException(status_code=404, detail="Sin institución asignada")
    cfg = db.exec(
        select(ConfiguracionInstitucion).where(
            ConfiguracionInstitucion.institucion_id == current_user.institucion_id
        )
    ).first()
    if not cfg:
        cfg = ConfiguracionInstitucion(institucion_id=current_user.institucion_id)
        db.add(cfg)
        db.commit()
        db.refresh(cfg)
    return cfg


@router.put("/configuracion")
def update_configuracion(
    payload: ConfiguracionUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    if not current_user.institucion_id:
        raise HTTPException(status_code=404, detail="Sin institución asignada")
    cfg = db.exec(
        select(ConfiguracionInstitucion).where(
            ConfiguracionInstitucion.institucion_id == current_user.institucion_id
        )
    ).first()
    if not cfg:
        cfg = ConfiguracionInstitucion(institucion_id=current_user.institucion_id)
        db.add(cfg)
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(cfg, field, value)
    db.add(cfg)
    db.commit()
    db.refresh(cfg)
    return cfg


# ─────────────────────────────────────────────────────────────────────────────
# POLÍTICA DE SEGURIDAD
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/politica-seguridad")
def get_politica(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    politica = db.exec(
        select(PoliticaSeguridad).where(
            PoliticaSeguridad.institucion_id == current_user.institucion_id
        )
    ).first()
    if not politica:
        politica = PoliticaSeguridad(institucion_id=current_user.institucion_id)
        db.add(politica)
        db.commit()
        db.refresh(politica)
    return politica


@router.put("/politica-seguridad")
def update_politica(
    payload: PoliticaUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    politica = db.exec(
        select(PoliticaSeguridad).where(
            PoliticaSeguridad.institucion_id == current_user.institucion_id
        )
    ).first()
    if not politica:
        politica = PoliticaSeguridad(institucion_id=current_user.institucion_id)
        db.add(politica)
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(politica, field, value)
    db.add(politica)
    db.commit()
    db.refresh(politica)
    return politica


# ─────────────────────────────────────────────────────────────────────────────
# USUARIOS (admin-scoped CRUD + special actions)
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/usuarios")
def list_usuarios_admin(
    q: Optional[str] = Query(default=None),
    rol: Optional[str] = Query(default=None),
    activo: Optional[bool] = Query(default=None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    query = select(Usuario)
    if current_user.rol != RolUsuario.SUPERADMIN:
        query = query.where(Usuario.institucion_id == current_user.institucion_id)
    query = query.where(Usuario.rol != RolUsuario.SUPERADMIN)
    if q:
        query = query.where(
            Usuario.nombre_completo.ilike(f"%{q}%") | Usuario.username.ilike(f"%{q}%")
        )
    if rol:
        query = query.where(Usuario.rol == rol)
    if activo is not None:
        query = query.where(Usuario.activo == activo)
    usuarios = db.exec(query.order_by(Usuario.nombre_completo)).all()
    especialidades = {e.id: e.nombre for e in db.exec(select(Especialidad)).all()}
    result = []
    for u in usuarios:
        result.append({
            "id": u.id,
            "username": u.username,
            "nombre_completo": u.nombre_completo,
            "email": u.email,
            "rol": u.rol,
            "activo": u.activo,
            "cuenta_bloqueada": u.cuenta_bloqueada,
            "especialidad_id": u.especialidad_id,
            "especialidad_nombre": especialidades.get(u.especialidad_id) if u.especialidad_id else None,
            "registro_medico": u.registro_medico,
            "registro_rethus": u.registro_rethus,
            "requiere_cambio_password": u.requiere_cambio_password,
            "ultimo_cambio_password": u.ultimo_cambio_password.isoformat() if u.ultimo_cambio_password else None,
        })
    return result


@router.post("/usuarios", status_code=201)
def create_usuario_admin(
    payload: UsuarioCreateAdmin,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    existing = db.exec(select(Usuario).where(Usuario.username == payload.username)).first()
    if existing:
        raise HTTPException(status_code=400, detail="El username ya existe")
    hashed = bcrypt.hashpw(payload.password.encode(), bcrypt.gensalt()).decode()
    data = payload.model_dump(exclude={"password"})
    usuario = Usuario(
        **data,
        hashed_password=hashed,
        institucion_id=current_user.institucion_id,
    )
    db.add(usuario)
    db.commit()
    db.refresh(usuario)
    return {"id": usuario.id, "username": usuario.username, "nombre_completo": usuario.nombre_completo}


@router.put("/usuarios/{usuario_id}")
def update_usuario_admin(
    usuario_id: int,
    payload: UsuarioUpdateAdmin,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    usuario = obtener(db, Usuario, usuario_id, current_user, que="usuario")
    if current_user.rol != RolUsuario.SUPERADMIN and usuario.institucion_id != current_user.institucion_id:
        raise HTTPException(status_code=403, detail="Sin permisos")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(usuario, field, value)
    db.add(usuario)
    db.commit()
    db.refresh(usuario)
    return {"id": usuario.id, "activo": usuario.activo, "rol": usuario.rol}


@router.post("/usuarios/{usuario_id}/desbloquear")
def desbloquear_usuario(
    usuario_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    usuario = obtener(db, Usuario, usuario_id, current_user, que="usuario")
    usuario.cuenta_bloqueada = False
    usuario.intentos_fallidos = 0
    usuario.fecha_bloqueo = None
    db.add(usuario)
    db.commit()
    return {"ok": True, "mensaje": "Cuenta desbloqueada"}


@router.post("/usuarios/{usuario_id}/reset-password")
def reset_password(
    usuario_id: int,
    payload: ResetPasswordPayload,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    usuario = obtener(db, Usuario, usuario_id, current_user, que="usuario")
    if len(payload.nueva_password) < 6:
        raise HTTPException(status_code=400, detail="La contraseña debe tener al menos 6 caracteres")
    hashed = bcrypt.hashpw(payload.nueva_password.encode(), bcrypt.gensalt()).decode()
    usuario.hashed_password = hashed
    usuario.requiere_cambio_password = True
    usuario.ultimo_cambio_password = datetime.now()
    db.add(usuario)
    db.commit()
    return {"ok": True, "mensaje": "Contraseña actualizada. El usuario deberá cambiarla al iniciar sesión."}


@router.delete("/usuarios/{usuario_id}")
def delete_usuario(
    usuario_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    if usuario_id == current_user.id:
        raise HTTPException(status_code=400, detail="No puede eliminarse a sí mismo")
    usuario = obtener(db, Usuario, usuario_id, current_user, que="usuario")
    if current_user.rol != RolUsuario.SUPERADMIN and usuario.institucion_id != current_user.institucion_id:
        raise HTTPException(status_code=403, detail="Sin permisos")
    # Soft delete to preserve audit trail
    usuario.activo = False
    db.add(usuario)
    db.commit()
    return {"ok": True}


# ─────────────────────────────────────────────────────────────────────────────
# CAMPOS DINÁMICOS DE HISTORIA CLÍNICA
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/campos-dinamicos")
def list_campos(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    return db.exec(select(ConfigCampo).order_by(ConfigCampo.orden)).all()


@router.post("/campos-dinamicos", status_code=201)
def create_campo(
    payload: CampoCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    campo = ConfigCampo(**payload.model_dump())
    db.add(campo)
    db.commit()
    db.refresh(campo)
    return campo


@router.put("/campos-dinamicos/{campo_id}")
def update_campo(
    campo_id: int,
    payload: CampoUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    campo = obtener(db, ConfigCampo, campo_id, current_user, que="configcampo")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(campo, field, value)
    db.add(campo)
    db.commit()
    db.refresh(campo)
    return campo


@router.delete("/campos-dinamicos/{campo_id}")
def delete_campo(
    campo_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    campo = obtener(db, ConfigCampo, campo_id, current_user, que="configcampo")
    db.delete(campo)
    db.commit()
    return {"ok": True}


# ─────────────────────────────────────────────────────────────────────────────
# AUDITORÍA
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/auditoria")
def list_auditoria(
    fecha_inicio: Optional[date] = Query(default=None),
    fecha_fin: Optional[date] = Query(default=None),
    accion: Optional[str] = Query(default=None),
    usuario_id: Optional[int] = Query(default=None),
    page: int = Query(default=1, ge=1),
    size: int = Query(default=50, le=200),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    from sqlalchemy import cast, Date
    q = select(AuditoriaAcceso).order_by(AuditoriaAcceso.fecha_hora.desc())
    if fecha_inicio:
        q = q.where(cast(AuditoriaAcceso.fecha_hora, Date) >= fecha_inicio)
    if fecha_fin:
        q = q.where(cast(AuditoriaAcceso.fecha_hora, Date) <= fecha_fin)
    if accion:
        q = q.where(AuditoriaAcceso.accion.ilike(f"%{accion}%"))
    if usuario_id:
        q = q.where(AuditoriaAcceso.usuario_id == usuario_id)

    total = len(db.exec(q).all())
    rows = db.exec(q.offset((page - 1) * size).limit(size)).all()

    user_cache: dict = {}
    result = []
    for row in rows:
        if row.usuario_id not in user_cache:
            u = db.get(Usuario, row.usuario_id)  # aislamiento: ya autorizado por el padre
            user_cache[row.usuario_id] = u.nombre_completo if u else f"ID {row.usuario_id}"
        result.append({
            "id": row.id,
            "fecha_hora": row.fecha_hora.isoformat(),
            "usuario_id": row.usuario_id,
            "usuario_nombre": user_cache[row.usuario_id],
            "accion": row.accion,
            "tabla_afectada": row.tabla_afectada,
            "registro_id": row.registro_id,
            "paciente_id": row.paciente_id,
            "ip_address": row.ip_address,
            "exitoso": row.exitoso,
        })

    return {"total": total, "page": page, "size": size, "items": result}


# ─────────────────────────────────────────────────────────────────────────────
# ESPECIALIDADES
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/especialidades")
def list_especialidades(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    return db.exec(select(Especialidad).order_by(Especialidad.nombre)).all()


@router.post("/especialidades", status_code=201)
def create_especialidad(
    payload: EspecialidadCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    existing = db.exec(select(Especialidad).where(Especialidad.nombre == payload.nombre)).first()
    if existing:
        raise HTTPException(status_code=400, detail="Ya existe una especialidad con ese nombre")
    esp = Especialidad(nombre=payload.nombre)
    db.add(esp)
    db.commit()
    db.refresh(esp)
    return esp


@router.delete("/especialidades/{esp_id}")
def delete_especialidad(
    esp_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    esp = obtener(db, Especialidad, esp_id, current_user, que="especialidad")
    en_uso = db.exec(select(Usuario).where(Usuario.especialidad_id == esp_id)).all()
    if en_uso:
        raise HTTPException(
            status_code=400,
            detail=f"No se puede eliminar — {len(en_uso)} usuario(s) la usan"
        )
    db.delete(esp)
    db.commit()
    return {"ok": True}


# ─────────────────────────────────────────────────────────────────────────────
# ENTIDADES (EPS)
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/entidades")
def list_entidades(
    q: Optional[str] = Query(default=None),
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(get_current_active_user),
):
    query = select(Entidad).order_by(Entidad.nombre)
    if q:
        query = query.where(Entidad.nombre.ilike(f"%{q}%") | Entidad.nit.ilike(f"%{q}%"))
    return db.exec(query).all()


@router.post("/entidades", status_code=201)
def create_entidad(
    payload: EntidadCreate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    existing = db.exec(select(Entidad).where(Entidad.nit == payload.nit)).first()
    if existing:
        raise HTTPException(status_code=400, detail="Ya existe una entidad con ese NIT")
    entidad = Entidad(**payload.model_dump())
    db.add(entidad)
    db.commit()
    db.refresh(entidad)
    return entidad


@router.put("/entidades/{entidad_id}")
def update_entidad(
    entidad_id: int,
    payload: EntidadUpdate,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    entidad = obtener(db, Entidad, entidad_id, current_user, que="entidad")
    for field, value in payload.model_dump(exclude_none=True).items():
        setattr(entidad, field, value)
    db.add(entidad)
    db.commit()
    db.refresh(entidad)
    return entidad


@router.delete("/entidades/{entidad_id}")
def delete_entidad(
    entidad_id: int,
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    entidad = obtener(db, Entidad, entidad_id, current_user, que="entidad")
    db.delete(entidad)
    db.commit()
    return {"ok": True}


# ─────────────────────────────────────────────────────────────────────────────
# AUTORIZACIONES DE ACCESO
# ─────────────────────────────────────────────────────────────────────────────

@router.get("/autorizaciones")
def list_autorizaciones(
    db: Session = Depends(get_db),
    current_user: Usuario = Depends(require_role(*ADMIN_ROLES)),
):
    from models import AutorizacionAcceso, Paciente
    rows = db.exec(select(AutorizacionAcceso)).all()
    result = []
    for r in rows:
        pac = db.get(Paciente, r.paciente_id)  # aislamiento: ya autorizado por el padre
        medico = db.get(Usuario, r.medico_id)  # aislamiento: ya autorizado por el padre
        result.append({
            "id": r.id,
            "medico_nombre": medico.nombre_completo if medico else "—",
            "paciente_nombre": f"{pac.nombre1} {pac.apellido1}" if pac else "—",
            "fecha_expiracion": r.fecha_expiracion.isoformat(),
            "activo": r.activo,
        })
    return result
