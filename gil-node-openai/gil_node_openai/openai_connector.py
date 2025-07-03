
import os
from openai import OpenAI
from gil_py.core.node import Node
from gil_py.core.port import Port
from gil_py.core.data_types import DataType

class OpenAIConnectorNode(Node):
    """
    Manages the connection and authentication with the OpenAI API.
    It initializes the OpenAI client and provides it through an output port.
    """

    def __init__(self, node_id: str, node_config: dict):
        super().__init__(node_id, node_config)
        self.client = None
        
        # Define ports
        self.add_output_port(Port(
            name="client",
            data_type=DataType.OBJECT,
            description="The initialized OpenAI client instance."
        ))

        # Initialize client on creation
        self._initialize_client()

    def _initialize_client(self):
        """Initializes the OpenAI client using provided config or environment variables."""
        api_key = self.node_config.get("api_key")
        
        # If api_key is specified as an env var like ${VAR_NAME}
        if api_key and api_key.startswith("${") and api_key.endswith("}"):
            env_var_name = api_key[2:-1]
            api_key = os.getenv(env_var_name)
        
        # Fallback to default environment variable if no key is provided
        if not api_key:
            api_key = os.getenv("OPENAI_API_KEY")

        if not api_key:
            raise ValueError(f"API key for {self.node_id} is not found. Please provide it in the node config or set the OPENAI_API_KEY environment variable.")

        self.client = OpenAI(
            api_key=api_key,
            organization=self.node_config.get("organization"),
            base_url=self.node_config.get("base_url"),
        )

    def execute(self, data: dict) -> dict:
        """Provides the initialized client to the output port."""
        return {"client": self.client}
