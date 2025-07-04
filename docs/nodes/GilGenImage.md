# OpenAIGenerateImage 노드

텍스트 프롬프트를 기반으로 OpenAI DALL-E 모델을 사용하여 이미지를 생성합니다. 이 노드에는 `GilConnectorOpenAI` 노드에 대한 연결이 필요합니다.

## 설정 (config)

*   `model` (선택, 텍스트): 사용할 DALL-E 모델입니다. 기본값은 `dall-e-3`입니다.
*   `size` (선택, 텍스트): 생성할 이미지의 크기입니다. 기본값은 `1024x1024`입니다.
*   `quality` (선택, 텍스트): 생성할 이미지의 품질입니다. 기본값은 `standard`입니다.

## 입력 (inputs)

*   `client` (필수, 객체): `GilConnectorOpenAI` 노드에서 가져온 초기화된 OpenAI 클라이언트 인스턴스입니다.
*   `prompt` (필수, 텍스트): 이미지를 생성할 텍스트 프롬프트입니다.

## 출력 (outputs)

*   `image_url` (텍스트): 생성된 이미지의 URL입니다.

## 예시

```yaml
image_generator:
  type: "OpenAIGenerateImage"
  config:
    model: "dall-e-3"
    size: "1024x1024"
  inputs:
    client: "@openai_connection.client"
    prompt: "A futuristic city at sunset, cyberpunk style"
```