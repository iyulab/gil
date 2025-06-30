# Gil-Flow (Python Implementation)

Language-neutral YAML-based workflow system with AI integration.

## 설치

```bash
pip install gil-flow
```

## 빠른 시작

### YAML 워크플로우
```yaml
gil_flow_version: "1.0"
name: "이미지 생성"

nodes:
  openai:
    type: "GilConnectorOpenAI"
    config: { api_key: "${OPENAI_API_KEY}" }
  
  generator:
    type: "GilGenImage"
    config: { connector: "@openai" }
    inputs: { prompt: "아름다운 일몰" }

flow: [openai, generator]
outputs: { image: "@generator.image_url" }
```

### 실행
```bash
# CLI 사용
gil-flow run workflow.yaml

# Python에서 직접 사용
python -m gil_py workflow.yaml
```

### Python SDK
```python
from gil_py import GilWorkflow
import asyncio

async def main():
    workflow = GilWorkflow("workflow.yaml")
    result = await workflow.execute()
    print(result)

asyncio.run(main())
```

## 주요 기능

- 🎯 **YAML 기반**: 코드 없이 워크플로우 정의
- 🤖 **AI 통합**: OpenAI DALL-E 3 이미지 생성 지원
- 🔧 **CLI 도구**: 명령줄에서 바로 실행
- 📦 **모듈형**: 노드 기반 확장 가능 아키텍처
- ⚡ **비동기**: 고성능 비동기 실행

## 문서

자세한 문서는 [Gil-Flow 저장소](https://github.com/gil-flow/gil-flow)를 참조하세요.

if __name__ == "__main__":
    asyncio.run(main())
```

## 라이센스

MIT License
