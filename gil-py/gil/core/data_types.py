"""
Gil 데이터 타입 정의
"""

from enum import Enum
from typing import Any, Dict, List, Union


class DataType(Enum):
    """Gil 시스템에서 사용하는 데이터 타입"""
    TEXT = "text"
    NUMBER = "number"
    BOOLEAN = "boolean"
    JSON = "json"
    ARRAY = "array"
    BINARY = "binary"
    IMAGE = "image"
    AUDIO = "audio"
    VIDEO = "video"
    FILE = "file"
    ANY = "any"


# 타입 매핑
DataTypeMap = {
    DataType.TEXT: str,
    DataType.NUMBER: Union[int, float],
    DataType.BOOLEAN: bool,
    DataType.JSON: Dict[str, Any],
    DataType.ARRAY: List[Any],
    DataType.BINARY: bytes,
    DataType.IMAGE: bytes,
    DataType.AUDIO: bytes,
    DataType.VIDEO: bytes,
    DataType.FILE: bytes,
    DataType.ANY: Any,
}
