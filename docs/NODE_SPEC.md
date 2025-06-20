# Gil-Flow 노드 타입 표준 (v1.0)

이 문서는 Gil-Flow에서 사용되는 표준 노드 타입들과 각 노드의 인터페이스를 정의합니다. 모든 구현체는 이 표준을 준수하여 언어에 관계없이 동일한 동작을 보장해야 합니다.

> **참고**: Gil-Flow YAML 문법은 [YAML_SPEC.md](YAML_SPEC.md)를, 아키텍처 가이드는 [ARCHITECTURE.md](ARCHITECTURE.md)를 참조하세요.

## 📋 노드 인터페이스 표준

### 기본 구조
```yaml
node_name:
  type: "NodeType"              # 표준 노드 타입
  config:                       # 노드별 설정
    parameter: "value"
  inputs:                       # 입력 포트
    input_port: "value"
    reference: "@other.output"
  timeout: 30000               # 실행 타임아웃 (ms)
```

### 출력 형식
모든 노드는 다음 구조로 결과를 반환합니다:
```json
{
  "success": true,             // 실행 성공 여부
  "data": { },                // 실제 출력 데이터
  "metadata": {               // 메타데이터
    "execution_time": 100,
    "node_type": "NodeType"
  },
  "error": null               // 에러 정보 (실패 시)
}
```
  "success": true,               // 실행 성공 여부
  "data": {},                   // 주요 출력 데이터
  "metadata": {
    "execution_time": 1234,     // 실행 시간 (ms)
    "timestamp": "2025-01-01T00:00:00Z",
    "node_type": "NodeType",
    "version": "1.0.0"
  },
  "error": null                 // 에러 정보 (실패 시)
}
```

## 📊 표준 노드 타입

### 데이터 노드 (Data Nodes)
| 노드 타입 | 목적 | 주요 입력 | 주요 출력 |
|-----------|------|-----------|-----------|
| DataFile | 파일 읽기/쓰기 | file_path, content | content, file_info |
| DataCSV | CSV 파일 처리 | file_path, data | rows, headers |
| DataDatabase | DB 연산 | sql, parameters | rows, affected_rows |
| DataJSON | JSON 처리 | data, schema | parsed_data, validated |
| DataExcel | Excel 처리 | file_path, sheet | data, sheets |

### 변환 노드 (Transform Nodes)
| 노드 타입 | 목적 | 주요 입력 | 주요 출력 |
|-----------|------|-----------|-----------|
| TransformData | 데이터 변환 | data, operations | transformed_data |
| TransformTemplate | 템플릿 처리 | template, variables | rendered_content |
| TransformValidate | 데이터 검증 | data, schema | validated_data, errors |
| TransformAggregate | 집계 연산 | data, group_by | aggregated_data |

### AI 노드 (AI Nodes)
| 노드 타입 | 목적 | 주요 입력 | 주요 출력 |
|-----------|------|-----------|-----------|
| AITextGeneration | 텍스트 생성 | prompt, model | generated_text |
| AIImageGeneration | 이미지 생성 | prompt, size | image_url, image_data |
| AIAnalyzeText | 텍스트 분석 | text, analysis_types | sentiment, entities |
| AITranslate | 번역 | text, target_language | translated_text |

### 통신 노드 (Communication Nodes)
| 노드 타입 | 목적 | 주요 입력 | 주요 출력 |
|-----------|------|-----------|-----------|
| CommAPI | REST API 호출 | url, method, data | response, status_code |
| CommEmail | 이메일 발송 | to, subject, body | sent_status, message_id |
| CommSlack | 슬랙 메시지 | channel, message | sent_status, timestamp |
| CommWebhook | 웹훅 호출 | url, payload | response, delivered |

### 제어 노드 (Control Nodes)
| 노드 타입 | 목적 | 주요 입력 | 주요 출력 |
|-----------|------|-----------|-----------|
| ControlCondition | 조건부 실행 | condition, then_data | result, branch_taken |
| ControlLoop | 반복/배치 처리 | items, operation | processed_items |
| ControlMerge | 데이터 병합 | datasets, merge_key | merged_data |
| ControlSplit | 데이터 분할 | data, split_criteria | split_results |

```yaml
data_transformer:
  type: "TransformData"
  inputs:
    data: "@source_node.data"
    operations:
      - type: "filter"
        condition: "age >= 18"
      - type: "map"
        expression: "name.toUpperCase()"
      - type: "sort"
        key: "age"
        order: "desc"
