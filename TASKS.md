# Gil 프로젝트 개발 계획

## 아키텍처 비전 (v2)

Gil 프로젝트는 모듈성과 확장성을 극대화하기 위해 다음과 같은 세 가지 핵심 구성 요소로 재편됩니다.

1.  **`gil-py` (Core SDK)**: 워크플로우 실행의 핵심 로직을 담은 미니멀한 라이브러리입니다. 모든 노드 패키지와 실행 환경의 기반이 됩니다.
2.  **`gil-node-*` (독립 노드 패키지)**: 특정 기능(AI, 데이터 처리 등)을 수행하는 독립적인 Python 패키지입니다. 각 패키지는 PyPI를 통해 배포 및 설치되며, `gil-py`에 의존합니다. 이를 통해 사용자는 필요한 노드만 선택하여 설치할 수 있습니다.
3.  **`gil-flow-py` (Runner & API Host)**: YAML 워크플로우를 실행하고, API 엔드포인트를 통해 워크플로우를 서비스로 제공하는 실행 환경입니다. `gil-py`와 여러 `gil-node-*` 패키지를 조합하여 사용합니다.

## 개발 로드맵

### Phase 1: `gil-py` 코어 SDK 리팩토링

> **목표**: `gil-py`를 특정 노드 구현에서 분리된 순수 핵심 SDK로 재정의합니다.

- [x] **`gil-py` 구조 재설계**: `nodes`, `connectors`, `generators` 디렉토리 제거
- [x] **노드 동적 로딩 시스템 구현**: `entry_points` 기반 `NodeFactory` 수정
- [x] **코어 노드 설계 및 구현 (`Control-`, `Util-`)**
    - [x] `gil_py/core/control` 및 `gil_py/core/util` 디렉토리 구조 생성
    - [x] `Util-LogMessage`, `Util-SetVariable`, `Control-Branch` 노드 구현 및 등록
- [x] **`pyproject.toml` 업데이트**
    - [x] 의존성을 최소화하고, 코어 노드를 위한 `entry_points` 설정
- [x] **테스트 환경 분리**
    - [x] `test-py`가 `gil-py`와 필요한 노드 패키지를 `pip`으로 설치하여 테스트하도록 구성 변경

### Phase 2: `gil-node-*` 패키지 생태계 구축

> **목표**: 기존 기능을 독립적인 `gil-node-*` 패키지로 분리하고 PyPI 배포를 준비합니다.

- [x] **`gil-node-openai` 패키지 리팩토링 및 이름 변경**
    - [x] `OpenAIConnector` -> `OpenAI-Connector` 이름 변경
    - [x] `ImageGenerator` -> `OpenAI-GenerateImage` 이름 변경
    - [x] `pyproject.toml`의 `entry_points` 수정
    - [x] 파일 이름 및 클래스 이름 변경 (`connector.py` -> `openai_connector.py` 등)
- [x] **`gil-node-text` 패키지 생성**
    - [x] `AITextGeneration` 등 범용 텍스트 처리 노드 이전 (`OpenAI-GenerateText` 등)
- [x] **`gil-node-data` 패키지 생성**
    - [x] `DataFile`, `TransformData` 등 데이터 입출력 및 변환 노드 이전 (`Data-ReadFile`, `Data-Transform` 등)
- [ ] **PyPI 배포 파이프라인 구축**
    - [ ] `gil-py` 및 각 `gil-node-*` 패키지를 위한 CI/CD 설정 (GitHub Actions)

### Phase 3: `gil-flow-py` API 호스트 개발

> **목표**: YAML 워크플로우를 실행하고 관리하는 고성능 API 서버를 구축합니다.

- [x] **FastAPI 기반 서버 구축**
    - [x] `gil-flow-py` 프로젝트 초기 설정 (기존 활용)
    - [x] `pyproject.toml`에 `fastapi`, `uvicorn`, `gil-py` 및 필요한 `gil-node-*` 패키지 의존성 추가
- [x] **API 엔드포인트 구현**
    - [x] `POST /workflows/run`: YAML 워크플로우를 받아 실행하고 결과를 반환
    - [x] `GET /nodes`: 현재 설치된 모든 사용 가능한 노드 목록 반환
    - [x] `GET /health`: 서비스 상태 확인
- [x] **인증 및 보안**
    - [x] API Key를 발급하고 검증하는 미들웨어 또는 의존성 주입 구현
    - [x] 환경 변수를 통해 API Key를 설정하는 기능
- [x] **Docker 컨테이너화**
    - [x] `Dockerfile` 작성
    - [x] 멀티-스테이지 빌드를 사용하여 이미지 최적화
    - [x] `docker-compose.yml`을 통한 로컬 테스트 환경 구성
- [ ] **문서화 및 테스트**
    - [ ] `README.md` 작성 (API 사용법, 배포 방법 명시)
    - [ ] API 엔드포인트에 대한 단위/통합 테스트 작성 (pytest)
        - **현재 상태**: `ModuleNotFoundError: No module named 'gil_py.workflow.yaml_parser'` 오류 및 API 키 관련 테스트 실패 지속. `gil_py` 패키지 구조 및 임포트 문제 해결 중.

## 장기 비전

- [ ] **웹 인터페이스**: 워크플로우를 시각적으로 편집하고 실행 결과를 모니터링하는 웹 UI 개발
- [ ] **커넥터 생태계 확장**: `Anthropic`, `HuggingFace`, `Google AI` 등 주요 AI 서비스 커넥터 노드 추가
- [ ] **성능 최적화**: 비동기/병렬 실행 강화 및 대규모 워크플로우 처리 능력 개선
- [ ] **커뮤니티**: 개발자들이 직접 `gil-node-*` 패키지를 만들고 공유할 수 있는 가이드 및 템플릿 제공