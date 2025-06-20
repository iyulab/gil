"""
Gil-Py PyPI 배포 완료 가이드

🎉 Gil-Py 패키지 빌드가 성공적으로 완료되었습니다!
"""

print("""
🎉 Gil-Py PyPI 배포 준비 완료!
==================================================

✅ 완료된 작업:
- 패키지 빌드 성공 (wheel + tar.gz)
- 패키지 검증 통과
- 배포 준비 완료

📦 생성된 파일:
- gil_py-0.1.0-py3-none-any.whl
- gil_py-0.1.0.tar.gz

🚀 PyPI 배포 방법:

1. PyPI 계정 생성
   - https://pypi.org/account/register/ 에서 계정 생성
   - https://test.pypi.org/account/register/ 에서 테스트 계정 생성

2. API 토큰 생성
   - PyPI 계정 > Account settings > Add API token
   - Scope: "Entire account" 선택
   - 토큰을 안전하게 보관

3. TestPyPI에 먼저 업로드 (권장)
   cd gil-py
   py -m twine upload --repository testpypi dist/*
   # 사용자명: __token__
   # 비밀번호: 생성한 API 토큰

4. TestPyPI에서 설치 테스트
   pip install -i https://test.pypi.org/simple/ gil-py

5. 실제 PyPI에 배포
   py -m twine upload dist/*

💡 참고사항:
- 한 번 업로드된 버전은 삭제할 수 없습니다
- 새 버전 업로드 시 pyproject.toml의 version 수정 필요
- TestPyPI에서 충분히 테스트 후 실제 PyPI에 업로드하세요

🔗 유용한 링크:
- PyPI 가이드: https://packaging.python.org/tutorials/packaging-projects/
- Twine 문서: https://twine.readthedocs.io/
- Gil-Py 패키지 위치: https://pypi.org/project/gil-py/ (업로드 후)

==================================================
""")

# 패키지 테스트 코드 생성
test_code = '''
"""
Gil-Py 설치 후 테스트 코드
pip install gil-py 후 실행하세요
"""

def test_gil_py_installation():
    """Gil-Py 설치 테스트"""
    try:
        # 모듈 임포트 테스트
        from gil_py import GilConnectorOpenAI, GilGenImage
        print("✅ Gil-Py 임포트 성공!")
        
        # 클래스 인스턴스 생성 테스트
        connector = GilConnectorOpenAI(api_key="test-key")
        generator = GilGenImage(connector=connector)
        
        print("✅ 클래스 생성 성공!")
        print(f"   커넥터: {connector.name}")
        print(f"   생성기: {generator.name}")
        
        return True
        
    except ImportError as e:
        print(f"❌ 임포트 실패: {e}")
        return False
    except Exception as e:
        print(f"❌ 테스트 실패: {e}")
        return False

if __name__ == "__main__":
    print("🧪 Gil-Py 설치 테스트")
    print("=" * 30)
    
    success = test_gil_py_installation()
    
    if success:
        print("\\n🎉 Gil-Py가 정상적으로 설치되어 사용할 수 있습니다!")
        print("\\n📖 사용 예제:")
        print("from gil_py import GilConnectorOpenAI, GilGenImage")
        print("# ... 코드 작성 ...")
    else:
        print("\\n💥 Gil-Py 설치에 문제가 있습니다.")
        print("pip install gil-py 명령어로 다시 설치해보세요.")
'''

with open("../test-py/test_installation.py", "w", encoding="utf-8") as f:
    f.write(test_code)

print("✅ 설치 테스트 스크립트 생성: test-py/test_installation.py")
