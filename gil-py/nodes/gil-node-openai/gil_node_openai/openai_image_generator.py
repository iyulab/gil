
from gil_py.core.node import Node
from gil_py.core.port import InputPort, OutputPort
from gil_py.core.data_types import DataType
from gil_py.core.context import Context

class OpenAIGenerateImageNode(Node):
    """
    Generates an image using the OpenAI DALL-E model based on a text prompt.
    This node requires a connection to an OpenAIConnector.
    """

    def __init__(self, node_id: str, node_config: dict):
        super().__init__(node_id=node_id, node_config=node_config)
        
        # Define ports
        self.add_input_port(InputPort(
            name="client",
            data_type=DataType.ANY,
            description="The OpenAI client instance from an OpenAIConnector.",
            required=True
        ))
        self.add_input_port(InputPort(
            name="prompt",
            data_type=DataType.TEXT,
            description="The text prompt to generate the image from.",
            required=True
        ))
        self.add_output_port(OutputPort(
            name="image_url",
            data_type=DataType.TEXT,
            description="The URL of the generated image."
        ))

    async def execute(self, data: dict, context: Context) -> dict:
        """
        Receives a client and a prompt, then calls the DALL-E API to generate an image.
        """
        client = data.get("client")
        prompt = data.get("prompt")

        if not client:
            raise ValueError("OpenAI client is not provided. Connect an OpenAIConnector.")
        if not prompt:
            raise ValueError("Prompt is not provided.")

        try:
            response = client.images.generate(
                model=self.node_config.get("model", "dall-e-3"),
                prompt=prompt,
                size=self.node_config.get("size", "1024x1024"),
                quality=self.node_config.get("quality", "standard"),
                n=1,
            )
            
            image_url = response.data[0].url
            self.get_output_port("image_url").set_data(image_url)
            return {"image_url": image_url}
        
        except Exception as e:
            # It's good practice to handle potential API errors
            raise RuntimeError(f"Failed to generate image: {e}") from e
