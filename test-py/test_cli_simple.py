#!/usr/bin/env python3
"""
간단한 CLI 테스트
"""

import subprocess
import sys
from pathlib import Path

def run_command(command, timeout=30):
    """명령어 실행"""
    print(f"🔧 실행: {command}")
    
    try:
        # Gil 라이브러리 경로를 PYTHONPATH에 추가
        import os
        env = os.environ.copy()
        gil_path = str(Path(__file__).parent.parent / "gil-py")
        env['PYTHONPATH'] = gil_path + os.pathsep + env.get('PYTHONPATH', '')
        
        result = subprocess.run(
            command,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout,
            env=env,
            cwd=str(Path(__file__).parent)
        )
        
        if result.returncode == 0:
            print(f"  ✅ 성공")
            if result.stdout.strip():
                # 출력이 너무 길면 줄임
                output = result.stdout.strip()
                if len(output) > 200:
                    output = output[:200] + "..."
                print(f"  📤 출력: {output}")
        else:
            print(f"  ❌ 실패 (코드: {result.returncode})")
            if result.stderr.strip():
                error = result.stderr.strip()
                if len(error) > 200:
                    error = error[:200] + "..."
                print(f"  ❌ 에러: {error}")
        
        return result.returncode == 0, result.stdout, result.stderr
        
    except subprocess.TimeoutExpired:
        print(f"  ⏰ 타임아웃 ({timeout}초)")
        return False, "", "timeout"
    except Exception as e:
        print(f"  ❌ 예외: {e}")
        return False, "", str(e)

def test_cli_basic():
    """기본 CLI 테스트"""
    print("🔍 기본 CLI 기능 테스트")
    
    commands = [
        "py -m gil_py.cli.main --help",
        "py -m gil_py.cli.main list-nodes",
        "py -m gil_py.cli.main describe GilGenImage",
    ]
    
    results = []
    
    for cmd in commands:
        success, stdout, stderr = run_command(cmd)
        results.append((cmd, success))
        
        if not success:
            print(f"    ⚠️  명령 실패: {cmd}")
    
    return results

def test_cli_validate():
    """CLI 검증 테스트"""
    print("\n🔍 CLI 검증 기능 테스트")
    
    # workflows 디렉토리의 YAML 파일들 검증
    workflows_dir = Path("workflows")
    yaml_files = list(workflows_dir.glob("*.yaml"))
    
    if not yaml_files:
        print("⚠️  테스트할 YAML 파일이 없습니다")
        return []
    
    results = []
    
    for yaml_file in yaml_files:
        cmd = f"py -m gil_py.cli.main validate {yaml_file}"
        success, stdout, stderr = run_command(cmd)
        results.append((f"validate {yaml_file.name}", success))
    
    return results

def test_cli_run_dry():
    """CLI 드라이런 테스트"""
    print("\n🔍 CLI 드라이런 테스트")
    
    # 간단한 데이터 파이프라인을 드라이런으로 실행
    yaml_file = Path("workflows/data_pipeline.yaml")
    
    if not yaml_file.exists():
        print(f"⚠️  테스트 파일 없음: {yaml_file}")
        return []
    
    cmd = f"py -m gil_py.cli.main run {yaml_file} --dry-run"
    success, stdout, stderr = run_command(cmd, timeout=60)
    
    return [("run --dry-run", success)]

def main():
    """메인 함수"""
    print("🚀 Gil CLI 간단 테스트")
    print("=" * 50)
    
    all_results = []
    
    # 기본 CLI 테스트
    print(f"\n{'='*30}")
    print("기본 CLI 기능")
    print(f"{'='*30}")
    basic_results = test_cli_basic()
    all_results.extend(basic_results)
    
    # 검증 테스트
    print(f"\n{'='*30}")
    print("CLI 검증 기능")
    print(f"{'='*30}")
    validate_results = test_cli_validate()
    all_results.extend(validate_results)
    
    # 드라이런 테스트
    print(f"\n{'='*30}")
    print("CLI 실행 기능")
    print(f"{'='*30}")
    run_results = test_cli_run_dry()
    all_results.extend(run_results)
    
    # 결과 요약
    print(f"\n{'='*50}")
    print("🎯 CLI 테스트 결과 요약")
    print(f"{'='*50}")
    
    success_count = 0
    for test_name, success in all_results:
        status = "✅" if success else "❌"
        print(f"  {status} {test_name}")
        if success:
            success_count += 1
    
    total_tests = len(all_results)
    if total_tests > 0:
        print(f"\n총 {total_tests}개 테스트 중 {success_count}개 성공 ({success_count/total_tests*100:.1f}%)")
        
        if success_count == total_tests:
            print("🎉 모든 CLI 테스트 통과!")
        elif success_count > total_tests * 0.7:
            print("👍 대부분의 CLI 테스트 통과!")
        else:
            print("⚠️  CLI 테스트 일부 실패")
    else:
        print("❌ 실행된 테스트가 없습니다")

if __name__ == "__main__":
    main()
