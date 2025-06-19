# Gil-Py 테스트 환경

Gil-Py 라이브러리를 테스트하기 위한 환경입니다.

## 설정

1. 의존성 설치:
```bash
pip install -r requirements.txt
```

2. 환경 변수 설정:
```bash
cp .env.example .env
# .env 파일을 편집하여 실제 OpenAI API 키를 입력하세요
```

## 실행

### 기본 예제 실행
```bash
python example.py
```

### 전체 테스트 실행
```bash
python test_gil_py.py
```

## 파일 설명

- `example.py`: 간단한 사용 예제
- `test_gil_py.py`: 전체 기능 테스트 스위트
- `.env.example`: 환경 변수 설정 예제
- `requirements.txt`: 의존성 목록
