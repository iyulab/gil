# GilGenImage 노드 사용법

OpenAI DALL-E를 사용하여 이미지를 생성하는 노드입니다.

## 📋 기본 정보

- **노드 타입**: `GilGenImage`
- **카테고리**: AI Generation (AI 생성)
- **목적**: 텍스트 프롬프트로부터 이미지 생성

## 🔧 설정 (Config)

### 필수 설정
- **connector**: OpenAI 커넥터 노드 참조 (`@connector_node_name`)

### 선택 설정
- **model** (string): 사용할 모델 (기본값: "dall-e-3")
- **timeout** (number): 요청 타임아웃 (밀리초)

## 📥 입력 (Inputs)

### 필수 입력
- **prompt** (string): 이미지 생성을 위한 텍스트 설명

### 선택 입력
- **size** (string): 이미지 크기
  - DALL-E 3: "1024x1024" (기본값), "1792x1024", "1024x1792"
  - DALL-E 2: "1024x1024", "512x512", "256x256"
- **quality** (string): 이미지 품질
  - "standard" (기본값): 표준 품질
  - "hd": 고품질 (DALL-E 3만 지원)
- **style** (string): 이미지 스타일 (DALL-E 3만)
  - "vivid" (기본값): 생생하고 극적인 이미지
  - "natural": 자연스럽고 사실적인 이미지
- **n** (number): 생성할 이미지 수 (1-10, 기본값: 1)

## 📤 출력 (Outputs)

### images
- **타입**: Array
- **설명**: 생성된 이미지 정보 배열
- **구조**:
```json
[
  {
    "url": "https://oaidalleapiprodscus.blob.core.windows.net/...",
    "revised_prompt": "실제 사용된 프롬프트 (DALL-E 3)"
  }
]
```

### prompt
- **타입**: TEXT
- **설명**: 사용된 원본 프롬프트

### metadata
- **타입**: JSON
- **설명**: 생성 메타데이터 (모델, 크기, 품질 등)

## 📝 사용 예시

### 1. 기본 이미지 생성
```yaml
nodes:
  openai_connector:
    type: "GilConnectorOpenAI"
    config:
      api_key: "${OPENAI_API_KEY}"
  
  image_generator:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "A beautiful sunset over a mountain lake"

flow:
  - openai_connector
  - image_generator
```

### 2. 고품질 이미지 생성
```yaml
nodes:
  image_generator:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "A futuristic city with flying cars, cyberpunk style"
      size: "1792x1024"
      quality: "hd"
      style: "vivid"
```

### 3. 여러 이미지 생성
```yaml
nodes:
  batch_image_generator:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "Abstract geometric patterns in blue and gold"
      size: "1024x1024"
      n: 4
```

### 4. 동적 프롬프트 사용
```yaml
nodes:
  prompt_builder:
    type: "GilUtilTemplate"
    inputs:
      template: "A {{style}} painting of {{subject}} in {{color}} colors"
      variables:
        style: "${flow_context.variables.art_style}"
        subject: "${flow_context.variables.subject}"
        color: "${flow_context.variables.color_scheme}"
  
  image_generator:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "@prompt_builder.rendered_text"
      style: "natural"

flow:
  - prompt_builder
  - image_generator
```

### 5. 조건부 이미지 생성
```yaml
nodes:
  image_generator:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "@text_analyzer.subject"
      size: |
        {% if flow_context.variables.premium_user %}
          "1792x1024"
        {% else %}
          "1024x1024"
        {% endif %}
      quality: |
        {% if flow_context.variables.premium_user %}
          "hd"
        {% else %}
          "standard"
        {% endif %}
    conditions:
      - condition: "@text_analyzer.has_visual_content == true"
        action: "execute"
```

## 🔄 컨텍스트 활용

### Flow Context로 결과 저장
```yaml
nodes:
  image_generator:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "Logo design for ${flow_context.variables.company_name}"
    outputs:
      generated_images:
        target: "flow_context.shared_data.logo_images"
        operation: "append"
```

