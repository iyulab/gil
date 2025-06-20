# AITextGeneration 노드 사용 가이드

## 개요
AITextGeneration 노드는 AI 모델을 활용한 텍스트 생성을 담당하는 노드입니다. GPT, Claude, LLaMA 등 다양한 언어 모델을 지원하며, 창작, 요약, 번역, 코드 생성 등 다용도 텍스트 생성 작업을 수행할 수 있습니다.

## 노드 타입
- **타입**: `AITextGeneration`
- **카테고리**: AI Nodes
- **버전**: 1.0

## 구성 (Configuration)

### 기본 설정
```yaml
config:
  provider: "openai"          # AI 제공업체: openai, anthropic, huggingface, etc.
  model: "gpt-3.5-turbo"     # 사용할 모델명
  max_tokens: 1000           # 최대 토큰 수
  temperature: 0.7           # 창의성 수준 (0.0~1.0)
  top_p: 1.0                # 토큰 선택 다양성
  presence_penalty: 0.0      # 주제 반복 억제
  frequency_penalty: 0.0     # 단어 반복 억제
```

### 고급 설정
```yaml
config:
  system_message: "You are a helpful assistant."  # 시스템 메시지
  stop_sequences: ["\n\n", "END"]                # 생성 중단 시퀀스
  seed: 42                                        # 재현 가능한 결과를 위한 시드
  stream: false                                   # 스트리밍 응답 여부
  retry_count: 3                                  # 실패 시 재시도 횟수
  timeout: 60000                                  # 타임아웃 (ms)
```

### 제공업체별 설정

#### OpenAI
```yaml
config:
  provider: "openai"
  api_key: "${secrets.openai_api_key}"
  model: "gpt-4"
  max_tokens: 2000
  temperature: 0.7
```

#### Anthropic Claude
```yaml
config:
  provider: "anthropic"
  api_key: "${secrets.anthropic_api_key}"
  model: "claude-3-sonnet-20240229"
  max_tokens: 4000
  temperature: 0.5
```

#### Hugging Face
```yaml
config:
  provider: "huggingface"
  api_key: "${secrets.hf_api_key}"
  model: "microsoft/DialoGPT-large"
  max_length: 1024
```

## 입력 (Inputs)

### 필수 입력
- **prompt**: 생성을 위한 프롬프트 텍스트

### 선택 입력
- **context**: 추가 컨텍스트 정보
- **examples**: Few-shot learning을 위한 예시들
- **constraints**: 생성 제약 조건
- **format**: 출력 형식 지정

### 대화형 입력 (채팅 모델)
```yaml
inputs:
  messages:
    - role: "system"
      content: "You are a helpful coding assistant."
    - role: "user"
      content: "Write a Python function to calculate fibonacci numbers"
    - role: "assistant"
      content: "Here's a Python function..."
    - role: "user"
      content: "@user_input.question"
```

## 출력 (Outputs)

### 성공 시
```json
{
  "success": true,
  "data": {
    "generated_text": "생성된 텍스트 내용...",
    "prompt": "원본 프롬프트",
    "model_info": {
      "provider": "openai",
      "model": "gpt-3.5-turbo",
      "version": "0613"
    },
    "usage": {
      "prompt_tokens": 150,
      "completion_tokens": 200,
      "total_tokens": 350
    },
    "finish_reason": "stop",      // stop, length, content_filter
    "alternatives": [...]         // 여러 후보 생성 시
  },
  "metadata": {
    "execution_time": 3500,
    "api_calls": 1,
    "retry_count": 0
  }
}
```

### 실패 시
```json
{
  "success": false,
  "data": null,
  "error": {
    "type": "AIGenerationError",
    "message": "토큰 한도 초과: 요청한 토큰 수가 모델 제한을 초과했습니다",
    "details": {
      "provider": "openai",
      "model": "gpt-3.5-turbo",
      "requested_tokens": 5000,
      "max_tokens": 4096,
      "error_code": "context_length_exceeded"
    }
  }
}
```

## 컨텍스트 (Context)

### Flow Context 사용
```yaml
ai_writer:
  type: "AITextGeneration"
  config:
    provider: "${flow.ai_provider}"
    model: "${flow.ai_model}"
    api_key: "${secrets.ai_api_key}"
  inputs:
    prompt: "${flow.writing_prompt}"
```

### Node Context 활용
```yaml
contextual_generation:
  type: "AITextGeneration"
  inputs:
    prompt: "Summarize the following text: @previous.content"
    context: "@document.metadata"
    constraints:
      max_length: 200
      style: "professional"
```

## 사용 예시

### 1. 기본 텍스트 생성
```yaml
story_generator:
  type: "AITextGeneration"
  config:
    provider: "openai"
    model: "gpt-3.5-turbo"
    max_tokens: 500
    temperature: 0.8
  inputs:
    prompt: "Write a short story about a robot learning to paint"
```

