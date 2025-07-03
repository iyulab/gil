"""
워크플로우 실행 엔진
"""

from typing import Dict, Any
from ..core.node import Node
from ..core.context import Context
from ..yaml_parser import WorkflowConfig
from ..workflow.node_factory import NodeFactory


class WorkflowExecutor:
    """워크플로우 실행 엔진"""
    
    def __init__(self, node_factory: NodeFactory):
        self.node_factory = node_factory
        self.execution_results: Dict[str, Any] = {}
    
    async def execute(self, workflow: WorkflowConfig, context: Context) -> Dict[str, Any]:
        """워크플로우 실행"""
        self.execution_results = {}
        
        # 노드 인스턴스 생성
        instantiated_nodes: Dict[str, Node] = {}
        for node_id, node_config in workflow.nodes.items():
            instantiated_nodes[node_id] = self.node_factory.create_node(
                node_config.type, node_config.config, node_id
            )
        
        print(f"🚀 워크플로우 실행 시작 (노드 {len(instantiated_nodes)}개)")
        print(f"📋 실행 순서: {' -> '.join(workflow.flow)}")
        
        # 순차적으로 노드 실행
        for node_id in workflow.flow:
            if node_id not in instantiated_nodes:
                print(f"⚠️ 노드 '{node_id}'를 찾을 수 없습니다")
                continue
            
            node = instantiated_nodes[node_id]
            node_config = workflow.nodes[node_id]
            
            # 입력 데이터 준비
            node_inputs = {}
            for input_name, input_value in node_config.inputs.items():
                # 컨텍스트 참조 해결
                resolved_value = context.resolve_reference(input_value)
                node_inputs[input_name] = resolved_value
            
            print(f"⚡ 노드 '{node_id}' 실행 중...")
            print(f"   📥 입력 데이터: {node_inputs}")
            
            try:
                # 노드 실행
                result = await node.execute(node_inputs, context)
                self.execution_results[node_id] = result
                print(f"✅ 노드 '{node_id}' 완료")
                
            except Exception as e:
                error_msg = f"❌ 노드 '{node_id}' 실행 실패: {e}"
                print(error_msg)
                self.execution_results[node_id] = {"error": str(e)}
        
        print("🎉 워크플로우 실행 완료!")
        return {"node_outputs": self.execution_results, "context": context.to_dict()}
    
    def get_node_result(self, node_name: str) -> Any:
        """특정 노드의 실행 결과 조회"""
        return self.execution_results.get(node_name)
    
    def get_all_results(self) -> Dict[str, Any]:
        """모든 노드의 실행 결과 조회"""
        return self.execution_results.copy()
    
    def clear_results(self) -> None:
        """실행 결과 초기화"""
        self.execution_results.clear()
