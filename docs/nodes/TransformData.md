# DataTransform 노드

제공된 Python 표현식을 사용하여 입력 데이터를 변환합니다.

## 설정 (config)

*   `transform_expression` (필수, 텍스트): 입력 데이터에 적용할 Python 표현식입니다. 표현식 내에서 `data` 변수를 사용하여 입력 데이터에 액세스할 수 있습니다.

## 입력 (inputs)

*   `input_data` (필수, 모든 타입): 변환할 데이터입니다.

## 출력 (outputs)

*   `output_data` (모든 타입): 변환된 데이터입니다.

## 예시

```yaml
multiply_by_two:
  type: "DataTransform"
  config:
    transform_expression: "data * 2"
  inputs:
    input_data: 5
```

이 예시는 `input_data` 5를 2배로 늘려 `output_data`로 10을 반환합니다.