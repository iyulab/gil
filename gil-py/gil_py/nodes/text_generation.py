from gil_py.core.node import Node
from gil_py.core.port import InputPort, OutputPort
from gil_py.core.data_types import DataType

class TextGenerationNode(Node):
    def __init__(self, node_id: str, node_config: dict = None):
        super().__init__(node_id=node_id, node_config=node_config)
        self.add_input_port(InputPort(name="input_text", data_type=DataType.TEXT))
        self.add_output_port(OutputPort(name="generated_text", data_type=DataType.TEXT))

    async def execute(self):
        input_text = self.get_input_port("input_text").get_data()
        prefix = self.node_config.get("prefix", "")
        suffix = self.node_config.get("suffix", "")

        generated_text = f"{prefix}{input_text}{suffix}"
        self.get_output_port("generated_text").set_data(generated_text)
