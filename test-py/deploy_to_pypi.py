"""
Gil-Py PyPI 배포 스크립트
"""

import subprocess
import sys
import os
from pathlib import Path

def run_command(cmd, cwd=None):
    """명령어 실행 헬퍼"""
    print(f"🔧 실행: {cmd}")
    result = subprocess.run(cmd, shell=True, cwd=cwd, capture_output=True, text=True)
    
    if result.returncode == 0:
        print(f"✅ 성공")
        if result.stdout:
            print(result.stdout)
    else:
        print(f"❌ 실패: {result.stderr}")
        return False
    return True

def setup_and_deploy():
    """PyPI 배포 전체 과정"""
    print("🚀 Gil-Py PyPI 배포 시작...")
    
    # 현재 디렉토리를 gil-py로 변경
    gil_py_path = Path("../gil-py").resolve()
    print(f"📁 작업 디렉토리: {gil_py_path}")
    
    if not gil_py_path.exists():
        print("❌ gil-py 디렉토리를 찾을 수 없습니다.")
        return False
    
    # 1. 필요한 도구 설치
    print("\n📦 배포 도구 설치...")
    tools = ["build", "twine", "setuptools", "wheel"]
    for tool in tools:
        if not run_command(f"py -m pip install --upgrade {tool}"):
            return False
    
    # 2. 기존 dist 폴더 정리
    print("\n🧹 기존 빌드 파일 정리...")
    dist_path = gil_py_path / "dist"
    if dist_path.exists():
        run_command("rmdir /s /q dist", gil_py_path)
    
    # 3. 패키지 빌드
    print("\n🔨 패키지 빌드...")
    if not run_command("py -m build", gil_py_path):
        return False
    
    # 4. 빌드 결과 확인
    print("\n🔍 빌드 결과 확인...")
    if dist_path.exists():
        files = list(dist_path.glob("*"))
        print(f"✅ 빌드 완료! 생성된 파일: {len(files)}개")
        for file in files:
            print(f"   📄 {file.name}")
    else:
        print("❌ dist 폴더가 생성되지 않았습니다.")
        return False
    
    # 5. 패키지 검증
    print("\n🧪 패키지 검증...")
    if not run_command("py -m twine check dist/*", gil_py_path):
        return False
    
    # 6. TestPyPI에 업로드 (선택사항)
    print("\n🧪 TestPyPI 업로드를 진행하시겠습니까? (권장)")
    choice = input("TestPyPI에 먼저 업로드하시겠습니까? (y/n): ").lower()
    
    if choice == 'y':
        print("📤 TestPyPI 업로드...")
        print("⚠️ PyPI 계정 정보를 입력해야 합니다.")
        if not run_command("py -m twine upload --repository testpypi dist/*", gil_py_path):
            print("❌ TestPyPI 업로드 실패. 계정 정보를 확인해주세요.")
            return False
        
        print("✅ TestPyPI 업로드 성공!")
        print("🧪 TestPyPI에서 설치 테스트:")
        print("   pip install -i https://test.pypi.org/simple/ gil-py")
        
        test_choice = input("\nTestPyPI에서 테스트를 완료한 후 실제 PyPI에 업로드하시겠습니까? (y/n): ").lower()
        if test_choice != 'y':
            print("🛑 실제 PyPI 업로드를 취소했습니다.")
            return True
    
    # 7. 실제 PyPI에 업로드
    print("\n🚀 실제 PyPI 업로드...")
    print("⚠️ 주의: 한 번 업로드된 버전은 삭제할 수 없습니다!")
    
    final_choice = input("정말로 PyPI에 업로드하시겠습니까? (y/n): ").lower()
    if final_choice != 'y':
        print("🛑 PyPI 업로드를 취소했습니다.")
        return True
    
    if not run_command("py -m twine upload dist/*", gil_py_path):
        print("❌ PyPI 업로드 실패.")
        return False
    
    print("🎉 PyPI 업로드 성공!")
    print_success_message()
    return True

def print_success_message():
    """성공 메시지 출력"""
    print("\n" + "="*60)
    print("🎉 Gil-Py PyPI 배포 완료!")
    print("="*60)
    print("✅ 이제 전 세계 누구나 다음 명령어로 설치할 수 있습니다:")
    print("   pip install gil-py")
    print("\n📖 사용 예제:")
    print("   from gil_py import GilConnectorOpenAI, GilGenImage")
    print("   # ... 코드 작성 ...")
    print("\n🔗 링크:")
    print("   PyPI: https://pypi.org/project/gil-py/")
    print("   설치 통계: https://pypistats.org/packages/gil-py")
    print("="*60)

def main():
    """메인 함수"""
    try:
        success = setup_and_deploy()
        if not success:
            print("\n💥 배포 과정에서 오류가 발생했습니다.")
            sys.exit(1)
    except KeyboardInterrupt:
        print("\n🛑 사용자에 의해 중단되었습니다.")
        sys.exit(1)
    except Exception as e:
        print(f"\n💥 예상치 못한 오류: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
