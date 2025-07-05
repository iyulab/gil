# Gil-Flow 노드 타입 표준 (v1.0)

모든 구현체 공통 노드 사양을 정의합니다.

> **참고**: YAML 문법은 [YAML_SPEC](YAML_SPEC), 아키텍처는 [ARCHITECTURE](ARCHITECTURE) 참조

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

#### Data-ReadFile
파일 읽기
```yaml
file_reader:
  type: "Data-ReadFile"
  inputs:
    file_path: "data.txt"
```

**출력**: `content` (파일 내용)

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

#### Data-Transform
데이터 변환
```yaml
transformer:
  type: "Data-Transform"
  config:
    transform_expression: "data * 2" # Python 표현식
  inputs:
    input_data: "@source.result"
```

**출력**: `output_data` (변환된 데이터)

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

#### OpenAI-GenerateText
텍스트 생성
```yaml
text_gen:
  type: "OpenAI-GenerateText"
  config:
    # 이 노드에는 특정 설정이 없습니다.
  inputs:
    client: "@openai_connector.client"
    prompt: "요약해주세요: {{content}}"
    model: "gpt-3.5-turbo"
```

**출력**: `generated_text` (생성된 텍스트)

#### OpenAI-GenerateImage
이미지 생성
```yaml
image_gen:
  type: "OpenAI-GenerateImage"
  config:
    model: "dall-e-3"
    size: "1024x1024"
    quality: "standard"
  inputs:
    client: "@openai_connector.client"
    prompt: "아름다운 풍경"
```

**출력**: `image_url` (이미지 URL)

### 유틸리티 노드

#### Util-SetVariable
워크플로우 컨텍스트에 변수 설정
```yaml
set_my_var:
  type: "Util-SetVariable"
  config:
    variable_name: "my_variable"
  inputs:
    value: "Hello from variable!"
```

**출력**: 없음 (컨텍스트를 수정함)

### 제어 노드

#### Control-Branch
조건부 실행
```yaml
branch_node:
  type: "Control-Branch"
  inputs:
    condition: "@validator.is_valid"
    input: "@data_source.output"
```

**출력**: `true_output` (조건이 참일 때 데이터), `false_output` (조건이 거짓일 때 데이터)

## 커넥터 노드

### OpenAI-Connector
OpenAI API 연결
```yaml
openai_connector:
  type: "OpenAI-Connector"
  config:
    api_key: "${OPENAI_API_KEY}"
    organization: "${OPENAI_ORG}"
    base_url: "https://api.openai.com/v1"
```

**출력**: `client` (초기화된 OpenAI 클라이언트 인스턴스)

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