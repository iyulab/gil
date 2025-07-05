# UtilLogMessage 노드

입력 데이터를 콘솔에 로깅하는 유틸리티 노드입니다. 디버깅 목적으로 사용됩니다.

## 설정 (config)

*   `prefix` (선택, 텍스트): 로그 메시지 앞에 추가할 접두사입니다. 기본값은 `[노드 ID]`입니다.

## 입력 (inputs)

*   `input` (필수, 모든 타입): 로깅할 데이터입니다.

## 출력 (outputs)

*   `output` (모든 타입): 로깅된 것과 동일한 데이터입니다.

## 예시

```yaml
log_data:
  type: "UtilLogMessage"
  config:
    prefix: "Workflow Debug"
  inputs:
    input: "@previous_node.result"
```
