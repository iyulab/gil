from gil_py.core.node import Node
from gil_py.core.port import Port
from gil_py.core.data_types import DataType

class DataTransformNode(Node):
    """
    Transforms input data using a specified transformation logic.
    The transformation logic can be defined in the node's configuration.
    """

    def __init__(self, node_id: str, node_config: dict):
        super().__init__(node_id, node_config)

        self.transform_expression = self.node_config.get("transform_expression")
        if not self.transform_expression:
            raise ValueError(f"Missing 'transform_expression' in config for {self.node_id}")

        self.add_input_port(Port(
            name="input_data",
            data_type=DataType.ANY,
            description="The data to be transformed.",
            required=True
        ))
        self.add_output_port(Port(
            name="output_data",
            data_type=DataType.ANY,
            description="The transformed data."
        ))

    def execute(self, data: dict) -> dict:
        """
        Applies the transformation expression to the input data.
        """
        input_data = data.get("input_data")

        try:
            # For simplicity, we'll use eval. In a real scenario, consider safer alternatives
            # like a sandboxed environment or a dedicated transformation engine.
            transformed_data = eval(self.transform_expression, {'data': input_data})
            return {"output_data": transformed_data}
        except Exception as e:
            raise RuntimeError(f"Failed to transform data with expression '{self.transform_expression}': {e}") from e
