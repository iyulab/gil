# Control-Branch 노드

부울 조건에 따라 실행 흐름을 지시합니다. 조건이 참이면 입력 데이터가 `true_output`으로 전달됩니다. 그렇지 않으면 `false_output`으로 전달됩니다.

## 설정 (config)

없음.

## 입력 (inputs)

*   `condition` (필수, 부울): 분기를 결정하는 부울 값입니다.
*   `input` (필수, 모든 타입): 선택한 분기로 전달할 데이터입니다.

## 출력 (outputs)

*   `true_output` (모든 타입): 조건이 참일 때의 출력입니다.
*   `false_output` (모든 타입): 조건이 거짓일 때의 출력입니다.

## 예시

```yaml
check_value:
  type: "Control-Branch"
  inputs:
    condition: "@some_node.is_valid"
    input: "@data_source.result"
```