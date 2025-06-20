# gil-py 개발 가이드

**gil-py**는 [Gil-Flow 표준](YAML_SPEC.md)의 Python 구현체입니다. 이 문서는 gil-py의 내부 구조, 확장 방법, 개발 절차를 다룹니다.

> **참고**: Gil 전체 프로젝트 개요는 [README.md](../README.md)를, Gil-Flow 표준은 [YAML_SPEC.md](YAML_SPEC.md)를 참조하세요.

### 🏗 아키텍처 개요

```
gil-py/
├── gil_py/
│   ├── core/                   # 핵심 시스템
│   │   ├── node.py            # 노드 기본 클래스
│   │   ├── port.py            # 포트 시스템
│   │   ├── connection.py      # 노드 간 연결
│   │   └── data_types.py      # 데이터 타입 정의
│   ├── workflow/              # 워크플로우 엔진
│   │   ├── workflow.py        # 메인 워크플로우 클래스
│   │   ├── yaml_parser.py     # YAML 파서
│   │   ├── executor.py        # 실행 엔진
│   │   └── node_factory.py    # 노드 팩토리
│   ├── nodes/                 # 표준 노드 구현
│   │   ├── data/              # 데이터 노드 (DataFile, DataCSV 등)
│   │   ├── transform/         # 변환 노드 (TransformData 등)
│   │   ├── ai/                # AI 노드 (AITextGen, AIImageGen 등)
│   │   ├── comm/              # 통신 노드 (CommAPI, CommEmail 등)
│   │   └── control/           # 제어 노드 (ControlCondition 등)
│   └── cli/                   # CLI 도구
│       ├── main.py            # CLI 메인
│       └── commands/          # CLI 명령어들
└── tests/                     # 테스트 코드
```
## 🧩 Gil-Py 핵심 컴포넌트

### 1. 노드 시스템 (core/)

#### GilNode (node.py)
모든 노드의 기본 클래스입니다:

```python
from abc import ABC, abstractmethod
from typing import Dict, Any, List
import asyncio

class GilNode(ABC):
    """Gil 노드 기본 클래스"""
    
    def __init__(self, config: Dict[str, Any] = None, name: str = ""):
        self.config = config or {}
        self.name = name
        self.input_ports: List[GilPort] = []
        self.output_ports: List[GilPort] = []
        self._setup_ports()
    
    @abstractmethod
    async def run(self, inputs: Dict[str, Any]) -> Dict[str, Any]:
        """노드 실행 메서드 - 반드시 구현해야 함"""
        pass
    
    @abstractmethod
    def _setup_ports(self) -> None:
        """입출력 포트 설정 - 반드시 구현해야 함"""
        pass
    
    def validate_inputs(self, inputs: Dict[str, Any]) -> bool:
        """입력 데이터 검증"""
        for port in self.input_ports:
            if port.required and port.name not in inputs:
                raise ValueError(f"Required input '{port.name}' is missing")
        return True
    
    def get_schema(self) -> Dict[str, Any]:
        """노드 스키마 반환"""
        return {
            "type": self.__class__.__name__,
            "input_ports": [port.to_dict() for port in self.input_ports],
            "output_ports": [port.to_dict() for port in self.output_ports],
            "config": self.config
        }
```

#### GilPort (port.py)
노드의 입출력 포트를 정의합니다:

```python
from enum import Enum
from typing import Any, Optional
from pydantic import BaseModel

class GilDataType(Enum):
    """Gil 데이터 타입"""
    TEXT = "text"
    NUMBER = "number"
    BOOLEAN = "boolean"
    ARRAY = "array"
    OBJECT = "object"
    FILE = "file"
    IMAGE = "image"
    ANY = "any"

class GilPort(BaseModel):
    """Gil 포트 정의"""
    name: str
    data_type: GilDataType
    required: bool = False
    description: str = ""
    default_value: Optional[Any] = None
    
    def validate_data(self, data: Any) -> bool:
        """데이터 타입 검증"""
        if self.data_type == GilDataType.TEXT:
            return isinstance(data, str)
        elif self.data_type == GilDataType.NUMBER:
            return isinstance(data, (int, float))
        elif self.data_type == GilDataType.BOOLEAN:
            return isinstance(data, bool)
        elif self.data_type == GilDataType.ARRAY:
            return isinstance(data, list)
        elif self.data_type == GilDataType.OBJECT:
            return isinstance(data, dict)
        elif self.data_type == GilDataType.ANY:
            return True
        return False
```

