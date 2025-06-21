# gil-py 개발 가이드

Gil-Flow 표준의 Python 구현체 개발 가이드입니다.

> **참고**: 전체 프로젝트는 [README.md](../README.md), Gil-Flow 표준은 [YAML_SPEC.md](YAML_SPEC.md) 참조

## 프로젝트 구조

```
gil-py/
├── gil_py/
│   ├── core/              # 핵심 시스템
│   │   ├── node.py        # 노드 기본 클래스
│   │   ├── port.py        # 포트 시스템
│   │   ├── connection.py  # 노드 간 연결
│   │   └── data_types.py  # 데이터 타입
│   ├── workflow/          # 워크플로우 엔진
│   │   ├── workflow.py    # 워크플로우 클래스
│   │   ├── yaml_parser.py # YAML 파서
│   │   ├── executor.py    # 실행 엔진
│   │   └── node_factory.py# 노드 팩토리
│   ├── connectors/        # 외부 서비스 연동
│   ├── generators/        # AI 생성기
│   └── cli/               # CLI 도구
└── tests/                 # 테스트
```

## 핵심 클래스

### GilNode (기본 노드)
```python
from abc import ABC, abstractmethod
from typing import Dict, Any

class GilNode(ABC):
    def __init__(self, config: Dict[str, Any] = None):
        self.config = config or {}
    
    @abstractmethod
    async def execute(self, inputs: Dict[str, Any]) -> Dict[str, Any]:
        """노드 실행 로직"""
        pass
```

### GilWorkflow (워크플로우)
```python
class GilWorkflow:
    def __init__(self, yaml_path: str):
        self.yaml_path = yaml_path
        self.nodes = {}
        self.flow = []
    
    async def execute(self) -> Dict[str, Any]:
        """워크플로우 실행"""
        pass
```

### YamlParser (YAML 파서)
```python
class YamlParser:
    @staticmethod
    def parse(yaml_content: str) -> Dict[str, Any]:
        """YAML 파싱 및 변수 치환"""
        pass
    
    @staticmethod
    def resolve_references(data: Dict, context: Dict) -> Dict:
        """노드 참조 해석 (@node.output)"""
        pass
```

## 새 노드 추가

### 1. 노드 클래스 구현
```python
# gil_py/generators/text_generator.py
from ..core import GilNode

class GilGenText(GilNode):
    async def execute(self, inputs: Dict[str, Any]) -> Dict[str, Any]:
        prompt = inputs.get('prompt', '')
        # AI 텍스트 생성 로직
        
        return {
            'success': True,
            'data': {
                'text': generated_text,
                'usage': token_usage
            },
            'metadata': {
                'execution_time': execution_time,
                'node_type': 'GilGenText'
            },
            'error': None
        }
```

### 2. 노드 팩토리에 등록
```python
# gil_py/workflow/node_factory.py
from ..generators import GilGenText

class GilNodeFactory:
    _node_registry = {
        'GilGenText': GilGenText,
        # 기타 노드들...
    }
```

### 3. YAML에서 사용
```yaml
nodes:
  text_gen:
    type: "GilGenText"
    config:
      connector: "@openai"
    inputs:
      prompt: "요약해주세요: {{content}}"
```

## 커넥터 개발

### OpenAI 커넥터 예시
```python
# gil_py/connectors/openai_connector.py
import openai
from ..core import GilNode

class GilConnectorOpenAI(GilNode):
    def __init__(self, config: Dict[str, Any]):
        super().__init__(config)
        self.client = openai.OpenAI(
            api_key=config.get('api_key')
        )
    
    async def execute(self, inputs: Dict[str, Any]) -> Dict[str, Any]:
        # 연결 테스트
        try:
            models = self.client.models.list()
            return {
                'success': True,
                'data': {'connected': True, 'models': len(models.data)},
                'error': None
            }
        except Exception as e:
            return {
                'success': False,
                'data': None,
                'error': {'message': str(e)}
            }
```

## CLI 개발

### 새 명령어 추가
```python
# gil_py/cli/main.py
import click

@click.group()
def cli():
    """Gil-Py CLI"""
    pass

@cli.command()
@click.argument('yaml_file')
def run(yaml_file):
    """워크플로우 실행"""
    workflow = GilWorkflow(yaml_file)
    result = asyncio.run(workflow.execute())
    print(result)

@cli.command()
@click.argument('yaml_file')
def validate(yaml_file):
    """YAML 파일 검증"""
    # 검증 로직
    pass
```

## 테스트 작성

### 단위 테스트
```python
# tests/test_nodes.py
import pytest
from gil_py.generators import GilGenText

@pytest.mark.asyncio
async def test_text_generation():
    node = GilGenText({'model': 'gpt-4'})
    inputs = {'prompt': 'Hello'}
    
    result = await node.execute(inputs)
    
    assert result['success'] == True
    assert 'text' in result['data']
```

### 통합 테스트
```python
# tests/test_workflow.py
@pytest.mark.asyncio
async def test_yaml_workflow():
    workflow = GilWorkflow('test_workflow.yaml')
    result = await workflow.execute()
    
    assert result['success'] == True
```

## 개발 환경 설정

### 의존성 설치
```bash
# 개발 환경
pip install -e .[dev]

# 의존성만
pip install -r requirements.txt
```

### 프로젝트 구조 생성
```bash
# 새 노드 타입 디렉토리
mkdir -p gil_py/nodes/new_category

# 테스트 파일
touch tests/test_new_node.py
```

## 패키징 및 배포

### pyproject.toml 설정
```toml
[build-system]
requires = ["setuptools>=61.0"]
build-backend = "setuptools.build_meta"

[project]
name = "gil-py"
version = "0.1.0"
description = "Gil-Flow Python 구현체"
dependencies = [
    "pyyaml>=6.0",
    "aiohttp>=3.8.0",
    "click>=8.0.0"
]
```

### 빌드 및 배포
```bash
# 빌드
python -m build

# 테스트 환경 배포
python -m twine upload --repository testpypi dist/*

# 프로덕션 배포
python -m twine upload dist/*
```

## 성능 최적화

### 비동기 실행
- 모든 노드는 `async def execute()` 구현
- I/O 바운드 작업은 비동기 라이브러리 사용
- 병렬 실행은 `asyncio.gather()` 활용

### 메모리 관리
- 대용량 데이터는 스트림 처리
- 불필요한 객체 참조 제거
- 가비지 컬렉션 최적화

### 캐싱
- API 응답 캐싱
- 컴파일된 템플릿 캐싱
- 데이터베이스 연결 풀링

---

*표준 노드 사양은 [NODE_SPEC.md](NODE_SPEC.md)를 참조하세요.*
