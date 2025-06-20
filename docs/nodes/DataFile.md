# DataFile 노드 사용 가이드

## 개요
DataFile 노드는 파일 시스템과의 상호작용을 담당하는 기본 노드입니다. 텍스트 파일, 바이너리 파일의 읽기/쓰기를 지원하며, 파일 정보 조회 기능도 제공합니다.

## 노드 타입
- **타입**: `DataFile`
- **카테고리**: Data Nodes
- **버전**: 1.0

## 구성 (Configuration)

### 기본 설정
```yaml
config:
  operation: "read"           # 작업 타입: read, write, append, info
  encoding: "utf-8"          # 인코딩 (텍스트 파일 시)
  binary: false              # 바이너리 모드 여부
  create_dirs: true          # 디렉토리 자동 생성 여부
  backup: false              # 백업 파일 생성 여부
```

### 설정 상세
- **operation**: 수행할 파일 작업
  - `read`: 파일 읽기
  - `write`: 파일 쓰기 (덮어쓰기)
  - `append`: 파일 끝에 추가
  - `info`: 파일 정보만 조회
- **encoding**: 텍스트 파일 인코딩 (기본값: utf-8)
- **binary**: 바이너리 모드로 작업할지 여부
- **create_dirs**: 경로의 디렉토리가 없으면 자동 생성
- **backup**: 기존 파일을 백업 후 작업 (write 시)

## 입력 (Inputs)

### 필수 입력
- **file_path**: 작업할 파일 경로

### 선택 입력 (작업별)
- **content**: 쓸 내용 (write, append 시)
- **data**: 바이너리 데이터 (바이너리 모드 시)

## 출력 (Outputs)

### 성공 시
```json
{
  "success": true,
  "data": {
    "content": "파일 내용",
    "file_info": {
      "path": "/path/to/file.txt",
      "size": 1024,
      "modified": "2025-01-01T00:00:00Z",
      "exists": true,
      "is_file": true,
      "is_directory": false
    },
    "operation": "read",
    "encoding": "utf-8"
  },
  "metadata": {
    "execution_time": 50,
    "bytes_processed": 1024
  }
}
```

### 실패 시
```json
{
  "success": false,
  "data": null,
  "error": {
    "type": "FileNotFoundError",
    "message": "파일을 찾을 수 없습니다: /path/to/file.txt",
    "details": {
      "file_path": "/path/to/file.txt",
      "operation": "read"
    }
  }
}
```

## 컨텍스트 (Context)

### Flow Context 사용
```yaml
read_config:
  type: "DataFile"
  config:
    operation: "read"
  inputs:
    file_path: "${flow.config_file_path}"
```

### Node Context 활용
```yaml
backup_file:
  type: "DataFile"
  config:
    operation: "write"
    backup: true
  inputs:
    file_path: "@source.file_info.path"
    content: "@processed.content"
```

## 사용 예시

### 1. 텍스트 파일 읽기
```yaml
read_log:
  type: "DataFile"
  config:
    operation: "read"
    encoding: "utf-8"
  inputs:
    file_path: "/var/log/application.log"
```

### 2. 파일 쓰기 (백업 포함)
```yaml
write_report:
  type: "DataFile"
  config:
    operation: "write"
    backup: true
    create_dirs: true
  inputs:
    file_path: "/reports/daily_report.txt"
    content: "@report_generator.content"
```

### 3. 바이너리 파일 처리
```yaml
process_image:
  type: "DataFile"
  config:
    operation: "read"
    binary: true
  inputs:
    file_path: "/images/photo.jpg"
```

### 4. 파일 정보 조회
```yaml
check_file:
  type: "DataFile"
  config:
    operation: "info"
  inputs:
    file_path: "/data/input.csv"
```

### 5. 로그 파일에 추가
```yaml
append_log:
  type: "DataFile"
  config:
    operation: "append"
  inputs:
    file_path: "/logs/workflow.log"
    content: "[${system.timestamp}] Workflow completed successfully\n"
```

## 에러 처리

### 일반적인 에러 타입
- **FileNotFoundError**: 파일을 찾을 수 없음
- **PermissionError**: 파일 접근 권한 없음
- **IsADirectoryError**: 파일 경로가 디렉토리임
- **OSError**: 일반적인 파일 시스템 에러
- **UnicodeDecodeError**: 인코딩 문제

### 에러 처리 예시
```yaml
safe_read:
  type: "DataFile"
  config:
    operation: "read"
    encoding: "utf-8"
  inputs:
    file_path: "/data/input.txt"
  timeout: 30000
  on_error:
    action: "continue"
    default_value: ""
```

## 보안 고려사항

- 파일 경로는 반드시 검증되어야 합니다
- 사용자 입력에서 오는 경로는 path traversal 공격에 주의
- 민감한 파일에 대한 접근 권한 검증 필요
- 대용량 파일 처리 시 메모리 사용량 모니터링

## 성능 고려사항

- 대용량 파일은 스트리밍 방식으로 처리 고려
- 동시 파일 접근 시 락 메커니즘 필요
- 네트워크 파일 시스템 사용 시 타임아웃 설정 중요
- 바이너리 파일 처리 시 메모리 효율성 고려

## 관련 노드

- **DataCSV**: CSV 파일 전용 처리
- **DataJSON**: JSON 파일 전용 처리
- **DataExcel**: Excel 파일 전용 처리
- **TransformTemplate**: 파일 내용 템플릿 처리
