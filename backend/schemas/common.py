from typing import Generic, List, TypeVar
from pydantic import BaseModel

T = TypeVar("T")


class PaginatedResponse(BaseModel, Generic[T]):
    total: int
    items: List[T]
    page: int
    size: int


class MsgResponse(BaseModel):
    message: str
