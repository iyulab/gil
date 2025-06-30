"""
노드 팩토리 - YAML 설정에서 노드 인스턴스 생성
"""

from typing import Dict, Any, Type, List
from ..core.node import Node
from ..connectors.openai_connector import OpenAIConnector
from ..generators.image_generator import ImageGenerator
from ..nodes.text_generation import TextGenerationNode
from ..nodes.image_analysis import ImageAnalysisNode


class NodeFactory:
    """노드 팩토리 클래스"""
    
    def __init__(self):
        self._node_registry: Dict[str, Type[Node]] = {}
        self._register_builtin_nodes()
    
    def _register_builtin_nodes(self) -> None:
        """내장 노드 타입들 등록"""
        self.register("OpenAIConnector", OpenAIConnector)
        self.register("ImageGenerator", ImageGenerator)
        self.register("TextGeneration", TextGenerationNode)
        self.register("ImageAnalysis", ImageAnalysisNode)
        
        # 향후 추가할 노드들
        # self.register("GilGenText", GilGenText)
        # self.register("GilAnalyzeVision", GilAnalyzeVision)
        # self.register("GilUtilTransform", GilUtilTransform)
    
    def register(self, node_type: str, node_class: Type[Node]) -> None:
        """노드 타입 등록"""
        self._node_registry[node_type] = node_class
    
    def create_node(self, node_type: str, config: Dict[str, Any], name: str = None) -> Node:
        """노드 인스턴스 생성"""
        if node_type not in self._node_registry:
            raise ValueError(f"알 수 없는 노드 타입: {node_type}")
        
        node_class = self._node_registry[node_type]
        
        # 노드별 특별 처리
        if node_type == "OpenAIConnector":
            return self._create_openai_connector(node_class, config, name)
        elif node_type == "ImageGenerator":
            return self._create_image_generator(node_class, config, name)
        else:
            # 기본 생성
            return node_class(node_id=name or f"{node_type}_instance", config=config)
    
    def _create_openai_connector(self, node_class: Type, config: Dict[str, Any], name: str) -> Node:
        """OpenAI 커넥터 생성"""
        # 환경변수에서 API 키 가져오기
        import os
        api_key = config.get("api_key")
        
        if api_key and api_key.startswith("${") and api_key.endswith("}"):
            env_var = api_key[2:-1]  # ${OPENAI_API_KEY} -> OPENAI_API_KEY
            api_key = os.getenv(env_var)
        
        if not api_key:
            raise ValueError("OpenAI API 키가 설정되지 않았습니다")
        
        return node_class(
            node_id=name or "OpenAI Connector",
            config={
                "api_key": api_key,
                "organization": config.get("organization"),
                "base_url": config.get("base_url")
            }
        )
    
    def _create_image_generator(self, node_class: Type, config: Dict[str, Any], name: str) -> Node:
        """이미지 생성기 생성"""
        connector_ref = config.get("connector")
        
        if connector_ref and connector_ref.startswith("@"):
            # 참조는 나중에 해결되므로 일단 None으로 설정
            # 실제 연결은 워크플로우 실행 시 처리
            connector = None
        else:
            raise ValueError("이미지 생성기는 connector 참조가 필요합니다")
        
        # 임시로 더미 커넥터 생성 (실행 시 교체됨)
        import os
        api_key = os.getenv("OPENAI_API_KEY", "dummy-key")
        dummy_connector = OpenAIConnector(node_id="dummy_connector", config={"api_key": api_key})
        
        return node_class(
            node_id=name or "Image Generator",
            config={
                "connector": dummy_connector
            }
        )
    
    def get_available_nodes(self) -> List[str]:
        """사용 가능한 노드 타입 목록"""
        return list(self._node_registry.keys())
    
    def get_node_info(self, node_type: str) -> Dict[str, Any]:
        """노드 타입 정보 조회"""
        if node_type not in self._node_registry:
            return {"error": f"알 수 없는 노드 타입: {node_type}"}
        
        node_class = self._node_registry[node_type]
        
        # 노드 클래스에서 정보 추출
        info = {
            "type": node_type,
            "class": node_class.__name__,
            "description": node_class.__doc__ or "설명 없음",
        }
        
        # 더미 인스턴스로 포트 정보 가져오기 (가능한 경우)
        try:
            if node_type == "OpenAIConnector":
                dummy_instance = node_class(node_id="dummy", node_config={"api_key": "dummy"})
            elif node_type == "ImageGenerator":
                dummy_connector = OpenAIConnector(node_id="dummy_connector", node_config={"api_key": "dummy"})
                dummy_instance = node_class(node_id="dummy", node_config={"connector": dummy_connector})
            else:
                dummy_instance = node_class(node_id="dummy")
            
            info["input_ports"] = [
                {
                    "name": port.name,
                    "type": port.data_type,
                    "required": port.required,
                    "description": port.description
                }
                for port in dummy_instance.input_ports
            ]
            
            info["output_ports"] = [
                {
                    "name": port.name, 
                    "type": port.data_type,
                    "description": port.description
                }
                for port in dummy_instance.output_ports
            ]
            
        except Exception:
            info["ports"] = "포트 정보를 가져올 수 없습니다"
        
        return info
