from datetime import datetime, timedelta, timezone
from typing import Optional, Dict, Any
from jose import jwt
from jose.exceptions import JOSEError
from fastapi import HTTPException, status
from core.config import JWT_SECRET_KEY, JWT_ALGORITHM, ACCESS_TOKEN_EXPIRE_MINUTES, REFRESH_TOKEN_EXPIRE_DAYS


def create_access_token(data: Dict[str, Any], expires_delta: Optional[timedelta] = None) -> str:
    to_encode = data.copy()
    expire = datetime.now(timezone.utc) + (expires_delta or timedelta(minutes=ACCESS_TOKEN_EXPIRE_MINUTES))
    to_encode.update({"exp": expire, "type": "access"})
    return jwt.encode(to_encode, JWT_SECRET_KEY, algorithm=JWT_ALGORITHM)


def create_refresh_token(user_id: int) -> str:
    expire = datetime.now(timezone.utc) + timedelta(days=REFRESH_TOKEN_EXPIRE_DAYS)
    to_encode = {"sub": str(user_id), "exp": expire, "type": "refresh"}
    return jwt.encode(to_encode, JWT_SECRET_KEY, algorithm=JWT_ALGORITHM)


def decode_token(token: str) -> Dict[str, Any]:
    try:
        payload = jwt.decode(token, JWT_SECRET_KEY, algorithms=[JWT_ALGORITHM])
        return payload
    # JOSEError y no JWTError, y la diferencia se midió: `JWKError` **no** es
    # subclase de `JWTError`, así que se escapaba de aquí y salía como 500. Hoy
    # ese camino es inalcanzable —los once tokens forjados que probó la revisión
    # dan los once 401—, pero deja de serlo en cuanto alguien toque la lista de
    # `algorithms`: con `RS256` dentro, un token RS256 produce `JWKError` y el
    # servidor responde 500 a una credencial inválida. `JOSEError` es la raíz de
    # la familia y cierra las tres ramas (JWT, JWS, JWK) de una vez.
    except JOSEError:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Token inválido o expirado",
            headers={"WWW-Authenticate": "Bearer"},
        )
