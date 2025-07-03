from gil_py.core.node import Node
from gil_py.core.port import Port
from gil_py.core.data_types import DataType
from gil_py.core.context import Context

class UtilLogMessageNode(Node):
    """
    Logs the input data to the console for debugging purposes.
    It takes any data type and passes it through to the output.
    """

    def __init__(self, node_id: str, node_config: dict):
        super().__init__(node_id, node_config)

        self.add_input_port(Port(
            name="input",
            data_type=DataType.ANY,
            description="The data to be logged.",
            required=True
        ))
        self.add_output_port(Port(
            name="output",
            data_type=DataType.ANY,
            description="The same data that was logged."
        ))

    async def execute(self, data: dict, context: Context) -> dict:
        """
        Prints the input data to the console and returns it.
        """
        input_data = data.get("input")
        
        log_prefix = self.node_config.get("prefix", f"[{self.node_id}]")
        print(f"{log_prefix} {input_data}")
        
        return {"output": input_data}
