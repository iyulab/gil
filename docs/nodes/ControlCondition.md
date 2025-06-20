# ControlCondition 노드 사용 가이드

## 개요
ControlCondition 노드는 조건부 실행을 담당하는 제어 노드입니다. 조건에 따라 다른 데이터나 워크플로우 경로를 선택할 수 있으며, if-then-else 로직을 구현할 수 있습니다.

## 노드 타입
- **타입**: `ControlCondition`
- **카테고리**: Control Nodes
- **버전**: 1.0

## 구성 (Configuration)

### 기본 설정
```yaml
config:
  strict_mode: true          # 엄격한 조건 평가 모드
  default_branch: "else"     # 기본 분기 (then, else, null)
  lazy_evaluation: true      # 지연 평가 (불필요한 분기 실행 방지)
```

### 설정 상세
- **strict_mode**: 조건 평가 시 타입 검사 엄격 적용
- **default_branch**: 조건이 null/undefined일 때 기본 분기
- **lazy_evaluation**: 선택된 분기만 평가 (성능 최적화)

## 입력 (Inputs)

### 필수 입력
- **condition**: 평가할 조건식 또는 값

### 분기 입력
- **then_data**: 조건이 참일 때 반환할 데이터
- **else_data**: 조건이 거짓일 때 반환할 데이터

### 고급 분기 (다중 조건)
```yaml
inputs:
  conditions:
    - condition: "score >= 90"
      data: "excellent"
    - condition: "score >= 70"
      data: "good"
    - condition: "score >= 50"
      data: "average"
  default_data: "poor"
```

## 출력 (Outputs)

### 성공 시
```json
{
  "success": true,
  "data": {
    "result": "선택된 분기의 데이터",
    "branch_taken": "then",          // 선택된 분기: then, else, condition_0, condition_1, ...
    "condition_result": true,        // 조건 평가 결과
    "evaluated_conditions": [        // 평가된 조건들 (다중 조건 시)
      {
        "condition": "score >= 90",
        "result": false
      },
      {
        "condition": "score >= 70", 
        "result": true
      }
    ]
  },
  "metadata": {
    "execution_time": 15,
    "conditions_evaluated": 2
  }
}
```

### 실패 시
```json
{
  "success": false,
  "data": null,
  "error": {
    "type": "ConditionEvaluationError",
    "message": "조건 평가 중 에러: 'undefined_variable'를 찾을 수 없습니다",
    "details": {
      "condition": "undefined_variable > 100",
      "variable": "undefined_variable"
    }
  }
}
```

## 컨텍스트 (Context)

### Flow Context 사용
```yaml
environment_check:
  type: "ControlCondition"
  inputs:
    condition: "${flow.environment} == 'production'"
    then_data: "${flow.prod_config}"
    else_data: "${flow.dev_config}"
```

### Node Context 활용
```yaml
user_access_check:
  type: "ControlCondition"
  inputs:
    condition: "@auth.user.role == 'admin'"
    then_data: "@admin_data.full_access"
    else_data: "@user_data.limited_access"
```

## 사용 예시

### 1. 간단한 조건 분기
```yaml
age_check:
  type: "ControlCondition"
  inputs:
    condition: "@user.age >= 18"
    then_data: "성인"
    else_data: "미성년자"
```

### 2. 복합 조건
```yaml
access_control:
  type: "ControlCondition"
  inputs:
    condition: "@user.role == 'admin' || (@user.role == 'user' && @user.verified == true)"
    then_data:
      access: "granted"
      level: "@user.role"
    else_data:
      access: "denied"
      reason: "insufficient_privileges"
```

### 3. 다중 조건 분기
```yaml
grade_calculator:
  type: "ControlCondition"
  inputs:
    conditions:
      - condition: "@student.score >= 90"
        data:
          grade: "A"
          message: "Excellent work!"
      - condition: "@student.score >= 80"
        data:
          grade: "B"
          message: "Good job!"
      - condition: "@student.score >= 70"
        data:
          grade: "C"
          message: "Satisfactory"
      - condition: "@student.score >= 60"
        data:
          grade: "D"
          message: "Needs improvement"
    default_data:
      grade: "F"
      message: "Failed - please retake"
```

