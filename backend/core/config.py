import os
from pathlib import Path
from dotenv import load_dotenv

_BASE_DIR = Path(__file__).resolve().parent.parent
load_dotenv(_BASE_DIR.parent / ".env")

DATABASE_URL: str = os.getenv("DATABASE_URL", "")
SECRET_KEY: str = os.getenv("SECRET_KEY", "changeme")
JWT_SECRET_KEY: str = os.getenv("JWT_SECRET_KEY", SECRET_KEY)
JWT_ALGORITHM: str = "HS256"
ACCESS_TOKEN_EXPIRE_MINUTES: int = 15
REFRESH_TOKEN_EXPIRE_DAYS: int = 7
UPLOAD_DIR: str = str(_BASE_DIR.parent / "uploads")
