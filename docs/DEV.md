# gil-py 개발 가이드

Gil-Flow 표준의 Python 구현체 개발 가이드입니다.

> **참고**: 전체 프로젝트는 [README](../README), Gil-Flow 표준은 [YAML_SPEC](YAML_SPEC) 참조

## 프로젝트 구조

```
gil-py/
├── gil/                   # 핵심 gil-py 라이브러리
│   ├── core/              # 핵심 시스템
│   │   ├── node.py        # 노드 기본 클래스
│   │   ├── port.py        # 포트 시스템
│   │   ├── connection.py  # 노드 간 연결
│   │   ├── data_types.py  # 데이터 타입
│   │   └── context.py     # 컨텍스트 시스템
│   ├── workflow/          # 워크플로우 엔진
│   │   ├── workflow.py    # 워크플로우 클래스
│   │   ├── yaml_parser.py # YAML 파서
│   │   ├── executor.py    # 실행 엔진
│   │   └── node_factory.py# 노드 팩토리
│   └── cli/               # CLI 도구
├── nodes/                 # 확장 노드 패키지
│   ├── gil-node-data/
│   ├── gil-node-openai/
│   └── gil-node-text/
├── gil-flow/              # gil-flow-py (API 서버)
└── tests/                 # 테스트
```

## 핵심 클래스

### Node (기본 노드)
```python
from abc import ABC, abstractmethod
from typing import Dict, Any, Optional
from pydantic import BaseModel, Field, ConfigDict

from .port import InputPort, OutputPort
from .context import NodeContext, FlowContext, Context

class Node(BaseModel, ABC):
    node_id: str = Field(default_factory=lambda: str(uuid.uuid4()))
    name: str = Field()
    node_type: str = Field()
    version: str = Field(default="1.0.0")
    node_config: Dict[str, Any] = Field(default_factory=dict)
    
    input_ports: List[InputPort] = Field(default_factory=list)
    output_ports: List[OutputPort] = Field(default_factory=list)
    
    # ... (생략된 다른 필드)

    def __init__(self, node_id: str, name: Optional[str] = None, node_type: Optional[str] = None, version: str = "1.0.0", node_config: Optional[Dict[str, Any]] = None, **data):
        super().__init__(node_id=node_id, name=name or node_id, node_type=node_type or self.__class__.__name__, version=version, node_config=node_config if node_config is not None else {}, **data)
    
    @abstractmethod
    async def execute(self, data: Dict[str, Any], context: Context) -> Dict[str, Any]:
        """노드 실행 로직"""
        pass
```

### GilWorkflow (워크플로우)
```python
from typing import Dict, Any, List, Optional
from pathlib import Path

from ..core.node import Node
from ..core.context import Context
from ..yaml_parser import YamlWorkflowParser, WorkflowConfig
from .executor import WorkflowExecutor
from .node_factory import NodeFactory

class GilWorkflow:
    def __init__(self, name: str = "Gil Workflow"):
        self.name = name
        self.nodes: Dict[str, Node] = {}
        self.connections: List[Dict[str, str]] = []
        self.config: Optional[WorkflowConfig] = None
        self.executor = WorkflowExecutor(node_factory=NodeFactory())
    
    @classmethod
    def from_yaml(cls, yaml_path: str | Path) -> 'GilWorkflow':
        # ... (생략된 구현)
        pass
    
    async def run(self, inputs: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        # ... (생략된 구현)
        pass
```

### YamlWorkflowParser (YAML 파서)
```python
from typing import Dict, Any, Optional
from pathlib import Path
import yaml
from pydantic import BaseModel, Field

# ... (WorkflowConfig 정의)

class YamlWorkflowParser:
    def parse_file(self, yaml_path: str | Path) -> WorkflowConfig:
        # ... (생략된 구현)
        pass
    
    def parse_dict(self, config_dict: Dict[str, Any]) -> WorkflowConfig:
        # ... (생략된 구현)
        pass
    
    def resolve_references(self, value: Any, context_data: Dict[str, Any]) -> Any:
        # ... (생략된 구현)
        pass
```

## 새 노드 추가

Gil-Flow에 새 노드를 추가하는 과정은 다음과 같습니다.

### 1. 노드 클래스 구현
`gil-py/nodes/` 디렉토리 아래에 새 Python 패키지를 생성하고 `Node` 기본 클래스를 상속하는 노드 클래스를 구현합니다. `__init__` 메서드에서 입력 및 출력 포트를 정의하고 `execute` 메서드에 노드의 핵심 로직을 구현합니다.

```python
# gil-py/nodes/your-node-package/your_node_package/your_node.py
from gil_py.core.node import Node
from gil_py.core.port import InputPort, OutputPort
from gil_py.core.data_types import DataType
from gil_py.core.context import Context

class YourNewNode(Node):
    def __init__(self, node_id: str, node_config: dict):
        super().__init__(node_id=node_id, node_config=node_config)
        self.add_input_port(InputPort(name="input_data", data_type=DataType.ANY, required=True))
        self.add_output_port(OutputPort(name="output_data", data_type=DataType.ANY))

    async def execute(self, data: dict, context: Context) -> dict:
        input_data = data.get("input_data")
        # 노드 로직 구현
        processed_data = f"Processed: {input_data}"
        self.get_output_port("output_data").set_data(processed_data)
        return {"output_data": processed_data}
```

