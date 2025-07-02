from gil_py.core.node import Node
from gil_py.core.port import Port, PortType
from gil_py.core.context import GilContext

class UtilSetVariableNode(Node):
    """
    Sets a variable in the workflow context.
    The variable can be accessed by other nodes using context expressions.
    """

    def __init__(self, node_id: str, node_config: dict):
        super().__init__(node_id, node_config)

        self.variable_name = self.node_config.get("variable_name")
        if not self.variable_name:
            raise ValueError(f"Missing 'variable_name' in config for {self.node_id}")

        self.add_input_port(Port(
            name="value",
            port_type=PortType.ANY,
            description="The value to set for the variable.",
            is_required=True
        ))
        # This node does not produce a direct output, it modifies the context.

    def execute(self, data: dict, context: GilContext) -> dict:
        """
        Sets the variable in the provided context.
        """
        value_to_set = data.get("value")
        context.set(self.variable_name, value_to_set)
        print(f"Context variable '{self.variable_name}' set to: {value_to_set}")
        return {}