### 2. 코드 생성
```yaml
code_generator:
  type: "AITextGeneration"
  config:
    provider: "openai"
    model: "gpt-4"
    temperature: 0.2
    system_message: "You are an expert programmer. Write clean, documented code."
  inputs:
    prompt: "Create a @language.name function that @requirement.description"
    format: "code_only"
```

### 3. 문서 요약
```yaml
document_summarizer:
  type: "AITextGeneration"
  config:
    model: "gpt-3.5-turbo"
    max_tokens: 300
    temperature: 0.3
  inputs:
    prompt: "Summarize the following document in 3 key points:\n\n@document.content"
    constraints:
      format: "bullet_points"
      language: "korean"
```

### 4. 대화형 챗봇
```yaml
chatbot_response:
  type: "AITextGeneration"
  config:
    model: "gpt-3.5-turbo"
    temperature: 0.7
  inputs:
    messages:
      - role: "system"
        content: "You are a helpful customer service assistant."
      - role: "user"
        content: "@user.message"
    context:
      customer_info: "@customer.profile"
      conversation_history: "@session.history"
```

### 5. 창작 도구
```yaml
creative_writer:
  type: "AITextGeneration"
  config:
    model: "gpt-4"
    temperature: 0.9
    max_tokens: 1000
  inputs:
    prompt: "@user.creative_prompt"
    examples:
      - input: "A mysterious door"
        output: "The door stood there, ancient and weathered..."
    constraints:
      genre: "@user.preferred_genre"
      tone: "@user.preferred_tone"
```

### 6. 번역 작업
```yaml
translator:
  type: "AITextGeneration"
  config:
    model: "gpt-3.5-turbo"
    temperature: 0.1
    system_message: "You are a professional translator."
  inputs:
    prompt: "Translate the following text from @source.language to @target.language:\n\n@source.text"
    constraints:
      preserve_formatting: true
      formal_tone: true
```

### 7. 이메일 작성
```yaml
email_composer:
  type: "AITextGeneration"
  config:
    model: "gpt-3.5-turbo"
    temperature: 0.5
  inputs:
    prompt: "Write a professional email with the following details:"
    context:
      recipient: "@email.recipient"
      subject: "@email.subject"
      key_points: "@email.key_points"
      tone: "professional"
    format: "email_format"
```

### 8. 데이터 분석 인사이트
```yaml
data_insights:
  type: "AITextGeneration"
  config:
    model: "gpt-4"
    temperature: 0.3
  inputs:
    prompt: "Analyze the following data and provide insights:\n@data.summary"
    context:
      data_type: "@data.type"
      business_context: "@analysis.context"
    constraints:
      include_recommendations: true
      format: "structured_report"
```

## 프롬프트 엔지니어링 팁

### 효과적인 프롬프트 구성
```yaml
structured_prompt:
  type: "AITextGeneration"
  inputs:
    prompt: |
      Role: You are an expert technical writer.
      
      Task: Create documentation for the following API endpoint.
      
      Context: 
      - API: @api.name
      - Endpoint: @api.endpoint
      - Method: @api.method
      
      Requirements:
      - Include examples
      - Explain parameters
      - Show response format
      
      Format: Markdown documentation
      
      Endpoint Details:
      @api.details
```

### Few-shot Learning
```yaml
few_shot_example:
  type: "AITextGeneration"
  inputs:
    prompt: "Convert the following requirements to user stories:"
    examples:
      - input: "Users need to login"
        output: "As a user, I want to log in so that I can access my account"
      - input: "Admin can delete users"
        output: "As an admin, I want to delete users so that I can manage the system"
    context:
      requirement: "@project.requirement"
```

## 에러 처리

### API 제한 대응
```yaml
resilient_generation:
  type: "AITextGeneration"
  config:
    retry_count: 5
    retry_delay: 2000
  inputs:
    prompt: "@content.prompt"
  on_error:
    - condition: "error.details.error_code == 'rate_limit_exceeded'"
      action: "retry"
      delay: 10000
    - condition: "error.details.error_code == 'context_length_exceeded'"
      action: "fallback"
      fallback_value: "Content too long for processing"
```

### 비용 관리
```yaml
cost_aware_generation:
  type: "AITextGeneration"
  config:
    max_tokens: 500
    stop_sequences: ["\n\nEND", "---"]
  inputs:
    prompt: "@user.prompt"
    constraints:
      budget_limit: 100  # 토큰 예산
```

## 보안 및 윤리 고려사항

- API 키는 반드시 secrets로 관리
- 사용자 입력 프롬프트 인젝션 공격 방지
- 생성된 콘텐츠의 부적절한 내용 필터링
- 저작권 및 개인정보 보호 준수
- 토큰 사용량 모니터링으로 비용 관리

## 관련 노드

- **AIImageGeneration**: 이미지 생성
- **AIAnalyzeText**: 텍스트 분석
- **AITranslate**: 전용 번역 노드
- **TransformTemplate**: 생성된 텍스트 후처리
- **CommEmail**: 생성된 텍스트로 이메일 발송
