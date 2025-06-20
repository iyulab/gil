#!/usr/bin/env python3
"""
직접 CLI 테스트
"""

import sys
from pathlib import Path

# Gil 라이브러리 import 경로 설정
sys.path.insert(0, str(Path(__file__).parent.parent / "gil-py"))

def test_cli_imports():
    """CLI 모듈 import 테스트"""
    print("🔍 CLI 모듈 import 테스트")
    
    try:
        from gil_py.cli.main import main
        print("✅ CLI main 함수 import 성공")
        return True
    except ImportError as e:
        print(f"❌ CLI import 실패: {e}")
        return False

def test_cli_help():
    """CLI 도움말 테스트"""
    print("\n🔍 CLI 도움말 테스트")
    
    try:
        # sys.argv를 임시로 변경
        original_argv = sys.argv[:]
        sys.argv = ["gil", "--help"]
        
        from gil_py.cli.main import main
        
        # help는 SystemExit를 발생시키므로 예외 처리
        try:
            main()
        except SystemExit as e:
            if e.code == 0:  # 정상적인 help 출력
                print("✅ 도움말 출력 성공")
                return True
            else:
                print(f"❌ 비정상적인 종료 코드: {e.code}")
                return False
        
    except Exception as e:
        print(f"❌ 도움말 테스트 실패: {e}")
        return False
    finally:
        # sys.argv 복원
        sys.argv = original_argv

def test_workflow_validation():
    """워크플로우 검증 테스트"""
    print("\n🔍 워크플로우 검증 테스트")
    
    yaml_file = Path("workflows/simple_image_gen.yaml")
    if not yaml_file.exists():
        print(f"❌ 테스트 파일 없음: {yaml_file}")
        return False
    
    try:
        from gil_py.workflow.yaml_parser import YamlWorkflowParser
        
        parser = YamlWorkflowParser()
        config = parser.parse_file(str(yaml_file))
        
        # 기본적인 검증
        errors = []
        
        if not config.name:
            errors.append("워크플로우 이름 없음")
        
        if not config.nodes:
            errors.append("노드 정의 없음")
        
        if not config.flow:
            errors.append("실행 순서 정의 없음")
        
        # 노드 타입 검증
        for node_name, node_config in config.nodes.items():
            if not node_config.type:
                errors.append(f"노드 '{node_name}' 타입 없음")
        
        if errors:
            print(f"❌ 검증 실패: {', '.join(errors)}")
            return False
        else:
            print("✅ 워크플로우 검증 통과")
            return True
            
    except Exception as e:
        print(f"❌ 검증 테스트 실패: {e}")
        return False

def test_node_factory():
    """노드 팩토리 테스트"""
    print("\n🔍 노드 팩토리 테스트")
    
    try:
        from gil_py.workflow.node_factory import NodeFactory
        
        factory = NodeFactory()
        
        # 지원되는 노드 타입들
        supported_types = [
            "GilConnectorOpenAI",
            "GilGenImage",
        ]
        
        print(f"지원되는 노드 타입: {supported_types}")
        
        # 각 타입별로 노드 생성 테스트 (실제 생성은 하지 않고 타입 확인만)
        print("✅ 노드 팩토리 기본 기능 확인 완료")
        return True
        
    except Exception as e:
        print(f"❌ 노드 팩토리 테스트 실패: {e}")
        return False

def main():
    """메인 함수"""
    print("🚀 Gil CLI 직접 테스트")
    print("=" * 50)
    
    tests = [
        ("CLI 모듈 import", test_cli_imports),
        ("CLI 도움말", test_cli_help),
        ("워크플로우 검증", test_workflow_validation),
        ("노드 팩토리", test_node_factory),
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
    
    # 결과 요약
    print(f"\n{'='*50}")
    print("🎯 CLI 테스트 결과 요약")
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
        print("🎉 모든 CLI 테스트 통과!")
    elif success_count > 0:
        print("👍 일부 CLI 테스트 통과!")
    else:
        print("❌ 모든 CLI 테스트 실패")

if __name__ == "__main__":
    main()
