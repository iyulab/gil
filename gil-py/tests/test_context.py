#!/usr/bin/env python3
"""
Gil 워크플로우 컨텍스트 테스트 실행기

컨텍스트 시스템을 완전히 지원하는 테스트 실행기

사용법:
    py test_context.py context-test.yaml
    py test_context.py smart-content-generator.yaml
"""

import os
import sys
import asyncio
import json
import yaml
from pathlib import Path
from datetime import datetime
from typing import Dict, Any

# gil-py 모듈 경로 추가
sys.path.insert(0, str(Path(__file__).parent.parent / "gil-py"))

def load_env():
    """환경 변수 로드"""
    env_path = Path(__file__).parent / ".env"
    if env_path.exists():
        try:
            from dotenv import load_dotenv
            load_dotenv(env_path)
        except ImportError:
            # dotenv가 없으면 수동으로 로드
            with open(env_path, 'r') as f:
                for line in f:
                    if '=' in line and not line.strip().startswith('#'):
                        key, value = line.strip().split('=', 1)
                        os.environ[key] = value

def load_yaml_with_context(filepath: Path) -> Dict[str, Any]:
    """컨텍스트 변수 치환을 지원하는 YAML 로더"""
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # 간단한 환경 변수 치환
    import re
    
    # ${secrets.KEY} 패턴을 환경 변수로 치환
    def replace_secrets(match):
        key = match.group(1)
        return os.getenv(key, f"${{secrets.{key}}}")
    
    content = re.sub(r'\$\{secrets\.([^}]+)\}', replace_secrets, content)
    
    return yaml.safe_load(content)

