# Gil-Flow 컨텍스트 시스템

Gil-Flow에서는 워크플로우 실행 중 상태와 정보를 관리하기 위한 이중 컨텍스트 시스템을 제공합니다.

## 🌊 Flow Context (플로우 컨텍스트)

**Flow Context**는 전체 워크플로우 실행 과정에서 유지되는 전역 상태입니다.

### 구조
```yaml
flow_context:
  variables:           # 전역 변수
    user_id: "12345"
    start_time: "2025-06-20T10:00:00Z"
    
  errors:             # 누적 에러 정보
    - node: "data_processor"
      message: "Invalid input format"
      timestamp: "2025-06-20T10:05:00Z"
      
  metadata:           # 실행 메타데이터
    workflow_id: "wf_abc123"
    execution_id: "exec_456"
    total_nodes: 5
    completed_nodes: 3
    
  shared_data:        # 노드 간 공유 데이터
    temp_files: []
    cache_keys: []
```

### 주요 기능
- **전역 변수 관리**: 모든 노드에서 접근 가능한 변수
- **에러 누적**: 워크플로우 전체 에러 추적
- **메타데이터 관리**: 실행 상태 및 통계 정보
- **공유 데이터**: 노드 간 임시 데이터 공유

### 접근 방법
```yaml
# YAML에서 접근
inputs:
  user_id: "${flow_context.variables.user_id}"
  error_count: "${flow_context.errors.length}"
  
# 조건부 실행
conditions:
  - condition: "${flow_context.errors.length} == 0"
    action: "execute"
```

## 🔗 Node Context (노드 컨텍스트)

**Node Context**는 개별 노드 실행 중에만 유지되는 지역 상태입니다.

### 구조
```yaml
node_context:
  variables:          # 노드 지역 변수
    temp_result: "processing"
    iteration_count: 3
    
  errors:            # 노드 내부 에러
    - type: "validation_error"
      field: "email"
      message: "Invalid format"
      
  metadata:          # 노드 실행 정보
    node_id: "data_processor"
    start_time: "2025-06-20T10:05:00Z"
    retry_count: 1
    
  internal_state:    # 노드 내부 상태
    connection_pool: {}
    cache: {}
    progress: 0.75
```

### 주요 기능
- **지역 변수**: 노드 내부에서만 사용되는 변수
- **내부 에러**: 노드 수준의 세밀한 에러 관리
- **실행 상태**: 노드별 상세 실행 정보
- **내부 상태**: 노드의 임시 상태 저장

### 접근 방법
```yaml
# 노드 내부에서만 접근 가능
node_config:
  max_retries: "${node_context.metadata.retry_count}"
  use_cache: "${node_context.internal_state.cache != null}"
```

## 🔄 컨텍스트 상호작용

### Flow Context → Node Context
```yaml
# 플로우 컨텍스트 값을 노드 컨텍스트로 전달
node_processor:
  type: "DataProcessor"
  inputs:
    user_data: "${flow_context.variables.user_id}"
    config:
      timeout: "${flow_context.metadata.default_timeout}"
```

### Node Context → Flow Context
```yaml
# 노드 실행 결과를 플로우 컨텍스트에 저장
node_processor:
  type: "DataProcessor"
  outputs:
    processed_count: 
      target: "flow_context.variables.total_processed"
      operation: "set"
```

### 에러 전파
```yaml
# 노드 에러를 플로우 컨텍스트로 전파
error_handling:
  on_node_error:
    - action: "log_to_flow_context"
    - action: "increment_error_count"
    - condition: "${flow_context.errors.length} > 5"
      action: "stop_workflow"
```

## 📋 컨텍스트 활용 패턴

### 1. 조건부 실행
```yaml
conditional_node:
  type: "ConditionalProcessor"
  conditions:
    - condition: "${flow_context.variables.user_type} == 'premium'"
      action: "execute"
    - condition: "${node_context.errors.length} > 0"
      action: "skip"
```

### 2. 에러 누적 관리
```yaml
error_monitor:
  type: "ErrorMonitor"
  inputs:
    flow_errors: "${flow_context.errors}"
    current_errors: "${node_context.errors}"
  actions:
    - threshold: 3
      action: "send_alert"
    - threshold: 10
      action: "stop_workflow"
```

### 3. 진행률 추적
```yaml
progress_tracker:
  type: "ProgressTracker"
  inputs:
    total_nodes: "${flow_context.metadata.total_nodes}"
    completed_nodes: "${flow_context.metadata.completed_nodes}"
    current_progress: "${node_context.internal_state.progress}"
```

### 4. 캐시 관리
```yaml
cache_manager:
  type: "CacheManager"
  inputs:
    cache_key: "${flow_context.variables.cache_prefix}_${node_context.metadata.node_id}"
    data: "@previous_node.result"
  outputs:
    cached_result:
      target: "node_context.internal_state.cache"
```

## 🛠 구현 가이드

### Python (gil-py) 구현
```python
class FlowContext:
    def __init__(self):
        self.variables = {}
        self.errors = []
        self.metadata = {}
        self.shared_data = {}
    
    def set_variable(self, key: str, value: Any):
        self.variables[key] = value
    
    def add_error(self, node_id: str, error: str):
        self.errors.append({
            "node": node_id,
            "message": error,
            "timestamp": datetime.now().isoformat()
        })

class NodeContext:
    def __init__(self, node_id: str):
        self.variables = {}
        self.errors = []
        self.metadata = {"node_id": node_id}
        self.internal_state = {}
```

### 컨텍스트 참조 문법
```yaml
# 플로우 컨텍스트 참조
"${flow_context.variables.key}"
"${flow_context.errors.length}"
"${flow_context.metadata.workflow_id}"

# 노드 컨텍스트 참조  
"${node_context.variables.key}"
"${node_context.errors[0].message}"
"${node_context.metadata.retry_count}"

# 조건부 참조
"${flow_context.variables.key|default_value}"
"${node_context.errors.length > 0 ? 'error' : 'success'}"
```

## 📊 컨텍스트 라이프사이클

### Flow Context
1. **초기화**: 워크플로우 시작 시 생성
2. **변수 설정**: environment 섹션에서 초기 변수 로드
3. **노드 실행**: 각 노드에서 읽기/쓰기 가능
4. **상태 누적**: 에러, 메타데이터 지속적 업데이트
5. **종료**: 워크플로우 완료 시 최종 상태 반환

### Node Context
1. **생성**: 노드 실행 직전 생성
2. **초기화**: 노드별 설정 및 상태 로드
3. **실행**: 노드 내부에서만 접근 가능
4. **정리**: 필요한 데이터만 Flow Context로 전파
5. **소멸**: 노드 실행 완료 후 정리

이 컨텍스트 시스템을 통해 Gil-Flow는 복잡한 워크플로우에서도 상태를 체계적으로 관리하고, 노드 간 안전한 데이터 공유를 지원합니다.
