# TransformData 노드 사용 가이드

## 개요
TransformData 노드는 데이터 변환 작업을 담당하는 핵심 노드입니다. 필터링, 매핑, 정렬, 그룹화 등 다양한 데이터 변환 연산을 체인 방식으로 수행할 수 있습니다.

## 노드 타입
- **타입**: `TransformData`
- **카테고리**: Transform Nodes
- **버전**: 1.0

## 구성 (Configuration)

### 기본 설정
```yaml
config:
  preserve_original: false   # 원본 데이터 보존 여부
  fail_on_error: true       # 변환 실패 시 에러 발생 여부
  parallel: false           # 병렬 처리 여부
  batch_size: 1000         # 배치 처리 크기
```

### 설정 상세
- **preserve_original**: 변환 결과와 함께 원본 데이터도 반환
- **fail_on_error**: 개별 아이템 변환 실패 시 전체 실패 여부
- **parallel**: 가능한 경우 병렬 처리 수행
- **batch_size**: 대용량 데이터 처리 시 배치 크기

## 입력 (Inputs)

### 필수 입력
- **data**: 변환할 데이터 (배열 또는 객체)
- **operations**: 수행할 변환 연산 목록

### 변환 연산 타입

#### 1. Filter (필터링)
```yaml
operations:
  - type: "filter"
    condition: "age >= 18"           # 조건식
    # 또는
    # field: "status"                # 필드명
    # value: "active"                # 값
    # operator: "equals"             # 연산자
```

#### 2. Map (매핑/변환)
```yaml
operations:
  - type: "map"
    expression: "name.toUpperCase()" # 변환 표현식
    # 또는
    # field: "full_name"             # 새 필드명
    # source: "first_name + ' ' + last_name"  # 소스 표현식
```

#### 3. Sort (정렬)
```yaml
operations:
  - type: "sort"
    key: "age"                      # 정렬 키
    order: "desc"                   # asc 또는 desc
    # 또는 다중 키 정렬
    # keys:
    #   - field: "department"
    #     order: "asc"
    #   - field: "salary"
    #     order: "desc"
```

#### 4. Group (그룹화)
```yaml
operations:
  - type: "group"
    by: "department"                # 그룹화 필드
    aggregate:
      count: "count()"
      avg_salary: "avg(salary)"
      max_age: "max(age)"
```

#### 5. Select (필드 선택)
```yaml
operations:
  - type: "select"
    fields: ["id", "name", "email"] # 선택할 필드 목록
    # 또는
    # exclude: ["password", "secret"] # 제외할 필드 목록
```

#### 6. Rename (필드 이름 변경)
```yaml
operations:
  - type: "rename"
    mapping:
      old_name: "new_name"
      user_id: "id"
      full_name: "name"
```

## 출력 (Outputs)

### 성공 시
```json
{
  "success": true,
  "data": {
    "transformed_data": [...],
    "original_data": [...],        // preserve_original이 true인 경우
    "statistics": {
      "original_count": 100,
      "final_count": 85,
      "operations_applied": ["filter", "map", "sort"],
      "processing_time": 234
    }
  },
  "metadata": {
    "execution_time": 250,
    "operations_count": 3,
    "batch_processed": false
  }
}
```

### 실패 시
```json
{
  "success": false,
  "data": {
    "partial_result": [...],       // 부분 처리 결과 (있는 경우)
    "failed_items": [...]          // 실패한 아이템들
  },
  "error": {
    "type": "TransformationError",
    "message": "필터 조건 처리 중 오류: 'age' 필드를 찾을 수 없습니다",
    "details": {
      "operation": "filter",
      "operation_index": 0,
      "condition": "age >= 18"
    }
  }
}
```

## 컨텍스트 (Context)

### Flow Context 사용
```yaml
transform_users:
  type: "TransformData"
  inputs:
    data: "@api_call.response.json.users"
    operations:
      - type: "filter"
        condition: "status == '${flow.target_status}'"
```

### Node Context 활용
```yaml
process_data:
  type: "TransformData"
  inputs:
    data: "@source.data"
    operations:
      - type: "map"
        field: "processed_at"
        source: "${system.current_timestamp}"
      - type: "filter"
        condition: "@previous.validation_result == true"
```

## 사용 예시

### 1. 기본 필터링과 정렬
```yaml
filter_and_sort:
  type: "TransformData"
  inputs:
    data: "@users_api.response.json"
    operations:
      - type: "filter"
        condition: "age >= 18 && status == 'active'"
      - type: "sort"
        key: "created_at"
        order: "desc"
```