### 4. 상태 기반 워크플로우 분기
```yaml
order_status_handler:
  type: "ControlCondition"
  inputs:
    condition: "@order.status"
    conditions:
      - condition: "@order.status == 'pending'"
        data:
          action: "process_payment"
          next_step: "payment_gateway"
      - condition: "@order.status == 'paid'"
        data:
          action: "prepare_shipment"
          next_step: "warehouse"
      - condition: "@order.status == 'shipped'"
        data:
          action: "track_delivery"
          next_step: "tracking_system"
    default_data:
      action: "error_handling"
      next_step: "customer_service"
```

### 5. API 응답 상태 확인
```yaml
api_response_handler:
  type: "ControlCondition"
  inputs:
    condition: "@api_call.response.status_code < 400"
    then_data: "@api_call.response.json"
    else_data:
      error: true
      status: "@api_call.response.status_code"
      message: "@api_call.response.body"
```

### 6. 데이터 존재 여부 확인
```yaml
data_validator:
  type: "ControlCondition"
  inputs:
    condition: "@input.data != null && @input.data.length > 0"
    then_data:
      valid: true
      count: "@input.data.length"
      data: "@input.data"
    else_data:
      valid: false
      error: "No data provided"
      data: []
```

### 7. 환경별 설정 선택
```yaml
config_selector:
  type: "ControlCondition"
  config:
    lazy_evaluation: true
  inputs:
    conditions:
      - condition: "${flow.environment} == 'production'"
        data:
          database_url: "${secrets.prod_db_url}"
          debug: false
          log_level: "error"
      - condition: "${flow.environment} == 'staging'"
        data:
          database_url: "${secrets.staging_db_url}"
          debug: true
          log_level: "info"
    default_data:
      database_url: "${secrets.dev_db_url}"
      debug: true
      log_level: "debug"
```

## 조건식 문법

### 비교 연산자
```yaml
# 숫자 비교
condition: "@value > 100"
condition: "@score >= 80"
condition: "@count == 0"
condition: "@price != 1000"

# 문자열 비교
condition: "@status == 'active'"
condition: "@name != ''"
```

### 논리 연산자
```yaml
# AND 연산
condition: "@age >= 18 && @verified == true"

# OR 연산
condition: "@role == 'admin' || @role == 'moderator'"

# NOT 연산
condition: "!@user.deleted"
condition: "@status != 'inactive'"
```

### null/undefined 확인
```yaml
# 값 존재 확인
condition: "@data != null"
condition: "@optional_field != undefined"

# 빈 값 확인
condition: "@array.length > 0"
condition: "@string != ''"
```

### 배열/객체 확인
```yaml
# 배열 길이
condition: "@items.length >= 5"

# 객체 속성 존재
condition: "@user.email != undefined"

# 배열 포함 여부 (구현체 지원 시)
condition: "@allowed_roles.includes(@user.role)"
```

## 에러 처리

### 안전한 조건 평가
```yaml
safe_condition:
  type: "ControlCondition"
  config:
    strict_mode: false
  inputs:
    condition: "@potentially_undefined?.property > 0"
    then_data: "valid"
    else_data: "invalid_or_missing"
```

### 기본값 설정
```yaml
with_fallback:
  type: "ControlCondition"
  config:
    default_branch: "else"
  inputs:
    condition: "@uncertain_data.exists"
    then_data: "@uncertain_data.value"
    else_data: "default_value"
```

## 성능 고려사항

### 지연 평가 활용
```yaml
optimized_condition:
  type: "ControlCondition"
  config:
    lazy_evaluation: true
  inputs:
    condition: "@quick_check == true"
    then_data: "@expensive_computation.result"  # 조건이 참일 때만 계산
    else_data: "default_result"
```

### 조건 순서 최적화
```yaml
efficient_multi_condition:
  type: "ControlCondition"
  inputs:
    conditions:
      # 빠르고 일반적인 조건을 먼저
      - condition: "@cache.exists"
        data: "@cache.value"
      # 느린 조건을 나중에
      - condition: "@database.expensive_check() == true"
        data: "@database.result"
    default_data: "fallback_value"
```

## 관련 노드

- **ControlLoop**: 반복적 조건 처리
- **TransformData**: 조건부 데이터 변환
- **ControlMerge**: 조건부 데이터 병합
- **ControlSplit**: 조건부 데이터 분할
