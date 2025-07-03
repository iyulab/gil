import pytest
from unittest.mock import MagicMock


from gil_node_text.openai_text_generation import OpenAIGenerateTextNode
from gil_node_openai.openai_image_generator import OpenAIGenerateImageNode
from gil_py.core.context import Context

@pytest.mark.asyncio
async def test_text_generation_node_basic():
    node = OpenAIGenerateTextNode(node_id="test_text_gen_node", node_config={})
    context = Context({})
    mock_client = MagicMock()
    mock_client.chat.completions.create.return_value.choices[0].message.content = "world"
    input_data = {"prompt": "world", "client": mock_client}

    await node.execute(input_data, context)

    assert node.get_output_port("generated_text").get_data() == "world"

@pytest.mark.asyncio
async def test_text_generation_node_with_prefix_suffix():
    node = OpenAIGenerateTextNode(node_id="test_text_gen_node_prefix_suffix", node_config={"prefix": "Hello, ", "suffix": "!"})
    context = Context({})
    mock_client = MagicMock()
    mock_client.chat.completions.create.return_value.choices[0].message.content = "Hello, Gil-Flow!"
    input_data = {"prompt": "Gil-Flow", "client": mock_client}

    await node.execute(input_data, context)

    assert node.get_output_port("generated_text").get_data() == "Hello, Gil-Flow!"

@pytest.mark.asyncio
async def test_image_analysis_node_placeholder():
    node = OpenAIGenerateImageNode(node_id="test_image_analysis_node", node_config={})
    context = Context({})
    mock_client = MagicMock()
    mock_client.images.generate.return_value.data[0].url = "Image at /path/to/image.jpg analyzed: This is a placeholder result."
    input_data = {"prompt": "/path/to/image.jpg", "client": mock_client}

    await node.execute(input_data, context)

    expected_result = "Image at /path/to/image.jpg analyzed: This is a placeholder result."
    assert node.get_output_port("image_url").get_data() == expected_result