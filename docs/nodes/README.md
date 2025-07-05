# Gil-Flow 노드 사용 가이드

이 디렉토리는 Gil-Flow에서 지원하는 각 노드 타입별 상세한 사용 가이드를 제공합니다.

## 📚 사용 가능한 노드 문서

### 데이터 노드 (Data Nodes)
- **[Data-ReadFile](DataReadFile)** - 파일 읽기
- **[Data-Transform](TransformData)** - 데이터 변환

### AI 노드 (AI Nodes)
- **[OpenAI-GenerateText](AITextGeneration)** - AI 모델을 활용한 텍스트 생성
- **[OpenAI-GenerateImage](GilGenImage)** - AI 이미지 생성 (OpenAI DALL-E)

### 커넥터 노드 (Connector Nodes)
- **[OpenAI-Connector](GilConnectorOpenAI)** - OpenAI API 전용 커넥터

### 제어 노드 (Control Nodes)
- **[Control-Branch](ControlBranch)** - 조건부 실행 및 분기 처리

### 유틸리티 노드 (Utility Nodes)
- **[Util-LogMessage](UtilLogMessage)** - 콘솔에 메시지 로깅
- **[Util-SetVariable](UtilSetVariable)** - 워크플로우 컨텍스트에 변수 설정

## 📋 표준 노드 목록

### 구현 완료 노드 ✅
| 노드 타입 | 문서 상태 | 설명 |
|-----------|-----------|------|
| Data-ReadFile | ✅ | 파일 읽기 |
| Data-Transform | ✅ | 데이터 변환 |
| OpenAI-GenerateText | ✅ | AI 텍스트 생성 |
| OpenAI-GenerateImage | ✅ | AI 이미지 생성 |
| OpenAI-Connector | ✅ | OpenAI 커넥터 |
| Control-Branch | ✅ | 조건부 실행 |
| Util-LogMessage | ✅ | 콘솔에 메시지 로깅 |
| Util-SetVariable | ✅ | 워크플로우 컨텍스트에 변수 설정 |

### 문서 작성 대기 노드 📝
| 노드 타입 | 카테고리 | 우선순위 |
|-----------|----------|----------|
| DataCSV | Data | 높음 |
| DataJSON | Data | 높음 |
| DataDatabase | Data | 중간 |
| DataExcel | Data | 낮음 |
| TransformTemplate | Transform | 높음 |
| TransformValidate | Transform | 중간 |
| TransformAggregate | Transform | 중간 |
| AIAnalyzeText | AI | 중간 |
| AITranslate | AI | 중간 |
| CommEmail | Communication | 중간 |
| CommSlack | Communication | 낮음 |
| CommWebhook | Communication | 중간 |
| ControlLoop | Control | 높음 |
| ControlMerge | Control | 중간 |
| ControlSplit | Control | 중간 |

## 🔗 문서 구조

각 노드 문서는 다음 구조를 따릅니다:

1. **개요** - 노드의 목적과 기능
2. **노드 타입** - 타입명, 카테고리, 버전
3. **구성 (Configuration)** - 노드 설정 옵션
4. **입력 (Inputs)** - 필수/선택 입력 포트
5. **출력 (Outputs)** - 성공/실패 시 출력 형식
6. **컨텍스트 (Context)** - Flow/Node Context 사용법
7. **사용 예시** - 실제 YAML 사용 예제들
8. **에러 처리** - 일반적인 에러 상황과 대응
9. **보안/성능 고려사항** - 실무 사용 시 주의점
10. **관련 노드** - 함께 사용하면 좋은 노드들

## 🎯 문서 사용 가이드

### 새로운 노드 추가 시
1. [NODE_SPEC](../NODE_SPEC)에 노드 스펙 정의
2. 해당 노드명으로 `.md` 파일 생성
3. 표준 문서 구조 따라 작성
4. 이 README에 노드 목록 업데이트

### 기존 노드 업데이트 시
1. 노드 문서 직접 수정
2. 변경사항이 있으면 [NODE_SPEC](../NODE_SPEC)도 동기화
3. 관련 예제 및 워크플로우 업데이트

## 📖 추가 참고 문서

- **[NODE_SPEC](../NODE_SPEC)** - 전체 노드 타입 표준 명세
- **[YAML_SPEC](../YAML_SPEC)** - Gil-Flow YAML 문법 가이드
- **[CONTEXT_SYSTEM](../CONTEXT_SYSTEM)** - 컨텍스트 시스템 상세 가이드
- **[ARCHITECTURE](../ARCHITECTURE)** - 전체 아키텍처 가이드

## 🤝 기여 가이드

새로운 노드 문서 작성이나 기존 문서 개선에 참여하실 분들은:

1. 문서 작성 시 실제 사용 예제 포함
2. 에러 처리 및 엣지 케이스 고려
3. 보안과 성능 측면 가이드 제공
4. 다른 노드와의 연관성 명시

문서 작성 우선순위는 **높음 > 중간 > 낮음** 순서로 진행하며, 실제 사용 빈도와 중요도를 고려하여 결정됩니다.