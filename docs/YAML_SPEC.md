# Gil-Flow YAML 문법 표준 (v1.0)

**Gil-Flow YAML**은 언어 중립적인 노드 기반 워크플로우 정의 표준입니다. 이 문서는 Gil-Flow YAML의 문법, 구조, 참조 체계 등을 상세히 정의합니다.

> **참고**: Gil 프로젝트의 전체 개요는 [README.md](../README.md)를, 노드 인터페이스는 [NODE_SPEC.md](NODE_SPEC.md)를 참조하세요.

## 📋 기본 구조

```yaml
# Gil-Flow 워크플로우 정의
gil_flow_version: "1.0"           # 필수: Gil-Flow 표준 버전
name: "워크플로우 이름"            # 필수: 워크플로우 식별자
description: "워크플로우 설명"     # 선택: 워크플로우 목적과 기능

# 메타데이터
metadata:                         # 선택: 추가 메타정보
  author: "작성자"
  version: "1.0.0"
  created: "2025-01-01"
  tags: ["data", "ai", "automation"]

# 환경 설정
environment:                      # 선택: 환경 변수 및 전역 설정
  variables:
    API_KEY: "${API_KEY}"
    BASE_URL: "https://api.example.com"
  timeouts:
    default: 30000               # 기본 타임아웃 (ms)
    network: 60000               # 네트워크 타임아웃 (ms)

# 노드 정의
nodes:                           # 필수: 워크플로우 구성 노드들
  node_name:
    type: "NodeType"             # 필수: 노드 타입
    config: {}                   # 선택: 노드별 설정
    inputs: {}                   # 선택: 입력 데이터
    outputs: {}                  # 선택: 출력 매핑
    conditions: []               # 선택: 실행 조건
    retry: {}                    # 선택: 재시도 설정

# 실행 플로우
flow:                            # 필수: 노드 실행 순서
  - node_name
  - [parallel_node1, parallel_node2]
  - conditional_node

# 출력 정의
outputs:                         # 선택: 워크플로우 최종 출력
  result_name:
    source: "@node_name.output_port"
    format: "json"
```

## 🧩 노드 정의 표준

### 기본 노드 구조

```yaml
nodes:
  node_identifier:               # 워크플로우 내 고유 식별자
    type: "NodeType"             # 노드 타입 (표준 또는 커스텀)
    
    # 노드 설정 (노드 초기화 시 사용)
    config:
      parameter: "value"
      nested:
        key: "value"
    
    # 입력 데이터 (런타임 시 노드에 전달)
    inputs:
      input_port: "static_value"
      data_port: "@source_node.output_port"
      computed_port: "${environment.BASE_URL}/api"
    
    # 출력 매핑 (노드 출력을 다른 이름으로 매핑)
    outputs:
      custom_name: "original_output_port"
    
    # 실행 조건
    conditions:
      - condition: "@previous_node.status == 'success'"
        action: "execute"
      - condition: "@previous_node.error_count > 0"
        action: "skip"
    
    # 재시도 설정
    retry:
      max_attempts: 3
      delay: 1000                # ms
      backoff: "exponential"     # linear, exponential, fixed
      on_error: ["network_error", "timeout"]
    
    # 타임아웃 설정
    timeout: 30000               # ms
    
    # 메타데이터
    metadata:
      description: "노드 기능 설명"
      tags: ["processing", "data"]
```

### 노드 타입 분류

#### 1. 데이터 노드 (Data Nodes)
```yaml
# 파일 읽기
file_reader:
  type: "DataFile"
  config:
    operation: "read"
  inputs:
    file_path: "./data/input.csv"
    encoding: "utf-8"
    format: "csv"

# 데이터베이스 쿼리
db_query:
  type: "DataDatabase"
  config:
    connection_string: "${DB_CONNECTION}"
    driver: "postgresql"
  inputs:
    query: "SELECT * FROM users WHERE active = true"
    parameters: {}
```

