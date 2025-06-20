"""
워크플로우 실행 엔진
"""

import asyncio
from typing import Dict, Any, List, Set
from ..core import GilNode
from collections import defaultdict, deque


class WorkflowExecutor:
    """워크플로우 실행 엔진"""
    
    def __init__(self):
        self.execution_results: Dict[str, Any] = {}
      async def execute(self, nodes: Dict[str, GilNode], 
                     connections: List[Dict[str, str]], 
                     inputs: Dict[str, Any]) -> Dict[str, Any]:
        """워크플로우 실행"""
        self.execution_results = {}
        
        # 실행 순서 계산
        execution_order = self._calculate_execution_order(nodes, connections)
        
        print(f"🚀 워크플로우 실행 시작 (노드 {len(nodes)}개)")
        print(f"📋 실행 순서: {' -> '.join(execution_order)}")
        
        # 순차적으로 노드 실행
        for node_name in execution_order:
            if node_name not in nodes:
                print(f"⚠️ 노드 '{node_name}'를 찾을 수 없습니다")
                continue
            
            node = nodes[node_name]
            node_inputs = self._prepare_node_inputs(node_name, inputs, connections)
            
            print(f"⚡ 노드 '{node_name}' 실행 중...")
            print(f"   📥 입력 데이터: {node_inputs}")
            
            try:
                # 노드 실행
                result = await node.run(node_inputs)
                self.execution_results[node_name] = result
                print(f"✅ 노드 '{node_name}' 완료")
                
            except Exception as e:
                error_msg = f"❌ 노드 '{node_name}' 실행 실패: {e}"
                print(error_msg)
                self.execution_results[node_name] = {"error": str(e)}
        
        print("🎉 워크플로우 실행 완료!")
        return self.execution_results
    
    def _calculate_execution_order(self, nodes: Dict[str, GilNode], 
                                  connections: List[Dict[str, str]]) -> List[str]:
        """위상 정렬로 실행 순서 계산"""
        # 인접 리스트와 진입 차수 계산
        graph = defaultdict(list)
        in_degree = defaultdict(int)
        
        # 모든 노드 초기화
        for node_name in nodes:
            in_degree[node_name] = 0
        
        # 연결 정보로 그래프 구성
        for connection in connections:
            source = connection["source_node"]
            target = connection["target_node"]
            if source in nodes and target in nodes:
                graph[source].append(target)
                in_degree[target] += 1
        
        # 위상 정렬
        queue = deque([node for node in nodes if in_degree[node] == 0])
        result = []
        
        while queue:
            node = queue.popleft()
            result.append(node)
            
            for neighbor in graph[node]:
                in_degree[neighbor] -= 1                if in_degree[neighbor] == 0:
                    queue.append(neighbor)
        
        # 연결이 없는 고립된 노드들도 추가
        isolated_nodes = [node for node in nodes if node not in result]
        result.extend(isolated_nodes)
        
        return result
    
    def _prepare_node_inputs(self, node_name: str, 
                           workflow_inputs: Dict[str, Any],
                           connections: List[Dict[str, str]]) -> Dict[str, Any]:
        """노드 입력 데이터 준비"""
        node_inputs = {}
        
        # 워크플로우 입력에서 이 노드에 해당하는 입력 가져오기
        # 노드별로 명시적으로 설정된 입력만 사용
        if node_name in workflow_inputs:
            node_specific_inputs = workflow_inputs[node_name]
            if isinstance(node_specific_inputs, dict):
                node_inputs.update(node_specific_inputs)
        
        # 연결된 노드들의 출력 데이터 가져오기
        for connection in connections:
            if connection["target_node"] == node_name:
                source_node = connection["source_node"]
                source_port = connection.get("source_port", "output")
                target_port = connection.get("target_port", "input")
                
                # 소스 노드의 실행 결과 가져오기
                if source_node in self.execution_results:
                    source_result = self.execution_results[source_node]
                    
                    # 포트별 데이터 추출
                    if isinstance(source_result, dict) and source_port in source_result:
                        node_inputs[target_port] = source_result[source_port]
                    else:
                        # 전체 결과를 입력으로 사용
                        node_inputs[target_port] = source_result
        
        return node_inputs
    
    def get_node_result(self, node_name: str) -> Any:
        """특정 노드의 실행 결과 조회"""
        return self.execution_results.get(node_name)
    
    def get_all_results(self) -> Dict[str, Any]:
        """모든 노드의 실행 결과 조회"""
        return self.execution_results.copy()
    
    def clear_results(self) -> None:
        """실행 결과 초기화"""
        self.execution_results.clear()