### Node Context에서 진행률 추적
```yaml
nodes:
  batch_processor:
    type: "GilControlLoop"
    inputs:
      items: "${flow_context.variables.image_prompts}"
      operation:
        type: "GilGenImage"
        config:
          connector: "@openai_connector"
        inputs:
          prompt: "${item.prompt}"
          size: "${item.size}"
        progress_tracking:
          target: "node_context.internal_state.progress"
```

## 💾 이미지 저장 워크플로우

### 로컬 저장
```yaml
nodes:
  image_generator:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "Digital art landscape"
  
  image_downloader:
    type: "GilCommDownload"
    inputs:
      urls: "@image_generator.images[*].url"
      destination: "./generated_images/"
      filename_pattern: "image_{timestamp}_{index}.png"

flow:
  - openai_connector
  - image_generator
  - image_downloader
```

### 클라우드 업로드
```yaml
nodes:
  image_generator:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "Product showcase image"
  
  cloud_uploader:
    type: "GilCommCloudUpload"
    config:
      provider: "aws_s3"
      bucket: "${flow_context.variables.image_bucket}"
    inputs:
      image_urls: "@image_generator.images[*].url"
      metadata:
        prompt: "@image_generator.prompt"
        generated_at: "${timestamp}"

flow:
  - openai_connector
  - image_generator
  - cloud_uploader
```

## ⚡ 성능 최적화

### 병렬 이미지 생성
```yaml
nodes:
  # 여러 프롬프트를 병렬로 처리
  image_gen_1:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "Portrait of a scientist"
  
  image_gen_2:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "Laboratory equipment"
  
  image_gen_3:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "DNA helix structure"

flow:
  - openai_connector
  - [image_gen_1, image_gen_2, image_gen_3]  # 병렬 실행
```

### 캐시 활용
```yaml
nodes:
  cache_checker:
    type: "GilUtilCache"
    inputs:
      key: "image_${hash(@prompt_builder.rendered_text)}"
      operation: "get"
  
  image_generator:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "@prompt_builder.rendered_text"
    conditions:
      - condition: "@cache_checker.found == false"
        action: "execute"
  
  cache_saver:
    type: "GilUtilCache"
    inputs:
      key: "image_${hash(@prompt_builder.rendered_text)}"
      value: "@image_generator.images"
      operation: "set"
      ttl: 86400  # 24시간
```

## ⚠️ 주의사항

### 비용 관리
```yaml
# 비용 추적
nodes:
  cost_monitor:
    type: "GilUtilCounter"
    inputs:
      counter_name: "dalle_api_calls"
      increment: "@image_generator.images.length"
    conditions:
      - condition: "${flow_context.shared_data.dalle_calls} > 100"
        action: "send_alert"
```

### 콘텐츠 정책
- OpenAI 콘텐츠 정책 준수 필요
- 부적절한 프롬프트는 자동 거부됨
- 에러 처리 및 대체 프롬프트 준비 권장

### 레이트 리미팅
```yaml
nodes:
  rate_limiter:
    type: "GilUtilRateLimit"
    config:
      requests_per_minute: 50
      requests_per_hour: 1000
    inputs:
      operation: "image_generation"
  
  image_generator:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "@prompt_processor.final_prompt"
    conditions:
      - condition: "@rate_limiter.allowed == true"
        action: "execute"
```

## 🧪 테스트 및 디버깅

### 프롬프트 검증
```yaml
nodes:
  prompt_validator:
    type: "GilUtilValidate"
    inputs:
      text: "${flow_context.variables.user_prompt}"
      rules:
        - type: "length"
          min: 10
          max: 1000
        - type: "content_filter"
          blocked_words: ["inappropriate", "content"]
  
  image_generator:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "@prompt_validator.validated_text"
    conditions:
      - condition: "@prompt_validator.valid == true"
        action: "execute"
```

## 📚 관련 노드

- **GilConnectorOpenAI**: OpenAI API 커넥터
- **GilCommDownload**: 이미지 다운로드
- **GilUtilCache**: 결과 캐싱
- **GilAnalyzeImage**: 생성된 이미지 분석

## 🔗 참고 링크

- [DALL-E API 문서](https://platform.openai.com/docs/guides/images)
- [Gil-Flow 컨텍스트 시스템](../CONTEXT_SYSTEM.md)
- [OpenAI 콘텐츠 정책](https://openai.com/policies/usage-policies)