#### 2. 변환 노드 (Transform Nodes)
```yaml
# 데이터 변환
data_transformer:
  type: "TransformData"
  inputs:
    data: "@file_reader.rows"
    operations:
      - type: "filter"
        condition: "age > 18"
      - type: "map"
        expression: "name.toUpperCase()"
      - type: "group_by"
        key: "department"

# 템플릿 처리
template_processor:
  type: "TransformTemplate"
  inputs:
    template: "@template_loader.content"
    variables:
      title: "${environment.TITLE}"
      date: "${system.current_date}"
```

#### 3. AI 노드 (AI Nodes)
```yaml
# 텍스트 생성
text_generator:
  type: "AITextGeneration"
  config:
    provider: "openai"
    model: "gpt-4"
  inputs:
    prompt: "Generate a summary of: @data_transformer.result"
    max_tokens: 500
    temperature: 0.7

# 이미지 생성
image_generator:
  type: "AIImageGeneration"
  config:
    provider: "openai"
    model: "dall-e-3"
  inputs:
    prompt: "@text_generator.summary"
    size: "1024x1024"
    style: "vivid"
```

#### 4. 통신 노드 (Communication Nodes)
```yaml
# API 호출
api_caller:
  type: "CommAPI"
  config:
    base_url: "${environment.API_BASE}"
  inputs:
    endpoint: "/users"
    method: "POST"
    headers:
      Authorization: "Bearer ${environment.TOKEN}"
    body: "@data_transformer.result"

# 이메일 발송
email_sender:
  type: "CommEmail"
  config:
    smtp_server: "${environment.SMTP_SERVER}"
    smtp_port: 587
  inputs:
    to: "@user_data.email"
    subject: "Processing Complete"
    body: "@text_generator.summary"
    attachments: ["@file_writer.file_path"]
```

#### 5. 제어 노드 (Control Nodes)
```yaml
# 조건부 실행
conditional_processor:
  type: "ControlCondition"
  inputs:
    condition: "@data_validator.is_valid"
    if_true: 
      - "process_data"
      - "send_notification"
    if_false:
      - "log_error"
      - "send_alert"

# 반복 처리
loop_processor:
  type: "ControlLoop"
  inputs:
    items: "@data_reader.items"
    max_iterations: 100
    parallel: true
    batch_size: 10
    operations:
      - "transform_item"
      - "validate_item"
```

## 🔄 데이터 흐름과 참조 체계

### 1. 참조 문법 (Reference Syntax)

```yaml
# 노드 출력 참조
"@node_name.output_port"         # 특정 출력 포트
"@node_name"                     # 기본 출력 (output 포트)
"@node_name.result.nested.value" # 중첩된 값 접근

# 환경 변수 참조
"${environment.VARIABLE_NAME}"   # 환경 변수
"${system.current_time}"         # 시스템 값
"${workflow.name}"               # 워크플로우 메타데이터

# 조건부 참조 (기본값 포함)
"${environment.API_KEY|default_key}"        # 기본값 지정
"@node_name.result|@fallback_node.result"  # 폴백 참조
```

### 2. 데이터 타입 시스템

```yaml
# 기본 타입
inputs:
  text_input: "string value"        # string
  number_input: 42                  # number
  boolean_input: true               # boolean
  null_input: null                  # null

# 복합 타입
  array_input:                      # array
    - "item1"
    - "item2"
  object_input:                     # object
    key1: "value1"
    key2: "value2"

# 타입 검증
  validated_input:
    value: "@source_node.output"
    type: "number"                  # 타입 강제
    constraints:
      min: 0
      max: 100
```

### 3. 조건부 실행

```yaml
# 단순 조건
conditions:
  - condition: "@validator.is_valid == true"
    action: "execute"
  - condition: "@previous_node.error != null"
    action: "skip"

# 복합 조건
conditions:
  - condition: "(@data_count.value > 100) && (@time.hour >= 9)"
    action: "execute"
  - condition: "@user.role in ['admin', 'manager']"
    action: "execute"
  - condition: "default"
    action: "skip"

# 조건부 입력
inputs:
  dynamic_input: |
    {% if validator.is_premium %}
      @premium_processor.result
    {% else %}
      @standard_processor.result
    {% endif %}
```