```

**출력 스키마:**
```json
{
  "success": true,
  "data": {
    "transformed_data": [...],
    "original_count": 100,
    "final_count": 85,
    "operations_applied": ["filter", "map", "sort"]
  },
  "metadata": {
    "transformation_time": 234
  }
}
```

### TransformTemplate - 템플릿 처리

```yaml
template_processor:
  type: "TransformTemplate"
  config:
    engine: "jinja2"             # jinja2, mustache, handlebars
  inputs:
    template: "Hello, {{name}}! Today is {{date}}."
    variables:
      name: "@user_data.name"
      date: "${system.current_date}"
```

**출력 스키마:**
```json
{
  "success": true,
  "data": {
    "rendered": "Hello, Alice! Today is 2025-01-01.",
    "template": "Hello, {{name}}! Today is {{date}}.",
    "variables_used": ["name", "date"]
  }
}
```

### TransformAggregate - 집계 연산

```yaml
aggregator:
  type: "TransformAggregate"
  inputs:
    data: "@data_source.rows"
    group_by: "department"
    operations:
      - type: "count"
        alias: "employee_count"
      - type: "avg"
        field: "salary"
        alias: "avg_salary"
      - type: "sum"
        field: "budget"
        alias: "total_budget"
```

## 🤖 AI 노드 (AI Nodes)

### AITextGeneration - 텍스트 생성

```yaml
text_generator:
  type: "AITextGeneration"
  config:
    provider: "openai"           # openai, anthropic, local
    model: "gpt-4"
    api_key: "${OPENAI_API_KEY}"
  inputs:
    prompt: "다음 데이터를 요약해 주세요: @data_processor.result"
    max_tokens: 500
    temperature: 0.7
    system_prompt: "당신은 데이터 분석 전문가입니다."
```

**출력 스키마:**
```json
{
  "success": true,
  "data": {
    "generated_text": "데이터 분석 결과...",
    "prompt": "다음 데이터를 요약해 주세요...",
    "model": "gpt-4",
    "tokens_used": 234,
    "finish_reason": "stop"
  },
  "metadata": {
    "provider": "openai",
    "generation_time": 2340,
    "cost_estimate": 0.0047
  }
}
```

### AIImageGeneration - 이미지 생성

```yaml
image_generator:
  type: "AIImageGeneration"
  config:
    provider: "openai"
    model: "dall-e-3"
    api_key: "${OPENAI_API_KEY}"
  inputs:
    prompt: "A beautiful sunset over mountains, digital art style"
    size: "1024x1024"           # 1024x1024, 1024x1792, 1792x1024
    quality: "standard"         # standard, hd
    style: "vivid"              # vivid, natural
    n: 1
```

**출력 스키마:**
```json
{
  "success": true,
  "data": {
    "images": [
      {
        "url": "https://...",
        "revised_prompt": "A beautiful sunset...",
        "size": "1024x1024"
      }
    ],
    "prompt": "A beautiful sunset over mountains...",
    "model": "dall-e-3"
  },
  "metadata": {
    "generation_time": 15000,
    "cost_estimate": 0.04
  }
}
```

### AIAnalyzeText - 텍스트 분석

```yaml
text_analyzer:
  type: "AIAnalyzeText"
  config:
    provider: "openai"
    model: "gpt-4"
  inputs:
    text: "@content_loader.text"
    analysis_types: ["sentiment", "intent", "entities", "summary"]
    language: "auto"
