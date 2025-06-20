"""
Gil-Py: 플로우차트 기반 워크플로우 노드 시스템
"""

__version__ = "0.1.0"

# 핵심 모듈
from .core import GilNode, GilPort, GilConnection, GilDataType

# 지연 임포트로 순환 참조 방지
def _get_openai_connector():
    from .connectors.openai_connector import GilConnectorOpenAI
    return GilConnectorOpenAI

def _get_image_generator():
    from .generators.image_generator import GilGenImage
    return GilGenImage

# 실제 클래스들을 모듈 레벨에서 사용 가능하게 함
GilConnectorOpenAI = _get_openai_connector()
GilGenImage = _get_image_generator()

__all__ = [
    "GilNode",
    "GilPort", 
    "GilConnection",
    "GilDataType",
    "GilConnectorOpenAI",
    "GilGenImage",
]
