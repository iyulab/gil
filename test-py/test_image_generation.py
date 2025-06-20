#!/usr/bin/env python3
"""
이미지 생성 노드 테스트

이 스크립트는 Gil-Py의 이미지 생성 기능을 테스트합니다.
- OpenAI DALL-E 3를 통한 이미지 생성
- 생성된 이미지 저장
- 결과 검증
"""

import os
import sys
import asyncio
import aiohttp
from datetime import datetime
from pathlib import Path

# gil-py 모듈 경로 추가
sys.path.insert(0, str(Path(__file__).parent.parent / "gil-py"))

try:
    from gil_py.workflow.workflow import GilWorkflow
    from gil_py.workflow.yaml_parser import YamlParser
    from gil_py.core.data_types import GilResult
except ImportError as e:
    print(f"❌ Gil-Py 모듈을 찾을 수 없습니다: {e}")
    print("현재 디렉토리에서 gil-py 폴더를 확인해주세요.")
    sys.exit(1)

class ImageGenerationTester:
    """이미지 생성 기능 테스터"""
    
    def __init__(self):
        self.test_dir = Path(__file__).parent
        self.output_dir = self.test_dir / "generated_images"
        self.output_dir.mkdir(exist_ok=True)
        
        # 환경 변수 확인
        self.api_key = os.getenv("OPENAI_API_KEY")
        if not self.api_key:
            raise ValueError("OPENAI_API_KEY 환경 변수가 설정되지 않았습니다.")
    
    async def test_simple_image_generation(self):
        """간단한 이미지 생성 테스트"""
        print("🎨 간단한 이미지 생성 테스트 시작...")
        
        workflow_path = self.test_dir / "workflows" / "simple_image_gen.yaml"
        
        try:
            # 워크플로우 로드
            workflow = GilWorkflow.from_yaml(str(workflow_path))
            print(f"✅ 워크플로우 로드 완료: {workflow.name}")
            
            # 워크플로우 실행
            print("🚀 워크플로우 실행 중...")
            result = await workflow.run()
            
            # 결과 확인
            if result.success:
                print("✅ 이미지 생성 성공!")
                await self._save_generated_images(result, "simple_test")
                self._print_result_summary(result)
                return True
            else:
                print(f"❌ 이미지 생성 실패: {result.error}")
                return False
                
        except Exception as e:
            print(f"❌ 테스트 실행 중 오류 발생: {e}")
            return False
    
    async def test_custom_prompts(self):
        """사용자 정의 프롬프트로 이미지 생성 테스트"""
        print("\n🎨 사용자 정의 프롬프트 테스트 시작...")
        
        test_prompts = [
            "A futuristic city with flying cars, cyberpunk style",
            "A serene Japanese garden with cherry blossoms",
            "Abstract geometric patterns in blue and gold",
            "A cute robot reading a book in a cozy library"
        ]
        
        results = []
        
        for i, prompt in enumerate(test_prompts, 1):
            print(f"\n📝 테스트 {i}/{len(test_prompts)}: {prompt}")
            
            try:
                # 동적 워크플로우 생성
                workflow_config = self._create_dynamic_workflow(prompt)
                workflow = GilWorkflow.from_dict(workflow_config)
                
                # 실행
                result = await workflow.run()
                
                if result.success:
                    print(f"✅ 이미지 {i} 생성 성공")
                    await self._save_generated_images(result, f"custom_test_{i}")
                    results.append(True)
                else:
                    print(f"❌ 이미지 {i} 생성 실패: {result.error}")
                    results.append(False)
                    
            except Exception as e:
                print(f"❌ 테스트 {i} 실행 중 오류: {e}")
                results.append(False)
        
        success_count = sum(results)
        print(f"\n📊 사용자 정의 프롬프트 테스트 결과: {success_count}/{len(test_prompts)} 성공")
        return success_count == len(test_prompts)
    
    def _create_dynamic_workflow(self, prompt):
        """동적 워크플로우 생성"""
        return {
            "name": "Dynamic Image Generation",
            "description": f"Generate image with prompt: {prompt}",
            "environment": {
                "OPENAI_API_KEY": self.api_key
            },
            "nodes": {
                "openai_connector": {
                    "type": "GilConnectorOpenAI",
                    "config": {
                        "api_key": self.api_key
                    }
                },
                "image_generator": {
                    "type": "GilGenImage",
                    "config": {
                        "connector": "@openai_connector"
                    },
                    "inputs": {
                        "prompt": prompt,
                        "size": "1024x1024",
                        "quality": "standard",
                        "style": "vivid"
                    }
                }
            },
            "flow": [
                "openai_connector",
                "image_generator"
            ],
            "outputs": {
                "generated_images": {
                    "source": "@image_generator.images"
                }
            }
        }
    
    async def _save_generated_images(self, result: GilResult, test_name: str):
        """생성된 이미지 저장"""
        try:
            # 결과에서 이미지 URL 추출
            image_data = result.get_node_result("image_generator")
            if not image_data or "images" not in image_data:
                print("⚠️ 생성된 이미지 데이터를 찾을 수 없습니다.")
                return
            
            images = image_data["images"]
            if not images:
                print("⚠️ 생성된 이미지가 없습니다.")
                return
            
            # 각 이미지 다운로드 및 저장
            for i, image_info in enumerate(images):
                if "url" in image_info:
                    image_url = image_info["url"]
                    filename = f"{test_name}_{i+1}_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"
                    filepath = self.output_dir / filename
                    
                    await self._download_image(image_url, filepath)
                    print(f"💾 이미지 저장: {filepath}")
                    
        except Exception as e:
            print(f"❌ 이미지 저장 중 오류: {e}")
    
    async def _download_image(self, url: str, filepath: Path):
        """이미지 다운로드"""
        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(url) as response:
                    if response.status == 200:
                        with open(filepath, 'wb') as f:
                            async for chunk in response.content.iter_chunked(8192):
                                f.write(chunk)
                    else:
                        print(f"⚠️ 이미지 다운로드 실패: HTTP {response.status}")
        except Exception as e:
            print(f"❌ 이미지 다운로드 오류: {e}")
    
    def _print_result_summary(self, result: GilResult):
        """결과 요약 출력"""
        print("\n📋 실행 결과 요약:")
        print(f"  • 성공: {result.success}")
        print(f"  • 실행 시간: {result.execution_time:.2f}초")
        print(f"  • 실행된 노드 수: {len(result.node_results)}")
        
        # 이미지 생성 노드 결과 상세
        image_result = result.get_node_result("image_generator")
        if image_result:
            print(f"  • 생성된 이미지 수: {len(image_result.get('images', []))}")
            if 'prompt' in image_result:
                print(f"  • 사용된 프롬프트: {image_result['prompt']}")
    
    async def run_all_tests(self):
        """모든 테스트 실행"""
        print("🚀 Gil-Py 이미지 생성 노드 테스트 시작")
        print("=" * 50)
        
        # 출력 디렉토리 정보
        print(f"📁 이미지 저장 경로: {self.output_dir}")
        
        # 테스트 실행
        test_results = []
        
        # 1. 간단한 이미지 생성 테스트
        result1 = await self.test_simple_image_generation()
        test_results.append(("간단한 이미지 생성", result1))
        
        # 2. 사용자 정의 프롬프트 테스트
        result2 = await self.test_custom_prompts()
        test_results.append(("사용자 정의 프롬프트", result2))
        
        # 최종 결과
        print("\n" + "=" * 50)
        print("🏁 테스트 완료 결과:")
        
        success_count = 0
        for test_name, success in test_results:
            status = "✅ 성공" if success else "❌ 실패"
            print(f"  • {test_name}: {status}")
            if success:
                success_count += 1
        
        print(f"\n📊 전체 결과: {success_count}/{len(test_results)} 테스트 성공")
        
        # 생성된 파일 목록
        generated_files = list(self.output_dir.glob("*.png"))
        if generated_files:
            print(f"\n🖼️ 생성된 이미지 파일 ({len(generated_files)}개):")
            for file in sorted(generated_files):
                print(f"  • {file.name}")
        
        return success_count == len(test_results)


async def main():
    """메인 함수"""
    try:
        # 환경 변수 로드
        env_path = Path(__file__).parent / ".env"
        if env_path.exists():
            from dotenv import load_dotenv
            load_dotenv(env_path)
        
        # 테스터 생성 및 실행
        tester = ImageGenerationTester()
        success = await tester.run_all_tests()
        
        # 종료 코드 설정
        sys.exit(0 if success else 1)
        
    except Exception as e:
        print(f"❌ 테스트 실행 중 치명적 오류 발생: {e}")
        sys.exit(1)


if __name__ == "__main__":
    asyncio.run(main())
