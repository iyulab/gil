# OpenAIGenerateText 노드

OpenAI Chat Completions API를 사용하여 텍스트를 생성합니다. 이 노드에는 `GilConnectorOpenAI` 노드에 대한 연결이 필요합니다.

## 설정 (config)

없음.

## 입력 (inputs)

*   `client` (필수, 객체): `GilConnectorOpenAI` 노드에서 가져온 초기화된 OpenAI 클라이언트 인스턴스입니다.
*   `prompt` (필수, 텍스트): 텍스트 생성을 위한 프롬프트입니다.
*   `model` (선택, 텍스트): 사용할 OpenAI 모델입니다 (예: `gpt-4`, `gpt-3.5-turbo`). 기본값은 `gpt-3.5-turbo`입니다.

## 출력 (outputs)

*   `generated_text` (텍스트): OpenAI 모델이 생성한 텍스트입니다.

## 예시

```yaml
text_generator:
  type: "OpenAIGenerateText"
  inputs:
    client: "@openai_connection.client"
    prompt: "Write a short poem about a cat."
    model: "gpt-4o-mini"
```