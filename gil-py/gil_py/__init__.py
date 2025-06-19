"""
Gil-Py: 플로우차트 기반 워크플로우 노드 시스템
"""

__version__ = "0.1.0"

# 핵심 모듈만 기본으로 import
from .core import GilNode, GilPort, GilConnection, GilDataType

__all__ = [
    "GilNode",
    "GilPort", 
    "GilConnection",
    "GilDataType",
]
