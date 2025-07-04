# Gil-Flow 컨텍스트 시스템

Gil-Flow에서는 워크플로우 실행 중 상태와 정보를 관리하기 위한 컨텍스트 시스템을 제공합니다.

## 📊 컨텍스트 (Context)

**Context**는 워크플로우 실행 중에 노드 간에 데이터를 전달하고 참조를 해결하는 데 사용되는 간단한 키-값 저장소입니다.

### 구조
```python
class Context:
    def __init__(self, initial_data: Dict[str, Any] = None):
        self._data = initial_data if initial_data is not None else {}

    def to_dict(self) -> Dict[str, Any]:
        return self._data

    def get(self, key: str, default: Any = None) -> Any:
        return self._data.get(key, default)

    def set(self, key: str, value: Any):
        self._data[key] = value

    def resolve_reference(self, value: Any) -> Any:
        if isinstance(value, str) and value.startswith("$"):
            key = value[1:]
            return self.get(key, value)  # Return original value if not found
        return value
```

### 주요 기능
- **데이터 저장**: 워크플로우 실행 중 필요한 모든 데이터를 저장합니다.
- **참조 해결**: `${variable_name}` 형식의 문자열 참조를 실제 값으로 해결합니다.

### 접근 방법
```yaml
# YAML에서 접근
inputs:
  user_id: "${user_data.id}"
  api_key: "${env.OPENAI_API_KEY}"
```

## 🔄 컨텍스트 상호작용

노드는 `execute` 메서드의 `context` 인수를 통해 컨텍스트에 액세스하고 수정할 수 있습니다.

```python
async def execute(self, data: dict, context: Context) -> dict:
    # 컨텍스트에서 값 가져오기
    api_key = context.get("env.OPENAI_API_KEY")

    # 컨텍스트에 값 설정
    context.set("processed_count", 10)

    # 참조 해결
    resolved_prompt = context.resolve_reference("${user_input.prompt}")
```

## 📊 컨텍스트 라이프사이클

1.  **초기화**: 워크플로우 시작 시 `GilWorkflow`에 의해 생성됩니다.
2.  **데이터 주입**: 워크플로우 YAML의 `environment` 섹션 또는 `run` 메서드의 `inputs`를 통해 초기 데이터가 주입될 수 있습니다.
3.  **노드 실행**: 각 노드는 `execute` 메서드를 통해 컨텍스트에 액세스하고 수정할 수 있습니다.
4.  **종료**: 워크플로우 완료 시 최종 컨텍스트 상태를 포함하는 결과가 반환될 수 있습니다.

이 컨텍스트 시스템을 통해 Gil-Flow는 복잡한 워크플로우에서도 상태를 체계적으로 관리하고, 노드 간 안전한 데이터 공유를 지원합니다.