from gil_py.core.node import Node
from gil_py.core.port import Port
from gil_py.core.data_types import DataType
from gil_py.core.context import Context

class ControlBranchNode(Node):
    """
    Directs execution flow based on a boolean condition.
    If the condition is true, the input data is passed to the 'true_output'.
    Otherwise, it is passed to the 'false_output'.
    """

    def __init__(self, node_id: str, node_config: dict):
        super().__init__(node_id, node_config)

        self.add_input_port(Port(
            name="condition",
            data_type=DataType.BOOLEAN,
            description="The boolean value to determine the branch.",
            required=True
        ))
        self.add_input_port(Port(
            name="input",
            data_type=DataType.ANY,
            description="The data to pass through to the selected branch.",
            required=True
        ))
        
        self.add_output_port(Port(
            name="true_output",
            data_type=DataType.ANY,
            description="Output for when the condition is true."
        ))
        self.add_output_port(Port(
            name="false_output",
            data_type=DataType.ANY,
            description="Output for when the condition is false."
        ))

    async def execute(self, data: dict, context: Context) -> dict:
        """
        Evaluates the condition and routes the input data to the appropriate output port.
        """
        condition = data.get("condition")
        input_data = data.get("input")

        if condition:
            return {"true_output": input_data, "false_output": None}
        else:
            return {"true_output": None, "false_output": input_data}
