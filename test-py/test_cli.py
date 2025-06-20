#!/usr/bin/env python3
"""
Gil CLI 테스트 스크립트

Gil CLI 명령어들을 테스트합니다.
"""

import os
import sys
import subprocess
import asyncio
from pathlib import Path

# 환경 변수 로드
from dotenv import load_dotenv
load_dotenv()

class CLITester:
    """CLI 테스트 클래스"""
    
    def __init__(self):
        self.workflows_dir = Path(__file__).parent / "workflows"
        self.gil_path = Path(__file__).parent.parent / "gil-py"
        
    def run_command(self, command: str, timeout: int = 30) -> dict:
        """명령어 실행"""
        print(f"🔧 실행: {command}")
        
        try:
            # Gil 라이브러리 경로를 PYTHONPATH에 추가
            env = os.environ.copy()
            env['PYTHONPATH'] = str(self.gil_path) + os.pathsep + env.get('PYTHONPATH', '')
            
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
                    print(f"  📤 출력: {result.stdout.strip()[:200]}...")
            else:
                print(f"  ❌ 실패 (코드: {result.returncode})")
                if result.stderr.strip():
                    print(f"  ❌ 에러: {result.stderr.strip()[:200]}...")
            
            return {
                "success": result.returncode == 0,
                "stdout": result.stdout,
                "stderr": result.stderr,
                "returncode": result.returncode
            }
            
        except subprocess.TimeoutExpired:
            print(f"  ⏰ 타임아웃 ({timeout}초)")
            return {
                "success": False,
                "error": "timeout",
                "timeout": timeout
            }
        except Exception as e:
            print(f"  ❌ 예외: {e}")
            return {
                "success": False,
                "error": str(e)
            }
    
    def test_cli_commands(self):
        """CLI 명령어 테스트"""
        print("🎯 Gil CLI 테스트 시작")
        print("=" * 50)
        
        # 테스트할 명령어들
        commands = [
            {
                "name": "도움말",
                "command": "python -m gil_py.cli.main --help",
                "should_succeed": True
            },
            {
                "name": "노드 목록",
                "command": "python -m gil_py.cli.main list-nodes",
                "should_succeed": True
            },
            {
                "name": "노드 설명",
                "command": "python -m gil_py.cli.main describe GilGenImage",
                "should_succeed": True
            },
            {
                "name": "워크플로우 검증 - 간단한 이미지 생성",
                "command": f"python -m gil_py.cli.main validate {self.workflows_dir / 'simple_image_gen.yaml'}",
                "should_succeed": True
            },
            {
                "name": "워크플로우 검증 - 데이터 파이프라인",
                "command": f"python -m gil_py.cli.main validate {self.workflows_dir / 'data_pipeline.yaml'}",
                "should_succeed": True
            },
            {
                "name": "워크플로우 실행 - 데이터 파이프라인 (시뮬레이션)",
                "command": f"python -m gil_py.cli.main run {self.workflows_dir / 'data_pipeline.yaml'} --dry-run",
                "should_succeed": True
            }
        ]
        
        # API 키가 있는 경우 실제 실행 테스트 추가
        if os.getenv("OPENAI_API_KEY"):
            commands.append({
                "name": "워크플로우 실행 - 간단한 이미지 생성 (실제)",
                "command": f"python -m gil_py.cli.main run {self.workflows_dir / 'simple_image_gen.yaml'}",
                "should_succeed": True,
                "timeout": 60
            })
        
        results = {}
        
        for cmd_info in commands:
            cmd_name = cmd_info["name"]
            command = cmd_info["command"]
            should_succeed = cmd_info.get("should_succeed", True)
            timeout = cmd_info.get("timeout", 30)
            
            print(f"\n{'='*40}")
            print(f"테스트: {cmd_name}")
            print(f"{'='*40}")
            
            result = self.run_command(command, timeout)
            results[cmd_name] = result
            
            # 예상 결과와 비교
            if should_succeed and result["success"]:
                print("  🎉 예상대로 성공!")
            elif not should_succeed and not result["success"]:
                print("  🎉 예상대로 실패!")
            elif should_succeed and not result["success"]:
                print("  ⚠️  예상과 다름: 성공해야 하는데 실패함")
            else:
                print("  ⚠️  예상과 다름: 실패해야 하는데 성공함")
        
        # 결과 요약
        print(f"\n{'='*50}")
        print("🎯 CLI 테스트 결과 요약")
        print(f"{'='*50}")
        
        success_count = sum(1 for r in results.values() if r["success"])
        total_count = len(results)
        
        print(f"총 테스트: {total_count}")
        print(f"성공: {success_count}")
        print(f"실패: {total_count - success_count}")
        print(f"성공률: {success_count/total_count*100:.1f}%")
        
        for cmd_name, result in results.items():
            status = "✅" if result["success"] else "❌"
            print(f"  {status} {cmd_name}")
        
        return results

def main():
    """메인 함수"""
    tester = CLITester()
    tester.test_cli_commands()

if __name__ == "__main__":
    main()
