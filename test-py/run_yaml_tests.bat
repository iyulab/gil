@echo off
echo Gil YAML 워크플로우 테스트 시작
echo ================================

echo.
echo 환경 변수 확인...
if not exist .env (
    echo ⚠️  .env 파일이 없습니다. .env.example을 복사해서 .env를 만들고 API 키를 설정하세요.
    echo.
    echo copy .env.example .env
    echo.
    pause
    exit /b 1
)

echo ✅ .env 파일 발견

echo.
echo Python 환경 확인...
python --version
if %errorlevel% neq 0 (
    echo ❌ Python이 설치되지 않았거나 PATH에 없습니다.
    pause
    exit /b 1
)

echo.
echo 필요한 패키지 확인...
python -c "import dotenv; print('✅ python-dotenv 사용 가능')" 2>nul || (
    echo ⚠️  python-dotenv가 설치되지 않았습니다. 설치 중...
    pip install python-dotenv
)

echo.
echo Gil 라이브러리 경로 확인...
if not exist "..\gil-py\gil_py" (
    echo ❌ Gil 라이브러리를 찾을 수 없습니다. ../gil-py/ 경로를 확인하세요.
    pause
    exit /b 1
)

echo ✅ Gil 라이브러리 경로 확인됨

echo.
echo ==========================================
echo 통합 테스트 실행 중...
echo ==========================================
python test_integrated.py

echo.
echo ==========================================
echo 테스트 완료!
echo ==========================================

echo.
echo 결과 파일들:
echo   - results\*.json: 테스트 결과 데이터
echo   - workflows\*.yaml: 테스트 워크플로우

echo.
pause
