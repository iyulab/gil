"""
Gil-Flow 컨텍스트 시스템

플로우 컨텍스트와 노드 컨텍스트를 관리하는 클래스들
"""

from typing import Dict, Any, List, Optional
from datetime import datetime
import threading
from dataclasses import dataclass, field


@dataclass
class ContextError:
    """컨텍스트 에러 정보"""
    node: str
    message: str
    timestamp: str = field(default_factory=lambda: datetime.now().isoformat())
    error_type: str = "general"
    details: Optional[Dict[str, Any]] = None


class FlowContext:
    """워크플로우 전체에서 유지되는 전역 컨텍스트"""
    
    def __init__(self, workflow_id: str = None):
        self._lock = threading.RLock()
        
        # 기본 구조 초기화
        self.variables: Dict[str, Any] = {}
        self.errors: List[ContextError] = []
        self.metadata: Dict[str, Any] = {
            "workflow_id": workflow_id or f"wf_{int(datetime.now().timestamp())}",
            "execution_id": f"exec_{int(datetime.now().timestamp())}",
            "start_time": datetime.now().isoformat(),
            "total_nodes": 0,
            "completed_nodes": 0
        }
        self.shared_data: Dict[str, Any] = {}
    
    def set_variable(self, key: str, value: Any) -> None:
        """전역 변수 설정"""
        with self._lock:
            self.variables[key] = value
    
    def get_variable(self, key: str, default: Any = None) -> Any:
        """전역 변수 조회"""
        with self._lock:
            return self.variables.get(key, default)
    
    def add_error(self, node_id: str, message: str, error_type: str = "general", 
                  details: Dict[str, Any] = None) -> None:
        """에러 추가"""
        with self._lock:
            error = ContextError(
                node=node_id,
                message=message,
                error_type=error_type,
                details=details
            )
            self.errors.append(error)
    
    def get_errors(self, node_id: str = None, error_type: str = None) -> List[ContextError]:
        """에러 조회"""
        with self._lock:
            errors = self.errors.copy()
            
            if node_id:
                errors = [e for e in errors if e.node == node_id]
            
            if error_type:
                errors = [e for e in errors if e.error_type == error_type]
            
            return errors
    
    def update_metadata(self, key: str, value: Any) -> None:
        """메타데이터 업데이트"""
        with self._lock:
            self.metadata[key] = value
    
    def increment_completed_nodes(self) -> None:
        """완료된 노드 수 증가"""
        with self._lock:
            self.metadata["completed_nodes"] = self.metadata.get("completed_nodes", 0) + 1
    
    def set_shared_data(self, key: str, value: Any) -> None:
        """공유 데이터 설정"""
        with self._lock:
            self.shared_data[key] = value
    
    def get_shared_data(self, key: str, default: Any = None) -> Any:
        """공유 데이터 조회"""
        with self._lock:
            return self.shared_data.get(key, default)
    
    def append_shared_data(self, key: str, value: Any) -> None:
        """공유 데이터에 값 추가 (리스트)"""
        with self._lock:
            if key not in self.shared_data:
                self.shared_data[key] = []
            
            if isinstance(self.shared_data[key], list):
                self.shared_data[key].append(value)
            else:
                # 기존 값을 리스트로 변환
                self.shared_data[key] = [self.shared_data[key], value]
    
    def to_dict(self) -> Dict[str, Any]:
        """딕셔너리로 변환"""
        with self._lock:
            return {
                "variables": self.variables.copy(),
                "errors": [
                    {
                        "node": e.node,
                        "message": e.message,
                        "timestamp": e.timestamp,
                        "error_type": e.error_type,
                        "details": e.details
                    }
                    for e in self.errors
                ],
                "metadata": self.metadata.copy(),
                "shared_data": self.shared_data.copy()
            }


