# OpenAI-Connector 노드

OpenAI API와의 연결 및 인증을 관리합니다. OpenAI 클라이언트를 초기화하고 출력 포트를 통해 제공합니다.

## 설정 (config)

*   `api_key` (선택, 텍스트): OpenAI API 키입니다. 환경 변수 참조 (`${ENV_VAR_NAME}`)를 지원합니다. 제공되지 않으면 `OPENAI_API_KEY` 환경 변수를 사용합니다.
*   `organization` (선택, 텍스트): OpenAI 조직 ID입니다.
*   `base_url` (선택, 텍스트): OpenAI API의 기본 URL입니다. 기본값은 `https://api.openai.com/v1`입니다.

## 입력 (inputs)

없음.

## 출력 (outputs)

*   `client` (객체): 초기화된 OpenAI 클라이언트 인스턴스입니다. 다른 OpenAI 관련 노드에 연결하는 데 사용됩니다.

## 예시

```yaml
openai_connection:
  type: "OpenAI-Connector"
  config:
    api_key: "${OPENAI_API_KEY}"
    organization: "org-your_org_id"
```