## 🔀 실행 플로우 정의

### 1. 순차 실행
```yaml
flow:
  - step1
  - step2
  - step3
```

### 2. 병렬 실행
```yaml
flow:
  - setup_node
  - [parallel_node1, parallel_node2, parallel_node3]
  - merge_results
```

### 3. 조건부 분기
```yaml
flow:
  - input_validator
  - decision_point:
      type: "ControlSwitch"
      inputs:
        condition: "@input_validator.category"
        cases:
          "type_a": ["process_a", "output_a"]
          "type_b": ["process_b", "output_b"]
          "default": ["error_handler"]
```

### 4. 반복 및 루프
```yaml
flow:
  - data_loader
  - batch_processor:
      type: "ControlLoop"
      inputs:
        items: "@data_loader.items"
        operations:
          - "validate_item"
          - "transform_item"
          - "save_item"
        parallel: true
        batch_size: 10
```

## 🔧 구성 및 환경 관리

### 1. 환경 변수 시스템
```yaml
environment:
  variables:
    # 정적 값
    APP_NAME: "MyWorkflow"
    VERSION: "1.0.0"
    
    # 외부 환경 변수
    API_KEY: "${API_KEY}"
    DATABASE_URL: "${DATABASE_URL}"
    
    # 계산된 값
    TIMESTAMP: "${system.current_timestamp}"
    WORKFLOW_ID: "${system.uuid}"
    
    # 조건부 값
    ENVIRONMENT: "${DEPLOY_ENV|development}"
    DEBUG_MODE: "${DEBUG|false}"

  # 전역 설정
  settings:
    timeout: 300000              # 기본 타임아웃
    retry_count: 3               # 기본 재시도
    log_level: "INFO"            # 로그 레벨
    parallel_limit: 5            # 병렬 실행 제한
```

### 2. 설정 상속 및 오버라이드
```yaml
# 기본 설정 템플릿
defaults: &default_config
  timeout: 30000
  retry:
    max_attempts: 3
    delay: 1000

# 노드에서 상속 및 확장
nodes:
  api_node:
    <<: *default_config          # YAML 앵커로 기본값 상속
    type: "CommAPI"
    config:
      timeout: 60000             # 특정 설정만 오버라이드
```

## 🎛 고급 기능

### 1. 동적 워크플로우
```yaml
# 런타임에 노드 생성
dynamic_nodes:
  generator: "@config_loader.node_definitions"
  template:
    type: "ProcessorTemplate"
    inputs:
      data: "@source.${item.source}"
      config: "${item.config}"

# 조건부 노드 정의
conditional_nodes:
  feature_processor:
    enabled: "${environment.ENABLE_FEATURE_X}"
    type: "FeatureProcessor"
    inputs:
      data: "@input_validator.data"
```

### 2. 에러 처리 및 복구
```yaml
# 글로벌 에러 핸들러
error_handling:
  default_handler: "log_error"
  timeout_handler: "retry_node"
  critical_handler: "stop_workflow"

# 노드별 에러 처리
nodes:
  risky_operation:
    type: "RiskyProcessor"
    error_handling:
      on_error: "continue"       # continue, stop, retry
      fallback: "@backup_processor.result"
      notify: ["admin@company.com"]
```

### 3. 모니터링 및 로깅
```yaml
# 워크플로우 모니터링
monitoring:
  metrics:
    - execution_time
    - memory_usage
    - throughput
  alerts:
    - condition: "execution_time > 300000"
      action: "send_alert"
    - condition: "error_rate > 0.1"
      action: "escalate"

# 노드 로깅
nodes:
  processor:
    type: "DataProcessor"
    logging:
      level: "DEBUG"
      include: ["inputs", "outputs", "performance"]
      exclude: ["sensitive_data"]
```

