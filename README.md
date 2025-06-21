# Gil - 언어 중립적 노드 기반 워크플로우 표준

**Gil**은 자기완결적인 노드들을 조합하여 복잡한 워크플로우를 구성할 수 있는 언어 중립적 표준입니다.

## 핵심 철학

### 언어 중립성
Gil-Flow YAML로 한 번 정의하면 Python, C#, Node.js 등 다양한 구현체에서 동일하게 실행됩니다.

### 자기완결성  
각 노드는 독립적으로 작동하며, 명확한 입출력 인터페이스를 통해 예측 가능한 결과를 보장합니다.

### 범용성
AI, 데이터 처리, 통신, 파일 처리 등 모든 영역에서 사용 가능하며, 새로운 노드 타입을 쉽게 추가할 수 있습니다.

## 생태계 구조

```
Gil-Flow YAML (언어 중립적 표준)
    ↓
├── gil-py (Python) ✅
├── gil-sharp (C#) 🚧  
└── gil-node (Node.js) 🚧
```

## 빠른 시작

### 설치 (gil-py)
```bash
cd gil-py
pip install -e .
```

### YAML 워크플로우 예시
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
# 워크플로우 실행
python -m gil_py workflow.yaml

# CLI 도구 사용
gil run workflow.yaml
gil validate workflow.yaml
gil list-nodes
```

## 문서 구조

- **[YAML_SPEC.md](docs/YAML_SPEC.md)**: Gil-Flow YAML 문법 표준
- **[NODE_SPEC.md](docs/NODE_SPEC.md)**: 표준 노드 타입과 인터페이스
- **[ARCHITECTURE.md](docs/ARCHITECTURE.md)**: 언어 중립적 아키텍처 가이드  
- **[DEV.md](docs/DEV.md)**: gil-py 구현체 개발 가이드
- **[TASKS.md](TASKS.md)**: 작업 현황 및 계획

## 현재 상태

**Gil-Flow**는 실제 사용 가능한 YAML 기반 AI 워크플로우 시스템으로 완성되었습니다.

- 🎯 **워크플로우 엔진**: YAML 파싱, 실행, 노드 관리 완성
- 🚀 **CLI 도구**: 완전한 명령줄 인터페이스 제공
- 🤖 **AI 통합**: OpenAI DALL-E 3 이미지 생성 지원
- 📚 **문서화**: 간결하고 명확한 구조의 완전한 문서
- ✅ **검증 완료**: 실제 API 테스트 및 사용성 검증

---

*자세한 내용은 [docs/](docs/) 디렉토리를 참조하세요.*