```

**출력 스키마:**
```json
{
  "success": true,
  "data": {
    "sentiment": {
      "label": "positive",
      "confidence": 0.87
    },
    "intent": {
      "label": "inquiry",
      "confidence": 0.92
    },
    "entities": [
      {"text": "Seoul", "type": "location", "confidence": 0.95}
    ],
    "summary": "고객이 서울 지역 서비스에 대해 문의함"
  }
}
```

## 📡 통신 노드 (Communication Nodes)

### CommAPI - API 호출

```yaml
api_caller:
  type: "CommAPI"
  config:
    base_url: "${API_BASE_URL}"
    timeout: 30000
    retry_attempts: 3
  inputs:
    endpoint: "/users"
    method: "POST"               # GET, POST, PUT, DELETE, PATCH
    headers:
      Authorization: "Bearer ${API_TOKEN}"
      Content-Type: "application/json"
    query_params:
      limit: 100
      offset: 0
    body: "@data_processor.result"
```

**출력 스키마:**
```json
{
  "success": true,
  "data": {
    "response": {...},
    "status_code": 200,
    "headers": {...},
    "url": "https://api.example.com/users"
  },
  "metadata": {
    "method": "POST",
    "response_time": 1234,
    "content_length": 1024
  }
}
```

### CommEmail - 이메일 발송

```yaml
email_sender:
  type: "CommEmail"
  config:
    smtp_server: "${SMTP_SERVER}"
    smtp_port: 587
    username: "${SMTP_USERNAME}"
    password: "${SMTP_PASSWORD}"
    use_tls: true
  inputs:
    to: ["user@example.com", "admin@company.com"]
    cc: ["manager@company.com"]
    bcc: []
    subject: "처리 완료 알림"
    body: "@report_generator.html_content"
    body_type: "html"           # text, html
    attachments: ["@file_generator.file_path"]
```

### CommSlack - 슬랙 메시지

```yaml
slack_notifier:
  type: "CommSlack"
  config:
    webhook_url: "${SLACK_WEBHOOK_URL}"
    bot_token: "${SLACK_BOT_TOKEN}"
  inputs:
    channel: "#general"
    message: "워크플로우 실행 완료: @workflow_summary.result"
    username: "Gil Bot"
    icon_emoji: ":robot_face:"
    blocks: []                  # 슬랙 블록 형식 메시지
```

## 🎛 제어 노드 (Control Nodes)

### ControlCondition - 조건부 실행

```yaml
conditional_processor:
  type: "ControlCondition"
  inputs:
    condition: "@validator.is_valid == true"
    if_true:
      type: "ProcessSuccess"
      inputs:
        data: "@validator.data"
    if_false:
      type: "ProcessError"
      inputs:
        error: "@validator.error"
```

### ControlLoop - 반복 처리

```yaml
loop_processor:
  type: "ControlLoop"
  config:
    max_iterations: 1000
    parallel: true
    batch_size: 10
  inputs:
    items: "@data_loader.items"
    operation:
      type: "ProcessItem"
      inputs:
        item: "${current_item}"
        index: "${current_index}"
```

### ControlMerge - 데이터 병합

```yaml
data_merger:
  type: "ControlMerge"
  inputs:
    sources:
      - "@source1.data"
      - "@source2.data"
      - "@source3.data"
    merge_strategy: "union"      # union, intersection, concat
    conflict_resolution: "first" # first, last, merge
```

## 🔧 유틸리티 노드 (Utility Nodes)

### UtilValidate - 데이터 검증

```yaml
validator:
  type: "UtilValidate"
  inputs:
    data: "@input_processor.result"
    schema:
      type: "object"
      required: ["name", "email"]
      properties:
        name:
          type: "string"
          minLength: 2
        email:
          type: "string"
          format: "email"
    strict: true
```

### UtilCache - 캐시 관리

```yaml
cache_manager:
  type: "UtilCache"
  config:
    provider: "redis"            # memory, redis, file
    connection: "${REDIS_URL}"
    ttl: 3600                   # 기본 TTL (초)
  inputs:
    operation: "get"            # get, set, delete, clear
    key: "user_data_${user_id}"
    value: "@data_processor.result"
    ttl: 7200                   # 특정 TTL
