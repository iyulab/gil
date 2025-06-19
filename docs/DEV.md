# Gil 라이브러리 설계서

## 프로젝트 개요
**Gil**은 플로우차트 기반의 워크플로우 노드 시스템을 제공하는 라이브러리입니다. 다양한 작업을 노드로 모듈화하고, 이들을 연결하여 복잡한 워크플로우를 구성할 수 있습니다.

## 언어별 라이브러리

### GilPy (Python)
- **패키지명**: `gil-py`
- **설치**: `pip install gil-py`
- **타겟**: Python 3.8+
- **특징**: 비동기 처리, 타입 힌트, 데코레이터 지원

### GilNode (Node.js)
- **패키지명**: `gil-node`
- **설치**: `npm install gil-node`
- **타겟**: Node.js 16+
- **특징**: Promise/async-await, TypeScript 지원, 스트림 처리

### GilSharp (C#)
- **패키지명**: `Gil.Sharp`
- **설치**: `dotnet add package Gil.Sharp`
- **타겟**: .NET 6.0+
- **특징**: LINQ 지원, 강타입 시스템, Task 기반 비동기

## 노드 종류

### 1. AI 생성 노드 (GilGen 시리즈)
- **GilGenText**: 텍스트 생성/처리
- **GilGenImage**: 이미지 생성
- **GilGenAudio**: 음성/오디오 생성
- **GilGenCode**: 코드 생성
- **GilGenTranslate**: 번역 생성
- **GilGenSummary**: 요약 생성
- **GilGenChat**: 대화형 응답 생성
- **GilGenEmbedding**: 텍스트 임베딩 생성

### 2. AI 분석 노드 (GilAnalyze 시리즈)
- **GilAnalyzeVision**: 이미지 분석
- **GilAnalyzeSentiment**: 감정 분석
- **GilAnalyzeIntent**: 의도 분석
- **GilAnalyzeData**: 데이터 분석
- **GilAnalyzeText**: 텍스트 분석 (키워드, 분류 등)
- **GilAnalyzeSimilarity**: 유사도 분석

### 3. AI 커넥터 노드
- **GilConnectorOpenAI**: OpenAI API 연결
- **GilConnectorAnthropic**: Anthropic Claude API 연결
- **GilConnectorLocal**: 로컬 모델 연결
- **GilConnectorHuggingFace**: HuggingFace 모델 연결
- **GilConnectorOllama**: Ollama 로컬 모델 연결

### 4. 통신 노드 (GilComm 시리즈)
- **GilCommEmail**: 이메일 발송
- **GilCommSMS**: SMS 발송
- **GilCommWebhook**: 웹훅 호출
- **GilCommAPI**: REST API 호출
- **GilCommSlack**: 슬랙 메시지 발송
- **GilCommDiscord**: 디스코드 메시지 발송
- **GilCommTelegram**: 텔레그램 메시지 발송
- **GilCommKakao**: 카카오톡 메시지 발송

### 5. 데이터 처리 노드 (GilData 시리즈)
- **GilDataDB**: 데이터베이스 처리
- **GilDataFile**: 파일 읽기/쓰기
- **GilDataCSV**: CSV 처리
- **GilDataJSON**: JSON 처리
- **GilDataXML**: XML 처리
- **GilDataExcel**: Excel 처리
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