### 2. 워크플로우 엔진 (workflow/)

#### GilWorkflow (workflow.py)
메인 워크플로우 클래스입니다:

```python
import asyncio
from typing import Dict, Any, List, Optional
from pathlib import Path
from .yaml_parser import YamlWorkflowParser
from .executor import WorkflowExecutor
from .node_factory import NodeFactory

class GilWorkflow:
    """Gil 워크플로우 메인 클래스"""
    
    def __init__(self, name: str = "Gil Workflow"):
        self.name = name
        self.nodes: Dict[str, GilNode] = {}
        self.connections: List[Dict[str, str]] = []
        self.config: Optional[Any] = None
        self.executor = WorkflowExecutor()
    
    @classmethod
    def from_yaml(cls, yaml_path: str | Path) -> 'GilWorkflow':
        """YAML 파일에서 워크플로우 생성"""
        # .env 파일 로드
        from dotenv import load_dotenv
        load_dotenv()
        
        parser = YamlWorkflowParser()
        config = parser.parse_file(yaml_path)
        
        workflow = cls(name=config.name)
        workflow.config = config
        workflow._build_from_config(config)
        
        return workflow
    
    async def run(self, inputs: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        """워크플로우 실행"""
        if not self.nodes:
            raise ValueError("워크플로우에 노드가 없습니다")
        
        inputs = inputs or {}
        
        # YAML 설정 기반 입력 처리
        if self.config:
            inputs = self._resolve_yaml_inputs(inputs)
        
        # 워크플로우 실행
        return await self.executor.execute(self.nodes, self.connections, inputs)
    
    def validate(self) -> Dict[str, Any]:
        """워크플로우 유효성 검증"""
        validation_result = {
            "valid": True,
            "errors": [],
            "warnings": []
        }
        
        # 노드 검증
        for node_name, node in self.nodes.items():
            try:
                node.get_schema()  # 스키마 검증
            except Exception as e:
                validation_result["errors"].append(f"노드 '{node_name}': {e}")
                validation_result["valid"] = False
        
        # 연결 검증
        for connection in self.connections:
            source = connection.get("source_node")
            target = connection.get("target_node")
            
            if source not in self.nodes:
                validation_result["errors"].append(f"소스 노드 '{source}'를 찾을 수 없습니다")
                validation_result["valid"] = False
                
            if target not in self.nodes:
                validation_result["errors"].append(f"타겟 노드 '{target}'를 찾을 수 없습니다")
                validation_result["valid"] = False
        
        return validation_result
```

### 3. YAML 파서 (yaml_parser.py)

Gil-Flow YAML 표준을 파싱합니다:

```python
import yaml
import os
from typing import Dict, Any, List, Optional
from pathlib import Path
from pydantic import BaseModel, Field

class NodeConfig(BaseModel):
    """노드 설정"""
    type: str = Field(description="노드 타입")
    config: Dict[str, Any] = Field(default_factory=dict)
    inputs: Dict[str, Any] = Field(default_factory=dict)
    conditions: List[str] = Field(default_factory=list)

class WorkflowConfig(BaseModel):
    """워크플로우 설정"""
    gil_flow_version: str = Field(default="1.0")
    name: str = Field(description="워크플로우 이름")
    description: Optional[str] = None
    environment: Dict[str, Any] = Field(default_factory=dict)
    nodes: Dict[str, NodeConfig] = Field(description="노드 정의")
    flow: List[Any] = Field(description="실행 순서")
    outputs: Dict[str, Any] = Field(default_factory=dict)

class YamlWorkflowParser:
    """YAML 워크플로우 파서"""
    
    def parse_file(self, yaml_path: str | Path) -> WorkflowConfig:
        """YAML 파일 파싱"""
        yaml_path = Path(yaml_path)
        
        if not yaml_path.exists():
            raise FileNotFoundError(f"YAML 파일을 찾을 수 없습니다: {yaml_path}")
        
        with open(yaml_path, 'r', encoding='utf-8') as f:
            yaml_data = yaml.safe_load(f)
        
        return self.parse_dict(yaml_data)
    
    def parse_dict(self, yaml_data: Dict[str, Any]) -> WorkflowConfig:
        """딕셔너리 파싱"""
        # 노드 설정 파싱
        nodes = {}
        for node_name, node_data in yaml_data.get("nodes", {}).items():
            nodes[node_name] = NodeConfig(**node_data)
        
        # 워크플로우 설정 생성
        config_data = {
            "gil_flow_version": yaml_data.get("gil_flow_version", "1.0"),
            "name": yaml_data.get("name", "Unnamed Workflow"),
            "description": yaml_data.get("description"),
            "environment": yaml_data.get("environment", {}),
            "nodes": nodes,
            "flow": yaml_data.get("flow", []),
            "outputs": yaml_data.get("outputs", {})
        }
        
        return WorkflowConfig(**config_data)
    
    def resolve_references(self, value: Any, context: Dict[str, Any]) -> Any:
        """참조 해결 (@node.output, ${env.VAR} 등)"""
        if isinstance(value, str):
            if value.startswith("@"):
                # @node_name.output 형태 참조 해결
                return self._resolve_node_reference(value, context)
            elif value.startswith("${") and value.endswith("}"):
                # ${environment.VAR} 형태 환경변수 해결
                return self._resolve_env_reference(value, context)
        
        return value
    
    def _resolve_env_reference(self, env_var: str, context: Dict[str, Any]) -> str:
        """환경변수 및 입력 변수 해결"""
        # ${} 제거
        var_expr = env_var[2:-1]
        
        # 기본값 처리 (| 구분자)
        default_value = None
        if "|" in var_expr:
            var_expr, default_value = var_expr.split("|", 1)
            var_expr = var_expr.strip()
            default_value = default_value.strip()
        
        # input.* 형태 처리
        if var_expr.startswith("input."):
            input_key = var_expr[6:]  # "input." 제거
            input_data = context.get("input", {})
            if input_key in input_data:
                return str(input_data[input_key])
            elif default_value is not None:
                return default_value
        
        # environment.* 형태 처리
        if var_expr.startswith("environment."):
            env_key = var_expr[12:]  # "environment." 제거
            env_data = context.get("environment", {})
            if env_key in env_data:
                return str(env_data[env_key])
        
        # 시스템 환경변수에서 직접 찾기
        value = os.getenv(var_expr)
        if value is not None:
            return value
        
        # 기본값 반환 또는 원래 문자열
        return default_value if default_value is not None else env_var
```

### 4. 노드 팩토리 (node_factory.py)

동적 노드 생성을 담당합니다:

```python
from typing import Dict, Any, Type, List
from ..core import GilNode

class NodeFactory:
    """노드 팩토리 - 동적 노드 생성"""
    
    def __init__(self):
        self._node_registry: Dict[str, Type[GilNode]] = {}
        self._register_builtin_nodes()
    
    def _register_builtin_nodes(self) -> None:
        """내장 노드 타입 등록"""
        # AI 노드들
        from ..nodes.ai.text_generation import AITextGeneration
        from ..nodes.ai.image_generation import AIImageGeneration
        
        # 데이터 노드들  
        from ..nodes.data.file import DataFile
        from ..nodes.data.csv import DataCSV
        
        # 변환 노드들
        from ..nodes.transform.data import TransformData
        
        # 통신 노드들
        from ..nodes.comm.api import CommAPI
        from ..nodes.comm.email import CommEmail
        
        # 노드 등록
        self.register("AITextGeneration", AITextGeneration)
        self.register("AIImageGeneration", AIImageGeneration)
        self.register("DataFile", DataFile)
        self.register("DataCSV", DataCSV)
        self.register("TransformData", TransformData)
        self.register("CommAPI", CommAPI)
        self.register("CommEmail", CommEmail)
    
    def register(self, node_type: str, node_class: Type[GilNode]) -> None:
        """노드 타입 등록"""
        self._node_registry[node_type] = node_class
    
    def create_node(self, node_type: str, config: Dict[str, Any] = None, 
                   name: str = "") -> GilNode:
        """노드 인스턴스 생성"""
        if node_type not in self._node_registry:
            raise ValueError(f"알 수 없는 노드 타입: {node_type}")
        
        node_class = self._node_registry[node_type]
        return node_class(config=config or {}, name=name)
    
    def get_available_nodes(self) -> List[str]:
        """사용 가능한 노드 타입 목록"""
        return list(self._node_registry.keys())
    
    def get_node_info(self, node_type: str) -> Dict[str, Any]:
        """노드 타입 정보 조회"""
        if node_type not in self._node_registry:
            return {"error": f"알 수 없는 노드 타입: {node_type}"}
        
        node_class = self._node_registry[node_type]
        # 임시 인스턴스 생성해서 스키마 조회
        temp_node = node_class()
        schema = temp_node.get_schema()
        
        return {
            "type": node_type,
            "description": getattr(node_class, "__doc__", "설명 없음"),
            "input_ports": schema.get("input_ports", []),
            "output_ports": schema.get("output_ports", [])
        }
```
- **GilDataPDF**: PDF 처리
- **GilDataImage**: 이미지 파일 처리
- **GilDataAudio**: 오디오 파일 처리

### 6. 제어 노드 (GilControl 시리즈)
- **GilControlCondition**: 조건부 분기
- **GilControlLoop**: 반복 처리
- **GilControlDelay**: 지연 처리
- **GilControlMerge**: 다중 입력 병합
- **GilControlSplit**: 출력 분할
- **GilControlFilter**: 데이터 필터링
- **GilControlSwitch**: 다중 분기
- **GilControlParallel**: 병렬 처리

### 7. 유틸리티 노드 (GilUtil 시리즈)
- **GilUtilLog**: 로깅
- **GilUtilValidate**: 데이터 검증
- **GilUtilTransform**: 데이터 변환
- **GilUtilCache**: 캐시 처리
- **GilUtilSchedule**: 스케줄링
- **GilUtilTrigger**: 트리거 처리
- **GilUtilTemplate**: 템플릿 처리
- **GilUtilHash**: 해시/암호화 처리

### 8. 웹 크롤링 노드 (GilWeb 시리즈)
- **GilWebScraper**: 웹 스크래핑
- **GilWebBrowser**: 브라우저 자동화
- **GilWebRSS**: RSS 피드 처리
- **GilWebSitemap**: 사이트맵 처리

### 9. 스토리지 노드 (GilStorage 시리즈)
- **GilStorageCloud**: 클라우드 스토리지 (AWS S3, Google Cloud 등)
- **GilStorageLocal**: 로컬 스토리지
- **GilStorageFTP**: FTP 서버 연결
- **GilStorageDropbox**: Dropbox 연결

## 노드 구성요소

### 1. 핵심 구성요소

#### 1.1 식별 정보
- **node_id**: 노드 고유 식별자 (UUID)
- **name**: 노드 이름 (사용자 정의 가능)
- **node_type**: 노드 타입 분류
- **version**: 노드 버전 정보

