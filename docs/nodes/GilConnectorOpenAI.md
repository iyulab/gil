# GilConnectorOpenAI 노드 사용법

OpenAI API와의 연결을 담당하는 커넥터 노드입니다. 다른 AI 노드들이 OpenAI 서비스를 사용할 수 있도록 인증된 클라이언트를 제공합니다.

## 📋 기본 정보

- **노드 타입**: `GilConnectorOpenAI`
- **카테고리**: Connector (연결)
- **목적**: OpenAI API 인증 및 클라이언트 제공

## 🔧 설정 (Config)

### 필수 설정
- **api_key** (string): OpenAI API 키

### 선택 설정
- **organization** (string): OpenAI 조직 ID (선택사항)
- **base_url** (string): 커스텀 베이스 URL (선택사항)

## 📥 입력 (Inputs)

### request_data (선택)
- **타입**: JSON
- **설명**: 직접 API 요청 데이터 (일반적으로 다른 노드에서 사용)
- **구조**:
```yaml
request_data:
  endpoint: "chat.completions.create"  # 또는 "images.generate"
  params:
    model: "gpt-4"
    messages: [...]
```

## 📤 출력 (Outputs)

### response
- **타입**: JSON
- **설명**: API 응답 데이터 또는 연결 상태

### error
- **타입**: TEXT
- **설명**: 에러 메시지 (에러 발생 시)

## 📝 사용 예시

### 1. 기본 커넥터 설정
```yaml
nodes:
  openai_connector:
    type: "GilConnectorOpenAI"
    config:
      api_key: "${OPENAI_API_KEY}"
```

### 2. 조직 ID 포함 설정
```yaml
nodes:
  openai_connector:
    type: "GilConnectorOpenAI"
    config:
      api_key: "${OPENAI_API_KEY}"
      organization: "org-xxxxxxxxx"
```

### 3. 커스텀 URL 사용
```yaml
nodes:
  openai_connector:
    type: "GilConnectorOpenAI"
    config:
      api_key: "${OPENAI_API_KEY}"
      base_url: "https://custom-openai-proxy.com/v1"
```

### 4. 다른 노드와 연결
```yaml
nodes:
  # 커넥터 설정
  openai_connector:
    type: "GilConnectorOpenAI"
    config:
      api_key: "${OPENAI_API_KEY}"
  
  # 텍스트 생성 노드에서 커넥터 사용
  text_generator:
    type: "GilGenText"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "Hello, how are you?"
      model: "gpt-4"
  
  # 이미지 생성 노드에서 커넥터 사용
  image_generator:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "A beautiful landscape"
      size: "1024x1024"

flow:
  - openai_connector
  - [text_generator, image_generator]  # 병렬 실행
```

## 🔄 컨텍스트 활용

### Flow Context 변수 사용
```yaml
nodes:
  openai_connector:
    type: "GilConnectorOpenAI"
    config:
      api_key: "${flow_context.variables.openai_key}"
      organization: "${flow_context.variables.org_id}"
```

### 에러 처리
```yaml
nodes:
  openai_connector:
    type: "GilConnectorOpenAI"
    config:
      api_key: "${OPENAI_API_KEY}"
    error_handling:
      on_error:
        - action: "log_to_flow_context"
        - action: "retry"
          max_attempts: 3
```

## ⚠️ 주의사항

### 보안
- API 키는 절대 하드코딩하지 마세요
- 환경변수나 secure vault 사용 권장
```yaml
# ❌ 잘못된 예시
config:
  api_key: "sk-proj-abc123..."

# ✅ 올바른 예시  
config:
  api_key: "${OPENAI_API_KEY}"
```

### 연결 관리
- 커넥터는 워크플로우 시작 시 한 번만 초기화
- 여러 AI 노드가 동일한 커넥터 재사용 가능
- 연결 실패 시 관련 모든 노드 영향받음

### 비용 관리
```yaml
# 비용 추적을 위한 컨텍스트 활용
nodes:
  cost_tracker:
    type: "CostTracker"
    inputs:
      api_calls: "${flow_context.shared_data.openai_calls}"
      tokens_used: "${flow_context.shared_data.total_tokens}"
```

## 🧪 테스트 및 디버깅

### 연결 테스트
```yaml
nodes:
  openai_connector:
    type: "GilConnectorOpenAI"
    config:
      api_key: "${OPENAI_API_KEY}"
    
  connection_test:
    type: "GilTest"
    inputs:
      test_type: "connection"
      target: "@openai_connector"
    conditions:
      - condition: "@openai_connector.error == null"
        action: "execute"
      - condition: "@openai_connector.error != null"
        action: "stop_workflow"
```

### 디버그 모드
```yaml
environment:
  DEBUG_OPENAI: "true"

nodes:
  openai_connector:
    type: "GilConnectorOpenAI"
    config:
      api_key: "${OPENAI_API_KEY}"
    metadata:
      debug: "${environment.DEBUG_OPENAI}"
      log_requests: true
```

## 📚 관련 노드

이 커넥터와 함께 사용되는 노드들:
- **GilGenText**: 텍스트 생성
- **GilGenImage**: 이미지 생성  
- **GilAnalyzeText**: 텍스트 분석
- **GilTranslate**: 번역

## 🔗 참고 링크

- [OpenAI API 문서](https://platform.openai.com/docs)
- [Gil-Flow 컨텍스트 시스템](CONTEXT_SYSTEM.md)
- [환경 변수 설정 가이드](../README.md#환경-설정)
