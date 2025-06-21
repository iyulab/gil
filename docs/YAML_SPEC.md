# Gil-Flow YAML 문법 표준 (v1.0)

**Gil-Flow YAML**은 언어 중립적인 노드 기반 워크플로우 정의 표준입니다.

> **참고**: 전체 개요는 [README.md](../README.md), 노드 사양은 [NODE_SPEC.md](NODE_SPEC.md) 참조

## 기본 구조

```yaml
gil_flow_version: "1.0"
name: "워크플로우 이름"
description: "설명"

nodes:
  node_id:
    type: "NodeType"
    config: {}
    inputs: {}
    outputs: {}
    conditions: []

flow:
  - node_id
  - [parallel_1, parallel_2]

outputs:
  result: "@node_id.output"
```

## 필수 필드

| 필드 | 설명 | 예시 |
|------|------|------|
| `gil_flow_version` | Gil-Flow 표준 버전 | `"1.0"` |
| `name` | 워크플로우 식별자 | `"데이터 처리"` |
| `nodes` | 노드 정의 목록 | 아래 참조 |
| `flow` | 실행 순서 | `["step1", "step2"]` |

## 노드 정의

### 기본 구조
```yaml
node_id:
  type: "NodeType"               # 필수: 노드 타입
  config: {}                     # 선택: 설정
  inputs: {}                     # 선택: 입력 데이터
  outputs: {}                    # 선택: 출력 매핑
```

### 주요 노드 타입
- **DataFile**: 파일 읽기/쓰기
- **TransformData**: 데이터 변환
- **AITextGen**: AI 텍스트 생성
- **AIImageGen**: AI 이미지 생성
- **CommAPI**: API 호출
- **ControlCondition**: 조건부 실행

## 참조 체계

### 환경 변수
```yaml
config:
  api_key: "${OPENAI_API_KEY}"   # 환경 변수 참조
```

### 노드 출력 참조
```yaml
inputs:
  data: "@previous_node.result"  # 이전 노드 출력 참조
```

### 조건부 실행
```yaml
conditions:
  - "@validator.is_valid == true"
```

## 실행 플로우

### 순차 실행
```yaml
flow:
  - step1
  - step2
  - step3
```

### 병렬 실행
```yaml
flow:
  - init
  - [parallel_1, parallel_2, parallel_3]
  - merge
```

### 조건부 플로우
```yaml
flow:
  - validator
  - condition:
      if: "@validator.is_valid"
      then: [process_valid]
      else: [handle_error]
```

## 출력 정의

### 단순 출력
```yaml
outputs:
  result: "@final_node.output"
```

### 복합 출력
```yaml
outputs:
  processed_data: "@transform.result"
  metadata: "@analyzer.summary"
  status: "@validator.status"
```

## 고급 기능

### 재시도 설정
```yaml
node_id:
  type: "CommAPI"
  retry:
    max_attempts: 3
    delay: 1000
    backoff: "exponential"
```

### 타임아웃 설정
```yaml
node_id:
  type: "AITextGen"
  timeout: 30000                 # 30초
```

### 메타데이터 (선택)
```yaml
metadata:
  author: "작성자"
  version: "1.0.0"
  tags: ["data", "ai"]
```

## 예시 워크플로우

### 이미지 생성
```yaml
gil_flow_version: "1.0"
name: "AI 이미지 생성"

nodes:
  openai:
    type: "GilConnectorOpenAI"
    config:
      api_key: "${OPENAI_API_KEY}"
  
  generator:
    type: "GilGenImage"
    config:
      connector: "@openai"
    inputs:
      prompt: "아름다운 일몰"
      size: "1024x1024"

flow:
  - openai
  - generator

outputs:
  image: "@generator.image_url"
```

### 데이터 파이프라인
```yaml
gil_flow_version: "1.0"
name: "데이터 처리 파이프라인"

nodes:
  reader:
    type: "DataFile"
    inputs:
      path: "data.csv"
      format: "csv"
  
  transformer:
    type: "TransformData"
    inputs:
      data: "@reader.content"
      operation: "filter"
      condition: "age > 18"
  
  writer:
    type: "DataFile"
    inputs:
      data: "@transformer.result"
      path: "output.json"
      format: "json"

flow:
  - reader
  - transformer
  - writer

outputs:
  processed_count: "@transformer.count"
  output_file: "@writer.path"
```

---

*상세한 노드 인터페이스는 [NODE_SPEC.md](NODE_SPEC.md)에서 확인하세요.*