async def run_workflow_with_context(yaml_file: Path) -> bool:
    """컨텍스트를 지원하는 워크플로우 실행"""
    try:
        from gil_py.core.context import FlowContext, NodeContext
        from gil_py.connectors.openai_connector import GilConnectorOpenAI
        
        print(f"🚀 워크플로우 실행 시작: {yaml_file.name}")
        print(f"📂 파일 경로: {yaml_file}")
        print("=" * 60)
        
        # YAML 로드
        workflow_data = load_yaml_with_context(yaml_file)
        
        # Flow Context 초기화
        flow_context = FlowContext(workflow_id=workflow_data.get("name", "test_workflow"))
        
        # YAML에서 정의된 초기 컨텍스트 설정
        if "context" in workflow_data:
            context_config = workflow_data["context"]
            
            # 초기 변수 설정
            if "variables" in context_config:
                for key, value in context_config["variables"].items():
                    flow_context.set_variable(key, value)
                print(f"📝 Flow 변수 설정 완료: {list(context_config['variables'].keys())}")
            
            # 초기 메타데이터 설정
            if "metadata" in context_config:
                for key, value in context_config["metadata"].items():
                    flow_context.update_metadata(key, value)
                print(f"📊 Flow 메타데이터 설정 완료: {list(context_config['metadata'].keys())}")
        
        # 노드 실행 (단순화된 버전)
        nodes = workflow_data.get("nodes", {})
        flow_order = workflow_data.get("flow", [])
        
        node_results = {}
        
        print(f"\n🔄 노드 실행 시작 (총 {len(flow_order)}개 노드)")
        print("-" * 40)
        
        for node_name in flow_order:
            if node_name not in nodes:
                error_msg = f"노드 '{node_name}'를 찾을 수 없습니다"
                flow_context.add_error(node_name, error_msg, "node_not_found")
                print(f"❌ {error_msg}")
                continue
            
            node_config = nodes[node_name]
            node_type = node_config.get("type")
            
            print(f"⚙️ 실행 중: {node_name} ({node_type})")
            
            # Node Context 생성
            node_context = NodeContext(node_name, flow_context)
            
            try:
                # 노드별 실행 시뮬레이션
                if node_type == "GilConnectorOpenAI":
                    # OpenAI 커넥터 실행
                    connector = GilConnectorOpenAI(
                        api_key=os.getenv("OPENAI_API_KEY", ""),
                        name=node_name
                    )
                    connector.set_contexts(node_context, flow_context)
                    
                    # 연결 테스트만 수행
                    result = await connector.execute({})
                    node_results[node_name] = result
                    print("   ✅ OpenAI 커넥터 연결 확인 완료")
                
                elif node_type in ["GilGenImage", "AITextGeneration"]:
                    # AI 생성 노드 시뮬레이션
                    node_context.set_variable("api_calls", 1)
                    node_context.set_variable("tokens_used", 150)  # 시뮬레이션
                    
                    # Flow 컨텍스트에 토큰 사용량 누적
                    total_tokens = flow_context.get_shared_data("total_tokens_used", 0)
                    flow_context.set_shared_data("total_tokens_used", total_tokens + 150)
                    
                    result = {
                        "success": True,
                        "generated_text": f"[시뮬레이션] {node_name}에서 생성된 콘텐츠",
                        "metadata": {
                            "tokens_used": 150,
                            "execution_time": 2.5
                        }
                    }
                    node_results[node_name] = result
                    print("   ✅ AI 생성 완료 (시뮬레이션)")
                
                elif node_type == "TransformData":
                    # 데이터 변환 노드 시뮬레이션
                    operations = node_config.get("inputs", {}).get("operations", [])
                    
                    result = {
                        "success": True,
                        "transformed_data": {
                            "processed": True,
                            "operations_count": len(operations),
                            "result": "변환 완료"
                        }
                    }
                    node_results[node_name] = result
                    print(f"   ✅ 데이터 변환 완료 ({len(operations)}개 연산)")
                
                else:
                    # 기타 노드 시뮬레이션
                    result = {
                        "success": True,
                        "message": f"{node_type} 노드 실행 완료 (시뮬레이션)"
                    }
                    node_results[node_name] = result
                    print(f"   ✅ {node_type} 실행 완료 (시뮬레이션)")
                
                # 완료된 노드 수 증가
                flow_context.increment_completed_nodes()
                
            except Exception as e:
                error_msg = f"노드 실행 중 오류: {str(e)}"
                flow_context.add_error(node_name, error_msg, "execution_error")
                node_context.add_error(error_msg, "execution_error")
                print(f"   ❌ 오류: {error_msg}")
                
                node_results[node_name] = {
                    "success": False,
                    "error": error_msg
                }
        
        # 실행 결과 요약
        print("\n" + "=" * 60)
        print("📊 실행 결과 요약")
        print("-" * 40)
        
        # Flow Context 정보 출력
        flow_dict = flow_context.to_dict()
        print(f"✅ 완료된 노드: {flow_dict['metadata']['completed_nodes']}")
        print(f"❌ 발생한 에러: {len(flow_dict['errors'])}")
        print(f"🎯 총 토큰 사용량: {flow_dict['shared_data'].get('total_tokens_used', 0)}")
        
        # Flow 변수 출력
        if flow_dict['variables']:
            print("📝 Flow 변수:")
            for key, value in flow_dict['variables'].items():
                print(f"   - {key}: {value}")
        
        # 공유 데이터 출력
        if flow_dict['shared_data']:
            print("📊 공유 데이터:")
            for key, value in flow_dict['shared_data'].items():
                print(f"   - {key}: {value}")
        
        # 에러 정보 출력
        if flow_dict['errors']:
            print("⚠️ 발생한 에러들:")
            for error in flow_dict['errors']:
                print(f"   - [{error['node']}] {error['message']}")
        
        # 결과 저장
        output_dir = Path(__file__).parent / "context_results"
        output_dir.mkdir(exist_ok=True)
        
        result_file = output_dir / f"{yaml_file.stem}_result.json"
        with open(result_file, 'w', encoding='utf-8') as f:
            json.dump({
                "workflow_name": workflow_data.get("name"),
                "execution_time": datetime.now().isoformat(),
                "flow_context": flow_dict,
                "node_results": node_results
            }, f, indent=2, ensure_ascii=False)
        
        print(f"\n💾 결과 저장: {result_file}")
        
        # 성공 여부 반환
        return len(flow_dict['errors']) == 0
        
    except ImportError as e:
        print(f"❌ Gil-Py 모듈을 찾을 수 없습니다: {e}")
        return False
    except Exception as e:
        print(f"❌ 실행 중 오류 발생: {e}")
        import traceback
        traceback.print_exc()
        return False

async def main():
    """메인 함수"""
    if len(sys.argv) < 2:
        print("사용법: py test_context.py <workflow.yaml>")
        print("\n사용 가능한 워크플로우:")
        print("  - context-test.yaml")
        print("  - smart-content-generator.yaml")
        return
    
    # 환경 변수 로드
    load_env()
    
    # YAML 파일 경로
    yaml_file = Path(__file__).parent / sys.argv[1]
    
    if not yaml_file.exists():
        print(f"❌ 파일을 찾을 수 없습니다: {yaml_file}")
        return
    
    # 워크플로우 실행
    success = await run_workflow_with_context(yaml_file)
    
    if success:
        print("\n🎉 워크플로우 실행 성공!")
    else:
        print("\n💥 워크플로우 실행 실패!")
        sys.exit(1)

if __name__ == "__main__":
    asyncio.run(main())
