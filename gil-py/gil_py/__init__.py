"""
Gil-Py: 플로우차트 기반 워크플로우 노드 시스템
"""

__version__ = "0.1.0"

# 핵심 모듈
from .core import Node, Port, Connection, DataType

__all__ = [
    "Node",
    "Port", 
    "Connection",
    "DataType",
]
