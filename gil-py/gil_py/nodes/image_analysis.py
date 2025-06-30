from gil_py.core.node import Node
from gil_py.core.port import InputPort, OutputPort
from gil_py.core.data_types import DataType

class ImageAnalysisNode(Node):
    def __init__(self, node_id: str, node_config: dict = None):
        super().__init__(node_id=node_id, node_config=node_config)
        self.add_input_port(InputPort(name="image_path", data_type=DataType.TEXT))
        self.add_output_port(OutputPort(name="analysis_result", data_type=DataType.TEXT))

    async def execute(self):
        image_path = self.get_input_port("image_path").get_data()
        # Placeholder for actual image analysis logic
        analysis_result = f"Image at {image_path} analyzed: This is a placeholder result."
        self.get_output_port("analysis_result").set_data(analysis_result)
