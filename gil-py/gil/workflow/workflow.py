
"""
Gil 워크플로우 클래스
"""

from typing import Dict, Any, List, Optional
from pathlib import Path
from ..core.node import Node
from ..yaml_parser import YamlWorkflowParser, WorkflowConfig
from .executor import WorkflowExecutor


class GilWorkflow:
    """Gil 워크플로우 클래스"""
    
    def __init__(self, name: str = "Gil Workflow"):
        self.name = name
        self.nodes: Dict[str, Node] = {}
        self.connections: List[Dict[str, str]] = []
        self.config: Optional[WorkflowConfig] = None
        self.executor = WorkflowExecutor()
    
    @classmethod
    def from_yaml(cls, yaml_path: str | Path) -> 'GilWorkflow':
        """YAML 파일에서 워크플로우 생성"""
        parser = YamlWorkflowParser()
        config = parser.parse_file(yaml_path)
        
        workflow = cls(name=config.name)
        workflow.config = config
        workflow._build_from_config(config)
        
        return workflow
    
    @classmethod 
    def from_dict(cls, config_dict: Dict[str, Any]) -> 'GilWorkflow':
        """딕셔너리에서 워크플로우 생성"""
        parser = YamlWorkflowParser()
        config = parser.parse_dict(config_dict)
        
        workflow = cls(name=config.name)
        workflow.config = config
        workflow._build_from_config(config)
        
        return workflow
    
    def add_node(self, name: str, node: Node) -> None:
        """노드 추가"""
        self.nodes[name] = node
    
    def connect(self, source_node: str, target_node: str, 
                source_port: str = "output", target_port: str = "input") -> None:
        """노드 연결"""
        connection = {
            "source_node": source_node,
            "target_node": target_node,
            "source_port": source_port,
            "target_port": target_port
        }
        self.connections.append(connection)
    
    def _build_from_config(self, config: WorkflowConfig) -> None:
        """설정에서 워크플로우 구축"""
        # 노드 팩토리를 통해 노드 생성
        from .node_factory import NodeFactory
        
        factory = NodeFactory()
        
        # 노드 생성
        for node_name, node_config in config.nodes.items():
            try:
                node = factory.create_node(
                    node_type=node_config.type,
                    config=node_config.config,
                    name=node_name
                )
                self.add_node(node_name, node)
            except Exception as e:
                print(f"⚠️ 노드 '{node_name}' 생성 실패: {e}")
        
        # 플로우에서 연결 추론
        self._infer_connections_from_flow(config.flow)
    
    def _infer_connections_from_flow(self, flow: List[Any]) -> None:
        """플로우 정의에서 연결 추론"""
        prev_nodes = []
        
        for step in flow:
            if isinstance(step, str):
                # 단일 노드
                if prev_nodes:
                    for prev_node in prev_nodes:
                        self.connect(prev_node, step)
                prev_nodes = [step]
            
            elif isinstance(step, list):
                # 병렬 노드들
                if prev_nodes:
                    for prev_node in prev_nodes:
                        for parallel_node in step:
                            self.connect(prev_node, parallel_node)
                prev_nodes = step
            
            elif isinstance(step, dict):
                # 복잡한 연결 정의
                node_name = step.get("node")
                depends_on = step.get("depends_on", [])
                
                for dependency in depends_on:
                    self.connect(dependency, node_name)
                
                if node_name:
                    prev_nodes = [node_name]
    
    async def run(self, inputs: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """워크플로우 실행"""
        if not self.nodes:
            raise ValueError("워크플로우에 노드가 없습니다")
        
        inputs = inputs or {}
        
        # 설정 기반 입력 처리
        if self.config:
            context = {
                "input": inputs,
                "environment": self.config.environment
            }
            
            # 노드별 입력 설정 적용
            for node_name, node_config in self.config.nodes.items():
                if node_config.inputs and node_name in self.nodes:
                    node_inputs = {}
                    parser = YamlWorkflowParser()
                    
                    for input_name, input_value in node_config.inputs.items():
                        resolved_value = parser.resolve_references(input_value, context)
                        node_inputs[input_name] = resolved_value
                    
                    inputs[node_name] = node_inputs
        
        # 워크플로우 실행
        return await self.executor.execute(self.nodes, self.connections, inputs)
    
    def validate(self) -> Dict[str, Any]:
        """워크플로우 유효성 검증"""
        validation_result = {
            "valid": True,
            "errors": [],
            "warnings": []
        }
        
        # 노드 존재 확인
        if not self.nodes:
            validation_result["errors"].append("노드가 정의되지 않았습니다")
            validation_result["valid"] = False
        
        # 연결 유효성 확인
        for connection in self.connections:
            source = connection["source_node"]
            target = connection["target_node"]
            
            if source not in self.nodes:
                validation_result["errors"].append(f"소스 노드 '{source}'를 찾을 수 없습니다")
                validation_result["valid"] = False
            
            if target not in self.nodes:
                validation_result["errors"].append(f"타겟 노드 '{target}'를 찾을 수 없습니다")
                validation_result["valid"] = False
        
        # 순환 참조 확인
        if self._has_cycle():
            validation_result["errors"].append("순환 참조가 발견되었습니다")
            validation_result["valid"] = False
        
        return validation_result
    
    def _has_cycle(self) -> bool:
        """순환 참조 확인"""
        # 간단한 DFS 기반 순환 확인
        visited = set()
        rec_stack = set()
        
        def dfs(node: str) -> bool:
            visited.add(node)
            rec_stack.add(node)
            
            # 이 노드의 모든 이웃을 확인
            for connection in self.connections:
                if connection["source_node"] == node:
                    neighbor = connection["target_node"]
                    if neighbor not in visited:
                        if dfs(neighbor):
                            return True
                    elif neighbor in rec_stack:
                        return True
            
            rec_stack.remove(node)
            return False
        
        for node in self.nodes:
            if node not in visited:
                if dfs(node):
                    return True
        
        return False
    
    def get_execution_order(self) -> List[str]:
        """실행 순서 계산 (위상 정렬)"""
        from collections import defaultdict, deque
        
        # 인접 리스트와 진입 차수 계산
        graph = defaultdict(list)
        in_degree = defaultdict(int)
        
        # 모든 노드 초기화
        for node in self.nodes:
            in_degree[node] = 0
        
        # 연결 정보로 그래프 구성
        for connection in self.connections:
            source = connection["source_node"]
            target = connection["target_node"]
            graph[source].append(target)
            in_degree[target] += 1
        
        # 위상 정렬
        queue = deque([node for node in self.nodes if in_degree[node] == 0])
        result = []
        
        while queue:
            node = queue.popleft()
            result.append(node)
            
            for neighbor in graph[node]:
                in_degree[neighbor] -= 1
                if in_degree[neighbor] == 0:
                    queue.append(neighbor)
        
        return result
    
    def visualize(self, output_path: Optional[str] = None) -> str:
        """워크플로우 시각화 (Mermaid 다이어그램)"""
        mermaid = ["graph TD"]
        
        # 노드 정의
        for node_name, node in self.nodes.items():
            node_type = node.node_type if hasattr(node, 'node_type') else type(node).__name__
            mermaid.append(f"    {node_name}[{node_name}<br/>{node_type}]")
        
        # 연결 정의
        for connection in self.connections:
            source = connection["source_node"]
            target = connection["target_node"]
            mermaid.append(f"    {source} --> {target}")
        
        diagram = "\n".join(mermaid)
        
        if output_path:
            with open(output_path, 'w', encoding='utf-8') as f:
                f.write(diagram)
        
        return diagram

