"""
Gil CLI 메인 모듈
"""

import argparse
import asyncio
import sys
import json
from pathlib import Path
from typing import Dict, Any

# 환경 변수 로드
from dotenv import load_dotenv
load_dotenv()

from ..workflow import GilWorkflow
from ..workflow.node_factory import NodeFactory


def main():
    """CLI 메인 함수"""
    parser = argparse.ArgumentParser(
        description="Gil - AI Workflow Engine",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
예시:
  gil run workflow.yaml                    # 워크플로우 실행
  gil run workflow.yaml --input theme=ai  # 입력과 함께 실행
  gil validate workflow.yaml              # 워크플로우 검증
  gil list-nodes                          # 사용 가능한 노드 목록
  gil describe GilGenImage                # 노드 상세 정보
        """
    )
    
    parser.add_argument("--version", action="version", version="gil-py 0.1.0")
    
    subparsers = parser.add_subparsers(dest="command", help="사용 가능한 명령어")
    
    # run 명령어
    run_parser = subparsers.add_parser("run", help="워크플로우 실행")
    run_parser.add_argument("workflow", help="워크플로우 YAML 파일")
    run_parser.add_argument("--input", action="append", help="입력 파라미터 (key=value 형태)")
    run_parser.add_argument("--nodes", help="실행할 노드들 (쉼표로 구분)")
    run_parser.add_argument("--debug", action="store_true", help="디버그 모드")
    run_parser.add_argument("--output", help="결과 저장할 JSON 파일")
    
    # validate 명령어
    validate_parser = subparsers.add_parser("validate", help="워크플로우 검증")
    validate_parser.add_argument("workflow", help="워크플로우 YAML 파일")
    
    # visualize 명령어
    visualize_parser = subparsers.add_parser("visualize", help="워크플로우 시각화")
    visualize_parser.add_argument("workflow", help="워크플로우 YAML 파일")
    visualize_parser.add_argument("--output", default="workflow_diagram.md", help="출력 파일")
    
    # list-nodes 명령어
    list_parser = subparsers.add_parser("list-nodes", help="사용 가능한 노드 타입 목록")
    
    # describe 명령어
    describe_parser = subparsers.add_parser("describe", help="노드 타입 상세 정보")
    describe_parser.add_argument("node_type", help="노드 타입 이름")
    
    # generate 명령어 (빠른 이미지 생성)
    generate_parser = subparsers.add_parser("generate", help="빠른 콘텐츠 생성")
    generate_parser.add_argument("--image", help="이미지 생성 프롬프트")
    generate_parser.add_argument("--size", default="1024x1024", help="이미지 크기")
    generate_parser.add_argument("--api-key", help="OpenAI API 키")
    
    args = parser.parse_args()
    
    if args.command == "run":
        asyncio.run(handle_run(args))
    elif args.command == "validate":
        handle_validate(args)
    elif args.command == "visualize":
        handle_visualize(args)
    elif args.command == "list-nodes":
        handle_list_nodes()
    elif args.command == "describe":
        handle_describe(args)
    elif args.command == "generate":
        asyncio.run(handle_generate(args))
    else:
        parser.print_help()


async def handle_run(args):
    """워크플로우 실행 처리"""
    workflow_path = Path(args.workflow)
    
    if not workflow_path.exists():
        print(f"❌ 워크플로우 파일을 찾을 수 없습니다: {workflow_path}")
        sys.exit(1)
    
    try:
        # 워크플로우 로드
        print(f"📋 워크플로우 로드 중: {workflow_path}")
        workflow = GilWorkflow.from_yaml(workflow_path)
        
        # 입력 파라미터 파싱
        inputs = {}
        if args.input:
            for input_item in args.input:
                if "=" in input_item:
                    key, value = input_item.split("=", 1)
                    inputs[key.strip()] = value.strip()
        
        if args.debug:
            print(f"🔍 디버그 모드 활성화")
            print(f"📥 입력 파라미터: {inputs}")
            print(f"🧩 노드 수: {len(workflow.nodes)}")
            
            # 워크플로우 검증
            validation = workflow.validate()
            if not validation["valid"]:
                print("⚠️ 워크플로우 검증 실패:")
                for error in validation["errors"]:
                    print(f"   - {error}")
                return
        
        # 워크플로우 실행
        print(f"🚀 워크플로우 실행 시작...")
        result = await workflow.run(inputs)
        
        # 결과 출력
        print(f"✅ 워크플로우 실행 완료!")
        
        if args.output:
            # JSON 파일로 저장
            with open(args.output, 'w', encoding='utf-8') as f:
                json.dump(result, f, indent=2, ensure_ascii=False, default=str)
            print(f"💾 결과 저장됨: {args.output}")
        else:
            # 콘솔에 출력
            print(f"📊 실행 결과:")
            for node_name, node_result in result.items():
                print(f"   🔹 {node_name}:")
                if isinstance(node_result, dict):
                    for key, value in node_result.items():
                        if isinstance(value, str) and len(value) > 100:
                            print(f"      {key}: {value[:100]}...")
                        else:
                            print(f"      {key}: {value}")
                else:
                    print(f"      {node_result}")
    
    except Exception as e:
        print(f"❌ 오류 발생: {e}")
        if args.debug:
            import traceback
            traceback.print_exc()
        sys.exit(1)


def handle_validate(args):
    """워크플로우 검증 처리"""
    workflow_path = Path(args.workflow)
    
    if not workflow_path.exists():
        print(f"❌ 워크플로우 파일을 찾을 수 없습니다: {workflow_path}")
        sys.exit(1)
    
    try:
        workflow = GilWorkflow.from_yaml(workflow_path)
        validation = workflow.validate()
        
        if validation["valid"]:
            print("✅ 워크플로우 검증 성공!")
            print(f"   - 노드 수: {len(workflow.nodes)}")
            print(f"   - 연결 수: {len(workflow.connections)}")
            
            execution_order = workflow.get_execution_order()
            print(f"   - 실행 순서: {' -> '.join(execution_order)}")
        else:
            print("❌ 워크플로우 검증 실패:")
            for error in validation["errors"]:
                print(f"   🔴 {error}")
            
            if validation["warnings"]:
                print("⚠️ 경고:")
                for warning in validation["warnings"]:
                    print(f"   🟡 {warning}")
            
            sys.exit(1)
    
    except Exception as e:
        print(f"❌ 검증 중 오류 발생: {e}")
        sys.exit(1)


def handle_visualize(args):
    """워크플로우 시각화 처리"""
    workflow_path = Path(args.workflow)
    
    if not workflow_path.exists():
        print(f"❌ 워크플로우 파일을 찾을 수 없습니다: {workflow_path}")
        sys.exit(1)
    
    try:
        workflow = GilWorkflow.from_yaml(workflow_path)
        diagram = workflow.visualize(args.output)
        
        print(f"📊 워크플로우 다이어그램 생성됨: {args.output}")
        print(f"🔗 Mermaid 다이어그램:")
        print(diagram)
        
    except Exception as e:
        print(f"❌ 시각화 중 오류 발생: {e}")
        sys.exit(1)


def handle_list_nodes():
    """노드 목록 출력"""
    factory = NodeFactory()
    node_types = factory.get_available_nodes()
    
    print(f"🧩 사용 가능한 노드 타입 ({len(node_types)}개):")
    print()
    
    categories = {
        "AI 커넥터": [n for n in node_types if "Connector" in n],
        "생성 노드": [n for n in node_types if "Gen" in n],
        "분석 노드": [n for n in node_types if "Analyze" in n],
        "유틸리티": [n for n in node_types if "Util" in n or n not in 
                    [nt for cat_nodes in [
                        [n for n in node_types if "Connector" in n],
                        [n for n in node_types if "Gen" in n],
                        [n for n in node_types if "Analyze" in n]
                    ] for nt in cat_nodes]]
    }
    
    for category, nodes in categories.items():
        if nodes:
            print(f"📁 {category}:")
            for node in sorted(nodes):
                print(f"   - {node}")
            print()


def handle_describe(args):
    """노드 상세 정보 출력"""
    factory = NodeFactory()
    node_info = factory.get_node_info(args.node_type)
    
    if "error" in node_info:
        print(f"❌ {node_info['error']}")
        sys.exit(1)
    
    print(f"🧩 {node_info['type']} 노드 정보")
    print(f"📝 설명: {node_info['description']}")
    print()
    
    if "input_ports" in node_info:
        print("📥 입력 포트:")
        for port in node_info["input_ports"]:
            required = "필수" if port["required"] else "선택"
            print(f"   - {port['name']} ({port['type']}, {required}): {port['description']}")
        print()
    
    if "output_ports" in node_info:
        print("📤 출력 포트:")
        for port in node_info["output_ports"]:
            print(f"   - {port['name']} ({port['type']}): {port['description']}")
        print()


async def handle_generate(args):
    """빠른 생성 처리"""
    if not args.image:
        print("❌ --image 옵션으로 프롬프트를 제공해주세요")
        sys.exit(1)
    
    import os
    from ..connectors.openai_connector import OpenAIConnector
    from ..generators.image_generator import ImageGenerator
    
    api_key = args.api_key or os.getenv("OPENAI_API_KEY")
    if not api_key:
        print("❌ OpenAI API 키가 필요합니다")
        print("   --api-key 옵션이나 OPENAI_API_KEY 환경변수를 설정해주세요")
        sys.exit(1)
    
    try:
        print(f"🎨 이미지 생성 중: '{args.image}'")
        
        connector = OpenAIConnector(node_id="openai_connector", node_config={"api_key": api_key})
        generator = ImageGenerator(node_id="image_generator", node_config={"connector": connector})
        
        result = await generator.generate(
            prompt=args.image,
            size=args.size
        )
        
        if result.get("error"):
            print(f"❌ 생성 실패: {result['error']}")
            sys.exit(1)
        
        images = result.get("images", [])
        if images:
            print(f"✅ 이미지 생성 완료!")
            for i, img in enumerate(images):
                print(f"   이미지 {i+1}: {img['url']}")
                if img.get('revised_prompt'):
                    print(f"   개선된 프롬프트: {img['revised_prompt']}")
        else:
            print("❌ 이미지가 생성되지 않았습니다")
    
    except Exception as e:
        print(f"❌ 오류 발생: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
