# Gil-Py 테스트 환경

Gil-Py 라이브러리를 테스트하기 위한 정리된 환경입니다. 핵심 기능 테스트와 YAML 워크플로우 실행을 지원합니다.

## 🚀 빠른 시작

### 1. 환경 설정

```bash
# 의존성 설치
pip install -r requirements.txt

# 환경 변수 설정
cp .env.example .env
# .env 파일을 편집하여 실제 OpenAI API 키를 입력하세요
```

### 2. 테스트 실행

```bash
# 핵심 기능 테스트
python test_gil.py

# 컨텍스트 기능 테스트
python test_context.py
```

### 3. YAML 워크플로우 실행

```bash
# 이미지 생성 워크플로우
python -m gil_py generate-image.yaml

# 컨텍스트 테스트 워크플로우
python -m gil_py context-test.yaml

# 스마트 콘텐츠 생성
python -m gil_py smart-content-generator.yaml
```

## 📁 파일 구조

### 🧪 테스트 스크립트
- `test_gil.py`: 핵심 라이브러리 기능 테스트
- `test_context.py`: 컨텍스트 관련 기능 테스트
### 📄 YAML 워크플로우
- `generate-image.yaml`: 이미지 생성 워크플로우
- `context-test.yaml`: 컨텍스트 기능 테스트
- `smart-content-generator.yaml`: 스마트 콘텐츠 생성
- `workflows/`: 추가 워크플로우 예제 모음

### 📊 결과 디렉토리
- `generated_images/`: 생성된 이미지 저장소
- `context_results/`: 컨텍스트 테스트 결과

### 🔧 환경 설정
- `.env.example`: 환경 변수 설정 예제
- `requirements.txt`: 의존성 목록

## 🎯 테스트 시나리오

### ✅ 핵심 기능 테스트 (`test_gil.py`)
1. **라이브러리 임포트**: Gil-Py 모듈 정상 로드 확인
2. **기본 노드 생성**: 핵심 노드 타입 인스턴스화
3. **워크플로우 실행**: 간단한 워크플로우 처리

### ✅ 컨텍스트 테스트 (`test_context.py`)
1. **컨텍스트 관리**: 상태 저장 및 복원
2. **변수 바인딩**: 노드 간 데이터 전달
3. **조건부 실행**: 동적 워크플로우 제어

### ✅ YAML 워크플로우 테스트
1. **파싱 검증**: YAML 구문 분석 및 구조 검증
2. **실행 테스트**: 실제 워크플로우 실행
3. **결과 확인**: 출력 데이터 검증

## 🚀 사용 예제

### 기본 라이브러리 테스트
```bash
python test_gil.py
```

### 컨텍스트 기능 테스트
```bash
python test_context.py
```

### YAML 워크플로우 실행
```bash
# 이미지 생성
python -m gil_py generate-image.yaml

# 컨텍스트 테스트
python -m gil_py context-test.yaml
```

## � 테스트 결과 확인

### 성공 케이스
- ✅ 모든 단계 통과
- 📤 결과 파일 생성 (`generated_images/`, `context_results/`)
- 🎉 예상 출력 확인

### API 키 설정
```bash
# .env 파일 생성 및 편집
cp .env.example .env
# OPENAI_API_KEY=your_actual_key_here 설정
```

### 의존성 설치
```bash
pip install -r requirements.txt
```
