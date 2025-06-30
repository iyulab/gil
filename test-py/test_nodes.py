import pytest
import asyncio


from gil_py.nodes.text_generation import TextGenerationNode
from gil_py.nodes.image_analysis import ImageAnalysisNode
from gil_py.core.context import Context

@pytest.mark.asyncio
async def test_text_generation_node_basic():
    node = TextGenerationNode(node_id="test_text_gen_node", config={})
    node.get_input_port("input_text").set_data("world")

    context = Context({})
    node.set_contexts(None, context) # NodeContext is not strictly needed for this test

    await node.execute()

    assert node.get_output_port("generated_text").get_data() == "world"

@pytest.mark.asyncio
async def test_text_generation_node_with_prefix_suffix():
    node = TextGenerationNode(node_id="test_text_gen_node_prefix_suffix", config={"prefix": "Hello, ", "suffix": "!"})
    node.get_input_port("input_text").set_data("Gil-Flow")

    context = Context({})
    node.set_contexts(None, context)

    await node.execute()

    assert node.get_output_port("generated_text").get_data() == "Hello, Gil-Flow!"

@pytest.mark.asyncio
async def test_image_analysis_node_placeholder():
    node = ImageAnalysisNode(node_id="test_image_analysis_node", node_config={})
    node.get_input_port("image_path").set_data("/path/to/image.jpg")

    context = Context({})
    node.set_contexts(None, context)

    await node.execute()

    expected_result = "Image at /path/to/image.jpg analyzed: This is a placeholder result."
    assert node.get_output_port("analysis_result").get_data() == expected_result
