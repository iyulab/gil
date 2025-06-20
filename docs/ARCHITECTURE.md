# Gil 아키텍처 가이드

이 문서는 Gil의 언어 중립적 아키텍처와 다양한 언어 구현체 간의 상호 운용성을 설명합니다. Gil-Flow 표준이 실제로 어떻게 구현되고 확장되는지에 대한 가이드를 제공합니다.

> **참고**: Gil-Flow YAML 문법은 [YAML_SPEC.md](YAML_SPEC.md)를, 노드 타입은 [NODE_SPEC.md](NODE_SPEC.md)를 참조하세요.

## 🏗 Gil 아키텍처 개요

```
┌─────────────────────────────────────────────────────────────┐
│                    Gil-Flow YAML                            │
│                (언어 중립적 워크플로우 정의)                   │
└─────────────────────┬───────────────────────────────────────┘
                      │
         ┌────────────┼────────────┐
         │            │            │
    ┌────▼────┐  ┌────▼────┐  ┌────▼────┐
    │ gil-py  │  │gil-sharp│  │gil-node │
    │(Python) │  │  (C#)   │  │(Node.js)│
    └────┬────┘  └────┬────┘  └────┬────┘
         │            │            │
    ┌────▼──────────────────────────▼────┐
    │        표준 노드 인터페이스          │
    │     (DataFile, TransformData,      │
    │     AITextGen, CommAPI, etc.)      │
    └────────────────────────────────────┘
```

## 📋 구현체별 개발 현황

### ✅ gil-py (Python 구현체)
- **상태**: 프로토타입 완성
- **특징**: 
  - 빠른 프로토타이핑과 AI 통합
  - Rich Python 생태계 활용
  - Jupyter Notebook 지원
- **주요 사용처**: 데이터 분석, AI/ML 워크플로우, 프로토타이핑

### 🚧 gil-sharp (C# 구현체) - 계획 중
- **목표**: 엔터프라이즈급 성능과 안정성
- **특징**: 
  - .NET 생태계 통합
  - 높은 성능과 메모리 효율성
  - Azure/Microsoft 서비스 네이티브 연동
- **주요 사용처**: 엔터프라이즈 시스템, 고성능 처리

### 🚧 gil-node (Node.js 구현체) - 계획 중
- **목표**: 웹 통합과 실시간 처리
- **특징**: 
  - 비동기 I/O 최적화
  - 웹 서비스와 자연스러운 통합
  - NPM 생태계 활용
- **주요 사용처**: 웹 서비스, 실시간 데이터 처리

## 🔧 노드 개발 표준

### 노드 인터페이스
모든 구현체에서 노드는 동일한 인터페이스를 구현해야 합니다:

**핵심 메서드:**
- `run(inputs)`: 노드 실행 (비동기)
- `validate_inputs(inputs)`: 입력 검증
- `get_schema()`: 노드 스키마 반환

**구현 예시:**
```
// 의사코드 (Pseudocode)
class GilNode {
    constructor(config) { this.config = config }
    
    async run(inputs) {
        // 1. 입력 검증
        // 2. 노드 로직 실행
        // 3. 표준 형식으로 결과 반환
    }
    
    validate_inputs(inputs) { /* 타입/필수값 검증 */ }
    get_schema() { /* 노드 스키마 정의 */ }
}
```
        this.config = config;
    }
    
    async run(inputs) {
        // 표준 실행 인터페이스
        throw new Error('Must be implemented by subclass');
    }
    
    validateInputs(inputs) {
        // 입력 검증
        return true;
    }
    
    getSchema() {
        // 노드 스키마 반환
        return {};
    }
}
```

### 2. 표준 출력 형식

모든 노드는 다음 형식으로 결과를 반환해야 합니다:

```json
{
  "success": true,              // 실행 성공 여부
  "data": {                     // 실제 출력 데이터
    "result": "...",
    "count": 100
  },
  "metadata": {                 // 실행 메타데이터
    "execution_time": 1234,     // 실행 시간 (ms)
    "timestamp": "2025-01-01T00:00:00Z",
    "node_type": "DataFile",
    "version": "1.0.0"
  },
  "error": null                 // 에러 정보 (실패 시)
}
```

### 3. 에러 처리 표준

```json
{
  "success": false,
  "data": null,
  "metadata": {
    "execution_time": 500,
    "timestamp": "2025-01-01T00:00:00Z",
    "node_type": "DataFile",
    "version": "1.0.0"
  },
  "error": {
    "code": "FILE_NOT_FOUND",
    "message": "지정된 파일을 찾을 수 없습니다: /path/to/file.txt",
    "details": {
      "file_path": "/path/to/file.txt",
      "attempted_encodings": ["utf-8", "cp949"]
    },
    "recoverable": true         // 재시도 가능 여부
  }
}
```

## 🔄 워크플로우 엔진 표준

### 1. YAML 파서
모든 구현체는 Gil-Flow YAML을 동일하게 해석해야 합니다:

```yaml
# 표준 워크플로우 정의
gil_flow_version: "1.0"
name: "Standard Workflow"
description: "Language-neutral workflow example"

