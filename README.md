# Gil - 언어 중립적 노드 기반 워크플로우 표준

**Gil**은 자기완결적인 노드들을 조합하여 복잡한 워크플로우를 구성할 수 있는 언어 중립적 표준입니다. Gil-Flow YAML 문법으로 한 번 정의하면 Python, C#, Node.js 등 다양한 언어 구현체에서 동일하게 실행됩니다.

## 🎯 핵심 철학

### 언어 중립성
프로그래밍 언어에 독립적인 Gil-Flow YAML 표준을 통해 한 번 작성한 워크플로우를 어떤 구현체에서든 실행할 수 있습니다.

### 자기완결성
각 노드는 외부 의존성 없이 독립적으로 작동하며, 명확한 입출력 인터페이스를 통해 예측 가능한 결과를 보장합니다.

### 범용성
AI, 데이터 처리, 통신, 파일 처리 등 모든 영역에서 사용할 수 있으며, 새로운 노드 타입을 쉽게 추가하고 조합할 수 있습니다.

## 🏗 생태계 구조

```
Gil-Flow YAML (언어 중립적 표준)
    ↓
├── gil-py (Python) - 개발 중 ✅
├── gil-sharp (C#) - 계획 중 🚧  
└── gil-node (Node.js) - 계획 중 🚧
```

### Gil-Flow YAML 예시
```yaml
gil_flow_version: "1.0"
name: "데이터 처리 파이프라인"

nodes:
  reader:
    type: "DataFile"
    inputs: { file_path: "./input.csv" }
  
  processor:
    type: "AITextGeneration" 
    inputs: { prompt: "요약: @reader.content" }
  
  writer:
    type: "DataFile"
    inputs: { 
      file_path: "./output.txt",
      content: "@processor.generated_text" 
    }

flow: [reader, processor, writer]
```

## 🚀 빠른 시작

1. **워크플로우 정의**: Gil-Flow YAML로 노드와 연결 정의
2. **구현체 선택**: 프로젝트에 맞는 언어 구현체 설치
3. **실행**: 표준 CLI 또는 프로그래밍 API로 워크플로우 실행

### Python (gil-py)
```bash
pip install gil-py
gil-py run workflow.yaml
```

## 📚 문서 구조

- **[YAML_SPEC.md](docs/YAML_SPEC.md)**: Gil-Flow YAML 문법 표준
- **[NODE_SPEC.md](docs/NODE_SPEC.md)**: 표준 노드 타입과 인터페이스
- **[CONTEXT_SYSTEM.md](docs/CONTEXT_SYSTEM.md)**: Flow/Node Context 시스템 가이드
- **[노드 사용 가이드](docs/nodes/)**: 각 노드별 상세 사용법과 예시
- **[ARCHITECTURE.md](docs/ARCHITECTURE.md)**: 언어 중립적 아키텍처 가이드
- **[DEV.md](docs/DEV.md)**: gil-py 구현체 개발 가이드

## 🤝 기여하기

Gil은 오픈소스 프로젝트입니다. 새로운 노드 타입 추가, 구현체 개발, 문서 개선 등 모든 기여를 환영합니다.

1. 이슈 등록으로 아이디어 공유
2. Pull Request로 코드 기여
3. 문서 개선 및 예제 추가

## 📄 라이선스

이 프로젝트는 [MIT License](LICENSE)를 따릅니다.
