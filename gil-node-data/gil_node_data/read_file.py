from gil_py.core.node import Node
from gil_py.core.port import Port
from gil_py.core.data_types import DataType

class DataReadFileNode(Node):
    """
    Reads content from a specified file path.
    Currently supports reading text files.
    """

    def __init__(self, node_id: str, node_config: dict):
        super().__init__(node_id, node_config)

        self.add_input_port(Port(
            name="file_path",
            data_type=DataType.TEXT,
            description="The absolute path to the file to read.",
            required=True
        ))
        self.add_output_port(Port(
            name="content",
            data_type=DataType.TEXT,
            description="The content read from the file."
        ))

    def execute(self, data: dict) -> dict:
        """
        Reads the file content and returns it.
        """
        file_path = data.get("file_path")

        if not file_path:
            raise ValueError("File path is not provided.")

        try:
            with open(file_path, 'r', encoding='utf-8') as f:
                content = f.read()
            return {"content": content}
        except FileNotFoundError:
            raise FileNotFoundError(f"File not found at: {file_path}")
        except Exception as e:
            raise RuntimeError(f"Failed to read file {file_path}: {e}") from e
