# Gil-Py

플로우차트 기반 워크플로우 노드 시스템

## 설치

```bash
pip install gil-py
```

## 빠른 시작

```python
from gil_py import GilGenImage, GilConnectorOpenAI
import asyncio

async def main():
    # OpenAI 커넥터 생성
    openai_connector = GilConnectorOpenAI(api_key="your-api-key")
    
    # 이미지 생성 노드 생성
    image_gen = GilGenImage(connector=openai_connector)
    
    # 이미지 생성
    result = await image_gen.generate(
        prompt="A beautiful sunset over mountains",
        size="1024x1024"
    )
    
    print(f"Generated image URL: {result['url']}")

if __name__ == "__main__":
    asyncio.run(main())
```

## 라이센스

MIT License