## 📊 출력 및 결과 처리

### 1. 출력 정의
```yaml
outputs:
  # 단순 출력
  processed_data:
    source: "@processor.result"
    
  # 변환된 출력
  summary:
    source: "@processor.statistics"
    transform:
      format: "json"
      fields: ["count", "average", "max"]
  
  # 조건부 출력
  conditional_result:
    source: |
      {% if processor.success %}
        @processor.result
      {% else %}
        @fallback.result
      {% endif %}
  
  # 집계 출력
  aggregated:
    sources:
      - "@node1.result"
      - "@node2.result" 
      - "@node3.result"
    aggregation: "merge"         # merge, concat, sum, etc.
```

### 2. 결과 형식화
```yaml
outputs:
  formatted_result:
    source: "@processor.data"
    format:
      type: "json"
      schema:
        type: "object"
        properties:
          timestamp: "${system.current_timestamp}"
          workflow: "${workflow.name}"
          data: "@processor.data"
          metadata:
            execution_time: "${system.execution_time}"
            node_count: "${workflow.node_count}"
```

## 🔄 버전 관리 및 호환성

### 1. 버전 지정
```yaml
gil_flow_version: "1.0"          # 필수: 사용하는 Gil-Flow 표준 버전
schema_version: "2024-01"        # 선택: 스키마 버전
compatibility: ["1.0", "1.1"]   # 선택: 호환 가능한 버전들
```

### 2. 마이그레이션 가이드
```yaml
# v1.0에서 v1.1로 마이그레이션 시
migration:
  from: "1.0"
  to: "1.1"
  changes:
    - "inputs.condition -> conditions[0].condition"
    - "retry_count -> retry.max_attempts"
    - "timeout_ms -> timeout"
```

## ✅ 검증 및 테스트

### 1. 스키마 검증
```yaml
# 워크플로우 검증 설정
validation:
  strict_mode: true              # 엄격한 검증
  required_fields: ["name", "nodes", "flow"]
  custom_validators:
    - "node_type_exists"
    - "circular_dependency_check"
    - "resource_availability"
```

### 2. 테스트 정의
```yaml
# 워크플로우 테스트
tests:
  unit_tests:
    - node: "data_processor"
      inputs:
        data: "test_data.json"
      expected_outputs:
        count: 100
        average: 42.5
  
  integration_tests:
    - scenario: "full_pipeline"
      inputs:
        source_file: "integration_test_data.csv"
      assertions:
        - "@final_output.success == true"
        - "@final_output.record_count > 0"
```

## 🌟 최적화 및 성능

### 1. 성능 최적화
```yaml
# 성능 설정
performance:
  caching:
    enabled: true
    ttl: 3600000                 # 캐시 TTL (ms)
    storage: "memory"            # memory, redis, file
  
  parallel_execution:
    max_concurrent: 10           # 최대 동시 실행
    resource_limit:
      cpu: "80%"
      memory: "2GB"
  
  optimization:
    lazy_loading: true           # 지연 로딩
    data_streaming: true         # 스트리밍 처리
    batch_processing: 1000       # 배치 크기
```

### 2. 리소스 관리
```yaml
# 리소스 제한
resources:
  limits:
    execution_time: 1800000      # 최대 실행 시간 (30분)
    memory_usage: "4GB"          # 메모리 제한
    disk_space: "10GB"           # 디스크 공간 제한
    
  allocation:
    cpu_cores: 4                 # CPU 코어 수
    gpu_enabled: false           # GPU 사용 여부
```

---

이 표준은 Gil-Flow v1.0을 기준으로 작성되었으며, 향후 커뮤니티 피드백과 실제 사용 경험을 바탕으로 발전해 나갈 예정입니다.

**참고**: 이 문서는 언어 중립적 표준이므로, 특정 구현체(gil-py, gil-sharp, gil-node)의 세부사항과 다를 수 있습니다. 각 구현체별 문서를 추가로 참조해 주세요.
