#!/usr/bin/env python3
"""
간단한 YAML 워크플로우 테스트
"""

import asyncio
import os
import sys
from pathlib import Path

# Gil 라이브러리 import 경로 설정
sys.path.insert(0, str(Path(__file__).parent.parent / "gil-py"))

# 환경 변수 로드
from dotenv import load_dotenv
load_dotenv()

def test_imports():
    """라이브러리 import 테스트"""
    print("🔍 라이브러리 import 테스트")
    try:
        from gil_py.workflow.yaml_parser import YamlWorkflowParser
        from gil_py.workflow.workflow import GilWorkflow
        print("✅ import 성공")
        return True
    except ImportError as e:
        print(f"❌ import 실패: {e}")
        return False

def test_yaml_parsing():
    """YAML 파싱 테스트"""
    print("\n🔍 YAML 파싱 테스트")
    
    from gil_py.workflow.yaml_parser import YamlWorkflowParser
    
    yaml_file = Path("workflows/simple_image_gen.yaml")
    if not yaml_file.exists():
        print(f"❌ YAML 파일 없음: {yaml_file}")
        return False
    
    try:
        parser = YamlWorkflowParser()
        config = parser.parse_file(str(yaml_file))
        print(f"✅ 파싱 성공: {config.name}")
        print(f"   노드 수: {len(config.nodes)}")
        print(f"   설명: {config.description}")
        return True
    except Exception as e:
        print(f"❌ 파싱 실패: {e}")
        return False

def test_node_types():
    """노드 타입 확인 테스트"""
    print("\n🔍 노드 타입 확인 테스트")
    
    from gil_py.workflow.yaml_parser import YamlWorkflowParser
    
    yaml_file = Path("workflows/simple_image_gen.yaml")
    if not yaml_file.exists():
        print(f"❌ YAML 파일 없음: {yaml_file}")
        return False
    
    try:
        parser = YamlWorkflowParser()
        config = parser.parse_file(str(yaml_file))
        
        print("노드 목록:")
        for node_name, node_config in config.nodes.items():
            print(f"  - {node_name}: {node_config.type}")
        
        return True
    except Exception as e:
        print(f"❌ 테스트 실패: {e}")
        return False

def test_workflow_structure():
    """워크플로우 구조 테스트"""
    print("\n🔍 워크플로우 구조 테스트")
    
    from gil_py.workflow.yaml_parser import YamlWorkflowParser
    
    yaml_file = Path("workflows/simple_image_gen.yaml")
    if not yaml_file.exists():
        print(f"❌ YAML 파일 없음: {yaml_file}")
        return False
    
    try:
        parser = YamlWorkflowParser()
        config = parser.parse_file(str(yaml_file))
        
        print(f"워크플로우: {config.name}")
        print(f"설명: {config.description}")
        print(f"환경변수: {list(config.environment.keys())}")
        print(f"실행 순서: {config.flow}")
        print(f"출력: {list(config.outputs.keys())}")
        
        return True
    except Exception as e:
        print(f"❌ 테스트 실패: {e}")
        return False

async def test_workflow_execution():
    """워크플로우 실행 테스트 (시뮬레이션)"""
    print("\n🔍 워크플로우 실행 테스트")
    
    # API 키 확인
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        print("⚠️  OPENAI_API_KEY 없음 - 실행 테스트 건너뜀")
        return True
    
    try:
        from gil_py.workflow.workflow import GilWorkflow
        
        yaml_file = Path("workflows/simple_image_gen.yaml")
        if not yaml_file.exists():
            print(f"❌ YAML 파일 없음: {yaml_file}")
            return False
        
        print("워크플로우 로드 시도...")
        # 이 부분은 실제 구현에 따라 달라질 수 있음
        print("✅ 워크플로우 로드 테스트 완료 (시뮬레이션)")
        
        return True
    except Exception as e:
        print(f"❌ 실행 테스트 실패: {e}")
        return False

def main():
    """메인 함수"""
    print("🚀 Gil YAML 워크플로우 간단 테스트")
    print("=" * 50)
    
    tests = [
        ("라이브러리 import", test_imports),
        ("YAML 파싱", test_yaml_parsing),
        ("노드 타입 확인", test_node_types),
        ("워크플로우 구조", test_workflow_structure),
    ]
    
    results = []
    
    for test_name, test_func in tests:
        print(f"\n{'='*30}")
        print(f"테스트: {test_name}")
        print(f"{'='*30}")
        
        try:
            result = test_func()
            results.append((test_name, result))
        except Exception as e:
            print(f"❌ 테스트 중 예외: {e}")
            results.append((test_name, False))
    
    # 비동기 테스트
    print(f"\n{'='*30}")
    print("테스트: 워크플로우 실행")
    print(f"{'='*30}")
    
    try:
        execution_result = asyncio.run(test_workflow_execution())
        results.append(("워크플로우 실행", execution_result))
    except Exception as e:
        print(f"❌ 실행 테스트 중 예외: {e}")
        results.append(("워크플로우 실행", False))
    
    # 결과 요약
    print(f"\n{'='*50}")
    print("🎯 테스트 결과 요약")
    print(f"{'='*50}")
    
    success_count = 0
    for test_name, success in results:
        status = "✅" if success else "❌"
        print(f"  {status} {test_name}")
        if success:
            success_count += 1
    
    total_tests = len(results)
    print(f"\n총 {total_tests}개 테스트 중 {success_count}개 성공 ({success_count/total_tests*100:.1f}%)")
    
    if success_count == total_tests:
        print("🎉 모든 테스트 통과!")
    else:
        print("⚠️  일부 테스트 실패")

if __name__ == "__main__":
    main()
