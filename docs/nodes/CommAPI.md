# CommAPI 노드 사용 가이드

## 개요
CommAPI 노드는 REST API 호출을 담당하는 통신 노드입니다. HTTP/HTTPS 프로토콜을 통해 외부 서비스와 상호작용하며, 다양한 HTTP 메서드와 인증 방식을 지원합니다.

## 노드 타입
- **타입**: `CommAPI`
- **카테고리**: Communication Nodes
- **버전**: 1.0

## 구성 (Configuration)

### 기본 설정
```yaml
config:
  method: "GET"              # HTTP 메서드
  timeout: 30000            # 요청 타임아웃 (ms)
  retry_count: 3            # 재시도 횟수
  retry_delay: 1000         # 재시도 간격 (ms)
  follow_redirects: true    # 리다이렉트 자동 따라가기
  verify_ssl: true          # SSL 인증서 검증
```

### 인증 설정
```yaml
config:
  auth:
    type: "bearer"          # 인증 타입: bearer, basic, api_key
    token: "${secrets.api_token}"
    # 또는
    # username: "${secrets.username}"
    # password: "${secrets.password}"
    # 또는
    # api_key: "${secrets.api_key}"
    # api_key_header: "X-API-Key"
```

### 설정 상세
- **method**: HTTP 메서드 (GET, POST, PUT, DELETE, PATCH 등)
- **timeout**: 요청 타임아웃 (밀리초)
- **retry_count**: 실패 시 재시도 횟수
- **retry_delay**: 재시도 간격 (밀리초)
- **follow_redirects**: 3xx 응답 시 자동 리다이렉트
- **verify_ssl**: SSL/TLS 인증서 검증 여부

## 입력 (Inputs)

### 필수 입력
- **url**: 요청할 API 엔드포인트 URL

### 선택 입력
- **headers**: 추가 HTTP 헤더
- **params**: URL 쿼리 파라미터
- **data**: 요청 본문 데이터 (POST, PUT 등)
- **json**: JSON 형태의 요청 본문
- **files**: 파일 업로드 (multipart/form-data)

## 출력 (Outputs)

### 성공 시
```json
{
  "success": true,
  "data": {
    "response": {
      "status_code": 200,
      "headers": {
        "content-type": "application/json",
        "content-length": "1024"
      },
      "body": {...},
      "text": "response text",
      "json": {...}
    },
    "request": {
      "url": "https://api.example.com/users",
      "method": "GET",
      "headers": {...}
    },
    "timing": {
      "total_time": 1250,
      "connect_time": 100,
      "response_time": 1150
    }
  },
  "metadata": {
    "execution_time": 1300,
    "retry_count": 0
  }
}
```

### 실패 시
```json
{
  "success": false,
  "data": {
    "response": {
      "status_code": 404,
      "headers": {...},
      "body": "Not Found"
    }
  },
  "error": {
    "type": "HTTPError",
    "message": "404 Client Error: Not Found",
    "details": {
      "status_code": 404,
      "url": "https://api.example.com/users/999"
    }
  }
}
```

## 컨텍스트 (Context)

### Flow Context 사용
```yaml
api_call:
  type: "CommAPI"
  config:
    method: "GET"
    auth:
      type: "bearer"
      token: "${flow.api_token}"
  inputs:
    url: "${flow.base_url}/users"
```

### Node Context 활용
```yaml
user_api:
  type: "CommAPI"
  config:
    method: "GET"
  inputs:
    url: "https://api.example.com/users/@previous.user_id"
    headers:
      Authorization: "Bearer @auth.token"
```

## 사용 예시

### 1. 간단한 GET 요청
```yaml
get_users:
  type: "CommAPI"
  config:
    method: "GET"
  inputs:
    url: "https://jsonplaceholder.typicode.com/users"
```

### 2. POST 요청으로 데이터 생성
```yaml
create_user:
  type: "CommAPI"
  config:
    method: "POST"
    auth:
      type: "bearer"
      token: "${secrets.api_token}"
  inputs:
    url: "https://api.example.com/users"
    json:
      name: "@form.name"
      email: "@form.email"
      role: "user"
```

