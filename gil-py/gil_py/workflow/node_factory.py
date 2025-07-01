"""
노드 팩토리 - 설치된 노드 패키지에서 동적으로 노드 인스턴스 생성
"""
import importlib.metadata
from typing import Dict, Any, Type, List

from ..core.node import Node

class NodeFactory:
    """
    설치된 `gil-node-*` 패키지에서 노드를 동적으로 발견하고 생성하는 팩토리 클래스.
    `setuptools`의 `entry_points` 메커니즘을 사용하여 'gil.nodes' 그룹에 등록된 노드를 찾습니다.
    """
    
    def __init__(self):
        self._node_registry: Dict[str, Type[Node]] = {}
        self._discover_nodes()
    
    def _discover_nodes(self) -> None:
        """
        `gil.nodes` entry point를 스캔하여 사용 가능한 모든 노드를 동적으로 등록합니다.
        """
        try:
            # For Python 3.10+
            entry_points = importlib.metadata.entry_points(group='gil.nodes')
        except TypeError:
            # Fallback for Python < 3.10
            entry_points = importlib.metadata.entry_points().get('gil.nodes', [])

        for entry_point in entry_points:
            try:
                node_class = entry_point.load()
                # The entry point name is the node_type
                node_type = entry_point.name
                self.register(node_type, node_class)
            except Exception as e:
                # Log or print a warning that a node could not be loaded
                print(f"Warning: Could not load node '{entry_point.name}': {e}")

    def register(self, node_type: str, node_class: Type[Node]) -> None:
        """
        주어진 타입 이름으로 노드 클래스를 레지스트리에 등록합니다.
        """
        if not issubclass(node_class, Node):
            print(f"Warning: Class {node_class.__name__} is not a subclass of Node. Skipping.")
            return
            
        if node_type in self._node_registry:
            # Handle potential conflicts if multiple packages register the same node type
            print(f"Warning: Node type '{node_type}' is already registered. Overwriting.")
            
        self._node_registry[node_type] = node_class
    
    def create_node(self, node_type: str, config: Dict[str, Any], name: str = None) -> Node:
        """
        레지스트리에서 노드 클래스를 찾아 인스턴스를 생성합니다.
        노드 생성에 필요한 모든 복잡한 로직(API 키 처리 등)은
        각 노드 클래스의 생성자(`__init__`)에서 처리해야 합니다.
        """
        if node_type not in self._node_registry:
            raise ValueError(f"Unknown node type: '{node_type}'. Is the corresponding node package installed?")
        
        node_class = self._node_registry[node_type]
        
        # The node's __init__ is now responsible for its own setup from the config.
        return node_class(node_id=name or f"{node_type}_instance", node_config=config)
    
    def get_available_nodes(self) -> List[str]:
        """
        사용 가능한 모든 노드 타입의 목록을 반환합니다.
        """
        return sorted(list(self._node_registry.keys()))
    
    def get_node_info(self, node_type: str) -> Dict[str, Any]:
        """
        특정 노드 타입에 대한 상세 정보(설명, 포트 등)를 조회합니다.
        """
        if node_type not in self._node_registry:
            return {"error": f"Unknown node type: '{node_type}'"}
        
        node_class = self._node_registry[node_type]
        
        info = {
            "type": node_type,
            "class": node_class.__name__,
            "description": node_class.__doc__ or "No description provided.",
            "input_ports": [],
            "output_ports": []
        }
        
        # Introspect port information from the node class.
        # This assumes the node can be instantiated without a complex config.
        # A more robust solution might be a classmethod on the Node base class.
        try:
            # We create a dummy instance to inspect its ports.
            # The node's __init__ should handle a None or empty config gracefully.
            dummy_instance = node_class(node_id="dummy_for_introspection", node_config={})
            
            info["input_ports"] = [
                {
                    "name": port.name,
                    "type": str(port.data_type), # Use str() to be safe
                    "required": port.required,
                    "description": port.description
                }
                for port in dummy_instance.input_ports
            ]
            
            info["output_ports"] = [
                {
                    "name": port.name, 
                    "type": str(port.data_type),
                    "description": port.description
                }
                for port in dummy_instance.output_ports
            ]
            
        except Exception as e:
            info["ports_error"] = f"Could not retrieve port information: {e}"
        
        return info
