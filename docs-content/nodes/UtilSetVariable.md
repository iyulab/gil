# UtilSetVariable 노드

워크플로우 컨텍스트에 변수를 설정합니다. 설정된 변수는 다른 노드에서 컨텍스트 표현식을 사용하여 액세스할 수 있습니다.

## 설정 (config)

*   `variable_name` (필수, 텍스트): 설정할 변수의 이름입니다.

## 입력 (inputs)

*   `value` (필수, 모든 타입): 변수에 설정할 값입니다.

## 출력 (outputs)

없음. 이 노드는 워크플로우 컨텍스트를 직접 수정합니다.

## 예시

```yaml
set_workflow_status:
  type: "UtilSetVariable"
  config:
    variable_name: "workflow_status"
  inputs:
    value: "completed"
```