### 3. 쿼리 파라미터가 있는 요청
```yaml
search_users:
  type: "CommAPI"
  config:
    method: "GET"
  inputs:
    url: "https://api.example.com/users"
    params:
      page: 1
      limit: 10
      search: "@input.search_term"
```

### 4. 파일 업로드
```yaml
upload_file:
  type: "CommAPI"
  config:
    method: "POST"
    timeout: 60000
  inputs:
    url: "https://api.example.com/upload"
    files:
      file: "@file_data.content"
    data:
      description: "Uploaded via Gil-Flow"
```

### 5. 커스텀 헤더가 있는 요청
```yaml
api_with_headers:
  type: "CommAPI"
  config:
    method: "GET"
  inputs:
    url: "https://api.example.com/data"
    headers:
      X-Custom-Header: "custom-value"
      Accept: "application/json"
      User-Agent: "Gil-Flow/1.0"
```

### 6. 재시도가 포함된 견고한 API 호출
```yaml
robust_api_call:
  type: "CommAPI"
  config:
    method: "GET"
    retry_count: 5
    retry_delay: 2000
    timeout: 15000
  inputs:
    url: "https://unreliable-api.example.com/data"
```

## 인증 방식

### Bearer Token
```yaml
config:
  auth:
    type: "bearer"
    token: "${secrets.access_token}"
```

### Basic Authentication
```yaml
config:
  auth:
    type: "basic"
    username: "${secrets.api_username}"
    password: "${secrets.api_password}"
```

### API Key (헤더)
```yaml
config:
  auth:
    type: "api_key"
    api_key: "${secrets.api_key}"
    api_key_header: "X-API-Key"
```

### API Key (쿼리 파라미터)
```yaml
config:
  auth:
    type: "api_key"
    api_key: "${secrets.api_key}"
    api_key_param: "api_key"
```

## 에러 처리

### HTTP 상태 코드별 처리
```yaml
api_call:
  type: "CommAPI"
  inputs:
    url: "https://api.example.com/data"
  on_error:
    - condition: "error.details.status_code == 404"
      action: "continue"
      default_value: null
    - condition: "error.details.status_code >= 500"
      action: "retry"
      max_retries: 3
```

### 네트워크 에러 처리
```yaml
api_call:
  type: "CommAPI"
  config:
    timeout: 10000
    retry_count: 3
  inputs:
    url: "https://api.example.com/data"
  on_error:
    action: "fail"
    message: "API 호출 실패: ${error.message}"
```

## 응답 데이터 처리

### JSON 응답 처리
```yaml
process_response:
  type: "TransformData"
  inputs:
    data: "@api_call.response.json"
    operations:
      - type: "extract"
        fields: ["id", "name", "status"]
```

### 응답 상태 코드 확인
```yaml
check_success:
  type: "ControlCondition"
  inputs:
    condition: "@api_call.response.status_code < 400"
    then_data: "@api_call.response.json"
    else_data: null
```

## 보안 고려사항

- API 키와 토큰은 반드시 secrets나 환경변수로 관리
- SSL/TLS 인증서 검증 비활성화 시 보안 위험 고려
- 민감한 데이터가 포함된 요청/응답 로깅 주의
- CORS 정책 준수 (브라우저 환경에서)

## 성능 고려사항

- 타임아웃 설정으로 무한 대기 방지
- 재시도 정책으로 일시적 네트워크 오류 대응
- 대용량 응답 처리 시 메모리 사용량 고려
- 병렬 API 호출 시 rate limiting 주의

## 관련 노드

- **CommEmail**: 이메일 발송
- **CommSlack**: 슬랙 메시지 발송
- **CommWebhook**: 웹훅 호출
- **TransformData**: API 응답 데이터 변환
- **ControlCondition**: 응답 상태별 조건부 처리