#### 1.2 포트 시스템
- **input_ports**: 입력 포트 컬렉션
  - **port_name**: 포트 식별자
  - **data_type**: 허용 데이터 타입
  - **required**: 필수 입력 여부
  - **default_value**: 기본값
  - **description**: 포트 설명
  - **validation_rules**: 입력 검증 규칙
- **output_ports**: 출력 포트 컬렉션
  - **port_name**: 포트 식별자
  - **data_type**: 출력 데이터 타입
  - **description**: 포트 설명

#### 1.3 연결 관리
- **input_connections**: 입력 연결 목록
- **output_connections**: 출력 연결 목록
- **connection_rules**: 연결 제약 조건

### 2. 데이터 타입 시스템

#### 2.1 기본 데이터 타입
- **text**: 텍스트 문자열
- **number**: 숫자 (정수/실수)
- **boolean**: 불린값
- **json**: JSON 객체
- **array**: 배열
- **binary**: 바이너리 데이터
- **image**: 이미지 데이터
- **audio**: 오디오 데이터
- **video**: 비디오 데이터
- **file**: 파일 객체
- **any**: 모든 타입 허용

#### 2.2 복합 데이터 타입
- **email_message**: 이메일 메시지 구조
- **api_request**: API 요청 구조
- **ai_response**: AI 응답 구조
- **database_record**: 데이터베이스 레코드

### 3. 상태 관리

#### 3.1 실행 상태
- **ready**: 실행 준비 완료
- **running**: 실행 중
- **completed**: 성공적으로 완료
- **error**: 오류 발생
- **paused**: 일시 정지
- **cancelled**: 실행 취소

#### 3.2 상태 정보
- **status**: 현재 실행 상태
- **error_message**: 오류 메시지
- **start_time**: 실행 시작 시간
- **end_time**: 실행 완료 시간
- **execution_duration**: 실행 소요 시간
- **retry_count**: 재시도 횟수

### 4. 설정 및 구성

#### 4.1 노드 설정
- **config**: 노드별 설정 객체
- **parameters**: 실행 파라미터
- **environment**: 환경 변수
- **secrets**: 보안 정보 (API 키 등)

#### 4.2 실행 옵션
- **timeout**: 타임아웃 설정
- **retry_policy**: 재시도 정책
- **async_mode**: 비동기 실행 여부
- **cache_enabled**: 캐시 사용 여부
- **logging_level**: 로깅 레벨

### 5. 이벤트 및 콜백

#### 5.1 이벤트 시스템
- **on_start**: 실행 시작 이벤트
- **on_complete**: 완료 이벤트
- **on_error**: 오류 이벤트
- **on_progress**: 진행 상황 이벤트

#### 5.2 훅 시스템
- **pre_execute**: 실행 전 훅
- **post_execute**: 실행 후 훅
- **pre_validate**: 검증 전 훅
- **post_validate**: 검증 후 훅

### 6. 메타데이터

#### 6.1 문서화
- **description**: 노드 설명
- **documentation**: 상세 문서
- **examples**: 사용 예시
- **tags**: 분류 태그

#### 6.2 성능 정보
- **metrics**: 성능 메트릭
- **resource_usage**: 리소스 사용량
- **benchmark**: 벤치마크 결과

### 7. 검증 및 보안

#### 7.1 입력 검증
- **type_validation**: 타입 검증
- **range_validation**: 범위 검증
- **format_validation**: 형식 검증
- **custom_validation**: 커스텀 검증 로직

#### 7.2 보안 기능
- **input_sanitization**: 입력 데이터 정제
- **permission_check**: 권한 확인
- **encryption**: 데이터 암호화
- **audit_log**: 감사 로그

### 8. 확장성 및 플러그인

#### 8.1 확장 인터페이스
- **plugin_hooks**: 플러그인 훅 포인트
- **custom_handlers**: 커스텀 핸들러
- **middleware**: 미들웨어 시스템

#### 8.2 호환성
- **version_compatibility**: 버전 호환성
- **migration_support**: 마이그레이션 지원
- **backward_compatibility**: 하위 호환성