nodes:
  data_reader:
    type: "DataFile"
    config:
      operation: "read"
    inputs:
      file_path: "./data/input.txt"
      encoding: "utf-8"
  
  processor:
    type: "TransformData"
    inputs:
      data: "@data_reader.content"
      operations:
        - type: "filter"
          condition: "length > 10"

flow:
  - data_reader
  - processor

outputs:
  result:
    source: "@processor.data"
```

### 2. 참조 해결 (Reference Resolution)
모든 구현체는 동일한 참조 문법을 지원해야 합니다:

- `@node_name.output_port` - 노드 출력 참조
- `${environment.VARIABLE}` - 환경 변수 참조  
- `${system.current_time}` - 시스템 값 참조
- `${input.parameter}` - 워크플로우 입력 참조

### 3. 실행 엔진
표준 실행 순서와 병렬 처리:

```python
# 의사코드 - 모든 구현체에서 동일한 로직
async def execute_workflow(nodes, flow, inputs):
    execution_order = calculate_topological_order(nodes, flow)
    results = {}
    
    for step in execution_order:
        if isinstance(step, list):  # 병렬 실행
            parallel_results = await execute_parallel(step, results, inputs)
            results.update(parallel_results)
        else:  # 순차 실행
            node_result = await execute_node(step, results, inputs)
            results[step] = node_result
    
    return results
```

## 📊 표준 노드 라이브러리

### 1. 데이터 처리 노드
- **DataFile**: 파일 읽기/쓰기
- **DataCSV**: CSV 처리
- **DataJSON**: JSON 처리
- **DataDatabase**: 데이터베이스 연산
- **DataExcel**: Excel 파일 처리

### 2. 변환 노드
- **TransformData**: 데이터 필터링, 매핑, 정렬
- **TransformTemplate**: 템플릿 엔진 (Jinja2, Mustache 등)
- **TransformAggregate**: 집계 연산
- **TransformValidate**: 데이터 유효성 검사

### 3. AI 노드
- **AITextGeneration**: 텍스트 생성 (GPT, Claude 등)
- **AIImageGeneration**: 이미지 생성 (DALL-E, Midjourney 등)
- **AIAnalyzeText**: 텍스트 분석 (감정, 의도, 개체명 인식)
- **AITranslate**: 번역 서비스

### 4. 통신 노드
- **CommAPI**: REST API 호출
- **CommEmail**: 이메일 발송
- **CommSlack**: 슬랙 메시지
- **CommWebhook**: 웹훅 호출

### 5. 제어 노드
- **ControlCondition**: 조건부 실행
- **ControlLoop**: 반복 처리
- **ControlMerge**: 데이터 병합
- **ControlSplit**: 데이터 분리

## 🛠 개발 환경 설정

### 공통 개발 도구

1. **Gil-Flow Validator**: YAML 워크플로우 검증 도구
2. **Gil Schema Generator**: 노드 스키마 자동 생성
3. **Gil Test Framework**: 크로스 플랫폼 테스트 도구
4. **Gil Visualizer**: 워크플로우 시각화 도구

### 개발 워크플로우

```bash
# 1. Gil-Flow YAML 작성
vim workflow.yaml

# 2. 문법 검증
gil-validate workflow.yaml

# 3. 스키마 검증  
gil-schema-check workflow.yaml

# 4. 테스트 실행
gil-test workflow.yaml --mock-data test-data.json

# 5. 시각화 확인
gil-visualize workflow.yaml --output diagram.png

# 6. 구현체별 실행
gil-py run workflow.yaml
gil-sharp run workflow.yaml  # 향후
gil-node run workflow.yaml   # 향후
```

## 🧪 테스트 전략

### 1. 단위 테스트
각 노드의 독립적 기능 테스트:

```yaml
# test-specs/data-file-test.yaml
test_suite: "DataFile Node Tests"
tests:
  - name: "Read text file"
    node:
      type: "DataFile"
      config:
        operation: "read"
      inputs:
        file_path: "./test-data/sample.txt"
        encoding: "utf-8"
    expected:
      success: true
      data:
        content: "Hello, World!"
        size: 13
```

### 2. 통합 테스트
워크플로우 전체 실행 테스트:

```yaml
# test-specs/workflow-integration-test.yaml
test_suite: "Workflow Integration Tests"
workflow: "./workflows/data-processing.yaml"
test_cases:
  - name: "Normal processing flow"
    inputs:
      source_file: "./test-data/normal-data.csv"
    expected_outputs:
      processed_count: 100
      success_rate: 1.0
```

### 3. 크로스 플랫폼 테스트
동일한 워크플로우가 모든 구현체에서 동일하게 동작하는지 검증:

```bash
# 동일한 워크플로우, 다른 구현체에서 실행
gil-py run test-workflow.yaml > py-result.json
gil-sharp run test-workflow.yaml > sharp-result.json
gil-node run test-workflow.yaml > node-result.json

