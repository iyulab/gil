# Gil-Flow 노드 타입 표준 (v1.0)

모든 구현체 공통 노드 사양을 정의합니다.

> **참고**: YAML 문법은 [YAML_SPEC.md](YAML_SPEC.md), 아키텍처는 [ARCHITECTURE.md](ARCHITECTURE.md) 참조

## 노드 인터페이스 표준

### 기본 구조
```yaml
node_name:
  type: "NodeType"
  config: {}
  inputs: {}
  timeout: 30000
```

### 출력 형식
```json
{
  "success": true,
  "data": {},
  "metadata": {
    "execution_time": 100,
    "node_type": "NodeType"
  },
  "error": null
}
```

## 표준 노드 타입

### 데이터 노드

#### DataFile
파일 읽기/쓰기
```yaml
file_reader:
  type: "DataFile"
  inputs:
    path: "data.csv"
    format: "csv"     # csv, json, yaml, txt
    encoding: "utf-8"
```

**출력**: `content` (파일 내용), `metadata` (파일 정보)

#### DataDatabase
데이터베이스 연동
```yaml
db_query:
  type: "DataDatabase"
  config:
    connection_string: "${DB_URL}"
  inputs:
    query: "SELECT * FROM users"
```

**출력**: `rows` (쿼리 결과), `count` (행 수)

### 변환 노드

#### TransformData
데이터 변환
```yaml
transformer:
  type: "TransformData"
  inputs:
    data: "@source.content"
    operation: "filter"    # filter, map, aggregate, sort
    params: { "age": "> 18" }
```

**출력**: `result` (변환된 데이터), `count` (처리된 항목 수)

#### TransformTemplate
템플릿 처리
```yaml
template:
  type: "TransformTemplate"
  inputs:
    template: "Hello {{name}}"
    data: { "name": "World" }
    engine: "jinja2"
```

**출력**: `result` (렌더링된 텍스트)

### AI 노드

#### AITextGen
텍스트 생성
```yaml
text_gen:
  type: "AITextGen"
  config:
    connector: "@openai"
    model: "gpt-4"
  inputs:
    prompt: "요약해주세요: {{content}}"
    max_tokens: 500
```

**출력**: `text` (생성된 텍스트), `usage` (토큰 사용량)

#### AIImageGen
이미지 생성
```yaml
image_gen:
  type: "AIImageGen"
  config:
    connector: "@openai"
  inputs:
    prompt: "아름다운 풍경"
    size: "1024x1024"
    quality: "hd"
```

**출력**: `image_url` (이미지 URL), `revised_prompt` (수정된 프롬프트)

### 통신 노드

#### CommAPI
HTTP API 호출
```yaml
api_call:
  type: "CommAPI"
  config:
    base_url: "https://api.example.com"
  inputs:
    method: "POST"
    endpoint: "/data"
    headers: { "Authorization": "Bearer ${TOKEN}" }
    body: { "data": "@source.result" }
```

**출력**: `response` (응답 데이터), `status` (HTTP 상태), `headers` (응답 헤더)

#### CommEmail
이메일 발송
```yaml
email:
  type: "CommEmail"
  config:
    smtp_server: "smtp.gmail.com"
    username: "${EMAIL_USER}"
    password: "${EMAIL_PASS}"
  inputs:
    to: "user@example.com"
    subject: "처리 완료"
    body: "@template.result"
```

**출력**: `sent` (발송 성공 여부), `message_id` (메시지 ID)

### 제어 노드

#### ControlCondition
조건부 실행
```yaml
condition:
  type: "ControlCondition"
  inputs:
    condition: "@validator.is_valid == true"
    if_true: "continue"
    if_false: "stop"
```

**출력**: `result` (조건 결과), `action` (실행할 액션)

#### ControlLoop
반복 실행
```yaml
loop:
  type: "ControlLoop"
  inputs:
    items: "@source.list"
    max_iterations: 100
  nodes:
    - process_item
```

**출력**: `results` (각 반복 결과), `iterations` (실행 횟수)

## 커넥터 노드

### GilConnectorOpenAI
OpenAI API 연결
```yaml
openai:
  type: "GilConnectorOpenAI"
  config:
    api_key: "${OPENAI_API_KEY}"
    organization: "${OPENAI_ORG}"
    timeout: 60000
```

**출력**: `connected` (연결 상태), `model_list` (사용 가능 모델)

### GilConnectorDatabase
데이터베이스 연결
```yaml
db_conn:
  type: "GilConnectorDatabase"
  config:
    type: "postgresql"
    host: "localhost"
    database: "mydb"
    username: "${DB_USER}"
    password: "${DB_PASS}"
```

**출력**: `connected` (연결 상태), `info` (데이터베이스 정보)

## 공통 설정

### 재시도 설정
```yaml
node_name:
  type: "NodeType"
  retry:
    max_attempts: 3
    delay: 1000
    backoff: "exponential"    # linear, exponential
```

### 타임아웃 설정
```yaml
node_name:
  type: "NodeType"
  timeout: 30000              # 밀리초
```

### 조건부 실행
```yaml
node_name:
  type: "NodeType"
  conditions:
    - "@previous.success == true"
    - "@data.count > 0"
```

## 에러 처리

### 에러 형식
```json
{
  "success": false,
  "data": null,
  "error": {
    "code": "INVALID_INPUT",
    "message": "입력 데이터가 유효하지 않습니다",
    "details": { "field": "required" }
  }
}
```

### 표준 에러 코드
- `INVALID_INPUT`: 입력 데이터 오류
- `CONNECTION_ERROR`: 연결 실패
- `TIMEOUT`: 타임아웃 
- `API_ERROR`: API 호출 실패
- `INTERNAL_ERROR`: 내부 오류

---

*각 노드의 상세 구현 가이드는 [docs/nodes/](nodes/) 디렉토리를 참조하세요.*
