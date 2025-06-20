#!/bin/bash

echo "Gil YAML 워크플로우 테스트 시작"
echo "================================"

echo ""
echo "환경 변수 확인..."
if [ ! -f .env ]; then
    echo "⚠️  .env 파일이 없습니다. .env.example을 복사해서 .env를 만들고 API 키를 설정하세요."
    echo ""
    echo "cp .env.example .env"
    echo ""
    exit 1
fi

echo "✅ .env 파일 발견"

echo ""
echo "Python 환경 확인..."
python3 --version || python --version
if [ $? -ne 0 ]; then
    echo "❌ Python이 설치되지 않았거나 PATH에 없습니다."
    exit 1
fi

echo ""
echo "필요한 패키지 확인..."
python3 -c "import dotenv; print('✅ python-dotenv 사용 가능')" 2>/dev/null || python -c "import dotenv; print('✅ python-dotenv 사용 가능')" 2>/dev/null || {
    echo "⚠️  python-dotenv가 설치되지 않았습니다. 설치 중..."
    pip install python-dotenv || pip3 install python-dotenv
}

echo ""
echo "Gil 라이브러리 경로 확인..."
if [ ! -d "../gil-py/gil_py" ]; then
    echo "❌ Gil 라이브러리를 찾을 수 없습니다. ../gil-py/ 경로를 확인하세요."
    exit 1
fi

echo "✅ Gil 라이브러리 경로 확인됨"

echo ""
echo "=========================================="
echo "통합 테스트 실행 중..."
echo "=========================================="

# Python 3 사용 시도, 없으면 python 사용
if command -v python3 &> /dev/null; then
    python3 test_integrated.py
else
    python test_integrated.py
fi

echo ""
echo "=========================================="
echo "테스트 완료!"
echo "=========================================="

echo ""
echo "결과 파일들:"
echo "  - results/*.json: 테스트 결과 데이터"
echo "  - workflows/*.yaml: 테스트 워크플로우"

echo ""