# 결과 비교
gil-compare py-result.json sharp-result.json node-result.json
```

## 📚 문서화 표준

### 1. 노드 문서
각 노드는 다음 정보를 포함해야 합니다:

```markdown
# DataFile 노드

## 개요
파일 시스템과 상호작용하는 범용 파일 처리 노드

## 설정 (Config)
- `operation`: "read" | "write" | "append" - 파일 연산 타입
- `encoding`: string - 파일 인코딩 (기본값: "utf-8")

## 입력 (Inputs)
- `file_path`: string (필수) - 파일 경로
- `content`: string (쓰기 시 필수) - 파일 내용

## 출력 (Outputs)
- `content`: string - 파일 내용 (읽기 시)
- `size`: number - 파일 크기 (바이트)
- `path`: string - 실제 파일 경로

## 사용 예제
```yaml
file_reader:
  type: "DataFile"
  config:
    operation: "read"
    encoding: "utf-8"
  inputs:
    file_path: "./data/input.txt"
```

## 에러 코드
- `FILE_NOT_FOUND`: 파일을 찾을 수 없음
- `PERMISSION_DENIED`: 파일 접근 권한 없음
- `ENCODING_ERROR`: 인코딩 오류
```

### 2. 워크플로우 문서
워크플로우 예제와 설명:

```markdown
# 데이터 처리 파이프라인

이 워크플로우는 CSV 파일을 읽어 데이터를 변환하고 결과를 저장합니다.

## 워크플로우 구조
1. `csv_reader`: CSV 파일 읽기
2. `data_transformer`: 데이터 필터링 및 변환
3. `result_writer`: 처리 결과 저장

## 입력 요구사항
- `input_file`: CSV 파일 경로
- `filter_condition`: 필터링 조건

## 출력
- `processed_data`: 변환된 데이터
- `statistics`: 처리 통계
```

## 🔄 버전 관리와 호환성

### 1. 의미론적 버전 관리
- **Gil-Flow 표준**: `major.minor` (예: 1.0, 1.1, 2.0)
- **구현체 버전**: `major.minor.patch` (예: 1.0.1, 1.1.0)
- **노드 버전**: 개별 노드별 버전 관리

### 2. 호환성 매트릭스

| Gil-Flow | gil-py | gil-sharp | gil-node |
|----------|--------|-----------|----------|
| 1.0      | ✅ 1.0+ | 🚧 계획중   | 🚧 계획중  |
| 1.1      | 🚧 개발중 | 🚧 계획중   | 🚧 계획중  |

### 3. 마이그레이션 가이드
버전 업그레이드 시 필요한 변경사항을 문서화:

```yaml
# Gil-Flow 1.0 -> 1.1 마이그레이션
migration:
  version: "1.0 -> 1.1"
  breaking_changes:
    - "`timeout_ms` -> `timeout` (단위 변경: ms -> 초)"
    - "`retry_count` -> `retry.max_attempts`"
  new_features:
    - "조건부 실행 구문 추가"
    - "병렬 처리 성능 향상"
```

## 🚀 배포 및 패키징

### 1. 패키지 구조
```
gil-{language}/
├── src/                    # 소스 코드
│   ├── core/              # 핵심 엔진
│   ├── nodes/             # 표준 노드들
│   └── cli/               # CLI 도구
├── tests/                 # 테스트
├── docs/                  # 문서
├── examples/              # 예제
└── package.json/setup.py/gil.csproj  # 패키지 설정
```

### 2. 배포 채널
- **Python**: PyPI (`pip install gil-py`)
- **C#**: NuGet (`dotnet add package Gil.Sharp`)
- **Node.js**: NPM (`npm install gil-node`)

### 3. CLI 도구 통합
모든 구현체는 동일한 CLI 인터페이스 제공:

```bash
# 공통 CLI 명령어
gil run workflow.yaml
gil validate workflow.yaml  
gil visualize workflow.yaml
gil list-nodes
gil describe NodeType
gil test workflow.yaml
```

---

## 🤝 커뮤니티와 기여

### 1. 기여 방법
- **표준 개선**: Gil-Flow YAML 표준 제안
- **노드 개발**: 새로운 표준 노드 구현
- **구현체 개발**: 새로운 언어 구현체
- **문서화**: 사용 가이드와 예제 작성

### 2. 거버넌스
- **RFC 프로세스**: 주요 변경사항은 RFC로 제안
- **표준 위원회**: 언어별 대표자들로 구성
- **호환성 보장**: 모든 변경사항은 호환성 테스트 통과 필수

Gil은 언어 경계를 넘나드는 진정한 범용 워크플로우 프레임워크가 되는 것을 목표로 합니다. 각 언어의 장점을 살리면서도 표준화된 경험을 제공하여, 개발자들이 자신에게 가장 적합한 도구로 워크플로우를 구축할 수 있도록 합니다.