### 2. 복합 데이터 변환
```yaml
complex_transform:
  type: "TransformData"
  config:
    preserve_original: true
  inputs:
    data: "@raw_data.content"
    operations:
      - type: "select"
        fields: ["id", "name", "email", "department", "salary"]
      - type: "map"
        field: "display_name"
        source: "name.toUpperCase()"
      - type: "filter"
        condition: "salary > 50000"
      - type: "group"
        by: "department"
        aggregate:
          count: "count()"
          avg_salary: "avg(salary)"
```

### 3. 다중 키 정렬
```yaml
multi_sort:
  type: "TransformData"
  inputs:
    data: "@employee_data.records"
    operations:
      - type: "sort"
        keys:
          - field: "department"
            order: "asc"
          - field: "hire_date"
            order: "desc"
          - field: "salary"
            order: "desc"
```

### 4. 필드 매핑과 이름 변경
```yaml
field_mapping:
  type: "TransformData"
  inputs:
    data: "@legacy_api.response"
    operations:
      - type: "rename"
        mapping:
          user_id: "id"
          full_name: "name"
          email_address: "email"
      - type: "map"
        field: "status"
        source: "active ? 'enabled' : 'disabled'"
```

### 5. 조건부 필터링
```yaml
conditional_filter:
  type: "TransformData"
  inputs:
    data: "@orders.data"
    operations:
      - type: "filter"
        field: "status"
        value: "completed"
        operator: "equals"
      - type: "filter"
        field: "amount"
        value: 100
        operator: "greater_than"
      - type: "select"
        fields: ["id", "customer_name", "amount", "date"]
```

### 6. 그룹화와 집계
```yaml
sales_summary:
  type: "TransformData"
  inputs:
    data: "@sales_data.records"
    operations:
      - type: "group"
        by: "region"
        aggregate:
          total_sales: "sum(amount)"
          order_count: "count()"
          avg_order: "avg(amount)"
          max_order: "max(amount)"
      - type: "sort"
        key: "total_sales"
        order: "desc"
```

## 연산자 참조

### 비교 연산자
- `equals`, `==`: 같음
- `not_equals`, `!=`: 같지 않음
- `greater_than`, `>`: 큼
- `less_than`, `<`: 작음
- `greater_equal`, `>=`: 크거나 같음
- `less_equal`, `<=`: 작거나 같음

### 문자열 연산자
- `contains`: 포함
- `starts_with`: 시작
- `ends_with`: 끝남
- `regex`: 정규표현식 매칭

### 논리 연산자
- `&&`, `and`: 그리고
- `||`, `or`: 또는
- `!`, `not`: 부정

### 집계 함수
- `count()`: 개수
- `sum(field)`: 합계
- `avg(field)`: 평균
- `min(field)`: 최솟값
- `max(field)`: 최댓값
- `first(field)`: 첫 번째 값
- `last(field)`: 마지막 값

## 에러 처리

### 관용적 에러 처리
```yaml
safe_transform:
  type: "TransformData"
  config:
    fail_on_error: false
  inputs:
    data: "@unreliable_source.data"
    operations:
      - type: "filter"
        condition: "age != null && age >= 0"
      - type: "map"
        field: "age_group"
        source: "age < 30 ? 'young' : 'adult'"
```

### 부분 실패 허용
```yaml
lenient_transform:
  type: "TransformData"
  config:
    fail_on_error: false
    preserve_original: true
  inputs:
    data: "@mixed_data.records"
    operations:
      - type: "filter"
        condition: "required_field != null"
```

## 성능 최적화

### 대용량 데이터 처리
```yaml
big_data_transform:
  type: "TransformData"
  config:
    parallel: true
    batch_size: 5000
  inputs:
    data: "@large_dataset.records"
    operations:
      - type: "filter"
        condition: "status == 'active'"
      - type: "select"
        fields: ["id", "name", "email"]
```

### 연산 순서 최적화
```yaml
optimized_transform:
  type: "TransformData"
  inputs:
    data: "@source.data"
    operations:
      # 필터를 먼저 수행하여 데이터 크기 줄이기
      - type: "filter"
        condition: "active == true"
      # 필요한 필드만 선택
      - type: "select"
        fields: ["id", "name", "score"]
      # 마지막에 정렬
      - type: "sort"
        key: "score"
        order: "desc"
```

## 관련 노드

- **TransformTemplate**: 템플릿 기반 변환
- **TransformValidate**: 데이터 검증
- **TransformAggregate**: 고급 집계 연산
- **ControlLoop**: 반복적 데이터 처리
- **DataCSV**: CSV 데이터와 함께 사용