class NodeContext:
    """개별 노드 실행 중에만 유지되는 지역 컨텍스트"""
    
    def __init__(self, node_id: str, flow_context: FlowContext):
        self.node_id = node_id
        self.flow_context = flow_context
        
        # 노드별 컨텍스트 초기화
        self.variables: Dict[str, Any] = {}
        self.errors: List[ContextError] = []
        self.metadata: Dict[str, Any] = {
            "node_id": node_id,
            "start_time": datetime.now().isoformat(),
            "retry_count": 0
        }
        self.internal_state: Dict[str, Any] = {}
    
    def set_variable(self, key: str, value: Any) -> None:
        """노드 지역 변수 설정"""
        self.variables[key] = value
    
    def get_variable(self, key: str, default: Any = None) -> Any:
        """노드 지역 변수 조회"""
        return self.variables.get(key, default)
    
    def add_error(self, message: str, error_type: str = "general", 
                  details: Dict[str, Any] = None) -> None:
        """노드 에러 추가"""
        error = ContextError(
            node=self.node_id,
            message=message,
            error_type=error_type,
            details=details
        )
        self.errors.append(error)
        
        # 플로우 컨텍스트에도 전파
        self.flow_context.add_error(self.node_id, message, error_type, details)
    
    def set_internal_state(self, key: str, value: Any) -> None:
        """내부 상태 설정"""
        self.internal_state[key] = value
    
    def get_internal_state(self, key: str, default: Any = None) -> Any:
        """내부 상태 조회"""
        return self.internal_state.get(key, default)
    
    def update_metadata(self, key: str, value: Any) -> None:
        """노드 메타데이터 업데이트"""
        self.metadata[key] = value
    
    def increment_retry_count(self) -> None:
        """재시도 횟수 증가"""
        self.metadata["retry_count"] = self.metadata.get("retry_count", 0) + 1
    
    def propagate_to_flow(self, key: str, target: str, operation: str = "set") -> None:
        """노드 결과를 플로우 컨텍스트로 전파
        
        Args:
            key: 노드 컨텍스트의 키
            target: 플로우 컨텍스트의 대상 경로 (예: "variables.user_count")
            operation: 연산 타입 ("set", "append", "increment")
        """
        value = self.get_variable(key)
        if value is None:
            return
        
        # 대상 경로 파싱
        parts = target.split(".")
        if len(parts) < 2:
            return
        
        context_type = parts[0]  # "variables", "shared_data", "metadata"
        target_key = ".".join(parts[1:])
        
        if context_type == "variables":
            if operation == "set":
                self.flow_context.set_variable(target_key, value)
            elif operation == "increment" and isinstance(value, (int, float)):
                current = self.flow_context.get_variable(target_key, 0)
                self.flow_context.set_variable(target_key, current + value)
        
        elif context_type == "shared_data":
            if operation == "set":
                self.flow_context.set_shared_data(target_key, value)
            elif operation == "append":
                self.flow_context.append_shared_data(target_key, value)
        
        elif context_type == "metadata":
            if operation == "set":
                self.flow_context.update_metadata(target_key, value)
    
    def to_dict(self) -> Dict[str, Any]:
        """딕셔너리로 변환"""
        return {
            "variables": self.variables.copy(),
            "errors": [
                {
                    "node": e.node,
                    "message": e.message,
                    "timestamp": e.timestamp,
                    "error_type": e.error_type,
                    "details": e.details
                }
                for e in self.errors
            ],
            "metadata": self.metadata.copy(),
            "internal_state": self.internal_state.copy()
        }


class ContextManager:
    """컨텍스트 관리자"""
    
    def __init__(self, workflow_id: str = None):
        self.flow_context = FlowContext(workflow_id)
        self._node_contexts: Dict[str, NodeContext] = {}
    
    def create_node_context(self, node_id: str) -> NodeContext:
        """노드 컨텍스트 생성"""
        node_context = NodeContext(node_id, self.flow_context)
        self._node_contexts[node_id] = node_context
        return node_context
    
    def get_node_context(self, node_id: str) -> Optional[NodeContext]:
        """노드 컨텍스트 조회"""
        return self._node_contexts.get(node_id)
    
    def cleanup_node_context(self, node_id: str) -> None:
        """노드 컨텍스트 정리"""
        if node_id in self._node_contexts:
            del self._node_contexts[node_id]
    
    def resolve_context_reference(self, reference: str) -> Any:
        """컨텍스트 참조 해석
        
        예시:
        - ${flow_context.variables.user_id}
        - ${node_context.errors.length}
        - ${flow_context.metadata.total_nodes}
        """
        # 간단한 참조 해석 (실제로는 더 복잡한 파서 필요)
        if reference.startswith("${flow_context."):
            path = reference[15:-1]  # "${flow_context." 제거하고 "}" 제거
            return self._resolve_flow_context_path(path)
        
        elif reference.startswith("${node_context."):
            # 현재 실행 중인 노드 컨텍스트에서 해석 (실제로는 현재 노드 ID 필요)
            path = reference[15:-1]
            return self._resolve_node_context_path(path)
        
        return reference
    
    def _resolve_flow_context_path(self, path: str) -> Any:
        """플로우 컨텍스트 경로 해석"""
        parts = path.split(".")
        
        if parts[0] == "variables":
            return self.flow_context.get_variable(".".join(parts[1:]))
        elif parts[0] == "errors":
            if len(parts) == 2 and parts[1] == "length":
                return len(self.flow_context.errors)
            return self.flow_context.errors
        elif parts[0] == "metadata":
            return self.flow_context.metadata.get(".".join(parts[1:]))
        elif parts[0] == "shared_data":
            return self.flow_context.get_shared_data(".".join(parts[1:]))
        
        return None
    
    def _resolve_node_context_path(self, path: str, node_id: str = None) -> Any:
        """노드 컨텍스트 경로 해석"""
        if not node_id or node_id not in self._node_contexts:
            return None
        
        node_context = self._node_contexts[node_id]
        parts = path.split(".")
        
        if parts[0] == "variables":
            return node_context.get_variable(".".join(parts[1:]))
        elif parts[0] == "errors":
            if len(parts) == 2 and parts[1] == "length":
                return len(node_context.errors)
            return node_context.errors
        elif parts[0] == "metadata":
            return node_context.metadata.get(".".join(parts[1:]))
        elif parts[0] == "internal_state":
            return node_context.get_internal_state(".".join(parts[1:]))
        
        return None