### 2. `pyproject.toml`에 진입점 등록
새 노드 패키지의 `pyproject.toml` 파일에 `[project.entry-points."gil.nodes"]` 섹션을 추가하여 노드를 Gil-Flow 시스템에 등록합니다. `NodeFactory`는 이 진입점을 사용하여 노드를 동적으로 검색합니다.

```toml
# gil-py/nodes/your-node-package/pyproject.toml
[project.entry-points."gil.nodes"]
YourNewNode = "your_node_package.your_node:YourNewNode"
```

### 3. YAML에서 사용
노드 패키지를 설치한 후 워크플로우 YAML 파일에서 새 노드 타입을 사용할 수 있습니다.

```yaml
nodes:
  my_custom_node:
    type: "YourNewNode"
    inputs:
      input_data: "Hello Gil-Flow!"
```

## CLI 개발

Gil-Flow CLI는 `argparse`를 사용하여 명령줄 인수를 처리합니다. `gil-py/gil/cli/main.py` 파일을 수정하여 새 명령어나 기능을 추가할 수 있습니다.

```python
# gil-py/gil/cli/main.py
import argparse
import asyncio
import sys
from pathlib import Path
from dotenv import load_dotenv
load_dotenv()

from ..workflow.workflow import GilWorkflow
from ..workflow.node_factory import NodeFactory

def main():
    parser = argparse.ArgumentParser(
        description="Gil - AI Workflow Engine",
        # ... (생략된 다른 인수 및 서브파서 정의)
    )
    args = parser.parse_args()
    
    if args.command == "run":
        asyncio.run(handle_run(args))
    # ... (다른 명령어 처리)

async def handle_run(args):
    # ... (실행 로직)
    pass
```

## 테스트 작성

`pytest`를 사용하여 노드 및 워크플로우에 대한 단위 및 통합 테스트를 작성할 수 있습니다. 테스트 파일은 `gil-py/tests/` 디렉토리에 위치해야 합니다.

### 단위 테스트 (노드)
노드 클래스를 직접 인스턴스화하고 `execute` 메서드를 호출하여 예상대로 작동하는지 확인합니다. `unittest.mock.MagicMock`을 사용하여 외부 종속성을 모의할 수 있습니다.

```python
# gil-py/tests/test_your_node.py
import pytest
from unittest.mock import MagicMock
from gil_py.core.context import Context
from your_node_package.your_node import YourNewNode # 실제 노드 가져오기

@pytest.mark.asyncio
async def test_your_new_node_basic():
    node = YourNewNode(node_id="test_node", node_config={})
    context = Context({})
    input_data = {"input_data": "test_value"}

    await node.execute(input_data, context)

    assert node.get_output_port("output_data").get_data() == "Processed: test_value"
```

### 통합 테스트 (워크플로우)
YAML 워크플로우 파일을 로드하고 `GilWorkflow` 클래스를 사용하여 전체 워크플로우를 실행합니다.

```python
# gil-py/tests/test_workflow_example.py
import pytest
from pathlib import Path
from gil_py.workflow.workflow import GilWorkflow

@pytest.mark.asyncio
async def test_example_workflow():
    workflow_path = Path("path/to/your/workflow.yaml") # 실제 워크플로우 파일 경로
    workflow = GilWorkflow.from_yaml(workflow_path)
    result = await workflow.run()

    assert "some_expected_output" in result
    assert result["some_expected_output"] == "expected_value"
```

## 개발 환경 설정

### 의존성 설치
`gil-py` 프로젝트의 루트 디렉토리에서 개발 종속성을 포함하여 프로젝트를 편집 가능한 모드로 설치합니다.

```bash
py -m pip install -e .[dev]
```

### 프로젝트 구조 생성
새 노드 패키지를 추가할 때 `gil-py/nodes/` 아래에 새 디렉토리를 생성하고 해당 `pyproject.toml` 파일을 설정합니다.

```bash
mkdir gil-py/nodes/my-new-node
# gil-py/nodes/my-new-node/pyproject.toml 파일 생성 및 편집
```

## 패키징 및 배포

`pyproject.toml` 파일은 프로젝트의 메타데이터와 빌드 설정을 정의합니다. `gil-py`는 `gil-flow`라는 이름으로 PyPI에 게시됩니다.

```toml
# gil-py/pyproject.toml
[project]
name = "gil-flow"
version = "0.1.0"
# ...

[tool.setuptools]
packages = ["gil_py"]
package-dir = {"gil_py" = "gil"}
```

### 빌드 및 배포

```bash
# 빌드
py -m build gil-py

# PyPI에 배포 (API 토큰 필요)
py -m twine upload dist/*
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