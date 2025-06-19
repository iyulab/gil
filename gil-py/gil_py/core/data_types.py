"""
Gil 데이터 타입 정의
"""

from enum import Enum
from typing import Any, Dict, List, Union


class GilDataType(Enum):
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
GilDataTypeMap = {
    GilDataType.TEXT: str,
    GilDataType.NUMBER: Union[int, float],
    GilDataType.BOOLEAN: bool,
    GilDataType.JSON: Dict[str, Any],
    GilDataType.ARRAY: List[Any],
    GilDataType.BINARY: bytes,
    GilDataType.IMAGE: bytes,
    GilDataType.AUDIO: bytes,
    GilDataType.VIDEO: bytes,
    GilDataType.FILE: bytes,
    GilDataType.ANY: Any,
}