```

### UtilLog - 로깅

```yaml
logger:
  type: "UtilLog"
  config:
    level: "INFO"               # DEBUG, INFO, WARN, ERROR
    format: "json"              # json, text
    output: "file"              # console, file, both
    file_path: "./logs/workflow.log"
  inputs:
    message: "처리 완료: @processor.summary"
    metadata:
      user_id: "@context.user_id"
      execution_time: "@processor.execution_time"
    level: "INFO"
```

## 🔐 보안 및 인증 노드

### SecurityAuth - 인증 처리

```yaml
authenticator:
  type: "SecurityAuth"
  config:
    provider: "oauth2"          # oauth2, jwt, basic, api_key
    client_id: "${OAUTH_CLIENT_ID}"
    client_secret: "${OAUTH_CLIENT_SECRET}"
  inputs:
    credentials: "@user_input.credentials"
    scope: ["read", "write"]
```

### SecurityEncrypt - 암호화/복호화

```yaml
encryptor:
  type: "SecurityEncrypt"
  config:
    algorithm: "AES-256-GCM"
    key: "${ENCRYPTION_KEY}"
  inputs:
    operation: "encrypt"        # encrypt, decrypt
    data: "@sensitive_data.content"
    metadata: "@data.metadata"
```

## 🧪 테스트 노드 (Test Nodes)

### TestMock - 모의 데이터 생성

```yaml
mock_generator:
  type: "TestMock"
  inputs:
    schema:
      type: "array"
      items:
        type: "object"
        properties:
          name: {type: "string", faker: "name.fullName"}
          email: {type: "string", faker: "internet.email"}
          age: {type: "integer", min: 18, max: 80}
    count: 100
```

### TestAssert - 검증 및 단언

```yaml
assertion:
  type: "TestAssert"
  inputs:
    data: "@processor.result"
    assertions:
      - expression: "data.length > 0"
        message: "결과 데이터가 비어있음"
      - expression: "data.every(item => item.age >= 18)"
        message: "미성년자 데이터 포함"
      - expression: "metadata.execution_time < 5000"
        message: "실행 시간 초과"
```

## 📏 성능 측정 노드

### MetricCollector - 지표 수집

```yaml
metrics:
  type: "MetricCollector"
  inputs:
    metrics:
      - name: "processing_time"
        source: "@processor.metadata.execution_time"
        type: "histogram"
      - name: "record_count"
        source: "@processor.data.count"
        type: "counter"
    tags:
      workflow: "${workflow.name}"
      environment: "${environment.ENV}"
```

## 🔄 데이터 스트리밍 노드

### StreamProcessor - 스트림 처리

```yaml
stream_processor:
  type: "StreamProcessor"
  config:
    buffer_size: 1000
    batch_timeout: 5000
  inputs:
    stream: "@data_stream.output"
    processor:
      type: "TransformData"
      operations:
        - type: "filter"
          condition: "value > 0"
```

---

## 📋 노드 개발 가이드라인

### 새로운 노드 타입 개발 시 준수사항

1. **표준 인터페이스 구현**
   - config, inputs, outputs 표준 구조
   - 표준 에러 처리 및 상태 반환
   - 타임아웃 및 재시도 메커니즘

2. **자기완결성 보장**
   - 외부 의존성 최소화
   - 명확한 입력 검증
   - 예측가능한 출력 형식

3. **문서화 요구사항**
   - 입출력 스키마 정의
   - 사용 예제 제공
   - 에러 케이스 문서화

4. **테스트 요구사항**
   - 단위 테스트 작성
   - 통합 테스트 제공
   - 성능 벤치마크

이 표준을 준수하여 개발된 노드는 모든 Gil-Flow 구현체(gil-py, gil-sharp, gil-node)에서 동일하게 동작합니다.
