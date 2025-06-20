#!/usr/bin/env python3
"""
YAML 워크플로우 테스트 스크립트

Gil의 YAML 기반 워크플로우 기능을 테스트합니다.
"""

import asyncio
import os
import sys
import json
from pathlib import Path
from typing import Dict, Any

# Gil 라이브러리 import 경로 설정
sys.path.insert(0, str(Path(__file__).parent.parent / "gil-py"))

try:
    from gil_py.workflow import GilWorkflow
    from gil_py.workflow.yaml_parser import YamlWorkflowParser
    print("✅ Gil 라이브러리 import 성공")
except ImportError as e:
    print(f"❌ Gil 라이브러리 import 실패: {e}")
    print("gil-py 패키지가 설치되어 있는지 확인하세요.")
    sys.exit(1)

# 환경 변수 로드
from dotenv import load_dotenv
load_dotenv()

class YAMLWorkflowTester:
    """YAML 워크플로우 테스터"""
    
    def __init__(self):
        self.workflows_dir = Path(__file__).parent / "workflows"
        self.results_dir = Path(__file__).parent / "results"
        self.results_dir.mkdir(exist_ok=True)
      async def test_yaml_parsing(self, yaml_file: str) -> Dict[str, Any]:
        """YAML 파일 파싱 테스트"""
        print(f"\n🔍 YAML 파싱 테스트: {yaml_file}")
        
        try:
            yaml_path = self.workflows_dir / yaml_file
            if not yaml_path.exists():
                return {"success": False, "error": f"파일을 찾을 수 없음: {yaml_path}"}
            
            parser = YamlWorkflowParser()
            workflow_config = parser.parse_file(str(yaml_path))
            
            print(f"  ✅ 워크플로우 이름: {workflow_config.name}")
            print(f"  ✅ 노드 개수: {len(workflow_config.nodes)}")
            print(f"  ✅ 출력 개수: {len(workflow_config.outputs)}")
            
            return {
                "success": True,
                "config": workflow_config,
                "stats": {
                    "name": workflow_config.name,
                    "node_count": len(workflow_config.nodes),
                    "output_count": len(workflow_config.outputs)
                }
            }
            
        except Exception as e:
            print(f"  ❌ 파싱 실패: {e}")
            return {"success": False, "error": str(e)}
    
    async def test_workflow_execution(self, yaml_file: str, inputs: Dict[str, Any] = None) -> Dict[str, Any]:
        """워크플로우 실행 테스트"""
        print(f"\n🚀 워크플로우 실행 테스트: {yaml_file}")
        
        try:
            yaml_path = self.workflows_dir / yaml_file
            
            # 워크플로우 로드
            workflow = GilWorkflow.from_yaml(str(yaml_path))
            print(f"  ✅ 워크플로우 로드 완료")
            
            # 실행
            if inputs:
                print(f"  📥 입력 데이터: {inputs}")
            
            result = await workflow.run(inputs or {})
            
            print(f"  ✅ 실행 완료")
            print(f"  📤 출력 키: {list(result.keys())}")
            
            # 결과 저장
            result_file = self.results_dir / f"{yaml_file.replace('.yaml', '_result.json')}"
            with open(result_file, 'w', encoding='utf-8') as f:
                json.dump(result, f, indent=2, ensure_ascii=False, default=str)
            
            print(f"  💾 결과 저장: {result_file}")
            
            return {
                "success": True,
                "result": result,
                "result_file": str(result_file)
            }
            
        except Exception as e:
            print(f"  ❌ 실행 실패: {e}")
            return {"success": False, "error": str(e)}
    
    async def test_workflow_validation(self, yaml_file: str) -> Dict[str, Any]:
        """워크플로우 검증 테스트"""
        print(f"\n✅ 워크플로우 검증 테스트: {yaml_file}")
        
        try:
            yaml_path = self.workflows_dir / yaml_file
              # 기본 파싱 검증
            parser = YamlWorkflowParser()
            config = parser.parse_file(str(yaml_path))
              # 구조 검증
            validation_results = []
            
            # 필수 필드 검증
            if not config.name:
                validation_results.append("⚠️  워크플로우 이름이 없습니다")
            
            if not config.nodes:
                validation_results.append("❌ 노드가 정의되지 않았습니다")
            
            if not config.flow:
                validation_results.append("⚠️  실행 순서(flow)가 정의되지 않았습니다")
            
            # 노드 참조 검증
            for node_name, node_config in config.nodes.items():
                if not node_config.type:
                    validation_results.append(f"❌ 노드 '{node_name}'에 타입이 없습니다")
            
            if not validation_results:
                validation_results.append("✅ 모든 검증 통과")
            
            for result in validation_results:
                print(f"  {result}")
            
            return {
                "success": len([r for r in validation_results if r.startswith("❌")]) == 0,
                "validation_results": validation_results
            }
            
        except Exception as e:
            print(f"  ❌ 검증 실패: {e}")
            return {"success": False, "error": str(e)}
    
    async def run_all_tests(self):
        """모든 테스트 실행"""
        print("🎯 Gil YAML 워크플로우 테스트 시작")
        print("=" * 50)
        
        # 테스트할 워크플로우 파일들
        test_workflows = [
            {"file": "simple_image_gen.yaml", "has_api": True},
            {"file": "data_pipeline.yaml", "has_api": False},
            {"file": "conditional_flow.yaml", "has_api": True, "inputs": {"user_message": "Hello, how are you?"}},
            {"file": "parallel_flow.yaml", "has_api": True, "inputs": {"theme": "space exploration"}},
        ]
        
        api_key = os.getenv("OPENAI_API_KEY")
        
        results = {}
        
        for workflow_info in test_workflows:
            workflow_file = workflow_info["file"]
            has_api = workflow_info.get("has_api", False)
            inputs = workflow_info.get("inputs", {})
            
            print(f"\n{'='*60}")
            print(f"테스트 워크플로우: {workflow_file}")
            print(f"{'='*60}")
            
            # 1. YAML 파싱 테스트
            parse_result = await self.test_yaml_parsing(workflow_file)
            
            # 2. 워크플로우 검증 테스트
            validation_result = await self.test_workflow_validation(workflow_file)
            
            # 3. 실행 테스트 (API 키가 필요한 경우 확인)
            execution_result = {"success": False, "skipped": True}
            
            if has_api and not api_key:
                print(f"\n⏭️  실행 테스트 건너뜀: API 키가 필요하지만 설정되지 않음")
            elif parse_result["success"] and validation_result["success"]:
                execution_result = await self.test_workflow_execution(workflow_file, inputs)
            else:
                print(f"\n⏭️  실행 테스트 건너뜀: 파싱 또는 검증 실패")
            
            results[workflow_file] = {
                "parsing": parse_result,
                "validation": validation_result,
                "execution": execution_result
            }
        
        # 결과 요약
        print(f"\n{'='*60}")
        print("🎯 테스트 결과 요약")
        print(f"{'='*60}")
        
        for workflow_file, test_results in results.items():
            print(f"\n📄 {workflow_file}:")
            print(f"  파싱: {'✅' if test_results['parsing']['success'] else '❌'}")
            print(f"  검증: {'✅' if test_results['validation']['success'] else '❌'}")
            
            if test_results['execution'].get('skipped'):
                print(f"  실행: ⏭️  건너뜀")
            else:
                print(f"  실행: {'✅' if test_results['execution']['success'] else '❌'}")
        
        # 전체 결과 저장
        summary_file = self.results_dir / "test_summary.json"
        with open(summary_file, 'w', encoding='utf-8') as f:
            json.dump(results, f, indent=2, ensure_ascii=False, default=str)
        
        print(f"\n💾 테스트 요약 저장: {summary_file}")
        print("\n🎉 테스트 완료!")

async def main():
    """메인 함수"""
    tester = YAMLWorkflowTester()
    await tester.run_all_tests()

if __name__ == "__main__":
    asyncio.run(main())
