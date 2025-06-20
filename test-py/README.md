# Gil-Py 테스트 환경

Gil-Py 라이브러리를 테스트하기 위한 환경입니다. SDK 방식과 YAML 워크플로우 방식을 모두 테스트할 수 있습니다.

## 🚀 빠른 시작

### 1. 환경 설정

```bash
# 의존성 설치
pip install -r requirements.txt

# 환경 변수 설정
cp .env.example .env
# .env 파일을 편집하여 실제 OpenAI API 키를 입력하세요
```

### 2. YAML 워크플로우 테스트 (권장)

```bash
# Windows
run_yaml_tests.bat

# Linux/Mac
chmod +x run_yaml_tests.sh
./run_yaml_tests.sh

# 또는 직접 실행
python test_integrated.py
```

### 3. 개별 테스트 실행

```bash
# YAML 워크플로우만 테스트
python test_yaml_workflows.py

# CLI 명령어만 테스트  
python test_cli.py

# SDK 방식 테스트 (기존)
python test_gil_py.py
```

## 📁 파일 구조

### 🧪 테스트 스크립트
- `test_integrated.py`: **통합 테스트 (권장)**
- `test_yaml_workflows.py`: YAML 워크플로우 테스트
- `test_cli.py`: CLI 명령어 테스트
- `test_gil_py.py`: SDK 방식 테스트
- `run_yaml_tests.bat/.sh`: 원클릭 테스트 실행

### 📄 워크플로우 파일들 (`workflows/`)
- `simple_image_gen.yaml`: 간단한 이미지 생성
- `data_pipeline.yaml`: 데이터 처리 파이프라인
- `conditional_flow.yaml`: 조건부 실행 워크플로우
- `parallel_flow.yaml`: 병렬 처리 워크플로우

### 📊 결과 파일들 (`results/`)
- `*_result.json`: 각 워크플로우 실행 결과
- `test_summary.json`: 전체 테스트 요약

### 🔧 기타 파일들
- `example.py`: 간단한 SDK 사용 예제
- `demo_workflow.py`: 데모 워크플로우
- `.env.example`: 환경 변수 설정 예제
- `requirements.txt`: 의존성 목록

## 🎯 테스트 시나리오

### ✅ YAML 워크플로우 테스트
1. **파싱 테스트**: YAML 파일 구문 분석
2. **검증 테스트**: 워크플로우 구조 검증
3. **실행 테스트**: 실제 워크플로우 실행

### ✅ CLI 테스트
1. **기본 명령어**: `--help`, `list-nodes`, `describe`
2. **검증 명령어**: `validate <workflow.yaml>`
3. **실행 명령어**: `run <workflow.yaml>`

### ✅ 통합 테스트
1. **환경 검증**: 라이브러리 import, API 키 확인
2. **전체 워크플로우**: 파싱 → 검증 → 실행
3. **에러 처리**: 예외 상황 처리 확인

## 🔍 테스트 결과 확인

### 성공 케이스
- ✅ 모든 단계 통과
- 📤 결과 파일 생성
- 🎉 예상 출력 확인

### 실패 케이스
- ❌ 단계별 에러 메시지
- 🔍 디버깅 정보 제공
- ⏭️ 다음 테스트 계속 진행

### API 키 없는 경우
- ⏭️ API 필요 테스트 건너뜀
- ✅ 로컬 테스트만 실행
- ⚠️ 제한 사항 안내

## 🎨 YAML 워크플로우 예제

### 간단한 이미지 생성
```yaml
name: "간단한 이미지 생성"
nodes:
  openai_connector:
    type: "GilConnectorOpenAI"
    config:
      api_key: "${OPENAI_API_KEY}"
  
  image_generator:
    type: "GilGenImage"
    config:
      connector: "@openai_connector"
    inputs:
      prompt: "A beautiful sunset over mountains"
      size: "1024x1024"

flow:
  - openai_connector  
  - image_generator

outputs:
  generated_images: "@image_generator.images"
```

### 조건부 실행
```yaml
name: "조건부 실행 테스트"
nodes:
  input_validator:
    type: "GilUtilTransform"
    inputs:
      operation: "validate_input"
      input_text: "${input.user_message}"
  
  ai_processor:
    type: "GilGenText"
    condition: "@input_validator.is_valid == true"
    # ... 설정
```

## 🚀 CLI 사용 예제

```bash
# 워크플로우 검증
python -m gil_py.cli.main validate workflows/simple_image_gen.yaml

# 워크플로우 실행
python -m gil_py.cli.main run workflows/simple_image_gen.yaml

# 입력 파라미터와 함께 실행
python -m gil_py.cli.main run workflows/conditional_flow.yaml --input user_message="Hello World"

# 노드 정보 확인
python -m gil_py.cli.main describe GilGenImage
```

## 🐛 문제 해결

### Import 에러
```bash
# Gil 라이브러리가 설치되지 않은 경우
cd ../gil-py
pip install -e .
```

### API 키 에러
```bash
# .env 파일 확인
cat .env
# OPENAI_API_KEY=your_actual_key_here
```

### 권한 에러 (Linux/Mac)
```bash
chmod +x run_yaml_tests.sh
chmod +x *.py
```
