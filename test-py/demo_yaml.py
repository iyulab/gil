#!/usr/bin/env python3
"""
Gil YAML 워크플로우 데모
"""

import os
import sys
from pathlib import Path

# Gil 라이브러리 import 경로 설정
sys.path.insert(0, str(Path(__file__).parent.parent / "gil-py"))

# 환경 변수 로드
from dotenv import load_dotenv
load_dotenv()

def demo_yaml_parsing():
    """YAML 파싱 데모"""
    print("🎯 Gil YAML 워크플로우 데모")
    print("=" * 50)
    
    # YAML 파서 import
    from gil_py.workflow.yaml_parser import YamlWorkflowParser
    
    # 테스트할 YAML 파일
    yaml_files = [
        "workflows/simple_image_gen.yaml",
        "workflows/data_pipeline.yaml", 
        "workflows/conditional_flow.yaml",
        "workflows/parallel_flow.yaml"
    ]
    
    parser = YamlWorkflowParser()
    
    for yaml_file in yaml_files:
        yaml_path = Path(yaml_file)
        if not yaml_path.exists():
            print(f"⏭️  건너뜀: {yaml_file} (파일 없음)")
            continue
            
        print(f"\n{'='*40}")
        print(f"📄 {yaml_path.name}")
        print(f"{'='*40}")
        
        try:
            config = parser.parse_file(str(yaml_path))
            
            print(f"📝 이름: {config.name}")
            print(f"📝 설명: {config.description}")
            print(f"🔧 노드 수: {len(config.nodes)}")
            print(f"🔄 실행 순서: {config.flow}")
            print(f"🌍 환경변수: {list(config.environment.keys())}")
            print(f"📤 출력: {list(config.outputs.keys())}")
            
            print("\n🧩 노드 목록:")
            for node_name, node_config in config.nodes.items():
                condition_text = f" (조건: {node_config.condition})" if node_config.condition else ""
                print(f"  • {node_name}: {node_config.type}{condition_text}")
                
                if node_config.inputs:
                    print(f"    📥 입력: {list(node_config.inputs.keys())}")
                if node_config.config:
                    print(f"    ⚙️  설정: {list(node_config.config.keys())}")
            
            print("✅ 파싱 성공!")
            
        except Exception as e:
            print(f"❌ 파싱 실패: {e}")
    
    print(f"\n{'='*50}")
    print("🎉 YAML 워크플로우 파싱 데모 완료!")
    print(f"{'='*50}")
    
    # API 키 확인
    api_key = os.getenv("OPENAI_API_KEY")
    if api_key:
        print("\n✅ OpenAI API 키가 설정되어 있습니다.")
        print("   실제 워크플로우 실행이 가능합니다!")
    else:
        print("\n⚠️  OpenAI API 키가 설정되지 않았습니다.")
        print("   .env 파일에 OPENAI_API_KEY를 설정하면 실제 실행이 가능합니다.")
    
    print("\n📖 다음 단계:")
    print("  1. 워크플로우 YAML 파일 편집")
    print("  2. 새로운 노드 타입 추가") 
    print("  3. CLI 인터페이스 완성")
    print("  4. 실제 워크플로우 실행 테스트")

if __name__ == "__main__":
    demo_yaml_parsing()
