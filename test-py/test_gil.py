#!/usr/bin/env python3
"""
Gil 워크플로우 테스트 실행기

사용법:
    py test_gil.py generate-image.yaml
"""

import os
import sys
import asyncio
import aiohttp
from pathlib import Path
from datetime import datetime

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

async def download_image(url: str, filepath: Path):
    """이미지 다운로드"""
    try:
        async with aiohttp.ClientSession() as session:
            async with session.get(url) as response:
                if response.status == 200:
                    with open(filepath, 'wb') as f:
                        async for chunk in response.content.iter_chunked(8192):
                            f.write(chunk)
                    print(f"💾 이미지 저장: {filepath}")
                    return True
                else:
                    print(f"❌ 이미지 다운로드 실패: HTTP {response.status}")
                    return False
    except Exception as e:
        print(f"❌ 이미지 다운로드 오류: {e}")
        return False

async def run_workflow(yaml_file: str):
    """워크플로우 실행"""
    try:
        # gil-py 모듈 import
        from gil_py.workflow.workflow import GilWorkflow
        
        # 환경 변수 로드
        load_env()
        
        # API 키 확인
        api_key = os.getenv("OPENAI_API_KEY")
        if not api_key:
            print("❌ OPENAI_API_KEY 환경 변수가 설정되지 않았습니다.")
            print("   .env 파일에 OPENAI_API_KEY를 설정해주세요.")
            return False
        
        print(f"🚀 워크플로우 실행: {yaml_file}")
        
        # 워크플로우 로드 및 실행
        workflow = GilWorkflow.from_yaml(yaml_file)
        print(f"📋 워크플로우: {workflow.name}")
          # 실행
        print("⏳ 실행 중...")
        result = await workflow.run()
        
        # 결과가 dict인 경우 처리
        if isinstance(result, dict):
            success = result.get('success', True)  # 기본적으로 성공으로 간주
            if success:
                print("✅ 워크플로우 실행 성공!")
                  # 결과 출력  
                print(f"📊 실행된 노드 수: {len(result)}")
                
                # 디버깅: 전체 결과 구조 출력
                print("� 디버깅 - 전체 결과:")
                for key, value in result.items():
                    print(f"  {key}: {type(value)} - {str(value)[:200]}...")
                
                # 이미지 다운로드
                await save_images_from_dict(result)
                
                return True
            else:
                error = result.get('error', '알 수 없는 오류')
                print(f"❌ 워크플로우 실행 실패: {error}")
                return False
        else:
            # GilResult 객체인 경우
            if result.success:
                print("✅ 워크플로우 실행 성공!")
                
                # 결과 출력
                print(f"📊 실행 시간: {result.execution_time:.2f}초")
                print(f"📊 실행된 노드 수: {len(result.node_results)}")
                
                # 이미지 다운로드
                await save_images(result)
                
                return True
            else:
                print(f"❌ 워크플로우 실행 실패: {result.error}")
                return False
            
    except ImportError as e:
        print(f"❌ Gil-Py 모듈을 찾을 수 없습니다: {e}")
        print("   gil-py 폴더가 상위 디렉토리에 있는지 확인해주세요.")
        return False
    except Exception as e:
        print(f"❌ 실행 중 오류 발생: {e}")
        return False

async def save_images(result):
    """생성된 이미지 저장"""
    try:
        # 출력 디렉토리 생성
        output_dir = Path(__file__).parent / "generated_images"
        output_dir.mkdir(exist_ok=True)
        
        # 이미지 생성 노드 결과 찾기
        image_result = None
        for node_name, node_result in result.node_results.items():
            if "images" in node_result:
                image_result = node_result
                break
        
        if not image_result or "images" not in image_result:
            print("⚠️ 생성된 이미지를 찾을 수 없습니다.")
            return
        
        images = image_result["images"]
        if not images:
            print("⚠️ 이미지가 생성되지 않았습니다.")
            return
        
        print(f"🎨 생성된 이미지 수: {len(images)}")
        
        # 각 이미지 다운로드
        for i, image_info in enumerate(images):
            if "url" in image_info:
                image_url = image_info["url"]
                timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
                filename = f"generated_image_{i+1}_{timestamp}.png"
                filepath = output_dir / filename
                
                print(f"⬇️ 이미지 {i+1} 다운로드 중...")
                await download_image(image_url, filepath)
        
        # 프롬프트 정보 출력
        if "prompt" in image_result:
            print(f"📝 사용된 프롬프트: {image_result['prompt']}")
            
    except Exception as e:
        print(f"❌ 이미지 저장 중 오류: {e}")

async def save_images_from_dict(result_dict):
    """Dict 형태 결과에서 이미지 저장"""
    try:
        # 출력 디렉토리 생성
        output_dir = Path(__file__).parent / "generated_images"
        output_dir.mkdir(exist_ok=True)
        
        # 이미지 생성 노드 결과 찾기        image_result = None
        for node_name, node_result in result_dict.items():
            if isinstance(node_result, dict) and "images" in node_result:
                image_result = node_result
                break
        
        if not image_result or "images" not in image_result:
            print("⚠️ 생성된 이미지를 찾을 수 없습니다.")
            print("📋 결과 구조:")
            for node_name, node_result in result_dict.items():
                print(f"  - {node_name}: {type(node_result)} {list(node_result.keys()) if isinstance(node_result, dict) else str(node_result)[:100]}")
            return
        
        images = image_result["images"]
        if not images:
            print("⚠️ 이미지가 생성되지 않았습니다.")
            return
        
        print(f"🎨 생성된 이미지 수: {len(images)}")
        
        # 각 이미지 다운로드
        for i, image_info in enumerate(images):
            if "url" in image_info:
                image_url = image_info["url"]
                timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
                filename = f"generated_image_{i+1}_{timestamp}.png"
                filepath = output_dir / filename
                
                print(f"⬇️ 이미지 {i+1} 다운로드 중...")
                await download_image(image_url, filepath)
        
        # 프롬프트 정보 출력
        if "prompt" in image_result:
            print(f"📝 사용된 프롬프트: {image_result['prompt']}")
            
    except Exception as e:
        print(f"❌ 이미지 저장 중 오류: {e}")

def main():
    """메인 함수"""
    if len(sys.argv) != 2:
        print("사용법: py test_gil.py <yaml_file>")
        print("예시: py test_gil.py generate-image.yaml")
        sys.exit(1)
    
    yaml_file = sys.argv[1]
    
    # 파일 존재 확인
    if not Path(yaml_file).exists():
        print(f"❌ 파일을 찾을 수 없습니다: {yaml_file}")
        sys.exit(1)
    
    # 워크플로우 실행
    success = asyncio.run(run_workflow(yaml_file))
    
    if success:
        print("\n🎉 테스트 완료!")
        sys.exit(0)
    else:
        print("\n💥 테스트 실패!")
        sys.exit(1)

if __name__ == "__main__":
    main()
