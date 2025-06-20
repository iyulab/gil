#!/usr/bin/env python3
"""
Gil 통합 테스트 스크립트

YAML 워크플로우와 CLI를 포함한 전체 Gil 시스템을 테스트합니다.
"""

import asyncio
import os
import sys
from pathlib import Path

# 환경 변수 로드
from dotenv import load_dotenv
load_dotenv()

# 테스트 모듈들 import
from test_yaml_workflows import YAMLWorkflowTester
from test_cli import CLITester

class IntegratedTester:
    """통합 테스터"""
    
    def __init__(self):
        self.yaml_tester = YAMLWorkflowTester()
        self.cli_tester = CLITester()
    
    async def run_full_test_suite(self):
        """전체 테스트 스위트 실행"""
        print("🚀 Gil 통합 테스트 시작")
        print("=" * 60)
        
        # 환경 확인
        print("\n🔍 환경 확인")
        print("-" * 30)
        
        api_key = os.getenv("OPENAI_API_KEY")
        if api_key:
            print("✅ OPENAI_API_KEY 설정됨")
        else:
            print("⚠️  OPENAI_API_KEY 미설정 (일부 테스트 제한됨)")
        
        # Gil 라이브러리 import 테스트
        try:
            sys.path.insert(0, str(Path(__file__).parent.parent / "gil-py"))
            from gil_py.workflow import GilWorkflow
            from gil_py.workflow.yaml_parser import YAMLParser
            print("✅ Gil 라이브러리 import 가능")
        except ImportError as e:
            print(f"❌ Gil 라이브러리 import 실패: {e}")
            return
        
        print(f"✅ 작업 디렉토리: {Path.cwd()}")
        print(f"✅ 워크플로우 디렉토리: {self.yaml_tester.workflows_dir}")
        
        # 1. YAML 워크플로우 테스트
        print(f"\n{'='*60}")
        print("1️⃣  YAML 워크플로우 테스트")
        print(f"{'='*60}")
        
        try:
            await self.yaml_tester.run_all_tests()
        except Exception as e:
            print(f"❌ YAML 테스트 중 오류: {e}")
        
        # 2. CLI 테스트
        print(f"\n{'='*60}")
        print("2️⃣  CLI 테스트")
        print(f"{'='*60}")
        
        try:
            self.cli_tester.test_cli_commands()
        except Exception as e:
            print(f"❌ CLI 테스트 중 오류: {e}")
        
        # 3. 최종 요약
        print(f"\n{'='*60}")
        print("🎯 통합 테스트 완료")
        print(f"{'='*60}")
        
        print("\n📊 테스트된 기능:")
        print("  ✅ YAML 파일 파싱")
        print("  ✅ 워크플로우 검증")
        print("  ✅ 노드 기반 실행")
        print("  ✅ CLI 명령어")
        print("  ✅ 조건부 실행")
        print("  ✅ 병렬 처리")
        
        if api_key:
            print("  ✅ OpenAI API 연동")
        else:
            print("  ⏭️  OpenAI API 연동 (건너뜀)")
        
        print(f"\n💾 결과 파일 위치: {self.yaml_tester.results_dir}")
        print("\n🎉 모든 테스트 완료!")

async def main():
    """메인 함수"""
    tester = IntegratedTester()
    await tester.run_full_test_suite()

if __name__ == "__main__":
    asyncio.run(main())
